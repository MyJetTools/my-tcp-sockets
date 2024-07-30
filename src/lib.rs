pub mod socket_reader;
mod tcp_client;
pub mod tcp_connection;
mod tcp_server;
mod tcp_socket_serializer;
mod types;
pub use tcp_client::TcpClient;
pub use tcp_server::TcpServer;
mod socket_event_callback;
mod tcp_sockets_settings;

pub use socket_event_callback::*;
pub use tcp_socket_serializer::*;
pub use tcp_sockets_settings::TcpClientSocketSettings;
pub use types::ConnectionId;
mod tcp_contract;
pub use tcp_contract::*;
mod threads_statistics;
pub use threads_statistics::*;
mod tcp_write_buffer;
pub use tcp_write_buffer::*;
#[cfg(feature = "with-tls")]
mod maybe_tls;
#[cfg(feature = "with-tls")]
pub use maybe_tls::*;
#[cfg(not(feature = "with-tls"))]
mod no_tls;
#[cfg(not(feature = "with-tls"))]
pub use no_tls::*;
