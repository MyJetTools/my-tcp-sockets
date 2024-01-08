use async_trait::async_trait;

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReader},
    TcpWriteBuffer,
};

#[async_trait]
pub trait TcpSocketSerializer<TContract: Send + Sync + 'static> {
    const PING_PACKET_IS_SINGLETON: bool;

    fn serialize(&self, out: &mut impl TcpWriteBuffer, contract: &TContract);

    fn get_ping(&self) -> TContract;

    fn get_ping_as_payload(&self) -> Vec<u8> {
        let ping_contract = self.get_ping();
        let mut ping_buffer: Vec<u8> = Vec::new();
        self.serialize(&mut ping_buffer, &ping_contract);
        ping_buffer
    }

    async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &mut self,
        socket_reader: &mut TSocketReader,
    ) -> Result<TContract, ReadingTcpContractFail>;
}
