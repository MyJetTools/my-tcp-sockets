use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::ReadHalf, net::TcpStream};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::{ConnectionEvent, SocketConnection},
    TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract>>,
    serializer: TSerializer,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    connection.callback_event(ConnectionEvent::Connected(connection.clone()));

    let read_result = tokio::spawn(read_loop(read_socket, connection.clone(), serializer)).await;

    if let Err(err) = read_result {
        println!("Socket {} loop exit with error: {}", connection.id, err);
    }

    connection.callback_event(ConnectionEvent::Disconnected(connection.clone()));
}

async fn read_loop<TContract, TSerializer>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract>>,
    serializer: TSerializer,
) -> Result<(), ReadingTcpContractFail>
where
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let mut socket_reader = SocketReaderTcpStream::new(tcp_stream);
    loop {
        socket_reader.start_calculating_read_size();

        let contract = serializer.deserialize(&mut socket_reader).await?;

        connection
            .statistics
            .update_read_amount(socket_reader.read_size);

        connection
            .statistics
            .last_receive_moment
            .update(DateTimeAsMicroseconds::now());

        connection.callback_event(ConnectionEvent::Payload {
            id: connection.clone(),
            payload: contract,
        });
    }
}
