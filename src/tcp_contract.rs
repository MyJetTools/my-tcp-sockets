pub trait TcpContract {
    fn is_pong(&self) -> bool;
}

pub trait SerializationMetadata<TContract> {
    fn apply_tcp_contract(&mut self, contract: &TContract);
}
