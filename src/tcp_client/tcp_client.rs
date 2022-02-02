use std::sync::Arc;
use std::time::Duration;

use my_logger::{LogLevel, MyLogger};
use tokio::net::TcpStream;

use tokio::io;

use crate::{
    tcp_connection::{ping_loop::PingData, SocketConnection},
    ConnectionId, SocketEventCallback, TcpSocketSerializer,
};

pub struct TcpClient {
    host_port: String,
    connect_timeout: Duration,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    name: String,
    pub logger: Arc<MyLogger>,
}

impl TcpClient {
    pub fn new(name: String, host_port: String) -> Self {
        Self {
            host_port,
            connect_timeout: Duration::from_secs(3),
            seconds_to_ping: 3,
            disconnect_timeout: Duration::from_secs(9),
            logger: Arc::new(MyLogger::to_concole()),
            name,
        }
    }

    pub fn new_with_logger(name: String, host_port: String, logger: Arc<MyLogger>) -> Self {
        Self {
            host_port,
            connect_timeout: Duration::from_secs(3),
            seconds_to_ping: 3,
            disconnect_timeout: Duration::from_secs(9),
            logger,
            name,
        }
    }

    pub fn start<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
        &self,
        serializer_factory: Arc<TSerializeFactory>,
        socket_callback: Arc<TSocketCallback>,
    ) where
        TContract: Send + Sync + 'static,
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        tokio::spawn(connection_loop(
            self.host_port.clone(),
            self.connect_timeout,
            serializer_factory,
            socket_callback,
            self.seconds_to_ping,
            self.disconnect_timeout,
            self.logger.clone(),
            self.name.clone(),
        ));
    }
}

async fn connection_loop<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
    host_port: String,
    connect_timeout: Duration,
    serializer_factory: Arc<TSerializeFactory>,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    logger: Arc<MyLogger>,
    socket_name: String,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let mut connection_id: ConnectionId = 0;

    const LOG_PROCESS: &str = "Tcp Client Connect";
    loop {
        tokio::time::sleep(connect_timeout).await;

        logger.write_log(
            LogLevel::Info,
            LOG_PROCESS.to_string(),
            format!("Trying to connect to {}", host_port),
            Some(format!(
                "TcpClient:{}. HostPort:{}",
                socket_name.as_str(),
                host_port
            )),
        );
        let connect_result = TcpStream::connect(host_port.as_str()).await;

        match connect_result {
            Ok(tcp_stream) => {
                let log_context = format!("ClientConnection:{}. Id:{}", socket_name, connection_id);

                logger.write_log(
                    LogLevel::Info,
                    LOG_PROCESS.to_string(),
                    format!("Connected to {}. Id: {}", host_port, connection_id),
                    Some(log_context.clone()),
                );

                let (read_socket, write_socket) = io::split(tcp_stream);

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    serializer_factory(),
                    connection_id,
                    None,
                    logger.clone(),
                    log_context.clone(),
                ));

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
                    log_context.clone(),
                )
                .await;

                logger.write_log(
                    LogLevel::Info,
                    LOG_PROCESS.to_string(),
                    format!("Disconnected from {}", host_port),
                    Some(log_context),
                );
                connection_id += 1;
            }
            Err(err) => {
                logger.write_log(
                    LogLevel::Error,
                    LOG_PROCESS.to_string(),
                    format!("Can not connect to {}. Reason: {}", host_port, err),
                    Some(format!("TcpClient:{}. HostPort:{}", socket_name, host_port)),
                );
            }
        }
    }
}
