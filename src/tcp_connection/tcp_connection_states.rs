use rust_extensions::ApplicationStates;

pub struct TcpConnectionStates {}

impl TcpConnectionStates {
    pub fn new() -> Self {
        Self {}
    }
}

impl ApplicationStates for TcpConnectionStates {
    fn is_initialized(&self) -> bool {
        true
    }

    fn is_shutting_down(&self) -> bool {
        false
    }
}
