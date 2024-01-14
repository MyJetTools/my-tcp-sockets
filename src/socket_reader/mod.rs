mod error;
mod read_buffer;
mod socket_reader;

mod socket_reader_in_mem;
pub use error::ReadingTcpContractFail;
pub use read_buffer::ReadBuffer;
pub use socket_reader::SocketReader;

pub use socket_reader_in_mem::SocketReaderInMem;
mod my_read_buffer;
pub use my_read_buffer::*;
mod tcp_socket_reader;
pub use tcp_socket_reader::*;
