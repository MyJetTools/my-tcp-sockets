use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rust_extensions::Logger;
use rust_extensions::{date_time::DateTimeAsMicroseconds, events_loop::EventsLoop};

use tokio::{io::WriteHalf, net::TcpStream};

use crate::{ConnectionId, TcpSocketSerializer};

use super::{TcpConnectionInner, TcpConnectionStream};

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
        match self {
            TcpThreadStatus::Finished => true,
            _ => false,
        }
    }
}

impl Into<TcpThreadStatus> for i32 {
    fn into(self) -> TcpThreadStatus {
        match self {
            0 => TcpThreadStatus::NotStarted,
            1 => TcpThreadStatus::Started,
            2 => TcpThreadStatus::Finished,
            _ => panic!("Invalid value {} for ThreadStatus", self),
        }
    }
}

pub struct TcpSocketConnection<TContract: Send + Sync + 'static, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    pub id: ConnectionId,
    inner: Arc<TcpConnectionInner<TContract, TSerializer>>,

    pub addr: Option<SocketAddr>,
    pub dead_disconnect_timeout: Duration,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub threads_statistics: Arc<crate::ThreadsStatistics>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
    > TcpSocketConnection<TContract, TSerializer>
{
    pub async fn new(
        master_socket_name: Arc<String>,
        socket: WriteHalf<TcpStream>,
        id: ConnectionId,
        addr: Option<SocketAddr>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        max_send_payload_size: usize,
        send_timeout: Duration,
        dead_disconnect_timeout: Duration,
        threads_statistics: Arc<crate::ThreadsStatistics>,
        reusable_send_buffer_size: usize,
    ) -> Self {
        let mut send_to_socket_event_loop = EventsLoop::new(
            format!("TcpConnection {}.{}", master_socket_name.as_str(), id),
            logger.clone(),
        )
        .set_iteration_timeout(Duration::from_secs(60));

        let connection_stream =
            TcpConnectionStream::new(id, socket, logger.clone(), send_timeout, master_socket_name);

        threads_statistics.connections_objects.increase();

        let inner = Arc::new(TcpConnectionInner::new(
            connection_stream,
            max_send_payload_size,
            reusable_send_buffer_size,
            logger.clone(),
            threads_statistics.clone(),
        ));

        send_to_socket_event_loop.register_event_loop(inner.clone());

        inner
            .set_send_to_socket_event_loop(send_to_socket_event_loop)
            .await;

        Self {
            id,
            inner,
            logger,
            addr,
            dead_disconnect_timeout,
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
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
    > Drop for TcpSocketConnection<TContract, TSerializer>
{
    fn drop(&mut self) {
        self.threads_statistics.connections_objects.decrease();
    }
}
