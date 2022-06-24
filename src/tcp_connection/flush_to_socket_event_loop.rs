use std::sync::Arc;

use rust_extensions::events_loop::EventsLoopTick;

use crate::TcpSocketSerializer;

use super::SocketConnection;

pub struct FlushToSocketEventLoop<TContract: Send + Sync + 'static, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    connection: Arc<SocketConnection<TContract, TSerializer>>,
}

impl<TContract: Send + Sync + 'static, TSerializer> FlushToSocketEventLoop<TContract, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    pub fn new(connection: Arc<SocketConnection<TContract, TSerializer>>) -> Self {
        Self { connection }
    }
}

#[async_trait::async_trait]
impl<TContract: Send + Sync + 'static, TSerializer> EventsLoopTick<()>
    for FlushToSocketEventLoop<TContract, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    async fn tick(&self, _: ()) {
        self.connection.flush_to_socket().await;
    }
}
