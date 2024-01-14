use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use crate::{
    socket_reader::SocketReaderTcpStream,
    tcp_connection::{TcpSocketConnection, TcpThreadStatus},
    ConnectionId, SocketEventCallback, TcpContract, TcpSerializationMetadata, TcpSocketSerializer,
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
        }
    }

    pub async fn start<TContract, TSerializer, TSocketCallback, TSerializationMetadata>(
        &self,

        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializer: Default
            + Send
            + Sync
            + 'static
            + TcpSocketSerializer<TContract, TSerializationMetadata>,
        TContract: TcpContract + Send + Sync + 'static,
        TSocketCallback: Send
            + Sync
            + 'static
            + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
        TSerializationMetadata:
            Default + TcpSerializationMetadata<TContract> + Send + Sync + 'static,
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
        ));
    }
}

async fn accept_sockets_loop<TContract, TSerializer, TSocketCallback, TSerializationMetadata>(
    addr: SocketAddr,
    socket_callback: Arc<TSocketCallback>,
    context_name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    threads_statistics: Arc<ThreadsStatistics>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSocketCallback:
        Send + Sync + 'static + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
    TSerializationMetadata: Default + TcpSerializationMetadata<TContract> + Send + Sync + 'static,
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

                let connection = TcpSocketConnection::new(
                    context_name.clone(),
                    None,
                    connection_id,
                    Some(socket_addr),
                    logger.clone(),
                    max_send_payload_size,
                    send_timeout,
                    Duration::from_secs(20),
                    threads_statistics.clone(),
                )
                .await;

                let logger_spawned = logger.clone();
                let socket_callback = socket_callback.clone();
                tokio::task::spawn(async move {
                    let threads_statistics = connection.threads_statistics.clone();
                    threads_statistics.read_threads.increase();
                    connection.update_read_thread_status(TcpThreadStatus::Started);

                    handle_new_connection(tcp_stream, connection, logger_spawned, socket_callback)
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
    TSocketCallback,
    TSerializationMetadata: Default + TcpSerializationMetadata<TContract> + Send + Sync + 'static,
>(
    tcp_stream: TcpStream,
    connection: TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSocketCallback:
        Send + Sync + 'static + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
{
    let mut socket_reader = SocketReaderTcpStream::new_as_tcp_stream(tcp_stream);

    let mut read_serializer = TSerializer::default();

    let result = super::read::read_first_server_packet(
        &connection,
        &mut socket_reader,
        &mut read_serializer,
    )
    .await;

    if result.is_err() {
        socket_reader.shutdown().await;
        connection.update_read_thread_status(TcpThreadStatus::Finished);
        return;
    }

    let contract = result.unwrap();

    let write_half = socket_reader.get_write_part();

    connection.set_write_half(write_half).await;

    let connection = Arc::new(connection);

    let socket_callback = socket_callback.clone();
    let connection = connection.clone();

    if !crate::tcp_connection::read_loop::execute_on_connected(
        &connection,
        &socket_callback,
        &logger,
    )
    .await
    {
        connection.disconnect().await;
        connection.update_read_thread_status(TcpThreadStatus::Finished);
        return;
    }

    let mut meta_data = TSerializationMetadata::default();

    if meta_data.is_tcp_contract_related_to_metadata(&contract) {
        meta_data.apply_tcp_contract(&contract);
        connection
            .apply_incoming_packet_to_metadata(&contract)
            .await;
    }

    if !super::read::callback_payload(&socket_callback, &connection, contract).await {
        connection.disconnect().await;
        connection.update_read_thread_status(TcpThreadStatus::Finished);
        return;
    }

    tokio::spawn(
        super::dead_connection_detector::start_server_dead_connection_detector(connection.clone()),
    );

    crate::tcp_connection::read_loop::start(
        socket_reader,
        read_serializer,
        &connection,
        meta_data,
        &socket_callback,
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
