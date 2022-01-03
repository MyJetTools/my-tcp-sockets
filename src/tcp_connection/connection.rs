use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use my_logger::{LogLevel, MyLogger};
use tokio::sync::mpsc::UnboundedSender;
use tokio::{io::WriteHalf, net::TcpStream, sync::Mutex};

use tokio::io::AsyncWriteExt;

use crate::ConnectionId;

use super::{ConnectionEvent, ConnectionName, ConnectionStatistics};

pub struct SocketConnection<TContract> {
    pub socket: Mutex<Option<WriteHalf<TcpStream>>>,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
    logger: Arc<MyLogger>,
    log_context: String,
    pub connection_name: Arc<ConnectionName>,
}

impl<TContract> SocketConnection<TContract> {
    pub fn new(
        socket: WriteHalf<TcpStream>,
        id: i32,
        addr: Option<SocketAddr>,
        sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
        logger: Arc<MyLogger>,
        log_context: String,
    ) -> Self {
        Self {
            socket: Mutex::new(Some(socket)),
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

    pub fn callback_event(&self, event: ConnectionEvent<TContract>) {
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

    pub async fn send_bytes(&self, payload: &[u8]) -> bool {
        let mut write_access = self.socket.lock().await;

        match &mut *write_access {
            Some(tcp_stream) => {
                if send_bytes(
                    tcp_stream,
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
                        &mut write_access,
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

async fn process_disconnect(
    tcp_stream: &mut Option<WriteHalf<TcpStream>>,
    id: ConnectionId,
    logger: &MyLogger,
    log_context: &str,
) {
    let mut result = None;
    std::mem::swap(&mut result, tcp_stream);

    let mut result = result.unwrap();

    if let Err(err) = result.shutdown().await {
        logger.write_log(
            LogLevel::Info,
            "process_disconnect".to_string(),
            format!("Error while disconnecting socket {}. Err: {}", id, err),
            Some(log_context.to_string()),
        );
    }
}
