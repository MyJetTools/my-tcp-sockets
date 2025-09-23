use std::pin::Pin;

pub enum MaybeTlsReadStream {
    NoTls(tokio::net::tcp::OwnedReadHalf),
    #[cfg(feature = "with-tls")]
    Tls(tokio::io::ReadHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    #[cfg(feature = "unix-socket")]
    UnixSocket(tokio::net::unix::OwnedReadHalf),
}

impl tokio::io::AsyncRead for MaybeTlsReadStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::NoTls(ref mut inner) => {
                let result = Pin::new(inner).poll_read(cx, buf);
                return result;
            }
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_read(cx, buf);
                return result;
            }
            #[cfg(feature = "unix-socket")]
            Self::UnixSocket(ref mut inner) => {
                let result = Pin::new(inner).poll_read(cx, buf);
                return result;
            }
        }
    }
}

impl Into<MaybeTlsReadStream> for tokio::net::tcp::OwnedReadHalf {
    fn into(self) -> MaybeTlsReadStream {
        MaybeTlsReadStream::NoTls(self)
    }
}

#[cfg(feature = "unix-socket")]
impl Into<MaybeTlsReadStream> for tokio::net::unix::OwnedReadHalf {
    fn into(self) -> MaybeTlsReadStream {
        MaybeTlsReadStream::UnixSocket(self)
    }
}

pub enum MaybeTlsWriteStream {
    NoTls(tokio::net::tcp::OwnedWriteHalf),
    #[cfg(feature = "with-tls")]
    Tls(tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    #[cfg(feature = "unix-socket")]
    UnixSocket(tokio::net::unix::OwnedWriteHalf),
}

impl tokio::io::AsyncWrite for MaybeTlsWriteStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match &mut *self {
            Self::NoTls(ref mut inner) => {
                let result = Pin::new(inner).poll_write(cx, buf);
                return result;
            }
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_write(cx, buf);
                return result;
            }
            #[cfg(feature = "unix-socket")]
            Self::UnixSocket(ref mut inner) => {
                let result = Pin::new(inner).poll_write(cx, buf);
                return result;
            }
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match &mut *self {
            Self::NoTls(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            #[cfg(feature = "unix-socket")]
            Self::UnixSocket(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match &mut *self {
            Self::NoTls(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            #[cfg(feature = "with-tls")]
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            #[cfg(feature = "unix-socket")]
            Self::UnixSocket(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
        }
    }
}

impl Into<MaybeTlsWriteStream> for tokio::net::tcp::OwnedWriteHalf {
    fn into(self) -> MaybeTlsWriteStream {
        MaybeTlsWriteStream::NoTls(self)
    }
}

#[cfg(feature = "unix-socket")]
impl Into<MaybeTlsWriteStream> for tokio::net::unix::OwnedWriteHalf {
    fn into(self) -> MaybeTlsWriteStream {
        MaybeTlsWriteStream::UnixSocket(self)
    }
}
