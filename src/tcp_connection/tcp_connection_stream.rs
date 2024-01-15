use std::{collections::HashMap, sync::Arc, time::Duration};

use rust_extensions::Logger;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::ConnectionId;

pub struct TcpConnectionStream {
    tcp_stream: Option<OwnedWriteHalf>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    send_time_out: Duration,
    connection_name: Option<String>,
    master_socket_name: Arc<String>,
    pub id: ConnectionId,
}

impl TcpConnectionStream {
    pub fn new(
        id: ConnectionId,
        tcp_stream: Option<OwnedWriteHalf>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        send_time_out: Duration,
        master_socket_name: Arc<String>,
    ) -> Self {
        Self {
            tcp_stream,
            logger,
            send_time_out,
            connection_name: None,
            master_socket_name,
            id,
        }
    }

    // If result is true - connections has error
    pub async fn send_payload_to_tcp_connection(&mut self, payload: &[u8]) -> bool {
        if let Some(tcp_stream) = self.tcp_stream.as_mut() {
            match send_with_timeout(tcp_stream, payload, self.send_time_out).await {
                Ok(_) => return false,
                Err(err) => self.logger.write_info(
                    "send_payload_to_tcp_connection".to_string(),
                    err,
                    Some(self.get_log_context()),
                ),
            }
        }

        false
    }

    // this function has to used only form connection_inner disconnect
    pub fn disconnect(&mut self) -> bool {
        if let Some(mut tcp_stream) = self.tcp_stream.take() {
            tokio::spawn(async move {
                let _ = tcp_stream.shutdown().await;
            });
            return true;
        }

        false
    }

    pub fn get_log_context(&self) -> HashMap<String, String> {
        let mut result = HashMap::with_capacity(3);
        result.insert(
            "TcpSocketName".to_string(),
            self.master_socket_name.as_str().to_string(),
        );
        result.insert("Id".to_string(), self.id.to_string());
        if let Some(connection_name) = &self.connection_name {
            result.insert("TcpConnectionName".to_string(), connection_name.to_string());
        }

        result
    }

    pub fn set_connection_name(&mut self, name: String) {
        self.connection_name = Some(name);
    }
}

async fn send_with_timeout(
    tcp_stream: &mut OwnedWriteHalf,
    payload: &[u8],
    send_timeout: Duration,
) -> Result<(), String> {
    let result = tokio::time::timeout(send_timeout, tcp_stream.write_all(payload)).await;

    if result.is_err() {
        return Err("Timeout".to_string());
    }

    let result = result.unwrap();

    if let Err(err) = result {
        return Err(format!("{}", err));
    }

    return Ok(());
}
