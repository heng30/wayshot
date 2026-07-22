use crate::{
    Result,
    filters::{
        keyframe::{AnimatableProperty, KeyframeTracks},
        subtitle::style::SubtitleStyle,
    },
    tracks::{segment::Segment, video_frame_cache::VideoImage},
};
use image::RgbaImage;
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum EffectPosition {
    #[default]
    Start = 0,
    End,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
    serde::Serialize,
    serde::Deserialize,
)]
#[repr(u8)]
pub enum EasingFunction {
    #[default]
    Linear = 0,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Debug, Clone)]
pub struct VideoFilterConfig {
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f32,
}

impl VideoFilterConfig {
    pub fn new(width: u32, height: u32, fps: f32) -> Self {
        Self {
            output_width: width,
            output_height: height,
            output_fps: fps,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalFilterData {
    pub image: RgbaImage,
    pub timeline_offset: Duration, // 绝对时间偏移
    pub total_duration: Duration,  // 总时长
}

#[derive(Debug, Clone)]
pub struct VideoData {
    pub config: VideoFilterConfig,
    pub frames: Vec<VideoImage>,
    pub from_segment: Arc<Segment>,
    pub relative_timeline_offset: Duration, // 相对于segment开始的时间偏移
}

pub type ImageData = VideoData;

#[derive(Debug, Clone)]
pub struct AudioFilterConfig {
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub struct AudioData {
    pub config: AudioFilterConfig,
    pub samples: Vec<f32>,
    pub from_segment: Arc<Segment>,
    pub relative_timeline_offset: Duration, // 相对于segment开始的时间偏移（块起始时间）
    pub chunk_duration: Duration,           // 块时长，用于计算每个样本的时间
}

#[derive(Debug, Clone)]
pub struct SubtitleEntry {
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

pub trait GlobalFilter: Any + Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, data: &mut GlobalFilterData) -> Result<()>;
    fn clone_box(&self) -> Box<dyn GlobalFilter>;
    fn as_any(&self) -> &dyn Any;

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        vec![]
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        KeyframeTracks::default()
    }

    fn set_keyframe_tracks(&mut self, _tracks: KeyframeTracks) {}

    fn supports_keyframes(&self) -> bool {
        !self.get_animatable_properties().is_empty()
    }

    fn update_keyframes_at_time(&self, _tracks: &mut KeyframeTracks, _time_ms: i64) -> bool {
        false
    }

    // Whether this filter should be applied after subtitle/text compositing
    fn apply_post_composite(&self) -> bool {
        false
    }
}

pub trait VideoFilter: Any + Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, data: &mut VideoData) -> Result<()>;
    fn clone_box(&self) -> Box<dyn VideoFilter>;
    fn as_any(&self) -> &dyn Any;
    fn take_effect_in_layer_frame(&self) -> bool {
        true
    }

    // Get the list of animatable properties for this filter
    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        vec![]
    }

    // Get all property tracks with keyframes
    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        KeyframeTracks::default()
    }

    // Set property tracks (for keyframe editing)
    fn set_keyframe_tracks(&mut self, _tracks: KeyframeTracks) {}

    // Check if this filter supports keyframes
    fn supports_keyframes(&self) -> bool {
        !self.get_animatable_properties().is_empty()
    }

    // Update keyframes at the given time if they exist
    fn update_keyframes_at_time(&self, _tracks: &mut KeyframeTracks, _time_ms: i64) -> bool {
        false
    }
}

pub trait AudioFilter: Any + Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, data: &mut AudioData) -> Result<()>;
    fn clone_box(&self) -> Box<dyn AudioFilter>;
    fn as_any(&self) -> &dyn Any;

    // Get the list of animatable properties for this filter
    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        vec![]
    }

    // Get all property tracks with keyframes
    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        KeyframeTracks::default()
    }

    // Set property tracks (for keyframe editing)
    fn set_keyframe_tracks(&mut self, _tracks: KeyframeTracks) {}

    // Check if this filter supports keyframes
    fn supports_keyframes(&self) -> bool {
        !self.get_animatable_properties().is_empty()
    }

    // Update keyframes at the given time if they exist
    fn update_keyframes_at_time(&self, _tracks: &mut KeyframeTracks, _time_ms: i64) -> bool {
        false
    }
}

pub trait SubtitleFilter: Any + Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, style: &mut SubtitleStyle);
    fn clone_box(&self) -> Box<dyn SubtitleFilter>;
    fn as_any(&self) -> &dyn Any;
}

pub struct VideoFilterWrapper {
    pub enabled: AtomicBool,
    pub inner: Box<dyn VideoFilter>,
}

impl std::fmt::Debug for VideoFilterWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFilterWrapper")
            .field("enabled", &self.enabled.load(Ordering::Relaxed))
            .field("inner", &self.inner.name())
            .finish()
    }
}

impl Clone for VideoFilterWrapper {
    fn clone(&self) -> Self {
        Self {
            enabled: AtomicBool::new(self.enabled.load(Ordering::Relaxed)),
            inner: self.inner.clone_box(),
        }
    }
}

impl VideoFilterWrapper {
    pub fn new(enabled: bool, inner: Box<dyn VideoFilter>) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            inner,
        }
    }

    pub fn toggle(&self) {
        self.enabled.fetch_xor(true, Ordering::SeqCst);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

pub struct AudioFilterWrapper {
    pub enabled: AtomicBool,
    pub inner: Box<dyn AudioFilter>,
}

impl Clone for AudioFilterWrapper {
    fn clone(&self) -> Self {
        Self {
            enabled: AtomicBool::new(self.enabled.load(Ordering::Relaxed)),
            inner: self.inner.clone_box(),
        }
    }
}

impl AudioFilterWrapper {
    pub fn new(enabled: bool, inner: Box<dyn AudioFilter>) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            inner,
        }
    }

    pub fn toggle(&self) {
        self.enabled.fetch_xor(true, Ordering::SeqCst);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

pub struct SubtitleFilterWrapper {
    pub enabled: AtomicBool,
    pub inner: Box<dyn SubtitleFilter>,
}

impl SubtitleFilterWrapper {
    pub fn new(enabled: bool, inner: Box<dyn SubtitleFilter>) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            inner,
        }
    }

    pub fn toggle(&self) {
        self.enabled.fetch_xor(true, Ordering::SeqCst);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

pub struct GlobalFilterWrapper {
    pub enabled: AtomicBool,
    pub inner: Box<dyn GlobalFilter>,
}

impl std::fmt::Debug for GlobalFilterWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalFilterWrapper")
            .field("enabled", &self.enabled.load(Ordering::Relaxed))
            .field("inner", &self.inner.name())
            .finish()
    }
}

impl Clone for GlobalFilterWrapper {
    fn clone(&self) -> Self {
        Self {
            enabled: AtomicBool::new(self.enabled.load(Ordering::Relaxed)),
            inner: self.inner.clone_box(),
        }
    }
}

impl GlobalFilterWrapper {
    pub fn new(enabled: bool, inner: Box<dyn GlobalFilter>) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            inner,
        }
    }

    pub fn toggle(&self) {
        self.enabled.fetch_xor(true, Ordering::SeqCst);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

pub type ImageFilterWrapper = VideoFilterWrapper;

#[macro_export]
macro_rules! impl_default_filter {
    ($type:ty, $trait:path) => {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn clone_box(&self) -> Box<dyn $trait> {
            Box::new((*self).clone())
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    };
}

#[macro_export]
macro_rules! impl_default_video_filter {
    ($type:ty) => {
        $crate::impl_default_filter!($type, $crate::filters::traits::VideoFilter);
    };
}

#[macro_export]
macro_rules! impl_default_audio_filter {
    ($type:ty) => {
        $crate::impl_default_filter!($type, $crate::filters::traits::AudioFilter);
    };
}

#[macro_export]
macro_rules! impl_default_subtitle_filter {
    ($type:ty) => {
        $crate::impl_default_filter!($type, $crate::filters::traits::SubtitleFilter);
    };
}

#[macro_export]
macro_rules! impl_default_global_filter {
    ($type:ty) => {
        $crate::impl_default_filter!($type, $crate::filters::traits::GlobalFilter);
    };
}
