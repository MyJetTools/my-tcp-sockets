use std::{sync::Arc, time::Duration};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    TcpContract, TcpSocketSerializer,
};

pub async fn read_first_server_packet<TContract, TSerializer>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    socket_reader: &mut SocketReaderTcpStream,
    read_serializer: &mut TSerializer,
) -> Result<TContract, ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let first_packet_reading =
        crate::tcp_connection::read_loop::read_packet(&connection, socket_reader, read_serializer);

    let response = tokio::time::timeout(Duration::from_secs(3), first_packet_reading).await;

    if response.is_err() {
        connection.disconnect().await;
        return Err(ReadingTcpContractFail::Timeout);
    }

    let response = response.unwrap();

    if response.is_err() {
        connection.disconnect().await;
    }

    response
}
