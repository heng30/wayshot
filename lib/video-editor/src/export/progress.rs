use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub type ProgressCallback = Box<dyn Fn(ExportProgress) + Send + Sync>;

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
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPhase {
    Initializing,
    EncodingVideo,
    ProcessingAudio,
    Finalizing,
    Complete,
}

impl ExportPhase {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportProgress {
    pub current_position: Duration,
    pub total_duration: Duration,
    pub frames_processed: u64,
    pub total_frames: u64,
    pub phase: ExportPhase,
}

impl ExportProgress {
    pub fn progress(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.frames_processed as f32 / self.total_frames as f32
    }
}
