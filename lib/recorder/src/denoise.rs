use crate::ProgressState;
use hound::{WavReader, WavSpec, WavWriter};
use nnnoiseless::{DenoiseState, RnnModel};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;

const FRAME_SIZE: usize = DenoiseState::FRAME_SIZE;

// 24-bit audio maximum value (2^23 - 1)
const I24_MAX: f32 = 8_388_607.0;

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

/// Denoise struct for audio denoising operations
pub struct Denoise {
    reader: WavReader<BufReader<File>>,
    writer: WavWriter<BufWriter<File>>,
}

impl Denoise {
    /// Create a new Denoise instance
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
        let writer = WavWriter::create(&output_file, reader.spec()).map_err(|e| {
            DenoiseError::Io(format!(
                "open {} failed: {e}",
                output_file.as_ref().display()
            ))
        })?;
        Ok(Self { reader, writer })
    }

    /// Note:
    ///     meybe need to resample to 48kHz. However, 44100Hz without resampling has a good denoising result
    ///
    /// Process audio in streaming mode directly from WAV reader
    pub fn run(
        mut self,
        stop_sig: Arc<AtomicBool>,
        mut progress_cb: Option<impl FnMut(f32)>,
    ) -> Result<ProgressState, DenoiseError> {
        let channels = self.reader.spec().channels as usize;
        let model = RnnModel::default();
        let format = &self.reader.spec();

        // Create denoise state for each channel
        let mut states: Vec<_> = (0..channels)
            .map(|_| DenoiseState::with_model(&model))
            .collect();

        // Input and output buffers
        let mut in_bufs = vec![vec![0.0; FRAME_SIZE]; channels];
        let mut out_bufs = vec![vec![0.0; FRAME_SIZE]; channels];

        let mut first_frame = true;
        let mut processed_data = 0;
        let total_data = self.reader.len() as usize;

        log::info!("Audio format validation:");
        log::info!("  Sample rate: {} Hz", format.sample_rate);
        log::info!("  Channels: {}", format.channels);
        log::info!("  Bits per sample: {}", format.bits_per_sample);
        log::info!("  Sample format: {:?}", format.sample_format);
        log::info!("  Frame size: {}", FRAME_SIZE);
        log::info!("  Total frames: {}", total_data / (FRAME_SIZE * channels));

        // Create sample iterators once to avoid recreation overhead
        let mut float_iter = None;
        let mut i16_iter = None;
        let mut i32_iter = None;

        match format.sample_format {
            hound::SampleFormat::Float => {
                float_iter = Some(self.reader.samples::<f32>());
            }
            hound::SampleFormat::Int => match format.bits_per_sample {
                16 => i16_iter = Some(self.reader.samples::<i16>()),
                24 | 32 => i32_iter = Some(self.reader.samples::<i32>()),
                bits => return Err(DenoiseError::UnsupportedBitDepth(bits)),
            },
        }

        // Process audio frame by frame
        'outer: loop {
            if stop_sig.load(Ordering::Relaxed) {
                return Ok(ProgressState::Stopped);
            }

            // Fill input buffers by reading samples from WAV file
            for frame_idx in 0..FRAME_SIZE {
                for channel in 0..channels {
                    // Read sample and convert to f32 in 16-bit PCM range
                    let sample = match format.sample_format {
                        hound::SampleFormat::Float => {
                            match float_iter.as_mut().unwrap().next() {
                                Some(Ok(s)) => s * i16::MAX as f32, // Convert to 16-bit PCM range
                                Some(Err(e)) => return Err(DenoiseError::Wav(e)),
                                None => break 'outer,
                            }
                        }
                        hound::SampleFormat::Int => match format.bits_per_sample {
                            16 => match i16_iter.as_mut().unwrap().next() {
                                Some(Ok(s)) => s as f32,
                                Some(Err(e)) => return Err(DenoiseError::Wav(e)),
                                None => break 'outer,
                            },
                            24 => match i32_iter.as_mut().unwrap().next() {
                                Some(Ok(s)) => (s as f32) * i16::MAX as f32 / I24_MAX,
                                Some(Err(e)) => return Err(DenoiseError::Wav(e)),
                                None => break 'outer,
                            },
                            32 => match i32_iter.as_mut().unwrap().next() {
                                Some(Ok(s)) => (s as f32) * i16::MAX as f32 / i32::MAX as f32,
                                Some(Err(e)) => return Err(DenoiseError::Wav(e)),
                                None => break 'outer,
                            },
                            bits => return Err(DenoiseError::UnsupportedBitDepth(bits)),
                        },
                    };
                    in_bufs[channel][frame_idx] = sample;
                }
            }

            // Denoise each channel
            for channel in 0..channels {
                states[channel].process_frame(&mut out_bufs[channel], &in_bufs[channel]);
            }

            // Skip first frame (contains initial delay)
            if !first_frame {
                // Re-interleave multi-channel output directly to writer
                for frame_idx in 0..FRAME_SIZE {
                    for channel in 0..channels {
                        let sample = out_bufs[channel][frame_idx];
                        let converted_sample = match format.sample_format {
                            hound::SampleFormat::Float => sample / i16::MAX as f32, // Convert to [-1.0, 1.0]
                            hound::SampleFormat::Int => {
                                match format.bits_per_sample {
                                    16 => sample, // Already in 16-bit PCM range
                                    24 => (sample * I24_MAX / i16::MAX as f32)
                                        .clamp(-I24_MAX - 1.0, I24_MAX),
                                    32 => (sample * i32::MAX as f32 / i16::MAX as f32)
                                        .clamp(i32::MIN as f32, i32::MAX as f32),
                                    _ => sample, // Should not happen due to validation
                                }
                            }
                        };
                        self.writer.write_sample(converted_sample)?;
                    }
                }
            }

            first_frame = false;

            if let Some(ref mut cb) = progress_cb
                && total_data > 0
            {
                processed_data += channels * FRAME_SIZE;
                cb(processed_data as f32 / total_data as f32);
            }
        }

        self.writer.finalize()?;

        if let Some(ref mut cb) = progress_cb {
            cb(1.0);
        }

        Ok(ProgressState::Finished)
    }
}

/// RealTimeDenoise struct for real-time audio denoising operations
pub struct RealTimeDenoise<'a> {
    spec: WavSpec,
    buffer: Vec<Vec<f32>>,
    states: Vec<Box<DenoiseState<'a>>>,
    states_output_frames: Vec<Vec<f32>>,
}

impl<'a> RealTimeDenoise<'a> {
    /// Create a new RealTimeDenoise instance
    pub fn new(model: &'a RnnModel, spec: WavSpec) -> Result<Self, DenoiseError> {
        // Validate audio format
        match spec.sample_format {
            hound::SampleFormat::Float => {}
            hound::SampleFormat::Int => match spec.bits_per_sample {
                16 | 24 | 32 => {}
                bits => return Err(DenoiseError::UnsupportedBitDepth(bits)),
            },
        }

        let channels = spec.channels as usize;

        // Create denoise state for each channel
        let states: Vec<_> = (0..channels)
            .map(|_| DenoiseState::with_model(&model))
            .collect();

        // Initialize input buffers for each channel
        let buffer = vec![Vec::with_capacity(FRAME_SIZE); channels];

        // Initialize fixed-size output frames for each channel
        let states_output_frames = vec![vec![0.0; FRAME_SIZE]; channels];

        Ok(Self {
            spec,
            buffer,
            states,
            states_output_frames,
        })
    }

    pub fn model() -> RnnModel {
        RnnModel::default()
    }

    /// Process audio data in real-time
    /// Returns Some(denoised_data) when at least one full frame is processed, None otherwise
    pub fn process_frame(&mut self, samples: &[f32]) -> Result<Option<Vec<f32>>, DenoiseError> {
        let channels = self.spec.channels as usize;

        // Validate input samples count
        if samples.len() % channels != 0 {
            return Err(DenoiseError::Processing(
                "Input samples count must be multiple of channels".to_string(),
            ));
        }

        // Add samples to buffer (convert to 16-bit PCM range)
        let mut sample_iter = samples.chunks_exact(channels);
        for chunk in &mut sample_iter {
            for (channel, &sample) in chunk.iter().enumerate() {
                let converted_sample = self.convert_to_pcm_range(sample);
                self.buffer[channel].push(converted_sample);
            }
        }

        // Calculate how many complete frames we can process
        let available_frames = self.buffer[0].len() / FRAME_SIZE;

        if available_frames > 0 {
            // Prepare output buffer for all processed frames
            let mut output = Vec::with_capacity(available_frames * FRAME_SIZE * channels);

            // Process all complete frames
            for frame_idx in 0..available_frames {
                let start = frame_idx * FRAME_SIZE;
                let end = start + FRAME_SIZE;

                // Process each channel for this frame
                for channel in 0..channels {
                    // Use buffer slice directly for processing
                    let input_slice = &self.buffer[channel][start..end];

                    // Perform denoising
                    self.states[channel]
                        .process_frame(&mut self.states_output_frames[channel], input_slice);
                }

                // Re-interleave multi-channel output for this frame and convert back to original format
                for sample_idx in 0..FRAME_SIZE {
                    for channel in 0..channels {
                        let sample = self.states_output_frames[channel][sample_idx];
                        // Convert back to original format range
                        let converted_sample = self.convert_from_pcm_range(sample);
                        output.push(converted_sample);
                    }
                }
            }

            // Remove all processed samples from buffer
            let samples_to_remove = available_frames * FRAME_SIZE;
            for channel in 0..channels {
                self.buffer[channel].drain(0..samples_to_remove);
            }

            Ok(Some(output))
        } else {
            // Not enough samples for a full frame
            Ok(None)
        }
    }

    /// Get the current audio specification
    pub fn spec(&self) -> &WavSpec {
        &self.spec
    }

    /// Get the number of buffered samples per channel
    pub fn buffered_samples(&self) -> usize {
        self.buffer[0].len()
    }

    /// Flush remaining buffered samples (less than FRAME_SIZE)
    /// Returns the remaining samples converted to original format and interleaved
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        let channels = self.spec.channels as usize;
        let remaining_samples = self.buffer[0].len();

        if remaining_samples == 0 {
            return None;
        }

        // Prepare output buffer for remaining samples
        let mut output = Vec::with_capacity(remaining_samples * channels);

        // Re-interleave remaining samples and convert back to original format
        for sample_idx in 0..remaining_samples {
            for channel in 0..channels {
                let sample = self.buffer[channel][sample_idx];
                // Convert back to original format range
                let converted_sample = self.convert_from_pcm_range(sample);
                output.push(converted_sample);
            }
        }

        // Clear buffer after flushing
        for channel in 0..channels {
            self.buffer[channel].clear();
        }

        Some(output)
    }

    /// Convert sample to 16-bit PCM range for processing
    #[inline]
    fn convert_to_pcm_range(&self, sample: f32) -> f32 {
        match self.spec.sample_format {
            hound::SampleFormat::Float => sample * i16::MAX as f32, // Convert to 16-bit PCM range
            hound::SampleFormat::Int => match self.spec.bits_per_sample {
                16 => sample, // Already in 16-bit PCM range
                24 => sample * i16::MAX as f32 / I24_MAX,
                32 => sample * i16::MAX as f32 / i32::MAX as f32,
                _ => sample, // Should not happen due to validation
            },
        }
    }

    /// Convert sample from 16-bit PCM range back to original format
    #[inline]
    fn convert_from_pcm_range(&self, sample: f32) -> f32 {
        match self.spec.sample_format {
            hound::SampleFormat::Float => sample / i16::MAX as f32, // Convert to [-1.0, 1.0]
            hound::SampleFormat::Int => match self.spec.bits_per_sample {
                16 => sample, // Already in 16-bit PCM range
                24 => (sample * I24_MAX / i16::MAX as f32).clamp(-I24_MAX - 1.0, I24_MAX),
                32 => (sample * i32::MAX as f32 / i16::MAX as f32)
                    .clamp(i32::MIN as f32, i32::MAX as f32),
                _ => sample, // Should not happen due to validation
            },
        }
    }
}
