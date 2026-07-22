use crate::{Error, Result, ensure_file_exists};
pub use ffmpeg_next::{
    self as ffmpeg,
    codec::Id,
    format::{Pixel, Sample},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

// This value (~24.8 days) is effectively unlimited for video editing purposes.
const SAFE_MAX_DURATION: Duration = Duration::from_secs(2_147_483);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum MetadataType {
    Video,
    Audio,
    Subtitle,
    Image,
    None,
}

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub index: usize,
    pub codec_id: ffmpeg::codec::Id,
    pub pix_fmt: ffmpeg::format::Pixel,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub language: Option<String>,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub index: usize,
    pub codec_id: ffmpeg::codec::Id,
    pub sample_format: ffmpeg::format::Sample,
    pub sample_rate: u32,
    pub channels: u16,
    pub language: Option<String>,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct SubtitleMetadata {
    pub index: usize,
    pub codec_id: ffmpeg::codec::Id,
    pub language: Option<String>,
    pub duration: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub path: PathBuf,
    pub size: u64,
    pub bitrate: u64,
    pub duration: Duration,
    pub format: Vec<String>,

    pub videos: Vec<VideoMetadata>,
    pub audios: Vec<AudioMetadata>,
    pub subtitles: Vec<SubtitleMetadata>,
}

impl Metadata {
    pub fn first_audio(&self) -> Option<&AudioMetadata> {
        self.audios.first()
    }

    pub fn first_video(&self) -> Option<&VideoMetadata> {
        self.videos.first()
    }

    pub fn get_audio(&self, index: usize) -> Option<&AudioMetadata> {
        self.audios.get(index)
    }

    pub fn get_video(&self, index: usize) -> Option<&VideoMetadata> {
        self.videos.get(index)
    }

    pub fn get_type(&self) -> MetadataType {
        if !self.videos.is_empty() {
            if self.videos.first().unwrap().duration.is_zero() {
                MetadataType::Image
            } else {
                MetadataType::Video
            }
        } else if !self.audios.is_empty() {
            MetadataType::Audio
        } else if !self.subtitles.is_empty() {
            MetadataType::Subtitle
        } else {
            MetadataType::None
        }
    }

    pub fn is_image(&self) -> bool {
        self.get_type() == MetadataType::Image
    }

    pub fn is_text(&self) -> bool {
        self.path.to_string_lossy().starts_with("text://")
    }

    pub fn is_subtitle(&self) -> bool {
        self.get_type() == MetadataType::Subtitle
    }

    pub fn new_text() -> Self {
        Self {
            duration: SAFE_MAX_DURATION,
            path: PathBuf::from(format!("text://{}", uuid::Uuid::new_v4())),
            ..Default::default()
        }
    }

    pub fn new_subtitle() -> Self {
        Self {
            duration: SAFE_MAX_DURATION,
            subtitles: vec![SubtitleMetadata {
                index: 0,
                codec_id: ffmpeg::codec::Id::None,
                language: None,
                duration: SAFE_MAX_DURATION,
            }],
            ..Default::default()
        }
    }

    // Returns true for types that don't have time constraints from source files.
    // Image and Subtitle segments can be freely positioned and stretched.
    pub fn is_time_independent(&self) -> bool {
        self.is_image() || self.is_subtitle() || self.is_text()
    }
}

pub fn get_metadata<P: AsRef<Path>>(path: P) -> Result<Metadata> {
    let path = path.as_ref();
    ensure_file_exists!(path);

    // SVG files cannot be opened by FFmpeg. Create synthetic Image metadata.
    if path
        .extension()
        .map(|e| e.to_ascii_lowercase() == "svg")
        .unwrap_or(false)
    {
        return get_svg_metadata(path);
    }

    // LRC files cannot be opened by FFmpeg. Create synthetic Subtitle metadata.
    if path
        .extension()
        .map(|e| e.to_ascii_lowercase() == "lrc")
        .unwrap_or(false)
    {
        return get_lrc_metadata(path);
    }

    let size = std::fs::metadata(path)?.len();
    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;
    let input_ctx = ffmpeg::format::input(&path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    let format_name = input_ctx.format().name().to_string();
    let format: Vec<String> = format_name
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let bitrate = input_ctx.bit_rate() as u64;
    let duration_micros = input_ctx.duration();
    let mut duration = Duration::from_micros(if duration_micros > 0 {
        duration_micros as u64
    } else {
        0
    });

    let mut videos: Vec<VideoMetadata> = input_ctx
        .streams()
        .filter(|stream| stream.parameters().medium() == ffmpeg::media::Type::Video)
        .filter_map(|stream| {
            let codec_par = stream.parameters();
            let codec_id = codec_par.id();
            let index = stream.index();

            let decoder_context =
                ffmpeg::codec::context::Context::from_parameters(codec_par).ok()?;
            let decoder = decoder_context.decoder().video().ok()?;

            let fps_rational = stream.avg_frame_rate();
            let fps = if fps_rational.numerator() > 0 && fps_rational.denominator() > 0 {
                fps_rational.numerator() as f32 / fps_rational.denominator() as f32
            } else {
                0.0
            };

            let video_duration_ts = stream.duration();
            let video_time_base = stream.time_base();
            let video_duration = if video_duration_ts > 0 && video_time_base.denominator() > 0 {
                let secs = video_duration_ts as f64 * video_time_base.numerator() as f64
                    / video_time_base.denominator() as f64;
                Duration::from_secs_f64(secs)
            } else {
                duration
            };

            Some(VideoMetadata {
                index,
                codec_id,
                pix_fmt: decoder.format(),
                width: decoder.width(),
                height: decoder.height(),
                fps,
                language: stream.metadata().get("language").map(|s| s.to_string()),
                duration: video_duration,
            })
        })
        .collect();

    // Treat single-frame videos (e.g., images converted by ffmpeg) as static images.
    // Such videos typically have duration ≈ 1/fps with only 1 frame.
    for video in &mut videos {
        if video.fps > 0.0 && !video.duration.is_zero() {
            let frame_count = video.duration.as_secs_f64() * video.fps as f64;
            if frame_count <= 1.5 {
                video.duration = Duration::ZERO;
            }
        }
    }

    // For static images, the metadata.duration should be zero since images
    // don't have inherent time duration. This ensures downstream logic
    // (like default image duration in track.rs) works correctly.
    // Images are identified by: VideoMetadata.duration.is_zero() for all video streams.
    let is_webp = path
        .extension()
        .map(|e| e.to_ascii_lowercase() == "webp")
        .unwrap_or(false);

    if is_webp {
        // FFmpeg cannot decode animated WebP (skips ANIM/ANMF chunks).
        // Use webpx (libwebp) to detect animation regardless of FFmpeg's behavior.
        if let Ok(mut animated_meta) = get_animated_webp_metadata(path) {
            // Single-frame WebP should be treated as a static image.
            if animated_meta.fps > 0.0 {
                let frame_count = animated_meta.duration.as_secs_f64() * animated_meta.fps as f64;
                if frame_count <= 1.5 {
                    animated_meta.duration = Duration::ZERO;
                }
            }
            if !animated_meta.duration.is_zero() {
                duration = animated_meta.duration;
                videos = vec![animated_meta];
            } else {
                // Single-frame WebP — static image
                duration = Duration::ZERO;
            }
        } else {
            // webpx failed — if FFmpeg also returned nothing or static images, treat as static
            if videos.iter().all(|v| v.duration.is_zero()) {
                duration = Duration::ZERO;
            }
        }
    } else if !videos.is_empty() && videos.iter().all(|v| v.duration.is_zero()) {
        // Non-WebP static image
        duration = Duration::ZERO;
    }

    let audios: Vec<AudioMetadata> = input_ctx
        .streams()
        .filter(|stream| stream.parameters().medium() == ffmpeg::media::Type::Audio)
        .filter_map(|stream| {
            let codec_par = stream.parameters();
            let codec_id = codec_par.id();
            let index = stream.index();

            let decoder_context =
                ffmpeg::codec::context::Context::from_parameters(codec_par).ok()?;
            let decoder = decoder_context.decoder().audio().ok()?;

            let audio_duration_ts = stream.duration();
            let audio_time_base = stream.time_base();
            let audio_duration = if audio_duration_ts > 0 && audio_time_base.denominator() > 0 {
                let secs = audio_duration_ts as f64 * audio_time_base.numerator() as f64
                    / audio_time_base.denominator() as f64;
                Duration::from_secs_f64(secs)
            } else {
                duration
            };

            Some(AudioMetadata {
                index,
                codec_id,
                sample_format: decoder.format(),
                sample_rate: decoder.rate(),
                channels: decoder.channels(),
                language: stream.metadata().get("language").map(|s| s.to_string()),
                duration: audio_duration,
            })
        })
        .collect();

    let subtitle_streams: Vec<_> = input_ctx
        .streams()
        .filter(|stream| stream.parameters().medium() == ffmpeg::media::Type::Subtitle)
        .collect();

    // 对于纯字幕文件，需要解析字幕包来获取实际时长
    let is_subtitle_only = videos.is_empty() && audios.is_empty() && !subtitle_streams.is_empty();
    let subtitle_file_duration = if is_subtitle_only {
        subtitle_streams
            .first()
            .and_then(|stream| {
                let stream_index = stream.index();
                ffmpeg::format::input(&path).ok().and_then(|mut ctx| {
                    let dur = get_subtitle_duration_from_packets(&mut ctx, stream_index);
                    if dur > Duration::ZERO {
                        Some(dur)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(Duration::ZERO)
    } else {
        Duration::ZERO
    };

    // 对于纯字幕文件，使用解析的时长
    let final_duration = if is_subtitle_only && subtitle_file_duration > Duration::ZERO {
        subtitle_file_duration
    } else {
        duration
    };

    let subtitles: Vec<SubtitleMetadata> = subtitle_streams
        .into_iter()
        .map(|stream| {
            let codec_par = stream.parameters();
            let codec_id = codec_par.id();
            let index = stream.index();

            let subtitle_duration_ts = stream.duration();
            let subtitle_time_base = stream.time_base();
            let subtitle_duration =
                if subtitle_duration_ts > 0 && subtitle_time_base.denominator() > 0 {
                    let secs = subtitle_duration_ts as f64 * subtitle_time_base.numerator() as f64
                        / subtitle_time_base.denominator() as f64;
                    Duration::from_secs_f64(secs)
                } else {
                    final_duration
                };

            SubtitleMetadata {
                index,
                codec_id,
                language: stream.metadata().get("language").map(|s| s.to_string()),
                duration: subtitle_duration,
            }
        })
        .collect();

    Ok(Metadata {
        path: path.to_path_buf(),
        size,
        bitrate,
        duration: final_duration,
        format,
        videos,
        audios,
        subtitles,
    })
}

/// Extract metadata for animated WebP files.
/// FFmpeg cannot decode animated WebP (skips ANIM/ANMF chunks), so we use
/// webpx for dimensions / frame count and decode only the first few frames
/// to estimate FPS — no full scan needed.
fn get_animated_webp_metadata(path: &Path) -> Result<VideoMetadata> {
    let data = std::fs::read(path)
        .map_err(|e| Error::InvalidFile(format!("Failed to read WebP file: {}", e)))?;

    let mut decoder = webpx::AnimationDecoder::with_options_limits(
        &data,
        webpx::ColorMode::Rgba,
        true,
        &webpx::Limits::none(),
    )
    .map_err(|e| Error::InvalidFile(format!("Failed to create WebP decoder: {e}")))?;

    let info = decoder.info();
    if info.frame_count == 0 {
        return Err(Error::InvalidConfig("WebP has no frames".into()));
    }
    let width = info.width;
    let height = info.height;
    let frame_count = info.frame_count as u64;

    // Decode first 3 frames to estimate average frame delay
    let sample_count = 3.min(frame_count as usize);
    let mut prev_ts: i32 = 0;
    let mut total_delay_ms: u64 = 0;
    let mut decoded: u64 = 0;
    for _ in 0..sample_count {
        match decoder.next_frame() {
            Ok(Some(frame)) => {
                if decoded > 0 {
                    total_delay_ms += (frame.timestamp_ms - prev_ts).max(1) as u64;
                }
                prev_ts = frame.timestamp_ms;
                decoded += 1;
            }
            _ => break,
        }
    }

    let avg_delay_ms = if decoded > 1 {
        total_delay_ms / (decoded - 1)
    } else {
        100
    };
    let total_duration_ms = frame_count * avg_delay_ms;
    let fps = if avg_delay_ms > 0 {
        1000.0 / avg_delay_ms as f32
    } else {
        25.0
    };

    Ok(VideoMetadata {
        index: 0,
        codec_id: ffmpeg::codec::Id::WEBP,
        pix_fmt: ffmpeg::format::Pixel::RGBA,
        width,
        height,
        fps,
        language: None,
        duration: Duration::from_millis(total_duration_ms),
    })
}

fn get_subtitle_duration_from_packets(
    input_ctx: &mut ffmpeg::format::context::Input,
    stream_index: usize,
) -> Duration {
    let stream = input_ctx
        .streams()
        .find(|s| s.index() == stream_index)
        .expect("Stream not found");

    let time_base = stream.time_base();
    let time_base_den = time_base.denominator() as f64;
    let time_base_num = time_base.numerator() as f64;

    let mut max_end_time_ts = 0i64;

    for (s, packet) in input_ctx.packets() {
        if s.index() != stream_index {
            continue;
        }

        let pts = match packet.pts() {
            Some(pts) if pts >= 0 => pts,
            _ => continue,
        };

        let duration = packet.duration();
        if duration > 0 {
            let end_ts = pts + duration;
            max_end_time_ts = max_end_time_ts.max(end_ts);
        }
    }

    let duration_secs = max_end_time_ts as f64 * time_base_num / time_base_den;
    Duration::from_secs_f64(duration_secs.max(0.0))
}

/// Create synthetic Metadata for SVG files.
/// FFmpeg cannot open SVG files, so we parse the SVG to extract dimensions
/// and construct a Metadata with MetadataType::Image.
fn get_svg_metadata(path: &Path) -> Result<Metadata> {
    let size = std::fs::metadata(path)?.len();

    let svg_data = std::fs::read(path).map_err(|e| Error::IO(e))?;
    let svg_str = String::from_utf8(svg_data)
        .map_err(|e| Error::InvalidFile(format!("SVG file is not valid UTF-8: {}", e)))?;

    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();

    let opts = usvg::Options {
        fontdb: db.into(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg_str, &opts)
        .map_err(|e| Error::InvalidFile(format!("Failed to parse SVG: {}", e)))?;

    let pixmap_size = tree.size().to_int_size();
    let width = pixmap_size.width();
    let height = pixmap_size.height();

    Ok(Metadata {
        path: path.to_path_buf(),
        size,
        bitrate: 0,
        duration: Duration::ZERO,
        format: vec!["svg".to_string()],
        videos: vec![VideoMetadata {
            index: 0,
            codec_id: ffmpeg::codec::Id::None,
            pix_fmt: ffmpeg::format::Pixel::RGBA,
            width,
            height,
            fps: 0.0,
            language: None,
            duration: Duration::ZERO,
        }],
        audios: Vec::new(),
        subtitles: Vec::new(),
    })
}

/// Create synthetic Metadata for LRC (lyrics) files.
/// FFmpeg cannot open LRC files, so we parse the LRC to calculate total duration
/// and construct a Metadata with MetadataType::Subtitle.
fn get_lrc_metadata(path: &Path) -> Result<Metadata> {
    let size = std::fs::metadata(path)?.len();
    let content = std::fs::read_to_string(path).map_err(|e| Error::IO(e))?;

    let entries = video_utils::subtitle::parse_lrc(&content);

    // Calculate duration from the last entry + a default tail duration
    let duration = if entries.is_empty() {
        Duration::ZERO
    } else {
        let last_ts = entries.last().unwrap().timestamp_ms;
        Duration::from_millis(last_ts + 3000) // 3s default for last lyric line
    };

    Ok(Metadata {
        path: path.to_path_buf(),
        size,
        bitrate: 0,
        duration,
        format: vec!["lrc".to_string()],
        videos: Vec::new(),
        audios: Vec::new(),
        subtitles: vec![SubtitleMetadata {
            index: 0,
            codec_id: ffmpeg::codec::Id::None,
            language: None,
            duration,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_image_duration_is_zero() {
        // Verify that JPEG images have metadata.duration = 0ns
        // This ensures the downstream 5-second default duration is applied.
        let jpeg_path = PathBuf::from("data/test.jpg");
        if !jpeg_path.exists() {
            // Skip test if test file not found
            return;
        }
        let metadata = get_metadata(&jpeg_path).expect("Failed to get JPEG metadata");
        assert!(
            metadata.duration.is_zero(),
            "JPEG metadata.duration should be zero, got {:?}",
            metadata.duration
        );
        assert_eq!(
            metadata.get_type(),
            MetadataType::Image,
            "JPEG should be classified as Image"
        );
    }

    #[test]
    fn test_png_image_duration_is_zero() {
        // Verify PNG images also have metadata.duration = 0ns
        let png_path = PathBuf::from("data/test.png");
        if !png_path.exists() {
            return;
        }
        let metadata = get_metadata(&png_path).expect("Failed to get PNG metadata");
        assert!(
            metadata.duration.is_zero(),
            "PNG metadata.duration should be zero, got {:?}",
            metadata.duration
        );
        assert_eq!(
            metadata.get_type(),
            MetadataType::Image,
            "PNG should be classified as Image"
        );
    }
}
