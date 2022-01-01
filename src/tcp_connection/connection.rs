use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use tokio::{io::WriteHalf, net::TcpStream, sync::Mutex};

use tokio::io::AsyncWriteExt;

use crate::ConnectionId;

use super::{ConnectionEvent, ConnectionStatistics};

pub struct SocketConnection<TContract> {
    pub socket: Mutex<Option<WriteHalf<TcpStream>>>,
    pub addr: Option<SocketAddr>,
    pub id: ConnectionId,
    connected: AtomicBool,
    pub statistics: ConnectionStatistics,
    sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
}

impl<TContract> SocketConnection<TContract> {
    pub fn new(
        socket: WriteHalf<TcpStream>,
        id: i32,
        addr: Option<SocketAddr>,
        sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
    ) -> Self {
        Self {
            socket: Mutex::new(Some(socket)),
            id,
            addr,
            connected: AtomicBool::new(true),
            statistics: ConnectionStatistics::new(),
            sender,
        }
    }

    pub fn callback_event(&self, event: ConnectionEvent<TContract>) {
        if let Err(err) = self.sender.send(event) {
            println!(
                "Error by sending callback to the connection: {}. Err: {}",
                self.id, err
            );
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

        process_disconnect(&mut write_access, self.id).await;

        self.statistics.disconnect();
        return true;
    }

    pub async fn send_bytes(&self, payload: &[u8]) -> bool {
        let mut write_access = self.socket.lock().await;

        match &mut *write_access {
            Some(tcp_stream) => {
                if send_bytes(tcp_stream, self.id, payload).await {
                    self.statistics.update_sent_amount(payload.len());
                    true
                } else {
                    process_disconnect(&mut write_access, self.id).await;
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
) -> bool {
    match tcp_stream.write_all(payload).await {
        Ok(_) => true,
        Err(err) => {
            println!("Can not send payload to socket {}. Err: {}", id, err);
            false
        }
    }
}

async fn process_disconnect(tcp_stream: &mut Option<WriteHalf<TcpStream>>, id: ConnectionId) {
    let mut result = None;
    std::mem::swap(&mut result, tcp_stream);

    let mut result = result.unwrap();

    if let Err(err) = result.shutdown().await {
        println!("Error while disconnecting socket {}. Err: {}", id, err);
    }
}
