#[async_trait::async_trait]
pub trait TcpClientSocketSettings {
    async fn get_host_port(&self) -> Option<String>;
}
