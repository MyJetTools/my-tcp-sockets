use std::{sync::Arc, time::Duration};

use rust_extensions::{date_time::DateTimeAsMicroseconds, Logger};

use crate::{
    tcp_connection::TcpSocketConnection, TcpContract, TcpSerializerMetadata, TcpSocketSerializer,
};

pub async fn start<
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Default + TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializerMetadata<TContract> + Send + Sync + 'static,
>(
    connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
    seconds_to_ping: usize,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
) {
    let ping_interval = Duration::from_secs(1);

    let mut seconds_remains_to_ping = seconds_to_ping;

    connection.threads_statistics.ping_threads.increase();

    loop {
        tokio::time::sleep(ping_interval).await;

        if connection.get_read_thread_status().is_finished()
            || connection.get_write_thread_status().is_finished()
        {
            connection.disconnect().await;
            break;
        }

        connection.statistics().one_second_tick();

        seconds_remains_to_ping -= 1;

        if seconds_remains_to_ping == 0 {
            seconds_remains_to_ping = seconds_to_ping;
            connection.statistics().set_ping_start();
            connection.send_ping().await;
        }

        let now = DateTimeAsMicroseconds::now();

        if connection.is_dead(now) {
            logger.write_debug_info(
                "TcpClientPingLoop".to_string(),
                format!("Detected dead socket {}. Disconnecting", connection.id),
                Some(connection.get_log_context().await),
            );

            connection.disconnect().await;
            break;
        }
    }

    connection.threads_statistics.ping_threads.decrease();
}
