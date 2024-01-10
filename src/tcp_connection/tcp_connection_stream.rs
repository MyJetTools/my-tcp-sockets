use std::{collections::HashMap, sync::Arc, time::Duration};

use rust_extensions::Logger;
use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

use crate::ConnectionId;

pub struct TcpConnectionStream {
    tcp_stream: Option<WriteHalf<TcpStream>>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    send_time_out: Duration,
    connection_name: Option<String>,
    master_socket_name: Arc<String>,
    pub id: ConnectionId,
}

impl TcpConnectionStream {
    pub fn new(
        id: ConnectionId,
        tcp_stream: WriteHalf<TcpStream>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        send_time_out: Duration,
        master_socket_name: Arc<String>,
    ) -> Self {
        Self {
            tcp_stream: Some(tcp_stream),
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
            let result = tokio::time::timeout(self.send_time_out, tcp_stream.write_all(payload));

            let err = match result.await {
                Ok(not_time_outed_result) => match not_time_outed_result {
                    Ok(_) => {
                        return false;
                    }
                    Err(err) => {
                        format!("{}", err)
                    }
                },
                Err(_) => format!("Timeout"),
            };

            self.logger.write_debug_info(
                "send_payload_to_tcp_connection".to_string(),
                err,
                Some(self.get_log_context()),
            );
            return true;
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
