use std::sync::Arc;

use tokio::sync::Mutex;

use crate::tcp_connection::TcpConnectionAbstraction;

pub struct TcpConnectionHolder {
    connection: Mutex<Option<Arc<dyn TcpConnectionAbstraction + Send + Sync + 'static>>>,
}

impl TcpConnectionHolder {
    pub fn new() -> Self {
        Self {
            connection: Mutex::new(None),
        }
    }

    pub async fn set_connection(
        &self,
        connection: Arc<dyn TcpConnectionAbstraction + Send + Sync + 'static>,
    ) {
        let mut write_access = self.connection.lock().await;
        *write_access = Some(connection);
    }

    pub async fn remove_connection(&self) {
        let mut write_access = self.connection.lock().await;
        *write_access = None;
    }

    pub async fn get_connection(
        &self,
    ) -> Option<Arc<dyn TcpConnectionAbstraction + Send + Sync + 'static>> {
        let read_access = self.connection.lock().await;
        read_access.clone()
    }
}
