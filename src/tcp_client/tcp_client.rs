use std::sync::Arc;
use std::time::Duration;

use rust_extensions::{AppStates, Logger};
use tokio::sync::Mutex;

use crate::{SocketEventCallback, TcpSocketSerializer};
use crate::{
    TcpClientSocketSettings, TcpContract, TcpSerializerFactory, TcpSerializerState,
    ThreadsStatistics,
};

use super::TcpConnectionHolder;

const DEFAULT_MAX_SEND_PAYLOAD_SIZE: usize = 1024 * 1024 * 3;
const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct TcpClientInner {
    pub send_timeout: Duration,
    pub name: Arc<String>,
    pub max_send_payload_size: usize,
    pub re_connect_timeout: Duration,
    pub seconds_to_ping: usize,
    pub disconnect_timeout: Duration,
    pub connect_timeout: Duration,
    pub threads_statistics: Arc<ThreadsStatistics>,
    pub settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    tcp_connection_holder: Arc<TcpConnectionHolder>, //todo!("TcpConnectionHolder cases")
   
}

pub struct TcpClient {
    background_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    inner: TcpClientInner,
     app_states: Option<Arc<AppStates>>
}

impl TcpClient {
    pub fn new(
        name: String,
        settings: Arc<dyn TcpClientSocketSettings + Send + Sync + 'static>,
    ) -> Self {
        Self {
            inner: TcpClientInner {
                re_connect_timeout: Duration::from_secs(3),
                seconds_to_ping: 3,
                disconnect_timeout: Duration::from_secs(9),
                name: Arc::new(name),
                max_send_payload_size: DEFAULT_MAX_SEND_PAYLOAD_SIZE,
                send_timeout: DEFAULT_SEND_TIMEOUT,
                connect_timeout: Duration::from_secs(10),
                threads_statistics: Arc::new(ThreadsStatistics::default()),
                settings,
                tcp_connection_holder: Arc::new(TcpConnectionHolder::new()),
    
            },
            background_task: Mutex::new(None),
            app_states: None,
        }
    }
    

    pub fn set_app_states(mut self, app_states: Arc<AppStates>)->Self{
        self.app_states = Some(app_states);
        self
    }

    pub fn set_seconds_to_ping(mut self, seconds: usize) -> Self {
        self.inner.seconds_to_ping = seconds;
        self
    }

    pub fn set_disconnect_timeout(mut self, disconnect_timeout: Duration) -> Self {
        self.inner.disconnect_timeout = disconnect_timeout;
        self
    }

    pub fn set_reconnect_timeout(mut self, re_connect_timeout: Duration) -> Self {
        self.inner.re_connect_timeout = re_connect_timeout;
        self
    }

    pub async fn start<
        TContract,
        TSerializer,
        TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
        TTcpSerializerStateFactory,
        TSocketCallback,
    >(
        &self,
        serializer_metadata_factory: Arc<TTcpSerializerStateFactory>,
        socket_callback: TSocketCallback,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
    ) where
        TContract: TcpContract + Send + Sync + 'static,
        TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializationMetadata>,

        TTcpSerializerStateFactory: TcpSerializerFactory<TContract, TSerializer, TSerializationMetadata>
            + Send
            + Sync
            + 'static,
        TSocketCallback: SocketEventCallback<TContract, TSerializer, TSerializationMetadata>
            + Send
            + 'static,
    {

        let app_states = match self.app_states.clone(){
            Some(app_states) => app_states,
            None => Arc::new(AppStates::create_initialized()),
        };
        let handle = tokio::spawn(super::connection_loop(
            self.inner.clone(),
            logger,
            serializer_metadata_factory,
            app_states,
            socket_callback,
        ));

        let mut background_task = self.background_task.lock().await;
        background_task.replace(handle);
    }

    pub async fn try_disconnect_current_connection(&self) {
        if let Some(connection) = self.inner.tcp_connection_holder.get_connection().await {
            connection.disconnect().await;
        }
    }

    pub async fn stop(&self) {
        let mut background_task = self.background_task.lock().await;

        if let Some(background_task) = background_task.take() {
            background_task.abort();
        }
    }
}
