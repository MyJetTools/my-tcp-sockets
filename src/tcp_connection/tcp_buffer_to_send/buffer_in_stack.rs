#[cfg(not(test))]
const REUSABLE_BUFFER_CAPACITY: usize = 65535;

#[cfg(test)]
const REUSABLE_BUFFER_CAPACITY: usize = 10;

pub struct BufferInStack {
    len: usize,
    data: [u8; REUSABLE_BUFFER_CAPACITY],
}

impl BufferInStack {
    pub fn new() -> Self {
        Self {
            data: [0; REUSABLE_BUFFER_CAPACITY],
            len: 0,
        }
    }

    pub fn push_byte(&mut self, b: u8) -> bool {
        if self.len == REUSABLE_BUFFER_CAPACITY {
            return false;
        }

        self.data[self.len] = b;
        self.len += 1;
        true
    }

    pub fn push_as_much_as_possible<'s>(&mut self, slice: &'s [u8]) -> Option<&'s [u8]> {
        if self.len == REUSABLE_BUFFER_CAPACITY {
            return Some(slice);
        }

        let remaining_capacity = REUSABLE_BUFFER_CAPACITY - self.len;

        if remaining_capacity > slice.len() {
            let new_len = self.len + slice.len();
            self.data[self.len..new_len].copy_from_slice(slice);
            self.len = new_len;
            return None;
        }

        self.data[self.len..].copy_from_slice(&slice[..remaining_capacity]);
        self.len = REUSABLE_BUFFER_CAPACITY;
        Some(&slice[remaining_capacity..])
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
}
