use std::sync::Arc;

use rust_extensions::events_loop::EventsLoop;

use crate::TcpSocketSerializer;

use super::{TcpBufferChunk, TcpBufferToSend, TcpConnectionStates};

pub struct BufferToSendWrapper<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: Default + Send + Sync + 'static,
> {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub events_loop: Option<EventsLoop<()>>,
    pub events_loop_is_started: bool,
    pub serializer: Option<TSerializer>,
    phantom_contract: std::marker::PhantomData<TContract>,
    pub meta_data: Option<TSerializationMetadata>,
}

impl<
        TContract: Send + Sync + 'static,
        TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
        TSerializationMetadata: Default + Send + Sync + 'static,
    > BufferToSendWrapper<TContract, TSerializer, TSerializationMetadata>
{
    pub fn new() -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::new()),
            events_loop: None,
            events_loop_is_started: false,
            serializer: None,
            phantom_contract: std::marker::PhantomData,
            meta_data: None,
        }
    }

    pub fn push_payload(&mut self, add_payload: impl Fn(&mut TcpBufferChunk) -> ()) -> usize {
        let mut result = 0;
        if let Some(buffer_to_send) = self.buffer_to_send.as_mut() {
            result = buffer_to_send.add_payload_directly_to_chunk(add_payload);

            if !self.events_loop_is_started {
                if let Some(events_loop) = &mut self.events_loop {
                    let tcp_connection_states = TcpConnectionStates::new();
                    events_loop.start(Arc::new(tcp_connection_states));
                    self.events_loop_is_started = true;
                } else {
                    panic!("Events loop is not set");
                }
            }

            if let Some(events_loop) = &self.events_loop {
                events_loop.send(());
            }
        }

        result
    }
}
