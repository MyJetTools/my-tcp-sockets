use std::{panic::AssertUnwindSafe, sync::Arc};
use futures::FutureExt;
use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};

use crate::{
    socket_reader::{ReadingTcpContractFail, SocketReaderTcpStream},
    tcp_connection::TcpSocketConnection,
    SocketEventCallback, TcpContract, TcpSerializerState, TcpSocketSerializer,
};

pub async fn start<TContract, TSerializer, TSerializationMetadata, TSocketCallback>(
    socket_reader: SocketReaderTcpStream,
    connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
     socket_callback: &mut TSocketCallback,
    read_serializer: TSerializer,
    read_serializer_metadata: TSerializationMetadata,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
)
 where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send +  'static,
{

    let future = read_loop(
            socket_reader,
            read_serializer,
            read_serializer_metadata,
            connection.clone(),
            socket_callback,
        );
    
    let result = AssertUnwindSafe(future).catch_unwind().await;

    match result{
        Ok(result) => {
            if let Err(err) = result{
                 logger.write_debug_info(
                "Socket Read Loop".to_string(),
                format!("Socket Read loop exited with error: {:?}", err),
                Some(connection.get_log_context().await),
            );
            }
        },
        Err(err) =>{
            logger.write_fatal_error(
                "Socket Read Loop".to_string(),
                format!("Socket Read loop exited with panic: {:?}", err),
                Some(connection.get_log_context().await),
            );

        },
    };
}

async fn read_loop<TContract, TSerializer, TSerializationMetadata, TSocketCallback>(
    mut socket_reader: SocketReaderTcpStream,
    mut read_serializer: TSerializer,
    mut meta_data: TSerializationMetadata,
    connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
     socket_callback: &mut TSocketCallback,
) -> Result<(), ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send + 'static,
{
    loop {  
        let contract = read_packet(
            &connection,
            &mut socket_reader,
            &mut read_serializer,
            &meta_data,
        )
        .await?;

        if meta_data.is_tcp_contract_related_to_metadata(&contract) {
            meta_data.apply_tcp_contract(&contract);
            connection.update_incoming_packet_to_state(&contract);
        }

        socket_callback.payload(&connection, contract).await;
    }
}

pub async fn read_packet<TContract, TSerializer, TSerializationMetadata>(
    connection: &TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>,
    socket_reader: &mut SocketReaderTcpStream,
    read_serializer: &mut TSerializer,
    meta_data: &TSerializationMetadata,
) -> Result<TContract, ReadingTcpContractFail>
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
{
    socket_reader.start_calculating_read_size();

    let read_future = read_serializer.deserialize(socket_reader, meta_data);

    let read_result = tokio::time::timeout(connection.dead_disconnect_timeout, read_future).await;

    if read_result.is_err() {
        connection.logger.write_debug_info(
            "read_loop".to_string(),
            format!("Read timeout {:?}", connection.dead_disconnect_timeout),
            Some(connection.get_log_context().await),
        );

        connection.disconnect().await;
        return Err(ReadingTcpContractFail::Timeout);
    }

    let contract = read_result.unwrap()?;

    if contract.is_pong() {
        connection.statistics().update_ping_pong_statistic();
    }

    connection
        .statistics()
        .update_read_amount(socket_reader.read_size);

    connection
        .statistics()
        .last_receive_moment
        .update(DateTimeAsMicroseconds::now());

    Ok(contract)
}

pub async fn execute_on_connected<TContract, TSerializer, TSerializationMetadata, TSocketCallback>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    socket_callback: &mut TSocketCallback,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
) -> bool
where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send +'static,
{


    let connected_fut = socket_callback.connected(connection.clone());

    let connected_result = AssertUnwindSafe(connected_fut).catch_unwind().await;
    
    if let Err(err) = connected_result {
        logger.write_fatal_error(
            "Socket Read Loop On Connected".to_string(),
            format!("{:?}", err),
            Some(connection.get_log_context().await),
        );
        return false;
    }

    true
}

pub async fn execute_on_disconnected<
    TContract,
    TSerializer,
    TSerializationMetadata,
    TSocketCallback,
>(
    connection: &Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    socket_callback: &mut TSocketCallback,
    logger: &Arc<dyn Logger + Send + Sync + 'static>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializationMetadata> + Send + 'static,
{

      let disconnect_fut = socket_callback.disconnected(connection.clone());

      let disconnect_result = AssertUnwindSafe(disconnect_fut).catch_unwind().await;
    
    if let Err(err) = disconnect_result {
        logger.write_fatal_error(
            "Socket Read Loop On Disconnect".to_string(),
            format!("{:?}", err),
            Some(connection.get_log_context().await),
        );
    }

}
