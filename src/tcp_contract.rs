pub trait TcpContract {
    fn is_pong(&self) -> bool;
}

pub trait TcpSerializationMetadata<TContract> {
    const THERE_IS_METADATA: bool; //True - means - there is no need to apply packets against metadata;
    fn apply_tcp_contract(&mut self, contract: &TContract);
}
