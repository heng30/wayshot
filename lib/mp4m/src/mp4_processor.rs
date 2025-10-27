use crossbeam::channel::{Receiver, Sender, bounded};
use derive_builder::Builder;
use fdk_aac::enc::{BitRate, ChannelMode, Encoder, EncoderParams, Transport};
use h264_reader::{
    annexb::AnnexBReader,
    nal::{Nal, RefNal, UnitType},
    push::NalInterest,
};
use hound::WavSpec;
use mp4::{
    AacConfig, AvcConfig, ChannelConfig, Mp4Config, Mp4Sample, Mp4Writer, SampleFreqIndex,
    TrackConfig, TrackType,
};
use std::{
    fs::File,
    io::{BufWriter, Read},
    path::PathBuf,
};
use thiserror::Error;

const VIDEO_TIMESCALE: u32 = 90000; // Standard video timescale (90kHz) for better compatibility
const DEFAULT_PPS: [u8; 6] = [0x68, 0xeb, 0xe3, 0xcb, 0x22, 0xc0];
const DEFAULT_SPS: [u8; 25] = [
    0x67, 0x64, 0x00, 0x1e, 0xac, 0xd9, 0x40, 0xa0, 0x2f, 0xf9, 0x70, 0x11, 0x00, 0x00, 0x03, 0x03,
    0xe9, 0x00, 0x00, 0xea, 0x60, 0x0f, 0x16, 0x2d, 0x96,
];

pub enum VideoFrameType {
    Frame(Vec<u8>),
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
    audio_receiver: Vec<Receiver<Vec<f32>>>,
    audio_buffer_cache: Vec<Vec<f32>>,
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
            audio_buffer_cache: vec![],
        }
    }

    pub fn h264_sender(&self) -> Sender<VideoFrameType> {
        self.h264_sender.clone()
    }

    pub fn add_audio_track(
        &mut self,
        config: AudioConfig,
    ) -> Result<Sender<Vec<f32>>, Mp4ProcessorError> {
        if config.spec.channels > 2 {
            return Err(Mp4ProcessorError::Mp4(
                "Audio channels is great then 2".to_string(),
            ));
        }

        let (sender, receiver) = bounded(self.config.channel_size);
        self.audio_config.push(config);
        self.audio_receiver.push(receiver);
        self.audio_buffer_cache.push(Vec::new());

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

        // Create MP4 configuration with better browser compatibility
        let mp4_config = Mp4Config {
            major_brand: str::parse("isom").unwrap(),
            minor_version: 512,
            compatible_brands: vec![
                str::parse("isom").unwrap(),
                str::parse("iso2").unwrap(),
                str::parse("avc1").unwrap(),
                str::parse("mp41").unwrap(),
            ],
            timescale: VIDEO_TIMESCALE,
        };

        Mp4Writer::write_start(writer, &mp4_config)
            .map_err(|e| Mp4ProcessorError::Mp4(e.to_string()))
    }

    fn extract_sps_pps_from_headers(
        &self,
        headers_data: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Mp4ProcessorError> {
        let mut sps = None;
        let mut pps = None;

        // Try to parse as Annex B format first
        let mut reader = AnnexBReader::accumulate(|nal: RefNal<'_>| {
            let nal_unit_type = nal.header().unwrap().nal_unit_type();

            // Read all data from the NAL unit
            let mut reader = nal.reader();
            let mut data = Vec::new();
            if let Ok(_) = reader.read_to_end(&mut data) {
                match nal_unit_type {
                    UnitType::SeqParameterSet => {
                        sps = Some(data);
                    }
                    UnitType::PicParameterSet => {
                        pps = Some(data);
                    }
                    _ => {}
                }
            }

            NalInterest::Buffer
        });

        reader.push(headers_data);
        reader.reset();

        // If Annex B parsing failed, try length-prefixed format
        if sps.is_none() || pps.is_none() {
            // log::debug!("Annex B parsing failed, trying length-prefixed format");
            let mut i = 0;
            while i + 4 <= headers_data.len() {
                // Read NAL unit length (big-endian)
                let nal_length = ((headers_data[i] as u32) << 24)
                    | ((headers_data[i + 1] as u32) << 16)
                    | ((headers_data[i + 2] as u32) << 8)
                    | (headers_data[i + 3] as u32);

                if i + 4 + nal_length as usize > headers_data.len() {
                    break;
                }

                let nal_start = i + 4;
                let nal_end = nal_start + nal_length as usize;
                let nal_data = &headers_data[nal_start..nal_end];

                if nal_data.len() > 0 {
                    let nal_unit_type = nal_data[0] & 0x1F;
                    match nal_unit_type {
                        7 => {
                            // SPS
                            sps = Some(nal_data.to_vec());
                        }
                        8 => {
                            // PPS
                            pps = Some(nal_data.to_vec());
                        }
                        _ => {}
                    }
                }

                i += 4 + nal_length as usize;
            }
        }

        match (sps, pps) {
            (Some(sps_data), Some(pps_data)) => {
                log::info!(
                    "Successfully extracted SPS ({} bytes) and PPS ({} bytes) from headers",
                    sps_data.len(),
                    pps_data.len()
                );
                log::debug!(
                    "SPS first 10 bytes: {:02x?}",
                    &sps_data[..sps_data.len().min(10)]
                );
                log::debug!(
                    "PPS first 10 bytes: {:02x?}",
                    &pps_data[..pps_data.len().min(10)]
                );
                Ok((sps_data, pps_data))
            }
            _ => {
                log::warn!("Failed to extract SPS/PPS from headers, using fallback");
                Ok((DEFAULT_SPS.to_vec(), DEFAULT_PPS.to_vec()))
            }
        }
    }

    fn setup_video_track(
        &self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        video_config: &VideoConfig,
        headers_data: Option<&[u8]>,
    ) -> Result<(), Mp4ProcessorError> {
        let (sps, pps) = if let Some(headers) = headers_data {
            self.extract_sps_pps_from_headers(headers)?
        } else {
            (DEFAULT_SPS.to_vec(), DEFAULT_PPS.to_vec())
        };

        let video_track_config = TrackConfig {
            track_type: TrackType::Video,
            timescale: VIDEO_TIMESCALE,
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

    pub fn run_processing_loop(
        &mut self,
        headers_data: Option<Vec<u8>>,
    ) -> Result<(), Mp4ProcessorError> {
        let mut mp4_writer = self.setup_mp4_writer()?;
        self.setup_video_track(
            &mut mp4_writer,
            &self.config.video_config,
            headers_data.as_deref(),
        )?;
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

        const VIDEO_TIMESCALE: u32 = 90000;

        // Calculate duration in 90kHz timescale units (90000 / fps)
        let duration = VIDEO_TIMESCALE / self.config.video_config.fps;

        // Detect if this is a keyframe (I-frame) by checking for SPS/PPS or start code
        let is_sync = self.is_keyframe(&data);

        let sample = Mp4Sample {
            start_time: *video_timestamp,
            duration,
            rendering_offset: 0,
            is_sync, // Only mark keyframes as sync points
            bytes: data.into(),
        };

        if let Err(e) = mp4_writer.write_sample(1, &sample) {
            log::warn!("Write video sample failed: {e}");
        }

        *video_timestamp += duration as u64;
    }

    fn is_keyframe(&self, data: &[u8]) -> bool {
        // Since we're using length-prefixed NAL units (not Annex B),
        // we need to parse the NAL units differently
        let mut i = 0;
        while i + 4 <= data.len() {
            // Read NAL unit length (big-endian)
            let nal_length = ((data[i] as u32) << 24)
                | ((data[i + 1] as u32) << 16)
                | ((data[i + 2] as u32) << 8)
                | (data[i + 3] as u32);

            if i + 4 + nal_length as usize > data.len() {
                break;
            }

            let nal_start = i + 4;
            let nal_end = nal_start + nal_length as usize;
            let nal_data = &data[nal_start..nal_end];

            if nal_data.len() > 0 {
                let nal_unit_type = nal_data[0] & 0x1F;
                // NAL unit types: 5 = IDR frame, 7 = SPS, 8 = PPS
                if nal_unit_type == 5 || nal_unit_type == 7 || nal_unit_type == 8 {
                    return true;
                }
            }

            i += 4 + nal_length as usize;
        }
        false
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
        let config = &self.audio_config[track_index];
        let channels = config.spec.channels as usize;

        // AAC encoder typically expects 1024 samples per channel
        let aac_frame_size = 1024 * channels;

        // Combine cached data with new data
        let mut combined_data = std::mem::take(&mut self.audio_buffer_cache[track_index]);
        combined_data.extend(data);

        // Process data in chunks suitable for AAC encoding
        for chunk_start in (0..combined_data.len()).step_by(aac_frame_size) {
            let chunk_end = (chunk_start + aac_frame_size).min(combined_data.len());
            let chunk = &combined_data[chunk_start..chunk_end];

            // If chunk is too small, cache it for next time
            if chunk.len() < aac_frame_size {
                log::debug!("Caching incomplete audio frame: {} samples", chunk.len());
                self.audio_buffer_cache[track_index] = chunk.to_vec();
                break;
            }

            match self.encode_samples_to_aac(track_index, chunk) {
                Ok(aac_data) => {
                    // log::info!("aac_data len: {} bytes", aac_data.len());

                    let samples_per_channel = chunk.len() / channels;

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
                // log::info!("audio data len: {} bytes", audio_data.len());

                all_ended = false;
                self.process_audio_frame(
                    mp4_writer,
                    audio_track_ids,
                    track_index,
                    audio_timestamps,
                    audio_data_counters,
                    audio_data,
                );
            }
        }
        all_ended
    }

    fn flush_audio_cache(
        &mut self,
        mp4_writer: &mut Mp4Writer<BufWriter<File>>,
        audio_track_ids: &[u32],
        audio_timestamps: &mut Vec<u64>,
        audio_data_counters: &mut Vec<u64>,
    ) {
        for track_index in 0..self.audio_buffer_cache.len() {
            if !self.audio_buffer_cache[track_index].is_empty() {
                // log::info!(
                //     "Flushing cached audio data for track {}: {} samples",
                //     track_index,
                //     self.audio_buffer_cache[track_index].len()
                // );

                // Process the remaining cached data
                let cached_data = std::mem::take(&mut self.audio_buffer_cache[track_index]);
                self.process_audio_frame(
                    mp4_writer,
                    audio_track_ids,
                    track_index,
                    audio_timestamps,
                    audio_data_counters,
                    cached_data,
                );
            }
        }
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
                        // Flush any remaining cached audio data before breaking
                        self.flush_audio_cache(
                            mp4_writer,
                            &audio_track_ids,
                            audio_timestamps,
                            audio_data_counters,
                        );
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
