use std::{sync::Arc, time::Duration};

use my_logger::{LogLevel, MyLogger};
use tokio::{io::ReadHalf, net::TcpStream};

use crate::TcpSocketSerializer;

use super::{ping_loop::PingData, SocketConnection};

pub async fn start<TContract, TSerializer>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    ping_data: Option<PingData>,
    disconnect_timeout: Duration,
    logger: Arc<MyLogger>,
    log_context: String,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let ping_handle = tokio::spawn(crate::tcp_connection::ping_loop::start(
        connection.clone(),
        ping_data,
        disconnect_timeout,
        logger.clone(),
        log_context.clone(),
    ));

    let connection_id = connection.id;

    crate::tcp_connection::read_loop::start(
        read_socket,
        connection,
        read_serializer,
        logger.clone(),
        log_context.clone(),
    )
    .await;

    if let Err(err) = ping_handle.await {
        logger.write_log(
            LogLevel::FatalError,
            "Connection handler".to_string(),
            format!(
                "Socket ping {} loop exit with error: {}",
                connection_id, err
            ),
            Some(log_context),
        );
    };
}
