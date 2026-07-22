use audio_utils::audio::{multi_to_mono, resample_audio};
use silero::{Session, SpeechOptions, detect_speech};

const VAD_SAMPLE_RATE: u32 = 16000;

pub fn detect_voice_segments(
    mono_samples: &[f32],
    sample_rate: u32,
) -> Result<Vec<(u64, u64)>, String> {
    let resampled = if sample_rate != VAD_SAMPLE_RATE {
        resample_audio(mono_samples, sample_rate, VAD_SAMPLE_RATE, 1)
            .map_err(|e| format!("Resample failed: {e}"))?
    } else {
        mono_samples.to_vec()
    };

    if resampled.is_empty() {
        return Ok(vec![]);
    }

    let mut session =
        Session::bundled().map_err(|e| format!("Failed to load silero model: {e}"))?;
    let config = SpeechOptions::default();
    let segments =
        detect_speech(&mut session, &resampled, config).map_err(|e| format!("VAD failed: {e}"))?;

    // Convert from 16kHz sample indices to milliseconds
    let speech_regions: Vec<(u64, u64)> = segments
        .iter()
        .map(|seg| {
            let start_ms =
                (seg.start_sample() as f64 / VAD_SAMPLE_RATE as f64 * 1000.0).round() as u64;
            let end_ms = (seg.end_sample() as f64 / VAD_SAMPLE_RATE as f64 * 1000.0).round() as u64;
            (start_ms, end_ms)
        })
        .collect();

    Ok(speech_regions)
}

pub fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    multi_to_mono(samples, channels)
}
