#[async_trait::async_trait]
pub trait TcpClientSocketSettings {
    async fn get_host_port(&self) -> Option<String>;
    async fn get_tls_settings(&self) -> Option<TlsSettings>;
}

#[derive(Debug, Clone)]
pub struct TlsSettings {
    pub server_name: String,
    /// DANGER: when `true` the client accepts ANY server certificate (self-signed, expired,
    /// hostname mismatch) and does not verify handshake signatures, so a MITM is not detected.
    /// Use only for endpoints trusted out-of-band. [`TlsSettings::new`] sets it to `false`.
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
