use async_trait::async_trait;

use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};
use tokio_util::bytes::buf;

use super::{MyReadBuffer, ReadBuffer, ReadingTcpContractFail};

#[async_trait]
pub trait SocketReader {
    async fn read_byte(&mut self) -> Result<u8, ReadingTcpContractFail>;

    async fn read_i32(&mut self) -> Result<i32, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 4;
        let mut buf = [0u8; DATA_SIZE];
        self.read_buf(&mut buf).await?;
        Ok(i32::from_le_bytes(buf))
    }

    async fn read_u32(&mut self) -> Result<u32, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 4;
        let mut buf = [0u8; DATA_SIZE];

        self.read_buf(&mut buf).await?;
        Ok(u32::from_le_bytes(buf))
    }

    async fn read_bool(&mut self) -> Result<bool, ReadingTcpContractFail> {
        let b = self.read_byte().await?;

        return Ok(b > 0);
    }

    async fn read_byte_array(&mut self) -> Result<Vec<u8>, ReadingTcpContractFail> {
        let size = self.read_i32().await? as usize;

        let mut value: Vec<u8> = Vec::with_capacity(size);
        unsafe { value.set_len(size) }
        self.read_buf(&mut value).await?;

        return Ok(value);
    }

    async fn read_i64(&mut self) -> Result<i64, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 8;
        let mut buf = [0u8; DATA_SIZE];

        self.read_buf(&mut buf).await?;

        Ok(i64::from_le_bytes(buf))
    }

    async fn read_u64(&mut self) -> Result<u64, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 8;
        let mut buf = [0u8; DATA_SIZE];
        self.read_buf(&mut buf).await?;
        Ok(u64::from_le_bytes(buf))
    }

    async fn read_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadingTcpContractFail>;
    async fn read_until_end_marker<'s>(
        &mut self,
        read_buffer: &'s mut ReadBuffer,
        end_marker: &[u8],
    ) -> Result<&'s [u8], ReadingTcpContractFail>;
}

pub struct SocketReaderTcpStream {
    tcp_stream: OwnedReadHalf,
    my_read_buffer: MyReadBuffer,
    pub read_size: usize,
}

impl SocketReaderTcpStream {
    pub fn new(tcp_stream: OwnedReadHalf) -> Self {
        Self {
            tcp_stream,
            read_size: 0,
            my_read_buffer: MyReadBuffer::new(),
        }
    }

    pub fn start_calculating_read_size(&mut self) {
        self.read_size = 0;
    }

    async fn read_to_internal_buffer(&mut self) -> Result<(), ReadingTcpContractFail> {
        let bytes_to_advance = self
            .tcp_stream
            .read(self.my_read_buffer.get_buffer_to_write())
            .await?;

        if bytes_to_advance == 0 {
            return Err(ReadingTcpContractFail::SocketDisconnected);
        }

        self.my_read_buffer.advance_data_len(bytes_to_advance);

        Ok(())
    }
}

#[async_trait]
impl SocketReader for SocketReaderTcpStream {
    async fn read_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadingTcpContractFail> {
        if buf.len() == 0 {
            return Ok(());
        }

        let mut pos = 0;
        loop {
            let data_we_have = self.my_read_buffer.write_into_target(&mut buf[pos..]);

            pos += data_we_have;

            if pos == buf.len() {
                return Ok(());
            }

            self.read_to_internal_buffer().await?;
        }
    }

    async fn read_byte(&mut self) -> Result<u8, ReadingTcpContractFail> {
        loop {
            if let Some(result) = self.my_read_buffer.read_byte() {
                self.read_size += 1;
                return Ok(result);
            }

            self.read_to_internal_buffer().await?;
        }
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
            let read = {
                let buf = read_buffer.get_buffer_to_write();

                if buf.is_none() {
                    return Err(ReadingTcpContractFail::ErrorReadingSize);
                }

                self.tcp_stream.read(buf.unwrap()).await
            };

            match read {
                Ok(size) => {
                    if size == 0 {
                        return Err(ReadingTcpContractFail::SocketDisconnected);
                    }
                    self.read_size += size;

                    read_buffer.commit_written_size(size);

                    if read_buffer.find_sequence(end_marker) {
                        return Ok(read_buffer.get_package());
                    }
                }
                Err(err) => {
                    return Err(ReadingTcpContractFail::IoError(err));
                }
            }
        }
    }
}
