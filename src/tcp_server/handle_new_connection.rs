use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::Logger;
use tokio::net::TcpStream;

use crate::{
    socket_reader::SocketReaderTcpStream,
    tcp_connection::{TcpSocketConnection, TcpThreadStatus},
    ConnectionId, SocketEventCallback, TcpContract, TcpSerializerFactory, TcpSerializerState,
    TcpSocketSerializer, ThreadsStatistics,
};

pub async fn handle_new_connection<
    TContract,
    TSerializer,
    TSerializerState,
    TTcpSerializerStateFactory,
    TSocketCallback,
>(
    master_socket_name: Arc<String>,
    tcp_stream: TcpStream,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
    connection_id: ConnectionId,
    socket_addr: SocketAddr,
    threads_statistics: &Arc<ThreadsStatistics>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    serializer_factory: Arc<TTcpSerializerStateFactory>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializerState>,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    TTcpSerializerStateFactory:
        TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
{
    let (read_half, write_half) = tcp_stream.into_split();

    #[cfg(feature = "with-tls")]
    let read_half = crate::MaybeTlsReadStream::NoTls(read_half);
    #[cfg(feature = "with-tls")]
    let write_half = crate::MaybeTlsWriteStream::NoTls(write_half);

    let socket_reader = SocketReaderTcpStream::new(read_half.into());

    let connection = TcpSocketConnection::new(
        master_socket_name,
        Some(write_half.into()),
        connection_id,
        Some(socket_addr.into()),
        logger.clone(),
        max_send_payload_size,
        send_timeout,
        Duration::from_secs(20),
        threads_statistics.clone(),
        serializer_factory.create_serializer().await,
        serializer_factory.create_serializer_state().await,
    )
    .await;

    let connection = Arc::new(connection);

    if !crate::tcp_connection::read_loop::execute_on_connected::<
        TContract,
        TSerializer,
        TSerializerState,
        TSocketCallback,
    >(&connection, &socket_callback, &logger)
    .await
    {
        connection.disconnect().await;
        connection.update_read_thread_status(TcpThreadStatus::Finished);
        return;
    }

    tokio::spawn(
        super::dead_connection_detector::start_server_dead_connection_detector(connection.clone()),
    );

    crate::tcp_connection::read_loop::start::<
        TContract,
        TSerializer,
        TSerializerState,
        TSocketCallback,
    >(
        socket_reader,
        &connection,
        &socket_callback,
        serializer_factory.create_serializer().await,
        serializer_factory.create_serializer_state().await,
        logger.clone(),
    )
    .await;

    connection.disconnect().await;

    crate::tcp_connection::read_loop::execute_on_disconnected(
        &connection,
        &socket_callback,
        &logger,
    )
    .await;

    connection.update_read_thread_status(TcpThreadStatus::Finished);
}
