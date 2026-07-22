use super::{
    TimeToFrameConverter, segment::Segment, track::InnerTrack, video_frame_cache::VideoImage,
};
use crate::{
    Result,
    filters::{
        SubtitleEntry,
        traits::{GlobalFilterData, GlobalFilterWrapper, VideoData, VideoFilterConfig},
    },
    metadata::AudioMetadata,
    tracks::{
        DecodeVideoConfig,
        audio_track::AudioTrack,
        subtitle_track::{SubtitleTrack, extract_subtitles},
    },
};
use crossbeam::channel::{self, Receiver, Sender};
use image::RgbaImage;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct FilteredLayerImages {
    pub original_image: VideoImage, // 未经过filter处理的原始图层
    pub image: VideoImage,          // 经过filter处理用于UI显示的图层
    pub image_for_composite: Option<VideoImage>, // 用于合成的图层（None则使用image）
}

impl FilteredLayerImages {
    pub fn composite_image(&self) -> &VideoImage {
        self.image_for_composite.as_ref().unwrap_or(&self.image)
    }
}

#[derive(Debug, Clone)]
pub struct LayerFrame {
    pub original_image: VideoImage, //  没有经过filter处理的图层
    pub image: VideoImage,          // 经过filter处理的图层
    pub image_for_composite: Option<VideoImage>, // 合成后的图片，如果为None，则使用`image`
    pub track_index: usize,
    pub from_segment: Option<(usize, Arc<Segment>)>, // 在轨道中的位置
}

impl LayerFrame {
    pub fn new(
        original_image: VideoImage,
        image: VideoImage,
        from_segment: Option<(usize, Arc<Segment>)>,
        track_index: usize,
    ) -> Self {
        Self {
            original_image,
            image,
            image_for_composite: None,
            from_segment,
            track_index,
        }
    }

    pub fn with_composite_image(mut self, image_for_composite: VideoImage) -> Self {
        self.image_for_composite = Some(image_for_composite);
        self
    }

    pub fn composite_image(&self) -> &VideoImage {
        self.image_for_composite.as_ref().unwrap_or(&self.image)
    }
}

#[derive(Debug, Clone)]
pub struct LayerFrames {
    pub layers: Vec<LayerFrame>, // 包含每个轨道一帧数据
    pub composited_image: RgbaImage,
    pub relative_timeline_offset: Duration, // 相对于segment开始的时间偏移
}

#[derive(Debug, Clone)]
pub struct VideoTrack {
    pub name: String,
    pub hiding: bool,
    pub muted: bool,
    pub locked: bool,
    pub track: InnerTrack,
}

impl VideoTrack {
    pub fn new(track: InnerTrack) -> Self {
        Self {
            name: "V".to_string(),
            hiding: false,
            muted: false,
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

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn has_audio_in_any_segment(&self) -> bool {
        self.track
            .segments
            .iter()
            .any(|seg| !seg.metadata.audios.is_empty())
    }

    pub fn first_audio_meta(&self) -> Option<AudioMetadata> {
        self.track
            .segments
            .iter()
            .filter_map(|seg| seg.metadata.audios.first().cloned())
            .next()
    }

    pub(crate) fn update_duration(&mut self) {
        self.track.duration = self
            .track
            .segments
            .last()
            .map(|seg| seg.timeline_offset + seg.duration)
            .unwrap_or(Duration::ZERO);
    }

    // 将视频轨道中的所有音频轨道分离出来，并移除视频轨道中音频轨道的信息
    pub fn detach_audio_tracks(&mut self) -> Vec<AudioTrack> {
        let metadata = &self.track.metadata;

        if metadata.audios.is_empty() {
            return Vec::new();
        }

        let mut audio_tracks = Vec::new();
        for audio_meta in &metadata.audios {
            // 创建一个只包含当前音频流的元数据副本
            let audio_metadata = Arc::new(crate::metadata::Metadata {
                path: metadata.path.clone(),
                size: metadata.size,
                bitrate: metadata.bitrate,
                duration: metadata.duration,
                format: metadata.format.clone(),
                videos: Vec::new(),
                audios: vec![audio_meta.clone()],
                subtitles: Vec::new(),
            });

            // 创建音频轨道
            let audio_track = AudioTrack {
                name: "A".to_string(),
                hiding: false,
                locked: false,
                track: InnerTrack {
                    metadata: audio_metadata.clone(),
                    duration: self.track.duration,
                    segments: self
                        .track
                        .segments
                        .iter()
                        .filter(|seg| !seg.metadata.audios.is_empty())
                        .map(|seg| {
                            let mut new_seg = (*seg).clone();
                            let inner_seg = Arc::make_mut(&mut new_seg);
                            inner_seg.generate_uuid();
                            inner_seg.metadata = audio_metadata.clone();
                            new_seg
                        })
                        .collect(),
                },
            };
            audio_tracks.push(audio_track);
        }

        // 移除视频轨道元数据中的音频信息
        let metadata = Arc::make_mut(&mut self.track.metadata);
        metadata.audios.clear();

        audio_tracks
    }

    pub fn detach_segment_audio_tracks(&mut self, segment_index: usize) -> Vec<AudioTrack> {
        let segment = match self.track.segments.get(segment_index) {
            Some(seg) => seg.clone(),
            None => return Vec::new(),
        };

        let metadata = &segment.metadata;
        if metadata.audios.is_empty() {
            return Vec::new();
        }

        let mut audio_tracks = Vec::new();

        for audio_meta in &metadata.audios {
            let audio_metadata = Arc::new(crate::metadata::Metadata {
                path: metadata.path.clone(),
                size: metadata.size,
                bitrate: metadata.bitrate,
                duration: metadata.duration,
                format: metadata.format.clone(),
                videos: Vec::new(),
                audios: vec![audio_meta.clone()],
                subtitles: Vec::new(),
            });

            let audio_track = AudioTrack {
                name: "A".to_string(),
                hiding: false,
                locked: false,
                track: InnerTrack {
                    metadata: audio_metadata.clone(),
                    duration: segment.duration,
                    segments: {
                        let mut new_seg = segment.clone();
                        let seg = Arc::make_mut(&mut new_seg);
                        seg.generate_uuid();
                        seg.metadata = audio_metadata.clone();
                        vec![new_seg]
                    },
                },
            };
            audio_tracks.push(audio_track);
        }

        // 从 segment 的元数据中移除音频信息
        let segment = Arc::make_mut(&mut self.track.segments[segment_index]);
        let segment_metadata = Arc::make_mut(&mut segment.metadata);
        segment_metadata.audios.clear();

        audio_tracks
    }

    pub fn detach_segment_subtitle_tracks(
        &mut self,
        segment_index: usize,
        global_speed: f32,
    ) -> Vec<SubtitleTrack> {
        let segment = match self.track.segments.get(segment_index) {
            Some(seg) => seg.clone(),
            None => return Vec::new(),
        };

        let metadata = &segment.metadata;
        if metadata.subtitles.is_empty() {
            return Vec::new();
        }

        let mut subtitle_tracks = Vec::new();

        for subtitle_meta in &metadata.subtitles {
            let entries = match extract_subtitles(&metadata.path, subtitle_meta.index) {
                Ok(entries) => entries,
                Err(e) => {
                    log::warn!(
                        "Failed to extract subtitles from stream {}: {}",
                        subtitle_meta.index,
                        e
                    );
                    continue;
                }
            };

            if entries.is_empty() {
                continue;
            }

            let subtitle_metadata = Arc::new(crate::metadata::Metadata {
                path: metadata.path.clone(),
                size: metadata.size,
                bitrate: metadata.bitrate,
                duration: metadata.duration,
                format: metadata.format.clone(),
                videos: Vec::new(),
                audios: Vec::new(),
                subtitles: vec![subtitle_meta.clone()],
            });

            // Filter entries to only include those within the segment's time range
            let segment_start = segment.source_offset;
            let segment_end = segment_start + segment.duration;
            let filtered_entries: Vec<SubtitleEntry> = entries
                .into_iter()
                .filter(|entry| entry.start >= segment_start && entry.end <= segment_end)
                .collect();

            if filtered_entries.is_empty() {
                continue;
            }

            let segments: Vec<Arc<Segment>> = filtered_entries
                .iter()
                .map(|entry| {
                    let segment_duration = entry.end.saturating_sub(entry.start);
                    Arc::new(
                        Segment::new_with_source_offset(
                            entry.start,      // timeline_offset
                            entry.start,      // source_offset
                            segment_duration, // duration
                            1.0,
                            global_speed,
                            subtitle_metadata.clone(),
                        )
                        .with_subtitle_text(&entry.text),
                    )
                })
                .collect();

            let subtitle_track = SubtitleTrack::new(InnerTrack {
                metadata: subtitle_metadata,
                duration: segment.duration,
                segments,
            });
            subtitle_tracks.push(subtitle_track);
        }

        // Remove subtitle metadata from segment
        let segment = Arc::make_mut(&mut self.track.segments[segment_index]);
        let segment_metadata = Arc::make_mut(&mut segment.metadata);
        segment_metadata.subtitles.clear();

        subtitle_tracks
    }

    // 将视频轨道中的所有字幕轨道分离出来，并移除视频轨道中字幕轨道的信息
    pub fn detach_subtitle_tracks(&mut self, global_speed: f32) -> Vec<SubtitleTrack> {
        let metadata = &self.track.metadata;

        if metadata.subtitles.is_empty() {
            return Vec::new();
        }

        let mut subtitle_tracks = Vec::new();

        for subtitle_meta in &metadata.subtitles {
            let entries = match extract_subtitles(&metadata.path, subtitle_meta.index) {
                Ok(entries) => entries,
                Err(e) => {
                    log::warn!(
                        "Failed to extract subtitles from stream {}: {}",
                        subtitle_meta.index,
                        e
                    );
                    continue;
                }
            };

            if entries.is_empty() {
                continue;
            }

            let subtitle_metadata = Arc::new(crate::metadata::Metadata {
                path: metadata.path.clone(),
                size: metadata.size,
                bitrate: metadata.bitrate,
                duration: metadata.duration,
                format: metadata.format.clone(),
                videos: Vec::new(),
                audios: Vec::new(),
                subtitles: vec![subtitle_meta.clone()],
            });

            let segments: Vec<Arc<Segment>> = entries
                .iter()
                .map(|entry| {
                    let segment_duration = entry.end.saturating_sub(entry.start);
                    Arc::new(
                        Segment::new_with_source_offset(
                            entry.start,      // timeline_offset
                            entry.start,      // source_offset
                            segment_duration, // duration
                            1.0,
                            global_speed,
                            subtitle_metadata.clone(),
                        )
                        .with_subtitle_text(&entry.text),
                    )
                })
                .collect();

            let subtitle_track = SubtitleTrack::new(InnerTrack::new(
                subtitle_metadata,
                self.track.duration,
                segments,
            ));
            subtitle_tracks.push(subtitle_track);
        }

        // Remove subtitle metadata from video track
        let metadata = Arc::make_mut(&mut self.track.metadata);
        metadata.subtitles.clear();

        subtitle_tracks
    }
}

#[derive(Debug, Clone)]
pub struct VideoSourceInfo {
    pub track_index: usize,
    pub segments: Vec<VideoSegmentSourceInfo>,
}

impl VideoSourceInfo {
    pub fn new(index: usize, segments: Vec<VideoSegmentSourceInfo>) -> Self {
        Self {
            track_index: index,
            segments,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoSegmentSourceInfo {
    pub path: Option<PathBuf>,
    pub fps: Option<f32>,
    pub segment: Arc<Segment>,
    pub segment_index: usize, // segment 在轨道中的下标
}

impl VideoSegmentSourceInfo {
    pub fn new(
        path: Option<PathBuf>,
        fps: Option<f32>,
        segment: Arc<Segment>,
        segment_index: usize,
    ) -> Self {
        Self {
            path,
            fps,
            segment,
            segment_index,
        }
    }
}

#[derive(Debug)]
pub struct UnifiedVideoTracksCompositorIterator {
    pub source_infos: Vec<VideoSourceInfo>,
    pub timeline_offset: Duration,
    pub cache_duration: Duration,     // 初始缓存时间
    pub max_cache_duration: Duration, // 最大缓存时间
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f32,

    cache_frames: Vec<LayerFrames>,
    next_cache_frame_timeline_index: usize, // 下一次需要获取缓存的帧索引
    sender: Sender<Vec<LayerFrames>>,
    receiver: Receiver<Vec<LayerFrames>>,
    is_loading: Arc<AtomicBool>,

    reached_end: bool,
    end_frames_timeline_index: usize, // 最后一帧索引
    remained_frames_count: usize,
}

impl UnifiedVideoTracksCompositorIterator {
    pub fn new(
        source_infos: Vec<VideoSourceInfo>,
        timeline_offset: Duration,
        cache_duration: Duration,
        max_cache_duration: Duration,
        output_width: u32,
        output_height: u32,
        output_fps: f32,
    ) -> Result<Self> {
        if output_width == 0 {
            return Err(crate::Error::InvalidConfig(
                "output_width must be greater than 0".into(),
            ));
        }
        if output_height == 0 {
            return Err(crate::Error::InvalidConfig(
                "output_height must be greater than 0".into(),
            ));
        }
        if output_fps <= 0.0 {
            return Err(crate::Error::InvalidConfig(
                "output_fps must be greater than 0".into(),
            ));
        }
        if cache_duration == Duration::ZERO {
            return Err(crate::Error::InvalidConfig(
                "cache_duration must be greater than 0".into(),
            ));
        }
        if max_cache_duration < cache_duration {
            return Err(crate::Error::InvalidConfig(
                "max_cache_duration must be greater than or equal to cache_duration".into(),
            ));
        }

        let (sender, receiver) = channel::unbounded();
        let converter = TimeToFrameConverter::from_f32(output_fps);
        let next_cache_frame_timeline_index = converter.duration_to_frame(timeline_offset);
        let end_frames_timeline_index = converter.duration_to_frame(
            source_infos
                .iter()
                .flat_map(|info| info.segments.iter())
                .map(|seg| seg.segment.timeline_offset + seg.segment.duration)
                .max()
                .unwrap_or(Duration::ZERO),
        );

        let remained_frames_count =
            end_frames_timeline_index.saturating_sub(next_cache_frame_timeline_index);

        let mut iter = Self {
            source_infos,
            timeline_offset,
            cache_duration,
            max_cache_duration,
            output_width,
            output_height,
            output_fps,
            cache_frames: Vec::new(),
            next_cache_frame_timeline_index,
            sender,
            receiver,
            is_loading: Arc::new(AtomicBool::new(false)),
            reached_end: false,
            end_frames_timeline_index,
            remained_frames_count,
        };

        // 预加载初始缓存，避免首次播放时卡顿
        iter.start_background_loader();
        Ok(iter)
    }

    fn start_background_loader(&mut self) {
        if self.reached_end
            || self.next_cache_frame_timeline_index >= self.end_frames_timeline_index
        {
            return;
        }

        if self
            .is_loading
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            log::trace!("[bg_loader] already running, skipping");
            return;
        }

        let source_infos = self.source_infos.clone();
        let converter = TimeToFrameConverter::from_f32(self.output_fps);
        let start_cache_timeline_offset =
            converter.frame_to_duration(self.next_cache_frame_timeline_index);

        let sender = self.sender.clone();
        let output_fps = self.output_fps;
        let output_width = self.output_width;
        let output_height = self.output_height;
        let cache_duration = self.cache_duration;
        let is_loading = self.is_loading.clone();

        log::debug!(
            "[bg_loader] START: offset={:?}, duration={:?}",
            start_cache_timeline_offset,
            start_cache_timeline_offset + cache_duration
        );

        let config = DecodeVideoConfig {
            request_timeline_offset: start_cache_timeline_offset,
            request_duration: cache_duration,
            output_width,
            output_height,
            output_fps,
            disable_cache: false,
        };

        thread::spawn(move || {
            super::decode_video::decode_frames(source_infos, config, sender);
            is_loading.store(false, Ordering::SeqCst);
        });
    }

    fn refill_cache(&mut self) {
        while let Ok(frames) = self.receiver.try_recv() {
            if !frames.is_empty() {
                self.next_cache_frame_timeline_index += frames.len();
                self.cache_frames.extend(frames);
            } else if self.next_cache_frame_timeline_index >= self.end_frames_timeline_index {
                self.reached_end = true;
            }
        }
    }

    fn wait_for_data(&mut self, wait_time: Duration) -> bool {
        if self.cache_frames.is_empty() && self.reached_end {
            return false;
        }

        let cache_duration =
            Duration::from_secs_f64((self.cache_frames.len() as f64) / self.output_fps as f64);

        if cache_duration >= self.max_cache_duration {
            return true;
        }

        if !self.reached_end && cache_duration < self.max_cache_duration {
            self.start_background_loader();
        }

        // 没有缓存，并且还没到时间轴末尾，等待缓存准备好
        if self.cache_frames.is_empty() {
            let now = Instant::now();

            loop {
                if !self.receiver.is_empty() {
                    return true;
                }

                if now.elapsed() > wait_time {
                    break;
                }

                std::thread::sleep(Duration::from_millis(10));
            }

            log::warn!("Wait of video frame timeout: {:?}", wait_time);
            return false;
        }

        true
    }
}

impl Iterator for UnifiedVideoTracksCompositorIterator {
    type Item = LayerFrames;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remained_frames_count == 0 {
            return None;
        }

        self.refill_cache();

        if !self.wait_for_data(Duration::from_secs(5)) {
            return None;
        }

        self.refill_cache();

        if self.cache_frames.is_empty() {
            return None;
        }

        self.remained_frames_count -= 1;
        self.cache_frames.drain(0..1).next()
    }
}

// 将前景帧合成到背景帧上（居中对齐，简单的 Alpha 混合）
// alpha == 255: 直接覆盖背景像素
// alpha > 0: Alpha 混合前景和背景
// alpha == 0: 完全透明，保持背景不变
pub fn composite_frame(composited: &mut RgbaImage, foreground: &VideoImage) {
    match foreground {
        VideoImage::Empty => (),
        VideoImage::Image { buffer } => {
            let fg_width = buffer.width();
            let fg_height = buffer.height();
            let bg_width = composited.width();
            let bg_height = composited.height();

            // Center the foreground if smaller than background
            let x_offset = if fg_width < bg_width {
                (bg_width - fg_width) / 2
            } else {
                0
            };
            let y_offset = if fg_height < bg_height {
                (bg_height - fg_height) / 2
            } else {
                0
            };

            // Clamp dimensions for iteration
            let iter_width = fg_width.min(bg_width);
            let iter_height = fg_height.min(bg_height);

            for y in 0..iter_height {
                for x in 0..iter_width {
                    let fg_pixel = buffer.get_pixel(x, y);
                    let fg_alpha = fg_pixel[3];

                    // Apply pixel at centered position
                    let bg_x = x_offset + x;
                    let bg_y = y_offset + y;

                    if fg_alpha == 255 {
                        // 不透明：直接覆盖
                        let bg_pixel = composited.get_pixel_mut(bg_x, bg_y);
                        bg_pixel[0] = fg_pixel[0];
                        bg_pixel[1] = fg_pixel[1];
                        bg_pixel[2] = fg_pixel[2];
                        bg_pixel[3] = fg_pixel[3];
                    } else if fg_alpha > 0 {
                        // 半透明：Alpha 混合
                        let bg_pixel = composited.get_pixel_mut(bg_x, bg_y);
                        let alpha = fg_alpha as f32 / 255.0;
                        let inv_alpha = 1.0 - alpha;

                        bg_pixel[0] =
                            (fg_pixel[0] as f32 * alpha + bg_pixel[0] as f32 * inv_alpha) as u8;
                        bg_pixel[1] =
                            (fg_pixel[1] as f32 * alpha + bg_pixel[1] as f32 * inv_alpha) as u8;
                        bg_pixel[2] =
                            (fg_pixel[2] as f32 * alpha + bg_pixel[2] as f32 * inv_alpha) as u8;
                        bg_pixel[3] =
                            (fg_pixel[3] as f32 * alpha + bg_pixel[3] as f32 * inv_alpha) as u8;
                    }
                    // alpha == 0: 完全透明，保持背景不变
                }
            }
        }
    }
}

pub fn apply_segment_video_filters(
    config: VideoFilterConfig,
    original_frame: VideoImage,
    segment: Arc<Segment>,
    relative_timeline_offset: Duration,
) -> FilteredLayerImages {
    if segment.video_filters.is_empty() {
        return FilteredLayerImages {
            original_image: original_frame.clone(),
            image: original_frame,
            image_for_composite: None,
        };
    }

    let original_image = original_frame.clone();
    let mut image = original_frame;
    let mut image_for_composite: Option<VideoImage> = None;

    for filter in &segment.video_filters {
        if !filter.enabled() {
            continue;
        }

        if filter.inner.take_effect_in_layer_frame() {
            // Preserve image_for_composite effects before applying to image
            // This ensures filters like DrawRectangle/Circle aren't lost when
            // followed by filters like Crop that operate on `image`
            if let Some(composite) = image_for_composite.take() {
                image = composite;
            }

            // Apply to image, use for both
            let mut video_data = VideoData {
                config: config.clone(),
                frames: vec![image],
                from_segment: segment.clone(),
                relative_timeline_offset,
            };
            if let Err(e) = filter.inner.apply(&mut video_data) {
                log::warn!("Apply filter: `{}` failed: {e}", filter.inner.name());
            }
            image = video_data
                .frames
                .into_iter()
                .next()
                .unwrap_or(VideoImage::Empty);
            // Reset composite to None since image changed (means they're the same)
            image_for_composite = None;
        } else {
            // Apply only to image_for_composite
            let base = image_for_composite.as_ref().unwrap_or(&image);
            let mut video_data = VideoData {
                config: config.clone(),
                frames: vec![base.clone()],
                from_segment: segment.clone(),
                relative_timeline_offset,
            };
            if let Err(e) = filter.inner.apply(&mut video_data) {
                log::warn!("Apply filter: `{}` failed: {e}", filter.inner.name());
            }
            image_for_composite = video_data.frames.into_iter().next();
        }
    }

    FilteredLayerImages {
        original_image,
        image,
        image_for_composite,
    }
}

pub fn apply_global_filters(
    image: &mut RgbaImage,
    filters: &[Arc<GlobalFilterWrapper>],
    timeline_offset: Duration,
    total_duration: Duration,
    post_composite: bool,
) {
    for filter in filters {
        if !filter.enabled() {
            continue;
        }

        if filter.inner.apply_post_composite() != post_composite {
            continue;
        }

        let mut data = GlobalFilterData {
            image: image.clone(),
            timeline_offset,
            total_duration,
        };

        if let Err(e) = filter.inner.apply(&mut data) {
            log::warn!("Global filter `{}` failed: {}", filter.inner.name(), e);
            continue;
        }

        *image = data.image;
    }
}
