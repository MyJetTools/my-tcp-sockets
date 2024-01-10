use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{
    io::{self, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
};

use crate::{
    socket_reader::SocketReaderTcpStream,
    tcp_connection::{TcpSocketConnection, TcpThreadStatus},
    ConnectionId, SerializationMetadata, SocketEventCallback, TcpContract, TcpSocketSerializer,
    ThreadsStatistics,
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

    pub async fn start<TContract, TSerializer, TSocketCallback, TSerializationMetadata>(
        &self,

        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
        TContract: TcpContract + Send + Sync + 'static,
        TSocketCallback: Send
            + Sync
            + 'static
            + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
        TSerializationMetadata: Default + SerializationMetadata<TContract> + Send + Sync + 'static,
    {
        let threads_statistics = self.threads_statistics.clone();
        tokio::spawn(accept_sockets_loop(
            self.addr,
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

async fn accept_sockets_loop<TContract, TSerializer, TSocketCallback, TSerializationMetadata>(
    addr: SocketAddr,
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
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSocketCallback:
        Send + Sync + 'static + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
    TSerializationMetadata: Default + SerializationMetadata<TContract> + Send + Sync + 'static,
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

                //let mut log_context = HashMap::new();
                //log_context.insert("Id".to_string(), connection_id.to_string());
                //log_context.insert("ServerSocketName".to_string(), context_name.to_string());

                let connection = Arc::new(
                    TcpSocketConnection::new(
                        context_name.clone(),
                        write_socket,
                        connection_id,
                        Some(socket_addr),
                        logger.clone(),
                        max_send_payload_size,
                        send_timeout,
                        Duration::from_secs(20),
                        threads_statistics.clone(),
                        reusable_send_buffer_size,
                    )
                    .await,
                );

                let logger_spawned = logger.clone();
                let socket_callback = socket_callback.clone();
                tokio::task::spawn(async move {
                    connection.threads_statistics.read_threads.increase();
                    connection.update_read_thread_status(TcpThreadStatus::Started);

                    handle_new_connection(
                        read_tcp_stream,
                        &connection,
                        logger_spawned,
                        socket_callback,
                    )
                    .await;

                    connection.threads_statistics.read_threads.decrease();
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

pub async fn handle_new_connection<
    TContract,
    TSerializer,
    TSocketCallback,
    TSerializationMetadata: Default + SerializationMetadata<TContract> + Send + Sync + 'static,
>(
    tcp_stream: ReadHalf<TcpStream>,
    connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSocketCallback:
        Send + Sync + 'static + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
{
    let mut socket_reader = SocketReaderTcpStream::new(tcp_stream);

    let mut read_serializer = TSerializer::create_serializer();

    let mut serialization_metadata = TSerializationMetadata::default();

    let result = super::read::read_first_server_packet(
        connection,
        &mut socket_reader,
        &mut read_serializer,
        &socket_callback,
        &mut serialization_metadata,
    )
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
        read_serializer,
        &connection,
        &socket_callback,
        serialization_metadata,
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
}
