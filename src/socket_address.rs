#[cfg(not(feature = "unix-socket"))]
pub type SocketAddress = std::net::SocketAddr;

#[cfg(feature = "unix-socket")]
pub enum SocketAddress {
    Tcp(std::net::SocketAddr),
    UnixSocket(tokio::net::unix::SocketAddr),
}
#[cfg(feature = "unix-socket")]
impl Into<SocketAddress> for std::net::SocketAddr {
    fn into(self) -> SocketAddress {
        SocketAddress::Tcp(self)
    }
}

#[cfg(feature = "unix-socket")]
impl Into<SocketAddress> for tokio::net::unix::SocketAddr {
    fn into(self) -> SocketAddress {
        SocketAddress::UnixSocket(self)
    }
}
