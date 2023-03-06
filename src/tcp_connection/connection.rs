use std::collections::HashMap;
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

pub struct SocketConnectionSingleThreaded<TSerializer> {
    connection: Option<SocketData<TSerializer>>,
    log_context: Option<HashMap<String, String>>,
}

impl<TSerializer> SocketConnectionSingleThreaded<TSerializer> {
    pub fn new(connection: SocketData<TSerializer>, log_context: HashMap<String, String>) -> Self {
        Self {
            connection: Some(connection),
            log_context: Some(log_context),
        }
    }
}

pub struct SocketConnection<TContract: Send + Sync + 'static, TSerializer>
where
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
{
    pub single_threaded: Mutex<SocketConnectionSingleThreaded<TSerializer>>,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub ping_packet: TContract,
    pub send_timeout: Duration,
    pub send_to_socket_event_loop: EventsLoop<()>,
    pub connection_state: Arc<TcpConnectionStates>,
    pub dead_disconnect_timeout: Duration,
    cached_ping_payload: Option<Vec<u8>>,
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
        log_context: HashMap<String, String>,
        dead_disconnect_timeout: Duration,
        cached_ping_payload: Option<Vec<u8>>,
    ) -> Self {
        let ping_packet = serializer.get_ping();

        let socket_data = SocketData::new(socket, serializer, max_send_payload_size);

        let socket = SocketConnectionSingleThreaded::new(socket_data, log_context);

        Self {
            single_threaded: Mutex::new(socket),
            id,
            addr,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            logger,
            ping_packet,
            send_timeout,
            send_to_socket_event_loop: EventsLoop::new(format!("Connection {}", id)),

            connection_state: Arc::new(TcpConnectionStates::new()),
            dead_disconnect_timeout,
            cached_ping_payload,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn disconnect(&self) -> bool {
        let mut write_access = self.single_threaded.lock().await;

        if write_access.connection.is_none() {
            return false;
        }

        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);

        process_disconnect(&mut write_access, self.id, &self.logger).await;

        self.statistics.disconnect();

        self.send_to_socket_event_loop.stop();

        return true;
    }

    pub async fn get_log_context(&self) -> Option<HashMap<String, String>> {
        let read_access = self.single_threaded.lock().await;
        read_access.log_context.clone()
    }

    pub async fn send(&self, payload: TContract) {
        let mut write_access = self.single_threaded.lock().await;

        if let Some(socket_data) = write_access.connection.as_mut() {
            let payload = socket_data.get_serializer().serialize(payload);
            self.add_payload_to_send(socket_data, payload.as_slice());
        }
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
        let mut write_access = self.single_threaded.lock().await;
        if let Some(socket_data) = write_access.connection.as_mut() {
            self.add_payload_to_send(socket_data, payload);
        }
    }

    fn add_payload_to_send(&self, socket_data: &mut SocketData<TSerializer>, payload: &[u8]) {
        #[cfg(feature = "debug_outcoming_traffic")]
        println!(
            "Send {} bytes to tcp socket. First byte:[{}]",
            payload.len(),
            payload[0]
        );

        socket_data.tcp_payloads.add_payload(payload);
        self.send_to_socket_event_loop.send(());

        self.statistics
            .pending_to_send_buffer_size
            .store(socket_data.tcp_payloads.get_size(), Ordering::SeqCst);
    }

    #[cfg(feature = "statefull_serializer")]
    pub async fn apply_payload_to_serializer(&self, contract: &TContract) {
        let mut write_access = self.single_threaded.lock().await;

        if let Some(socket_data) = write_access.connection.as_mut() {
            socket_data.get_serializer_mut().apply_packet(contract);
        }
    }

    pub async fn set_connection_name(&self, name: &str) {
        let mut write_access = self.single_threaded.lock().await;
        write_access
            .log_context
            .as_mut()
            .unwrap()
            .insert("ConnectionName".to_string(), name.to_string());
    }
    pub async fn flush_to_socket(&self) {
        let mut write_access = self.single_threaded.lock().await;

        if let Some(socket_data) = write_access.connection.as_mut() {
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
                    write_access.log_context.clone(),
                );
                process_disconnect(&mut *write_access, self.id, &self.logger).await;
            }
        }
    }
    pub async fn send_ping(&self) {
        let mut single_threaded = self.single_threaded.lock().await;

        if let Some(socket_data) = &mut single_threaded.connection {
            if let Some(ping_payload) = &self.cached_ping_payload {
                self.add_payload_to_send(socket_data, ping_payload);
                return;
            }

            let ping_contract = socket_data.get_serializer().get_ping();
            let payload = socket_data.get_serializer().serialize(ping_contract);
            self.add_payload_to_send(socket_data, payload.as_slice());
        }
    }
}

async fn process_disconnect<TSerializer>(
    socket_connection: &mut SocketConnectionSingleThreaded<TSerializer>,
    id: ConnectionId,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
) {
    let mut result = None;
    std::mem::swap(&mut result, &mut socket_connection.connection);

    let mut result = result.unwrap();

    if let Err(err) = result.tcp_stream.shutdown().await {
        logger.write_info(
            "process_disconnect".to_string(),
            format!("Error while disconnecting socket {}. Err: {}", id, err),
            socket_connection.log_context.clone(),
        );
    }
}
