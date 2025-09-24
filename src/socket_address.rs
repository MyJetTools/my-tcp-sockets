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

#[cfg(feature = "unix-socket")]
impl std::fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAddress::Tcp(addr) => write!(f, "Tcp: {}", addr),
            SocketAddress::UnixSocket(addr) => write!(f, "UnixSocket: {:?}", addr),
        }
    }
}

impl std::fmt::Debug for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
