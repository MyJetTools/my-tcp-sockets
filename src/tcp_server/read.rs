use std::{sync::Arc, time::Duration};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    ConnectionEvent, SocketEventCallback, TcpContract, TcpSocketSerializer,
};

pub async fn read_first_server_packet<TContract, TSerializer, TSocketCallback>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer>>,
    socket_reader: &mut SocketReaderTcpStream,
    read_serializer: &mut TSerializer,
    socket_callback: &Arc<TSocketCallback>,
) -> Result<(), ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract>,
    TSocketCallback: Send + Sync + 'static + SocketEventCallback<TContract, TSerializer>,
{
    let first_packet_reading =
        crate::tcp_connection::read_loop::read_packet(&connection, socket_reader, read_serializer);

    let response = tokio::time::timeout(Duration::from_secs(3), first_packet_reading).await;

    if response.is_err() {
        return Err(ReadingTcpContractFail::Timeout);
    }

    let contract = response.unwrap()?;

    let socket_callback = socket_callback.clone();
    let connection = connection.clone();

    let result = tokio::spawn(async move {
        socket_callback
            .handle(ConnectionEvent::Payload {
                connection: connection.clone(),
                payload: contract,
            })
            .await
    })
    .await;

    if result.is_err() {
        return Err(ReadingTcpContractFail::PacketHandlerError);
    }

    Ok(())
}
