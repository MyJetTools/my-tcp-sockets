use std::sync::Arc;

use async_trait::async_trait;

use crate::{tcp_connection::SocketConnection, TcpSocketSerializer};

pub enum ConnectionEvent<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    Connected(Arc<SocketConnection<TContract, TSerializer>>),
    Disconnected(Arc<SocketConnection<TContract, TSerializer>>),
    Payload {
        connection: Arc<SocketConnection<TContract, TSerializer>>,
        payload: TContract,
    },
}

#[async_trait]
pub trait SocketEventCallback<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    async fn handle(&self, socket_reader: ConnectionEvent<TContract, TSerializer>);
}
