use crossbeam::channel::{Receiver, Sender, bounded};
use derive_builder::Builder;
use fdk_aac::enc::{BitRate, ChannelMode, Encoder, EncoderParams, Transport};
use hound::WavSpec;
use mp4::{
    AacConfig, AvcConfig, ChannelConfig, Mp4Config, Mp4Sample, Mp4Writer, SampleFreqIndex,
    TrackConfig, TrackType,
};
use std::{fs::File, io::BufWriter, path::PathBuf};
use thiserror::Error;

pub enum VideoFrameType {
    Frame(Vec<u8>),
    End,
}

pub enum AudioFrameType {
    Frame(Vec<f32>),
    End,
}

#[derive(Error, Debug)]
pub enum Mp4ProcessorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("MP4 muxing error: {0}")]
    Mp4(String),
    #[error("AAC encoding error: {0}")]
    AacEncoding(String),
}

#[derive(Builder, Clone)]
pub struct VideoConfig {
    #[builder(default = "1920")]
    pub width: u32,

    #[builder(default = "1080")]
    pub height: u32,

    #[builder(default = "25")]
    pub fps: u32,
}

#[derive(Builder)]
pub struct AudioConfig {
    #[builder(default = "false")]
    pub convert_to_mono: bool,

    pub spec: WavSpec,
}

#[derive(Builder)]
pub struct Mp4ProcessorConfig {
    pub save_path: PathBuf,

    pub video_config: VideoConfig,

    #[builder(default = "1024")]
    pub channel_size: usize,
}

pub struct Mp4Processor {
    config: Mp4ProcessorConfig,
    h264_sender: Sender<VideoFrameType>,
    h264_receiver: Receiver<VideoFrameType>,
    total_video_frames: u64,

    aac_encoder: Vec<Encoder>,
    audio_config: Vec<AudioConfig>,
    audio_receiver: Vec<Receiver<AudioFrameType>>,
}

impl Mp4Processor {
    pub fn new(config: Mp4ProcessorConfig) -> Self {
        let (h264_sender, h264_receiver) = bounded(config.channel_size);

        Self {
            config,
            h264_sender,
            h264_receiver,
            total_video_frames: 0,
            aac_encoder: vec![],
            audio_config: vec![],
            audio_receiver: vec![],
        }
    }

    pub fn h264_sender(&self) -> Sender<VideoFrameType> {
        self.h264_sender.clone()
    }

    pub fn add_audio_track(
        &mut self,
        config: AudioConfig,
    ) -> Result<Sender<AudioFrameType>, Mp4ProcessorError> {
        if config.spec.channels > 2 {
            return Err(Mp4ProcessorError::Mp4(
                "Audio channels is great then 2".to_string(),
            ));
        }

        let (sender, receiver) = bounded(self.config.channel_size);
        self.audio_config.push(config);
        self.audio_receiver.push(receiver);

        // Initialize AAC encoder for this track
        let track_index = self.audio_config.len() - 1;
        let config = &self.audio_config[track_index];

        let channels = if config.convert_to_mono && config.spec.channels == 2 {
            ChannelMode::Mono
        } else {
            match config.spec.channels {
                1 => ChannelMode::Mono,
                2 => ChannelMode::Stereo,
                _ => ChannelMode::Stereo,
            }
        };

        let params = EncoderParams {
            bit_rate: BitRate::Cbr(128000), // Use CBR for more consistent quality
            sample_rate: config.spec.sample_rate,
            channels,
            transport: Transport::Adts,
            audio_object_type: fdk_aac::enc::AudioObjectType::Mpeg4LowComplexity,
        };

        match Encoder::new(params) {
            Ok(encoder) => {
                // Get encoder info to verify configuration
                if let Ok(info) = encoder.info() {
                    log::info!(
                        "AAC encoder initialized for track {}: {} input channels, frameLength: {}, maxAncBytes: {}",
                        track_index,
                        info.inputChannels,
                        info.frameLength,
                        info.maxAncBytes
                    );
                }

                self.aac_encoder.push(encoder);
                Ok(sender)
            }
            Err(e) => Err(Mp4ProcessorError::AacEncoding(e.to_string())),
        }
    }

    fn encode_samples_to_aac(
        &mut self,
        track_index: usize,
        samples: &[f32],
    ) -> Result<Vec<u8>, Mp4ProcessorError> {
        if track_index >= self.aac_encoder.len() {
            return Err(Mp4ProcessorError::AacEncoding(format!(
                "No AAC encoder for track index {}",
                track_index
            )));
        }

        let encoder = &self.aac_encoder[track_index];
        let config = &self.audio_config[track_index];

        // Handle channel conversion if needed
        let processed_samples = if config.convert_to_mono && config.spec.channels == 2 {
            // Convert stereo to mono by averaging left and right channels
            let mut mono_samples = Vec::with_capacity(samples.len() / 2);
            for i in (0..samples.len()).step_by(2) {
                if i + 1 < samples.len() {
                    let left = samples[i];
                    let right = samples[i + 1];
                    mono_samples.push((left + right) * 0.5); // Average the two channels
                }
            }
            mono_samples
        } else {
            samples.to_vec()
        };

        // Convert f32 to i16 for AAC encoding
        let pcm_i16: Vec<i16> = processed_samples
            .iter()
            .map(|&sample| (sample * i16::MAX as f32) as i16)
            .collect();

        // Allocate output buffer with sufficient size for AAC encoding
        // AAC typically compresses to about 1/4 of PCM size, but allocate more for safety
        let mut output_buffer = vec![0u8; pcm_i16.len() * 4];

        match encoder.encode(&pcm_i16, &mut output_buffer) {
            Ok(encode_info) => {
                output_buffer.truncate(encode_info.output_size);
                Ok(output_buffer)
            }
            Err(e) => Err(Mp4ProcessorError::AacEncoding(e.to_string())),
        }
    }

    fn setup_mp4_writer(&self) -> Result<Mp4Writer<BufWriter<File>>, Mp4ProcessorError> {
        let file = File::create(&self.config.save_path).map_err(|e| {
            Mp4ProcessorError::Io(std::io::Error::other(format!(
                "No found `{}`. error: {e}",
                self.config.save_path.display()
            )))
        })?;
        let writer = BufWriter::new(file);

        // Create MP4 configuration
        let mp4_config = Mp4Config {
            major_brand: str::parse("isom").unwrap(),
            minor_version: 512,
            compatible_brands: vec![
                str::parse("isom").unwrap(),
                str::parse("iso2").unwrap(),
                str::parse("avc1").unwrap(),
                str::parse("mp41").unwrap(),
            ],
            timescale: 1000, // 1ms units
        };

        Mp4Writer::write_start(writer, &mp4_config)
            .map_err(|e| Mp4ProcessorError::Mp4(e.to_string()))
    }

    fn setup_video_track(
        &self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        video_config: &VideoConfig,
    ) -> Result<(), Mp4ProcessorError> {
        // Setup video track with minimal SPS/PPS for H.264
        // These are basic parameters that should work for most cases
        let sps = vec![
            0x67, 0x64, 0x00, 0x1e, 0xac, 0xd9, 0x40, 0xa0, 0x2f, 0xf9, 0x70, 0x11, 0x00, 0x00,
            0x03, 0x03, 0xe9, 0x00, 0x00, 0xea, 0x60, 0x0f, 0x16, 0x2d, 0x96,
        ];
        let pps = vec![0x68, 0xeb, 0xe3, 0xcb, 0x22, 0xc0];

        let video_track_config = TrackConfig {
            track_type: TrackType::Video,
            timescale: video_config.fps, // Use fps as timescale for video
            language: "und".to_string(),
            media_conf: mp4::MediaConfig::AvcConfig(AvcConfig {
                width: video_config.width as u16,
                height: video_config.height as u16,
                seq_param_set: sps,
                pic_param_set: pps,
            }),
        };

        mp4_writer
            .add_track(&video_track_config)
            .map_err(|e| Mp4ProcessorError::Mp4(e.to_string()))
    }

    fn setup_audio_tracks(
        &self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
    ) -> Result<Vec<u32>, Mp4ProcessorError> {
        let mut audio_track_ids = Vec::new();

        for (track_index, config) in self.audio_config.iter().enumerate() {
            let freq_index = match config.spec.sample_rate {
                96000 => SampleFreqIndex::Freq96000,
                88200 => SampleFreqIndex::Freq88200,
                64000 => SampleFreqIndex::Freq64000,
                48000 => SampleFreqIndex::Freq48000,
                44100 => SampleFreqIndex::Freq44100,
                32000 => SampleFreqIndex::Freq32000,
                24000 => SampleFreqIndex::Freq24000,
                22050 => SampleFreqIndex::Freq22050,
                16000 => SampleFreqIndex::Freq16000,
                12000 => SampleFreqIndex::Freq12000,
                11025 => SampleFreqIndex::Freq11025,
                8000 => SampleFreqIndex::Freq8000,
                7350 => SampleFreqIndex::Freq7350,
                _ => SampleFreqIndex::Freq44100, // Default to 44100
            };

            let chan_conf = if config.convert_to_mono && config.spec.channels == 2 {
                ChannelConfig::Mono
            } else {
                match config.spec.channels {
                    1 => ChannelConfig::Mono,
                    2 => ChannelConfig::Stereo,
                    3 => ChannelConfig::Three,
                    4 => ChannelConfig::Four,
                    5 => ChannelConfig::Five,
                    6 => ChannelConfig::FiveOne,
                    7 => ChannelConfig::SevenOne,
                    _ => ChannelConfig::Stereo, // Default to stereo
                }
            };

            let audio_config = TrackConfig {
                track_type: TrackType::Audio,
                timescale: config.spec.sample_rate, // Use sample rate as timescale for audio
                language: "und".to_string(),
                media_conf: mp4::MediaConfig::AacConfig(AacConfig {
                    bitrate: 128000, // Default bitrate
                    profile: mp4::AudioObjectType::AacLowComplexity,
                    freq_index,
                    chan_conf,
                }),
            };

            mp4_writer
                .add_track(&audio_config)
                .map_err(|e| Mp4ProcessorError::Mp4(e.to_string()))?;

            // Track IDs start from 1 (video track) and increment for each audio track
            audio_track_ids.push(1 + track_index as u32 + 1);
            log::info!(
                "Audio track {} added with track ID: {}",
                track_index,
                audio_track_ids[track_index]
            );
        }

        Ok(audio_track_ids)
    }

    pub fn run_processing_loop(&mut self) -> Result<(), Mp4ProcessorError> {
        let mut mp4_writer = self.setup_mp4_writer()?;
        self.setup_video_track(&mut mp4_writer, &self.config.video_config)?;
        let audio_track_ids = self.setup_audio_tracks(&mut mp4_writer)?;

        let mut video_timestamp = 0u64;
        let mut audio_timestamps: Vec<u64> = vec![0; self.audio_config.len()];
        let mut audio_data_counters: Vec<u64> = vec![0; self.audio_config.len()];

        self.main_processing_loop(
            &mut mp4_writer,
            audio_track_ids,
            &mut video_timestamp,
            &mut audio_timestamps,
            &mut audio_data_counters,
        )?;

        mp4_writer
            .write_end()
            .map_err(|e| Mp4ProcessorError::Mp4(e.to_string()))?;

        Ok(())
    }

    fn process_video_frame(
        &mut self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        video_timestamp: &mut u64,
        data: Vec<u8>,
    ) {
        self.total_video_frames += 1;

        let sample = Mp4Sample {
            start_time: *video_timestamp,
            duration: 1,
            rendering_offset: 0,
            is_sync: true, // Assume all H.264 frames are sync points
            bytes: data.into(),
        };

        if let Err(e) = mp4_writer.write_sample(1, &sample) {
            log::warn!("Write video sample failed: {e}");
        }

        *video_timestamp += 1;
    }

    fn process_audio_frame(
        &mut self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        audio_track_ids: &[u32],
        track_index: usize,
        audio_timestamps: &mut Vec<u64>,
        audio_data_counters: &mut Vec<u64>,
        data: Vec<f32>,
    ) {
        match self.encode_samples_to_aac(track_index, &data) {
            Ok(aac_data) => {
                let config = &self.audio_config[track_index];
                let samples_per_channel = data.len() / config.spec.channels as usize;

                let sample = Mp4Sample {
                    start_time: audio_timestamps[track_index],
                    duration: samples_per_channel as u32, // Duration in audio timescale units (samples per channel)
                    rendering_offset: 0,
                    is_sync: true,
                    bytes: aac_data.into(),
                };

                if let Err(e) = mp4_writer.write_sample(audio_track_ids[track_index], &sample) {
                    log::warn!("Write audio sample failed for track {}: {e}", track_index);
                }

                audio_timestamps[track_index] += samples_per_channel as u64;
                audio_data_counters[track_index] += 1;
            }
            Err(e) => {
                log::warn!("AAC encoding failed for track {}: {e}", track_index);
            }
        }
    }

    fn process_audio_receivers(
        &mut self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        audio_track_ids: &[u32],
        audio_timestamps: &mut Vec<u64>,
        audio_data_counters: &mut Vec<u64>,
    ) -> bool {
        let mut all_ended = true;
        for track_index in 0..self.audio_receiver.len() {
            if let Ok(audio_data) = self.audio_receiver[track_index].try_recv() {
                match audio_data {
                    AudioFrameType::Frame(data) => {
                        all_ended = false;
                        self.process_audio_frame(
                            mp4_writer,
                            audio_track_ids,
                            track_index,
                            audio_timestamps,
                            audio_data_counters,
                            data,
                        );
                    }
                    AudioFrameType::End => {
                        log::info!("Audio track {} receive `End`", track_index);
                    }
                }
            }
        }
        all_ended
    }

    fn main_processing_loop(
        &mut self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        audio_track_ids: Vec<u32>,
        video_timestamp: &mut u64,
        audio_timestamps: &mut Vec<u64>,
        audio_data_counters: &mut Vec<u64>,
    ) -> Result<(), Mp4ProcessorError> {
        let mut video_ended = false;
        let mut audio_ended = false;

        loop {
            crossbeam::select! {
                recv(self.h264_receiver) -> video_frame => {
                    match video_frame {
                        Ok(frame_data) => match frame_data {
                            VideoFrameType::Frame(data) => {
                                self.process_video_frame(mp4_writer, video_timestamp, data);
                            },
                            VideoFrameType::End => {
                                log::info!("h264_receiver receive `End`");
                                video_ended = true;
                            }
                        }
                        Err(e) =>  {
                            log::info!("h264_receiver exit: {e}");
                            video_ended = true;
                        }
                    }
                }
                default => {
                    let all_ended = self.process_audio_receivers(
                        mp4_writer,
                        &audio_track_ids,
                        audio_timestamps,
                        audio_data_counters,
                    );

                    if all_ended {
                        audio_ended = true;
                    }

                    if video_ended && audio_ended && self.h264_receiver.is_empty() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
