use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use tokio::{net::TcpStream, sync::Mutex};

use tokio::io;

use crate::{
    tcp_connection::{ping_loop::PingData, FlushToSocketEventLoop, SocketConnection},
    ConnectionId, SocketEventCallback, TcpSocketSerializer,
};

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpClient {
    host_port: String,
    connect_timeout: Duration,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    name: String,
    max_send_payload_size: usize,
    send_timeout: Duration,
    background_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl TcpClient {
    pub fn new(name: String, host_port: String) -> Self {
        Self {
            host_port,
            connect_timeout: Duration::from_secs(3),
            seconds_to_ping: 3,
            disconnect_timeout: Duration::from_secs(9),
            name,
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            background_task: Mutex::new(None),
        }
    }

    pub async fn start<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
        &self,
        serializer_factory: Arc<TSerializeFactory>,
        socket_callback: Arc<TSocketCallback>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TContract: Send + Sync + 'static,
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        let handle = tokio::spawn(connection_loop(
            self.host_port.clone(),
            self.connect_timeout,
            serializer_factory,
            socket_callback,
            self.seconds_to_ping,
            self.disconnect_timeout,
            self.name.clone(),
            self.max_send_payload_size,
            self.send_timeout,
            logger,
        ));

        let mut background_task = self.background_task.lock().await;
        background_task.replace(handle);
    }

    pub async fn stop(&self) {
        let mut background_task = self.background_task.lock().await;

        if let Some(background_task) = background_task.take() {
            background_task.abort();
        }
    }
}

async fn connection_loop<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
    host_port: String,
    connect_timeout: Duration,
    serializer_factory: Arc<TSerializeFactory>,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    socket_name: String,
    max_send_payload_size: usize,
    send_timeout: Duration,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let mut connection_id: ConnectionId = 0;

    let socket_context = Some(format!("{}", socket_name));

    const LOG_PROCESS: &str = "Tcp Client Connect";
    loop {
        tokio::time::sleep(connect_timeout).await;

        logger.write_info(
            LOG_PROCESS.to_string(),
            format!("Trying to connect to {}", host_port),
            socket_context.clone(),
        );
        let connect_result = TcpStream::connect(host_port.as_str()).await;

        match connect_result {
            Ok(tcp_stream) => {
                logger.write_info(
                    LOG_PROCESS.to_string(),
                    format!("Connected to {}. Id: {}", host_port, connection_id),
                    socket_context.clone(),
                );

                let (read_socket, write_socket) = io::split(tcp_stream);

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    serializer_factory(),
                    connection_id,
                    None,
                    logger.clone(),
                    max_send_payload_size,
                    send_timeout,
                    socket_context.clone(),
                ));

                connection
                    .send_to_socket_event_loop
                    .register_event_loop(Arc::new(FlushToSocketEventLoop::new(connection.clone())))
                    .await;

                connection
                    .send_to_socket_event_loop
                    .start(connection.connection_state.clone(), logger.clone())
                    .await;

                let read_serializer = serializer_factory();

                let ping_data = PingData {
                    seconds_to_ping,
                    ping_packet: read_serializer.serialize(read_serializer.get_ping()),
                };

                crate::tcp_connection::new_connection::start(
                    read_socket,
                    connection.clone(),
                    read_serializer,
                    socket_callback.clone(),
                    Some(ping_data),
                    disconnect_timeout,
                    logger.clone(),
                    socket_context.clone(),
                )
                .await;

                logger.write_info(
                    LOG_PROCESS.to_string(),
                    format!("Disconnected from {}", host_port),
                    socket_context.clone(),
                );
                connection_id += 1;
            }
            Err(err) => {
                logger.write_error(
                    LOG_PROCESS.to_string(),
                    format!("Can not connect to {}. Reason: {}", host_port, err),
                    socket_context.clone(),
                );
            }
        }
    }
}
