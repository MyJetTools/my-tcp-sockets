use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;

use tokio::io;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    tcp_connection::{ping_loop::PingData, ConnectionCallback, ConnectionEvent, SocketConnection},
    ConnectionId, TcpSocketSerializer,
};

pub fn start<TContract, TSerializer>(
    host_port: String,
    connect_timeout: Duration,
    serializer: TSerializer,
) -> ConnectionCallback<TContract>
where
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(connection_loop(
        host_port,
        connect_timeout,
        Arc::new(sender),
        serializer,
    ));

    ConnectionCallback::new(receiver)
}

async fn connection_loop<TContract, TSerializer>(
    host_port: String,
    connect_timeout: Duration,
    sender: Arc<UnboundedSender<ConnectionEvent<TContract>>>,
    serializer: TSerializer,
) where
    TContract: Send + Sync + 'static,
    TSerializer: Clone + Send + Sync + 'static + TcpSocketSerializer<TContract>,
{
    let mut connection_id: ConnectionId = 0;
    loop {
        tokio::time::sleep(connect_timeout).await;

        println!("Connecting to {}", host_port);
        let connect_result = TcpStream::connect(host_port.as_str()).await;

        match connect_result {
            Ok(tcp_stream) => {
                println!("Connected to {}", host_port);

                let (read_socket, write_socket) = io::split(tcp_stream);

                let connection = Arc::new(SocketConnection::new(
                    write_socket,
                    connection_id,
                    None,
                    sender.clone(),
                ));

                let ping_data = PingData {
                    seconds_to_ping: 3,
                    ping_packet: serializer.get_ping_payload(),
                };

                crate::tcp_connection::new_connection::start(
                    read_socket,
                    connection,
                    serializer.clone(),
                    Some(ping_data),
                    Duration::from_secs(9),
                )
                .await;
                println!("Disconnected from {}", host_port);
                connection_id += 1;
            }
            Err(err) => {
                println!(
                    "Can not connect to the socket: {}. Reason: {}",
                    host_port, err
                );
            }
        }
    }
}
