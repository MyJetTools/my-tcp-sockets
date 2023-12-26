pub struct TcpPayloads {
    pub payloads: Vec<Vec<u8>>,
    max_send_payload_size: usize,
}

impl TcpPayloads {
    pub fn new(max_send_payload_size: usize) -> Self {
        Self {
            payloads: Vec::new(),
            max_send_payload_size,
        }
    }

    fn get_payload_to_append(&mut self) -> &mut Vec<u8> {
        if self.payloads.len() == 0 {
            self.payloads
                .push(Vec::with_capacity(self.max_send_payload_size));
        }

        if let Some(last_payload) = self.payloads.last() {
            if last_payload.len() >= self.max_send_payload_size {
                self.payloads
                    .push(Vec::with_capacity(self.max_send_payload_size));
            }
        }

        self.payloads.last_mut().unwrap()
    }

    pub fn add_payload(&mut self, payload: &[u8]) {
        let mut payload = payload;

        let max_payload_size = self.max_send_payload_size;
        loop {
            let payload_to_append = self.get_payload_to_append();

            let remains_payload_size = max_payload_size - payload_to_append.len();

            if remains_payload_size >= payload.len() {
                payload_to_append.extend_from_slice(payload);
                return;
            }

            payload_to_append.extend_from_slice(&payload[..remains_payload_size]);
            payload = &payload[remains_payload_size..];
        }
    }

    pub fn shrink_capacity(&mut self) {
        let capacity = self.payloads.capacity();

        if capacity < 512 {
            return;
        }

        if self.payloads.len() > capacity / 2 {
            return;
        }
    }

    pub fn get_payload(&mut self) -> Option<Vec<u8>> {
        if self.payloads.len() == 0 {
            return None;
        }

        let result = self.payloads.remove(0);

        Some(result)
    }

    pub fn get_size(&self) -> usize {
        let mut result = 0;

        for payload in &self.payloads {
            result += payload.len();
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_less_amount_than_buffer() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5]);

        assert_eq!(
            &[1, 2, 3, 4, 5],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_less_amount_exactly_as_buffer() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_ammount_bugger_than_buffer() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert_eq!(&[11, 12,], tcp_payloads.get_payload().unwrap().as_slice());

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_ammount_bugger_than_buffer_and_append_second_time_not_full_amount() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        tcp_payloads.add_payload(&[13, 14, 15]);
        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert_eq!(
            &[11, 12, 13, 14, 15],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_ammount_bugger_than_buffer_and_append_second_time_full_amount() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        tcp_payloads.add_payload(&[13, 14, 15, 16, 17, 18, 19, 20]);
        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert_eq!(
            &[11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert!(tcp_payloads.get_payload().is_none());
    }

    #[test]
    fn test_ammount_bugger_than_buffer_and_append_second_time_more_than_full_amount() {
        let mut tcp_payloads = TcpPayloads::new(10);

        tcp_payloads.add_payload(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

        tcp_payloads.add_payload(&[13, 14, 15, 16, 17, 18, 19, 20, 21]);
        assert_eq!(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert_eq!(
            &[11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
            tcp_payloads.get_payload().unwrap().as_slice()
        );

        assert_eq!(&[21], tcp_payloads.get_payload().unwrap().as_slice());

        assert!(tcp_payloads.get_payload().is_none());
    }
}
