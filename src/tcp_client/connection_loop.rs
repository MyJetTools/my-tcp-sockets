use std::collections::HashMap;

use std::sync::Arc;

use rust_extensions::Logger;

use crate::{ConnectionId, SocketEventCallback, TcpSocketSerializer};
use crate::{TcpContract, TcpSerializerFactory, TcpSerializerState};

use super::*;

pub async fn connection_loop<
    TContract,
    TSerializer,
    TSerializerState,
    TSerializerMetadataFactory,
    TSocketCallback,
>(
    inner: TcpClientInner,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    serializer_factory: Arc<TSerializerMetadataFactory>,
    socket_callback: Arc<TSocketCallback>,
) where
    TContract: TcpContract + Send + Sync + 'static,
    TSerializer: Send + Sync + 'static + TcpSocketSerializer<TContract, TSerializerState>,
    TSerializerState: TcpSerializerState<TContract> + Send + Sync + 'static,
    TSerializerMetadataFactory:
        TcpSerializerFactory<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
    TSocketCallback:
        SocketEventCallback<TContract, TSerializer, TSerializerState> + Send + Sync + 'static,
{
    let mut connection_id: ConnectionId = 0;

    let mut socket_context: HashMap<String, String> = HashMap::new();
    socket_context.insert("SocketName".to_string(), inner.name.to_string());

    loop {
        connection_id += 1;
        tokio::time::sleep(inner.re_connect_timeout).await;

        let host_port = inner.settings.get_host_port().await;

        let Some(host_port) = host_port else {
            continue;
        };

        let mut socket_context = socket_context.clone();
        socket_context.insert("HostPort".to_string(), host_port.to_string());

        logger.write_debug_info(
            LOG_PROCESS.to_string(),
            format!("Trying to connect to {}", host_port),
            Some(socket_context.clone()),
        );

        if host_port.starts_with("/") || host_port.starts_with("~") {
            #[cfg(feature = "unix-socket")]
            super::connect_to_unix_socket(
                host_port.as_str(),
                connection_id,
                &inner,
                &socket_callback,
                &serializer_factory,
                &logger,
                socket_context,
            )
            .await;

            #[cfg(not(feature = "unix-socket"))]
            logger.write_fatal_error(
                LOG_PROCESS.to_string(),
                format!(
                    "Can not connect to unix socket: {}. Feature unix-sockets is disabled",
                    host_port
                ),
                Some(socket_context.clone()),
            );
        } else {
            super::connect_to_tcp_socket(
                &host_port,
                connection_id,
                &inner,
                &socket_callback,
                &serializer_factory,
                &logger,
                socket_context,
            )
            .await;
        }
    }
}
