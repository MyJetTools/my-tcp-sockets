use std::{collections::HashMap, sync::Arc, time::Duration};

use rust_extensions::Logger;
use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

pub struct TcpConnectionStream {
    tcp_stream: Option<WriteHalf<TcpStream>>,
    log_context: HashMap<String, String>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    send_time_out: Duration,
}

impl TcpConnectionStream {
    pub fn new(
        tcp_stream: WriteHalf<TcpStream>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        log_context: HashMap<String, String>,
        send_time_out: Duration,
    ) -> Self {
        Self {
            tcp_stream: Some(tcp_stream),
            log_context,
            logger,
            send_time_out,
        }
    }

    // If result is true - connections just being dropped
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

            self.logger.write_info(
                "send_payload_to_tcp_connection".to_string(),
                err,
                Some(self.log_context.clone()),
            );
            return true;
        }

        false
    }

    pub fn disconnect(&mut self) -> bool {
        if let Some(mut tcp_stream) = self.tcp_stream.take() {
            tokio::spawn(async move {
                let _ = tcp_stream.shutdown().await;
            });
            return true;
        }

        false
    }

    pub fn get_log_context(&self) -> &HashMap<String, String> {
        &self.log_context
    }

    pub fn set_connection_name(&mut self, name: String) {
        self.log_context
            .insert("ConnectionName".to_string(), name.to_string());
    }
}
