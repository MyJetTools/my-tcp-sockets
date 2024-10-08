use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use crate::{
    socket_reader::SocketReaderTcpStream,
    tcp_connection::{TcpSocketConnection, TcpThreadStatus},
    ConnectionId, SocketEventCallback, TcpContract, TcpSerializerFactory, TcpSerializerState,
    TcpSocketSerializer, ThreadsStatistics,
};

//use super::ConnectionsList;

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpServer {
    addr: SocketAddr,
    name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    pub threads_statistics: Arc<ThreadsStatistics>,
}

impl TcpServer {
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Self {
            name: Arc::new(name),
            addr,
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            threads_statistics: Arc::new(ThreadsStatistics::default()),
        }
    }

    pub async fn start<
        TContract,
        TSerializer,
        TSerializerState,
        TTcpSerializerStateFactory,
        TSocketCallback,
    >(
        &self,
        serializer_metadata_factory: Arc<TTcpSerializerStateFactory>,
        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
        TContract: TcpContract + Send + Sync + 'static,
        TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
        TTcpSerializerStateFactory:
            TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
        TSocketCallback:
            SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    {
        let threads_statistics = self.threads_statistics.clone();
        tokio::spawn(accept_sockets_loop(
            self.addr,
            socket_callback.clone(),
            self.name.clone(),
            self.max_send_payload_size,
            self.send_timeout,
            app_states,
            logger,
            threads_statistics,
            serializer_metadata_factory,
        ));
    }
}

async fn accept_sockets_loop<
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

                    handle_new_connection(
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
    let read_half = crate::MaybeTlsReadStream::Plain(read_half);
    #[cfg(feature = "with-tls")]
    let write_half = crate::MaybeTlsWriteStream::Plain(write_half);

    let socket_reader = SocketReaderTcpStream::new(read_half);

    let connection = TcpSocketConnection::new(
        master_socket_name,
        Some(write_half),
        connection_id,
        Some(socket_addr),
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
