use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};
use tokio::{io::ReadHalf, net::TcpStream};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::SocketConnection,
    ConnectionEvent, SocketEventCallback, TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer, TSocketCallback>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_context: Option<String>,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    socket_callback
        .handle(ConnectionEvent::Connected(connection.clone()))
        .await;

    let read_result = tokio::spawn(read_loop(
        read_socket,
        connection.clone(),
        read_serializer,
        socket_callback.clone(),
    ))
    .await;

    if let Err(err) = read_result {
        logger.write_error(
            "Socket Read Loop".to_string(),
            format!("Socket {} loop exit with error: {}", connection.id, err),
            socket_context.clone(),
        );
    }

    connection.send_to_socket_event_loop.stop();
    socket_callback
        .handle(ConnectionEvent::Disconnected(connection.clone()))
        .await;
}

async fn read_loop<TContract, TSerializer, TSocketCallback>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    mut read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
) -> Result<(), ReadingTcpContractFail>
where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let mut socket_reader = SocketReaderTcpStream::new(tcp_stream);
    loop {
        socket_reader.start_calculating_read_size();

        let read_future = read_serializer.deserialize(&mut socket_reader);

        let read_result =
            tokio::time::timeout(connection.dead_disconnect_timeout, read_future).await;

        if let Err(_) = &read_result {
            connection.logger.write_info(
                "read_loop".to_string(),
                format!("Read timeout {:?}", connection.dead_disconnect_timeout),
                Some(format!("ConnectionId: {}", connection.id)),
            );

            connection.disconnect().await;
            return Err(ReadingTcpContractFail::SocketDisconnected);
        }

        let contract = read_result.unwrap()?;
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

        socket_callback
            .handle(ConnectionEvent::Payload {
                connection: connection.clone(),
                payload: contract,
            })
            .await;
    }
}
