use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use rust_extensions::{date_time::DateTimeAsMicroseconds, events_loop::EventsLoop};

use crate::{ConnectionId, MaybeTlsWriteStream, TcpSerializerState, TcpSocketSerializer};

use super::{
    TcpConnectionAbstraction, TcpConnectionInner, TcpConnectionStates, TcpConnectionStream,
};

#[derive(Debug)]
pub enum TcpThreadStatus {
    NotStarted,
    Started,
    Finished,
}

impl TcpThreadStatus {
    pub fn as_i32(&self) -> i32 {
        match self {
            TcpThreadStatus::NotStarted => 0,
            TcpThreadStatus::Started => 1,
            TcpThreadStatus::Finished => 2,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, TcpThreadStatus::Finished)
    }
}

impl From<i32> for TcpThreadStatus {
    fn from(val: i32) -> Self {
        match val {
            0 => TcpThreadStatus::NotStarted,
            1 => TcpThreadStatus::Started,
            2 => TcpThreadStatus::Finished,
            _ => panic!("Invalid value {} for ThreadStatus", val),
        }
    }
}

pub struct TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>
where
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
{
    pub id: ConnectionId,
    inner: Arc<TcpConnectionInner<TContract, TSerializer, TSerializationMetadata>>,

    pub addr: Option<SocketAddr>,
    pub dead_disconnect_timeout: Duration,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub threads_statistics: Arc<crate::ThreadsStatistics>,
    pub events_loop: EventsLoop<()>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
        TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    > TcpSocketConnection<TContract, TSerializer, TSerializerState>
{
    pub async fn new(
        master_socket_name: Arc<String>,
        socket: Option<MaybeTlsWriteStream>,
        id: ConnectionId,
        addr: Option<SocketAddr>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        max_send_payload_size: usize,
        send_timeout: Duration,
        dead_disconnect_timeout: Duration,
        threads_statistics: Arc<crate::ThreadsStatistics>,
        serializer: TSerializer,
        serializer_state: TSerializerState,
    ) -> Self {
        let connection_stream = TcpConnectionStream::new(
            id,
            socket,
            logger.clone(),
            send_timeout,
            master_socket_name.clone(),
        );

        let mut events_loop = EventsLoop::new(
            format!("TcpConnection {}.{}", master_socket_name, id),
            logger.clone(),
        )
        .set_iteration_timeout(Duration::from_secs(60));

        let inner = Arc::new(TcpConnectionInner::new(
            connection_stream,
            max_send_payload_size,
            logger.clone(),
            threads_statistics.clone(),
            events_loop.get_publisher(),
            serializer,
            serializer_state,
        ));

        threads_statistics.connections_objects.increase();

        events_loop.register_event_loop(inner.clone());

        events_loop.start(Arc::new(TcpConnectionStates::default()));

        Self {
            id,
            inner,
            logger,
            addr,
            dead_disconnect_timeout,
            threads_statistics,
            events_loop,
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

    pub fn update_read_thread_status(&self, status: TcpThreadStatus) {
        self.inner.update_read_thread_status(status);
    }

    pub fn get_read_thread_status(&self) -> TcpThreadStatus {
        self.inner.get_read_thread_status()
    }

    pub fn get_write_thread_status(&self) -> TcpThreadStatus {
        self.inner.get_write_thread_status()
    }

    pub async fn send(&self, contract: &TContract) -> usize {
        if !self.inner.is_connected() {
            return 0;
        }

        self.inner.push_contract(contract).await
    }

    pub async fn send_many(&self, contracts: &[TContract]) -> usize {
        if !self.inner.is_connected() {
            return 0;
        }
        self.inner.push_many_contracts(contracts).await
    }

    pub async fn send_bytes(&self, payload: &[u8]) -> usize {
        if !self.inner.is_connected() {
            return 0;
        }

        self.inner.push_payload(payload).await
    }

    pub async fn set_connection_name(&self, name: String) {
        let mut write_access = self.inner.stream.lock().await;
        write_access.set_connection_name(name);
    }

    pub async fn send_ping(&self) -> usize {
        if !self.inner.is_connected() {
            return 0;
        }

        self.inner.send_ping().await
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

    pub async fn update_incoming_packet_to_state(&self, contract: &TContract) {
        let mut write_access = self.inner.buffer_to_send_inner.lock().await;

        write_access
            .serializer_state
            .as_mut()
            .unwrap()
            .apply_tcp_contract(contract);
    }
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
    > Drop for TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>
{
    fn drop(&mut self) {
        self.threads_statistics.connections_objects.decrease();
    }
}

#[async_trait::async_trait]
impl<TContract, TSerializer, TSerializationMetadata> TcpConnectionAbstraction
    for TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>
where
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
{
    async fn disconnect(&self) {
        self.inner.disconnect().await;
    }
}
