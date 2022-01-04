use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use my_logger::{LogLevel, MyLogger};
use tokio::sync::mpsc::UnboundedSender;
use tokio::{io::WriteHalf, net::TcpStream, sync::Mutex};

use tokio::io::AsyncWriteExt;

use crate::socket_reader::{ReadingTcpContractFail, SocketReader};
use crate::{ConnectionId, TcpSocketSerializer};

use super::{ConnectionEvent, ConnectionName, ConnectionStatistics};

pub struct SocketData<TSerializer> {
    tcp_stream: WriteHalf<TcpStream>,
    serializer: TSerializer,
}

pub struct SocketConnection<TContract, TSerializer: TcpSocketSerializer<TContract>> {
    pub socket: Mutex<Option<SocketData<TSerializer>>>,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    sender: Arc<UnboundedSender<ConnectionEvent<TContract, TSerializer>>>,
    logger: Arc<MyLogger>,
    log_context: String,
    pub connection_name: Arc<ConnectionName>,
}

impl<TContract, TSerializer: TcpSocketSerializer<TContract>>
    SocketConnection<TContract, TSerializer>
{
    pub fn new(
        socket: WriteHalf<TcpStream>,
        serializer: TSerializer,
        id: ConnectionId,
        addr: Option<SocketAddr>,
        sender: Arc<UnboundedSender<ConnectionEvent<TContract, TSerializer>>>,
        logger: Arc<MyLogger>,
        log_context: String,
    ) -> Self {
        let socket_data = SocketData {
            tcp_stream: socket,
            serializer,
        };

        Self {
            socket: Mutex::new(Some(socket_data)),
            id,
            addr,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            sender,
            logger,
            log_context,
            connection_name: Arc::new(ConnectionName::new(format!("{:?}", addr))),
        }
    }

    pub fn callback_event(&self, event: ConnectionEvent<TContract, TSerializer>) {
        if let Err(err) = self.sender.send(event) {
            let connection_name = self.connection_name.clone();
            let logger = self.logger.clone();
            let connection_id = self.id;
            let log_context = self.log_context.clone();
            let message = format!(
                "Error by sending callback to the connection: {}. Err: {}",
                connection_id, err
            );

            tokio::spawn(async move {
                let connection_name = connection_name.get().await;
                logger.write_log(
                    LogLevel::FatalError,
                    "Tcp Accept Socket".to_string(),
                    message,
                    Some(format!(
                        "{}; ConnectionName:{}",
                        log_context, connection_name
                    )),
                )
            });
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn disconnect(&self) -> bool {
        let mut write_access = self.socket.lock().await;

        if write_access.is_none() {
            return false;
        }

        process_disconnect(
            &mut write_access,
            self.id,
            self.logger.as_ref(),
            self.log_context.as_str(),
        )
        .await;

        self.statistics.disconnect();
        return true;
    }

    pub async fn send(&self, payload: TContract) -> bool {
        let mut write_access = self.socket.lock().await;

        match &mut *write_access {
            Some(socket_data) => {
                let payload = socket_data.serializer.serialize(payload);
                self.send_package(&mut write_access, payload.as_slice())
                    .await
            }
            None => false,
        }
    }

    pub async fn deserialize<TSocketReader: Send + Sync + 'static + SocketReader>(
        &self,
        socket_reader: &mut TSocketReader,
    ) -> Result<TContract, ReadingTcpContractFail> {
        let mut write_access = self.socket.lock().await;

        match &mut *write_access {
            Some(socket_data) => {
                let packet = socket_data.serializer.deserialize(socket_reader).await?;
                socket_data.serializer.apply_packet(&packet);
                Ok(packet)
            }
            None => Err(ReadingTcpContractFail::SocketDisconnected),
        }
    }

    pub async fn send_bytes(&self, payload: &[u8]) -> bool {
        let mut write_access = self.socket.lock().await;
        self.send_package(&mut write_access, payload).await
    }

    async fn send_package(
        &self,
        write_access: &mut Option<SocketData<TSerializer>>,
        payload: &[u8],
    ) -> bool {
        match &mut *write_access {
            Some(socket_data) => {
                if send_bytes(
                    &mut socket_data.tcp_stream,
                    self.id,
                    payload,
                    self.logger.as_ref(),
                    self.log_context.as_str(),
                )
                .await
                {
                    self.statistics.update_sent_amount(payload.len());
                    true
                } else {
                    process_disconnect(
                        write_access,
                        self.id,
                        self.logger.as_ref(),
                        self.log_context.as_str(),
                    )
                    .await;
                    false
                }
            }
            None => false,
        }
    }
}

async fn send_bytes(
    tcp_stream: &mut WriteHalf<TcpStream>,
    id: ConnectionId,
    payload: &[u8],
    logger: &MyLogger,
    log_context: &str,
) -> bool {
    match tcp_stream.write_all(payload).await {
        Ok(_) => true,
        Err(err) => {
            logger.write_log(
                LogLevel::Info,
                "TcpConnection::send_bytes".to_string(),
                format!("Can not send payload to socket {}. Err: {}", id, err),
                Some(log_context.to_string()),
            );
            false
        }
    }
}

async fn process_disconnect<TSerializer>(
    tcp_stream: &mut Option<SocketData<TSerializer>>,
    id: ConnectionId,
    logger: &MyLogger,
    log_context: &str,
) {
    let mut result = None;
    std::mem::swap(&mut result, tcp_stream);

    let mut result = result.unwrap();

    if let Err(err) = result.tcp_stream.shutdown().await {
        logger.write_log(
            LogLevel::Info,
            "process_disconnect".to_string(),
            format!("Error while disconnecting socket {}. Err: {}", id, err),
            Some(log_context.to_string()),
        );
    }
}
