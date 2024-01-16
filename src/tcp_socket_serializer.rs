use async_trait::async_trait;

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReader},
    TcpWriteBuffer,
};

#[async_trait]
pub trait TcpSocketSerializer<
    TContract: Send + Sync + 'static,
    TSerializationMetadata: Send + Sync + 'static,
>
{
    fn serialize(
        &self,
        out: &mut impl TcpWriteBuffer,
        contract: &TContract,
        metadata: &TSerializationMetadata,
    );

    fn get_ping(&self) -> TContract;

    async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &mut self,
        socket_reader: &mut TSocketReader,
        metadata: &TSerializationMetadata,
    ) -> Result<TContract, ReadingTcpContractFail>;
}
