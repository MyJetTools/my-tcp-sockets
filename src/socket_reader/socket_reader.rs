use async_trait::async_trait;

use tokio::{
    io::{AsyncReadExt, ReadHalf},
    net::TcpStream,
};

use super::ReadingTcpContractFail;

#[async_trait]
pub trait SocketReader {
    async fn read_byte(&mut self) -> Result<u8, ReadingTcpContractFail>;
    async fn read_i32(&mut self) -> Result<i32, ReadingTcpContractFail>;
    async fn read_bool(&mut self) -> Result<bool, ReadingTcpContractFail>;
    async fn read_byte_array(&mut self) -> Result<Vec<u8>, ReadingTcpContractFail>;
    async fn read_i64(&mut self) -> Result<i64, ReadingTcpContractFail>;
    async fn read_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadingTcpContractFail>;
}

pub struct SocketReaderTcpStream {
    tcp_stream: ReadHalf<TcpStream>,
    pub read_size: usize,
}

impl SocketReaderTcpStream {
    pub fn new(tcp_stream: ReadHalf<TcpStream>) -> Self {
        Self {
            tcp_stream,
            read_size: 0,
        }
    }

    pub fn start_calculating_read_size(&mut self) {
        self.read_size = 0;
    }
}

#[async_trait]
impl SocketReader for SocketReaderTcpStream {
    async fn read_buf(&mut self, buf: &mut [u8]) -> Result<(), ReadingTcpContractFail> {
        let read = self.tcp_stream.read_exact(buf).await?;

        if read == 0 {
            return Err(ReadingTcpContractFail::SocketDisconnected);
        }

        if read != buf.len() {
            return Err(ReadingTcpContractFail::ErrorReadingSize);
        }

        self.read_size += read;

        Ok(())
    }

    async fn read_byte(&mut self) -> Result<u8, ReadingTcpContractFail> {
        let mut buf = [0u8];
        let read = self.tcp_stream.read(&mut buf).await?;

        if read == 0 {
            return Err(ReadingTcpContractFail::SocketDisconnected);
        }

        self.read_size += 1;

        return Ok(buf[0]);
    }

    async fn read_i32(&mut self) -> Result<i32, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 4;
        let mut buf = [0u8; DATA_SIZE];

        self.read_buf(&mut buf).await?;

        self.read_size += DATA_SIZE;

        Ok(i32::from_le_bytes(buf))
    }

    async fn read_i64(&mut self) -> Result<i64, ReadingTcpContractFail> {
        const DATA_SIZE: usize = 8;
        let mut buf = [0u8; DATA_SIZE];

        self.read_buf(&mut buf).await?;

        self.read_size += DATA_SIZE;

        Ok(i64::from_le_bytes(buf))
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

        self.read_size += size + 4;

        return Ok(value);
    }
}
