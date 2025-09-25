use std::collections::HashMap;
use std::sync::Arc;

use rust_extensions::Logger;

use crate::tcp_connection::TcpSocketConnection;
use crate::{SocketEventCallback, TcpClientInner, TcpSocketSerializer};
use crate::{TcpContract, TcpSerializerFactory, TcpSerializerState};

use crate::tcp_client::LOG_PROCESS;

pub async fn connect_to_tcp_socket<
    TContract,
    TSerializer,
    TSerializerMetadataFactory,
    TSocketCallback,
    TSerializerState,
>(
    host_port: &str,
    connection_id: i32,
    inner: &TcpClientInner,
    socket_callback: &Arc<TSocketCallback>,
    serializer_factory: &Arc<TSerializerMetadataFactory>,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
    socket_context: HashMap<String, String>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializerState>,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSerializerMetadataFactory:
        TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
{
    let connect_future = tokio::net::TcpStream::connect(host_port);

    let timeout_result = tokio::time::timeout(inner.connect_timeout, connect_future).await;

    let Ok(unix_socket_stream_result) = timeout_result else {
        logger.write_error(
            crate::tcp_client::LOG_PROCESS.to_string(),
            "Timeout during establishing connection".to_string(),
            socket_context.into(),
        );
        return;
    };

    let unix_socket = match unix_socket_stream_result {
        Ok(unix_socket) => unix_socket,
        Err(err) => {
            logger.write_error(
                crate::tcp_client::LOG_PROCESS.to_string(),
                format!("Can not connect to {}. Reason: {}", host_port, err),
                Some(socket_context.clone()),
            );
            return;
        }
    };

    logger.write_debug_info(
        LOG_PROCESS.to_string(),
        format!("Connected to {}. Id: {}", host_port, connection_id),
        Some(socket_context.clone()),
    );

    let (read_socket, write_socket) = unix_socket.into_split();

    let connection = Arc::new(
        TcpSocketConnection::new(
            inner.name.clone(),
            Some(write_socket.into()),
            connection_id,
            None,
            logger.clone(),
            inner.max_send_payload_size,
            inner.send_timeout,
            inner.disconnect_timeout,
            inner.threads_statistics.clone(),
            serializer_factory.create_serializer().await,
            serializer_factory.create_serializer_state().await,
        )
        .await,
    );

    super::handle_new_connection(
        read_socket.into(),
        connection,
        logger.clone(),
        socket_callback.clone(),
        inner.seconds_to_ping,
        serializer_factory.create_serializer().await,
        serializer_factory.create_serializer_state().await,
    )
    .await;
}
