use std::sync::Arc;

use async_trait::async_trait;

use crate::{tcp_connection::TcpSocketConnection, TcpSocketSerializer};

pub enum ConnectionEvent<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
> {
    Connected(Arc<TcpSocketConnection<TContract, TSerializer>>),
    Disconnected(Arc<TcpSocketConnection<TContract, TSerializer>>),
    Payload {
        connection: Arc<TcpSocketConnection<TContract, TSerializer>>,
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
