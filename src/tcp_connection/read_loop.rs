use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};
use tokio::{io::ReadHalf, net::TcpStream};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    ConnectionEvent, SocketEventCallback, TcpContract, TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer, TSocketCallback>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<TcpSocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let socket_callback_spawned = socket_callback.clone();
    let connection_spawned = connection.clone();
    let on_connect_result = tokio::spawn(async move {
        socket_callback_spawned
            .handle(ConnectionEvent::Connected(connection_spawned.clone()))
            .await;
    })
    .await;

    match on_connect_result {
        Ok(_) => {
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
                    format!("Socket Read loop exited with error: {}", err),
                    Some(connection.get_log_context().await),
                );
            }
        }
        Err(err) => {
            logger.write_fatal_error(
                "Socket Read Loop On Connect".to_string(),
                format!("{:?}", err),
                Some(connection.get_log_context().await),
            );
        }
    }

    connection.disconnect().await;

    let connection_spawned = connection.clone();
    let on_disconnect_result = tokio::spawn(async move {
        socket_callback
            .handle(ConnectionEvent::Disconnected(connection_spawned))
            .await;
    })
    .await;

    if let Err(err) = on_disconnect_result {
        logger.write_fatal_error(
            "Socket Read Loop On Disconnect".to_string(),
            format!("{:?}", err),
            Some(connection.get_log_context().await),
        );
    }
}

async fn read_loop<TContract, TSerializer, TSocketCallback>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: Arc<TcpSocketConnection<TContract, TSerializer>>,
    mut read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
) -> Result<(), ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
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
            connection.logger.write_debug_info(
                "read_loop".to_string(),
                format!("Read timeout {:?}", connection.dead_disconnect_timeout),
                Some(connection.get_log_context().await),
            );

            connection.disconnect().await;
            return Err(ReadingTcpContractFail::SocketDisconnected);
        }

        let contract = read_result.unwrap()?;

        if contract.is_pong() {
            connection.statistics().update_ping_pong_statistic();
        }

        connection
            .statistics()
            .update_read_amount(socket_reader.read_size);

        connection
            .statistics()
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
