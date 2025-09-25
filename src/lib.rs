pub mod socket_reader;
pub mod tcp_connection;
mod tcp_server;
mod tcp_socket_serializer;
mod types;
pub use tcp_server::TcpServer;
mod socket_event_callback;
mod tcp_sockets_settings;

pub use socket_event_callback::*;
pub use tcp_socket_serializer::*;
pub use tcp_sockets_settings::*;
pub use types::ConnectionId;
mod tcp_contract;
pub use tcp_contract::*;
mod threads_statistics;
pub use threads_statistics::*;
mod tcp_write_buffer;
pub use tcp_write_buffer::*;
mod tcp_client;
pub use tcp_client::*;

#[cfg(all(not(feature = "unix-socket"), not(feature = "with-tls")))]
mod no_tls_no_unix_sockets;
#[cfg(all(not(feature = "unix-socket"), not(feature = "with-tls")))]
pub use no_tls_no_unix_sockets::*;

#[cfg(feature = "unix-socket")]
pub mod unix_socket_server;

#[cfg(any(feature = "with-tls", feature = "unix-socket"))]
mod with_tls_or_unix_socket;
#[cfg(any(feature = "with-tls", feature = "unix-socket"))]
pub use with_tls_or_unix_socket::*;

mod socket_address;
pub use socket_address::*;
