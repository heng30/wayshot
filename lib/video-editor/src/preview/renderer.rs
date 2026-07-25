use super::{
    PreviewConfig,
    playback::{PlaybackController, PlaybackSpeed, PlaybackState},
};
use crate::{
    Error, Result,
    filters::traits::SubtitleEntry,
    tracks::{
        Track,
        audio_track::AudioSamples,
        frame_position::TimeToFrameConverter,
        manager::Manager,
        segment::Segment,
        subtitle_track::create_subtitle_layer_frame,
        text_track::create_text_layer_frame,
        unified_mixer::{
            UnifiedFrame, UnifiedFrameSubtitle, UnifiedFrameText, UnifiedTracksMixerIterator,
        },
        video_track::{LayerFrame, LayerFrames, apply_global_filters, composite_frame},
    },
};
use audio_utils::audio_level::calc_rms_level;
use image::RgbaImage;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, buffer::SamplesBuffer};
use std::{
    collections::{HashMap, VecDeque},
    num::NonZero,
    sync::{Arc, Mutex},
    time::Duration,
};

type AudioSink = Player;

// 字幕帧缓存 key：同一 segment 在相同分辨率下的渲染结果不变
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SubtitleCacheKey {
    track_index: usize,
    segment_index: usize,
    output_width: u32,
    output_height: u32,
}

// 文本帧缓存 key：没有关键帧的文本 segment 在相同分辨率下渲染结果不变
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TextCacheKey {
    track_index: usize,
    segment_index: usize,
    output_width: u32,
    output_height: u32,
}

pub struct PreviewRenderer {
    manager: Arc<Manager>,
    controller: PlaybackController,
    config: PreviewConfig,
    time_converter: TimeToFrameConverter,
    mixer_iter: Option<UnifiedTracksMixerIterator>,

    audio_sink: Option<Arc<Mutex<AudioSink>>>,
    audio_stream: Option<MixerDeviceSink>,

    current_frame: Option<LayerFrames>,
    current_audio: Option<AudioSamples>,
    current_subtitle: Vec<(SubtitleEntry, Arc<Segment>)>,

    // 字幕帧缓存：避免每帧重新渲染字幕文本
    subtitle_layer_cache: HashMap<SubtitleCacheKey, LayerFrame>,

    // 文本帧缓存：没有关键帧的文本 segment 缓存，避免每帧重新渲染
    text_layer_cache: HashMap<TextCacheKey, LayerFrame>,

    // 音频数据按帧间隔分割后计算 dB 值并缓存，然后按帧率消耗，用于UI显示
    db_level_cache_left: VecDeque<f32>,
    db_level_cache_right: VecDeque<f32>,
    current_db_level: (f32, f32),
}

impl PreviewRenderer {
    pub fn new(manager: Arc<Manager>, config: PreviewConfig) -> Self {
        let controller = PlaybackController::new(manager.clone(), config.frame_rate());
        let time_converter = TimeToFrameConverter::from_f32(config.frame_rate() as f32);
        Self {
            manager,
            controller,
            config,
            time_converter,
            mixer_iter: None,
            audio_sink: None,
            audio_stream: None,
            current_frame: None,
            current_audio: None,
            current_subtitle: Vec::new(),
            subtitle_layer_cache: HashMap::new(),
            text_layer_cache: HashMap::new(),
            db_level_cache_left: VecDeque::new(),
            db_level_cache_right: VecDeque::new(),
            current_db_level: (-200.0, -200.0), // 默认静音
        }
    }

    pub fn current_frame(&self) -> Option<&RgbaImage> {
        self.current_frame.as_ref().map(|lf| &lf.composited_image)
    }

    pub fn take_frame(&mut self) -> Option<RgbaImage> {
        self.current_frame.take().map(|lf| lf.composited_image)
    }

    pub fn current_layerframe(&self) -> Option<&LayerFrames> {
        self.current_frame.as_ref()
    }

    pub fn take_layerframe(&mut self) -> Option<LayerFrames> {
        self.current_frame.take()
    }

    pub fn current_audio(&self) -> Option<&AudioSamples> {
        self.current_audio.as_ref()
    }

    pub fn take_audio(&mut self) -> Option<AudioSamples> {
        self.current_audio.take()
    }

    pub fn current_subtitle(&self) -> &Vec<(SubtitleEntry, Arc<Segment>)> {
        &self.current_subtitle
    }

    pub fn current_db_level(&self) -> (f32, f32) {
        self.current_db_level
    }

    // 将音频数据按帧间隔分割，计算每个块的 dB 值并缓存
    fn process_audio_for_db_cache(&mut self, audio: &AudioSamples) {
        let fps = self.config.frame_rate();
        if fps <= 0.0 || audio.sample_rate == 0 || audio.channels == 0 {
            return;
        }

        // 计算每帧对应的采样数 (samples per frame per channel)
        // e.g., 48000 Hz / 25 fps = 1920 samples per frame (per channel)
        let samples_per_frame_per_channel = (audio.sample_rate as f64 / fps) as usize;

        if samples_per_frame_per_channel == 0 {
            return;
        }

        let channels = audio.channels as usize;

        // 根据声道数分别处理
        if channels == 1 {
            // 单声道：直接按帧间隔分割计算
            for chunk in audio.samples.chunks(samples_per_frame_per_channel) {
                if let Some(db) = calc_rms_level(chunk) {
                    self.db_level_cache_left.push_back(db);
                    self.db_level_cache_right.push_back(db);
                }
            }
        } else {
            // 多声道（通常是立体声）：分离左右声道分别计算
            for chunk in audio
                .samples
                .chunks(samples_per_frame_per_channel * channels)
            {
                let mut left_samples = Vec::with_capacity(samples_per_frame_per_channel);
                let mut right_samples = Vec::with_capacity(samples_per_frame_per_channel);

                // 交错采样：[L0, R0, L1, R1, ...]
                for frame_samples in chunk.chunks(channels) {
                    left_samples.push(frame_samples[0]);
                    right_samples.push(frame_samples.get(1).copied().unwrap_or(frame_samples[0]));
                }

                if let Some(db) = calc_rms_level(&left_samples) {
                    self.db_level_cache_left.push_back(db);
                }

                if let Some(db) = calc_rms_level(&right_samples) {
                    self.db_level_cache_right.push_back(db);
                }
            }
        }
    }

    fn consume_db_from_cache(&mut self) {
        let left = self.db_level_cache_left.pop_front().unwrap_or(-200.0);
        let right = self.db_level_cache_right.pop_front().unwrap_or(-200.0);
        self.current_db_level = (left, right);
    }

    fn subtitle_layer(video_frame: &mut LayerFrames, subtitle_layer: LayerFrame) {
        video_frame.layers.insert(0, subtitle_layer.clone());
        composite_frame(&mut video_frame.composited_image, &subtitle_layer.image);
    }

    fn text_layer(video_frame: &mut LayerFrames, text_layer: LayerFrame) {
        video_frame.layers.insert(0, text_layer.clone());
        composite_frame(&mut video_frame.composited_image, &text_layer.image);
    }

    fn create_transparent_layer_frames(width: u32, height: u32, offset: Duration) -> LayerFrames {
        LayerFrames {
            layers: Vec::new(),
            composited_image: RgbaImage::new(width, height),
            relative_timeline_offset: offset,
        }
    }

    /// 获取或创建字幕图层（带缓存）
    fn get_or_create_subtitle_layer(
        &mut self,
        sub: &UnifiedFrameSubtitle,
        output_width: u32,
        output_height: u32,
    ) -> Option<LayerFrame> {
        let cache_key = SubtitleCacheKey {
            track_index: sub.track_index,
            segment_index: sub.segment_index,
            output_width,
            output_height,
        };

        if let Some(cached) = self.subtitle_layer_cache.get(&cache_key) {
            return Some(cached.clone());
        }

        let layer = create_subtitle_layer_frame(
            &sub.subtitle,
            sub.segment.clone(),
            sub.segment_index,
            sub.track_index,
            output_width,
            output_height,
        )
        .ok()?;

        self.subtitle_layer_cache.insert(cache_key, layer.clone());
        Some(layer)
    }

    /// 获取或创建文本图层（带缓存，仅对没有关键帧的文本缓存）
    fn get_or_create_text_layer(
        &mut self,
        text_item: &UnifiedFrameText,
        timeline_offset: Duration,
        output_width: u32,
        output_height: u32,
    ) -> Option<LayerFrame> {
        // 有关键帧的文本不缓存，因为每帧的 position/opacity/rotation 可能不同
        if text_item.element.keyframe_tracks.has_keyframes() {
            return create_text_layer_frame(
                &text_item.element,
                text_item.segment.clone(),
                text_item.segment_index,
                text_item.track_index,
                timeline_offset,
                output_width,
                output_height,
            )
            .ok();
        }

        // 没有关键帧的文本，渲染结果在相同分辨率下不变，可以缓存
        let cache_key = TextCacheKey {
            track_index: text_item.track_index,
            segment_index: text_item.segment_index,
            output_width,
            output_height,
        };

        if let Some(cached) = self.text_layer_cache.get(&cache_key) {
            return Some(cached.clone());
        }

        let layer = create_text_layer_frame(
            &text_item.element,
            text_item.segment.clone(),
            text_item.segment_index,
            text_item.track_index,
            timeline_offset,
            output_width,
            output_height,
        )
        .ok()?;

        self.text_layer_cache.insert(cache_key, layer.clone());
        Some(layer)
    }

    pub fn update(&mut self) -> Result<()> {
        if self.controller.state() == PlaybackState::Playing {
            self.advance_frame()?;
        }
        Ok(())
    }

    fn advance_frame(&mut self) -> Result<()> {
        if self.mixer_iter.is_none() {
            let mut mixer_config = self.config.mixer.clone();
            mixer_config.timeline_offset = self.controller.position();
            self.mixer_iter = Some(
                self.manager
                    .unified_tracks_mixer_iter_with_config(mixer_config)?,
            );
        }

        // 设置音频获取策略：如果 audio_sink 不存在，不获取音频
        // 这避免了在 seek 时预取音频数据，确保音频和视频时间同步
        if let Some(ref mut mixer_iter) = self.mixer_iter {
            if let Some(ref sink_arc) = self.audio_sink
                && let Ok(sink) = sink_arc.try_lock()
            {
                // 根据队列长度决定是否获取音频
                mixer_iter.set_fetch_audio(sink.len() <= 2);
            } else {
                // 没有音频 sink 时，不获取音频数据
                mixer_iter.set_fetch_audio(false);
            }
        }

        if let Some(iter) = &mut self.mixer_iter {
            if let Some(frame) = iter.next() {
                let UnifiedFrame {
                    layer_frames,
                    audio,
                    subtitle,
                    text,
                    timeline_offset,
                    post_composite_global_filters,
                    duration: frame_duration,
                } = frame;

                let mut video_frame = layer_frames.or_else(|| {
                    // 没有视频帧，就创建透明帧，用于绘制文字或字幕
                    if !text.is_empty() || !subtitle.is_empty() {
                        let width = self.config.mixer.output_width.unwrap_or(1920);
                        let height = self.config.mixer.output_height.unwrap_or(1080);
                        Some(Self::create_transparent_layer_frames(
                            width,
                            height,
                            timeline_offset,
                        ))
                    } else {
                        None
                    }
                });

                if let Some(audio) = audio {
                    self.process_audio_for_db_cache(&audio);

                    if let Some(ref sink_arc) = self.audio_sink
                        && let Ok(sink) = sink_arc.try_lock()
                    {
                        let channels = NonZero::new(audio.channels).ok_or_else(|| {
                            Error::InvalidConfig("Audio channels must be non-zero".to_string())
                        })?;
                        let sample_rate = NonZero::new(audio.sample_rate).ok_or_else(|| {
                            Error::InvalidConfig("Audio sample rate must be non-zero".to_string())
                        })?;
                        let source =
                            SamplesBuffer::new(channels, sample_rate, audio.samples.clone());
                        sink.append(source);

                        if sink.is_paused() && self.controller.state() == PlaybackState::Playing {
                            sink.play();
                        }
                    }

                    self.current_audio = Some(audio);
                }

                self.consume_db_from_cache();

                self.apply_frame_overlays(&mut video_frame, &text, &subtitle, timeline_offset);

                // Apply post-composite global filters (e.g., rotation) after subtitle/text
                if let Some(vf) = video_frame.as_mut()
                    && !post_composite_global_filters.is_empty()
                {
                    apply_global_filters(
                        &mut vf.composited_image,
                        &post_composite_global_filters,
                        timeline_offset,
                        frame_duration,
                        true,
                    );
                }

                self.current_frame = video_frame;

                let current_frame = self
                    .time_converter
                    .duration_to_frame(self.controller.position());
                let next_pos = self.time_converter.frame_to_duration(current_frame + 1);
                self.controller.set_position(next_pos);

                if let Some(loop_region) = &self.config.loop_region
                    && !loop_region.contains(self.controller.position())
                {
                    self.controller
                        .set_position(loop_region.clamp(self.controller.position()));
                    self.mixer_iter = None;
                }
            } else {
                // 视频结束时，暂停播放控制器并清空音频缓冲区
                self.controller.pause();
                self.mixer_iter = None;
                self.clear_audio_sink();
            }
        }

        Ok(())
    }

    /// 将 text 和 subtitle 叠加到 video_frame 上，并更新 current_subtitle
    fn apply_frame_overlays(
        &mut self,
        video_frame: &mut Option<LayerFrames>,
        text: &[UnifiedFrameText],
        subtitle: &[UnifiedFrameSubtitle],
        timeline_offset: Duration,
    ) {
        for text_item in text {
            if let Some(video_frame) = video_frame {
                let w = video_frame.composited_image.width();
                let h = video_frame.composited_image.height();

                if let Some(text_layer) =
                    self.get_or_create_text_layer(text_item, timeline_offset, w, h)
                {
                    Self::text_layer(video_frame, text_layer);
                }
            }
        }

        self.current_subtitle.clear();
        for sub in subtitle {
            if let Some(video_frame) = video_frame {
                let w = video_frame.composited_image.width();
                let h = video_frame.composited_image.height();

                if let Some(subtitle_layer) = self.get_or_create_subtitle_layer(sub, w, h) {
                    Self::subtitle_layer(video_frame, subtitle_layer);
                }
            }
            self.current_subtitle
                .push((sub.subtitle.clone(), sub.segment.clone()));
        }
    }

    fn start_audio_playback(&mut self) -> Result<()> {
        if self.audio_sink.is_some() {
            return Ok(());
        }

        let device_sink = DeviceSinkBuilder::open_default_sink().map_err(|e| {
            Error::InvalidConfig(format!("Failed to get audio output stream: {:?}", e))
        })?;

        let sink = AudioSink::connect_new(&device_sink.mixer());
        sink.set_volume(self.controller.volume());

        self.audio_stream = Some(device_sink);
        self.audio_sink = Some(Arc::new(Mutex::new(sink)));

        Ok(())
    }

    fn stop_audio_playback(&mut self) {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                sink.stop();
            }
        }
        self.audio_sink = None;
        self.audio_stream = None;
    }

    fn clear_audio_sink(&mut self) {
        if let Some(sink) = &self.audio_sink
            && let Ok(sink) = sink.lock()
        {
            sink.stop();
            sink.empty();
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.controller.set_volume(volume);

        if let Some(sink) = &self.audio_sink
            && let Ok(sink) = sink.lock()
        {
            sink.set_volume(volume);
        }
    }

    pub fn play(&mut self) -> Result<()> {
        self.controller.play();
        self.start_audio_playback()?;
        self.advance_frame()?;

        if let Some(sink) = &self.audio_sink
            && let Ok(sink) = sink.lock()
            && sink.is_paused()
        {
            sink.set_volume(self.controller.volume());
            sink.play();
        }

        Ok(())
    }

    pub fn pause(&mut self) {
        self.controller.pause();
        if let Some(sink) = &self.audio_sink
            && let Ok(sink) = sink.lock()
        {
            sink.pause();
        }
    }

    pub fn toggle_playback(&mut self) -> Result<()> {
        if self.controller.state() == PlaybackState::Playing {
            self.pause();
            Ok(())
        } else {
            self.play()
        }
    }

    pub fn stop(&mut self) {
        self.controller.stop();
        self.mixer_iter = None;
        self.clear_audio_sink();
        self.stop_audio_playback();
        self.current_frame = None;
        self.current_audio = None;
        self.current_subtitle.clear();
        self.subtitle_layer_cache.clear();
        self.text_layer_cache.clear();
        self.db_level_cache_left.clear();
        self.db_level_cache_right.clear();
        self.current_db_level = (-200.0, -200.0);
    }

    pub fn seek(&mut self, position: Duration) -> Result<()> {
        self.controller.set_position(position);
        self.clear_audio_sink();
        self.db_level_cache_left.clear();
        self.db_level_cache_right.clear();
        self.subtitle_layer_cache.clear();

        // 创建 mixer_iter 并获取视频帧用于预览，但不推进 position
        // 这样 play() 时会从正确的位置开始获取音频和视频
        let mut mixer_config = self.config.mixer.clone();
        mixer_config.timeline_offset = position;
        self.mixer_iter = Some(
            self.manager
                .unified_tracks_mixer_iter_with_config(mixer_config)?,
        );

        // 禁用音频获取（因为 sink 不存在）
        if let Some(ref mut mixer_iter) = self.mixer_iter {
            mixer_iter.set_fetch_audio(false);
        }

        // 获取帧用于预览显示
        if let Some(iter) = &mut self.mixer_iter
            && let Some(frame) = iter.next()
        {
            let UnifiedFrame {
                layer_frames,
                audio: _,
                subtitle,
                text,
                timeline_offset,
                post_composite_global_filters,
                duration: frame_duration,
            } = frame;

            let mut video_frame = layer_frames.or_else(|| {
                if !text.is_empty() || !subtitle.is_empty() {
                    let width = self.config.mixer.output_width.unwrap_or(1920);
                    let height = self.config.mixer.output_height.unwrap_or(1080);
                    Some(Self::create_transparent_layer_frames(
                        width,
                        height,
                        timeline_offset,
                    ))
                } else {
                    None
                }
            });

            self.apply_frame_overlays(&mut video_frame, &text, &subtitle, timeline_offset);

            // Apply post-composite global filters (e.g., rotation) after subtitle/text
            if let Some(vf) = video_frame.as_mut()
                && !post_composite_global_filters.is_empty()
            {
                apply_global_filters(
                    &mut vf.composited_image,
                    &post_composite_global_filters,
                    timeline_offset,
                    frame_duration,
                    true,
                );
            }

            self.current_frame = video_frame;
        }

        // 重置 mixer_iter，这样 play() 会从头开始
        self.mixer_iter = None;

        // 当前播放位置后是否还有视频数据
        let before_max_video_track_duration = self.manager.iter().any(|track| {
            matches!(track, Track::Video(v) if !v.hiding) && track.duration() > position
        });

        if self.current_frame.is_none() && before_max_video_track_duration {
            let frame_duration = Duration::from_secs_f64(1.0 / self.config.frame_rate());
            for offset in 1..10 {
                let fallback_position = position.saturating_sub(frame_duration * offset);
                if fallback_position.is_zero() && offset > 1 {
                    break;
                }

                self.controller.set_position(fallback_position);

                let mut mixer_config = self.config.mixer.clone();
                mixer_config.timeline_offset = fallback_position;
                self.mixer_iter = Some(
                    self.manager
                        .unified_tracks_mixer_iter_with_config(mixer_config)?,
                );

                if let Some(ref mut mixer_iter) = self.mixer_iter {
                    mixer_iter.set_fetch_audio(false);
                }

                if let Some(iter) = &mut self.mixer_iter
                    && let Some(frame) = iter.next()
                {
                    self.current_frame = frame.layer_frames;
                    if self.current_frame.is_some() {
                        self.controller.set_position(position);
                        break;
                    }
                }
            }
        }

        self.mixer_iter = None;

        Ok(())
    }

    pub fn step_forward(&mut self) -> Result<()> {
        self.advance_frame()
    }

    pub fn step_backward(&mut self) -> Result<()> {
        self.controller.step_backward();
        self.clear_audio_sink();
        self.db_level_cache_left.clear();
        self.db_level_cache_right.clear();

        let mut mixer_config = self.config.mixer.clone();
        mixer_config.timeline_offset = self.controller.position();
        self.mixer_iter = Some(
            self.manager
                .unified_tracks_mixer_iter_with_config(mixer_config)?,
        );

        if let Some(iter) = &mut self.mixer_iter {
            if let Some(frame) = iter.next() {
                let UnifiedFrame {
                    layer_frames,
                    audio,
                    subtitle,
                    text,
                    timeline_offset,
                    post_composite_global_filters,
                    duration: frame_duration,
                } = frame;

                let mut video_frame = layer_frames.or_else(|| {
                    if !text.is_empty() || !subtitle.is_empty() {
                        let width = self.config.mixer.output_width.unwrap_or(1920);
                        let height = self.config.mixer.output_height.unwrap_or(1080);
                        Some(Self::create_transparent_layer_frames(
                            width,
                            height,
                            timeline_offset,
                        ))
                    } else {
                        None
                    }
                });

                if let Some(audio) = audio {
                    self.process_audio_for_db_cache(&audio);

                    if let Some(ref sink_arc) = self.audio_sink
                        && let Ok(sink) = sink_arc.try_lock()
                    {
                        let channels = NonZero::new(audio.channels).ok_or_else(|| {
                            Error::InvalidConfig("Audio channels must be non-zero".to_string())
                        })?;
                        let sample_rate = NonZero::new(audio.sample_rate).ok_or_else(|| {
                            Error::InvalidConfig("Audio sample rate must be non-zero".to_string())
                        })?;
                        let source =
                            SamplesBuffer::new(channels, sample_rate, audio.samples.clone());
                        sink.append(source);

                        if sink.is_paused() && self.controller.state() == PlaybackState::Playing {
                            sink.play();
                        }
                    }
                    self.current_audio = Some(audio);
                }

                self.consume_db_from_cache();

                self.apply_frame_overlays(&mut video_frame, &text, &subtitle, timeline_offset);

                // Apply post-composite global filters (e.g., rotation) after subtitle/text
                if let Some(vf) = video_frame.as_mut()
                    && !post_composite_global_filters.is_empty()
                {
                    apply_global_filters(
                        &mut vf.composited_image,
                        &post_composite_global_filters,
                        timeline_offset,
                        frame_duration,
                        true,
                    );
                }

                self.current_frame = video_frame;
            } else {
                // 视频结束时，暂停播放控制器并清空音频缓冲区
                self.controller.pause();
                self.mixer_iter = None;
                self.clear_audio_sink();
            }
        }

        Ok(())
    }

    pub fn skip_forward(&mut self, seconds: f64) -> Result<()> {
        let duration = Duration::from_secs_f64(seconds);
        let new_pos = self.controller.position() + duration;
        let clamped = new_pos.min(self.manager.duration);
        self.seek(clamped)
    }

    pub fn skip_backward(&mut self, seconds: f64) -> Result<()> {
        let duration = Duration::from_secs_f64(seconds);
        let new_pos = self.controller.position().saturating_sub(duration);
        self.seek(new_pos)
    }

    pub fn jump_to_percentage(&mut self, percentage: f64) -> Result<()> {
        let clamped = percentage.clamp(0.0, 100.0);
        let duration = self.manager.duration;
        let new_pos = Duration::from_secs_f64(duration.as_secs_f64() * clamped / 100.0);
        self.seek(new_pos)
    }

    pub fn state(&self) -> PlaybackState {
        self.controller.state()
    }

    pub fn is_playing(&self) -> bool {
        self.controller.state() == PlaybackState::Playing
    }

    pub fn position(&self) -> Duration {
        self.controller.position()
    }

    pub fn duration(&self) -> Duration {
        self.manager.duration
    }

    pub fn progress(&self) -> f64 {
        self.controller.percentage()
    }

    pub fn frame_rate(&self) -> f64 {
        self.config.frame_rate()
    }

    pub fn current_frame_number(&self) -> usize {
        self.time_converter
            .duration_to_frame(self.controller.position())
    }

    pub fn total_frames(&self) -> Option<usize> {
        Some(self.time_converter.duration_to_frame(self.manager.duration))
    }

    pub fn speed(&self) -> PlaybackSpeed {
        self.controller.speed()
    }

    pub fn set_speed(&mut self, speed: PlaybackSpeed) -> Result<()> {
        self.controller.set_speed(speed);

        if let Some(sink) = &self.audio_sink
            && let Ok(sink) = sink.lock()
        {
            sink.set_speed(speed.multiplier() as f32);
        }

        Ok(())
    }

    pub fn config(&self) -> &PreviewConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: PreviewConfig) -> Result<()> {
        self.controller.set_frame_rate(config.frame_rate());
        self.time_converter = TimeToFrameConverter::from_f32(config.frame_rate() as f32);
        self.config = config;
        self.mixer_iter = None;
        Ok(())
    }
}

impl Drop for PreviewRenderer {
    fn drop(&mut self) {
        self.stop_audio_playback();
    }
}
