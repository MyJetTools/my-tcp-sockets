use std::{sync::Arc, time::Duration};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    ConnectionEvent, SocketEventCallback, TcpContract, TcpSerializationMetadata,
    TcpSocketSerializer,
};

pub async fn read_first_server_packet<TContract, TSerializer, TSerializationMetadata>(
    connection: &TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>,
    socket_reader: &mut SocketReaderTcpStream,
    read_serializer: &mut TSerializer,
    meta_data: &mut TSerializationMetadata,
) -> Result<TContract, ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: Default + TcpSerializationMetadata<TContract> + Send + Sync + 'static,
{
    let first_packet_reading = crate::tcp_connection::read_loop::read_packet(
        &connection,
        socket_reader,
        read_serializer,
        meta_data,
    );

    let response = tokio::time::timeout(Duration::from_secs(3), first_packet_reading).await;

    if response.is_err() {
        return Err(ReadingTcpContractFail::Timeout);
    }

    let contract = response.unwrap()?;

    if meta_data.is_tcp_contract_related_to_metadata(&contract) {
        meta_data.apply_tcp_contract(&contract);
        connection
            .apply_incoming_packet_to_metadata(&contract)
            .await;
    }

    Ok(contract)
}

pub async fn callback_payload<TContract, TSerializer, TSocketCallback, TSerializationMetadata>(
    socket_callback: &Arc<TSocketCallback>,
    connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    contract: TContract,
) -> bool
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer:
        Default + Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSocketCallback:
        Send + Sync + 'static + SocketEventCallback<TContract, TSerializer, TSerializationMetadata>,
    TSerializationMetadata: Default + TcpSerializationMetadata<TContract> + Send + Sync + 'static,
{
    let connection = connection.clone();
    let socket_callback = socket_callback.clone();
    let result = tokio::spawn(async move {
        socket_callback
            .handle(ConnectionEvent::Payload {
                connection,
                payload: contract,
            })
            .await
    })
    .await;

    if result.is_err() {
        return false;
    }

    true
}
