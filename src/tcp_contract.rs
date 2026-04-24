pub trait TcpContract {
    fn is_ping(&self) -> bool;
    fn is_pong(&self) -> bool;
}
