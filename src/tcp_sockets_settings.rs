#[async_trait::async_trait]
pub trait TcpClientSocketSettings {
    async fn get_host_port(&self) -> Option<String>;
    async fn get_tls_settings(&self) -> Option<TlsSettings>;
}

#[derive(Debug, Clone)]
pub struct TlsSettings {
    pub server_name: String,
}
