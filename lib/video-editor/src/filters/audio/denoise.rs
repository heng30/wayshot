use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};
use denoise::{DENOISE_MODEL, RealTimeDenoise};
use hound::{SampleFormat, WavSpec};
use std::{cell::RefCell, sync::Arc};

thread_local! {
    static DENOISE_STATE: RefCell<(Option<RealTimeDenoise<'static, f32>>, Option<usize>)>
        = RefCell::new((None, None));
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DenoiseFilter;

impl DenoiseFilter {
    pub const NAME: &'static str = "denoise";

    pub fn new() -> Self {
        Self
    }
}

impl Default for DenoiseFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioFilter for DenoiseFilter {
    crate::impl_default_audio_filter!(DenoiseFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let channels = data.config.channels;
        let sample_rate = data.config.sample_rate;
        let segment_id = Arc::as_ptr(&data.from_segment) as usize;

        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };

        DENOISE_STATE.with(|state_cell| {
            let mut state_guard = state_cell.borrow_mut();
            let (rt, last_segment_id) = &mut *state_guard;

            // Check if we need to reinitialize:
            // - No existing state
            // - Segment changed (boundary crossing - causes popping artifacts)
            // - Audio config changed
            let needs_reinit = rt.is_none()
                || *last_segment_id != Some(segment_id)
                || rt.as_ref().unwrap().spec().channels != channels
                || rt.as_ref().unwrap().spec().sample_rate != sample_rate;

            if needs_reinit {
                // Creating a new RealTimeDenoise resets first_frame to true,
                // which will skip the first frame output to avoid popping
                *rt = Some(RealTimeDenoise::new(&DENOISE_MODEL, spec).map_err(|e| {
                    crate::Error::InvalidConfig(format!("Failed to create denoise state: {}", e))
                })?);
                *last_segment_id = Some(segment_id);
            }

            let rt = rt.as_mut().unwrap();

            match rt.process(&data.samples) {
                Ok(Some(processed)) => data.samples = processed,
                Ok(None) => {} // First frame skipped or insufficient samples
                Err(e) => {
                    return Err(crate::Error::InvalidConfig(format!(
                        "Denoise processing error: {}",
                        e
                    )));
                }
            }

            Ok(())
        })
    }
}
