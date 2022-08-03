use std::{sync::Arc, time::Duration};

use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};

use crate::{tcp_connection::SocketConnection, TcpSocketSerializer};

pub trait TcpContract {
    fn is_pong(&self) -> bool;
}

pub struct PingData {
    pub seconds_to_ping: usize,
    pub ping_packet: Vec<u8>,
}

pub async fn start<
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract> + Send + Sync + 'static,
>(
    connection: Arc<SocketConnection<TContract, TSerializer>>,
    ping_data: Option<PingData>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) {
    const PROCESS_NAME: &str = "ping_loop";
    let ping_interval = Duration::from_secs(1);

    let mut seconds_remains_to_ping = if let Some(ping_data) = &ping_data {
        Some(ping_data.seconds_to_ping)
    } else {
        None
    };

    while connection.is_connected() {
        tokio::time::sleep(ping_interval).await;

        connection.statistics.one_second_tick();

        if let Some(seconds_remains_to_ping) = &mut seconds_remains_to_ping {
            if let Some(ping_data) = &ping_data {
                *seconds_remains_to_ping -= 1;

                if *seconds_remains_to_ping == 0 {
                    *seconds_remains_to_ping = ping_data.seconds_to_ping;
                    connection.statistics.set_ping_start();
                    connection.send_bytes(ping_data.ping_packet.as_ref()).await;
                }
            }
        }

        let now = DateTimeAsMicroseconds::now();

        let last_recieved_moment = connection.statistics.last_receive_moment.as_date_time();

        let last_received = now
            .duration_since(last_recieved_moment)
            .as_positive_or_zero();

        if last_received > connection.dead_disconnect_timeout {
            logger.write_info(
                PROCESS_NAME.to_string(),
                format!("Detected dead socket {}. Disconnecting", connection.id),
                connection.get_log_context().await,
            );

            connection.disconnect().await;
            break;
        }
    }
}
