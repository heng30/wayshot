use crate::ProgressState;
use hound::{WavReader, WavWriter};
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
                                Some(Ok(s)) => s * 32767.0, // Convert to 16-bit PCM range
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
                                Some(Ok(s)) => (s as f32) * 32767.0 / 8388607.0,
                                Some(Err(e)) => return Err(DenoiseError::Wav(e)),
                                None => break 'outer,
                            },
                            32 => match i32_iter.as_mut().unwrap().next() {
                                Some(Ok(s)) => (s as f32) * 32767.0 / 2147483647.0,
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
                            hound::SampleFormat::Float => sample / 32767.0, // Convert to [-1.0, 1.0]
                            hound::SampleFormat::Int => {
                                match format.bits_per_sample {
                                    16 => sample, // Already in 16-bit PCM range
                                    24 => {
                                        (sample * 8388607.0 / 32767.0).clamp(-8388608.0, 8388607.0)
                                    }
                                    32 => (sample * 2147483647.0 / 32767.0)
                                        .clamp(-2147483648.0, 2147483647.0),
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
