use rust_extensions::auto_shrink::VecDequeAutoShrink;

use super::TcpBufferChunk;

pub struct TcpBufferToSend {
    pub payloads: VecDequeAutoShrink<TcpBufferChunk>,
    reusable_buffer_size: usize,
}

impl TcpBufferToSend {
    pub fn new(reusable_buffer_size: usize) -> Self {
        Self {
            payloads: VecDequeAutoShrink::new(16),
            reusable_buffer_size,
        }
    }

    fn get_payload_to_append(&mut self, payload_size: usize) -> &mut TcpBufferChunk {
        let len = self.payloads.len();

        if self.payloads.len() > 0 {
            let index = self
                .payloads
                .iter()
                .position(|itm| itm.has_buffer_to_fill());

            if let Some(index) = index {
                return self.payloads.get_mut(index).unwrap();
            }
        }

        let chink = if payload_size > self.reusable_buffer_size {
            TcpBufferChunk::new(payload_size)
        } else {
            TcpBufferChunk::new(self.reusable_buffer_size)
        };

        self.payloads.push_back(chink);
        self.payloads.get_mut(len).unwrap()
    }

    pub fn add_payload(&mut self, mut payload: &[u8]) {
        loop {
            let chunk = self.get_payload_to_append(payload.len());

            let remains = chunk.append(payload);

            if remains.is_none() {
                break;
            }

            payload = remains.unwrap();
        }
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
        if payload.capacity() != self.reusable_buffer_size {
            return;
        }

        let mut amount_of_payloads_with_standard_capacity = 0;

        for chunk in self.payloads.iter() {
            if chunk.capacity() == self.reusable_buffer_size {
                amount_of_payloads_with_standard_capacity += 1;
            }

            if amount_of_payloads_with_standard_capacity >= 2 {
                return;
            }
        }

        if amount_of_payloads_with_standard_capacity < 2 {
            payload.reset();
            self.payloads.push_back(payload);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_less_amount_than_buffer() {
        let mut tcp_payloads = TcpBufferToSend::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        let first_chunk_created = chunk.created.unix_microseconds;

        assert_eq!(&[1, 2, 3, 4, 5], chunk.get_slice_to_send(10).unwrap());

        assert!(chunk.get_slice_to_send(10).is_none());

        tcp_payloads.reuse_payload(chunk);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5]);
        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(first_chunk_created, chunk.created.unix_microseconds);

        assert_eq!(&[1, 2, 3, 4, 5], chunk.get_slice_to_send(10).unwrap());

        assert!(chunk.get_slice_to_send(10).is_none());

        tcp_payloads.reuse_payload(chunk);
    }

    #[test]
    fn test_less_amount_exactly_as_buffer() {
        let mut tcp_payloads = TcpBufferToSend::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_amount_bugger_than_buffer() {
        let mut tcp_payloads = TcpBufferToSend::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        let mut chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk.get_slice_to_send(10).unwrap()
        );

        assert_eq!(&[11, 12,], chunk.get_slice_to_send(10).unwrap());

        assert!(chunk.get_slice_to_send(10).is_none());
    }

    #[test]
    fn test_we_add_to_the_same_chunk() {
        let mut tcp_payloads = TcpBufferToSend::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4]);

        tcp_payloads.add_payload(&[5, 6, 7]);

        tcp_payloads.add_payload(&[8, 9, 10, 11, 12]);

        let chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], chunk.as_slice());

        let chunk = tcp_payloads.get_payload().unwrap();

        assert_eq!(chunk.capacity(), 10);
        assert_eq!(&[11, 12], chunk.as_slice());
    }
}
