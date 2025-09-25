mod tcp_client;
pub use tcp_client::*;
mod tcp_connection_holder;
pub use tcp_connection_holder::*;
mod connection_loop;
use connection_loop::*;

mod connect_to_tcp_socket;
pub use connect_to_tcp_socket::*;

pub const LOG_PROCESS: &str = "Tcp Client Connect";
mod dead_connection_detector;
#[cfg(feature = "with-tls")]
mod tls;
#[cfg(feature = "with-tls")]
pub use tls::*;

#[cfg(feature = "unix-socket")]
mod connect_to_unix_socket;
#[cfg(feature = "unix-socket")]
use connect_to_unix_socket::*;
mod handle_new_connection;
use handle_new_connection::*;
