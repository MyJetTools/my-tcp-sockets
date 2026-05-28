use std::sync::Weak;

use rust_extensions::background_executor::BackgroundExecutor;

use crate::TcpSocketSerializer;

use super::{TcpBufferChunk, TcpBufferToSend};

pub struct BufferToSendWrapper<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
    TSerializerState: Send + Sync + 'static,
> {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub serializer: Option<TSerializer>,
    phantom_contract: std::marker::PhantomData<TContract>,
    pub serializer_state: Option<TSerializerState>,
    pub background_executor: Weak<BackgroundExecutor>,
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
        background_executor: Weak<BackgroundExecutor>,
    ) -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::default()),

            serializer: Some(serializer),
            phantom_contract: std::marker::PhantomData,
            serializer_state: Some(serializer_state),
            background_executor,
        }
    }

    pub fn push_payload(&mut self, add_payload: impl Fn(&mut TcpBufferChunk)) -> usize {
        let mut result = 0;
        if let Some(buffer_to_send) = self.buffer_to_send.as_mut() {
            result = buffer_to_send.add_payload_directly_to_chunk(add_payload);

            if let Some(background_executor) = self.background_executor.upgrade() {
                background_executor.trigger();
            }
        }

        result
    }
}
