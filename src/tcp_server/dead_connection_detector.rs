use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{tcp_connection::TcpSocketConnection, TcpSerializerState, TcpSocketSerializer};

pub async fn start_server_dead_connection_detector<
    TContract: Send + Sync + 'static,
    TSerializer: TcpSocketSerializer<TContract, TSerializationMetadata> + Send + Sync + 'static,
    TSerializationMetadata: TcpSerializerState<TContract> + Send + Sync + 'static,
>(
    connection: Arc<TcpSocketConnection<TContract, TSerializer, TSerializationMetadata>>,
) {
    connection.threads_statistics.ping_threads.increase();
    let sleep_duration = tokio::time::Duration::from_secs(5);

    loop {
        tokio::time::sleep(sleep_duration).await;

        if connection.get_read_thread_status().is_finished()
            || connection.get_write_thread_status().is_finished()
        {
            connection.disconnect().await;
            break;
        }

        let now = DateTimeAsMicroseconds::now();

        if connection.is_dead(now) {
            connection.logger.write_debug_info(
                "Server dead connection detector".to_string(),
                format!("Detected dead socket. Disconnecting"),
                Some(connection.get_log_context().await),
            );

            println!(
                "Connection: {}. Read thread: {:?}",
                connection.id,
                connection.get_read_thread_status()
            );
            println!(
                "Connection{}. Write thread:{:?}",
                connection.id,
                connection.get_write_thread_status()
            );

            connection.disconnect().await;
            break;
        }
    }

    connection.threads_statistics.ping_threads.decrease();
}
