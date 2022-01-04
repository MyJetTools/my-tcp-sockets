use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::TcpSocketSerializer;

use super::SocketConnection;

pub enum ConnectionEvent<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    Connected(Arc<SocketConnection<TContract, TSerializer>>),
    Disconnected(Arc<SocketConnection<TContract, TSerializer>>),
    Payload {
        connection: Arc<SocketConnection<TContract, TSerializer>>,
        payload: TContract,
    },
}

pub struct ConnectionCallback<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    receiver: UnboundedReceiver<ConnectionEvent<TContract, TSerializer>>,
}

impl<TContract, TSerializer: TcpSocketSerializer<TContract>>
    ConnectionCallback<TContract, TSerializer>
{
    pub fn new(receiver: UnboundedReceiver<ConnectionEvent<TContract, TSerializer>>) -> Self {
        Self { receiver }
    }

    pub async fn get_next_event(&mut self) -> ConnectionEvent<TContract, TSerializer> {
        loop {
            if let Some(event) = self.receiver.recv().await {
                return event;
            }
        }
    }
}
