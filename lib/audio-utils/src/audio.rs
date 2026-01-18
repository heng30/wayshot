use crate::{AudioProcessError, Result};

pub fn resample_audio(
    audio_data: &[f32],
    original_sample_rate: u32,
    target_sample_rate: u32,
) -> Result<Vec<f32>> {
    if original_sample_rate == target_sample_rate {
        return Ok(audio_data.to_vec());
    }

    let ratio = target_sample_rate as f32 / original_sample_rate as f32;
    let new_length = (audio_data.len() as f32 * ratio).ceil() as usize;

    if new_length == 0 {
        return Ok(Vec::new());
    }

    let mut resampled = Vec::with_capacity(new_length);

    for i in 0..new_length {
        let src_pos = i as f32 / ratio;
        let src_index_float = src_pos.floor();
        let src_index = src_index_float as usize;
        let src_frac = src_pos - src_index_float;

        if src_index + 1 < audio_data.len() {
            // Linear interpolation
            let sample0 = audio_data[src_index];
            let sample1 = audio_data[src_index + 1];
            let interpolated = sample0 + (sample1 - sample0) * src_frac;
            resampled.push(interpolated);
        } else if src_index < audio_data.len() {
            // Near the end, just use the last sample
            resampled.push(audio_data[src_index]);
        } else {
            // Beyond the end, use zeros
            resampled.push(0.0);
        }
    }

    Ok(resampled)
}

pub fn mono_to_stereo(audio_data: &[f32]) -> Vec<f32> {
    let mut stereo = Vec::with_capacity(audio_data.len() * 2);
    for &sample in audio_data.iter() {
        stereo.push(sample);
        stereo.push(sample);
    }
    stereo
}

pub fn stereo_to_mono(audio_data: &[f32]) -> Vec<f32> {
    let mut mono = Vec::with_capacity(audio_data.len() / 2);
    for chunk in audio_data.chunks(2) {
        if chunk.len() == 2 {
            mono.push((chunk[0] + chunk[1]) / 2.0);
        } else {
            mono.push(chunk[0]);
        }
    }
    mono
}

pub fn normalize_audio(
    audio_data: &[f32],
    current_sample_rate: u32,
    current_channels: u32,
    target_sample_rate: u32,
    target_channels: u32,
) -> Result<Vec<f32>> {
    let mut processed = audio_data.to_vec();

    if current_channels != target_channels {
        log::info!(
            "Converting audio from {current_channels} channels to {target_channels} channels",
        );

        processed = if current_channels == 2 && target_channels == 1 {
            stereo_to_mono(&processed)
        } else if current_channels == 1 && target_channels == 2 {
            mono_to_stereo(&processed)
        } else {
            return Err(AudioProcessError::Audio(format!(
                "Unsupported channel conversion: {current_channels} -> {target_channels}"
            )));
        };
    }

    if current_sample_rate != target_sample_rate {
        log::info!("Resampling audio from {current_sample_rate} Hz to {target_sample_rate} Hz");
        processed = resample_audio(&processed, current_sample_rate, target_sample_rate)?;
    }

    Ok(processed)
}
