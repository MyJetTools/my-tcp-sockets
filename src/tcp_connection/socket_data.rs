use std::{sync::atomic::Ordering, time::Duration};

use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

use super::{ConnectionStatistics, TcpPayloads};

pub struct SocketData<TSerializer> {
    pub tcp_stream: WriteHalf<TcpStream>,
    serializer: TSerializer,
    pub tcp_payloads: TcpPayloads,
}

impl<TSerializer> SocketData<TSerializer> {
    pub fn new(
        tcp_stream: WriteHalf<TcpStream>,
        serializer: TSerializer,
        max_send_payload_size: usize,
    ) -> Self {
        Self {
            tcp_stream,
            serializer,
            tcp_payloads: TcpPayloads::new(max_send_payload_size),
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
                return Ok(());
            }

            let payload = payload.unwrap();

            self.send_bytes(payload.as_slice(), send_time_out).await?;

            statistics.update_sent_amount(payload.len());
            statistics
                .pending_to_send_buffer_size
                .store(self.tcp_payloads.get_size(), Ordering::SeqCst);
        }
    }

    async fn send_bytes(&mut self, payload: &[u8], send_time_out: Duration) -> Result<(), String> {
        let result = tokio::time::timeout(send_time_out, self.tcp_stream.write_all(payload));

        match result.await {
            Ok(not_timeouted_result) => match not_timeouted_result {
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
