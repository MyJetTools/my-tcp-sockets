use std::{sync::Arc, time::Duration};

use my_logger::MyLogger;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_connection::SocketConnection;

pub struct PingData {
    pub seconds_to_ping: usize,
    pub ping_packet: Vec<u8>,
}

pub async fn start<TContract>(
    connection: Arc<SocketConnection<TContract>>,
    ping_data: Option<PingData>,
    disconnect_interval: Duration,
    logger: Arc<MyLogger>,
    log_context: String,
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
                    if !connection.send_bytes(ping_data.ping_packet.as_ref()).await {
                        logger.write_log(
                            my_logger::LogLevel::Info,
                            PROCESS_NAME.to_string(),
                            format!(
                                "Could not send ping command to the socket {}. Disconnecting",
                                connection.id
                            ),
                            Some(log_context),
                        );

                        break;
                    }
                }
            }
        }

        let now = DateTimeAsMicroseconds::now();

        let last_received =
            now.duration_since(connection.statistics.last_receive_moment.as_date_time());

        if last_received > disconnect_interval {
            logger.write_log(
                my_logger::LogLevel::Info,
                PROCESS_NAME.to_string(),
                format!("Detected dead socket {}. Disconnecting", connection.id),
                Some(log_context),
            );

            connection.disconnect().await;
            break;
        }
    }
}
