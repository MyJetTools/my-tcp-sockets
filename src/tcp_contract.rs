pub trait TcpContract {
    fn is_pong(&self) -> bool;
}

pub trait TcpSerializationMetadata<TContract> {
    //We check if we have to go through MutexGuard or RwLockReadGuard before apply_tcp_contract
    fn is_tcp_contract_related_to_metadata(&self, contract: &TContract) -> bool;
    fn apply_tcp_contract(&mut self, contract: &TContract);
}
