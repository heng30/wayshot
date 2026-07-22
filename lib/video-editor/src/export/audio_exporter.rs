use super::progress::{CancellationToken, ExportPhase, ExportProgress};
use crate::{
    Error, Result,
    tracks::{
        audio_track::{AudioSamples, AudioSource, UnifiedAudioTracksMixerIterator},
        manager::Manager,
        track::Track,
    },
};
use fdk_aac::enc::{AudioObjectType, BitRate, ChannelMode, EncoderParams, Transport};
use flac_codec::encode::{FlacSampleWriter, Options};
use hound::{SampleFormat, WavWriter};
use mp3lame_encoder::{Builder, FlushGap, InterleavedPcm};
use std::{
    fs::File,
    io::{BufWriter, Write},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioExportFormat {
    #[default]
    Aac,
    Mp3,
    Wav,
    Flac,
    Ogg,
}

impl AudioExportFormat {
    pub fn extension(&self) -> &str {
        match self {
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
        }
    }

    pub fn mime_type(&self) -> &str {
        match self {
            Self::Aac => "audio/aac",
            Self::Mp3 => "audio/mpeg",
            Self::Wav => "audio/wav",
            Self::Flac => "audio/flac",
            Self::Ogg => "audio/ogg",
        }
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct AudioExportConfig {
    #[derivative(Default(value = "PathBuf::from(\"audio\")"))]
    pub output_path: PathBuf,

    pub cancellation_token: Option<CancellationToken>,

    #[derivative(Default(value = "AudioExportFormat::Aac"))]
    pub format: AudioExportFormat,

    #[derivative(Default(value = "192_000"))]
    pub bitrate: u32,

    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AudioExportResult {
    pub output_path: PathBuf,
    pub duration: Duration,
    pub channels: u16,
    pub sample_rate: u32,
    pub total_samples: usize,
}

pub struct AudioExporter {
    manager: Arc<Manager>,
    config: AudioExportConfig,
}

impl AudioExporter {
    pub fn new(manager: Arc<Manager>, config: AudioExportConfig) -> Self {
        Self { manager, config }
    }

    pub fn export(&self) -> Result<AudioExportResult> {
        self.export_with_progress(|_| {})
    }

    pub fn collect_audio_samples_direct(&self) -> Result<AudioSamples> {
        let (channels, sample_rate) = self.detect_audio_params()?;
        self.collect_audio_samples(channels, sample_rate)
    }

    fn check_cancelled(&self) -> Result<()> {
        if let Some(token) = &self.config.cancellation_token
            && token.is_cancelled()
        {
            return Err(Error::ExportCancelled);
        }
        Ok(())
    }

    pub fn export_with_progress<F>(&self, mut progress_fn: F) -> Result<AudioExportResult>
    where
        F: FnMut(ExportProgress),
    {
        progress_fn(ExportProgress {
            current_position: Duration::ZERO,
            total_duration: self.manager.duration,
            frames_processed: 0,
            total_frames: 0,
            phase: ExportPhase::Initializing,
        });

        let (channels, sample_rate) = self.detect_audio_params()?;
        let samples = self.collect_audio_samples(channels, sample_rate)?;

        let total_duration = Duration::from_secs_f64(
            (samples.samples.len() as f64) / (channels as f64 * sample_rate as f64),
        );

        progress_fn(ExportProgress {
            current_position: Duration::ZERO,
            total_duration,
            frames_processed: 0,
            total_frames: samples.samples.len() as u64 / channels as u64,
            phase: ExportPhase::ProcessingAudio,
        });

        let mut output_path = self.config.output_path.clone();
        output_path.set_extension(self.config.format.extension());

        match self.config.format {
            AudioExportFormat::Aac => {
                self.export_aac(&output_path, &samples, channels, sample_rate)?
            }
            AudioExportFormat::Mp3 => {
                self.export_mp3(&output_path, &samples, channels, sample_rate)?
            }
            AudioExportFormat::Wav => {
                self.export_wav(&output_path, &samples, channels, sample_rate)?
            }
            AudioExportFormat::Flac => {
                self.export_flac(&output_path, &samples, channels, sample_rate)?
            }
            AudioExportFormat::Ogg => {
                self.export_ogg(&output_path, &samples, channels, sample_rate)?
            }
        }

        progress_fn(ExportProgress {
            current_position: total_duration,
            total_duration,
            frames_processed: samples.samples.len() as u64 / channels as u64,
            total_frames: samples.samples.len() as u64 / channels as u64,
            phase: ExportPhase::Complete,
        });

        log::info!(
            "Exported audio to {} ({} ch, {} Hz, {:.2}s)",
            output_path.display(),
            channels,
            sample_rate,
            total_duration.as_secs_f64()
        );

        Ok(AudioExportResult {
            output_path,
            duration: total_duration,
            channels,
            sample_rate,
            total_samples: samples.samples.len() / channels as usize,
        })
    }

    fn detect_audio_params(&self) -> Result<(u16, u32)> {
        if let (Some(ch), Some(sr)) = (self.config.channels, self.config.sample_rate) {
            return Ok((ch, sr));
        }

        for track in &self.manager.tracks {
            if let Track::Audio(audio_track) = track
                && !audio_track.hiding
                && let Some(audio_meta) = audio_track.track.metadata.audios.first()
            {
                let ch = self.config.channels.unwrap_or(audio_meta.channels);
                let sr = self.config.sample_rate.unwrap_or(audio_meta.sample_rate);
                return Ok((ch, sr));
            }

            if let Track::Video(video_track) = track
                && !video_track.hiding
                && video_track.has_audio_in_any_segment()
                && let Some(audio_meta) = video_track.first_audio_meta()
            {
                let ch = self.config.channels.unwrap_or(audio_meta.channels);
                let sr = self.config.sample_rate.unwrap_or(audio_meta.sample_rate);
                return Ok((ch, sr));
            }
        }

        Ok((
            self.config.channels.unwrap_or(2),
            self.config.sample_rate.unwrap_or(48000),
        ))
    }

    pub fn collect_audio_samples(&self, channels: u16, sample_rate: u32) -> Result<AudioSamples> {
        self.collect_audio_samples_with_progress(channels, sample_rate, |_| {})
    }

    pub fn collect_audio_samples_with_progress<F>(
        &self,
        channels: u16,
        sample_rate: u32,
        mut progress_fn: F,
    ) -> Result<AudioSamples>
    where
        F: FnMut(f32),
    {
        let audio_sources: Vec<AudioSource> = self
            .manager
            .tracks
            .iter()
            .filter_map(|track| match track {
                Track::Audio(audio_track) if !audio_track.hiding => {
                    Some(AudioSource::Audio(Arc::clone(audio_track)))
                }
                Track::Video(video_track)
                    if !video_track.hiding && video_track.has_audio_in_any_segment() =>
                {
                    Some(AudioSource::VideoWithAudio(Arc::clone(video_track)))
                }
                _ => None,
            })
            .collect();

        if audio_sources.is_empty() {
            return Err(Error::InvalidConfig("No audio sources found".to_string()));
        }

        let request_samples_duration = Duration::from_millis(100);
        let cache_duration = Duration::from_secs(5);

        let mixer = UnifiedAudioTracksMixerIterator::new(
            audio_sources,
            Duration::ZERO,
            cache_duration,
            Duration::from_secs(60),
            channels,
            sample_rate,
            request_samples_duration,
        )?;

        let total_samples =
            self.manager.duration.as_secs() as usize * sample_rate as usize * channels as usize;

        let mut all_samples = Vec::new();
        for audio_data in mixer {
            self.check_cancelled()?;
            all_samples.extend_from_slice(&audio_data.samples);

            let progress = if total_samples > 0 {
                all_samples.len() as f32 / total_samples as f32
            } else {
                0.0
            };
            progress_fn(progress.min(1.0));
        }

        progress_fn(1.0);

        Ok(AudioSamples {
            samples: all_samples,
            sample_rate,
            channels,
        })
    }

    fn export_aac(
        &self,
        path: &Path,
        samples: &AudioSamples,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let channel_mode = if channels == 2 {
            ChannelMode::Stereo
        } else {
            ChannelMode::Mono
        };

        let bitrate_per_channel = self.config.bitrate / channels as u32;

        let params = EncoderParams {
            bit_rate: BitRate::Cbr(bitrate_per_channel),
            sample_rate,
            transport: Transport::Adts, // ADTS for standalone AAC files
            channels: channel_mode,
            audio_object_type: AudioObjectType::Mpeg4LowComplexity,
        };

        let encoder = fdk_aac::enc::Encoder::new(params)
            .map_err(|e| Error::InvalidConfig(format!("Failed to create AAC encoder: {:?}", e)))?;

        let info = encoder
            .info()
            .map_err(|e| Error::InvalidConfig(format!("Failed to get encoder info: {:?}", e)))?;

        let samples_per_frame = info.frameLength as usize * channels as usize;
        let output_buffer_size = info.maxOutBufBytes as usize;

        let mut file = BufWriter::new(File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?);

        let mut input_buffer = Vec::new();
        let mut output_buffer = vec![0u8; output_buffer_size];

        // Convert f32 to i16 for encoder
        let i16_samples: Vec<i16> = samples
            .samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        for chunk in i16_samples.chunks(samples_per_frame) {
            self.check_cancelled()?;
            if chunk.len() < samples_per_frame {
                // Pad the last chunk
                input_buffer.extend_from_slice(chunk);
                input_buffer.extend(std::iter::repeat(0i16).take(samples_per_frame - chunk.len()));
            } else {
                input_buffer.extend_from_slice(chunk);
            }

            match encoder.encode(&input_buffer, &mut output_buffer) {
                Ok(info) if info.output_size > 0 => {
                    file.write_all(&output_buffer[..info.output_size])?;
                }
                Ok(_) => {}
                Err(e) => log::warn!("AAC encoding error: {:?}", e),
            }

            input_buffer.clear();
        }

        // Flush encoder - continue until no more output
        loop {
            self.check_cancelled()?;
            match encoder.encode(&[], &mut output_buffer) {
                Ok(info) if info.output_size > 0 => {
                    file.write_all(&output_buffer[..info.output_size])?;
                }
                Ok(_) => break,
                Err(e) => {
                    log::warn!("AAC flush error: {:?}", e);
                    break;
                }
            }
        }

        file.flush()?;
        Ok(())
    }

    fn export_mp3(
        &self,
        path: &Path,
        samples: &AudioSamples,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let bitrate_kbps = (self.config.bitrate / 1000) as usize;

        let mp3_bitrate = match bitrate_kbps {
            0..=8 => mp3lame_encoder::Bitrate::Kbps8,
            9..=16 => mp3lame_encoder::Bitrate::Kbps16,
            17..=24 => mp3lame_encoder::Bitrate::Kbps24,
            25..=32 => mp3lame_encoder::Bitrate::Kbps32,
            33..=40 => mp3lame_encoder::Bitrate::Kbps40,
            41..=48 => mp3lame_encoder::Bitrate::Kbps48,
            49..=64 => mp3lame_encoder::Bitrate::Kbps64,
            65..=80 => mp3lame_encoder::Bitrate::Kbps80,
            81..=96 => mp3lame_encoder::Bitrate::Kbps96,
            97..=112 => mp3lame_encoder::Bitrate::Kbps112,
            113..=128 => mp3lame_encoder::Bitrate::Kbps128,
            129..=160 => mp3lame_encoder::Bitrate::Kbps160,
            161..=192 => mp3lame_encoder::Bitrate::Kbps192,
            193..=224 => mp3lame_encoder::Bitrate::Kbps224,
            225..=256 => mp3lame_encoder::Bitrate::Kbps256,
            257..=320 => mp3lame_encoder::Bitrate::Kbps320,
            _ => mp3lame_encoder::Bitrate::Kbps192,
        };

        let mut encoder = Builder::new()
            .ok_or_else(|| {
                Error::InvalidConfig("Failed to create MP3 encoder builder".to_string())
            })?
            .with_num_channels(channels as u8)
            .map_err(|e| Error::InvalidConfig(format!("Failed to set channels: {}", e)))?
            .with_sample_rate(sample_rate)
            .map_err(|e| Error::InvalidConfig(format!("Failed to set sample rate: {}", e)))?
            .with_brate(mp3_bitrate)
            .map_err(|e| Error::InvalidConfig(format!("Failed to set bitrate: {}", e)))?
            .with_quality(mp3lame_encoder::Quality::Good)
            .map_err(|e| Error::InvalidConfig(format!("Failed to set quality: {}", e)))?
            .build()
            .map_err(|e| Error::InvalidConfig(format!("Failed to build MP3 encoder: {}", e)))?;

        let file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;
        let mut writer = BufWriter::new(file);
        let mut mp3_buffer = Vec::new();

        // Convert f32 samples to i16 for MP3 encoder
        let i16_samples: Vec<i16> = samples
            .samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        // Encode in chunks (1152 is the typical MP3 frame size)
        let frame_size = 1152 * channels as usize;
        // Only encode complete frames to avoid mp3lame-encoder panic
        let complete_frames = (i16_samples.len() / frame_size) * frame_size;

        for chunk in i16_samples[..complete_frames].chunks(frame_size) {
            self.check_cancelled()?;
            // Reserve buffer space for this chunk
            mp3_buffer.clear();
            mp3_buffer.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len()));

            let input = InterleavedPcm(chunk);
            let encoded_size = encoder
                .encode(input, mp3_buffer.spare_capacity_mut())
                .map_err(|e| Error::InvalidConfig(format!("MP3 encoding error: {}", e)))?;

            // SAFETY: encode() guarantees the bytes written are valid
            unsafe {
                mp3_buffer.set_len(mp3_buffer.len().saturating_add(encoded_size));
            }

            writer.write_all(&mp3_buffer).map_err(|e| {
                Error::IO(std::io::Error::new(
                    e.kind(),
                    format!("Failed to write MP3 data: {}", e),
                ))
            })?;
        }

        // Handle partial frame at the end by padding with zeros
        if i16_samples.len() > complete_frames {
            self.check_cancelled()?;
            let remaining = &i16_samples[complete_frames..];
            if !remaining.is_empty() {
                // Pad to complete frame size
                let mut padded_chunk = Vec::with_capacity(frame_size);
                padded_chunk.extend_from_slice(remaining);
                padded_chunk.resize(frame_size, 0);

                mp3_buffer.clear();
                mp3_buffer.reserve(mp3lame_encoder::max_required_buffer_size(
                    padded_chunk.len(),
                ));

                let input = InterleavedPcm(&padded_chunk);
                let encoded_size = encoder
                    .encode(input, mp3_buffer.spare_capacity_mut())
                    .map_err(|e| Error::InvalidConfig(format!("MP3 encoding error: {}", e)))?;

                unsafe {
                    mp3_buffer.set_len(mp3_buffer.len().saturating_add(encoded_size));
                }

                writer.write_all(&mp3_buffer).map_err(|e| {
                    Error::IO(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write MP3 data: {}", e),
                    ))
                })?;
            }
        }

        // Flush encoder to write final frames
        loop {
            self.check_cancelled()?;
            mp3_buffer.clear();
            mp3_buffer.reserve(7200); // Standard MP3 frame size

            let encoded_size = encoder
                .flush::<FlushGap>(mp3_buffer.spare_capacity_mut())
                .map_err(|e| Error::InvalidConfig(format!("MP3 flush error: {}", e)))?;

            if encoded_size == 0 {
                break;
            }

            // SAFETY: flush() guarantees the bytes written are valid
            unsafe {
                mp3_buffer.set_len(mp3_buffer.len().saturating_add(encoded_size));
            }

            writer.write_all(&mp3_buffer).map_err(|e| {
                Error::IO(std::io::Error::new(
                    e.kind(),
                    format!("Failed to write final MP3 data: {}", e),
                ))
            })?;
        }

        writer.flush()?;
        Ok(())
    }

    fn export_wav(
        &self,
        path: &Path,
        samples: &AudioSamples,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };

        let mut writer = WavWriter::create(path, spec).map_err(|e| {
            Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create WAV file {}: {}", path.display(), e),
            ))
        })?;

        for &sample in &samples.samples {
            self.check_cancelled()?;
            let clamped = sample.clamp(-1.0, 1.0);

            writer.write_sample(clamped).map_err(|e| {
                Error::IO(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to write WAV sample: {}", e),
                ))
            })?;
        }

        writer.finalize().map_err(|e| {
            Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to finalize WAV file: {}", e),
            ))
        })?;

        Ok(())
    }

    fn export_flac(
        &self,
        path: &Path,
        samples: &AudioSamples,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;

        let writer = BufWriter::new(file);
        let options = Options::default();
        let mut encoder = FlacSampleWriter::new(
            writer,
            options,
            sample_rate,
            24, // bits_per_sample
            channels as u8,
            Some(samples.samples.len() as u64),
        )
        .map_err(|e| Error::InvalidConfig(format!("Failed to create FLAC encoder: {}", e)))?;

        // Convert f32 samples to i24 (stored in i32)
        // FLAC expects interleaved samples: [left0, right0, left1, right1, ...]
        let i24_samples: Vec<i32> = samples
            .samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                // Convert to 24-bit PCM (stored in i32)
                (clamped * (i32::MAX >> 8) as f32) as i32
            })
            .collect();

        self.check_cancelled()?;

        encoder.write(&i24_samples).map_err(|e| {
            Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write FLAC samples: {}", e),
            ))
        })?;

        encoder.finalize().map_err(|e| {
            Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to finalize FLAC file: {}", e),
            ))
        })?;

        Ok(())
    }

    fn export_ogg(
        &self,
        path: &Path,
        samples: &AudioSamples,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;
        let writer = BufWriter::new(file);

        let nz_sample_rate = NonZeroU32::new(sample_rate)
            .ok_or_else(|| Error::InvalidConfig("Invalid sample rate".to_string()))?;
        let nz_channels = std::num::NonZeroU8::new(channels as u8)
            .ok_or_else(|| Error::InvalidConfig("Invalid channel count".to_string()))?;

        let nz_bitrate = NonZeroU32::new(self.config.bitrate)
            .ok_or_else(|| Error::InvalidConfig("Invalid bitrate".to_string()))?;
        let bitrate_strategy = VorbisBitrateManagementStrategy::Vbr {
            target_bitrate: nz_bitrate,
        };

        let mut encoder = VorbisEncoderBuilder::new_with_serial(
            nz_sample_rate,
            nz_channels,
            writer,
            12345, // Stream serial number
        )
        .bitrate_management_strategy(bitrate_strategy)
        .build()
        .map_err(|e| Error::InvalidConfig(format!("Failed to create Vorbis encoder: {}", e)))?;

        // Convert interleaved f32 samples to planar format
        // vorbis_rs expects planar format: Vec<Vec<f32>> where each inner vec is a channel
        let samples_per_channel = samples.samples.len() / channels as usize;
        let mut planar_samples: Vec<Vec<f32>> = (0..channels as usize)
            .map(|_| Vec::with_capacity(samples_per_channel))
            .collect();

        for (i, sample) in samples.samples.iter().enumerate() {
            let channel = i % channels as usize;
            planar_samples[channel].push(*sample);
        }

        // Encode audio in blocks. 1024 samples per channel per block is a reasonable choice
        let block_size = 1024;
        let total_blocks = (samples_per_channel + block_size - 1) / block_size;
        let mut block_count = 0;

        for block_idx in 0..total_blocks {
            self.check_cancelled()?;
            let start = block_idx * block_size;
            let end = (start + block_size).min(samples_per_channel);

            // Extract a block for each channel
            let audio_block: Vec<Vec<f32>> = planar_samples
                .iter()
                .map(|channel_samples| {
                    if end <= channel_samples.len() {
                        channel_samples[start..end].to_vec()
                    } else {
                        // Pad the last block with zeros
                        let mut block = channel_samples[start..].to_vec();
                        block.resize(end - start, 0.0);
                        block
                    }
                })
                .collect();

            encoder
                .encode_audio_block(&audio_block)
                .map_err(|e| Error::InvalidConfig(format!("Vorbis encoding error: {}", e)))?;

            block_count += 1;
        }

        // Finish the encoder (writes final data and flushes)
        encoder
            .finish()
            .map_err(|e| Error::InvalidConfig(format!("Vorbis finish error: {}", e)))?;

        log::info!("Encoded {} Vorbis blocks", block_count);

        Ok(())
    }
}
