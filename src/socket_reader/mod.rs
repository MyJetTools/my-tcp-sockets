mod error;
mod socket_reader;

pub use error::ReadingTcpContractFail;
pub use socket_reader::{SocketReader, SocketReaderTcpStream};
