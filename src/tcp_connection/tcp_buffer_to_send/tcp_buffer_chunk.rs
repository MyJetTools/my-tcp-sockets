pub struct TcpBufferChunk {
    pub data: Vec<u8>,
    pub pos_to_send: usize,

    #[cfg(test)]
    pub created: rust_extensions::date_time::DateTimeAsMicroseconds,
}

impl TcpBufferChunk {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::with_capacity(max_size),
            pos_to_send: 0,
            #[cfg(test)]
            created: rust_extensions::date_time::DateTimeAsMicroseconds::now(),
        }
    }

    pub fn append<'s>(&mut self, new_data: &'s [u8]) -> Option<&'s [u8]> {
        let size_after = self.data.len() + new_data.len();
        if size_after <= self.data.capacity() {
            self.data.extend_from_slice(new_data);
            return None;
        }

        let size_to_upload = self.data.capacity() - self.data.len();

        self.data.extend_from_slice(&new_data[..size_to_upload]);

        Some(&new_data[size_to_upload..])
    }

    #[cfg(test)]
    pub fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn has_buffer_to_fill(&self) -> bool {
        self.data.len() < self.data.capacity()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    pub fn reset(&mut self) {
        self.pos_to_send = 0;
        self.data.clear();
    }

    pub fn get_slice_to_send(&mut self, max_buffer_size: usize) -> Option<&[u8]> {
        if self.pos_to_send >= self.len() {
            return None;
        }

        let available_to_send = self.len() - self.pos_to_send;
        let to_send = if available_to_send < max_buffer_size {
            available_to_send
        } else {
            max_buffer_size
        };

        let result = &self.data[self.pos_to_send..self.pos_to_send + to_send];

        self.pos_to_send += to_send;

        Some(result)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_overload() {
        let mut chunk = super::TcpBufferChunk::new(5);

        let data_to_add = vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8];

        let remaining = chunk.append(&data_to_add);

        assert_eq!(chunk.as_slice(), &[0u8, 1u8, 2u8, 3u8, 4u8]);

        assert_eq!(remaining.unwrap(), &[5u8, 6u8, 7u8, 8u8, 9u8]);
    }
}
