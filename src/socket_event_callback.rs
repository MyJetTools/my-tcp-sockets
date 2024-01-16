use std::sync::Arc;

use async_trait::async_trait;

use crate::{tcp_connection::TcpSocketConnection, TcpSerializerMetadata, TcpSocketSerializer};

#[async_trait]
pub trait SocketEventCallback<
    TContract: Send + Sync + 'static,
    TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializerMetadata<TContract> + Send + Sync + 'static,
>
{
    async fn connected(
        &self,
        connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    );

    async fn disconnected(
        &self,
        connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    );

    async fn payload(
        &self,
        connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
        contract: TContract,
    );
}
