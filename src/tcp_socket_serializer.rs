use async_trait::async_trait;

use crate::socket_reader::{ReadingTcpContractFail, SocketReader};

pub enum TcpPayload<'s> {
    AsVec(Vec<u8>),
    AsSlice(&'s [u8]),
}

impl<'s> TcpPayload<'s> {
    pub fn as_slice(&'s self) -> &'s [u8] {
        match self {
            TcpPayload::AsVec(v) => v.as_slice(),
            TcpPayload::AsSlice(s) => s,
        }
    }

    pub fn into_vec(self) -> Vec<u8> {
        match self {
            TcpPayload::AsVec(v) => v,
            TcpPayload::AsSlice(s) => s.to_vec(),
        }
    }
}

impl<'s> Into<TcpPayload<'s>> for &'s Vec<u8> {
    fn into(self) -> TcpPayload<'s> {
        TcpPayload::AsSlice(self)
    }
}

impl<'s> Into<TcpPayload<'s>> for &'s [u8] {
    fn into(self) -> TcpPayload<'s> {
        TcpPayload::AsSlice(self)
    }
}
impl<'s> Into<TcpPayload<'s>> for Vec<u8> {
    fn into(self) -> TcpPayload<'s> {
        TcpPayload::AsVec(self)
    }
}

#[async_trait]
pub trait TcpSocketSerializer<TContract: Send + Sync + 'static> {
    const PING_PACKET_IS_SINGLETON: bool;

    fn serialize<'s>(&self, contract: &'s TContract) -> TcpPayload<'s>;

    fn get_ping(&self) -> TContract;

    async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &mut self,
        socket_reader: &mut TSocketReader,
    ) -> Result<TContract, ReadingTcpContractFail>;
}
