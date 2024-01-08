use crate::TcpWriteBuffer;

pub struct TcpBufferChunk {
    pub reusable_data_is_sent: bool,
    pub reusable_buffer: Vec<u8>,
    pub pos_to_send: usize,
    pub additional_buffer: Vec<u8>,
    #[cfg(test)]
    pub created: rust_extensions::date_time::DateTimeAsMicroseconds,
}

impl TcpBufferChunk {
    pub fn new(max_size: usize) -> Self {
        Self {
            reusable_buffer: Vec::with_capacity(max_size),
            reusable_data_is_sent: false,
            pos_to_send: 0,
            additional_buffer: Vec::new(),
            #[cfg(test)]
            created: rust_extensions::date_time::DateTimeAsMicroseconds::now(),
        }
    }

    pub fn push_byte(&mut self, b: u8) {
        let reusable_buffer_capacity = self.reusable_buffer.capacity();
        if self.reusable_buffer.len() == reusable_buffer_capacity {
            self.additional_buffer.push(b);
            return;
        }

        let size_after = self.reusable_buffer.len() + 1;
        if size_after <= reusable_buffer_capacity {
            self.reusable_buffer.push(b);
            return;
        }

        self.reusable_buffer.push(b);
    }

    pub fn push_slice<'s>(&mut self, new_data: &'s [u8]) {
        let reusable_buffer_capacity = self.reusable_buffer.capacity();
        if self.reusable_buffer.len() == reusable_buffer_capacity {
            self.additional_buffer.extend_from_slice(new_data);
            return;
        }

        let size_after = self.reusable_buffer.len() + new_data.len();
        if size_after <= reusable_buffer_capacity {
            self.reusable_buffer.extend_from_slice(new_data);
            return;
        }

        let size_to_upload = self.reusable_buffer.capacity() - self.reusable_buffer.len();

        self.reusable_buffer
            .extend_from_slice(&new_data[..size_to_upload]);

        self.additional_buffer
            .extend_from_slice(&new_data[size_to_upload..]);
    }

    pub fn has_buffer_to_fill(&self) -> bool {
        self.reusable_buffer.len() < self.reusable_buffer.capacity()
    }

    pub fn len(&self) -> usize {
        self.reusable_buffer.len() + self.additional_buffer.len()
    }

    pub fn capacity(&self) -> usize {
        self.reusable_buffer.capacity()
    }

    pub fn reset(&mut self) {
        self.pos_to_send = 0;
        self.reusable_buffer.clear();
        self.reusable_data_is_sent = false;
        self.additional_buffer.clear();
        self.additional_buffer.shrink_to_fit();
    }

    pub fn get_next_slice_to_send(&mut self, max_buffer_size: usize) -> Option<&[u8]> {
        if !self.reusable_data_is_sent {
            self.reusable_data_is_sent = true;
            return Some(self.reusable_buffer.as_slice());
        }

        if self.pos_to_send >= self.additional_buffer.len() {
            return None;
        }

        let remain_to_send = self.additional_buffer.len() - self.pos_to_send;

        let size_to_send = if remain_to_send > max_buffer_size {
            max_buffer_size
        } else {
            remain_to_send
        };

        let slice_to_send =
            &self.additional_buffer[self.pos_to_send..self.pos_to_send + size_to_send];

        self.pos_to_send += size_to_send;

        Some(slice_to_send)
    }
}

impl TcpWriteBuffer for TcpBufferChunk {
    fn write_byte(&mut self, b: u8) {
        self.push_byte(b);
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.push_slice(slice)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_overload() {
        let mut chunk = super::TcpBufferChunk::new(5);

        let data_to_add = vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8];

        chunk.push_slice(&data_to_add);

        assert_eq!(chunk.reusable_buffer.as_slice(), &[0u8, 1u8, 2u8, 3u8, 4u8]);

        assert_eq!(chunk.additional_buffer, &[5u8, 6u8, 7u8, 8u8, 9u8]);
    }
}
