use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{tcp_connection::SocketConnection, ConnectionId, TcpSocketSerializer};

pub struct ConnectionsList<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
> {
    connections: Mutex<HashMap<ConnectionId, Arc<SocketConnection<TContract, TSerializer>>>>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
    > ConnectionsList<TContract, TSerializer>
{
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add(&self, connection: Arc<SocketConnection<TContract, TSerializer>>) {
        let mut write_access = self.connections.lock().await;
        write_access.insert(connection.id, connection);
    }

    pub async fn remove(
        &self,
        id: ConnectionId,
    ) -> Option<Arc<SocketConnection<TContract, TSerializer>>> {
        let mut write_access = self.connections.lock().await;
        write_access.remove(&id)
    }
}
