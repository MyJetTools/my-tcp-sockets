use std::time::Duration;

use std::sync::atomic::Ordering;

use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

use super::TcpBufferToSend;

use super::ConnectionStatistics;

pub struct SocketData<TSerializer> {
    pub tcp_stream: WriteHalf<TcpStream>,
    serializer: TSerializer,
    pub tcp_payloads: TcpBufferToSend,
    max_send_payload_size_to_send: usize,
}

impl<TSerializer> SocketData<TSerializer> {
    pub fn new(
        tcp_stream: WriteHalf<TcpStream>,
        serializer: TSerializer,
        max_send_payload_size_to_send: usize,
    ) -> Self {
        Self {
            tcp_stream,
            serializer,
            tcp_payloads: TcpBufferToSend::new(1024 * 1024),
            max_send_payload_size_to_send,
        }
    }
    pub fn get_serializer(&self) -> &TSerializer {
        &self.serializer
    }

    pub fn get_serializer_mut(&mut self) -> &mut TSerializer {
        &mut self.serializer
    }

    pub async fn flush_to_socket(
        &mut self,
        statistics: &ConnectionStatistics,
        send_time_out: Duration,
    ) -> Result<(), String> {
        loop {
            let payload = self.tcp_payloads.get_payload();

            if payload.is_none() {
                statistics
                    .pending_to_send_buffer_size
                    .store(self.tcp_payloads.get_size(), Ordering::SeqCst);
                break;
            }

            let mut payload = payload.unwrap();

            while let Some(to_send) =
                payload.get_next_slice_to_send(self.max_send_payload_size_to_send)
            {
                self.send_bytes(to_send, send_time_out).await?;
            }

            statistics.update_sent_amount(payload.len());

            self.tcp_payloads.reuse_payload(payload);

            statistics
                .pending_to_send_buffer_size
                .store(self.tcp_payloads.get_size(), Ordering::SeqCst);
        }

        Ok(())
    }

    async fn send_bytes(&mut self, payload: &[u8], send_time_out: Duration) -> Result<(), String> {
        let result = tokio::time::timeout(send_time_out, self.tcp_stream.write_all(payload));

        match result.await {
            Ok(not_time_outed_result) => match not_time_outed_result {
                Ok(_) => {
                    return Ok(());
                }
                Err(err) => {
                    return Err(format!("{}", err));
                }
            },
            Err(_) => {
                return Err(format!("Timeout"));
            }
        }
    }
}
