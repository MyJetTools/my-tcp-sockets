use std::sync::Arc;

use rust_extensions::events_loop::EventsLoop;

use super::{TcpBufferChunk, TcpBufferToSend, TcpConnectionStates};

pub struct BufferToSendWrapper {
    pub buffer_to_send: Option<TcpBufferToSend>,
    pub events_loop: Option<EventsLoop<()>>,
    pub events_loop_is_started: bool,
}

impl BufferToSendWrapper {
    pub fn new(reusable_send_buffer_size: usize) -> Self {
        Self {
            buffer_to_send: Some(TcpBufferToSend::new(reusable_send_buffer_size)),
            events_loop: None,
            events_loop_is_started: false,
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
