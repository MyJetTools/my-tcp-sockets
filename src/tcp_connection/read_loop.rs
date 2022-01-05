use std::sync::Arc;

use my_logger::MyLogger;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::ReadHalf, net::TcpStream};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::{ConnectionEvent, SocketConnection},
    TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    logger: Arc<MyLogger>,
    log_context: String,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    connection.callback_event(ConnectionEvent::Connected(connection.clone()));

    let read_result =
        tokio::spawn(read_loop(read_socket, connection.clone(), read_serializer)).await;

    if let Err(err) = read_result {
        logger.write_log(
            my_logger::LogLevel::FatalError,
            "Socket Read Loop".to_string(),
            format!("Socket {} loop exit with error: {}", connection.id, err),
            Some(log_context),
        );
    }

    connection.callback_event(ConnectionEvent::Disconnected(connection.clone()));
}

async fn read_loop<TContract, TSerializer>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    mut read_serializer: TSerializer,
) -> Result<(), ReadingTcpContractFail>
where
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let mut socket_reader = SocketReaderTcpStream::new(tcp_stream);
    loop {
        socket_reader.start_calculating_read_size();

        let contract = read_serializer.deserialize(&mut socket_reader).await?;
        let state_is_changed = read_serializer.apply_packet(&contract);

        if state_is_changed {
            connection.apply_payload_to_serializer(&contract).await;
        }

        connection
            .statistics
            .update_read_amount(socket_reader.read_size);

        connection
            .statistics
            .last_receive_moment
            .update(DateTimeAsMicroseconds::now());

        connection.callback_event(ConnectionEvent::Payload {
            connection: connection.clone(),
            payload: contract,
        });
    }
}
