use rust_extensions::events_loop::EventsLoopPublisher;

use crate::TcpSocketSerializer;

use super::{TcpBufferChunk, TcpBufferToSend};

pub enum PublishResult {
    EventLoopIsNotStarted,
    Disconnected,
}

pub struct BufferToSendWrapper<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: Default + Send + Sync + 'static,
> {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub serializer: Option<TSerializer>,
    phantom_contract: std::marker::PhantomData<TContract>,
    pub meta_data: Option<TSerializationMetadata>,
    pub events_loop_publisher: Option<EventsLoopPublisher<()>>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: Default + Send + Sync + 'static,
    > BufferToSendWrapper<TContract, TSerializer, TSerializationMetadata>
{
    pub fn new(events_loop_publisher: EventsLoopPublisher<()>) -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::new()),

            serializer: None,
            phantom_contract: std::marker::PhantomData,
            meta_data: None,
            events_loop_publisher: Some(events_loop_publisher),
        }
    }

    pub fn push_payload(&mut self, add_payload: impl Fn(&mut TcpBufferChunk) -> ()) -> usize {
        let mut result = 0;
        if let Some(buffer_to_send) = self.buffer_to_send.as_mut() {
            result = buffer_to_send.add_payload_directly_to_chunk(add_payload);

            if let Some(events_loop) = self.events_loop_publisher.as_ref() {
                events_loop.send(());
            }

            /*
            if !self.events_loop_is_started {
                let tcp_connection_states = TcpConnectionStates::new();
                self.events_loop.start(Arc::new(tcp_connection_states));
                self.events_loop_is_started = true;
            }

            self.events_loop.send(());
             */
        }

        result
    }
}
