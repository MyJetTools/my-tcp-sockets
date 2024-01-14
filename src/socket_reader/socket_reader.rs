use async_trait::async_trait;

use super::{ReadBuffer, ReadingTcpContractFail};

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
