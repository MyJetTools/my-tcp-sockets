use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};

use rust_extensions::{
    events_loop::{EventsLoop, EventsLoopTick},
    Logger,
};
use tokio::sync::Mutex;

use super::{ConnectionStatistics, TcpBufferToSend, TcpConnectionStates, TcpConnectionStream};

pub struct BufferToSendInner {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub events_loop: Option<EventsLoop<()>>,
    pub events_loop_is_started: bool,
}

impl BufferToSendInner {
    pub fn new() -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::new(1024 * 1024)),
            events_loop: None,
            events_loop_is_started: false,
        }
    }
}

pub struct TcpConnectionInner {
    pub stream: Mutex<TcpConnectionStream>,
    pub buffer_to_send_inner: Mutex<BufferToSendInner>,
    max_send_payload_size: usize,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub threads_statistics: Arc<crate::ThreadsStatistics>,
}

impl TcpConnectionInner {
    pub fn new(
        stream: TcpConnectionStream,
        max_send_payload_size: usize,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        threads_statistics: Arc<crate::ThreadsStatistics>,
    ) -> Self {
        Self {
            stream: Mutex::new(stream),
            buffer_to_send_inner: Mutex::new(BufferToSendInner::new()),
            max_send_payload_size,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            logger,
            threads_statistics,
        }
    }

    pub async fn set_send_to_socket_event_loop(&self, send_to_socket_event_loop: EventsLoop<()>) {
        let mut write_access = self.buffer_to_send_inner.lock().await;
        write_access.events_loop = Some(send_to_socket_event_loop);
    }

    pub async fn push_payload_to_send_buffer(&self, payload: &[u8]) {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if let Some(buffer_to_send) = write_access.buffer_to_send.as_mut() {
            buffer_to_send.add_payload(payload);

            if !write_access.events_loop_is_started {
                if let Some(events_loop) = &write_access.events_loop {
                    let tcp_connection_states = TcpConnectionStates::new();
                    events_loop.start(Arc::new(tcp_connection_states)).await;
                    write_access.events_loop_is_started = true;
                } else {
                    panic!("Events loop is not set");
                }
            }
        }
    }

    pub async fn push_send_buffer_to_connection(&self) {
        let payload_to_send = {
            let mut inner = self.buffer_to_send_inner.lock().await;

            if let Some(buffer_to_send) = &mut inner.buffer_to_send {
                buffer_to_send.get_payload()
            } else {
                None
            }
        };

        if payload_to_send.is_none() {
            return;
        }

        let mut payload_to_send = payload_to_send.unwrap();

        let mut disconnected = false;

        {
            let mut write_access = self.stream.lock().await;

            while let Some(payload) =
                payload_to_send.get_next_slice_to_send(self.max_send_payload_size)
            {
                if write_access.send_payload_to_tcp_connection(payload).await {
                    if self.process_disconnect(&mut write_access) {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        let mut inner = self.buffer_to_send_inner.lock().await;
        if disconnected {
            inner.buffer_to_send = None;
            if inner.events_loop_is_started {
                if let Some(events_loop) = &mut inner.events_loop {
                    events_loop.stop();
                }
            }
        } else {
            if let Some(buffer_to_send) = &mut inner.buffer_to_send {
                buffer_to_send.reuse_payload(payload_to_send);
            }
        }
    }

    pub async fn disconnect(&self) -> bool {
        let mut write_access = self.stream.lock().await;
        self.process_disconnect(&mut write_access)
    }

    fn process_disconnect(&self, inner: &mut TcpConnectionStream) -> bool {
        let result = inner.disconnect();
        if result {
            self.connected
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.statistics.disconnect();
        }
        result
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn get_log_context(&self) -> HashMap<String, String> {
        let read_access = self.stream.lock().await;
        read_access.get_log_context().clone()
    }
}

#[async_trait::async_trait]
impl EventsLoopTick<()> for TcpConnectionInner {
    async fn started(&self) {
        self.threads_statistics.increase_read_threads();
    }

    async fn tick(&self, _: ()) {
        self.push_send_buffer_to_connection().await;
    }

    async fn finished(&self) {
        self.threads_statistics.decrease_read_threads();
    }
}