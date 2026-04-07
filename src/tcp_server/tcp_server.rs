use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::{ApplicationStates, Logger};

use crate::{
    SocketEventCallback, TcpContract, TcpSerializerFactory, TcpSerializerState,
    TcpSocketSerializer, ThreadsStatistics,
};

//use super::ConnectionsList;

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);



pub struct TcpServer {
    addr: SocketAddr,
    name: Arc<String>,
    max_send_payload_size: usize,
    send_timeout: Duration,
    pub threads_statistics: Arc<ThreadsStatistics>,
}

impl TcpServer {
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Self {
            name: Arc::new(name),
            addr,
            max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            threads_statistics: Arc::new(ThreadsStatistics::default()),
        }
    }

    pub async fn start<
        TContract,
        TSerializer,
        TSerializerState,
        TTcpSerializerStateFactory,
        TSocketCallback,
    >(
        &self,
        serializer_metadata_factory: Arc<TTcpSerializerStateFactory>,
        socket_callback: Arc<TSocketCallback>,
        app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TSerializer: TcpSocketSerializer<TContract, TSerializerState> + Send + Sync + 'static,
        TContract: TcpContract + Send + Sync + 'static,
        TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
        TTcpSerializerStateFactory:
            TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
        TSocketCallback:
            SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    {
        let threads_statistics = self.threads_statistics.clone();
        tokio::spawn(super::accept_tcp_connections_loop(
            self.addr,
            socket_callback.clone(),
            self.name.clone(),
            self.max_send_payload_size,
            self.send_timeout,
            app_states,
            logger,
            threads_statistics,
            serializer_metadata_factory,
        ));
    }
}
