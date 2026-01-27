use crate::SampleType;
use audio_utils::audio::{
    mono_to_stereo, multi_to_mono, multi_to_stereo, normalize_audio, resample_audio,
};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_builder::Builder;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::{fs::File, io::BufWriter, marker::PhantomData, path::PathBuf};
use thiserror::Error;

const FRAME_DURATION_MS: usize = 20;

/// Common audio sample rates
pub mod sample_rate {
    /// CD quality sample rate
    pub const CD: u32 = 44100;

    /// Professional audio sample rate
    pub const PROFESSIONAL: u32 = 48000;

    /// High-resolution audio sample rate
    pub const HIGH_RES: u32 = 96000;

    /// Ultra high-resolution audio sample rate
    pub const ULTRA_HIGH_RES: u32 = 192000;

    /// Telephone quality sample rate
    pub const TELEPHONE: u32 = 8000;

    /// Low quality sample rate
    pub const LOW: u32 = 16000;

    /// Medium quality sample rate
    pub const MEDIUM: u32 = 22050;
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Track index {0} out of bounds")]
    TrackIndexOutOfBounds(usize),

    #[error("No track")]
    NoTrack,

    #[error("Resampling error: {0}")]
    ResamplingError(String),

    #[error("File I/O error: {0}")]
    FileIoError(#[from] std::io::Error),

    #[error("WAV file error: {0}")]
    WavError(#[from] hound::Error),

    #[error("Audio processor error: {0}")]
    AudioProcess(#[from] audio_utils::AudioProcessError),
}

#[derive(Debug, Clone)]
pub enum OutputDestination<T> {
    File(PathBuf),
    Channel(Sender<Vec<T>>),
    Both(PathBuf, Sender<Vec<T>>),
}

#[derive(Builder)]
pub struct AudioProcessorConfig<T> {
    #[builder(default = "1024")]
    channel_size: usize,

    #[builder(default = "44100")]
    target_sample_rate: u32,

    convert_to_mono: bool,

    output_destination: Option<OutputDestination<T>>,
}

pub struct AudioProcessor<T: SampleType = f32> {
    config: AudioProcessorConfig<T>,
    max_channels: u16,
    specs: Vec<WavSpec>,
    buffers: Vec<Vec<f32>>,
    original_channels: Vec<u16>,
    sample_receiver: Vec<Receiver<Vec<f32>>>,
    writer: Option<WavWriter<BufWriter<File>>>,
    _marker: PhantomData<T>,
}

impl<T: SampleType> AudioProcessor<T> {
    pub fn new(config: AudioProcessorConfig<T>) -> AudioProcessor<T> {
        Self {
            config,
            max_channels: 1,
            specs: vec![],
            buffers: vec![],
            original_channels: vec![],
            sample_receiver: vec![],
            writer: None,
            _marker: PhantomData,
        }
    }

    pub fn add_track(&mut self, mut spec: WavSpec) -> Sender<Vec<f32>> {
        log::info!("add track: {spec:?}");

        self.original_channels.push(spec.channels);
        spec.channels = spec.channels.min(2); // max support channel size is 2
        self.max_channels = self.max_channels.max(spec.channels);

        self.specs.push(spec);
        self.buffers
            .push(Vec::with_capacity(spec.sample_rate as usize * 3));

        let (sender, receiver) = bounded(self.config.channel_size);
        self.sample_receiver.push(receiver);

        sender
    }

    pub fn process_samples(&mut self) -> Result<(), AudioError> {
        if self.specs.len() == 0 {
            return Err(AudioError::NoTrack);
        }

        for i in 0..self.sample_receiver.len() {
            let receiver = &self.sample_receiver[i];
            while let Ok(samples) = receiver.try_recv() {
                let mut samples = if self.original_channels[i] > 2 {
                    multi_to_stereo(&samples, self.original_channels[i])
                } else {
                    samples
                };

                self.convert_samples_to_f32(&mut samples, i);
                self.buffers[i].extend(samples);
            }
        }

        loop {
            let mut max_frames = 0;
            let mut is_all_track_ready = true;

            for i in 0..self.specs.len() {
                let spec = &self.specs[i];
                let samples_per_frame =
                    spec.sample_rate as usize * spec.channels as usize * FRAME_DURATION_MS / 1000;

                if self.buffers[i].len() < samples_per_frame {
                    is_all_track_ready = false;
                }

                max_frames = max_frames.max(self.buffers[i].len() / samples_per_frame);
            }

            if !is_all_track_ready {
                if max_frames < 3 {
                    return Ok(());
                } else {
                    log::debug!(
                        "At least one audio buffer samples counts is great than {}ms samples counts",
                        3 * FRAME_DURATION_MS
                    );
                }
            }

            // Process all tracks with the same amount of samples
            let mut all_processed_tracks = Vec::new();
            for i in 0..self.specs.len() {
                let spec = &self.specs[i];
                let samples_per_frame =
                    spec.sample_rate as usize * spec.channels as usize * FRAME_DURATION_MS / 1000;

                if self.buffers[i].len() < samples_per_frame {
                    // Process available samples and pad with silence
                    let silence_samples = samples_per_frame - self.buffers[i].len();
                    self.buffers[i].extend(vec![0.0; silence_samples]);
                };

                let processed = self.resamples(i, samples_per_frame)?;

                if !processed.is_empty() {
                    all_processed_tracks.push(processed);
                }
            }

            if all_processed_tracks.is_empty() {
                return Ok(());
            }

            // Unify channel counts before mixing
            let mut unified_tracks = Vec::new();
            for (i, track_samples) in all_processed_tracks.into_iter().enumerate() {
                let spec = &self.specs[i];
                // After resampling, all tracks should have the same sample rate but may have different channel counts
                // Convert to max_channels for consistent mixing
                if spec.channels == 1 && self.max_channels > 1 {
                    unified_tracks.push(mono_to_stereo(&track_samples));
                } else {
                    unified_tracks.push(track_samples);
                }
            }

            let mut final_samples = if unified_tracks.len() > 1 {
                self.mix_multiple_tracks(&unified_tracks)
            } else {
                unified_tracks[0].clone()
            };

            // Apply mono conversion after mixing if needed
            if self.config.convert_to_mono && self.max_channels > 1 {
                final_samples = multi_to_mono(&final_samples, self.max_channels);
            }

            if self.specs.len() > 1 {
                // TODO: normalize audio may cause sound unbalanced compare to original sound
                let normalized = normalize_audio(&final_samples);
                self.handle_output(&normalized);
            } else {
                self.handle_output(&final_samples);
            }
        }
    }

    fn resamples(
        &mut self,
        track_index: usize,
        samples_to_process: usize,
    ) -> Result<Vec<f32>, AudioError> {
        let spec = &self.specs[track_index];
        let buffer = &mut self.buffers[track_index];

        if buffer.is_empty() {
            return Ok(Vec::new());
        }

        // Ensure we process complete frames (multiples of channel count)
        let channels = spec.channels as usize;
        let complete_samples = (buffer.len() / channels) * channels;
        let samples_to_process = samples_to_process.min(complete_samples);

        if samples_to_process == 0 {
            return Ok(Vec::new());
        }

        let resampled_samples = if spec.sample_rate != self.config.target_sample_rate {
            let samples = resample_audio(
                &buffer[0..samples_to_process],
                spec.sample_rate,
                self.config.target_sample_rate,
                spec.channels,
            )?;

            buffer.drain(0..samples_to_process);
            samples
        } else {
            buffer.drain(0..samples_to_process).collect()
        };

        Ok(resampled_samples)
    }

    fn convert_samples_to_f32(&self, samples: &mut [f32], track_index: usize) {
        let spec = &self.specs[track_index];

        match spec.sample_format {
            SampleFormat::Float => (),
            SampleFormat::Int => {
                // Convert integer samples to f32 in range [-1.0, 1.0]
                let max_value = (1 << (spec.bits_per_sample - 1)) as f32;
                for s in samples {
                    // log::debug!("{s} : {max_value}");
                    *s = *s / max_value;
                }
            }
        }
    }

    fn mix_multiple_tracks(&self, track_samples: &[Vec<f32>]) -> Vec<f32> {
        if track_samples.is_empty() {
            return Vec::new();
        }

        // All tracks should already have the same channel count (max_channels)
        // due to the conversion in resamples
        let max_len = track_samples.iter().map(|s| s.len()).max().unwrap_or(0);
        let mut mixed = vec![0.0; max_len];

        for samples in track_samples {
            for (i, &sample) in samples.iter().enumerate() {
                if i < mixed.len() {
                    mixed[i] += sample;
                }
            }
        }

        // Apply averaging if we have multiple tracks
        if track_samples.len() > 1 {
            for sample in &mut mixed {
                *sample /= track_samples.len() as f32;
            }
        }

        mixed
    }

    fn handle_output(&mut self, samples: &[f32]) {
        if let Some(destination) = &self.config.output_destination {
            match destination {
                OutputDestination::File(file_path) => {
                    let file_path = file_path.clone();
                    if let Err(e) = self.write_samples_to_file(&file_path, samples) {
                        log::warn!("Failed to write audio to file {:?}: {}", file_path, e);
                    }
                }
                OutputDestination::Channel(sender) => {
                    let sender = sender.clone();
                    if let Err(e) = sender.try_send(if T::sample_format() == SampleFormat::Float {
                        samples
                            .into_iter()
                            .map(|s| T::from_f32(*s))
                            .collect::<Vec<T>>()
                    } else {
                        samples
                            .into_iter()
                            .map(|s| T::from_f32(s * T::max().to_f32()))
                            .collect::<Vec<T>>()
                    }) {
                        log::warn!("Failed to send audio samples to receiver channel: {e}");
                    }
                }
                OutputDestination::Both(file_path, sender) => {
                    let (file_path, sender) = (file_path.clone(), sender.clone());

                    if let Err(e) = self.write_samples_to_file(&file_path, samples) {
                        log::warn!("Failed to write audio to file {:?}: {}", file_path, e);
                    }

                    if let Err(e) = sender.try_send(if T::sample_format() == SampleFormat::Float {
                        samples
                            .into_iter()
                            .map(|s| T::from_f32(*s))
                            .collect::<Vec<T>>()
                    } else {
                        samples
                            .into_iter()
                            .map(|s| T::from_f32(s * T::max().to_f32()))
                            .collect::<Vec<T>>()
                    }) {
                        log::warn!("Failed to send audio samples to receiver channel: {e}");
                    }
                }
            }
        }
    }

    fn write_samples_to_file(
        &mut self,
        file_path: &PathBuf,
        samples: &[f32],
    ) -> Result<(), AudioError> {
        if self.writer.is_none() {
            let output_channels = if self.config.convert_to_mono {
                1
            } else {
                self.max_channels
            };

            // Use 16-bit format for mono output, 32-bit float for stereo
            let spec = if self.config.convert_to_mono {
                hound::WavSpec {
                    channels: output_channels,
                    sample_rate: self.config.target_sample_rate,
                    bits_per_sample: 16,
                    sample_format: SampleFormat::Int,
                }
            } else {
                hound::WavSpec {
                    channels: output_channels,
                    sample_rate: self.config.target_sample_rate,
                    bits_per_sample: T::bits_per_sample(),
                    sample_format: T::sample_format(),
                }
            };
            self.writer = Some(hound::WavWriter::create(file_path, spec)?);
        }

        if let Some(writer) = &mut self.writer {
            // Ensure we write complete frames (multiples of channel count)
            let channels = writer.spec().channels as usize;
            let complete_frames = samples.len() / channels;
            let complete_samples = complete_frames * channels;

            for &sample in &samples[0..complete_samples] {
                if self.config.convert_to_mono {
                    let sample_i16 = (sample * i16::MAX as f32) as i16;
                    writer.write_sample(sample_i16)?;
                } else {
                    match T::sample_format() {
                        SampleFormat::Float => writer.write_sample(sample)?,
                        SampleFormat::Int => {
                            if T::bits_per_sample() == 16 {
                                writer.write_sample((sample * T::max().to_f32()) as i16)?;
                            } else if T::bits_per_sample() == 24 {
                                writer.write_sample((sample * T::max().to_f32()) as i32)?;
                            } else if T::bits_per_sample() == 32 {
                                writer.write_sample((sample * T::max().to_f32()) as i32)?;
                            } else {
                                unreachable!(
                                    "unsupported bits_per_sample: {}",
                                    T::bits_per_sample()
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), AudioError> {
        // Process any remaining samples in buffers
        loop {
            let mut all_processed_tracks = vec![];

            for i in 0..self.specs.len() {
                let spec = &self.specs[i];
                let samples_per_second = spec.sample_rate as usize * spec.channels as usize;
                let samples_to_process = self.buffers[i].len().min(samples_per_second);
                let processed = self.resamples(i, samples_to_process)?;

                if !processed.is_empty() {
                    all_processed_tracks.push(processed);
                }
            }

            if all_processed_tracks.is_empty() {
                break;
            }

            // Unify channel counts before mixing
            let mut unified_tracks = Vec::new();
            for (i, track_samples) in all_processed_tracks.iter().enumerate() {
                let spec = &self.specs[i];
                // After resampling, all tracks should have the same sample rate but may have different channel counts
                // Convert to max_channels for consistent mixing
                if spec.channels == 1 && self.max_channels > 1 {
                    // Convert mono to stereo for mixing
                    unified_tracks.push(mono_to_stereo(track_samples));
                } else {
                    unified_tracks.push(track_samples.clone());
                }
            }

            let mut final_samples = if unified_tracks.len() > 1 {
                self.mix_multiple_tracks(&unified_tracks)
            } else {
                unified_tracks[0].clone()
            };

            // Apply mono conversion after mixing if needed
            if self.config.convert_to_mono && self.max_channels > 1 {
                final_samples = multi_to_mono(&final_samples, self.max_channels);
            }

            if self.specs.len() > 1 {
                // TODO: normalize audio may cause sound unbalanced compare to original sound
                let normalized = normalize_audio(&final_samples);
                self.handle_output(&normalized);
            } else {
                self.handle_output(&final_samples);
            }
        }

        if let Some(writer) = self.writer.take() {
            writer.finalize()?;
        }

        Ok(())
    }
}
