pub enum MaybeTlsReadStream {
    NoTls(tokio::net::tcp::OwnedReadHalf),
    #[cfg(feature = "with-tls")]
    Tls(tokio::io::ReadHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    #[cfg(unix)]
    UnixSocket(tokio::net::unix::OwnedReadHalf),
}

impl MaybeTlsReadStream {
    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use tokio::io::AsyncReadExt;
        match self {
            Self::NoTls(ref mut inner) => inner.read(buf).await,
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => inner.read(buf).await,
            #[cfg(unix)]
            Self::UnixSocket(ref mut inner) => inner.read(buf).await,
        }
    }
}

impl Into<MaybeTlsReadStream> for tokio::net::tcp::OwnedReadHalf {
    fn into(self) -> MaybeTlsReadStream {
        MaybeTlsReadStream::NoTls(self)
    }
}

#[cfg(unix)]
impl Into<MaybeTlsReadStream> for tokio::net::unix::OwnedReadHalf {
    fn into(self) -> MaybeTlsReadStream {
        MaybeTlsReadStream::UnixSocket(self)
    }
}

pub enum MaybeTlsWriteStream {
    NoTls(tokio::net::tcp::OwnedWriteHalf),
    #[cfg(feature = "with-tls")]
    Tls(tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    #[cfg(unix)]
    UnixSocket(tokio::net::unix::OwnedWriteHalf),
}

impl MaybeTlsWriteStream {
    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        match self {
            Self::NoTls(ref mut inner) => inner.write_all(buf).await,
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => inner.write_all(buf).await,
            #[cfg(unix)]
            Self::UnixSocket(ref mut inner) => inner.write_all(buf).await,
        }
    }

    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        match self {
            Self::NoTls(ref mut inner) => inner.shutdown().await,
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => inner.shutdown().await,
            #[cfg(unix)]
            Self::UnixSocket(ref mut inner) => inner.shutdown().await,
        }
    }
}

impl Into<MaybeTlsWriteStream> for tokio::net::tcp::OwnedWriteHalf {
    fn into(self) -> MaybeTlsWriteStream {
        MaybeTlsWriteStream::NoTls(self)
    }
}

#[cfg(unix)]
impl Into<MaybeTlsWriteStream> for tokio::net::unix::OwnedWriteHalf {
    fn into(self) -> MaybeTlsWriteStream {
        MaybeTlsWriteStream::UnixSocket(self)
    }
}
