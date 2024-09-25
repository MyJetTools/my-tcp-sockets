use rust_extensions::events_loop::EventsLoopPublisher;

use crate::TcpSocketSerializer;

use super::{TcpBufferChunk, TcpBufferToSend};

pub enum PublishResult {
    EventLoopIsNotStarted,
    Disconnected,
}

pub struct BufferToSendWrapper<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
    TSerializerState: Send + Sync + 'static,
> {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub serializer: Option<TSerializer>,
    phantom_contract: std::marker::PhantomData<TContract>,
    pub serializer_state: Option<TSerializerState>,
    pub events_loop_publisher: Option<EventsLoopPublisher<()>>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
        TSerializerState: Send + Sync + 'static,
    > BufferToSendWrapper<TContract, TSerializer, TSerializerState>
{
    pub fn new(
        serializer: TSerializer,
        serializer_state: TSerializerState,
        events_loop_publisher: EventsLoopPublisher<()>,
    ) -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::default()),

            serializer: Some(serializer),
            phantom_contract: std::marker::PhantomData,
            serializer_state: Some(serializer_state),
            events_loop_publisher: Some(events_loop_publisher),
        }
    }

    pub fn push_payload(&mut self, add_payload: impl Fn(&mut TcpBufferChunk)) -> usize {
        let mut result = 0;
        if let Some(buffer_to_send) = self.buffer_to_send.as_mut() {
            result = buffer_to_send.add_payload_directly_to_chunk(add_payload);

            if let Some(events_loop) = self.events_loop_publisher.as_ref() {
                events_loop.send(());
            }
        }

        result
    }
}
