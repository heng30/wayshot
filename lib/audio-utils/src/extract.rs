use crate::{AudioProcessError, Result};
use candle_core::{D, DType, Device, IndexOp, Tensor};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use realfft::RealFftPlanner;
use tensor_utils::pad_replicate_last_dim;

/// Mel frequency scale type
#[derive(Debug, Clone, Copy)]
pub enum MelScale {
    Htk,
    Kaldi,
    Slaney,
}

/// Convert Hertz to Mel frequency
pub fn hertz_to_mel(freq: f32, mel_scale: MelScale) -> f32 {
    match mel_scale {
        MelScale::Htk => 2595.0 * ((1.0 + freq / 700.0).log10()),
        MelScale::Kaldi => 1127.0 * ((1.0 + freq / 700.0).ln()),
        MelScale::Slaney => {
            let min_log_hertz = 1000.0;
            let min_log_mel = 15.0;
            let logstep = 27.0 / 6.4_f32.ln();
            let mut mels = 3.0 * freq / 200.0;

            if freq >= min_log_hertz {
                mels = min_log_mel + (freq / min_log_hertz).ln() * logstep;
            }
            mels
        }
    }
}

/// Convert Mel frequency to Hertz
pub fn mel_to_hertz(mels: f32, mel_scale: MelScale) -> f32 {
    match mel_scale {
        MelScale::Htk => 700.0 * (10.0_f32.powf(mels / 2595.0) - 1.0),
        MelScale::Kaldi => 700.0 * (f32::exp(mels / 1127.0) - 1.0),
        MelScale::Slaney => {
            let min_log_hertz = 1000.0;
            let min_log_mel = 15.0;
            let logstep = 6.4_f32.ln() / 27.0;
            let mut freq = 200.0 * mels / 3.0;

            if mels >= min_log_mel {
                freq = min_log_hertz * f32::exp(logstep * (mels - min_log_mel));
            }
            freq
        }
    }
}

pub fn crate_hamming_window(
    window_size: usize,
    periodic: bool,
    alpha: f64,
    beta: f64,
    dtype: DType,
    device: &Device,
) -> Result<Tensor> {
    let denominator = if periodic {
        window_size as f64
    } else {
        (window_size - 1) as f64
    };

    let window: Vec<f32> = (0..window_size)
        .map(|i| {
            let i_f64 = i as f64;
            let val = alpha - beta * (2.0 * std::f64::consts::PI * i_f64 / denominator).cos();
            val as f32
        })
        .collect();

    Ok(Tensor::from_vec(window, window_size, device)?.to_dtype(dtype)?)
}

pub fn stft_audio(n_fft: usize, frame_wave: &[f32]) -> Result<Vec<f32>> {
    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(n_fft);
    let mut spectrum = r2c.make_output_vec();
    let mut frame_wave = frame_wave.to_owned();
    r2c.process(&mut frame_wave, &mut spectrum)
        .map_err(|e| AudioProcessError::Audio(format!("FFT processing error: {}", e)))?;
    let output: Vec<f32> = spectrum.iter().map(|complex| complex.norm_sqr()).collect();
    Ok(output)
}

pub fn apply_stft(waveform: &Tensor) -> Result<Tensor> {
    // waveform: (bs, n_frames, window_size)
    let mut wave_fft = vec![];
    let (batch_size, _, window_size) = waveform.dims3()?;
    for bs in 0..batch_size {
        let wave_i = waveform.i(bs)?;
        let wave_i_vec = wave_i.to_vec2::<f32>()?;
        let wave_i_fft_vec: Result<Vec<Vec<f32>>> = wave_i_vec
            .par_iter()
            .map(|frame_wave| stft_audio(window_size, frame_wave))
            .collect();
        let wave_i_fft_vec = wave_i_fft_vec?;

        let wave_i_fft = Tensor::new(wave_i_fft_vec, waveform.device())?.unsqueeze(0)?;
        wave_fft.push(wave_i_fft);
    }
    let magnitudes = Tensor::cat(&wave_fft, 0)?;
    Ok(magnitudes)
}

pub fn kaldi_fbank(
    waveform: &Tensor,
    mel_energies: &Tensor,
    window_shift: usize,
    window_size: usize,
    padded_window_size: usize,
    dither: f32,
) -> Result<Tensor> {
    let (strided_input, _) = get_window(
        waveform,
        padded_window_size,
        window_size,
        window_shift,
        dither,
        true,
        true,
        0.97,
    )?;

    let spectrum = apply_stft(&strided_input)?;
    let mel_energies = spectrum.broadcast_matmul(mel_energies)?;
    let epsilon =
        Tensor::new(1.192_092_9e-7_f32, waveform.device())?.broadcast_as(mel_energies.shape())?;
    let mel_energies = mel_energies.maximum(&epsilon)?.log()?;

    Ok(mel_energies)
}

pub fn apply_lfr(inputs: &Tensor, lfr_m: usize, lfr_n: usize) -> Result<Tensor> {
    let (t, feat_dim) = inputs.dims2()?;
    let t_lfr = (t as f32 / lfr_n as f32).ceil() as usize;
    let left_padding_size = (lfr_m - 1) / 2;
    let left_padding = inputs.narrow(0, 0, 1)?.repeat((left_padding_size, 1))?;
    let mut inputs = Tensor::cat(&[&left_padding, inputs], 0)?;
    let t = t + left_padding_size;
    let last_idx = (t - lfr_m) / lfr_n + 1;
    let num_padding = lfr_m - (t - last_idx * lfr_n);
    if num_padding > 0 {
        // Use i32 to prevent underflow
        let term1 = 2 * lfr_m as i32 - 2 * t as i32;
        let term2 = (t_lfr - 1 + last_idx) * lfr_n;
        let num_padding = ((term1 + term2 as i32) / 2 * (t_lfr - last_idx) as i32) as usize;
        let right_padding = inputs.narrow(0, t - 1, 1)?.repeat((num_padding, 1))?;
        inputs = Tensor::cat(&[&inputs, &right_padding], 0)?;
    }
    let mut outputs = vec![];
    for i in 0..t_lfr {
        let start = i * lfr_n;
        let frame = inputs
            .narrow(0, start, lfr_m)?
            .reshape((1, lfr_m * feat_dim))?;
        outputs.push(frame);
    }
    let lfr_outputs = Tensor::cat(&outputs, 0)?;
    Ok(lfr_outputs)
}

pub fn get_waveform_and_window_properties(
    sample_frequency: usize,
    frame_shift: f32,
    frame_length: f32,
    round_to_power_of_two: bool,
) -> Result<(usize, usize, usize)> {
    let window_shift = (sample_frequency as f32 * frame_shift * 0.001) as usize;
    let window_size = (sample_frequency as f32 * frame_length * 0.001) as usize;
    let padded_window_size = if round_to_power_of_two {
        (window_size - 1).next_power_of_two()
    } else {
        window_size
    };
    Ok((window_shift, window_size, padded_window_size))
}

pub fn get_window(
    waveform: &Tensor,
    padded_window_size: usize,
    window_size: usize,
    window_shift: usize,
    dither: f32,
    remove_dc_offset: bool,
    raw_energy: bool,
    preemphasis_coefficient: f32,
) -> Result<(Tensor, Tensor)> {
    let mut strided_input = extract_frames(waveform, window_size, window_shift)?;
    // (ba, m, window_size)
    if dither != 0.0 {
        let rand_gauss = strided_input
            .randn_like(0.0, 1.0)?
            .affine(dither as f64, 0.0)?;
        strided_input = strided_input.add(&rand_gauss)?;
    }
    if remove_dc_offset {
        let row_means = strided_input.mean_keepdim(D::Minus1)?;
        strided_input = strided_input.broadcast_sub(&row_means)?;
    }
    let signal_log_energy = if raw_energy {
        let energy = strided_input.powf(2.0)?.sum(1)?.log()?;
        Some(energy)
    } else {
        None
    };

    if preemphasis_coefficient != 0.0 {
        let offset_strided_input = pad_replicate_last_dim(&strided_input, (1, 0))?
            .affine(preemphasis_coefficient as f64, 0.0)?;
        strided_input =
            strided_input.sub(&offset_strided_input.narrow(D::Minus1, 0, window_size)?)?;
    }

    let windows = crate_hamming_window(
        window_size,
        false,
        0.54,
        0.46,
        waveform.dtype(),
        waveform.device(),
    )?
    .unsqueeze(0)?
    .unsqueeze(0)?;

    strided_input = strided_input.broadcast_mul(&windows)?;

    if padded_window_size != window_size {
        let padding_right = padded_window_size - window_size;
        strided_input = strided_input.pad_with_zeros(D::Minus1, 0, padding_right)?;
    }

    let signal_log_energy = signal_log_energy.unwrap_or(strided_input.powf(2.0)?.sum(1)?.log()?);
    Ok((strided_input, signal_log_energy))
}

/// Extract audio frames
pub fn extract_frames(
    waveform: &Tensor,
    window_size: usize,
    window_shift: usize,
) -> Result<Tensor> {
    // waveform ->(1, audio_len)
    let waveform_len = waveform.dim(1)?;
    let n_frames = 1 + (waveform_len - window_size) / window_shift;
    let mut frames = Vec::with_capacity(n_frames);

    for i in 0..n_frames {
        let start = i * window_shift;
        let frame = waveform.narrow(D::Minus1, start, window_size)?;
        frames.push(frame);
    }

    let result = Tensor::cat(&frames, D::Minus1)?;
    let bs = result.dim(0)?;
    let reshaped = result.reshape((bs, n_frames, window_size))?;
    Ok(reshaped)
}

pub fn inverse_mel_scale(mel_freq: &Tensor) -> Result<Tensor> {
    Ok(mel_freq
        .affine(1.0 / 1127.0, 0.0)?
        .exp()?
        .affine(1.0, -1.0)?
        .affine(700.0, 0.0)?)
}

pub fn mel_scale(freq: &Tensor) -> Result<Tensor> {
    Ok(freq.affine(1.0 / 700.0, 1.0)?.log()?.affine(1127.0, 0.0)?)
}

pub fn kaldi_get_mel_banks(
    num_bins: usize,
    window_length_padded: usize,
    sample_freq: f32,
    low_freq: f32,
    high_freq: f32,
    device: &Device,
) -> Result<(Tensor, Tensor)> {
    assert!(num_bins > 3, "Must have at least 3 mel bins");
    assert!(
        window_length_padded.is_multiple_of(2),
        "window_length_padded must be even"
    );

    let num_fft_bins = window_length_padded as f32 / 2.0;
    let nyquist = 0.5 * sample_freq;

    let mut high_freq = high_freq;
    if high_freq <= 0.0 {
        high_freq += nyquist;
    }

    assert!(
        (0.0 <= low_freq && low_freq < nyquist)
            && (0.0 < high_freq && high_freq <= nyquist)
            && (low_freq < high_freq),
        "Bad values in options: low-freq {} and high-freq {} vs. nyquist {}",
        low_freq,
        high_freq,
        nyquist
    );

    // FFT bin width
    let fft_bin_width = sample_freq / (window_length_padded as f32);
    let mel_low_freq = hertz_to_mel(low_freq, MelScale::Kaldi);
    let mel_high_freq = hertz_to_mel(high_freq, MelScale::Kaldi);

    // Frequency interval between bins
    let mel_freq_delta = (mel_high_freq - mel_low_freq) / ((num_bins + 1) as f32);

    // Create bin index tensor
    let bins = Tensor::arange(0u32, num_bins as u32, device)?
        .to_dtype(candle_core::DType::F32)?
        .unsqueeze(1)?; // size(num_bins, 1)

    // Calculate boundary frequencies on mel scale
    let left_mel = bins.affine(mel_freq_delta as f64, mel_low_freq as f64)?;
    let center_mel = bins
        .affine(1.0, 1.0)?
        .affine(mel_freq_delta as f64, mel_low_freq as f64)?;
    let right_mel = bins
        .affine(1.0, 2.0)?
        .affine(mel_freq_delta as f64, mel_low_freq as f64)?;

    // Convert center frequency back to Hz
    let center_freqs = inverse_mel_scale(&center_mel)?;

    // Create FFT bin frequencies
    let fft_bins = Tensor::arange(0u32, num_fft_bins as u32, device)?
        .to_dtype(candle_core::DType::F32)?
        .affine(fft_bin_width as f64, 0.0)?;
    let mel = mel_scale(&fft_bins)?.unsqueeze(0)?; // size(1, num_fft_bins)

    // Calculate slopes
    let up_slope = mel
        .broadcast_sub(&left_mel)?
        .broadcast_div(&center_mel.broadcast_sub(&left_mel)?)?;
    let down_slope = right_mel
        .broadcast_sub(&mel)?
        .broadcast_div(&right_mel.broadcast_sub(&center_mel)?)?;

    // left_mel < center_mel < right_mel, so we can take the minimum of both slopes and clamp negative values
    let min_slopes = up_slope.minimum(&down_slope)?;
    let zeros = Tensor::zeros(min_slopes.dims(), candle_core::DType::F32, device)?;
    let bins_tensor = min_slopes.maximum(&zeros)?;

    Ok((bins_tensor, center_freqs))
}
