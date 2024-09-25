use rust_extensions::auto_shrink::VecDequeAutoShrink;

use super::TcpBufferChunk;

pub struct TcpBufferToSend {
    pub payloads: VecDequeAutoShrink<TcpBufferChunk>,
}

impl Default for TcpBufferToSend {
    fn default() -> Self {
        Self {
            payloads: VecDequeAutoShrink::new(16),
        }
    }
}

impl TcpBufferToSend {
    fn get_payload_to_append(&mut self) -> &mut TcpBufferChunk {
        if self.payloads.is_empty() {
            self.payloads.push_back(TcpBufferChunk::default());
        }

        self.payloads.get_mut(0).unwrap()
    }

    pub fn add_payload(&mut self, payload: &[u8]) {
        let chunk = self.get_payload_to_append();

        chunk.push_slice(payload);
    }

    pub fn add_payload_directly_to_chunk(
        &mut self,
        tcp_buffer_chunk: impl Fn(&mut TcpBufferChunk),
    ) -> usize {
        let chunk = self.get_payload_to_append();
        let len_before = chunk.len();

        tcp_buffer_chunk(chunk);

        chunk.len() - len_before
    }

    pub fn get_payload(&mut self) -> Option<TcpBufferChunk> {
        self.payloads.pop_front()
    }

    pub fn get_size(&self) -> usize {
        let mut result = 0;

        for payload in self.payloads.iter() {
            result += payload.len();
        }

        result
    }

    pub fn reuse_payload(&mut self, mut payload: TcpBufferChunk) {
        if self.payloads.len() < 2 {
            payload.reset();
            self.payloads.push_back(payload)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_less_amount_than_buffer() {
        let mut tcp_payloads = TcpBufferToSend::default();

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        let first_chunk_created = chunk.created.unix_microseconds;

        assert_eq!(&[1, 2, 3, 4, 5], chunk.get_next_slice_to_send(10).unwrap());

        assert!(chunk.get_next_slice_to_send(10).is_none());

        tcp_payloads.reuse_payload(chunk);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5]);
        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(first_chunk_created, chunk.created.unix_microseconds);

        assert_eq!(&[1, 2, 3, 4, 5], chunk.get_next_slice_to_send(10).unwrap());

        assert!(chunk.get_next_slice_to_send(10).is_none());

        tcp_payloads.reuse_payload(chunk);
    }

    #[test]
    fn test_less_amount_exactly_as_buffer() {
        let mut tcp_payloads = TcpBufferToSend::default();

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk.get_next_slice_to_send(10).unwrap()
        );

        assert!(chunk.get_next_slice_to_send(10).is_none());

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_amount_bugger_than_reusable_buffer() {
        let mut tcp_payloads = TcpBufferToSend::default();

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk.get_next_slice_to_send(10).unwrap()
        );

        assert_eq!(&[11, 12,], chunk.get_next_slice_to_send(10).unwrap());

        assert!(chunk.get_next_slice_to_send(10).is_none());
    }

    #[test]
    fn test_we_add_to_the_same_chunk() {
        let mut tcp_payloads = TcpBufferToSend::default();

        tcp_payloads.add_payload(&[1, 2, 3, 4]);

        tcp_payloads.add_payload(&[5, 6, 7]);

        tcp_payloads.add_payload(&[8, 9, 10, 11, 12]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk.reusable_buffer.as_slice()
        );

        assert_eq!(&[11, 12], chunk.additional_buffer.as_slice());

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk.get_next_slice_to_send(10).unwrap()
        );

        assert_eq!(&[11, 12], chunk.get_next_slice_to_send(10).unwrap());

        assert!(chunk.get_next_slice_to_send(10).is_none());
    }
}
