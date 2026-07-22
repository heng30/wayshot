use super::{
    segment::Segment, track::InnerTrack, unified_mixer::UnifiedFrameSubtitle,
    video_frame_cache::VideoImage,
};
use crate::{
    Error, Result, ensure_file_exists,
    filters::{
        subtitle::{renderer::render_text_to_image, style::SubtitleStyle},
        traits::{SubtitleEntry, SubtitleFilter},
    },
    metadata::Metadata,
    tracks::video_track::LayerFrame,
};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use std::{path::Path, sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub name: String,
    pub hiding: bool,
    pub locked: bool,
    pub track: InnerTrack,
}

impl SubtitleTrack {
    pub fn new(track: InnerTrack) -> Self {
        Self {
            name: "S".to_string(),
            hiding: false,
            locked: false,
            track,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_hiding(&mut self, hiding: bool) {
        self.hiding = hiding;
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn update_duration(&mut self) {
        self.track.duration = self
            .track
            .segments
            .last()
            .map(|seg| seg.timeline_offset + seg.duration)
            .unwrap_or(Duration::ZERO);
    }

    // 获取所有字幕条目（用于导出等场景）
    pub fn get_subtitle_entries(&self) -> Vec<SubtitleEntry> {
        self.track
            .segments
            .iter()
            .filter_map(|seg| {
                seg.subtitle_text.as_ref().map(|text| SubtitleEntry {
                    start: seg.timeline_offset,
                    end: seg.timeline_offset + seg.duration,
                    text: text.clone(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct SubtitleSource {
    pub track_index: usize,
    pub track: Arc<SubtitleTrack>,
}

impl SubtitleSource {
    pub fn metadata(&self) -> &Arc<Metadata> {
        &self.track.track.metadata
    }

    pub fn duration(&self) -> Duration {
        self.track.track.duration
    }

    pub fn segments(&self) -> &[Arc<Segment>] {
        &self.track.track.segments
    }
}

#[derive(Debug)]
pub struct UnifiedSubtitleTracksCompositorIterator {
    pub timeline_offset: Duration,
    track_entries: Vec<Vec<UnifiedFrameSubtitle>>,
}

impl UnifiedSubtitleTracksCompositorIterator {
    pub fn new(sources: Vec<SubtitleSource>, timeline_offset: Duration) -> Result<Self> {
        let mut track_entries: Vec<Vec<UnifiedFrameSubtitle>> = Vec::new();

        for source in &sources {
            let track_index = source.track_index;
            let mut entries: Vec<UnifiedFrameSubtitle> = Vec::new();

            if source.track.track.segments.is_empty() {
                continue;
            }

            // 直接从 segments 构建字幕条目，同时获取 segment_index
            for (segment_index, segment) in source.track.track.segments.iter().enumerate() {
                let Some(text) = &segment.subtitle_text else {
                    continue;
                };

                let subtitle = SubtitleEntry {
                    start: segment.timeline_offset,
                    end: segment.timeline_offset + segment.duration,
                    text: text.clone(),
                };

                // 过滤掉已经结束的字幕
                if subtitle.end <= timeline_offset {
                    continue;
                }

                entries.push(UnifiedFrameSubtitle {
                    subtitle,
                    segment: segment.clone(),
                    segment_index,
                    track_index,
                });
            }

            if !entries.is_empty() {
                entries.sort_by(|a, b| a.subtitle.start.cmp(&b.subtitle.start));
                track_entries.push(entries);
            }
        }

        Ok(Self {
            timeline_offset,
            track_entries,
        })
    }

    pub fn set_timeline_offset(&mut self, offset: Duration) {
        self.timeline_offset = offset;
    }

    pub fn get_subtitle_at(&self, timestamp: Duration) -> Vec<UnifiedFrameSubtitle> {
        let mut result = Vec::new();

        for entries in self.track_entries.iter().rev() {
            let idx = entries.partition_point(|entry| entry.subtitle.start <= timestamp);

            if idx > 0 {
                let entry = &entries[idx - 1];
                if timestamp >= entry.subtitle.start && timestamp < entry.subtitle.end {
                    result.push(entry.clone());
                }
            }
        }

        result
    }
}

// 从字幕文件中提取字幕条目
pub fn extract_subtitles(path: &Path, stream_index: usize) -> Result<Vec<SubtitleEntry>> {
    ensure_file_exists!(path);

    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let mut input_ctx = ffmpeg::format::input(path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    let stream = input_ctx
        .streams()
        .find(|s| s.index() == stream_index)
        .ok_or_else(|| {
            Error::FFmpeg(format!("Subtitle stream index {} not found", stream_index))
        })?;

    let codec_par = stream.parameters();
    let codec_id = codec_par.id();

    let mut entries = Vec::new();

    match codec_id {
        ffmpeg::codec::Id::SUBRIP => {
            extract_subrip_subtitles(&mut input_ctx, stream_index, &mut entries)?;
        }
        ffmpeg::codec::Id::ASS => {
            extract_ass_subtitles(&mut input_ctx, stream_index, &mut entries)?;
        }
        ffmpeg::codec::Id::TEXT => {
            extract_text_subtitles(&mut input_ctx, stream_index, &mut entries)?;
        }
        _ => {
            extract_text_subtitles(&mut input_ctx, stream_index, &mut entries)?;
        }
    }

    Ok(entries)
}

// 从字幕文件中提取字幕并创建 Segments
pub fn extract_subtitles_as_segments(
    path: &Path,
    stream_index: usize,
    metadata: Arc<Metadata>,
    global_speed: f32,
) -> Result<Vec<Arc<Segment>>> {
    // Special handling for LRC files - FFmpeg cannot parse them
    if path
        .extension()
        .map(|e| e.to_ascii_lowercase() == "lrc")
        .unwrap_or(false)
    {
        return extract_lrc_as_segments(path, metadata, global_speed);
    }

    let entries = extract_subtitles(path, stream_index)?;

    let segments: Vec<Arc<Segment>> = entries
        .into_iter()
        .map(|entry| {
            let segment_duration = entry.end.saturating_sub(entry.start);
            Arc::new(
                Segment::new_with_source_offset(
                    entry.start,    // timeline_offset（初始化时使用源时间）
                    Duration::ZERO, // source_offset = 0（不受源时间限制）
                    segment_duration,
                    1.0,
                    global_speed,
                    metadata.clone(),
                )
                .with_subtitle_text(entry.text),
            )
        })
        .collect();

    Ok(segments)
}

fn extract_subrip_subtitles(
    input_ctx: &mut ffmpeg::format::context::Input,
    stream_index: usize,
    entries: &mut Vec<SubtitleEntry>,
) -> Result<()> {
    for (s, packet) in input_ctx.packets() {
        if s.index() != stream_index {
            continue;
        }

        let pts = match packet.pts() {
            Some(pts) => pts,
            None => continue,
        };

        let data = match packet.data() {
            Some(d) => d,
            None => continue,
        };
        let text = String::from_utf8_lossy(data);
        let text = text.trim().to_string();

        if !text.is_empty() {
            let duration = packet.duration();
            let time_base = s.time_base();

            let time_base_num = time_base.numerator() as u64;
            let time_base_den = time_base.denominator() as u64;

            // 计算：pts * (num/den) = (pts * num) / den 秒
            let start_micros = (pts as u64 * 1_000_000 * time_base_num) / time_base_den;
            let duration_micros = (duration as u64 * 1_000_000 * time_base_num) / time_base_den;

            entries.push(SubtitleEntry {
                start: Duration::from_micros(start_micros),
                end: Duration::from_micros(start_micros + duration_micros),
                text,
            });
        }
    }

    Ok(())
}

fn extract_ass_subtitles(
    input_ctx: &mut ffmpeg::format::context::Input,
    stream_index: usize,
    entries: &mut Vec<SubtitleEntry>,
) -> Result<()> {
    for (s, packet) in input_ctx.packets() {
        if s.index() != stream_index {
            continue;
        }

        let pts = match packet.pts() {
            Some(pts) => pts,
            None => continue,
        };

        let data = match packet.data() {
            Some(d) => d,
            None => continue,
        };
        let text = String::from_utf8_lossy(data);
        let text = text.trim().to_string();

        // ASS 格式有特定的头部信息，这里简化处理
        if !text.is_empty() && !text.starts_with("[Script Info]") {
            let duration = packet.duration();
            let time_base = s.time_base();

            let time_base_num = time_base.numerator() as u64;
            let time_base_den = time_base.denominator() as u64;

            let start_micros = (pts as u64 * 1_000_000 * time_base_num) / time_base_den;
            let duration_micros = (duration as u64 * 1_000_000 * time_base_num) / time_base_den;

            entries.push(SubtitleEntry {
                start: Duration::from_micros(start_micros),
                end: Duration::from_micros(start_micros + duration_micros),
                text,
            });
        }
    }

    Ok(())
}

fn extract_text_subtitles(
    input_ctx: &mut ffmpeg::format::context::Input,
    stream_index: usize,
    entries: &mut Vec<SubtitleEntry>,
) -> Result<()> {
    for (s, packet) in input_ctx.packets() {
        if s.index() != stream_index {
            continue;
        }

        let pts = match packet.pts() {
            Some(pts) => pts,
            None => continue,
        };

        let data = match packet.data() {
            Some(d) => d,
            None => continue,
        };
        let text = String::from_utf8_lossy(data);
        let text = text.trim().to_string();

        if !text.is_empty() {
            let duration = packet.duration();
            let time_base = s.time_base();

            let time_base_num = time_base.numerator() as u64;
            let time_base_den = time_base.denominator() as u64;

            let start_micros = (pts as u64 * 1_000_000 * time_base_num) / time_base_den;
            let duration_micros = (duration as u64 * 1_000_000 * time_base_num) / time_base_den;

            entries.push(SubtitleEntry {
                start: Duration::from_micros(start_micros),
                end: Duration::from_micros(start_micros + duration_micros),
                text,
            });
        }
    }

    Ok(())
}

pub fn extract_lrc_as_segments(
    path: &Path,
    metadata: Arc<Metadata>,
    global_speed: f32,
) -> Result<Vec<Arc<Segment>>> {
    let content = std::fs::read_to_string(path).map_err(|e| Error::IO(e))?;
    let lrc_entries = video_utils::subtitle::parse_lrc(&content);

    if lrc_entries.is_empty() {
        return Ok(Vec::new());
    }

    let subtitles = video_utils::subtitle::lrc_to_subtitles(&lrc_entries);

    let segments: Vec<Arc<Segment>> = subtitles
        .into_iter()
        .map(|sub| {
            let segment_duration =
                Duration::from_millis(sub.end_timestamp.saturating_sub(sub.start_timestamp));
            Arc::new(
                Segment::new_with_source_offset(
                    Duration::from_millis(sub.start_timestamp),
                    Duration::ZERO,
                    segment_duration,
                    1.0,
                    global_speed,
                    metadata.clone(),
                )
                .with_subtitle_text(sub.text),
            )
        })
        .collect();

    Ok(segments)
}

pub fn apply_segment_subtitle_filters(
    img: &mut RgbaImage,
    entry: &SubtitleEntry,
    segment: Arc<Segment>,
) -> Result<()> {
    let mut style = SubtitleStyle::default();
    let filters: Vec<Box<dyn SubtitleFilter>> = segment
        .subtitle_filters
        .iter()
        .filter(|f| f.enabled())
        .map(|f| f.inner.clone_box())
        .collect();

    style.apply_filters(&filters);

    render_text_to_image(img, &entry.text, &style)?;

    Ok(())
}

pub fn create_subtitle_layer_frame(
    entry: &SubtitleEntry,
    segment: Arc<Segment>,
    segment_index: usize,
    track_index: usize,
    output_width: u32,
    output_height: u32,
) -> Result<LayerFrame> {
    let mut style = SubtitleStyle::default();
    let filters: Vec<Box<dyn SubtitleFilter>> = segment
        .subtitle_filters
        .iter()
        .filter(|f| f.enabled())
        .map(|f| f.inner.clone_box())
        .collect();
    style.apply_filters(&filters);

    let mut img = RgbaImage::new(output_width, output_height);
    render_text_to_image(&mut img, &entry.text, &style)?;

    let video_image = VideoImage::image(img);
    Ok(LayerFrame::new(
        video_image.clone(),
        video_image,
        Some((segment_index, segment)),
        track_index,
    ))
}
