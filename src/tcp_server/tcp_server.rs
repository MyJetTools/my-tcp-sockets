use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};
use tokio::{
    io::{self, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
};

use crate::{
    tcp_connection::{FlushToSocketEventLoop, SocketConnection, TcpContract},
    ConnectionId, SocketEventCallback, TcpSocketSerializer,
};

use super::ConnectionsList;

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpServer<
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
> {
    addr: SocketAddr,
    pub connections: Arc<ConnectionsList<TContract, TSerializer>>,
    name: String,
    max_send_payload_size: usize,
    send_timeout: Duration,
}

impl<TContract, TSerializer> TcpServer<TContract, TSerializer>
where
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TContract: TcpContract + Send + Sync + 'static,
{
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Self {
            name,
            addr,
            connections: Arc::new(ConnectionsList::new()),
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
        }
    }

    pub async fn start<TSerializeFactory, TSocketCallback>(
        &self,
        serializer_factory: Arc<TSerializeFactory>,
        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializeFactory: Send + Sync + 'static + Fn() -> TSerializer,
        TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
    {
        tokio::spawn(accept_sockets_loop(
            self.addr,
            self.connections.clone(),
            serializer_factory.clone(),
            socket_callback.clone(),
            self.name.clone(),
            self.max_send_payload_size,
            self.send_timeout,
            app_states,
            logger,
        ));
    }
}

async fn accept_sockets_loop<TContract, TSerializer, TSerializeFactory, TSocketCallback>(
    addr: SocketAddr,
    connections: Arc<ConnectionsList<TContract, TSerializer>>,
    serializer_factory: Arc<TSerializeFactory>,
    socket_callback: Arc<TSocketCallback>,
    context_name: String,
    max_send_payload_size: usize,
    send_timeout: Duration,
    app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
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

    loop {
        match listener.accept().await {
            Ok((tcp_stream, socket_addr)) => {
                let (read_socket, mut write_socket) = io::split(tcp_stream);

                if app_states.is_shutting_down() {
                    write_socket.shutdown().await.unwrap();
                    break;
                }

                let log_context = Some(format!(
                    "ServerConnection:{}. Id:{}",
                    context_name, connection_id
                ));

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    serializer_factory(),
                    connection_id,
                    Some(socket_addr),
                    logger.clone(),
                    max_send_payload_size,
                    send_timeout,
                    log_context.clone(),
                    Duration::from_secs(60),
                ));

                connection
                    .send_to_socket_event_loop
                    .register_event_loop(Arc::new(FlushToSocketEventLoop::new(connection.clone())))
                    .await;

                connection
                    .send_to_socket_event_loop
                    .start(connection.connection_state.clone(), logger.clone())
                    .await;

                tokio::task::spawn(handle_new_connection(
                    read_socket,
                    connection,
                    connections.clone(),
                    serializer_factory(),
                    logger.clone(),
                    socket_callback.clone(),
                    log_context.clone(),
                ));
                connection_id += 1;
            }
            Err(err) => logger.write_error(
                "Tcp Accept Socket".to_string(),
                format!("Can not accept socket. Err:{}", err),
                format!("ServerConnection: {}.", context_name).into(),
            ),
        }
    }
}

pub async fn handle_new_connection<TContract, TSerializer, TSocketCallback>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    connections: Arc<ConnectionsList<TContract, TSerializer>>,
    read_serializer: TSerializer,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    socket_callback: Arc<TSocketCallback>,
    socket_context: Option<String>,
) where
    TContract: TcpContract + Send + Sync + 'static,
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
        logger.clone(),
        socket_context.clone(),
    )
    .await;
    connections.remove(id).await;
}
