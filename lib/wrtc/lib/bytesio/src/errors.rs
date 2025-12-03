#[derive(Debug, thiserror::Error)]
pub enum BytesIOError {
    #[error("not enough bytes")]
    NotEnoughBytes,

    #[error("empty stream")]
    EmptyStream,

    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("time out error: {0}")]
    TimeoutError(#[from] tokio::time::error::Elapsed),

    #[error("none return")]
    NoneReturn,
}

#[derive(Debug, thiserror::Error)]
pub enum BytesReadError {
    #[error("not enough bytes to read")]
    NotEnoughBytes,

    #[error("empty stream")]
    EmptyStream,

    #[error("io error: {0}")]
    IO(#[from] std::io::Error),

    #[error("index out of range")]
    IndexOutofRange,

    #[error("bytesio read error: {0}")]
    BytesIOError(#[from] BytesIOError),
}

#[derive(Debug, thiserror::Error)]
pub enum BytesWriteError {
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),

    #[error("bytes io error: {0}")]
    BytesIOError(#[from] BytesIOError),

    #[error("write time out")]
    Timeout,

    #[error("outof index")]
    OutofIndex,
}

#[derive(Debug, thiserror::Error)]
pub enum BitError {
    #[error("bytes read error: {0}")]
    BytesReadError(#[from] BytesReadError),

    #[error("bytes write error: {0}")]
    BytesWriteError(#[from] BytesWriteError),

    #[error("the size is bigger than 64")]
    TooBig,

    #[error("cannot write the whole 8 bits")]
    CannotWrite8Bit,

    #[error("cannot read byte")]
    CannotReadByte,
}
