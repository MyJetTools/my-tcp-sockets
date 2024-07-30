use std::pin::Pin;

use my_tls::tokio_rustls::client::TlsStream;

use tokio::{
    io::{ReadHalf, WriteHalf},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub enum MaybeTlsReadStream {
    Plain(OwnedReadHalf),
    Tls(ReadHalf<TlsStream<TcpStream>>),
}

impl tokio::io::AsyncRead for MaybeTlsReadStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(ref mut inner) => {
                let result = Pin::new(inner).poll_read(cx, buf);
                return result;
            }
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_read(cx, buf);
                return result;
            }
        }
    }
}

pub enum MaybeTlsWriteStream {
    Plain(OwnedWriteHalf),
    Tls(WriteHalf<TlsStream<TcpStream>>),
}

impl tokio::io::AsyncWrite for MaybeTlsWriteStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match &mut *self {
            Self::Plain(ref mut inner) => {
                let result = Pin::new(inner).poll_write(cx, buf);
                return result;
            }
            Self::Tls(ref mut inner) => {
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
            Self::Plain(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            Self::Tls(ref mut inner) => {
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
            Self::Plain(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
            Self::Tls(ref mut inner) => {
                let result = Pin::new(inner).poll_flush(cx);
                return result;
            }
        }
    }
}
