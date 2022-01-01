pub type ConnectionId = i32;

pub trait ApplicationStates {
    fn is_initialized(&self) -> bool;
    fn is_shutting_down(&self) -> bool;
}
