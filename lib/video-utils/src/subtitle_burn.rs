use crate::{Error, Result};
use derivative::Derivative;
use derive_setters::Setters;
use ffmpeg_next as ffmpeg;
use std::path::Path;

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct SubtitleStyle {
    #[derivative(Default(value = "24"))]
    pub font_size: u32,

    /// Font name (e.g., "Arial", "sans-serif")
    #[derivative(Default(value = "Some(\"Arial\".to_string())"))]
    pub font_name: Option<String>,

    /// Primary text color in AABBGGRR format (e.g., "&H00FFFFFF" for white)
    #[derivative(Default(value = "Some(\"&H00FFFFFF\".to_string())"))]
    pub primary_color: Option<String>,

    /// Outline color in AABBGGRR format (e.g., "&H00000000" for black)
    #[derivative(Default(value = "Some(\"&H00000000\".to_string())"))]
    pub outline_color: Option<String>,

    /// Background color in AABBGGRR format (e.g., "&H80000000" for semi-transparent black)
    pub background_color: Option<String>,

    /// Outline width in pixels (default: 2)
    #[derivative(Default(value = "Some(2)"))]
    pub outline_width: Option<u32>,

    /// Border style: 0=outline+shadow, 1=outline only, 3=opaque box
    #[derivative(Default(value = "Some(1)"))]
    pub border_style: Option<u32>,

    /// Text alignment: 1-9 (1=bottom-left, 2=bottom-center, 3=bottom-right, etc.)
    #[derivative(Default(value = "Some(2)"))]
    pub alignment: Option<u32>,

    /// Vertical margin from aligned position in pixels
    #[derivative(Default(value = "Some(30)"))]
    pub margin_vertical: Option<u32>,

    /// Horizontal margin from left in pixels
    pub margin_left: Option<u32>,

    /// Horizontal margin from right in pixels
    pub margin_right: Option<u32>,

    /// Whether to use bold text (-1=true, 0=false)
    #[derivative(Default(value = "Some(0)"))]
    pub bold: Option<i32>,

    /// Whether to use italic text (-1=true, 0=false)
    #[derivative(Default(value = "Some(0)"))]
    pub italic: Option<i32>,

    /// Whether to use underline (-1=true, 0=false)
    #[derivative(Default(value = "Some(0)"))]
    pub underline: Option<i32>,

    /// Background border radius in pixels (for rounded corners)
    #[derivative(Default(value = "Some(0)"))]
    pub border_radius: Option<u32>,

    /// Text padding within background box in pixels
    #[derivative(Default(value = "Some(4)"))]
    pub padding: Option<u32>,
}

impl SubtitleStyle {
    /// Create a new subtitle style with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set bold text (converts bool to i32)
    pub fn with_bold_bool(mut self, bold: bool) -> Self {
        self.bold = Some(if bold { -1 } else { 0 });
        self
    }

    /// Set italic text (converts bool to i32)
    pub fn with_italic_bool(mut self, italic: bool) -> Self {
        self.italic = Some(if italic { -1 } else { 0 });
        self
    }

    /// Set underline (converts bool to i32)
    pub fn with_underline_bool(mut self, underline: bool) -> Self {
        self.underline = Some(if underline { -1 } else { 0 });
        self
    }

    /// Build the force_style string for FFmpeg
    fn build_force_style(&self) -> String {
        let mut parts = vec![
            format!("FontSize={}", self.font_size),
            format!("Fontname={}", self.font_name.as_deref().unwrap_or("Arial")),
            format!(
                "PrimaryColour={}",
                self.primary_color.as_deref().unwrap_or("&H00FFFFFF")
            ),
            format!(
                "OutlineColour={}",
                self.outline_color.as_deref().unwrap_or("&H00000000")
            ),
            format!("Outline={}", self.outline_width.unwrap_or(2)),
            format!("BorderStyle={}", self.border_style.unwrap_or(1)),
            format!("Alignment={}", self.alignment.unwrap_or(2)),
            format!("MarginV={}", self.margin_vertical.unwrap_or(30)),
            format!("Bold={}", self.bold.unwrap_or(0)),
            format!("Italic={}", self.italic.unwrap_or(0)),
            format!("Underline={}", self.underline.unwrap_or(0)),
        ];

        if let Some(bg_color) = &self.background_color {
            parts.push(format!("BackColour={}", bg_color));
        }
        if let Some(margin_l) = self.margin_left {
            parts.push(format!("MarginL={}", margin_l));
        }
        if let Some(margin_r) = self.margin_right {
            parts.push(format!("MarginR={}", margin_r));
        }
        if let Some(border_radius) = self.border_radius
            && border_radius > 0 {
                parts.push(format!("BorderRadius={}", border_radius));
            }
        if let Some(padding) = self.padding {
            parts.push(format!("Padding={}", padding));
        }

        parts.join(",")
    }
}

/// Configuration for adding subtitles to video
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct SubtitleBurnConfig {
    /// Path to input video
    #[derivative(Default(value = "String::new()"))]
    pub input: String,

    /// Path to subtitle file (SRT, ASS, etc.)
    #[derivative(Default(value = "String::new()"))]
    pub subtitle: String,

    /// Path to output video
    #[derivative(Default(value = "String::new()"))]
    pub output: String,

    /// Subtitle styling options
    pub style: SubtitleStyle,
}

impl SubtitleBurnConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.input.is_empty() {
            return Err(Error::InvalidConfig("Input path is empty".to_string()));
        }
        if self.subtitle.is_empty() {
            return Err(Error::InvalidConfig("Subtitle path is empty".to_string()));
        }
        if self.output.is_empty() {
            return Err(Error::InvalidConfig("Output path is empty".to_string()));
        }

        let input_path = Path::new(&self.input);
        let subtitle_path = Path::new(&self.subtitle);

        if !input_path.exists() {
            return Err(Error::InvalidConfig(format!(
                "Input file does not exist: {}",
                self.input
            )));
        }
        if !subtitle_path.exists() {
            return Err(Error::InvalidConfig(format!(
                "Subtitle file does not exist: {}",
                self.subtitle
            )));
        }

        Ok(())
    }
}

/// Add subtitles to a video file with custom styling using ffmpeg-next library
///
/// This implementation uses FFmpeg's filter graph through the ffmpeg-next library.
/// It burns subtitles into the video permanently.
///
/// # Arguments
///
/// * `config` - Configuration for subtitle burning
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the operation fails.
///
/// # Example
///
/// ```no_run
/// use video_utils::subtitle_burn::{SubtitleBurnConfig, SubtitleStyle, add_subtitles};
///
/// let style = SubtitleStyle::new()
///     .with_font_size(28)
///     .with_primary_color(Some("&H00FFFFFF".to_string()))
///     .with_alignment(Some(2));
///
/// let config = SubtitleBurnConfig::new()
///     .with_input("input.mp4".to_string())
///     .with_subtitle("subtitles.srt".to_string())
///     .with_output("output.mp4".to_string())
///     .with_style(style);
///
/// add_subtitles(&config).unwrap();
/// ```
pub fn add_subtitles(config: &SubtitleBurnConfig) -> Result<()> {
    config.validate()?;

    log::info!("Adding subtitles to video: {}", config.input);
    log::info!("Subtitle file: {}", config.subtitle);
    log::info!("Output file: {}", config.output);

    // Initialize FFmpeg
    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input
    let mut input_ctx = ffmpeg::format::input(&config.input)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    // Find video stream
    let input_video_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found".to_string()))?;

    let video_stream_index = input_video_stream.index();

    // Find audio stream if exists
    let input_audio_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Audio);

    let audio_stream_index = input_audio_stream.as_ref().map(|s| s.index());

    // Create decoder
    let decoder_context =
        ffmpeg::codec::context::Context::from_parameters(input_video_stream.parameters())
            .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?;

    let mut decoder = decoder_context
        .decoder()
        .video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    // Create output context
    let mut output_ctx = ffmpeg::format::output(&config.output)
        .map_err(|e| Error::FFmpeg(format!("Failed to create output: {}", e)))?;

    // Create filter graph
    let mut filter_graph = ffmpeg::filter::Graph::new();

    // Buffer source arguments
    let pixel_format = decoder.format();
    let time_base = input_video_stream.time_base();
    let aspect_ratio = decoder.aspect_ratio();

    let buffer_args = format!(
        "video_size={}x{}:pix_fmt={}:time_base={}:pixel_aspect={}",
        decoder.width(),
        decoder.height(),
        pixel_format.descriptor().unwrap().name(),
        time_base,
        aspect_ratio
    );

    log::debug!("Buffer args: {}", buffer_args);

    // Add buffer source (input to filter graph)
    filter_graph
        .add(&ffmpeg::filter::find("buffer").unwrap(), "in", &buffer_args)
        .map_err(|e| Error::FFmpeg(format!("Failed to add buffer filter: {}", e)))?;

    // Add buffer sink (output from filter graph)
    filter_graph
        .add(&ffmpeg::filter::find("buffersink").unwrap(), "out", "")
        .map_err(|e| Error::FFmpeg(format!("Failed to add buffersink: {}", e)))?;

    // Get subtitle path and build filter spec
    let subtitle_path = Path::new(&config.subtitle);
    let subtitle_path_str = subtitle_path
        .canonicalize()
        .map_err(|_| Error::InvalidConfig("Invalid subtitle path".to_string()))?
        .to_str()
        .ok_or_else(|| Error::InvalidConfig("Invalid subtitle path".to_string()))?
        .to_string();

    // Build force_style string
    let force_style = config.style.build_force_style();
    log::debug!("Force style: {}", force_style);

    // Escape the subtitle path for FFmpeg filter
    let subtitle_path_escaped = subtitle_path_str
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'");

    // Build filter specification
    let filter_spec = format!(
        "subtitles='{}':force_style='{}'",
        subtitle_path_escaped, force_style
    );

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

    // Setup encoder
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or_else(|| Error::FFmpeg("H.264 encoder not found".to_string()))?;

    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create encoder: {}", e)))?;

    // Configure encoder
    encoder.set_width(decoder.width());
    encoder.set_height(decoder.height());
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);

    let decoder_framerate = decoder.frame_rate();
    log::debug!("Decoder frame rate: {:?}", decoder_framerate);

    encoder.set_frame_rate(decoder_framerate);
    encoder.set_time_base(time_base);

    // Open encoder
    let mut encoder = encoder
        .open_as(codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to open encoder: {}", e)))?;

    // Now create all output streams
    let output_stream_index;
    {
        let mut output_stream = output_ctx
            .add_stream(codec)
            .map_err(|e| Error::FFmpeg(format!("Failed to add output stream: {}", e)))?;
        output_stream.set_parameters(&encoder);
        output_stream_index = output_stream.index();
    } // output_stream borrow ends here

    // Setup audio stream if input has audio
    let mut output_audio_stream_index: Option<usize> = None;
    if let Some(ref audio_stream) = input_audio_stream {
        log::info!("Found audio stream in input, will copy to output");

        // Create output audio stream (stream copy, no re-encoding)
        let mut out_audio_stream = output_ctx
            .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
            .map_err(|e| Error::FFmpeg(format!("Failed to add audio stream: {}", e)))?;

        // Set audio stream parameters from input
        out_audio_stream.set_parameters(audio_stream.parameters());

        output_audio_stream_index = Some(out_audio_stream.index());
    }

    // Set the output stream time_base to match the encoder's time_base
    // The MP4 muxer will use this for timestamp calculations
    let encoder_time_base = encoder.time_base();
    let output_stream_time_base;
    {
        let mut output_stream = output_ctx.stream_mut(output_stream_index)
            .ok_or_else(|| Error::FFmpeg("Failed to get output stream".to_string()))?;
        output_stream.set_time_base(encoder_time_base);
        output_stream_time_base = output_stream.time_base();
    }

    // Write header
    output_ctx
        .write_header()
        .map_err(|e| Error::FFmpeg(format!("Failed to write header: {}", e)))?;

    // Processing variables
    let (mut in_frame, mut out_frame) = (ffmpeg::frame::Video::empty(), ffmpeg::frame::Video::empty());

    let mut packet = ffmpeg::Packet::empty();
    let mut frame_count = 0;
    let frame_rate = decoder.frame_rate().unwrap_or(ffmpeg::Rational::new(30, 1));
    let mut last_pts: Option<i64> = None;

    // Process each packet
    for (stream, mut packet) in input_ctx.packets() {
        // Handle audio packets - copy them directly to output
        if let Some(audio_idx) = audio_stream_index
            && stream.index() == audio_idx {
                if let Some(out_audio_idx) = output_audio_stream_index {
                    packet.set_stream(out_audio_idx);
                    packet
                        .write(&mut output_ctx)
                        .map_err(|e| Error::FFmpeg(format!("Failed to write audio packet: {}", e)))?;
                }
                continue;
            }

        // Skip non-video, non-audio packets (subtitles, etc.)
        if stream.index() != video_stream_index {
            continue;
        }

        // Send packet to decoder
        decoder
            .send_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Decoder send failed: {}", e)))?;

        // Receive decoded frames
        while decoder.receive_frame(&mut in_frame).is_ok() {
            // Only set PTS if the decoder didn't provide one
            if in_frame.pts().is_none() {
                let pts = (frame_count as i64) * time_base.denominator() as i64 / (time_base.numerator() as i64 * frame_rate.numerator() as i64);
                in_frame.set_pts(Some(pts));
                frame_count += 1;
            }

            // Add frame to filter
            in_filter
                .source()
                .add(&in_frame)
                .map_err(|e| Error::FFmpeg(format!("Filter add failed: {}", e)))?;

            // Get filtered frames
            while out_filter.sink().frame(&mut out_frame).is_ok() {
                // Store frame PTS before sending to encoder
                let frame_pts = out_frame.pts();

                // Send filtered frame to encoder
                encoder
                    .send_frame(&out_frame)
                    .map_err(|e| Error::FFmpeg(format!("Encoder send failed: {}", e)))?;

                // Receive encoded packets
                while encoder.receive_packet(&mut packet).is_ok() {
                    packet.set_stream(output_stream_index);

                    // If encoder didn't set PTS, use frame's PTS (in filter time_base)
                    if packet.pts().is_none()
                        && let Some(pts) = frame_pts {
                            packet.set_pts(Some(pts));
                            // Also set DTS for simple encoding (no B-frames)
                            if packet.dts().is_none() {
                                packet.set_dts(Some(pts));
                            }
                        }

                    // Rescale timestamps to output stream time_base
                    packet.rescale_ts(time_base, output_stream_time_base);

                    // Track last PTS for duration calculation
                    if let Some(pts) = packet.pts() {
                        last_pts = Some(pts);
                    }

                    packet
                        .write(&mut output_ctx)
                        .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
                }
            }
        }
    }

    // Flush decoder
    decoder
        .send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to flush decoder: {}", e)))?;

    while decoder.receive_frame(&mut in_frame).is_ok() {
        // Only set PTS if the decoder didn't provide one
        if in_frame.pts().is_none() {
            let pts = (frame_count as i64) * time_base.denominator() as i64 / (time_base.numerator() as i64 * frame_rate.numerator() as i64);
            in_frame.set_pts(Some(pts));
            frame_count += 1;
        }

        in_filter
            .source()
            .add(&in_frame)
            .map_err(|e| Error::FFmpeg(format!("Filter add failed: {}", e)))?;

        while out_filter.sink().frame(&mut out_frame).is_ok() {
            encoder
                .send_frame(&out_frame)
                .map_err(|e| Error::FFmpeg(format!("Encoder send failed: {}", e)))?;

            while encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(output_stream_index);
                packet.rescale_ts(time_base, output_stream_time_base);

                packet
                    .write(&mut output_ctx)
                    .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
            }
        }
    }

    // Flush filter
    in_filter
        .source()
        .flush()
        .map_err(|e| Error::FFmpeg(format!("Failed to flush filter: {}", e)))?;

    while out_filter.sink().frame(&mut out_frame).is_ok() {
        encoder
            .send_frame(&out_frame)
            .map_err(|e| Error::FFmpeg(format!("Encoder send failed: {}", e)))?;

        while encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(output_stream_index);
            packet.rescale_ts(time_base, output_stream_time_base);

            // Track last PTS for duration calculation
            if let Some(pts) = packet.pts() {
                last_pts = Some(pts);
            }

            packet
                .write(&mut output_ctx)
                .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
        }
    }

    // Flush encoder
    encoder
        .send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to send EOF to encoder: {}", e)))?;

    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(output_stream_index);
        packet.rescale_ts(time_base, output_stream_time_base);

        packet
            .write(&mut output_ctx)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    // Log final statistics
    log::info!("Last PTS written: {:?}", last_pts);
    if let Some(pts) = last_pts {
        let duration_seconds = pts as f64 * output_stream_time_base.numerator() as f64 / output_stream_time_base.denominator() as f64;
        log::info!("Estimated duration: {:.2} seconds", duration_seconds);
    }

    // Write trailer
    output_ctx
        .write_trailer()
        .map_err(|e| Error::FFmpeg(format!("Failed to write trailer: {}", e)))?;

    log::info!(
        "Successfully created subtitle-burned video: {}",
        config.output
    );

    Ok(())
}

/// Helper function to convert RGB color to FFmpeg AABBGGRR format
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
/// * `a` - Alpha component (0-255, 0=transparent, 255=opaque)
///
/// # Example
///
/// ```
/// use video_utils::subtitle_burn::rgb_to_ass_color;
///
/// // White with full opacity
/// let white = rgb_to_ass_color(255, 255, 255, 255);
/// assert_eq!(white, "&HFFFFFFFF");
///
/// // White with zero opacity (transparent)
/// let white_transparent = rgb_to_ass_color(255, 255, 255, 0);
/// assert_eq!(white_transparent, "&H00FFFFFF");
///
/// // Black with 50% opacity
/// let black_half = rgb_to_ass_color(0, 0, 0, 128);
/// assert_eq!(black_half, "&H80000000");
/// ```
pub fn rgb_to_ass_color(r: u8, g: u8, b: u8, a: u8) -> String {
    format!("&H{:02X}{:02X}{:02X}{:02X}", a, b, g, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtitle_style_default() {
        let style = SubtitleStyle::default();
        assert_eq!(style.font_size, 24);
        assert_eq!(style.alignment, Some(2));
        assert_eq!(style.margin_vertical, Some(30));
    }

    #[test]
    fn test_subtitle_style_builder() {
        let style = SubtitleStyle::new()
            .with_font_size(32)
            .with_bold_bool(true)
            .with_alignment(Some(8));

        assert_eq!(style.font_size, 32);
        assert_eq!(style.bold, Some(-1));
        assert_eq!(style.alignment, Some(8));
    }

    #[test]
    fn test_rgb_to_ass_color() {
        assert_eq!(rgb_to_ass_color(255, 255, 255, 0), "&H00FFFFFF");
        assert_eq!(rgb_to_ass_color(0, 0, 0, 128), "&H80000000");
        assert_eq!(rgb_to_ass_color(255, 0, 0, 255), "&HFF0000FF");
    }

    #[test]
    fn test_force_style_building() {
        let style = SubtitleStyle::new()
            .with_font_size(28)
            .with_primary_color(Some("&H00FFFFFF".to_string()))
            .with_alignment(Some(2));

        let force_style = style.build_force_style();
        assert!(force_style.contains("FontSize=28"));
        assert!(force_style.contains("PrimaryColour=&H00FFFFFF"));
        assert!(force_style.contains("Alignment=2"));
    }

    #[test]
    fn test_config_validation() {
        let config = SubtitleBurnConfig::new()
            .with_input("".to_string())
            .with_subtitle("".to_string());

        assert!(config.validate().is_err());
    }
}
