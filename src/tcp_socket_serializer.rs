use async_trait::async_trait;

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReader},
    TcpWriteBuffer,
};

#[async_trait]
pub trait TcpSocketSerializer<
    TContract: Send + Sync + 'static,
    TSerializerState: Send + Sync + 'static,
>
{
    fn serialize(
        &self,
        out: &mut impl TcpWriteBuffer,
        contract: &TContract,
        state: &TSerializerState,
    );

    fn get_ping(&self) -> TContract;

    async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &mut self,
        socket_reader: &mut TSocketReader,
        state: &TSerializerState,
    ) -> Result<TContract, ReadingTcpContractFail>;
}

pub trait TcpSerializerState<TContract> {
    //We check if we have to go through MutexGuard or RwLockReadGuard before apply_tcp_contract
    fn is_tcp_contract_related_to_metadata(&self, contract: &TContract) -> bool;
    fn apply_tcp_contract(&mut self, contract: &TContract);
}

#[async_trait::async_trait]
pub trait TcpSerializerFactory<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
>
{
    async fn create_serializer(&self) -> TSerializer;
    async fn create_serializer_state(&self) -> TSerializerState;
}
