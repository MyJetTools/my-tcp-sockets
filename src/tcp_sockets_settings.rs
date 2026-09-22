#[async_trait::async_trait]
pub trait TcpClientSocketSettings {
    async fn get_host_port(&self) -> Option<String>;
    async fn get_tls_settings(&self) -> Option<TlsSettings>;
}

#[derive(Debug, Clone)]
pub struct TlsSettings {
    pub server_name: String,
    /// Skips server certificate chain validation (self-signed, expired, hostname mismatch).
    /// Handshake signatures are still verified. Use only for endpoints trusted out-of-band.
    pub accept_invalid_certs: bool,
}

impl TlsSettings {
    pub fn new(server_name: String) -> Self {
        Self {
            server_name,
            accept_invalid_certs: false,
        }
    }
}
