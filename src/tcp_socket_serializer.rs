use async_trait::async_trait;

use crate::socket_reader::{ReadingTcpContractFail, SocketReader};

#[async_trait]
pub trait TcpSocketSerializer<TContract> {
    fn serialize(&self, contract: TContract) -> Vec<u8>;
    fn get_ping_payload(&self) -> Vec<u8>;
    async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &mut self,
        socket_reader: &mut TSocketReader,
    ) -> Result<TContract, ReadingTcpContractFail>;

    fn apply_packet(&mut self, contract: &TContract);
}
