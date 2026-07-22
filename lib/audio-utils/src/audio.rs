use crate::{AudioProcessError, Result};
use pitch_shift::{Shifter, TOTAL_F32};

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
        log::debug!(
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
        log::debug!("Resampling audio from {current_sample_rate} Hz to {target_sample_rate} Hz");
        processed = resample_audio(
            &processed,
            current_sample_rate,
            target_sample_rate,
            target_channels,
        )?;
    }

    Ok(processed)
}

// 将每个通道重采样到指定数量。注意：不是重采样函数，一般用于数据显示
pub fn resample_audio_to_target_samples(
    samples: &[f32],
    channels: u16,
    target_frames_per_channel: u32,
) -> Vec<f32> {
    if channels == 0 {
        return samples.to_vec();
    }

    let channels = channels as usize;
    let input_frames = samples.len() / channels;
    let target_frames = target_frames_per_channel as usize;

    if input_frames == target_frames || input_frames == 0 {
        return samples.to_vec();
    }

    let ratio = input_frames as f64 / target_frames as f64;
    let mut output = vec![0.0f32; target_frames * channels];

    for ch in 0..channels {
        // Extract samples for this channel
        let input_channel: Vec<f32> = samples.iter().skip(ch).step_by(channels).cloned().collect();

        for out_frame in 0..target_frames {
            let input_pos = out_frame as f64 * ratio;
            let input_frame = input_pos.floor() as usize;
            let fraction = input_pos - input_frame as f64;

            if input_frame + 1 >= input_channel.len() {
                output[out_frame * channels + ch] = *input_channel.last().unwrap_or(&0.0);
            } else {
                let sample1 = input_channel[input_frame];
                let sample2 = input_channel[input_frame + 1];
                let interpolated = sample1 + (sample2 - sample1) * fraction as f32;
                output[out_frame * channels + ch] = interpolated;
            }
        }
    }

    output
}

// 对 Vec<Option<f32>> 进行线性重采样，保留间隙信息
pub fn resample_audio_to_target_samples_option(
    samples: &[Option<f32>],
    channels: u16,
    target_frames_per_channel: u32,
) -> Vec<Option<f32>> {
    if channels == 0 || samples.is_empty() || target_frames_per_channel == 0 {
        return Vec::new();
    }

    let target_frames = target_frames_per_channel as usize;
    let channels = channels as usize;
    let input_frames = samples.len() / channels;

    if input_frames == target_frames || input_frames == 0 {
        return samples.to_vec();
    }

    let ratio = input_frames as f64 / target_frames as f64;
    let mut output = vec![None; target_frames * channels];

    for ch in 0..channels {
        let input_channel: Vec<Option<f32>> =
            samples.iter().skip(ch).step_by(channels).cloned().collect();

        for out_frame in 0..target_frames {
            let input_pos = out_frame as f64 * ratio;
            let lower_idx = input_pos.floor() as usize;

            if lower_idx >= input_channel.len() {
                output[out_frame * channels + ch] = input_channel.last().and_then(|v| *v);
                continue;
            }

            let upper_idx = (lower_idx + 1).min(input_channel.len() - 1);
            let frac = input_pos - lower_idx as f64;

            let lower = input_channel[lower_idx];
            let upper = input_channel[upper_idx];

            let value = match (lower, upper) {
                (None, None) => None,
                (Some(l), Some(r)) => {
                    let interpolated = l as f64 * (1.0 - frac) + r as f64 * frac;
                    Some(interpolated as f32)
                }
                (Some(l), None) => {
                    if frac < 0.5 {
                        Some(l)
                    } else {
                        None
                    }
                }
                (None, Some(r)) => {
                    if frac >= 0.5 {
                        Some(r)
                    } else {
                        None
                    }
                }
            };

            output[out_frame * channels + ch] = value;
        }
    }

    output
}

/// Time stretch audio while preserving pitch using pitch_shift library
///
/// This function changes the duration of audio without affecting the pitch.
/// Use this for speed adjustment in video editing where pitch preservation is desired.
///
/// # Arguments
/// * `input_samples` - Input audio samples (interleaved by channel)
/// * `stretch_ratio` - Stretch ratio (> 1.0 stretches/slower, < 1.0 compresses/faster)
/// * `channels` - Number of audio channels
/// * `sample_rate` - Audio sample rate (e.g., 44100, 48000)
///
/// # Returns
/// * Time-stretched audio samples preserving original pitch
///
/// # Example
/// ```ignore
/// // To play audio at 2x speed (faster), use stretch_ratio = 0.5
/// let stretched = time_stretch_preserving_pitch(&samples, 0.5, 2, 44100);
/// // To play audio at 0.5x speed (slower), use stretch_ratio = 2.0
/// let stretched = time_stretch_preserving_pitch(&samples, 2.0, 2, 44100);
/// ```
pub fn time_stretch_preserving_pitch(
    input_samples: &[f32],
    stretch_ratio: f32,
    channels: u16,
    sample_rate: u32,
) -> Vec<f32> {
    if input_samples.is_empty() || (stretch_ratio - 1.0).abs() < 0.001 {
        return input_samples.to_vec();
    }

    let channels = channels as usize;
    let sample_rate = sample_rate as f32;

    // pitch_shift: out_samples/128 = stretch_ratio
    // We limit out_samples to < 1024 (max 1023)
    let out_samples_per_chunk = (128.0 * stretch_ratio).min(1023.0) as usize;
    if out_samples_per_chunk == 0 {
        // Avoid division issues for very fast speeds
        return input_samples.to_vec();
    }

    // Process each channel independently
    let mut output_channels: Vec<Vec<f32>> = Vec::with_capacity(channels);

    for ch in 0..channels {
        // Extract samples for this channel
        let channel_samples: Vec<f32> = input_samples
            .iter()
            .skip(ch)
            .step_by(channels)
            .cloned()
            .collect();

        let stretched = stretch_channel_with_pitch_shift(
            &channel_samples,
            stretch_ratio,
            sample_rate,
        );
        output_channels.push(stretched);
    }

    // Interleave output channels back together
    let output_frames = output_channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let mut output = vec![0.0f32; output_frames * channels];

    for frame in 0..output_frames {
        for ch in 0..channels {
            output[frame * channels + ch] = output_channels[ch][frame];
        }
    }

    output
}

/// Stretch a single channel using pitch_shift library
fn stretch_channel_with_pitch_shift(
    input: &[f32],
    stretch_ratio: f32,
    sample_rate: f32,
) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }

    // Create Shifter state buffer
    let state: Box<[f32; TOTAL_F32]> = vec![0.0; TOTAL_F32]
        .try_into()
        .expect("State buffer size mismatch");
    let mut shifter = Shifter::new(state);

    let out_samples = (128.0 * stretch_ratio).min(1023.0) as usize;
    if out_samples == 0 {
        return input.to_vec();
    }

    let mut output = Vec::new();

    // Process in 128-sample chunks
    for chunk in input.chunks(128) {
        // Pad last chunk if needed
        let mut input_chunk = [0.0f32; 128];
        let copy_len = chunk.len().min(128);
        input_chunk[..copy_len].copy_from_slice(&chunk[..copy_len]);

        // shift_semitones = 0 preserves pitch
        let shifted = shifter.shift(
            &input_chunk,
            0.0,           // shift_semitones = 0 (preserve pitch)
            out_samples,
            sample_rate,
        );
        output.extend_from_slice(shifted);
    }

    output
}
