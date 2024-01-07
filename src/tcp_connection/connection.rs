use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use rust_extensions::{date_time::DateTimeAsMicroseconds, events_loop::EventsLoop};

use tokio::{io::WriteHalf, net::TcpStream};

use crate::{ConnectionId, TcpSocketSerializer};

use super::{TcpConnectionInner, TcpConnectionStream};

pub struct TcpSocketConnection<TContract: Send + Sync + 'static, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    inner: Arc<TcpConnectionInner>,
    serializer: TSerializer,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    pub ping_packet: TContract,
    pub dead_disconnect_timeout: Duration,
    cached_ping_payload: Option<Vec<u8>>,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub threads_statistics: Arc<crate::ThreadsStatistics>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
    > TcpSocketConnection<TContract, TSerializer>
{
    pub async fn new(
        socket: WriteHalf<TcpStream>,
        serializer: TSerializer,
        id: ConnectionId,
        addr: Option<SocketAddr>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        max_send_payload_size: usize,
        send_timeout: Duration,
        log_context: HashMap<String, String>,
        dead_disconnect_timeout: Duration,
        cached_ping_payload: Option<Vec<u8>>,
        master_connection_name: &str,
        threads_statistics: Arc<crate::ThreadsStatistics>,
    ) -> Self {
        let ping_packet = serializer.get_ping();

        let connection_stream =
            TcpConnectionStream::new(socket, logger.clone(), log_context, send_timeout);

        let inner = Arc::new(TcpConnectionInner::new(
            connection_stream,
            max_send_payload_size,
            logger.clone(),
            threads_statistics.clone(),
        ));

        let send_to_socket_event_loop = EventsLoop::new(
            format!("TcpConnection {}.{}", master_connection_name, id),
            logger.clone(),
        )
        .set_iteration_timeout(Duration::from_secs(60));

        send_to_socket_event_loop
            .register_event_loop(inner.clone())
            .await;

        inner
            .set_send_to_socket_event_loop(send_to_socket_event_loop)
            .await;

        Self {
            inner,
            logger,
            id,
            addr,
            ping_packet,
            dead_disconnect_timeout,
            cached_ping_payload,
            serializer,
            threads_statistics,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    pub async fn disconnect(&self) -> bool {
        self.inner.disconnect().await
    }

    pub async fn get_log_context(&self) -> HashMap<String, String> {
        self.inner.get_log_context().await
    }

    pub async fn send(&self, payload: TContract) {
        if !self.inner.is_connected() {
            return;
        }

        let payload = self.serializer.serialize(payload);
        self.send_bytes(payload.as_slice()).await;
    }

    #[cfg(feature = "serialize_as_ref")]
    pub async fn send_ref(&self, payload: &TContract) {
        let mut write_access = self.single_threaded.lock().await;
        if let Some(socket_data) = write_access.connection.as_mut() {
            let payload = socket_data.get_serializer().serialize_ref(payload);
            self.add_payload_to_send(socket_data, payload.as_slice());
        }
    }

    pub async fn send_bytes(&self, payload: &[u8]) {
        self.inner.push_payload_to_send_buffer(payload).await
    }

    pub async fn set_connection_name(&self, name: String) {
        let mut write_access = self.inner.stream.lock().await;
        write_access.set_connection_name(name);
    }

    pub async fn send_ping(&self) {
        if let Some(cached_ping_payload) = self.cached_ping_payload.as_ref() {
            self.send_bytes(cached_ping_payload).await;
            return;
        }

        let ping_contract = self.serializer.get_ping();
        let payload = self.serializer.serialize(ping_contract);
        self.send_bytes(payload.as_slice()).await;
    }

    pub fn statistics(&self) -> &super::ConnectionStatistics {
        &self.inner.statistics
    }

    pub fn is_dead(&self, now: DateTimeAsMicroseconds) -> bool {
        let silence_duration = now
            .duration_since(self.inner.statistics.last_receive_moment.as_date_time())
            .as_positive_or_zero();

        silence_duration > self.dead_disconnect_timeout
    }
}
