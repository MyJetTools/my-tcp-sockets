use std::sync::Arc;

use async_trait::async_trait;

use crate::{tcp_connection::TcpSocketConnection, TcpSocketSerializer};

pub enum ConnectionEvent<
    TContract: Send + Sync + 'static,
    TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: Default + Send + Sync + 'static,
> {
    Connected(Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>),
    Disconnected(Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>),
    Payload {
        connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
        payload: TContract,
    },
}

#[async_trait]
pub trait SocketEventCallback<
    TContract: Send + Sync + 'static,
    TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: Default + Send + Sync + 'static,
>
{
    async fn handle(
        &self,
        connection_event: ConnectionEvent<TContract, TSerializer, TSerializationMetadata>,
    );
}
