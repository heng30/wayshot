use audio_utils::audio::resample_audio;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use nnnoiseless::{DenoiseState, RnnModel};
use once_cell::sync::Lazy;
use std::{
    fs::File,
    io::BufReader,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;

const I24_MAX: f32 = 8_388_607.0; // 24-bit audio maximum value (2^23 - 1)
const I24_MIN: f32 = -8_388_608.0;
const FRAME_SIZE: usize = DenoiseState::FRAME_SIZE;
pub const DENOISE_SAMPLE_RATE: u32 = 48_000;
pub static DENOISE_MODEL: Lazy<RnnModel> = Lazy::new(|| denoise_model());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Finished,
    Stopped,
}

pub trait SampleType: Copy {
    fn to_f32(self) -> f32;
    fn from_f32(value: f32) -> Self;
}

impl SampleType for f32 {
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value
    }
}

impl SampleType for i16 {
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value as i16
    }
}

impl SampleType for i32 {
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value as i32
    }
}

#[derive(Error, Debug)]
pub enum DenoiseError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("WAV file error: {0}")]
    Wav(#[from] hound::Error),

    #[error("Unsupported bit depth: {0}")]
    UnsupportedBitDepth(u16),

    #[error("Invalid audio format")]
    InvalidFormat,

    #[error("Processing error: {0}")]
    Processing(String),
}

pub struct Denoise {
    reader: WavReader<BufReader<File>>,
    output_path: PathBuf,
    model: RnnModel,
}

impl Denoise {
    pub fn new(
        input_file: impl AsRef<Path>,
        output_file: impl AsRef<Path>,
    ) -> Result<Self, DenoiseError> {
        let reader = WavReader::open(&input_file).map_err(|e| {
            DenoiseError::Io(format!(
                "open {} failed: {e}",
                input_file.as_ref().display()
            ))
        })?;
        let output_path = output_file.as_ref().to_path_buf();
        Ok(Self {
            reader,
            output_path,
            model: RnnModel::default(),
        })
    }

    pub fn with_model(mut self, model: RnnModel) -> Self {
        self.model = model;
        self
    }

    pub fn process(
        mut self,
        stop_sig: Arc<AtomicBool>,
        mut progress_cb: Option<impl FnMut(f32)>,
    ) -> Result<ProgressState, DenoiseError> {
        let original_spec = self.reader.spec();
        let channels = original_spec.channels as usize;
        let original_sample_rate = original_spec.sample_rate;

        log::info!("Audio format validation:");
        log::info!("  Sample rate: {} Hz", original_spec.sample_rate);
        log::info!("  Channels: {}", original_spec.channels);
        log::info!("  Bits per sample: {}", original_spec.bits_per_sample);
        log::info!("  Sample format: {:?}", original_spec.sample_format);
        log::info!("  Frame size: {}", FRAME_SIZE);

        // Step 1: Read all samples into memory
        let mut all_samples: Vec<f32> = Vec::new();

        match original_spec.sample_format {
            SampleFormat::Float => {
                for sample in self.reader.samples::<f32>() {
                    let s = sample?;
                    all_samples.push(s * i16::MAX as f32);
                }
            }
            SampleFormat::Int => match original_spec.bits_per_sample {
                16 => {
                    for sample in self.reader.samples::<i16>() {
                        let s = sample?;
                        all_samples.push(s as f32);
                    }
                }
                24 => {
                    for sample in self.reader.samples::<i32>() {
                        let s = sample?;
                        all_samples.push((s as f32) * i16::MAX as f32 / I24_MAX);
                    }
                }
                32 => {
                    for sample in self.reader.samples::<i32>() {
                        let s = sample?;
                        all_samples.push((s as f32) * i16::MAX as f32 / i32::MAX as f32);
                    }
                }
                bits => return Err(DenoiseError::UnsupportedBitDepth(bits)),
            },
        }

        let total_samples = all_samples.len();
        if total_samples == 0 {
            return Ok(ProgressState::Finished);
        }

        // Step 2: Resample to denoise rate (48000 Hz) if needed
        let samples_at_denoise_rate = if original_sample_rate != DENOISE_SAMPLE_RATE {
            log::info!(
                "Resampling from {} Hz to {} Hz for denoising",
                original_sample_rate,
                DENOISE_SAMPLE_RATE
            );
            resample_to_denoise_rate(&all_samples, original_sample_rate, original_spec.channels)?
        } else {
            all_samples
        };

        // Step 3: Denoise
        let mut states: Vec<_> = (0..channels)
            .map(|_| DenoiseState::with_model(&self.model))
            .collect();

        let mut in_bufs = vec![vec![0.0; FRAME_SIZE]; channels];
        let mut out_bufs = vec![vec![0.0; FRAME_SIZE]; channels];

        // Deinterleave samples into channels
        let total_frames = samples_at_denoise_rate.len() / channels;
        let mut denoised_samples = vec![0.0f32; samples_at_denoise_rate.len()];

        let mut first_frame = true;
        let frames_count = total_frames / FRAME_SIZE;
        log::info!("  Total frames to process: {}", frames_count);

        for frame_num in 0..frames_count {
            if stop_sig.load(Ordering::Relaxed) {
                return Ok(ProgressState::Stopped);
            }

            let start = frame_num * FRAME_SIZE;

            // Fill input buffers
            for frame_idx in 0..FRAME_SIZE {
                for channel in 0..channels {
                    let sample_idx = (start + frame_idx) * channels + channel;
                    in_bufs[channel][frame_idx] = samples_at_denoise_rate[sample_idx];
                }
            }

            // Process denoising
            for channel in 0..channels {
                states[channel].process_frame(&mut out_bufs[channel], &in_bufs[channel]);
            }

            // Write output (skip first frame as per original logic)
            if !first_frame {
                for frame_idx in 0..FRAME_SIZE {
                    for channel in 0..channels {
                        let sample_idx = (start + frame_idx) * channels + channel;
                        denoised_samples[sample_idx] = out_bufs[channel][frame_idx];
                    }
                }
            }

            first_frame = false;

            if let Some(ref mut cb) = progress_cb {
                let progress = (frame_num + 1) as f32 / frames_count as f32 * 0.9; // 90% for denoising
                cb(progress);
            }
        }

        // Step 4: Resample back to original sample rate if needed
        let final_samples = if original_sample_rate != DENOISE_SAMPLE_RATE {
            log::info!(
                "Resampling back from {} Hz to {} Hz",
                DENOISE_SAMPLE_RATE,
                original_sample_rate
            );
            resample_from_denoise_rate(
                &denoised_samples,
                original_sample_rate,
                original_spec.channels,
            )?
        } else {
            denoised_samples
        };

        // Step 5: Write output file
        let output_spec = WavSpec {
            sample_rate: original_sample_rate,
            channels: original_spec.channels,
            bits_per_sample: original_spec.bits_per_sample,
            sample_format: original_spec.sample_format,
        };
        let mut writer = WavWriter::create(&self.output_path, output_spec)
            .map_err(|e| DenoiseError::Io(format!("create output file failed: {e}")))?;

        match original_spec.sample_format {
            hound::SampleFormat::Float => {
                for sample in final_samples.iter() {
                    let converted = sample / i16::MAX as f32;
                    writer.write_sample(converted)?;
                }
            }
            hound::SampleFormat::Int => match original_spec.bits_per_sample {
                16 => {
                    for sample in final_samples.iter() {
                        let converted = *sample as i16;
                        writer.write_sample(converted)?;
                    }
                }
                24 => {
                    for sample in final_samples.iter() {
                        let converted =
                            (sample * I24_MAX / i16::MAX as f32).clamp(I24_MIN, I24_MAX) as i32;
                        writer.write_sample(converted)?;
                    }
                }
                32 => {
                    for sample in final_samples.iter() {
                        let converted = (sample * i32::MAX as f32 / i16::MAX as f32)
                            .clamp(i32::MIN as f32, i32::MAX as f32)
                            as i32;
                        writer.write_sample(converted)?;
                    }
                }
                _ => {
                    for sample in final_samples.iter() {
                        writer.write_sample(*sample as i16)?;
                    }
                }
            },
        }

        writer.finalize()?;

        if let Some(ref mut cb) = progress_cb {
            cb(1.0);
        }

        Ok(ProgressState::Finished)
    }
}

pub struct RealTimeDenoise<'a, T: SampleType = f32> {
    spec: WavSpec,
    original_sample_rate: u32,
    buffer: Vec<Vec<f32>>,
    states: Vec<Box<DenoiseState<'a>>>,
    states_output_frames: Vec<Vec<f32>>,
    first_frame: bool,
    _marker: PhantomData<T>,
}

impl<'a, T: SampleType> RealTimeDenoise<'a, T> {
    pub fn new(model: &'a RnnModel, spec: WavSpec) -> Result<Self, DenoiseError> {
        match spec.sample_format {
            hound::SampleFormat::Float => {}
            hound::SampleFormat::Int => match spec.bits_per_sample {
                16 | 24 | 32 => {}
                bits => return Err(DenoiseError::UnsupportedBitDepth(bits)),
            },
        }

        let channels = spec.channels as usize;

        let states: Vec<_> = (0..channels)
            .map(|_| DenoiseState::with_model(&model))
            .collect();

        let buffer = vec![Vec::with_capacity(FRAME_SIZE); channels];
        let states_output_frames = vec![vec![0.0; FRAME_SIZE]; channels];

        Ok(Self {
            spec,
            buffer,
            states,
            states_output_frames,
            original_sample_rate: spec.sample_rate,
            first_frame: true,
            _marker: PhantomData,
        })
    }

    pub fn process(&mut self, samples: &[T]) -> Result<Option<Vec<T>>, DenoiseError> {
        let channels = self.spec.channels as usize;

        if samples.len() % channels != 0 {
            return Err(DenoiseError::Processing(
                "Input samples count must be multiple of channels".to_string(),
            ));
        }

        // Step 1: Convert input samples to f32
        let mut input_f32: Vec<f32> = samples.iter().map(|s| s.to_f32()).collect();

        // Step 2: Resample to denoise rate if needed
        let samples_at_denoise_rate = if self.original_sample_rate != DENOISE_SAMPLE_RATE {
            // Convert to PCM range first
            for sample in &mut input_f32 {
                *sample = self.convert_to_pcm_range(*sample);
            }
            let resampled = resample_to_denoise_rate(
                &input_f32,
                self.original_sample_rate,
                self.spec.channels,
            )?;
            resampled
        } else {
            input_f32
                .iter()
                .map(|s| self.convert_to_pcm_range(*s))
                .collect()
        };

        // Step 3: Add to buffer
        for chunk in samples_at_denoise_rate.chunks_exact(channels) {
            for (channel, &sample) in chunk.iter().enumerate() {
                self.buffer[channel].push(sample);
            }
        }

        let available_frames = self.buffer[0].len() / FRAME_SIZE;

        if available_frames > 0 {
            let mut output = Vec::with_capacity(available_frames * FRAME_SIZE * channels);

            for frame_idx in 0..available_frames {
                let start = frame_idx * FRAME_SIZE;
                let end = start + FRAME_SIZE;

                for channel in 0..channels {
                    let input_slice = &self.buffer[channel][start..end];

                    self.states[channel]
                        .process_frame(&mut self.states_output_frames[channel], input_slice);
                }

                // Skip output for first frame to avoid popping artifacts from
                // uninitialized synthesis_mem in nnnoiseless overlap-add
                if !self.first_frame {
                    for sample_idx in 0..FRAME_SIZE {
                        for channel in 0..channels {
                            let sample = self.states_output_frames[channel][sample_idx];
                            output.push(sample);
                        }
                    }
                } else {
                    self.first_frame = false;
                }
            }

            let samples_to_remove = available_frames * FRAME_SIZE;
            for channel in 0..channels {
                self.buffer[channel].drain(0..samples_to_remove);
            }

            // If no output (first frame case), return None
            if output.is_empty() {
                return Ok(None);
            }

            // Step 4: Resample back to original rate if needed
            let final_output = if self.original_sample_rate != DENOISE_SAMPLE_RATE {
                resample_from_denoise_rate(&output, self.original_sample_rate, self.spec.channels)?
            } else {
                output
            };

            // Step 5: Convert back from PCM range
            let result: Vec<T> = final_output
                .iter()
                .map(|s| {
                    let converted = self.convert_from_pcm_range(*s);
                    T::from_f32(converted)
                })
                .collect();

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    pub fn spec(&self) -> &WavSpec {
        &self.spec
    }

    pub fn buffered_samples(&self) -> usize {
        self.buffer[0].len()
    }

    pub fn is_first_frame(&self) -> bool {
        self.first_frame
    }

    pub fn flush(&mut self) -> Option<Vec<T>> {
        let channels = self.spec.channels as usize;
        let remaining_samples = self.buffer[0].len();

        if remaining_samples == 0 {
            return None;
        }

        // Collect samples from buffer (in denoise rate)
        let mut output = Vec::with_capacity(remaining_samples * channels);
        for sample_idx in 0..remaining_samples {
            for channel in 0..channels {
                let sample = self.buffer[channel][sample_idx];
                output.push(sample);
            }
        }

        for channel in 0..channels {
            self.buffer[channel].clear();
        }

        // Resample back to original rate if needed
        let final_output = if self.original_sample_rate != DENOISE_SAMPLE_RATE {
            match resample_from_denoise_rate(&output, self.original_sample_rate, self.spec.channels)
            {
                Ok(resampled) => resampled,
                Err(_) => return None,
            }
        } else {
            output
        };

        // Convert back from PCM range
        let result: Vec<T> = final_output
            .iter()
            .map(|s| {
                let converted = self.convert_from_pcm_range(*s);
                T::from_f32(converted)
            })
            .collect();

        Some(result)
    }

    #[inline]
    fn convert_to_pcm_range(&self, sample: f32) -> f32 {
        match self.spec.sample_format {
            SampleFormat::Float => sample * i16::MAX as f32,
            SampleFormat::Int => match self.spec.bits_per_sample {
                16 => sample,
                24 => sample * i16::MAX as f32 / I24_MAX,
                32 => sample * i16::MAX as f32 / i32::MAX as f32,
                _ => sample,
            },
        }
    }

    #[inline]
    fn convert_from_pcm_range(&self, sample: f32) -> f32 {
        match self.spec.sample_format {
            SampleFormat::Float => sample / i16::MAX as f32, // Convert to [-1.0, 1.0]
            SampleFormat::Int => match self.spec.bits_per_sample {
                16 => sample,
                24 => (sample * I24_MAX / i16::MAX as f32).clamp(I24_MIN, I24_MAX),
                32 => (sample * i32::MAX as f32 / i16::MAX as f32)
                    .clamp(i32::MIN as f32, i32::MAX as f32),
                _ => sample,
            },
        }
    }
}

pub fn denoise_model() -> RnnModel {
    RnnModel::default()
}

pub fn resample_to_denoise_rate(
    input_samples: &[f32],
    input_sample_rate: u32,
    channels: u16,
) -> Result<Vec<f32>, DenoiseError> {
    if input_sample_rate == DENOISE_SAMPLE_RATE {
        return Ok(input_samples.to_vec());
    }

    resample_audio(
        input_samples,
        input_sample_rate,
        DENOISE_SAMPLE_RATE,
        channels,
    )
    .map_err(|e| DenoiseError::Processing(format!("Resample error: {}", e)))
}

pub fn resample_from_denoise_rate(
    input_samples: &[f32],
    output_sample_rate: u32,
    channels: u16,
) -> Result<Vec<f32>, DenoiseError> {
    if output_sample_rate == DENOISE_SAMPLE_RATE {
        return Ok(input_samples.to_vec());
    }

    resample_audio(
        input_samples,
        DENOISE_SAMPLE_RATE,
        output_sample_rate,
        channels,
    )
    .map_err(|e| DenoiseError::Processing(format!("Resample error: {}", e)))
}
