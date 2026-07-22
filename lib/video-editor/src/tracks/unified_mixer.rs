use super::{
    audio_track::UnifiedAudioTracksMixerIterator,
    subtitle_track::UnifiedSubtitleTracksCompositorIterator,
    text_track::UnifiedTextTracksCompositorIterator,
    video_track::UnifiedVideoTracksCompositorIterator,
};
use crate::{
    filters::{SubtitleEntry, traits::GlobalFilterWrapper},
    tracks::{
        audio_track::AudioSamples,
        segment::Segment,
        text_track::TextElement,
        video_track::{LayerFrames, apply_global_filters},
    },
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, derivative::Derivative, derive_setters::Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct UnifiedMixerConfig {
    pub timeline_offset: Duration,

    #[derivative(Default(value = "Duration::from_secs(3)"))]
    pub cache_duration: Duration,

    #[derivative(Default(value = "Duration::from_secs(5)"))]
    pub max_cache_duration: Duration,

    // Output video width (None = auto-detect)
    pub output_width: Option<u32>,
    // Output video height (None = auto-detect)
    pub output_height: Option<u32>,
    // Output frame rate (None = auto-detect)
    pub output_fps: Option<f32>,
    // Output audio channels (None = auto-detect)
    pub output_channels: Option<u16>,
    // Output audio sample rate (None = auto-detect)
    pub output_sample_rate: Option<u32>,
    // Duration to mix (None = use maximum track duration)
    pub duration: Option<Duration>,
    // Disable global video frame cache during processing
    pub disable_global_cache: bool,

    // Global speed multiplier applied to all segments (default 1.0)
    #[derivative(Default(value = "1.0"))]
    pub global_speed: f32,
}

#[derive(Debug, Clone)]
pub struct UnifiedFrameSubtitle {
    pub subtitle: SubtitleEntry,
    pub segment: Arc<Segment>,
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Debug, Clone)]
pub struct UnifiedFrameText {
    pub element: TextElement,
    pub segment: Arc<Segment>,
    pub segment_index: usize,
    pub track_index: usize,
}

#[derive(Debug, Clone)]
pub struct UnifiedFrame {
    pub layer_frames: Option<LayerFrames>,
    pub audio: Option<AudioSamples>,
    pub subtitle: Vec<UnifiedFrameSubtitle>,
    pub text: Vec<UnifiedFrameText>,
    pub timeline_offset: Duration,
    pub post_composite_global_filters: Vec<Arc<GlobalFilterWrapper>>,
    pub duration: Duration,
}

// Unified mixer that combines video, audio, and subtitle tracks.
// This iterator yields synchronized frames containing video, audio, and subtitle data.
pub struct UnifiedTracksMixerIterator {
    video_iter: Option<UnifiedVideoTracksCompositorIterator>,
    audio_iter: Option<UnifiedAudioTracksMixerIterator>,
    subtitle_iter: Option<UnifiedSubtitleTracksCompositorIterator>,
    text_iter: Option<UnifiedTextTracksCompositorIterator>,
    output_fps: f32,

    // 播放开始位置（时间轴偏移）
    timeline_offset: Duration,

    // 获取缓存数据时间戳（基于时间轴）
    current_timestamp: Duration,

    // Current frame number
    current_frame: u64,

    // Duration for the mixing session
    duration: Duration,

    // 音频获取控制：true = 获取音频，false = 跳过音频
    // 用于避免音频预取和累积，根据 sink 队列长度动态设置
    fetch_audio: bool,

    // Global filters applied to composited image before subtitle/text overlay
    global_filters: Vec<Arc<GlobalFilterWrapper>>,
}

impl UnifiedTracksMixerIterator {
    pub fn new(
        video_iter: Option<UnifiedVideoTracksCompositorIterator>,
        audio_iter: Option<UnifiedAudioTracksMixerIterator>,
        subtitle_iter: Option<UnifiedSubtitleTracksCompositorIterator>,
        text_iter: Option<UnifiedTextTracksCompositorIterator>,
        timeline_offset: Duration,
        output_fps: f32,
        duration: Duration,
        global_filters: Vec<Arc<GlobalFilterWrapper>>,
    ) -> Self {
        Self {
            video_iter,
            audio_iter,
            subtitle_iter,
            text_iter,
            timeline_offset,
            current_timestamp: timeline_offset,
            output_fps,
            current_frame: 0,
            duration,
            fetch_audio: true,
            global_filters,
        }
    }

    // Calculate the maximum duration based on available tracks
    pub fn calculate_duration(&self) -> Duration {
        self.duration
    }

    pub fn set_fetch_audio(&mut self, fetch: bool) {
        self.fetch_audio = fetch;
    }

    pub fn should_fetch_audio(&self) -> bool {
        self.fetch_audio
    }
}

impl Iterator for UnifiedTracksMixerIterator {
    type Item = UnifiedFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let current_timeline_position = self.timeline_offset
            + Duration::from_secs_f64(self.current_frame as f64 / self.output_fps as f64);

        if current_timeline_position >= self.duration {
            return None;
        }

        let mut video = self.video_iter.as_mut().and_then(|iter| iter.next());

        if let Some(ref mut lf) = video
            && !self.global_filters.is_empty()
        {
            apply_global_filters(
                &mut lf.composited_image,
                &self.global_filters,
                current_timeline_position,
                self.duration,
                false,
            );
        }

        let audio = if self.fetch_audio {
            self.audio_iter.as_mut().and_then(|iter| iter.next())
        } else {
            None
        };

        let subtitle = self
            .subtitle_iter
            .as_ref()
            .map(|iter| iter.get_subtitle_at(self.current_timestamp))
            .unwrap_or_default();

        let text = self
            .text_iter
            .as_ref()
            .map(|iter| iter.get_text_at(self.current_timestamp))
            .unwrap_or_default();

        let post_composite_filters: Vec<Arc<GlobalFilterWrapper>> = self
            .global_filters
            .iter()
            .filter(|f| f.enabled() && f.inner.apply_post_composite())
            .cloned()
            .collect();

        let frame = UnifiedFrame {
            layer_frames: video,
            audio,
            subtitle,
            text,
            timeline_offset: self.current_timestamp,
            post_composite_global_filters: post_composite_filters,
            duration: self.duration,
        };

        self.current_frame += 1;
        self.current_timestamp += Duration::from_secs_f64(1.0 / self.output_fps as f64);

        Some(frame)
    }
}
