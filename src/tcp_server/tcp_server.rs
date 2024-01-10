use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{
    io::{self, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
};

use crate::{
    socket_reader::SocketReaderTcpStream,
    tcp_connection::{TcpSocketConnection, TcpThreadStatus},
    ConnectionId, SocketEventCallback, TcpContract, TcpSocketSerializer, ThreadsStatistics,
};

//use super::ConnectionsList;

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpServer {
    addr: SocketAddr,
    name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    reusable_send_buffer_size: usize,
    pub threads_statistics: Arc<ThreadsStatistics>,
}

impl TcpServer {
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Self {
            name: Arc::new(name),
            addr,
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            threads_statistics: Arc::new(ThreadsStatistics::new()),
            reusable_send_buffer_size: 65535,
        }
    }

    pub fn set_reusable_send_buffer_size(mut self, reusable_send_buffer_size: usize) -> Self {
        self.reusable_send_buffer_size = reusable_send_buffer_size;
        self
    }

    pub async fn start<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
        &self,
        serializer_factory: Arc<TSerializeFactory>,
        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
        TContract: TcpContract + Send + Sync + 'static,
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        let threads_statistics = self.threads_statistics.clone();
        tokio::spawn(accept_sockets_loop(
            self.addr,
            serializer_factory.clone(),
            socket_callback.clone(),
            self.name.clone(),
            self.max_send_payload_size,
            self.reusable_send_buffer_size,
            self.send_timeout,
            app_states,
            logger,
            threads_statistics,
        ));
    }
}

async fn accept_sockets_loop<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
    addr: SocketAddr,
    serializer_factory: Arc<TSerializeFactory>,
    socket_callback: Arc<TSocketCallback>,
    context_name: Arc<String>,
    max_send_payload_size: usize,
    reusable_send_buffer_size: usize,
    send_timeout: Duration,
    app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    threads_statistics: Arc<ThreadsStatistics>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSerializeFactory: Fn() -> TSerializer,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
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
            Ok((tcp_stream, socket_addr)) => {
                let (read_tcp_stream, mut write_socket) = io::split(tcp_stream);

                if app_states.is_shutting_down() {
                    write_socket.shutdown().await.unwrap();
                    break;
                }

                let mut log_context = HashMap::new();
                log_context.insert("Id".to_string(), connection_id.to_string());
                log_context.insert("ServerSocketName".to_string(), context_name.to_string());

                let serializer = serializer_factory();

                let cached_ping_payload = if TSerializer::PING_PACKET_IS_SINGLETON {
                    Some(serializer.get_ping_as_payload())
                } else {
                    None
                };

                let connection = Arc::new(
                    TcpSocketConnection::new(
                        write_socket,
                        serializer,
                        connection_id,
                        Some(socket_addr),
                        logger.clone(),
                        max_send_payload_size,
                        send_timeout,
                        log_context,
                        Duration::from_secs(20),
                        cached_ping_payload,
                        context_name.as_str(),
                        threads_statistics.clone(),
                        reusable_send_buffer_size,
                    )
                    .await,
                );

                let logger_spawned = logger.clone();
                let socket_callback = socket_callback.clone();
                let read_serializer = serializer_factory();
                tokio::task::spawn(async move {
                    connection.threads_statistics.increase_read_threads();
                    connection.update_read_thread_status(TcpThreadStatus::Started);

                    handle_new_connection(
                        read_tcp_stream,
                        &connection,
                        read_serializer,
                        logger_spawned,
                        socket_callback,
                    )
                    .await;

                    connection.threads_statistics.decrease_read_threads();
                    connection.update_read_thread_status(TcpThreadStatus::Finished);
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

pub async fn handle_new_connection<TContract, TSerializer, TSocketCallback>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    mut read_serializer: TSerializer,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let mut socket_reader = SocketReaderTcpStream::new(tcp_stream);

    let result =
        super::read::read_first_server_packet(connection, &mut socket_reader, &mut read_serializer)
            .await;

    if result.is_err() {
        connection.disconnect().await;
        return;
    }

    if !crate::tcp_connection::read_loop::execute_on_connected(
        connection,
        &socket_callback,
        &logger,
    )
    .await
    {
        connection.disconnect().await;
        return;
    }

    tokio::spawn(
        super::dead_connection_detector::start_server_dead_connection_detector(connection.clone()),
    );

    crate::tcp_connection::read_loop::start(
        socket_reader,
        &connection,
        read_serializer,
        &socket_callback,
        logger.clone(),
    )
    .await;

    crate::tcp_connection::read_loop::execute_on_disconnected(
        &connection,
        &socket_callback,
        &logger,
    )
    .await;
}
