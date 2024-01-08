pub trait TcpWriteBuffer {
    fn write_byte(&mut self, b: u8);
    fn write_slice(&mut self, slice: &[u8]);

    fn write_bool(&mut self, v: bool) {
        if v {
            self.write_byte(1)
        } else {
            self.write_byte(0);
        }
    }

    fn write_i16(&mut self, v: i16) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_u16(&mut self, v: u16) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_i32(&mut self, v: i32) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_u32(&mut self, v: u32) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_i64(&mut self, v: i64) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_u64(&mut self, v: u64) {
        self.write_slice(&v.to_le_bytes());
    }

    fn write_pascal_string(&mut self, v: &str) {
        let str_len = v.len() as u8;
        self.write_byte(str_len);
        self.write_slice(v.as_bytes());
    }

    fn serialize_list_of_arrays(&mut self, v: &Vec<Vec<u8>>) {
        let array_len = v.len() as i32;
        self.write_i32(array_len);

        for arr in v {
            self.write_byte_array(arr.as_slice());
        }
    }

    fn write_byte_array(&mut self, v: &[u8]) {
        let array_len = v.len() as i32;
        self.write_i32(array_len);
        self.write_slice(v);
    }

    fn write_list_of_pascal_strings(&mut self, v: &Vec<String>) {
        let array_len = v.len() as i32;
        self.write_i32(array_len);

        for str in v {
            self.write_pascal_string(str);
        }
    }
}

impl TcpWriteBuffer for Vec<u8> {
    fn write_byte(&mut self, b: u8) {
        self.push(b);
    }

    fn write_slice(&mut self, slice: &[u8]) {
        self.extend_from_slice(slice);
    }
}
