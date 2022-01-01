use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{self, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
    sync::mpsc::UnboundedSender,
};

use crate::{
    tcp_connection::{ConnectionCallback, ConnectionEvent, SocketConnection},
    types::ApplicationStates,
    ConnectionId, TcpSocketSerializer,
};

use super::ConnectionsList;

pub struct TcpServer<TContract> {
    addr: SocketAddr,
    connections: Arc<ConnectionsList<TContract>>,
}

impl<TContract: Send + Sync + 'static> TcpServer<TContract> {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connections: Arc::new(ConnectionsList::new()),
        }
    }

    pub async fn start<TSerializer, TAppSates>(
        &self,
        app_states: Arc<TAppSates>,
        serializer: TSerializer,
    ) -> ConnectionCallback<TContract>
    where
        TAppSates: Send + Sync + 'static + ApplicationStates,
        TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
    {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(accept_sockets_loop(
            app_states,
            self.addr,
            Arc::new(sender),
            self.connections.clone(),
            serializer,
        ));

        ConnectionCallback::new(receiver)
    }
}

async fn accept_sockets_loop<TContract, TSerializer, TAppSates>(
    app_states: Arc<TAppSates>,
    addr: SocketAddr,
    sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
    connections: Arc<ConnectionsList<TContract>>,
    serializer: TSerializer,
) where
    TAppSates: Send + Sync + 'static + ApplicationStates,
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    while !app_states.is_initialized() {
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    let listener = TcpListener::bind(addr).await.unwrap();
    let connection_id: ConnectionId = 0;

    loop {
        match listener.accept().await {
            Ok((tcp_stream, socket_addr)) => {
                let (read_socket, mut write_socket) = io::split(tcp_stream);

                if app_states.is_shutting_down() {
                    write_socket.shutdown().await.unwrap();
                    break;
                }

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    connection_id,
                    Some(socket_addr),
                    sender.clone(),
                ));
                tokio::task::spawn(handle_new_connection(
                    read_socket,
                    connection,
                    connections.clone(),
                    serializer.clone(),
                ));
            }
            Err(err) => println!("Can not accept socket {}. Err: {}", addr, err),
        }
    }
}

pub async fn handle_new_connection<TContract, TSerializer>(
    read_socket: ReadHalf<TcpStream>,
    connection: Arc<SocketConnection<TContract>>,
    connections: Arc<ConnectionsList<TContract>>,
    serializer: TSerializer,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let id = connection.id;

    connections.add(connection.clone()).await;

    crate::tcp_connection::new_connection::start(
        read_socket,
        connection,
        serializer,
        None,
        Duration::from_secs(60),
    )
    .await;
    connections.remove(id).await;
}
