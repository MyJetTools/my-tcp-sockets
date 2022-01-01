use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{tcp_connection::SocketConnection, ConnectionId};

pub struct ConnectionsList<TContract> {
    connections: Mutex<HashMap<ConnectionId, Arc<SocketConnection<TContract>>>>,
}

impl<TContract> ConnectionsList<TContract> {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add(&self, connection: Arc<SocketConnection<TContract>>) {
        let mut write_access = self.connections.lock().await;
        write_access.insert(connection.id, connection);
    }

    pub async fn remove(&self, id: ConnectionId) -> Option<Arc<SocketConnection<TContract>>> {
        let mut write_access = self.connections.lock().await;
        write_access.remove(&id)
    }
}
