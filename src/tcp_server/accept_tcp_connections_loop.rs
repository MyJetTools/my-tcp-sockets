use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{io::AsyncWriteExt, net::TcpListener};

use crate::{
    ConnectionId, SocketEventCallback, TcpContract, TcpSerializerFactory, TcpSerializerState,
    TcpSocketSerializer, ThreadsStatistics,
};

pub async fn accept_tcp_connections_loop<
    TContract,
    TSerializer,
    TSerializerState,
    TTcpSerializerStateFactory,
    TSocketCallback,
>(
    addr: SocketAddr,
    socket_callback: Arc<TSocketCallback>,
    context_name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    threads_statistics: Arc<ThreadsStatistics>,
    serializer_metadata_factory: Arc<TTcpSerializerStateFactory>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    TTcpSerializerStateFactory:
        TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
{
    while !app_states.is_initialized() {
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    let listener = TcpListener::bind(addr).await.unwrap();
    let mut connection_id: ConnectionId = 0;

    let mut server_socket_log_context = HashMap::new();
    server_socket_log_context.insert("ServerSocketName".to_string(), context_name.to_string());
    server_socket_log_context.insert("Addr".to_string(), format!("{}", addr));

    let server_socket_log_context = Some(server_socket_log_context);

    loop {
        match listener.accept().await {
            Ok((mut tcp_stream, socket_addr)) => {
                if app_states.is_shutting_down() {
                    tcp_stream.shutdown().await.unwrap();
                    break;
                }

                //    let (read_half, write_half) = tcp_stream.into_split();
                //let mut log_context = HashMap::new();
                //log_context.insert("Id".to_string(), connection_id.to_string());
                //log_context.insert("ServerSocketName".to_string(), context_name.to_string());

                let logger_spawned = logger.clone();
                let socket_callback = socket_callback.clone();
                let threads_statistics = threads_statistics.clone();
                let context_name = context_name.clone();
                let serializer_metadata_factory = serializer_metadata_factory.clone();
                tokio::task::spawn(async move {
                    threads_statistics.read_threads.increase();

                    super::handle_new_connection(
                        context_name,
                        tcp_stream,
                        logger_spawned,
                        socket_callback,
                        connection_id,
                        socket_addr,
                        &threads_statistics,
                        max_send_payload_size,
                        send_timeout,
                        serializer_metadata_factory,
                    )
                    .await;

                    threads_statistics.read_threads.decrease();
                });

                connection_id += 1;
            }
            Err(err) => logger.write_error(
                "Tcp Accept Socket".to_string(),
                format!("Can not accept socket. Err:{}", err),
                server_socket_log_context.clone(),
            ),
        }
    }
}
