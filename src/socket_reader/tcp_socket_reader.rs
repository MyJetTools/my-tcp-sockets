use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

use super::{MyReadBuffer, ReadBuffer, ReadingTcpContractFail, SocketReader};

pub enum TcpStreamMode {
    Owned(OwnedReadHalf),
    TcpStream(Option<tokio::net::TcpStream>),
}

impl TcpStreamMode {
    pub fn get_write_half(&mut self) -> Option<OwnedWriteHalf> {
        match self {
            TcpStreamMode::Owned(_) => {
                return None;
            }
            TcpStreamMode::TcpStream(tcp_stream) => {
                let tcp_stream = tcp_stream.take().unwrap();
                let (read_half, write_half) = tcp_stream.into_split();
                *self = TcpStreamMode::Owned(read_half);
                Some(write_half)
            }
        }
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        match self {
            TcpStreamMode::Owned(tcp_stream) => tcp_stream.read(buf).await,
            TcpStreamMode::TcpStream(tcp_stream) => tcp_stream.as_mut().unwrap().read(buf).await,
        }
    }
}

pub struct SocketReaderTcpStream {
    tcp_stream: TcpStreamMode,
    my_read_buffer: MyReadBuffer,
    pub read_size: usize,
}

impl SocketReaderTcpStream {
    pub fn new_as_tcp_stream(tcp_stream: TcpStream) -> Self {
        Self {
            tcp_stream: TcpStreamMode::TcpStream(Some(tcp_stream)),
            read_size: 0,
            my_read_buffer: MyReadBuffer::new(),
        }
    }

    pub fn new_as_owned_tcp_stream(tcp_stream: OwnedReadHalf) -> Self {
        Self {
            tcp_stream: TcpStreamMode::Owned(tcp_stream),
            read_size: 0,
            my_read_buffer: MyReadBuffer::new(),
        }
    }

    pub fn start_calculating_read_size(&mut self) {
        self.read_size = 0;
    }

    pub fn get_write_part(&mut self) -> OwnedWriteHalf {
        if let Some(write_half) = self.tcp_stream.get_write_half() {
            return write_half;
        }

        panic!("OwnedWriteHalf is already taken");
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

    pub async fn shutdown(&mut self) {
        match &mut self.tcp_stream {
            TcpStreamMode::Owned(_) => {}
            TcpStreamMode::TcpStream(tcp_stream) => {
                let _ = tcp_stream.as_mut().unwrap().shutdown().await;
            }
        }
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