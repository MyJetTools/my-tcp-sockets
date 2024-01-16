use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use tokio::net::tcp::OwnedReadHalf;
use tokio::{net::TcpStream, sync::Mutex};

use crate::socket_reader::SocketReaderTcpStream;
use crate::tcp_connection::TcpThreadStatus;
use crate::{
    tcp_connection::TcpSocketConnection, ConnectionId, SocketEventCallback, TcpSocketSerializer,
};
use crate::{
    TcpClientSocketSettings, TcpContract, TcpSerializerMetadata, TcpSerializerMetadataFactory,
    ThreadsStatistics,
};

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpClient {
    settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    re_connect_timeout: Duration,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    background_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub threads_statistics: Arc<ThreadsStatistics>,
}

impl TcpClient {
    pub fn new(
        name: String,
        settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    ) -> Self {
        Self {
            settings,
            re_connect_timeout: Duration::from_secs(3),
            seconds_to_ping: 3,
            disconnect_timeout: Duration::from_secs(9),
            name: Arc::new(name),
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            background_task: Mutex::new(None),
            threads_statistics: Arc::new(ThreadsStatistics::new()),
        }
    }

    pub fn set_seconds_to_ping(mut self, seconds_to_ping: usize) -> Self {
        self.seconds_to_ping = seconds_to_ping;
        self
    }

    pub fn set_disconnect_timeout(mut self, disconnect_timeout: Duration) -> Self {
        self.disconnect_timeout = disconnect_timeout;
        self
    }

    pub fn set_reconnect_timeout(mut self, re_connect_timeout: Duration) -> Self {
        self.re_connect_timeout = re_connect_timeout;
        self
    }

    pub async fn start<
        TContract,
        TSerializer,
        TSerializationMetadata: TcpSerializerMetadata<TContract> + Send + Sync + 'static,
        TTcpSerializerMetadataFactory,
        TSocketCallback,
    >(
        &self,
        serializer_metadata_factory: Arc<TTcpSerializerMetadataFactory>,
        socket_callback: Arc<TSocketCallback>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TContract: TcpContract + Send + Sync + 'static,
        TSerializer: Default
            + Send
            + Sync
            + 'static
            + TcpSocketSerializer<TContract, TSerializationMetadata>,

        TTcpSerializerMetadataFactory:
            TcpSerializerMetadataFactory<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSocketCallback: SocketEventCallback<TContract, TSerializer, TSerializationMetadata>
            + Send
            + Sync
            + 'static,
    {
        let threads_statistics = self.threads_statistics.clone();
        let handle = tokio::spawn(connection_loop(
            self.settings.clone(),
            self.re_connect_timeout,
            socket_callback,
            self.seconds_to_ping,
            self.disconnect_timeout,
            self.name.clone(),
            self.max_send_payload_size,
            self.send_timeout,
            logger,
            threads_statistics,
            serializer_metadata_factory,
        ));

        let mut background_task = self.background_task.lock().await;
        background_task.replace(handle);
    }

    pub async fn stop(&self) {
        let mut background_task = self.background_task.lock().await;

        if let Some(background_task) = background_task.take() {
            background_task.abort();
        }
    }
}

async fn connection_loop<
    TContract,
    TSerializer,
    TSerializationMetadata,
    TSerializerMetadataFactory,
    TSocketCallback,
>(
    settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    re_connect_timeout: Duration,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: usize,
    disconnect_timeout: Duration,
    master_socket_name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    threads_statistics: Arc<ThreadsStatistics>,
    meta_data_factory: Arc<TSerializerMetadataFactory>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerMetadata<TContract> + Send + Sync + 'static,
    TSerializerMetadataFactory:
        TcpSerializerMetadataFactory<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send + Sync + 'static,
{
    let mut connection_id: ConnectionId = 0;

    let mut socket_context = HashMap::new();
    socket_context.insert(
        "SocketName".to_string(),
        master_socket_name.as_str().to_string(),
    );

    const LOG_PROCESS: &str = "Tcp Client Connect";
    loop {
        tokio::time::sleep(re_connect_timeout).await;

        let host_port = settings.get_host_port().await;

        if host_port.is_none() {
            continue;
        }

        let host_port = host_port.unwrap();

        let mut socket_context = socket_context.clone();
        socket_context.insert("HostPort".to_string(), host_port.clone());

        logger.write_debug_info(
            LOG_PROCESS.to_string(),
            format!("Trying to connect to {}", host_port),
            Some(socket_context.clone()),
        );
        let connect_result = TcpStream::connect(host_port.as_str()).await;

        match connect_result {
            Ok(tcp_stream) => {
                logger.write_debug_info(
                    LOG_PROCESS.to_string(),
                    format!("Connected to {}. Id: {}", host_port, connection_id),
                    Some(socket_context.clone()),
                );

                let (read_socket, write_socket) = tcp_stream.into_split();

                // let mut log_context = HashMap::new();
                // log_context.insert("ConnectionId".to_string(), connection_id.to_string());
                // log_context.insert("SocketName".to_string(), socket_name.to_string());

                let connection = Arc::new(
                    TcpSocketConnection::new(
                        master_socket_name.clone(),
                        Some(write_socket),
                        connection_id,
                        None,
                        logger.clone(),
                        max_send_payload_size,
                        send_timeout,
                        disconnect_timeout,
                        threads_statistics.clone(),
                        meta_data_factory.create().await,
                    )
                    .await,
                );

                let serializer_meta_data = meta_data_factory.create().await;

                handle_new_connection(
                    read_socket,
                    connection.clone(),
                    logger.clone(),
                    socket_callback.clone(),
                    seconds_to_ping,
                    serializer_meta_data,
                )
                .await;

                logger.write_debug_info(
                    LOG_PROCESS.to_string(),
                    format!("Disconnected from {}", host_port),
                    Some(socket_context.clone()),
                );
                connection_id += 1;
            }
            Err(err) => {
                logger.write_error(
                    LOG_PROCESS.to_string(),
                    format!("Can not connect to {}. Reason: {}", host_port, err),
                    Some(socket_context.clone()),
                );
            }
        }
    }
}

pub async fn handle_new_connection<
    TContract,
    TSerializer,
    TSocketCallback,
    TSerializationMetadata,
>(
    tcp_stream: OwnedReadHalf,
    connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
    seconds_to_ping: usize,
    serializer_metadata: TSerializationMetadata,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerMetadata<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send + Sync + 'static,
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

    let socket_reader = SocketReaderTcpStream::new_as_owned_tcp_stream(tcp_stream);

    connection.threads_statistics.read_threads.increase();
    connection.update_read_thread_status(TcpThreadStatus::Started);
    crate::tcp_connection::read_loop::start(
        socket_reader,
        &connection,
        &socket_callback,
        serializer_metadata,
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
