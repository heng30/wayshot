use crate::{AudioProcessError, Result};

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

pub fn multi_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }

    samples
        .chunks(channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn multi_to_stereo(samples: &[f32], input_channels: u16) -> Vec<f32> {
    let (input_ch, output_ch) = (input_channels as usize, 2);
    let frame_count = samples.len() / input_ch;
    let mut output = Vec::with_capacity(frame_count * output_ch);

    for frame in 0..frame_count {
        let frame_start = frame * input_ch;
        let frame_samples = &samples[frame_start..frame_start + input_ch];
        let (left, right) = downmix_frame(frame_samples, input_channels);
        output.extend_from_slice(&[left, right]);
    }

    output
}

fn downmix_frame(frame_samples: &[f32], input_channels: u16) -> (f32, f32) {
    match input_channels {
        // 3 channels: lefe, right, middle
        3 => (
            frame_samples[0] + frame_samples[2] * 0.707,
            frame_samples[1] + frame_samples[2] * 0.707,
        ),
        // 4 channels: front-left, front-right, back-left, back-right
        4 => (
            frame_samples[0] + frame_samples[2] * 0.7,
            frame_samples[1] + frame_samples[3] * 0.7,
        ),
        // 5.1 channels: left, right, middle, LFE, left-surround, right-surround
        6 => (
            frame_samples[0]
                + frame_samples[2] * 0.707
                + frame_samples[4] * 0.5
                + frame_samples[3] * 0.1,
            frame_samples[1]
                + frame_samples[2] * 0.707
                + frame_samples[5] * 0.5
                + frame_samples[3] * 0.1,
        ),

        _ => generic_downmix(frame_samples, input_channels),
    }
}

// FIXME: we don't know the channel layout, so it maybe output strange sounds
fn generic_downmix(frame_samples: &[f32], input_channels: u16) -> (f32, f32) {
    let input_ch = input_channels as usize;
    let (mut left, mut right) = (0.0, 0.0);

    for (i, &sample) in frame_samples.iter().enumerate() {
        // Calculating the weights of the left and right channels based on the channel positions.
        let pan = i as f32 / (input_ch - 1) as f32; // 0.0 = Left, 1.0 = Right

        // Using the square root curve for more natural panning effects.
        left += sample * (1.0 - pan).sqrt();
        right += sample * pan.sqrt();
    }

    let normalization = (input_ch as f32 / 2.0).sqrt();
    (left / normalization, right / normalization)
}

pub fn apply_fade_in(samples: &mut [f32], channels: u16, sample_rate: u32, duration_ms: u32) {
    let fade_frames = (sample_rate as f32 * duration_ms as f32 / 1000.0) as usize;
    let total_frames = samples.len() / channels as usize;
    let frames_to_process = fade_frames.min(total_frames);

    for i in 0..frames_to_process {
        let gain = i as f32 / fade_frames as f32;
        for c in 0..channels {
            let idx = i * channels as usize + c as usize;
            samples[idx] *= gain;
        }
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares: f32 = samples.iter().map(|&sample| sample * sample).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

pub fn max_sound_wave_amplitude(samples: &[f32]) -> f32 {
    let max_value = samples
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(&0.0);

    if *max_value == 0.0 {
        1.0
    } else {
        1.0 / max_value.abs().min(1.0)
    }
}

pub fn downsample_audio(audio_data: &[f32], target_length: usize) -> Vec<f32> {
    if audio_data.len() <= target_length {
        return audio_data.to_vec();
    }

    let chunk_size = (audio_data.len() as f32 / target_length as f32).ceil() as usize;

    audio_data
        .chunks(chunk_size)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .collect()
}

pub fn normalize_audio(samples: &[f32]) -> Vec<f32> {
    let max_amplitude = samples
        .iter()
        .map(|&s| s.abs())
        .fold(0.0f32, |a, b| a.max(b));

    if max_amplitude > 1.0 {
        samples.iter().map(|&s| s / max_amplitude).collect()
    } else {
        samples.to_vec()
    }
}

// pub fn resample_audio(
//     audio_data: &[f32],
//     original_sample_rate: u32,
//     target_sample_rate: u32,
// ) -> Result<Vec<f32>> {
//     if original_sample_rate == target_sample_rate {
//         return Ok(audio_data.to_vec());
//     }
//
//     let ratio = target_sample_rate as f32 / original_sample_rate as f32;
//     let new_length = (audio_data.len() as f32 * ratio).ceil() as usize;
//
//     if new_length == 0 {
//         return Ok(Vec::new());
//     }
//
//     let mut resampled = Vec::with_capacity(new_length);
//
//     for i in 0..new_length {
//         let src_pos = i as f32 / ratio;
//         let src_index_float = src_pos.floor();
//         let src_index = src_index_float as usize;
//         let src_frac = src_pos - src_index_float;
//
//         if src_index + 1 < audio_data.len() {
//             // Linear interpolation
//             let sample0 = audio_data[src_index];
//             let sample1 = audio_data[src_index + 1];
//             let interpolated = sample0 + (sample1 - sample0) * src_frac;
//             resampled.push(interpolated);
//         } else if src_index < audio_data.len() {
//             // Near the end, just use the last sample
//             resampled.push(audio_data[src_index]);
//         } else {
//             // Beyond the end, use zeros
//             resampled.push(0.0);
//         }
//     }
//
//     Ok(resampled)
// }
//
pub fn resample_audio(
    input_samples: &[f32],
    input_sample_rate: u32,
    output_sample_rate: u32,
    channels: u16,
) -> Result<Vec<f32>> {
    if input_sample_rate == output_sample_rate {
        return Ok(input_samples.to_vec());
    }

    let channels = channels as usize;
    let input_frames = input_samples.len() / channels;
    let ratio = output_sample_rate as f64 / input_sample_rate as f64;
    let output_frames = (input_frames as f64 * ratio).round() as usize;
    let output_samples = output_frames * channels;
    let mut output = vec![0.0f32; output_samples];

    // Simple linear interpolation resampling for each channel independently
    for ch in 0..channels {
        let input_channel: Vec<f32> = input_samples
            .iter()
            .skip(ch)
            .step_by(channels)
            .cloned()
            .collect();

        for out_frame in 0..output_frames {
            let input_pos = out_frame as f64 / ratio;
            let input_frame = input_pos.floor() as usize;
            let fraction = input_pos - input_frame as f64;

            if input_frame + 1 >= input_channel.len() {
                // At the end, just copy the last sample
                output[out_frame * channels + ch] = input_channel[input_channel.len() - 1];
            } else {
                // Linear interpolation between neighboring samples
                let sample1 = input_channel[input_frame];
                let sample2 = input_channel[input_frame + 1];
                let interpolated = sample1 + (sample2 - sample1) * fraction as f32;
                output[out_frame * channels + ch] = interpolated;
            }
        }
    }

    let actual_ratio = output_samples as f64 / input_samples.len() as f64;
    let expected_ratio = output_sample_rate as f64 / input_sample_rate as f64;
    let ratio_error = (actual_ratio - expected_ratio).abs();
    if ratio_error > 0.001 {
        return Err(AudioProcessError::Audio(format!(
            "Ratio error: expected {:.6}, got {:.6}",
            expected_ratio, actual_ratio
        )));
    }

    Ok(output)
}

pub fn resample_audio_with_channel(
    audio_data: &[f32],
    current_sample_rate: u32,
    current_channels: u16,
    target_sample_rate: u32,
    target_channels: u16,
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
        processed = resample_audio(
            &processed,
            current_sample_rate,
            target_sample_rate,
            target_channels,
        )?;
    }

    Ok(processed)
}
