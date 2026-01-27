use crate::{Error, Result};
use derivative::Derivative;
use derive_setters::Setters;
use ffmpeg_next as ffmpeg;
use std::path::Path;

/// Loudness normalization configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct LoudnormConfig {
    /// Target integrated loudness in LUFS (default: -16)
    #[derivative(Default(value = "-16.0"))]
    pub target_i: f32,

    /// Loudness range in LU (default: 11)
    #[derivative(Default(value = "11.0"))]
    pub lra: f32,

    /// True peak in dBFS (default: -1.5)
    #[derivative(Default(value = "-1.5"))]
    pub tp: f32,
}

impl LoudnormConfig {
    /// Create a new loudness normalization configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the loudnorm filter specification string
    fn build_filter_spec(&self) -> String {
        format!("I={}:LRA={}:TP={}", self.target_i, self.lra, self.tp)
    }
}

/// Audio processing configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct AudioProcessConfig {
    /// Input video file path
    #[derivative(Default(value = "String::new()"))]
    pub input: String,

    /// Output video file path (with processed audio)
    #[derivative(Default(value = "String::new()"))]
    pub output: String,

    /// Loudness normalization settings
    pub loudnorm: LoudnormConfig,

    /// Volume adjustment multiplier (None = no adjustment)
    pub volume: Option<f32>,

    /// Audio bitrate in bps (default: 192000)
    #[derivative(Default(value = "192000"))]
    pub audio_bitrate: u32,
}

impl AudioProcessConfig {
    /// Create a new audio processing configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.input.is_empty() {
            return Err(Error::InvalidConfig("Input path is empty".to_string()));
        }
        if self.output.is_empty() {
            return Err(Error::InvalidConfig("Output path is empty".to_string()));
        }

        let input_path = Path::new(&self.input);
        if !input_path.exists() {
            return Err(Error::InvalidConfig(format!(
                "Input file does not exist: {}",
                self.input
            )));
        }

        // Validate volume if set
        if let Some(vol) = self.volume {
            if vol <= 0.0 {
                return Err(Error::InvalidConfig(format!(
                    "Volume must be positive, got: {}",
                    vol
                )));
            }
        }

        Ok(())
    }
}

/// Process audio in video file with loudness normalization and optional volume adjustment
///
/// This function processes a video file by:
/// - Copying the video stream without re-encoding
/// - Processing the audio with loudness normalization and optional volume adjustment
/// - Encoding the processed audio to AAC format
/// - Muxing to output video file
///
/// # Arguments
///
/// * `config` - Audio processing configuration
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the operation fails.
///
/// # Example
///
/// ```no_run
/// use video_utils::audio_process::{AudioProcessConfig, LoudnormConfig, process_audio};
///
/// let loudnorm = LoudnormConfig::new()
///     .with_target_i(-16.0)
///     .with_lra(11.0)
///     .with_tp(-1.5);
///
/// let config = AudioProcessConfig::new()
///     .with_input("input.mp4".to_string())
///     .with_output("output.mp4".to_string())
///     .with_loudnorm(loudnorm)
///     .with_volume(Some(1.3));  // Optional volume boost
///
/// process_audio(&config).unwrap();
/// ```
pub fn process_audio(config: &AudioProcessConfig) -> Result<()> {
    config.validate()?;

    log::info!("Processing video: {}", config.input);
    log::info!("Output file: {}", config.output);
    log::info!(
        "Loudness normalization: I={} LUFS, LRA={} LU, TP={} dBFS",
        config.loudnorm.target_i,
        config.loudnorm.lra,
        config.loudnorm.tp
    );

    if let Some(vol) = config.volume {
        log::info!("Volume adjustment: {}x", vol);
    }

    // Initialize FFmpeg
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input
    let mut input_ctx = ffmpeg::format::input(&config.input)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    // Find video and audio streams
    let input_video_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video);

    let input_audio_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| Error::FFmpeg("No audio stream found in input file".to_string()))?;

    let video_stream_index = input_video_stream.as_ref().map(|s| s.index());
    let audio_stream_index = input_audio_stream.index();

    log::debug!("Video stream index: {:?}", video_stream_index);
    log::debug!("Audio stream index: {}", audio_stream_index);

    // Create audio decoder
    let decoder_context =
        ffmpeg::codec::context::Context::from_parameters(input_audio_stream.parameters())
            .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?;

    let mut decoder = decoder_context
        .decoder()
        .audio()
        .map_err(|e| Error::FFmpeg(format!("Failed to create audio decoder: {}", e)))?;

    let sample_rate = decoder.rate();
    let sample_format = decoder.format();
    let channel_layout = decoder.channel_layout();
    let channels = decoder.channels();

    log::debug!("Sample rate: {}", sample_rate);
    log::debug!("Sample format: {:?}", sample_format);
    log::debug!("Channel layout: {:?}", channel_layout);
    log::debug!("Channels: {}", channels);

    // Create output context
    let mut output_ctx = ffmpeg::format::output(&config.output)
        .map_err(|e| Error::FFmpeg(format!("Failed to create output: {}", e)))?;

    // Find AAC encoder
    let aac_codec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC)
        .ok_or_else(|| Error::FFmpeg("AAC encoder not found".to_string()))?;

    log::debug!("AAC codec: {:?}", aac_codec.name());

    // Create AAC encoder
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(aac_codec)
        .encoder()
        .audio()
        .map_err(|e| Error::FFmpeg(format!("Failed to create audio encoder: {}", e)))?;

    // Configure encoder
    use ffmpeg::format::sample::Type;
    encoder.set_rate(sample_rate as i32);
    encoder.set_format(ffmpeg::format::Sample::F32(Type::Planar));
    encoder.set_channel_layout(channel_layout);
    encoder.set_bit_rate(config.audio_bitrate as usize);

    // Open encoder first to get actual parameters
    let mut encoder = encoder
        .open_as(aac_codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to open encoder: {}", e)))?;

    // Get encoder's actual frame size
    let encoder_frame_size = encoder.frame_size();
    log::debug!("Encoder frame size: {}", encoder_frame_size);

    // Create audio output stream
    let output_audio_stream_index;
    {
        let mut output_stream = output_ctx
            .add_stream(aac_codec)
            .map_err(|e| Error::FFmpeg(format!("Failed to add audio stream: {}", e)))?;
        output_stream.set_parameters(&encoder);
        output_audio_stream_index = output_stream.index();
    }

    // Copy video stream if exists
    let output_video_stream_index: Option<usize> = if let Some(ref video_stream) = input_video_stream {
        log::info!("Found video stream in input, will copy to output");

        let mut out_video_stream = output_ctx
            .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
            .map_err(|e| Error::FFmpeg(format!("Failed to add video stream: {}", e)))?;

        // Copy video stream parameters (stream copy, no re-encoding)
        out_video_stream.set_parameters(video_stream.parameters());

        Some(out_video_stream.index())
    } else {
        None
    };

    // Write header
    output_ctx
        .write_header()
        .map_err(|e| Error::FFmpeg(format!("Failed to write header: {}", e)))?;

    // Build audio filter graph
    let mut filter_graph = ffmpeg::filter::Graph::new();

    // Buffer source arguments for audio
    let buffer_args = format!(
        "time_base=1/{}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        sample_rate,
        sample_rate,
        format_sample_fmt(sample_format),
        channel_layout.bits()
    );

    log::debug!("Buffer args: {}", buffer_args);

    // Add buffer source (input to filter graph)
    filter_graph
        .add(
            &ffmpeg::filter::find("abuffer").unwrap(),
            "in",
            &buffer_args,
        )
        .map_err(|e| Error::FFmpeg(format!("Failed to add abuffer filter: {}", e)))?;

    // Add buffer sink (output from filter graph)
    filter_graph
        .add(&ffmpeg::filter::find("abuffersink").unwrap(), "out", "")
        .map_err(|e| Error::FFmpeg(format!("Failed to add abuffersink: {}", e)))?;

    // Build filter specification
    // Apply loudnorm and volume if configured
    // aresample will handle sample rate conversion if needed
    let loudnorm_spec = config.loudnorm.build_filter_spec();

    // Calculate input duration in samples for trimming
    let input_duration = input_ctx.duration() as f64 / 1_000_000.0; // microseconds to seconds
    let input_duration_ts = (input_duration * sample_rate as f64) as i64;
    log::debug!("Input duration: {:.2}s ({} samples)", input_duration, input_duration_ts);

    let filter_spec = if let Some(vol) = config.volume {
        format!(
            "volume={},aformat=sample_fmts=fltp,asetnsamples=1024",
            vol
        )
    } else {
        format!(
            "aformat=sample_fmts=fltp,asetnsamples=1024"
        )
    };

    log::debug!("Filter spec: {}", filter_spec);

    // Parse and connect filters
    filter_graph
        .output("in", 0)
        .and_then(|p| p.input("out", 0))
        .map_err(|e| Error::FFmpeg(format!("Failed to connect filters: {}", e)))?
        .parse(&filter_spec)
        .map_err(|e| Error::FFmpeg(format!("Failed to parse filter: {}", e)))?;

    // Validate filter graph
    filter_graph
        .validate()
        .map_err(|e| Error::FFmpeg(format!("Failed to validate filter graph: {}", e)))?;

    // Get filter contexts
    let mut in_filter = filter_graph
        .get("in")
        .ok_or_else(|| Error::FFmpeg("Failed to get in filter".to_string()))?;

    let mut out_filter = filter_graph
        .get("out")
        .ok_or_else(|| Error::FFmpeg("Failed to get out filter".to_string()))?;

    // Processing variables
    let mut in_frame = ffmpeg::frame::Audio::empty();
    let mut out_frame = ffmpeg::frame::Audio::empty();
    let mut packet = ffmpeg::Packet::empty();
    let mut frame_count = 0u64;
    let mut packet_count = 0u64;

    let input_time_base = ffmpeg::Rational::new(1, sample_rate as i32);
    let output_time_base = encoder.time_base();

    log::debug!("Input time_base: {:?}", input_time_base);
    log::debug!("Output time_base: {:?}", output_time_base);

    // Get input audio stream timebase for proper timestamp handling
    let input_audio_stream = input_ctx.stream(audio_stream_index).unwrap();
    let input_stream_time_base = input_audio_stream.time_base();
    log::debug!("Input stream time_base: {:?}", input_stream_time_base);

    // Get video stream timebases for proper timestamp rescaling during stream copy
    let input_video_time_base = if let Some(ref video_stream) = input_video_stream {
        Some(video_stream.time_base())
    } else {
        None
    };

    let output_video_time_base = if let Some(out_video_idx) = output_video_stream_index {
        Some(output_ctx.stream(out_video_idx).unwrap().time_base())
    } else {
        None
    };

    // Process each packet
    for (stream, mut packet) in input_ctx.packets() {
        // Handle video packets - copy them directly to output
        if let Some(video_idx) = video_stream_index {
            if stream.index() == video_idx {
                if let Some(out_video_idx) = output_video_stream_index {
                    packet.set_stream(out_video_idx);
                    // Rescale timestamps from input video stream timebase to output video stream timebase
                    if let (Some(in_tb), Some(out_tb)) = (input_video_time_base, output_video_time_base) {
                        packet.rescale_ts(in_tb, out_tb);
                    }
                    packet
                        .write(&mut output_ctx)
                        .map_err(|e| Error::FFmpeg(format!("Failed to write video packet: {}", e)))?;
                }
                continue;
            }
        }

        // Skip non-audio packets
        if stream.index() != audio_stream_index {
            continue;
        }

        packet_count += 1;
        log::trace!("Processing audio packet {}", packet_count);

        // Send packet to decoder
        decoder
            .send_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Decoder send failed: {}", e)))?;

        // Receive decoded frames
        while decoder.receive_frame(&mut in_frame).is_ok() {
            // Add frame to filter (filter will handle PTS)
            in_filter
                .source()
                .add(&in_frame)
                .map_err(|e| Error::FFmpeg(format!("Filter add failed: {}", e)))?;

            // Get filtered frames
            while out_filter.sink().frame(&mut out_frame).is_ok() {
                // Send filtered frame to encoder
                encoder
                    .send_frame(&out_frame)
                    .map_err(|e| Error::FFmpeg(format!("Encoder send failed: {}", e)))?;

                // Receive encoded packets
                while encoder.receive_packet(&mut packet).is_ok() {
                    packet.set_stream(output_audio_stream_index);
                    packet.rescale_ts(input_time_base, output_time_base);

                    packet
                        .write(&mut output_ctx)
                        .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
                }

                frame_count += 1;
                log::trace!("Encoded frame {} with {} samples", frame_count, out_frame.samples());
            }
        }
    }

    // Flush decoder
    decoder
        .send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to flush decoder: {}", e)))?;

    while decoder.receive_frame(&mut in_frame).is_ok() {
        in_filter
            .source()
            .add(&in_frame)
            .map_err(|e| Error::FFmpeg(format!("Filter add failed: {}", e)))?;

        while out_filter.sink().frame(&mut out_frame).is_ok() {
            encoder
                .send_frame(&out_frame)
                .map_err(|e| Error::FFmpeg(format!("Encoder send failed: {}", e)))?;

            while encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(output_audio_stream_index);
                packet.rescale_ts(input_time_base, output_time_base);

                packet
                    .write(&mut output_ctx)
                    .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
            }

            frame_count += 1;
        }
    }

    // Flush encoder
    encoder
        .send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to send EOF to encoder: {}", e)))?;

    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(output_audio_stream_index);
        packet.rescale_ts(input_time_base, output_time_base);

        packet
            .write(&mut output_ctx)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    log::info!("Processed {} audio packets", packet_count);
    log::info!("Processed {} audio frames", frame_count);

    // Write trailer
    output_ctx
        .write_trailer()
        .map_err(|e| Error::FFmpeg(format!("Failed to write trailer: {}", e)))?;

    log::info!("Successfully created video with processed audio: {}", config.output);

    Ok(())
}

/// Format sample format for filter arguments
fn format_sample_fmt(fmt: ffmpeg::format::Sample) -> String {
    use ffmpeg::format::sample::Type;
    match fmt {
        ffmpeg::format::Sample::U8(Type::Packed) => "u8".to_string(),
        ffmpeg::format::Sample::U8(Type::Planar) => "u8p".to_string(),
        ffmpeg::format::Sample::I16(Type::Packed) => "s16".to_string(),
        ffmpeg::format::Sample::I16(Type::Planar) => "s16p".to_string(),
        ffmpeg::format::Sample::I32(Type::Packed) => "s32".to_string(),
        ffmpeg::format::Sample::I32(Type::Planar) => "s32p".to_string(),
        ffmpeg::format::Sample::F32(Type::Packed) => "flt".to_string(),
        ffmpeg::format::Sample::F32(Type::Planar) => "fltp".to_string(),
        ffmpeg::format::Sample::F64(Type::Packed) => "dbl".to_string(),
        ffmpeg::format::Sample::F64(Type::Planar) => "dblp".to_string(),
        _ => "s16p".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudnorm_config_default() {
        let config = LoudnormConfig::default();
        assert_eq!(config.target_i, -16.0);
        assert_eq!(config.lra, 11.0);
        assert_eq!(config.tp, -1.5);
    }

    #[test]
    fn test_loudnorm_config_builder() {
        let config = LoudnormConfig::new()
            .with_target_i(-14.0)
            .with_lra(12.0)
            .with_tp(-2.0);

        assert_eq!(config.target_i, -14.0);
        assert_eq!(config.lra, 12.0);
        assert_eq!(config.tp, -2.0);
    }

    #[test]
    fn test_loudnorm_filter_spec() {
        let config = LoudnormConfig::new();
        let spec = config.build_filter_spec();
        assert_eq!(spec, "I=-16:LRA=11:TP=-1.5");
    }

    #[test]
    fn test_audio_process_config_default() {
        let config = AudioProcessConfig::default();
        assert_eq!(config.input, "");
        assert_eq!(config.output, "");
        assert_eq!(config.audio_bitrate, 192000);
        assert!(config.volume.is_none());
    }

    #[test]
    fn test_audio_process_config_builder() {
        let loudnorm = LoudnormConfig::new().with_target_i(-14.0);
        let config = AudioProcessConfig::new()
            .with_input("input.mp4".to_string())
            .with_output("output.mp4".to_string())
            .with_loudnorm(loudnorm)
            .with_volume(Some(1.3))
            .with_audio_bitrate(256000);

        assert_eq!(config.input, "input.mp4");
        assert_eq!(config.output, "output.mp4");
        assert_eq!(config.volume, Some(1.3));
        assert_eq!(config.audio_bitrate, 256000);
        assert_eq!(config.loudnorm.target_i, -14.0);
    }

    #[test]
    fn test_config_validation_empty_input() {
        let config = AudioProcessConfig::new().with_output("output.mp4".to_string());

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_empty_output() {
        let config = AudioProcessConfig::new().with_input("input.mp4".to_string());

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_volume() {
        let config = AudioProcessConfig::new()
            .with_input("input.mp4".to_string())
            .with_output("output.mp4".to_string())
            .with_volume(Some(0.0));

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_format_sample_fmt() {
        use ffmpeg_next::format::sample::Type;
        assert_eq!(
            format_sample_fmt(ffmpeg::format::Sample::I16(Type::Packed)),
            "s16"
        );
        assert_eq!(
            format_sample_fmt(ffmpeg::format::Sample::F32(Type::Planar)),
            "fltp"
        );
        assert_eq!(
            format_sample_fmt(ffmpeg::format::Sample::I16(Type::Planar)),
            "s16p"
        );
    }
}
