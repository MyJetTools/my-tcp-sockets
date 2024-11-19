use rust_extensions::ApplicationStates;

#[derive(Default)]
pub struct TcpConnectionStates {}

impl ApplicationStates for TcpConnectionStates {
    fn is_initialized(&self) -> bool {
        true
    }

    fn is_shutting_down(&self) -> bool {
        false
    }
}
