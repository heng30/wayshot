use super::{
    frame_position::{FrameRange, TimeToFrameConverter},
    video_frame_cache::{FrameCacheKey, VideoImage, get_global_video_cache},
};
use crate::{
    Error, Result, ensure_file_exists,
    filters::traits::{
        AudioFilter, AudioFilterWrapper, ImageFilterWrapper, SubtitleFilter, SubtitleFilterWrapper,
        VideoFilter, VideoFilterWrapper,
    },
    metadata::Metadata,
    preview::cache::{DEFAULT_CACHE_SAMPLE_RATE, get_global_audio_display_cache},
    tracks::{
        audio_track::{AudioSamples, extract_samples_from_frame},
        text_track::TextElement,
    },
};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use std::{fmt, path::Path, sync::Arc, time::Duration};

// 单个音频段的帧数据
#[derive(Debug)]
pub struct SegmentSamples {
    pub from_segment: Option<Arc<Segment>>, // 来源的segment，None表示静音
    pub samples: Vec<Option<f32>>,          // None = 间隙/gap, Some(f32) = 实际音频数据
    pub relative_timeline_offset: Duration, // 相对于segment开始的时间偏移
}

#[derive(Clone, Debug, Default)]
pub struct DisplayAudioSamples {
    pub channels: u16,
    pub samples: Vec<f32>,
}

// 用户UI显示的缓存数据
#[derive(Clone, Debug, Default)]
pub struct DisplayCache {
    pub thumbnail_left: Option<RgbaImage>,
    pub thumbnail_right: Option<RgbaImage>,
    pub audio_samples: Option<DisplayAudioSamples>,
}

#[derive(Clone)]
pub struct Segment {
    pub uuid: String,
    pub hiding: bool,
    pub audio_muted: bool, // 禁用segment的音频（仅对含音频的video segment生效）
    pub timeline_offset: Duration, // 片段在时间轴上的位置（何时播放）
    pub source_offset: Duration, // 片段在源文件中的起始位置（从哪里读取）
    pub duration: Duration, // 片段在时间轴上显示的时长（受 playback_speed 影响）
    pub original_duration: Duration, // 片段在源文件的原始时长（不受 playback_speed 影响，用于 speed 计算）
    pub playback_speed: f32,         // 播放速度倍率（默认1.0，范围0.25-4.0）
    pub global_speed: f32,           // 全局速度倍率（所有 segment 共享同一值，默认1.0）
    pub metadata: Arc<Metadata>,
    pub subtitle_text: Option<String>, // 字幕文本（仅字幕 segment 使用）
    pub text_element: Option<TextElement>, // TextSegment 的文字数据

    pub video_filters: Vec<Arc<VideoFilterWrapper>>,
    pub audio_filters: Vec<Arc<AudioFilterWrapper>>,
    pub subtitle_filters: Vec<Arc<SubtitleFilterWrapper>>,
    pub image_filters: Vec<Arc<ImageFilterWrapper>>,

    // Display cache for UI (not serialized, rebuilt on project load)
    pub display_cache: DisplayCache,
}

impl fmt::Debug for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Segment")
            .field("timeline_offset", &self.timeline_offset)
            .field("source_offset", &self.source_offset)
            .field("duration", &self.duration)
            .field("original_duration", &self.original_duration)
            .field("playback_speed", &self.playback_speed)
            .field("metadata", &self.metadata)
            .field("subtitle_text", &self.subtitle_text)
            .field("text_element", &self.text_element.is_some())
            .field("video_filters", &self.video_filters.len())
            .field("audio_filters", &self.audio_filters.len())
            .field("subtitle_filters", &self.subtitle_filters.len())
            .field("image_filters", &self.image_filters.len())
            .field("display_cache", &self.display_cache)
            .finish()
    }
}

impl Segment {
    pub fn new(
        timeline_offset: Duration,
        duration: Duration,
        metadata: Arc<Metadata>,
        global_speed: f32,
    ) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            hiding: false,
            audio_muted: false,
            timeline_offset,
            source_offset: Duration::ZERO, // 默认从源文件开头开始
            duration: Duration::from_secs_f64(duration.as_secs_f64() / global_speed as f64),
            original_duration: duration, // 原始时长等于初始时长
            playback_speed: 1.0,         // 默认播放速度
            global_speed,
            metadata,
            subtitle_text: None,
            text_element: None,
            video_filters: Vec::new(),
            audio_filters: Vec::new(),
            subtitle_filters: Vec::new(),
            image_filters: Vec::new(),
            display_cache: DisplayCache::default(),
        }
    }

    pub fn new_with_source_offset(
        timeline_offset: Duration,
        source_offset: Duration,
        original_duration: Duration,
        playback_speed: f32,
        global_speed: f32,
        metadata: Arc<Metadata>,
    ) -> Self {
        // 对于 image/subtitle 类型，不限制 duration，因为它们不受源文件时间限制
        let original_duration = if metadata.is_time_independent() {
            original_duration
        } else {
            original_duration.min(metadata.duration.saturating_sub(source_offset))
        };

        // 计算时间轴上的显示时长
        let timeline_duration = Duration::from_secs_f64(
            original_duration.as_secs_f64() / (playback_speed * global_speed) as f64,
        );

        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            hiding: false,
            audio_muted: false,
            timeline_offset,
            source_offset,
            duration: timeline_duration,
            original_duration,
            playback_speed,
            global_speed,
            metadata,
            subtitle_text: None,
            text_element: None,
            video_filters: Vec::new(),
            audio_filters: Vec::new(),
            subtitle_filters: Vec::new(),
            image_filters: Vec::new(),
            display_cache: DisplayCache::default(),
        }
    }

    pub fn with_subtitle_text(mut self, text: impl Into<String>) -> Self {
        self.subtitle_text = Some(text.into());
        self
    }

    pub fn with_text_element(mut self, element: TextElement) -> Self {
        self.text_element = Some(element);
        self
    }

    pub fn with_hiding(mut self, hiding: bool) -> Self {
        self.hiding = hiding;
        self
    }

    pub fn with_audio_muted(mut self, audio_muted: bool) -> Self {
        self.audio_muted = audio_muted;
        self
    }

    pub fn with_global_speed(mut self, global_speed: f32) -> Self {
        self.global_speed = global_speed;
        self.duration = Duration::from_secs_f64(
            self.original_duration.as_secs_f64() / (self.playback_speed * global_speed) as f64,
        );
        self
    }

    pub fn generate_uuid(&mut self) {
        self.uuid = uuid::Uuid::new_v4().to_string();
    }

    pub fn set_display_thumbnail_left(&mut self, thumbnail: RgbaImage) {
        self.display_cache.thumbnail_left = Some(thumbnail);
    }

    pub fn set_display_thumbnail_right(&mut self, thumbnail: RgbaImage) {
        self.display_cache.thumbnail_right = Some(thumbnail);
    }

    pub fn set_display_audio_samples(&mut self, channels: u16, samples: Vec<f32>) {
        self.display_cache.audio_samples = Some(DisplayAudioSamples { channels, samples });
    }

    pub fn clear_display_audio_samples(&mut self) {
        self.display_cache.audio_samples = None;
    }

    pub fn clear_display_cache(&mut self) {
        self.display_cache = DisplayCache::default();
    }

    pub fn has_display_cache(&self) -> bool {
        self.display_cache.thumbnail_left.is_some()
            || self.display_cache.thumbnail_right.is_some()
            || self.display_cache.audio_samples.is_some()
    }

    pub fn add_audio_filter(&mut self, filter: Box<dyn AudioFilter>) {
        self.audio_filters
            .push(Arc::new(AudioFilterWrapper::new(true, filter)));
    }

    pub fn clear_audio_filters(&mut self) {
        self.audio_filters.clear();
    }

    pub fn insert_audio_filter(
        &mut self,
        index: usize,
        filter: Box<dyn AudioFilter>,
    ) -> Result<()> {
        if index > self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.audio_filters.len(),
            ));
        }
        self.audio_filters
            .insert(index, Arc::new(AudioFilterWrapper::new(true, filter)));
        Ok(())
    }

    pub fn move_audio_filter(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                from,
                self.audio_filters.len(),
            ));
        }
        if to > self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(to, self.audio_filters.len()));
        }
        if from == to {
            return Ok(());
        }

        let filter = self.audio_filters.remove(from);
        self.audio_filters.insert(to, filter);
        Ok(())
    }

    pub fn remove_audio_filter(&mut self, index: usize) -> Result<Arc<AudioFilterWrapper>> {
        if index >= self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.audio_filters.len(),
            ));
        }
        Ok(self.audio_filters.remove(index))
    }

    pub fn toggle_audio_filter(&mut self, index: usize) -> Result<()> {
        if index >= self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.audio_filters.len(),
            ));
        }

        self.audio_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.audio_filters.len()))?
            .toggle();

        Ok(())
    }

    pub fn set_audio_filter_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        if index >= self.audio_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.audio_filters.len(),
            ));
        }

        self.audio_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.audio_filters.len()))?
            .set_enabled(enabled);

        Ok(())
    }

    pub fn add_video_filter(&mut self, filter: Box<dyn VideoFilter>) {
        self.video_filters
            .push(Arc::new(VideoFilterWrapper::new(true, filter)));
    }

    pub fn clear_video_filters(&mut self) {
        self.video_filters.clear();
    }

    pub fn insert_video_filter(
        &mut self,
        index: usize,
        filter: Box<dyn VideoFilter>,
    ) -> Result<()> {
        if index > self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.video_filters.len(),
            ));
        }
        self.video_filters
            .insert(index, Arc::new(VideoFilterWrapper::new(true, filter)));
        Ok(())
    }

    pub fn move_video_filter(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                from,
                self.video_filters.len(),
            ));
        }
        if to > self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(to, self.video_filters.len()));
        }
        if from == to {
            return Ok(());
        }

        let filter = self.video_filters.remove(from);
        self.video_filters.insert(to, filter);
        Ok(())
    }

    pub fn remove_video_filter(&mut self, index: usize) -> Result<Arc<VideoFilterWrapper>> {
        if index >= self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.video_filters.len(),
            ));
        }
        Ok(self.video_filters.remove(index))
    }

    pub fn toggle_video_filter(&mut self, index: usize) -> Result<()> {
        if index >= self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.video_filters.len(),
            ));
        }

        self.video_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.video_filters.len()))?
            .toggle();

        Ok(())
    }

    pub fn set_video_filter_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        if index >= self.video_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.video_filters.len(),
            ));
        }

        self.video_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.video_filters.len()))?
            .set_enabled(enabled);

        Ok(())
    }

    pub fn add_subtitle_filter(&mut self, filter: Box<dyn SubtitleFilter>) {
        self.subtitle_filters
            .push(Arc::new(SubtitleFilterWrapper::new(true, filter)));
    }

    pub fn clear_subtitle_filters(&mut self) {
        self.subtitle_filters.clear();
    }

    pub fn insert_subtitle_filter(
        &mut self,
        index: usize,
        filter: Box<dyn SubtitleFilter>,
    ) -> Result<()> {
        if index > self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.subtitle_filters.len(),
            ));
        }
        self.subtitle_filters
            .insert(index, Arc::new(SubtitleFilterWrapper::new(true, filter)));
        Ok(())
    }

    pub fn move_subtitle_filter(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                from,
                self.subtitle_filters.len(),
            ));
        }
        if to > self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                to,
                self.subtitle_filters.len(),
            ));
        }
        if from == to {
            return Ok(());
        }

        let filter = self.subtitle_filters.remove(from);
        self.subtitle_filters.insert(to, filter);
        Ok(())
    }

    pub fn remove_subtitle_filter(&mut self, index: usize) -> Result<Arc<SubtitleFilterWrapper>> {
        if index >= self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.subtitle_filters.len(),
            ));
        }
        Ok(self.subtitle_filters.remove(index))
    }

    pub fn toggle_subtitle_filter(&mut self, index: usize) -> Result<()> {
        if index >= self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.subtitle_filters.len(),
            ));
        }

        self.subtitle_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.subtitle_filters.len()))?
            .toggle();

        Ok(())
    }

    pub fn set_subtitle_filter_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        if index >= self.subtitle_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.subtitle_filters.len(),
            ));
        }

        self.subtitle_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.subtitle_filters.len()))?
            .set_enabled(enabled);

        Ok(())
    }

    pub fn add_image_filter(&mut self, filter: ImageFilterWrapper) {
        self.image_filters.push(Arc::new(filter));
    }

    pub fn clear_image_filters(&mut self) {
        self.image_filters.clear();
    }

    pub fn insert_image_filter(&mut self, index: usize, filter: ImageFilterWrapper) -> Result<()> {
        if index > self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.image_filters.len(),
            ));
        }
        self.image_filters.insert(index, Arc::new(filter));
        Ok(())
    }

    pub fn move_image_filter(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                from,
                self.image_filters.len(),
            ));
        }
        if to > self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(to, self.image_filters.len()));
        }
        if from == to {
            return Ok(());
        }

        let filter = self.image_filters.remove(from);
        self.image_filters.insert(to, filter);
        Ok(())
    }

    pub fn remove_image_filter(&mut self, index: usize) -> Result<Arc<ImageFilterWrapper>> {
        if index >= self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.image_filters.len(),
            ));
        }
        Ok(self.image_filters.remove(index))
    }

    pub fn toggle_image_filter(&mut self, index: usize) -> Result<()> {
        if index >= self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.image_filters.len(),
            ));
        }

        self.image_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.image_filters.len()))?
            .toggle();

        Ok(())
    }

    pub fn set_image_filter_enabled(&mut self, index: usize, enabled: bool) -> Result<()> {
        if index >= self.image_filters.len() {
            return Err(crate::Error::IndexOutOfBounds(
                index,
                self.image_filters.len(),
            ));
        }

        self.image_filters
            .get(index)
            .ok_or_else(|| crate::Error::IndexOutOfBounds(index, self.image_filters.len()))?
            .set_enabled(enabled);

        Ok(())
    }

    pub fn source_frame_range(&self) -> Result<FrameRange> {
        let video_meta = self.metadata.first_video().ok_or_else(|| {
            crate::Error::InvalidConfig("Segment must have video metadata".into())
        })?;

        let converter = TimeToFrameConverter::from_f32(video_meta.fps);
        let start_frame = converter.duration_to_frame(self.source_offset);
        let end_frame = converter.duration_to_frame(self.source_offset + self.duration);

        Ok(FrameRange::new(
            converter.frame_position(start_frame),
            converter.frame_position(end_frame),
        ))
    }

    pub fn frame_count(&self) -> Result<usize> {
        Ok(self.source_frame_range()?.frame_count())
    }

    // 获取指定timeline时间点对应的源文件帧索引
    pub fn frame_at_timeline_offset(&self, timeline_offset: Duration) -> Result<usize> {
        // 检查时间点是否在 Segment 范围内
        if timeline_offset < self.timeline_offset {
            return Err(crate::Error::InvalidConfig(format!(
                "Timeline offset {:?} is before segment start {:?}",
                timeline_offset, self.timeline_offset
            )));
        }

        if timeline_offset >= self.timeline_offset + self.duration {
            return Err(crate::Error::InvalidConfig(format!(
                "Timeline offset {:?} is after segment end {:?}",
                timeline_offset,
                self.timeline_offset + self.duration
            )));
        }

        // 获取帧范围并计算相对帧索引
        let frame_range = self.source_frame_range()?;
        let video_meta = self.metadata.first_video().ok_or_else(|| {
            crate::Error::InvalidConfig("No video stream found in segment".into())
        })?;
        let converter = TimeToFrameConverter::from_f32(video_meta.fps);

        // 计算相对于 Segment 开始位置的偏移
        let relative_offset = timeline_offset - self.timeline_offset;
        let relative_frame_index = converter.duration_to_frame(relative_offset);

        // 计算源文件中的帧索引
        let source_frame_index = frame_range.start.frame_index() + relative_frame_index;

        // 确保不超出范围
        if source_frame_index >= frame_range.end.frame_index() {
            return Err(crate::Error::InvalidConfig(format!(
                "Calculated frame index {} exceeds segment range [{}, {})",
                source_frame_index,
                frame_range.start.frame_index(),
                frame_range.end.frame_index()
            )));
        }

        Ok(source_frame_index)
    }

    pub fn first_frame_index(&self) -> Result<usize> {
        Ok(self.source_frame_range()?.start.frame_index())
    }

    pub fn last_frame_index(&self) -> Result<usize> {
        Ok(self.source_frame_range()?.end.frame_index() - 1)
    }

    pub fn first_frame_image(&self) -> Result<RgbaImage> {
        self.metadata.first_video().ok_or_else(|| {
            crate::Error::InvalidConfig("No video stream found in segment".into())
        })?;

        let start_frame = self.first_frame_index()?;
        let frames = self.extract_video(start_frame, 1)?;

        frames
            .into_iter()
            .next()
            .and_then(|video_image| match video_image {
                VideoImage::Image { buffer, .. } => Some(buffer),
                _ => None,
            })
            .ok_or_else(|| crate::Error::FFmpeg("Failed to extract first frame".into()))
    }

    // 用于UI展示的音频重采样，简化版本，不需要高精度. 返回 (通道数, 采样数据)
    pub fn audio_resampling_for_display(&self, samples_per_channel: u32) -> (u16, Vec<f32>) {
        let audio_meta = match self.metadata.audios.first() {
            Some(meta) => meta,
            None => return (0, Vec::new()),
        };

        let sample_count = if samples_per_channel == 0 {
            DEFAULT_CACHE_SAMPLE_RATE
        } else {
            samples_per_channel
        };

        if self.duration.is_zero() {
            return (audio_meta.channels, Vec::new());
        }

        // Try to get from global cache first
        let cache = get_global_audio_display_cache();
        if let Some(cache_data) = cache.get_by_path(&self.metadata.path, audio_meta.index) {
            return cache_data.extract_segment(self.source_offset, self.duration, sample_count);
        }

        // Fallback: synchronous loading if cache miss (should have been preloaded)
        log::debug!(
            "Audio cache miss for {}, loading synchronously",
            self.metadata.path.display()
        );

        match cache.load_and_cache(&self.metadata.path, audio_meta.index, audio_meta) {
            Ok(cache_data) => {
                cache_data.extract_segment(self.source_offset, self.duration, sample_count)
            }
            Err(e) => {
                log::warn!("Failed to load audio cache: {:?}", e);
                (audio_meta.channels, Vec::new())
            }
        }
    }

    pub fn last_frame_image(&self) -> Result<RgbaImage> {
        self.metadata.first_video().ok_or_else(|| {
            crate::Error::InvalidConfig("No video stream found in segment".into())
        })?;

        let last_frame_index = self.last_frame_index()?;

        // 尝试提取最后一帧，如果失败则向前回退
        for offset in 0..10 {
            let target_frame = last_frame_index.saturating_sub(offset);
            let frames = self.extract_video(target_frame, 1)?;

            if let Some(video_image) = frames.into_iter().next()
                && let VideoImage::Image { buffer, .. } = video_image
            {
                return Ok(buffer);
            }
        }

        Err(crate::Error::FFmpeg(format!(
            "Failed to extract last frame or nearby frames (tried from {} back to {})",
            last_frame_index,
            last_frame_index.saturating_sub(9)
        )))
    }

    pub fn frame_image_at_timeline_offset(&self, timeline_offset: Duration) -> Result<RgbaImage> {
        self.metadata.first_video().ok_or_else(|| {
            crate::Error::InvalidConfig("No video stream found in segment".into())
        })?;

        // 获取源文件帧索引
        let frame_index = self.frame_at_timeline_offset(timeline_offset)?;

        let frames = self.extract_video(frame_index, 1)?;

        frames
            .into_iter()
            .next()
            .and_then(|video_image| match video_image {
                VideoImage::Image { buffer, .. } => Some(buffer),
                _ => None,
            })
            .ok_or_else(|| {
                crate::Error::FFmpeg(format!(
                    "Failed to extract frame at timeline offset {:?} (frame index {})",
                    timeline_offset, frame_index
                ))
            })
    }

    // 从视频中提取指定范围的帧
    pub fn extract_video(
        &self,
        extract_start_frame: usize,
        extract_frames_count: usize,
    ) -> Result<Vec<VideoImage>> {
        let path = &self.metadata.path;
        ensure_file_exists!(path);

        let video_meta = self
            .metadata
            .first_video()
            .ok_or_else(|| Error::InvalidConfig("No video stream found in segment".into()))?;

        let stream_index = video_meta.index;
        let source_fps = video_meta.fps;
        let converter = TimeToFrameConverter::from_f32(source_fps);

        let start_frame_index = extract_start_frame;
        let end_frame_index = start_frame_index + extract_frames_count;

        let (mut cached_frames, uncached_indices) =
            get_cached_frames(path, stream_index, start_frame_index, end_frame_index);

        log::debug!(
            "Cached: {} frames, uncached: {} frames (request: {} to {} = {} frames)",
            cached_frames.len(),
            uncached_indices.len(),
            start_frame_index,
            end_frame_index,
            end_frame_index - start_frame_index
        );

        // 如果所有帧都已缓存，直接返回
        if uncached_indices.is_empty() {
            cached_frames.sort_by_key(|(idx, _)| *idx);
            log::debug!("✓ All {} frames cached", cached_frames.len());
            return Ok(cached_frames.into_iter().map(|(_, frame)| frame).collect());
        }

        let (mut input_ctx, mut decoder, time_base) = initialize_video_decoder(path, stream_index)?;
        let seek_time = converter.frame_to_duration(uncached_indices[0]);
        seek_to_frame(&mut input_ctx, path, seek_time, time_base);

        // Flush decoder after seek to clear stale buffered frames.
        // Without this, formats like GIF that maintain internal canvas state
        // may produce incorrect frames after seeking.
        decoder.flush();

        let decoded_frames = decode_video_packets_by_frame_range(
            &mut input_ctx,
            &mut decoder,
            stream_index,
            time_base,
            start_frame_index,
            end_frame_index,
            &uncached_indices,
            path,
            &converter,
        );

        cached_frames.extend(decoded_frames);
        cached_frames.sort_by_key(|(idx, _)| *idx);

        Ok(cached_frames.into_iter().map(|(_, frame)| frame).collect())
    }

    pub fn audio_sampling(&self, sample_per_channel: u32) -> Result<AudioSamples> {
        let audio_meta = match self.metadata.audios.first() {
            Some(meta) => meta,
            None => return Err(Error::InvalidConfig("Not audio stream".to_string())),
        };

        let path = &self.metadata.path;
        if !path.exists() {
            return Err(Error::InvalidFile(format!(
                "No found path: {}",
                path.display()
            )));
        }

        if sample_per_channel == 0 || self.duration.as_secs_f64() == 0.0 {
            return Err(Error::InvalidConfig(
                "Requested counts or duration is 0".to_string(),
            ));
        }

        audio_sampling_with_decoder(
            path,
            audio_meta.index,
            audio_meta.channels,
            self,
            sample_per_channel,
        )
    }
}

fn audio_sampling_with_decoder(
    path: &Path,
    stream_index: usize,
    channels: u16,
    segment: &Segment,
    sample_per_channel: u32,
) -> Result<AudioSamples> {
    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let mut input_ctx = ffmpeg::format::input(path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    let stream = input_ctx
        .streams()
        .find(|s| s.index() == stream_index)
        .ok_or_else(|| Error::FFmpeg(format!("Audio stream {} not found", stream_index)))?;

    let codec_par = stream.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par.clone())
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?
        .decoder()
        .audio()
        .map_err(|e| Error::FFmpeg(format!("Failed to get audio decoder: {}", e)))?;

    let time_base = stream.time_base();
    let source_format = decoder.format();

    let effective_sample_rate = (sample_per_channel as f64 / segment.duration.as_secs_f64()) as u32;
    let sample_interval = segment.duration.as_secs_f64() / sample_per_channel as f64;
    let mut samples = Vec::with_capacity(sample_per_channel as usize * channels as usize);

    let window_duration = Duration::from_millis(10);

    for i in 0..sample_per_channel {
        // 计算当前采样点的时间偏移
        let sample_offset = Duration::from_secs_f64(sample_interval * i as f64);
        let timeline_offset = segment.timeline_offset + sample_offset;

        // 计算在源文件中的位置
        let relative_offset = timeline_offset
            .checked_sub(segment.timeline_offset)
            .unwrap_or(Duration::ZERO)
            .as_secs_f64();
        let source_position = segment.source_offset.as_secs_f64() + relative_offset;

        // Seek 到目标位置（提前 1 秒以处理关键帧对齐）
        let seek_timestamp = if source_position > 1.0 {
            let seek_offset = 1.0 * time_base.denominator() as f64 / time_base.numerator() as f64;
            (source_position * time_base.denominator() as f64 / time_base.numerator() as f64) as i64
                - seek_offset as i64
        } else {
            0
        };

        if seek_timestamp > 0
            && let Err(e) = input_ctx.seek(seek_timestamp, ..)
        {
            log::warn!("Seek to timestamp {} failed: {:?}", seek_timestamp, e);
        }

        // 提取该时间点附近的小片段
        let target_end_source_time = Duration::from_secs_f64(
            (source_position + window_duration.as_secs_f64())
                .min(segment.source_offset.as_secs_f64() + segment.duration.as_secs_f64()),
        );

        let mut window_samples = Vec::new();

        for (s, packet) in input_ctx.packets() {
            if s.index() != stream_index {
                continue;
            }

            let packet_dts = match packet.dts() {
                Some(dts) => dts,
                None => continue,
            };

            let packet_time = Duration::from_secs_f64(
                (packet_dts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64)
                    .max(0.0),
            );

            if packet_time > target_end_source_time {
                break;
            }

            // 如果还没到目标位置，只解码不保存
            if packet_time < Duration::from_secs_f64(source_position) {
                _ = decoder.send_packet(&packet);
                let mut decoded_frame = ffmpeg::frame::Audio::empty();
                while decoder.receive_frame(&mut decoded_frame).is_ok() {
                    // 丢弃不在目标范围内的帧
                }
                continue;
            }

            if let Err(e) = decoder.send_packet(&packet) {
                log::warn!("Error sending packet to decoder: {:?}", e);
                continue;
            }

            let mut decoded_frame = ffmpeg::frame::Audio::empty();
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                if decoded_frame.samples() > 0 {
                    extract_samples_from_frame(
                        &decoded_frame,
                        source_format,
                        channels,
                        &mut window_samples,
                    )?;
                }
            }
        }

        // 刷新解码器缓冲区
        let mut decoded_frame = ffmpeg::frame::Audio::empty();
        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            if decoded_frame.samples() > 0 {
                extract_samples_from_frame(
                    &decoded_frame,
                    source_format,
                    channels,
                    &mut window_samples,
                )?;
            }
        }

        // 计算每个声道的平均值
        let channel_values = if window_samples.is_empty() {
            vec![0.0; channels as usize]
        } else {
            let ch_count = channels as usize;
            let mut channel_sums = vec![0.0_f32; ch_count];
            let mut channel_counts = vec![0_usize; ch_count];

            for (idx, &sample) in window_samples.iter().enumerate() {
                let ch = idx % ch_count;
                channel_sums[ch] += sample.abs();
                channel_counts[ch] += 1;
            }

            channel_sums
                .iter()
                .zip(channel_counts.iter())
                .map(|(&sum, &count)| if count > 0 { sum / count as f32 } else { 0.0 })
                .collect()
        };

        samples.extend(channel_values.into_iter());
    }

    // 确保数量正确
    if sample_per_channel as usize * channels as usize > samples.len() {
        samples.extend(std::iter::repeat_n(
            0.0_f32,
            sample_per_channel as usize * channels as usize - samples.len(),
        ));
    };

    Ok(AudioSamples {
        channels,
        sample_rate: effective_sample_rate,
        samples,
    })
}

fn get_cached_frames(
    path: &Path,
    stream_index: usize,
    start_frame_index: usize,
    end_frame_index: usize,
) -> (Vec<(usize, VideoImage)>, Vec<usize>) {
    let mut cached_frames = Vec::new();
    let mut uncached_indices = Vec::new();

    for frame_idx in start_frame_index..end_frame_index {
        let cache_key = FrameCacheKey::from_path(path, stream_index, frame_idx);

        if let Some(cached_frame) = get_global_video_cache().get(&cache_key) {
            cached_frames.push((frame_idx, cached_frame));
        } else {
            uncached_indices.push(frame_idx);
        }
    }

    (cached_frames, uncached_indices)
}

pub fn initialize_video_decoder(
    path: &Path,
    stream_index: usize,
) -> Result<(
    ffmpeg::format::context::Input,
    ffmpeg::decoder::Video,
    ffmpeg::Rational,
)> {
    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;
    let input_ctx = ffmpeg::format::input(path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    let stream = input_ctx
        .streams()
        .find(|s| s.index() == stream_index)
        .ok_or_else(|| Error::FFmpeg(format!("Video stream {} not found", stream_index)))?;

    let codec_par = stream.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par.clone())
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?
        .decoder()
        .video()
        .map_err(|e| Error::FFmpeg(format!("Failed to get video decoder: {}", e)))?;

    decoder.set_threading(ffmpeg::threading::Config {
        kind: ffmpeg::threading::Type::Frame,
        count: 2, // Limit to 2 threads to reduce per-decoder memory overhead
    });

    let time_base = stream.time_base();
    Ok((input_ctx, decoder, time_base))
}

pub fn seek_to_frame(
    input_ctx: &mut ffmpeg::format::context::Input,
    path: &Path,
    target_frame_time: Duration,
    _time_base: ffmpeg::Rational,
) {
    // FFmpeg format-level seek expects AV_TIME_BASE (microseconds), not stream time base.
    // Using stream time base causes seek to fail, resulting in reading from the beginning
    // of the file which severely degrades performance for late positions.
    let seek_timestamp =
        (target_frame_time.as_secs_f64() * ffmpeg::sys::AV_TIME_BASE as f64) as i64;

    if let Err(e) = input_ctx.seek(seek_timestamp, ..) {
        log::warn!(
            "{} seek to timestamp {} ({:.3}s) failed: {e}",
            path.display(),
            seek_timestamp,
            target_frame_time.as_secs_f64()
        );
    }
}

fn decode_video_packets_by_frame_range(
    input_ctx: &mut ffmpeg::format::context::Input,
    decoder: &mut ffmpeg::decoder::Video,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    start_frame_index: usize,
    end_frame_index: usize,
    uncached_indices: &[usize],
    path: &Path,
    converter: &TimeToFrameConverter,
) -> Vec<(usize, VideoImage)> {
    let mut decoded_frames = Vec::new();

    for (stream_idx, packet) in input_ctx.packets() {
        if stream_idx.index() != stream_index {
            continue;
        }

        let packet_time = match packet.dts() {
            Some(dts) if dts >= 0 => Duration::from_secs_f64(
                dts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64,
            ),
            Some(_) => Duration::ZERO,
            None => {
                let _ = decoder.send_packet(&packet);
                continue;
            }
        };

        let current_frame_index = converter.duration_to_frame(packet_time);

        // 如果超出目标范围太多，停止处理（但允许一定缓冲区以处理延迟帧）
        if current_frame_index >= end_frame_index + 5 {
            break;
        }

        // 不在范围内的帧，直接丢弃
        if current_frame_index < start_frame_index {
            // 发送数据包并尝试接收帧（避免解码器缓冲区满）
            match decoder.send_packet(&packet) {
                Ok(()) | Err(ffmpeg::Error::Other { .. }) => {
                    let mut decoded_frame = ffmpeg::frame::Video::empty();
                    loop {
                        match decoder.receive_frame(&mut decoded_frame) {
                            Ok(_) => {} // 这些帧不在目标范围内，不缓存
                            Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                            Err(_) => break,
                        }
                    }
                }
                Err(e) => log::warn!("Error sending packet to decoder: {:?}", e),
            }
            continue;
        }

        // EAGAIN 不是真正的错误，表示需要先接收帧
        if let Err(e) = decoder.send_packet(&packet)
            && !matches!(e, ffmpeg::Error::Other { .. })
        {
            log::warn!("Error sending packet to decoder: {:?}", e);
        }

        // 接收所有可用的帧
        let mut decoded_frame = ffmpeg::frame::Video::empty();
        loop {
            match decoder.receive_frame(&mut decoded_frame) {
                Ok(_) if uncached_indices.contains(&current_frame_index) => {
                    match convert_frame_to_image(&decoded_frame) {
                        Ok(image) => {
                            let cache_key =
                                FrameCacheKey::from_path(path, stream_index, current_frame_index);
                            get_global_video_cache().put(cache_key, image.clone());
                            decoded_frames.push((current_frame_index, image));
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to convert frame {}: {}x{} {:?}: {:?}",
                                current_frame_index,
                                decoded_frame.width(),
                                decoded_frame.height(),
                                decoded_frame.format(),
                                e
                            );
                        }
                    }
                }
                Ok(_) => (),
                Err(ffmpeg::Error::Other { .. }) => break,
                Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    log::warn!("receive_frame error: {e:?}",);
                    break;
                }
            }
        }
    }

    // FFmpeg解码有延迟，获取延迟帧中包含的目标帧
    // 先通知解码器输入结束，再 drain 剩余帧
    _ = decoder.send_eof();
    let mut decoded_frame = ffmpeg::frame::Video::empty();
    loop {
        match decoder.receive_frame(&mut decoded_frame) {
            Ok(_) => {
                let packet_time = match decoded_frame.timestamp() {
                    Some(ts) if ts >= 0 => Duration::from_secs_f64(
                        ts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64,
                    ),
                    Some(_) => Duration::ZERO,
                    None => break,
                };

                let current_frame_index = converter.duration_to_frame(packet_time);

                if uncached_indices.contains(&current_frame_index) {
                    match convert_frame_to_image(&decoded_frame) {
                        Ok(image) => {
                            let cache_key =
                                FrameCacheKey::from_path(path, stream_index, current_frame_index);
                            get_global_video_cache().put(cache_key, image.clone());
                            decoded_frames.push((current_frame_index, image));
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to convert frame {} (flush): {}x{} {:?}: {:?}",
                                current_frame_index,
                                decoded_frame.width(),
                                decoded_frame.height(),
                                decoded_frame.format(),
                                e
                            );
                        }
                    }
                }
            }
            Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
            Err(e) => {
                log::warn!("receive_frame error during flush: {e:?}");
                break;
            }
        }
    }

    decoded_frames
}

// 将 FFmpeg 视频帧转换为 VideoImage
pub fn convert_frame_to_image(frame: &ffmpeg::frame::Video) -> Result<VideoImage> {
    use video_utils::convert::rgb_into_rgba;

    let input_width = frame.width();
    let input_height = frame.height();
    let format = frame.format();

    // 先转换为原始大小的 RGBA 图像
    let video_image = match format {
        ffmpeg::format::Pixel::RGB24 => {
            let stride = frame.stride(0);
            let data = frame.data(0);
            let expected_len = input_width as usize * input_height as usize * 3;

            // 先创建 RGB 图像，然后转换为 RGBA
            let rgb_img = if data.len() == expected_len {
                let pixel_data: Vec<u8> = data[..expected_len].to_vec();
                image::RgbImage::from_raw(input_width, input_height, pixel_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGB image".into()))?
            } else {
                // 数据不连续，需要逐行复制
                let mut pixel_data = Vec::with_capacity(expected_len);
                let row_size = input_width as usize * 3;
                for y in 0..input_height as usize {
                    let row_start = y * stride;
                    let row_end = row_start + row_size;
                    if row_end <= data.len() {
                        pixel_data.extend_from_slice(&data[row_start..row_end]);
                    }
                }
                image::RgbImage::from_raw(input_width, input_height, pixel_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGB image".into()))?
            };

            let buffer = rgb_into_rgba(rgb_img);
            Ok(VideoImage::Image { buffer })
        }
        ffmpeg::format::Pixel::RGBA => {
            let stride = frame.stride(0);
            let data = frame.data(0);
            let expected_len = input_width as usize * input_height as usize * 4;

            // 数据是连续的
            let buffer = if data.len() == expected_len {
                let pixel_data: Vec<u8> = data[..expected_len].to_vec();
                image::RgbaImage::from_raw(input_width, input_height, pixel_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGBA image".into()))?
            } else {
                // 数据不连续，逐行复制
                let mut pixel_data = Vec::with_capacity(expected_len);
                let row_size = input_width as usize * 4;
                for y in 0..input_height as usize {
                    let row_start = y * stride;
                    let row_end = row_start + row_size;
                    if row_end <= data.len() {
                        pixel_data.extend_from_slice(&data[row_start..row_end]);
                    }
                }
                image::RgbaImage::from_raw(input_width, input_height, pixel_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGBA image".into()))?
            };

            Ok(VideoImage::Image { buffer })
        }
        ffmpeg::format::Pixel::BGRA => {
            // gif解码
            let stride = frame.stride(0);
            let data = frame.data(0);
            let expected_len = input_width as usize * input_height as usize * 4;

            let buffer = if data.len() == expected_len {
                let rgba_data: Vec<u8> = data[..expected_len]
                    .chunks_exact(4)
                    .flat_map(|px| [px[2], px[1], px[0], px[3]])
                    .collect();
                image::RgbaImage::from_raw(input_width, input_height, rgba_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGBA image from BGRA".into()))?
            } else {
                // Data is not contiguous — copy row by row, then swap B↔R
                let mut pixel_data = Vec::with_capacity(expected_len);
                let row_size = input_width as usize * 4;
                for y in 0..input_height as usize {
                    let row_start = y * stride;
                    let row_end = row_start + row_size;
                    if row_end <= data.len() {
                        pixel_data.extend_from_slice(&data[row_start..row_end]);
                    }
                }
                // Swap B↔R channels in the copied data
                for px in pixel_data.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                image::RgbaImage::from_raw(input_width, input_height, pixel_data)
                    .ok_or_else(|| Error::FFmpeg("Failed to create RGBA image from BGRA".into()))?
            };

            Ok(VideoImage::Image { buffer })
        }
        ffmpeg::format::Pixel::YUV420P => convert_yuv420p_to_rgba(frame),
        _ => Err(Error::FFmpeg(format!(
            "Unsupported pixel format: {:?}",
            format
        ))),
    }?;

    Ok(video_image)
}

fn convert_yuv420p_to_rgba(frame: &ffmpeg::frame::Video) -> Result<VideoImage> {
    use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix, yuv420_to_rgba};

    let width = frame.width();
    let height = frame.height();

    let y_plane = frame.data(0);
    let u_plane = frame.data(1);
    let v_plane = frame.data(2);

    let y_stride = frame.stride(0) as u32;
    let u_stride = frame.stride(1) as u32;
    let v_stride = frame.stride(2) as u32;

    let yuv_planar_image = YuvPlanarImage {
        y_plane,
        y_stride,
        u_plane,
        u_stride,
        v_plane,
        v_stride,
        width,
        height,
    };

    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    yuv420_to_rgba(
        &yuv_planar_image,
        &mut rgba_data,
        width * 4,                // RGBA stride (4 bytes per pixel)
        YuvRange::Limited,        // TV range (16-235) - matches encoder
        YuvStandardMatrix::Bt601, // BT.601 standard - matches encoder
    )
    .map_err(|e| Error::FFmpeg(format!("YUV to RGBA conversion failed: {:?}", e)))?;

    let buffer = image::RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| Error::FFmpeg("Failed to create RGBA image from YUV".into()))?;

    Ok(VideoImage::Image { buffer })
}
