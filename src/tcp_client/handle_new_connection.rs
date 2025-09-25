use std::sync::Arc;

use rust_extensions::Logger;

use crate::socket_reader::SocketReaderTcpStream;
use crate::tcp_connection::TcpThreadStatus;
use crate::{tcp_connection::TcpSocketConnection, SocketEventCallback, TcpSocketSerializer};
use crate::{MaybeTlsReadStream, TcpContract, TcpSerializerState};

pub async fn handle_new_connection<TContract, TSerializer, TSocketCallback, TSerializerState>(
    tcp_stream: MaybeTlsReadStream,
    connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializerState>>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: usize,
    serializer: TSerializer,
    serializer_state: TSerializerState,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializerState>,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
{
    if !crate::tcp_connection::read_loop::execute_on_connected(
        &connection,
        &socket_callback,
        &logger,
    )
    .await
    {
        return;
    }

    tokio::spawn(super::dead_connection_detector::start(
        connection.clone(),
        seconds_to_ping,
        logger.clone(),
    ));

    let socket_reader = SocketReaderTcpStream::new(tcp_stream);

    connection.threads_statistics.read_threads.increase();
    connection.update_read_thread_status(TcpThreadStatus::Started);
    crate::tcp_connection::read_loop::start(
        socket_reader,
        &connection,
        &socket_callback,
        serializer,
        serializer_state,
        logger.clone(),
    )
    .await;
    connection.update_read_thread_status(TcpThreadStatus::Finished);
    connection.threads_statistics.read_threads.decrease();

    crate::tcp_connection::read_loop::execute_on_disconnected(
        &connection,
        &socket_callback,
        &logger,
    )
    .await;
}
