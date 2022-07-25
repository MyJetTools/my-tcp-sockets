use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::events_loop::EventsLoop;
use rust_extensions::Logger;
use tokio::{io::WriteHalf, net::TcpStream, sync::Mutex};

use tokio::io::AsyncWriteExt;

use crate::{ConnectionId, TcpSocketSerializer};

use super::{ConnectionStatistics, SocketData, TcpConnectionStates};

pub struct SocketConnection<TContract: Send + Sync + 'static, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    pub socket: Mutex<Option<SocketData<TSerializer>>>,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub ping_packet: TContract,
    pub send_timeout: Duration,
    pub send_to_socket_event_loop: EventsLoop<()>,
    socket_context: Option<String>,
    pub connection_state: Arc<TcpConnectionStates>,
    pub dead_disconnect_timeout: Duration,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
    > SocketConnection<TContract, TSerializer>
{
    pub fn new(
        socket: WriteHalf<TcpStream>,
        serializer: TSerializer,
        id: ConnectionId,
        addr: Option<SocketAddr>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        max_send_payload_size: usize,
        send_timeout: Duration,
        socket_context: Option<String>,
        dead_disconnect_timeout: Duration,
    ) -> Self {
        let ping_packet = serializer.get_ping();

        let socket_data = SocketData::new(socket, serializer, max_send_payload_size);

        Self {
            socket: Mutex::new(Some(socket_data)),
            id,
            addr,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            logger,
            ping_packet,
            send_timeout,
            send_to_socket_event_loop: EventsLoop::new(format!("Connection {}", id)),
            socket_context,
            connection_state: Arc::new(TcpConnectionStates::new()),
            dead_disconnect_timeout,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn disconnect(&self) -> bool {
        let mut write_access = self.socket.lock().await;

        if write_access.is_none() {
            return false;
        }

        process_disconnect(
            &mut write_access,
            self.id,
            &self.logger,
            &self.socket_context,
        )
        .await;

        self.statistics.disconnect();
        return true;
    }

    pub async fn send(&self, payload: TContract) {
        let mut write_access = self.socket.lock().await;

        if let Some(socket_data) = write_access.as_mut() {
            let payload = socket_data.get_serializer().serialize(payload);
            self.add_payload_to_send(socket_data, payload.as_slice());
        }
    }

    pub async fn send_ref(&self, payload: &TContract) {
        let mut write_access = self.socket.lock().await;
        if let Some(socket_data) = write_access.as_mut() {
            let payload = socket_data.get_serializer().serialize_ref(payload);
            self.add_payload_to_send(socket_data, payload.as_slice());
        }
    }

    pub async fn send_bytes(&self, payload: &[u8]) {
        let mut write_access = self.socket.lock().await;
        if let Some(socket_data) = write_access.as_mut() {
            self.add_payload_to_send(socket_data, payload);
        }
    }

    fn add_payload_to_send(&self, socket_data: &mut SocketData<TSerializer>, payload: &[u8]) {
        socket_data.tcp_payloads.add_payload(payload);
        self.send_to_socket_event_loop.send(());
        self.statistics
            .pending_to_send_buffer_size
            .store(socket_data.tcp_payloads.get_size(), Ordering::SeqCst);
    }

    pub async fn apply_payload_to_serializer(&self, contract: &TContract) {
        let mut write_access = self.socket.lock().await;

        if let Some(socket_data) = &mut *write_access {
            socket_data.get_serializer_mut().apply_packet(contract);
        }
    }

    pub async fn flush_to_socket(&self) {
        let mut write_access = self.socket.lock().await;

        if let Some(socket_data) = &mut *write_access {
            if let Err(err) = socket_data
                .flush_to_socket(&self.statistics, self.send_timeout)
                .await
            {
                self.logger.write_info(
                    "flush_to_socket".to_string(),
                    format!(
                        "Error while sending to socket socket {}. Err: {:?}",
                        self.id, err
                    ),
                    self.socket_context.clone(),
                );
                process_disconnect(
                    &mut *write_access,
                    self.id,
                    &self.logger,
                    &self.socket_context,
                )
                .await;
            }
        }
    }
}

async fn process_disconnect<TSerializer>(
    socket_data: &mut Option<SocketData<TSerializer>>,
    id: ConnectionId,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
    context: &Option<String>,
) {
    let mut result = None;
    std::mem::swap(&mut result, socket_data);

    let mut result = result.unwrap();

    if let Err(err) = result.tcp_stream.shutdown().await {
        logger.write_info(
            "process_disconnect".to_string(),
            format!("Error while disconnecting socket {}. Err: {}", id, err),
            context.clone(),
        );
    }
}
