use std::{sync::Arc, time::Duration};

use tokio::{io::ReadHalf, net::TcpStream};

use crate::TcpSocketSerializer;

use super::{ping_loop::PingData, ConnectionEvent, SocketConnection};

pub async fn start<TContract, TSerializer>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract>>,

    serializer: TSerializer,
    ping_data: Option<PingData>,
    disconnect_interval: Duration,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    connection.callback_event(ConnectionEvent::Connected(connection.clone()));

    let ping_handle = tokio::spawn(crate::tcp_connection::ping_loop::start(
        connection.clone(),
        ping_data,
        disconnect_interval,
    ));

    let connection_id = connection.id;

    crate::tcp_connection::read_loop::start(read_socket, connection, serializer).await;

    if let Err(err) = ping_handle.await {
        println!(
            "Socket ping {} loop exit with error: {}",
            connection_id, err
        );
    };
}
