// Error types module
//
// Central error definitions using thiserror, replacing anyhow

/// Core error type for slint-term library
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tab index out of bounds: {index}")]
    TabOutOfBounds { index: usize },

    #[error("Slint error: {0}")]
    Slint(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),
}

/// Convenience alias for Results using our Error type
pub type Result<T> = std::result::Result<T, Error>;
