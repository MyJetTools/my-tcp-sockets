#[async_trait::async_trait]
pub trait TcpConnectionAbstraction {
    async fn disconnect(&self);
}
