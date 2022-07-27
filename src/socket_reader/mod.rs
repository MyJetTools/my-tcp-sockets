mod error;
mod read_buffer;
mod socket_reader;

mod socket_reader_in_mem;
pub use error::ReadingTcpContractFail;
pub use read_buffer::ReadBuffer;
pub use socket_reader::{SocketReader, SocketReaderTcpStream};

pub use socket_reader_in_mem::SocketReaderInMem;
