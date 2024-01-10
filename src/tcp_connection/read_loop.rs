use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    ConnectionEvent, SocketEventCallback, TcpContract, TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer, TSocketCallback>(
    socket_reader: SocketReaderTcpStream,
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    read_serializer: TSerializer,
    socket_callback: &Arc<TSocketCallback>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let connection_spawned = connection.clone();
    let socket_callback = socket_callback.clone();

    let logger_spawned = logger.clone();

    let read_result = tokio::spawn(async move {
        let read_result = read_loop(
            socket_reader,
            connection_spawned.clone(),
            read_serializer,
            socket_callback,
        )
        .await;

        if let Err(err) = read_result {
            logger_spawned.write_error(
                "Socket Read Loop".to_string(),
                format!("Socket Read loop exited with error: {:?}", err),
                Some(connection_spawned.get_log_context().await),
            );
        }
    })
    .await;

    if read_result.is_err() {
        logger.write_error(
            "Socket Read Loop".to_string(),
            format!("Socket Read loop exited with panic"),
            Some(connection.get_log_context().await),
        );
    }
}

async fn read_loop<TContract, TSerializer, TSocketCallback>(
    mut socket_reader: SocketReaderTcpStream,
    connection: Arc<TcpSocketConnection<TContract, TSerializer>>,
    mut read_serializer: TSerializer,
    socket_callback: Arc<TSocketCallback>,
) -> Result<(), ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    loop {
        let contract = read_packet(&connection, &mut socket_reader, &mut read_serializer).await?;

        socket_callback
            .handle(ConnectionEvent::Payload {
                connection: connection.clone(),
                payload: contract,
            })
            .await;
    }
}

pub async fn read_packet<TContract, TSerializer>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    socket_reader: &mut SocketReaderTcpStream,
    read_serializer: &mut TSerializer,
) -> Result<TContract, ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    socket_reader.start_calculating_read_size();

    let read_future = read_serializer.deserialize(socket_reader);

    let read_result = tokio::time::timeout(connection.dead_disconnect_timeout, read_future).await;

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

    Ok(contract)
}

pub async fn execute_on_connected<TContract, TSerializer, TSocketCallback>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    socket_callback: &Arc<TSocketCallback>,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
) -> bool
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let socket_callback = socket_callback.clone();
    let connection_spawned = connection.clone();

    let on_connect_result = tokio::spawn(async move {
        socket_callback
            .handle(ConnectionEvent::Connected(connection_spawned.clone()))
            .await;
    })
    .await;

    if let Err(err) = on_connect_result {
        logger.write_fatal_error(
            "Socket Read Loop On Connect".to_string(),
            format!("{:?}", err),
            Some(connection.get_log_context().await),
        );
        return false;
    }

    true
}

pub async fn execute_on_disconnected<TContract, TSerializer, TSocketCallback>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    socket_callback: &Arc<TSocketCallback>,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let connection_spawned = connection.clone();
    let socket_callback = socket_callback.clone();
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
