use crossbeam::channel::Sender;
use derive_setters::Setters;
use hound::WavSpec;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32},
};

#[cfg(target_os = "linux")]
mod speaker_recorder_linux;

#[derive(Debug, thiserror::Error)]
pub enum SpeakerRecorderError {
    #[error("Pipewire error: {0}")]
    PipewireError(String),
}

#[derive(Setters)]
#[setters(prefix = "with_")]
pub struct SpeakerRecorderConfig {
    #[setters(skip)]
    stop_sig: Arc<AtomicBool>,

    level_sender: Option<Sender<f32>>,
    frame_sender: Option<Sender<Vec<f32>>>,
    gain: Option<Arc<AtomicI32>>, // db
}

impl SpeakerRecorderConfig {
    pub fn new(stop_sig: Arc<AtomicBool>) -> Self {
        Self {
            stop_sig,
            level_sender: None,
            frame_sender: None,
            gain: None,
        }
    }
}

impl Default for SpeakerRecorderConfig {
    fn default() -> Self {
        Self::new(Arc::new(AtomicBool::new(false)))
    }
}

pub trait SpeakerRecorder {
    fn spec(&self) -> WavSpec;
    fn get_device_info(&self) -> Option<(u32, String)>;
    fn find_default_output(&self) -> Result<Option<(u32, String)>, SpeakerRecorderError>;
    fn start_recording(self) -> Result<(), SpeakerRecorderError>;
}

pub fn platform_speaker_recoder(
    config: SpeakerRecorderConfig,
) -> Result<impl SpeakerRecorder, SpeakerRecorderError> {
    #[cfg(target_os = "linux")]
    let recoder = speaker_recorder_linux::SpeakerRecorderLinux::new(config);

    recoder
}
