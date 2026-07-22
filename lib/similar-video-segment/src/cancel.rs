//! Cancellation token for aborting long-running operations.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A token that can be used to signal cancellation of a running operation.
///
/// Clone the token and share it across threads. Call `cancel()` from any thread
/// to request cancellation; the worker checks `is_cancelled()` periodically.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Create a new, non-cancelled token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Helper: check an optional cancellation token and return `Error::Cancelled` if set.
pub fn check_cancelled(token: &Option<CancellationToken>) -> crate::Result<()> {
    if let Some(t) = token
        && t.is_cancelled()
    {
        return Err(crate::Error::Cancelled);
    }
    Ok(())
}
