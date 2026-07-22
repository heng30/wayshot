use super::{
    audio_track::{AudioSource, UnifiedAudioTracksMixerIterator},
    subtitle_track::{SubtitleSource, UnifiedSubtitleTracksCompositorIterator},
    text_track::{TextSource, UnifiedTextTracksCompositorIterator},
    track::Track,
    unified_mixer::{UnifiedMixerConfig, UnifiedTracksMixerIterator},
    video_track::{UnifiedVideoTracksCompositorIterator, VideoSegmentSourceInfo, VideoSourceInfo},
};
use crate::{
    Error, Result,
    filters::{global::GlobalSpeedFilter, traits::GlobalFilterWrapper},
    tracks::segment::Segment,
};
use std::{slice::Iter, sync::Arc, time::Duration};

#[derive(Debug, Clone, Default)]
pub struct Manager {
    pub duration: Duration,
    pub tracks: Vec<Track>,
    pub global_filters: Vec<Arc<GlobalFilterWrapper>>,
}

impl Manager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> Iter<'_, Track> {
        self.tracks.iter()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    pub fn add_track(&mut self, track: Track) -> usize {
        let track_duration = match &track {
            Track::Video(inner) => inner.track.duration,
            Track::Audio(inner) => inner.track.duration,
            Track::Subtitle(inner) => inner.track.duration,
            Track::Image(inner) => inner.track.duration,
            Track::Text(inner) => inner.track.duration,
        };

        if track_duration > self.duration {
            self.duration = track_duration;
        }

        // 根据优先级找到正确的插入位置
        let insert_idx = self.calc_track_position(&track);

        self.tracks.insert(insert_idx, track);
        insert_idx
    }

    pub fn calc_track_position(&self, track: &Track) -> usize {
        let priority = track.priority();
        self.tracks
            .iter()
            .position(|t| t.priority() > priority)
            .unwrap_or(self.tracks.len())
    }

    pub fn insert_track(&mut self, index: usize, track: Track) -> Result<usize> {
        if index > self.tracks.len() {
            return Err(Error::IndexOutOfBounds(index, self.tracks.len()));
        }

        let actual_index = self.find_valid_insert_position(index, &track);

        let track_duration = match &track {
            Track::Video(inner) => inner.track.duration,
            Track::Audio(inner) => inner.track.duration,
            Track::Subtitle(inner) => inner.track.duration,
            Track::Image(inner) => inner.track.duration,
            Track::Text(inner) => inner.track.duration,
        };

        if track_duration > self.duration {
            self.duration = track_duration;
        }

        self.tracks.insert(actual_index, track);
        Ok(actual_index)
    }

    pub fn remove_track(&mut self, index: usize) -> Result<()> {
        if index >= self.tracks.len() {
            return Err(Error::IndexOutOfBounds(index, self.tracks.len()));
        }

        self.tracks.remove(index);
        self.update_duration();

        Ok(())
    }

    pub fn update_duration(&mut self) {
        self.duration = self
            .tracks
            .iter()
            .map(|track| match track {
                Track::Video(inner) => inner.track.duration,
                Track::Audio(inner) => inner.track.duration,
                Track::Subtitle(inner) => inner.track.duration,
                Track::Image(inner) => inner.track.duration,
                Track::Text(inner) => inner.track.duration,
            })
            .max()
            .unwrap_or(Duration::ZERO);
    }

    pub fn move_track(&mut self, from_index: usize, to_index: usize) -> Result<()> {
        if from_index >= self.tracks.len() {
            return Err(Error::IndexOutOfBounds(from_index, self.tracks.len()));
        }
        if to_index >= self.tracks.len() {
            return Err(Error::IndexOutOfBounds(to_index, self.tracks.len()));
        }

        if !self.can_move_track(from_index, to_index) {
            return Err(Error::InvalidConfig(
                "Cannot move track: would violate track priority ordering constraint".to_string(),
            ));
        }

        if from_index != to_index {
            let track = self.tracks.remove(from_index);
            self.tracks.insert(to_index, track);
        }

        Ok(())
    }

    // 检查是否可以将轨道从 from_index 移动到 to_index
    pub fn can_move_track(&self, from_index: usize, to_index: usize) -> bool {
        if from_index == to_index {
            return true;
        }

        let Some(moving_track) = self.tracks.get(from_index) else {
            return false;
        };

        // 相同优先级的轨道可以自由互换
        if let Some(target_track) = self.tracks.get(to_index)
            && moving_track.priority() == target_track.priority()
        {
            return true;
        }

        // 构建移动后的轨道列表（模拟）
        // 检查移动后是否满足优先级顺序（优先级数值应递增或相等）
        let mut simulated_tracks: Vec<_> = self.tracks.iter().collect();
        let track = simulated_tracks.remove(from_index);
        simulated_tracks.insert(to_index, track);

        for window in simulated_tracks.windows(2) {
            if window[0].priority() > window[1].priority() {
                return false;
            }
        }

        true
    }

    // 计算轨道可以插入的有效位置范围
    pub fn valid_insert_range(&self, track: &Track) -> (usize, usize) {
        let priority = track.priority();

        // min_idx: 第一个优先级 >= 当前轨道优先级的位置
        // 这样保证插入位置之后的轨道优先级不比当前高（数值不更小）
        // 如果没有这样的轨道，则插入到末尾
        let min_idx = self
            .tracks
            .iter()
            .position(|t| t.priority() >= priority)
            .unwrap_or(self.tracks.len());

        // max_idx: 最后一个优先级 <= 当前轨道优先级的位置之后
        // 这样保证插入位置之前的轨道优先级不比当前低（数值不更大）
        // 如果没有这样的轨道，则 max_idx = 0（只能插入到最前面）
        let max_idx = self.tracks.len()
            - self
                .tracks
                .iter()
                .rev()
                .position(|t| t.priority() <= priority)
                .unwrap_or(self.tracks.len());

        (min_idx, max_idx)
    }

    // 检查是否可以在指定位置插入轨道
    pub fn can_insert_track_at(&self, index: usize, track: &Track) -> bool {
        if index > self.tracks.len() {
            return false;
        }

        let (min_idx, max_idx) = self.valid_insert_range(track);
        index >= min_idx && index <= max_idx
    }

    /// 查找最近的可用插入位置。如果请求的位置有效，直接返回
    /// 否则向上查找（返回 min_idx），再向下查找（返回 max_idx）
    pub fn find_valid_insert_position(&self, requested_index: usize, track: &Track) -> usize {
        let (min_idx, max_idx) = self.valid_insert_range(track);

        // 如果请求的位置在有效范围内，直接使用
        if requested_index >= min_idx && requested_index <= max_idx {
            return requested_index;
        }

        // requested_index < min_idx：向上查找返回 min_idx
        // requested_index > max_idx：向下查找返回 max_idx
        if requested_index < min_idx {
            min_idx
        } else {
            max_idx
        }
    }

    // 轨道自动排序，确保优先级顺序正确
    pub fn sort_tracks_by_priority(&mut self) {
        self.tracks.sort_by_key(|t| t.priority());
    }

    // 计算轨道在相同优先级组内可以移动到的最顶部位置
    pub fn priority_group_top(&self, track_index: usize) -> Option<usize> {
        let track = self.tracks.get(track_index)?;
        let priority = track.priority();

        // 找第一个优先级 == 当前轨道优先级的位置
        // 因为轨道按优先级排序，相同优先级的轨道是连续的
        let group_start = self
            .tracks
            .iter()
            .position(|t| t.priority() == priority)
            .unwrap_or(track_index);

        Some(group_start)
    }

    // 计算轨道在相同优先级组内可以移动到的最底部位置
    pub fn priority_group_bottom(&self, track_index: usize) -> Option<usize> {
        let track = self.tracks.get(track_index)?;
        let priority = track.priority();

        // 找最后一个优先级 == 当前轨道优先级的位置
        let group_end = self
            .tracks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, t)| t.priority() == priority)
            .map(|(i, _)| i)
            .unwrap_or(track_index);

        Some(group_end)
    }

    pub fn unified_audio_tracks_mixer_iter(
        &self,
        start_timestamp: Duration,
        cache_duration: Duration,
        max_cache_duration: Duration,
        request_samples_duration: Duration,
        output_channels: Option<u16>,
        output_sample_rate: Option<u32>,
    ) -> Result<UnifiedAudioTracksMixerIterator> {
        let mut sources = Vec::new();

        for track in &self.tracks {
            match track {
                Track::Audio(audio_track) => {
                    if !audio_track.hiding {
                        sources.push(AudioSource::Audio(audio_track.clone()));
                    }
                }
                Track::Video(video_track) => {
                    if !video_track.hiding && video_track.has_audio_in_any_segment() {
                        sources.push(AudioSource::VideoWithAudio(video_track.clone()));
                    }
                }
                _ => {}
            }
        }

        let (channels, sample_rate) = if output_channels.is_none() || output_sample_rate.is_none() {
            let (detected_channels, detected_sample_rate) = sources
                .iter()
                .filter_map(|source| source.metadata().audios.first())
                .fold((0, 0), |(max_channels, max_rate), audio_meta| {
                    (
                        max_channels.max(audio_meta.channels),
                        max_rate.max(audio_meta.sample_rate),
                    )
                });

            (
                output_channels.unwrap_or(detected_channels),
                output_sample_rate.unwrap_or(detected_sample_rate),
            )
        } else {
            (output_channels.unwrap(), output_sample_rate.unwrap())
        };

        if channels == 0 || sample_rate == 0 {
            return Err(Error::InvalidConfig(format!(
                "Invalid audio output parameters: channels={}, sample_rate={}. Please provide valid values > 0, or None for auto-detection",
                channels, sample_rate
            )));
        }

        log::info!(
            "Audio output format: channels={} ({}), sample_rate={} Hz ({})",
            channels,
            if output_channels.is_some() {
                "user"
            } else {
                "auto"
            },
            sample_rate,
            if output_sample_rate.is_some() {
                "user"
            } else {
                "auto"
            }
        );

        UnifiedAudioTracksMixerIterator::new(
            sources,
            start_timestamp,
            cache_duration,
            max_cache_duration,
            channels,
            sample_rate,
            request_samples_duration,
        )
    }

    // 创建视频合成迭代器，使用自适应缓存时间
    pub fn unified_video_tracks_compositor_iter(
        &self,
        timeline_offset: Duration,    //开始播放的时间
        cache_duration: Duration,     //每次获取缓存的时间长度
        max_cache_duration: Duration, //最大缓存时间，避免单次加载过多导致卡顿
        output_width: Option<u32>,
        output_height: Option<u32>,
        output_fps: Option<f32>,
    ) -> Result<UnifiedVideoTracksCompositorIterator> {
        let mut source_infos = Vec::new();

        for (index, track) in self.tracks.iter().enumerate() {
            match track {
                Track::Video(video_track) if !video_track.hiding => {
                    let segments: Vec<VideoSegmentSourceInfo> = video_track
                        .track
                        .segments
                        .iter()
                        .enumerate()
                        .filter_map(|(seg_index, segment)| {
                            let metadata = &segment.metadata;
                            metadata.videos.first().map(|video| {
                                VideoSegmentSourceInfo::new(
                                    Some(metadata.path.clone()),
                                    Some(video.fps),
                                    segment.clone(),
                                    seg_index,
                                )
                            })
                        })
                        .collect();

                    if !segments.is_empty() {
                        source_infos.push(VideoSourceInfo {
                            track_index: index,
                            segments,
                        });
                    }
                }
                Track::Image(image_track) if !image_track.hiding => {
                    let segments: Vec<VideoSegmentSourceInfo> = image_track
                        .track
                        .segments
                        .iter()
                        .enumerate()
                        .map(|(seg_index, segment)| {
                            let metadata = &segment.metadata;
                            VideoSegmentSourceInfo::new(
                                Some(metadata.path.clone()),
                                None, // fps 使用输出帧率
                                segment.clone(),
                                seg_index,
                            )
                        })
                        .collect();

                    if !segments.is_empty() {
                        source_infos.push(VideoSourceInfo {
                            track_index: index,
                            segments,
                        });
                    }
                }
                _ => {}
            }
        }

        // 按 track_index 排序确保层级顺序（索引小的在上层，先处理）
        source_infos.sort_by_key(|info| info.track_index);

        let (output_width, output_height, output_fps) =
            self.detected_video_info(output_width, output_height, output_fps)?;

        UnifiedVideoTracksCompositorIterator::new(
            source_infos,
            timeline_offset,
            cache_duration,
            max_cache_duration,
            output_width,
            output_height,
            output_fps,
        )
    }

    pub fn unified_subtitle_tracks_compositor_iter(
        &self,
        timeline_offset: Duration,
    ) -> Result<UnifiedSubtitleTracksCompositorIterator> {
        let mut sources = Vec::new();

        for (index, track) in self.tracks.iter().enumerate() {
            match track {
                Track::Subtitle(subtitle_track) if !subtitle_track.hiding => {
                    sources.push(SubtitleSource {
                        track_index: index,
                        track: subtitle_track.clone(),
                    });
                }
                _ => {}
            }
        }

        UnifiedSubtitleTracksCompositorIterator::new(sources, timeline_offset)
    }

    pub fn unified_text_tracks_compositor_iter(
        &self,
        timeline_offset: Duration,
    ) -> UnifiedTextTracksCompositorIterator {
        let mut sources = Vec::new();

        for (index, track) in self.tracks.iter().enumerate() {
            if let Track::Text(text_track) = track
                && !text_track.hiding
            {
                sources.push(TextSource {
                    track_index: index,
                    track: text_track.clone(),
                });
            }
        }

        UnifiedTextTracksCompositorIterator::new(sources, timeline_offset)
    }

    // Create a unified iterator that combines video, audio, and subtitle tracks.
    pub fn unified_tracks_mixer_iter(
        &self,
        timeline_offset: Duration,
        cache_duration: Duration,
        max_cache_duration: Duration,
        output_width: Option<u32>,
        output_height: Option<u32>,
        output_fps: Option<f32>,
    ) -> Result<UnifiedTracksMixerIterator> {
        self.unified_tracks_mixer_iter_with_config(
            UnifiedMixerConfig::default()
                .with_timeline_offset(timeline_offset)
                .with_cache_duration(cache_duration)
                .with_max_cache_duration(max_cache_duration)
                .with_output_width(output_width)
                .with_output_height(output_height)
                .with_output_fps(output_fps),
        )
    }

    pub fn unified_tracks_mixer_iter_with_config(
        &self,
        config: UnifiedMixerConfig,
    ) -> Result<UnifiedTracksMixerIterator> {
        let has_video = self.tracks.iter().any(|track| match track {
            Track::Video(video_track) if !video_track.hiding => true,
            Track::Image(image_track) if !image_track.hiding => true,
            _ => false,
        });

        let has_text = self
            .tracks
            .iter()
            .any(|track| matches!(track, Track::Text(text_track) if !text_track.hiding));

        let video_iter = if has_video {
            self.unified_video_tracks_compositor_iter(
                config.timeline_offset,
                config.cache_duration,
                config.max_cache_duration,
                config.output_width,
                config.output_height,
                config.output_fps,
            )
            .ok()
        } else {
            None
        };

        let has_audio = self.tracks.iter().any(|track| match track {
            Track::Audio(audio_track) if !audio_track.hiding => true,
            Track::Video(video_track) => {
                !video_track.hiding && video_track.has_audio_in_any_segment()
            }
            _ => false,
        });

        let audio_iter = if has_audio {
            Some(self.unified_audio_tracks_mixer_iter(
                config.timeline_offset,
                config.cache_duration,
                config.max_cache_duration,
                Duration::from_secs(1),    // 调用next，每次返回最大1秒的数据
                config.output_channels,    // 输出声道数
                config.output_sample_rate, // 输出采样率
            )?)
        } else {
            None
        };

        let has_subtitle = self.tracks.iter().any(
            |track| matches!(track, Track::Subtitle(subtitle_track) if !subtitle_track.hiding),
        );

        let subtitle_iter = if has_subtitle {
            self.unified_subtitle_tracks_compositor_iter(config.timeline_offset)
                .ok()
        } else {
            None
        };

        let text_iter = if has_text {
            Some(self.unified_text_tracks_compositor_iter(config.timeline_offset))
        } else {
            None
        };

        let output_fps = if let Some(ref iter) = video_iter {
            iter.output_fps
        } else {
            config.output_fps.unwrap_or(25.0)
        };

        let duration = config.duration.unwrap_or(self.duration);

        Ok(super::unified_mixer::UnifiedTracksMixerIterator::new(
            video_iter,
            audio_iter,
            subtitle_iter,
            text_iter,
            config.timeline_offset,
            output_fps,
            duration,
            self.global_filters.clone(),
        ))
    }

    // 自动检测输出参数：从所有视频轨道中选择最大值
    fn detected_video_info(
        &self,
        output_width: Option<u32>,
        output_height: Option<u32>,
        output_fps: Option<f32>,
    ) -> Result<(u32, u32, f32)> {
        let (output_width, output_height, output_fps) =
            if output_width.is_none() || output_height.is_none() || output_fps.is_none() {
                let (detected_width, detected_height, detected_fps) = self
                    .tracks
                    .iter()
                    .filter_map(|track| match track {
                        Track::Video(video_track) if !video_track.hiding => {
                            video_track.track.metadata.videos.first()
                        }
                        _ => None,
                    })
                    .fold(
                        (0u32, 0u32, 0.0f32),
                        |(max_width, max_height, max_fps), video_meta| {
                            (
                                max_width.max(video_meta.width),
                                max_height.max(video_meta.height),
                                max_fps.max(video_meta.fps),
                            )
                        },
                    );
                (
                    output_width.unwrap_or(detected_width),
                    output_height.unwrap_or(detected_height),
                    output_fps.unwrap_or(detected_fps),
                )
            } else {
                (
                    output_width.unwrap(),
                    output_height.unwrap(),
                    output_fps.unwrap(),
                )
            };

        if output_width == 0 || output_height == 0 || output_fps == 0.0 {
            return Err(Error::InvalidConfig(
                "No valid video tracks found".to_string(),
            ));
        }

        Ok((output_width, output_height, output_fps))
    }

    // 获取指定时间段内的所有 (track_index segment)
    pub fn get_segments_span(
        &self,
        from_timeline_offset: Duration,
        to_timeline_offset: Duration,
    ) -> Vec<(usize, Arc<Segment>)> {
        let mut segments = Vec::new();

        for (track_index, track) in self.tracks.iter().enumerate() {
            let track_segments: Vec<Arc<Segment>> = match track {
                Track::Video(video_track) => video_track
                    .track
                    .segments
                    .iter()
                    .filter(|segment| {
                        if segment.hiding {
                            return false;
                        }

                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;
                        segment_start <= to_timeline_offset && segment_end > from_timeline_offset
                    })
                    .cloned()
                    .collect(),
                Track::Audio(audio_track) => audio_track
                    .track
                    .segments
                    .iter()
                    .filter(|segment| {
                        if segment.hiding {
                            return false;
                        }

                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;
                        segment_start <= to_timeline_offset && segment_end > from_timeline_offset
                    })
                    .cloned()
                    .collect(),
                Track::Subtitle(subtitle_track) => subtitle_track
                    .track
                    .segments
                    .iter()
                    .filter(|segment| {
                        if segment.hiding {
                            return false;
                        }

                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;
                        segment_start <= to_timeline_offset && segment_end > from_timeline_offset
                    })
                    .cloned()
                    .collect(),
                Track::Image(image_track) => image_track
                    .track
                    .segments
                    .iter()
                    .filter(|segment| {
                        if segment.hiding {
                            return false;
                        }

                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;
                        segment_start <= to_timeline_offset && segment_end > from_timeline_offset
                    })
                    .cloned()
                    .collect(),
                Track::Text(text_track) => text_track
                    .track
                    .segments
                    .iter()
                    .filter(|segment| {
                        if segment.hiding {
                            return false;
                        }

                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;
                        segment_start <= to_timeline_offset && segment_end > from_timeline_offset
                    })
                    .cloned()
                    .collect(),
            };

            for segment in track_segments {
                segments.push((track_index, segment));
            }
        }

        segments
    }

    pub fn add_global_filter(&mut self, filter: Arc<GlobalFilterWrapper>) -> usize {
        self.global_filters.push(filter);
        self.global_filters.len() - 1
    }

    pub fn remove_global_filter(&mut self, index: usize) -> Result<()> {
        if index >= self.global_filters.len() {
            return Err(Error::IndexOutOfBounds(index, self.global_filters.len()));
        }
        self.global_filters.remove(index);
        Ok(())
    }

    pub fn get_global_filters(&self) -> &[Arc<GlobalFilterWrapper>] {
        &self.global_filters
    }

    pub fn clear_global_filters(&mut self) {
        self.global_filters.clear();
    }

    pub fn get_global_speed(&self) -> f32 {
        self.global_filters
            .iter()
            .find(|f| f.inner.name() == GlobalSpeedFilter::NAME && f.enabled())
            .and_then(|f| f.inner.as_any().downcast_ref::<GlobalSpeedFilter>())
            .map(|f| f.speed)
            .unwrap_or(1.0)
    }
}

impl<'a> IntoIterator for &'a Manager {
    type Item = &'a Track;
    type IntoIter = Iter<'a, Track>;

    fn into_iter(self) -> Self::IntoIter {
        self.tracks.iter()
    }
}

impl<'a> IntoIterator for &'a mut Manager {
    type Item = &'a mut Track;
    type IntoIter = std::slice::IterMut<'a, Track>;

    fn into_iter(self) -> Self::IntoIter {
        self.tracks.iter_mut()
    }
}

impl IntoIterator for Manager {
    type Item = Track;
    type IntoIter = std::vec::IntoIter<Track>;

    fn into_iter(self) -> Self::IntoIter {
        self.tracks.into_iter()
    }
}
