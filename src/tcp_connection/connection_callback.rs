use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;

use super::SocketConnection;

pub enum ConnectionEvent<TContract> {
    Connected(Arc<SocketConnection<TContract>>),
    Disconnected(Arc<SocketConnection<TContract>>),
    Payload {
        connection: Arc<SocketConnection<TContract>>,
        payload: TContract,
    },
}

pub struct ConnectionCallback<TContract> {
    receiver: UnboundedReceiver<ConnectionEvent<TContract>>,
}

impl<TContract> ConnectionCallback<TContract> {
    pub fn new(receiver: UnboundedReceiver<ConnectionEvent<TContract>>) -> Self {
        Self { receiver }
    }

    pub async fn get_next_event(&mut self) -> ConnectionEvent<TContract> {
        loop {
            if let Some(event) = self.receiver.recv().await {
                return event;
            }
        }
    }
}
