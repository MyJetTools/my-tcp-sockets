use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use tokio::{net::TcpStream, sync::Mutex};

use tokio::io;

use crate::tcp_connection::TcpContract;
use crate::TcpClientSocketSettings;
use crate::{
    tcp_connection::{FlushToSocketEventLoop, SocketConnection},
    ConnectionId, SocketEventCallback, TcpSocketSerializer,
};

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpClient {
    settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    connect_timeout: Duration,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    name: String,
    max_send_payload_size: usize,
    send_timeout: Duration,
    background_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl TcpClient {
    pub fn new(
        name: String,
        settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    ) -> Self {
        Self {
            settings,
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
        TContract: TcpContract + Send + Sync + 'static,
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        let handle = tokio::spawn(connection_loop(
            self.settings.clone(),
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
    settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
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
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let mut connection_id: ConnectionId = 0;

    let mut socket_context = HashMap::new();
    socket_context.insert("SocketName".to_string(), socket_name.clone());

    const LOG_PROCESS: &str = "Tcp Client Connect";
    loop {
        tokio::time::sleep(connect_timeout).await;

        let host_port = settings.get_host_port().await;

        let mut socket_context = socket_context.clone();
        socket_context.insert("HostPort".to_string(), host_port.clone());

        logger.write_info(
            LOG_PROCESS.to_string(),
            format!("Trying to connect to {}", host_port),
            Some(socket_context.clone()),
        );
        let connect_result = TcpStream::connect(host_port.as_str()).await;

        match connect_result {
            Ok(tcp_stream) => {
                logger.write_info(
                    LOG_PROCESS.to_string(),
                    format!("Connected to {}. Id: {}", host_port, connection_id),
                    Some(socket_context.clone()),
                );

                let (read_socket, write_socket) = io::split(tcp_stream);

                let mut log_context = HashMap::new();
                log_context.insert("ConnectionId".to_string(), connection_id.to_string());
                log_context.insert("SocketName".to_string(), socket_name.to_string());

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    serializer_factory(),
                    connection_id,
                    None,
                    logger.clone(),
                    max_send_payload_size,
                    send_timeout,
                    log_context,
                    disconnect_timeout,
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

                crate::tcp_connection::new_connection::start(
                    read_socket,
                    connection.clone(),
                    read_serializer,
                    socket_callback.clone(),
                    Some(seconds_to_ping),
                    logger.clone(),
                )
                .await;

                logger.write_info(
                    LOG_PROCESS.to_string(),
                    format!("Disconnected from {}", host_port),
                    Some(socket_context.clone()),
                );
                connection_id += 1;
            }
            Err(err) => {
                logger.write_error(
                    LOG_PROCESS.to_string(),
                    format!("Can not connect to {}. Reason: {}", host_port, err),
                    Some(socket_context.clone()),
                );
            }
        }
    }
}
