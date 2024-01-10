use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
};

use rust_extensions::{
    events_loop::{EventsLoop, EventsLoopTick},
    Logger,
};
use tokio::sync::Mutex;

use crate::TcpSocketSerializer;

use super::{
    tcp_connection::TcpThreadStatus, BufferToSendWrapper, ConnectionStatistics, TcpConnectionStream,
};

pub struct TcpConnectionInner<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: Default + Send + Sync + 'static,
> {
    pub stream: Mutex<TcpConnectionStream>,
    pub buffer_to_send_inner:
        Mutex<BufferToSendWrapper<TContract, TSerializer, TSerializationMetadata>>,
    max_send_payload_size: usize,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    pub logger: Arc<dyn Logger + Send + Sync + 'static>,
    pub threads_statistics: Arc<crate::ThreadsStatistics>,
    read_thread_status: AtomicI32,
    write_thread_status: AtomicI32,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: Default + Send + Sync + 'static,
    > TcpConnectionInner<TContract, TSerializer, TSerializationMetadata>
{
    pub fn new(
        stream: TcpConnectionStream,
        max_send_payload_size: usize,
        reusable_send_buffer_size: usize,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        threads_statistics: Arc<crate::ThreadsStatistics>,
    ) -> Self {
        Self {
            stream: Mutex::new(stream),
            buffer_to_send_inner: Mutex::new(BufferToSendWrapper::new(reusable_send_buffer_size)),
            max_send_payload_size,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            logger,
            threads_statistics,
            read_thread_status: AtomicI32::new(TcpThreadStatus::NotStarted.as_i32()),
            write_thread_status: AtomicI32::new(TcpThreadStatus::NotStarted.as_i32()),
        }
    }

    pub fn update_write_thread_status(&self, status: TcpThreadStatus) {
        self.write_thread_status
            .store(status.as_i32(), std::sync::atomic::Ordering::SeqCst);
    }

    pub fn update_read_thread_status(&self, status: TcpThreadStatus) {
        self.read_thread_status
            .store(status.as_i32(), std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get_read_thread_status(&self) -> TcpThreadStatus {
        self.read_thread_status
            .load(std::sync::atomic::Ordering::SeqCst)
            .into()
    }

    pub fn get_write_thread_status(&self) -> TcpThreadStatus {
        self.write_thread_status
            .load(std::sync::atomic::Ordering::SeqCst)
            .into()
    }

    pub async fn set_send_to_socket_event_loop(&self, send_to_socket_event_loop: EventsLoop<()>) {
        let mut write_access = self.buffer_to_send_inner.lock().await;
        write_access.events_loop = Some(send_to_socket_event_loop);
    }

    pub async fn push_contract(
        &self,
        contract: &TContract,
        meta_data: &TSerializationMetadata,
    ) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::create_serializer());
        }

        let serializer = write_access.serializer.take().unwrap();

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            serializer.serialize(tcp_buffer_chunk, contract, meta_data);
        });

        write_access.serializer = Some(serializer);

        result
    }

    pub async fn push_many_contracts(
        &self,
        contracts: &[TContract],
        meta_data: &TSerializationMetadata,
    ) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::create_serializer());
        }

        let serializer = write_access.serializer.take().unwrap();

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            for contract in contracts {
                serializer.serialize(tcp_buffer_chunk, contract, meta_data);
            }
        });

        write_access.serializer = Some(serializer);

        result
    }
    pub async fn send_ping(&self, meta_data: &TSerializationMetadata) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::create_serializer());
        }

        let serializer = write_access.serializer.take().unwrap();

        let ping = serializer.get_ping();

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            serializer.serialize(tcp_buffer_chunk, &ping, meta_data);
        });

        write_access.serializer = Some(serializer);

        result
    }

    pub async fn push_payload(&self, payload: &[u8]) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        write_access.push_payload(|tcp_buffer_chunk| {
            tcp_buffer_chunk.push_slice(payload);
        })
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

        let mut connection_has_error = false;

        {
            let mut write_access = self.stream.lock().await;

            while let Some(payload) =
                payload_to_send.get_next_slice_to_send(self.max_send_payload_size)
            {
                if write_access.send_payload_to_tcp_connection(payload).await {
                    connection_has_error = true;
                    break;
                }
            }
        }

        if connection_has_error {
            self.disconnect().await;
        } else {
            let mut inner = self.buffer_to_send_inner.lock().await;
            if let Some(buffer_to_send) = &mut inner.buffer_to_send {
                buffer_to_send.reuse_payload(payload_to_send);
            }
        }
    }

    pub async fn disconnect(&self) -> bool {
        let just_disconnected = {
            let mut tcp_stream = self.stream.lock().await;

            tcp_stream.disconnect()
        };

        if just_disconnected {
            self.connected
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.statistics.disconnect();

            let mut inner = self.buffer_to_send_inner.lock().await;

            inner.buffer_to_send = None;
            if inner.events_loop_is_started {
                if let Some(events_loop) = &mut inner.events_loop {
                    events_loop.stop();
                }
            }
        }
        just_disconnected
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
impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: Default + Send + Sync + 'static,
    > EventsLoopTick<()> for TcpConnectionInner<TContract, TSerializer, TSerializationMetadata>
{
    async fn started(&self) {
        //println!("EventsLoop started: {:?}", self.get_log_context().await);

        self.update_write_thread_status(TcpThreadStatus::Started);
        self.threads_statistics.write_threads.increase();
    }

    async fn tick(&self, _: ()) {
        self.push_send_buffer_to_connection().await;
    }

    async fn finished(&self) {
        // println!("EventsLoop finished: {:?}", self.get_log_context().await);
        self.update_write_thread_status(TcpThreadStatus::Finished);
        self.threads_statistics.write_threads.decrease();
    }
}
