const READ_BUFFER_SIZE: usize = 16384;

pub struct MyReadBuffer {
    data: [u8; READ_BUFFER_SIZE],
    start_pos: usize,
    data_len: usize,
}

impl MyReadBuffer {
    pub fn new() -> Self {
        Self {
            data: [0; READ_BUFFER_SIZE],
            start_pos: 0,
            data_len: 0,
        }
    }

    pub fn data_len_ready(&self) -> usize {
        self.data_len - self.start_pos
    }

    pub fn advance_data_len(&mut self, size: usize) {
        self.data_len += size;
    }

    fn fix_start_pos_if_possible(&mut self) {
        if self.start_pos == self.data_len {
            self.start_pos = 0;
            self.data_len = 0;
        }
    }

    pub fn read_byte(&mut self) -> Option<u8> {
        if self.data_len_ready() == 0 {
            return None;
        }

        let result = self.data[self.start_pos];

        self.start_pos += 1;

        self.fix_start_pos_if_possible();

        Some(result)
    }

    pub fn write_into_target(&mut self, target: &mut [u8]) -> usize {
        let data_len = self.data_len_ready();
        if data_len == 0 {
            return 0;
        }

        let result = if data_len > target.len() {
            target.len()
        } else {
            data_len
        };

        target[..result].copy_from_slice(&self.data[self.start_pos..self.start_pos + result]);

        self.start_pos += result;
        self.fix_start_pos_if_possible();

        result
    }

    pub fn get_buffer_to_write(&mut self) -> &mut [u8] {
        &mut self.data[self.data_len..]
    }
}
