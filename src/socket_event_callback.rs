use std::sync::Arc;

use async_trait::async_trait;

use crate::{tcp_connection::SocketConnection, TcpSocketSerializer};

pub enum ConnectionEvent<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
> {
    Connected(Arc<SocketConnection<TContract, TSerializer>>),
    Disconnected(Arc<SocketConnection<TContract, TSerializer>>),
    Payload {
        connection: Arc<SocketConnection<TContract, TSerializer>>,
        payload: TContract,
    },
}

#[async_trait]
pub trait SocketEventCallback<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
>
{
    async fn handle(&self, connection_event: ConnectionEvent<TContract, TSerializer>);
}
