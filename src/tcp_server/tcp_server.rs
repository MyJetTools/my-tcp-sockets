use std::{net::SocketAddr, sync::Arc, time::Duration};

use my_logger::{GetMyLoggerReader, LogLevel, MyLogger};
use rust_extensions::ApplicationStates;
use tokio::{
    io::{self, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
};

use crate::{
    tcp_connection::SocketConnection, ConnectionId, SocketEventCallback, TcpSocketSerializer,
};

use super::ConnectionsList;

pub struct TcpServer<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    addr: SocketAddr,
    pub connections: Arc<ConnectionsList<TContract, TSerializer>>,
    name: String,
    pub logger: Arc<MyLogger>,
}

impl<TContract, TSerializer> TcpServer<TContract, TSerializer>
where
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TContract: Send + Sync + 'static,
{
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Self {
            name,
            addr,
            connections: Arc::new(ConnectionsList::new()),
            logger: Arc::new(MyLogger::new(None)),
        }
    }

    pub fn new_with_logger<TGetMyLoggerReader: GetMyLoggerReader>(
        name: String,
        addr: SocketAddr,
        logger: Arc<MyLogger>,
    ) -> Self {
        Self {
            name,
            addr,
            connections: Arc::new(ConnectionsList::new()),
            logger,
        }
    }

    pub fn new_with_logger_reader<TGetMyLoggerReader: GetMyLoggerReader>(
        name: String,
        addr: SocketAddr,
        get_logger: &TGetMyLoggerReader,
    ) -> Self {
        let logger = get_logger.get();
        Self {
            name,
            addr,
            connections: Arc::new(ConnectionsList::new()),
            logger: Arc::new(MyLogger::new(Some(logger.as_ref()))),
        }
    }

    pub fn plug_logger<TGetMyLoggerReader: GetMyLoggerReader>(
        &mut self,
        get_logger: &TGetMyLoggerReader,
    ) {
        let logger = get_logger.get();
        self.logger = Arc::new(MyLogger::new(Some(logger.as_ref())))
    }

    pub async fn start<TAppSates, TSerializeFactory, TSocketCallback>(
        &self,
        app_states: Arc<TAppSates>,
        serializer_factory: Arc<TSerializeFactory>,
        socket_callback: Arc<TSocketCallback>,
    ) where
        TAppSates: Send + Sync + 'static + ApplicationStates,
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        tokio::spawn(accept_sockets_loop(
            app_states,
            self.addr,
            self.connections.clone(),
            serializer_factory.clone(),
            socket_callback.clone(),
            self.logger.clone(),
            self.name.clone(),
        ));
    }
}

async fn accept_sockets_loop<
    TContract,
    TSerializer,
    TAppSates,
    TSerializeFactory,
    TSocketCallback,
>(
    app_states: Arc<TAppSates>,
    addr: SocketAddr,
    connections: Arc<ConnectionsList<TContract, TSerializer>>,
    serializer_factory: Arc<TSerializeFactory>,
    socket_callback: Arc<TSocketCallback>,
    logger: Arc<MyLogger>,
    context_name: String,
) where
    TAppSates: Send + Sync + 'static + ApplicationStates,
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSerializeFactory: Fn() -> TSerializer,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    while !app_states.is_initialized() {
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    let listener = TcpListener::bind(addr).await.unwrap();
    let mut connection_id: ConnectionId = 0;

    loop {
        match listener.accept().await {
            Ok((tcp_stream, socket_addr)) => {
                let (read_socket, mut write_socket) = io::split(tcp_stream);

                if app_states.is_shutting_down() {
                    write_socket.shutdown().await.unwrap();
                    break;
                }

                let log_context =
                    format!("ServerConnection:{}. Id:{}", context_name, connection_id);

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    serializer_factory(),
                    connection_id,
                    Some(socket_addr),
                    logger.clone(),
                    log_context.clone(),
                ));
                tokio::task::spawn(handle_new_connection(
                    read_socket,
                    connection,
                    connections.clone(),
                    serializer_factory(),
                    logger.clone(),
                    log_context,
                    socket_callback.clone(),
                ));
                connection_id += 1;
            }
            Err(err) => logger.write_log(
                LogLevel::FatalError,
                "Tcp Accept Socket".to_string(),
                format!("Can not accept socket. Err:{}", err),
                Some(format!(
                    "TcpServerSocket:{}, EndPoint:{}",
                    context_name, addr
                )),
            ),
        }
    }
}

pub async fn handle_new_connection<TContract, TSerializer, TSocketCallback>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    connections: Arc<ConnectionsList<TContract, TSerializer>>,
    read_serializer: TSerializer,
    logger: Arc<MyLogger>,
    log_context: String,
    socket_callback: Arc<TSocketCallback>,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let id = connection.id;

    connections.add(connection.clone()).await;

    crate::tcp_connection::new_connection::start(
        read_socket,
        connection,
        read_serializer,
        socket_callback,
        None,
        Duration::from_secs(60),
        logger.clone(),
        log_context,
    )
    .await;
    connections.remove(id).await;
}
