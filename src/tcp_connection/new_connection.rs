use std::sync::Arc;

use rust_extensions::Logger;
use tokio::{io::ReadHalf, net::TcpStream};

use crate::{SocketEventCallback, TcpSocketSerializer};

use super::{SocketConnection, TcpContract};

pub async fn start<TContract, TSerializer, TSocketCallback>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: Option<usize>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let ping_handle = tokio::spawn(crate::tcp_connection::ping_loop::start(
        connection.clone(),
        seconds_to_ping,
        logger.clone(),
    ));

    let connection_id = connection.id;

    crate::tcp_connection::read_loop::start(
        read_socket,
        connection.clone(),
        read_serializer,
        socket_callback.clone(),
        logger.clone(),
        connection.get_log_context().await,
    )
    .await;

    if let Err(err) = ping_handle.await {
        logger.write_error(
            "Connection handler".to_string(),
            format!(
                "Socket ping {} loop exit with error: {}",
                connection_id, err
            ),
            connection.get_log_context().await,
        );
    };
}
