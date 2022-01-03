#[cfg(test)]
use super::{ReadBuffer, ReadingTcpContractFail, SocketReader};
#[cfg(test)]
use async_trait::async_trait;

#[cfg(test)]
pub struct SocketReaderMock {
    data: Vec<u8>,
}
#[cfg(test)]
impl SocketReaderMock {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.data.extend(data);
    }
}

#[cfg(test)]
#[async_trait]
impl SocketReader for SocketReaderMock {
    async fn read_byte(&mut self) -> Result<u8, ReadingTcpContractFail> {
        let result = self.data.remove(0);
        Ok(result)
    }

    async fn read_i32(&mut self) -> Result<i32, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 4;

        let mut buf = [0u8; DATA_SIZE];

        buf.copy_from_slice(&self.data[0..DATA_SIZE]);

        let result = i32::from_le_bytes(buf);

        for _ in 0..DATA_SIZE {
            self.data.remove(0);
        }

        Ok(result)
    }

    async fn read_bool(&mut self) -> Result<bool, ReadingTcpContractFail> {
        let result = self.read_byte().await?;
        Ok(result > 0u8)
    }

    async fn read_byte_array(&mut self) -> Result<Vec<u8>, ReadingTcpContractFail> {
        let len = self.read_i32().await? as usize;

        let mut result: Vec<u8> = Vec::new();

        for b in self.data.drain(0..len) {
            result.push(b);
        }

        Ok(result)
    }

    async fn read_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadingTcpContractFail> {
        buf.copy_from_slice(self.data.drain(0..buf.len()).as_slice());
        Ok(())
    }

    async fn read_i64(&mut self) -> Result<i64, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 8;

        let mut buf = [0u8; DATA_SIZE];

        buf.copy_from_slice(&self.data[0..DATA_SIZE]);

        let result = i64::from_le_bytes(buf);

        for _ in 0..DATA_SIZE {
            self.data.remove(0);
        }

        Ok(result)
    }

    async fn read_until_end_marker<'s>(
        &mut self,
        read_buffer: &'s mut ReadBuffer,
        end_marker: &[u8],
    ) -> Result<&'s [u8], ReadingTcpContractFail> {
        if read_buffer.find_sequence(end_marker) {
            return Ok(read_buffer.get_package());
        }

        loop {
            let read_size = {
                let buf = read_buffer.get_buffer_to_write();

                if buf.is_none() {
                    return Err(ReadingTcpContractFail::ErrorReadingSize);
                }

                let mut read = 0;
                let buf = buf.unwrap();
                for index in 0..buf.len() {
                    read = index;
                    if self.data.len() == 0 {
                        break;
                    }
                    buf[index] = self.data.remove(0);
                }

                read
            };

            if read_size == 0 {
                return Err(ReadingTcpContractFail::SocketDisconnected);
            }

            read_buffer.commit_written_size(read_size);

            if read_buffer.find_sequence(end_marker) {
                return Ok(read_buffer.get_package());
            }
        }
    }
}
