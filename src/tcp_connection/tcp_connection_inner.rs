use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
};

use rust_extensions::{
    events_loop::{EventsLoopPublisher, EventsLoopTick},
    Logger,
};
use tokio::sync::Mutex;

use crate::{TcpSerializationMetadata, TcpSocketSerializer};

use super::{
    tcp_connection::TcpThreadStatus, BufferToSendWrapper, ConnectionStatistics, TcpConnectionStream,
};

pub struct TcpConnectionInner<
    TContract: Send + Sync + 'static,
    TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializationMetadata<TContract> + Default + Send + Sync + 'static,
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
        TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: TcpSerializationMetadata<TContract> + Default + Send + Sync + 'static,
    > TcpConnectionInner<TContract, TSerializer, TSerializationMetadata>
{
    pub fn new(
        stream: TcpConnectionStream,
        max_send_payload_size: usize,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        threads_statistics: Arc<crate::ThreadsStatistics>,
        events_loop_publisher: EventsLoopPublisher<()>,
    ) -> Self {
        Self {
            stream: Mutex::new(stream),
            buffer_to_send_inner: Mutex::new(BufferToSendWrapper::new(events_loop_publisher)),
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

    pub async fn push_contract(&self, contract: &TContract) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::default());
        }

        if write_access.meta_data.is_none() {
            write_access.meta_data = Some(TSerializationMetadata::default());
        }

        let serializer = write_access.serializer.take().unwrap();

        let meta_data = write_access.meta_data.take().unwrap();

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::default());
        }

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            serializer.serialize(tcp_buffer_chunk, contract, &meta_data);
        });

        write_access.serializer = Some(serializer);
        write_access.meta_data = Some(meta_data);

        result
    }

    pub async fn push_many_contracts(&self, contracts: &[TContract]) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::default());
        }

        if write_access.meta_data.is_none() {
            write_access.meta_data = Some(TSerializationMetadata::default());
        }

        let serializer = write_access.serializer.take().unwrap();
        let meta_data = write_access.meta_data.take().unwrap();

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            for contract in contracts {
                serializer.serialize(tcp_buffer_chunk, contract, &meta_data);
            }
        });

        write_access.serializer = Some(serializer);

        write_access.meta_data = Some(meta_data);

        result
    }
    pub async fn send_ping(&self) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        if write_access.serializer.is_none() {
            write_access.serializer = Some(TSerializer::default());
        }

        if write_access.meta_data.is_none() {
            write_access.meta_data = Some(TSerializationMetadata::default());
        }

        let serializer = write_access.serializer.take().unwrap();

        let ping = serializer.get_ping();

        let meta_data = write_access.meta_data.take().unwrap();

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            serializer.serialize(tcp_buffer_chunk, &ping, &meta_data);
        });

        write_access.serializer = Some(serializer);

        write_access.meta_data = Some(meta_data);

        result
    }

    pub async fn push_payload(&self, payload: &[u8]) -> usize {
        let mut write_access = self.buffer_to_send_inner.lock().await;

        let result = write_access.push_payload(|tcp_buffer_chunk| {
            tcp_buffer_chunk.push_slice(payload);
        });

        result
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
        }

        {
            let mut inner = self.buffer_to_send_inner.lock().await;
            inner.buffer_to_send = None;
            if let Some(events_loop_publisher) = inner.events_loop_publisher.take() {
                events_loop_publisher.stop();
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
        TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: TcpSerializationMetadata<TContract> + Default + Send + Sync + 'static,
    > EventsLoopTick<()> for TcpConnectionInner<TContract, TSerializer, TSerializationMetadata>
{
    async fn started(&self) {
        //  println!("EventsLoop started: {:?}", self.get_log_context().await);

        self.update_write_thread_status(TcpThreadStatus::Started);
        self.threads_statistics.write_threads.increase();
    }

    async fn tick(&self, _: ()) {
        self.push_send_buffer_to_connection().await;
    }

    async fn finished(&self) {
        //        println!("EventsLoop finished: {:?}", self.get_log_context().await);
        self.update_write_thread_status(TcpThreadStatus::Finished);
        self.threads_statistics.write_threads.decrease();
    }
}
