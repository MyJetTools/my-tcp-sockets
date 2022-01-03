pub struct ReadBuffer {
    buffer: Vec<u8>,
    pos_start: usize,
    pos_end: usize,
    max_capacity: usize,
    packet_end: usize,
}

impl ReadBuffer {
    pub fn new(max_capacity: usize) -> Self {
        let mut result = Self {
            max_capacity,
            buffer: Vec::with_capacity(max_capacity),
            pos_start: 0,
            pos_end: 0,
            packet_end: 0,
        };

        result.buffer.resize(max_capacity, 0);

        result
    }

    fn gc(&mut self) {
        if self.pos_start == 0 {
            return;
        }

        if self.pos_start == self.pos_end {
            self.pos_start = 0;
            self.pos_end = 0;
            return;
        }

        let pos_start = self.pos_start;
        self.buffer.drain(..pos_start);
        unsafe {
            self.buffer.set_len(self.max_capacity);
        }

        self.pos_start = 0;
        self.pos_end -= pos_start;
    }

    pub fn get_buffer_to_write(&mut self) -> Option<&mut [u8]> {
        if self.pos_end == self.buffer.capacity() {
            if self.pos_start == 0 {
                return None;
            }

            self.gc()
        }

        return Some(&mut self.buffer[self.pos_end..]);
    }

    pub fn commit_written_size(&mut self, size: usize) {
        self.pos_end += size;
    }

    pub fn find_sequence<'s>(&'s mut self, sequence_to_find: &[u8]) -> bool {
        if self.packet_end > self.pos_start {
            self.pos_start = self.packet_end;

            if self.pos_start == self.pos_end {
                self.gc();
            }
        }

        let buf_len = self.pos_end - self.pos_start;
        if buf_len < sequence_to_find.len() {
            self.gc();
            return false;
        }

        for index in self.pos_start..=self.pos_end - sequence_to_find.len() {
            if arrays_are_equal(
                &self.buffer[index..index + sequence_to_find.len()],
                sequence_to_find,
            ) {
                self.packet_end = index + sequence_to_find.len();
                return true;
            }
        }

        self.gc();
        false
    }

    pub fn get_package(&self) -> &[u8] {
        if self.packet_end > self.pos_start {
            return &self.buffer[self.pos_start..self.packet_end];
        }

        panic!("You are reading the packet which is not detected yet");
    }
}

fn arrays_are_equal(a1: &[u8], a2: &[u8]) -> bool {
    if a1.len() != a2.len() {
        return false;
    }

    for index in 0..a1.len() {
        if a1[index] != a2[index] {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_eq_algo() {
        let data1 = [0u8; 3];
        let data2 = [0u8; 3];

        assert_eq!(true, arrays_are_equal(&data1[..], &data2[..]));
    }

    #[test]
    fn test_brand_new_read_buffer() {
        const MAX_CAPACITY: usize = 10;
        let read_buffer = ReadBuffer::new(MAX_CAPACITY);

        assert_eq!(MAX_CAPACITY, read_buffer.max_capacity);
        assert_eq!(0, read_buffer.pos_start);
        assert_eq!(0, read_buffer.pos_end);
        assert_eq!(MAX_CAPACITY, read_buffer.buffer.len());
    }

    #[test]
    fn test_we_have_exact_amount_of_data() {
        let mut read_buffer = ReadBuffer::new(10);
        let seq_to_fine = vec![13u8, 10u8];

        {
            let buffer_to_write = read_buffer.get_buffer_to_write().unwrap();

            assert_eq!(10, buffer_to_write.len());

            buffer_to_write[0] = 51;
            buffer_to_write[1] = 52;
            buffer_to_write[2] = 53;
            buffer_to_write[3] = 13;
            buffer_to_write[4] = 10;
        }

        read_buffer.commit_written_size(5);

        let found_result = read_buffer.find_sequence(seq_to_fine.as_ref());

        assert_eq!(true, found_result);

        assert_eq!(read_buffer.get_package().to_vec(), vec![51, 52, 53, 13, 10]);
    }

    #[test]
    fn test_we_two_payloads_exact_size() {
        let mut read_buffer = ReadBuffer::new(10);
        let seq_to_fine = vec![13u8, 10u8];

        {
            let buffer_to_write = read_buffer.get_buffer_to_write().unwrap();

            assert_eq!(10, buffer_to_write.len());

            buffer_to_write[0] = 51;
            buffer_to_write[1] = 52;
            buffer_to_write[2] = 53;
            buffer_to_write[3] = 13;
            buffer_to_write[4] = 10;

            buffer_to_write[5] = 54;
            buffer_to_write[6] = 55;
            buffer_to_write[7] = 56;
            buffer_to_write[8] = 13;
            buffer_to_write[9] = 10;
        }

        read_buffer.commit_written_size(10);

        let found_result = read_buffer.find_sequence(seq_to_fine.as_ref());
        assert_eq!(true, found_result);
        assert_eq!(read_buffer.get_package().to_vec(), vec![51, 52, 53, 13, 10]);

        let found_result = read_buffer.find_sequence(seq_to_fine.as_ref());
        assert_eq!(true, found_result);
        assert_eq!(read_buffer.get_package().to_vec(), vec![54, 55, 56, 13, 10]);

        let found_result = read_buffer.find_sequence(seq_to_fine.as_ref());

        assert_eq!(false, found_result);

        assert_eq!(0, read_buffer.pos_start);
        assert_eq!(0, read_buffer.pos_end);
    }
}
