use super::{segment::Segment, track::InnerTrack, video_track::VideoTrack};
use crate::{
    Error, Result, ensure_file_exists,
    filters::traits::{AudioData, AudioFilterConfig},
    metadata::Metadata,
    tracks::segment::SegmentSamples,
};
use audio_utils::{
    audio::{resample_audio_to_target_samples_option, resample_audio_with_channel},
    time_stretch_preserving_pitch,
};
use crossbeam::channel::{self, Receiver, Sender};
use ffmpeg_next as ffmpeg;
use std::{
    iter::repeat_n,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct AudioSamples {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct AudioTrack {
    pub name: String,
    pub hiding: bool,
    pub locked: bool,
    pub track: InnerTrack,
}

impl AudioTrack {
    pub fn new(track: InnerTrack) -> Self {
        Self {
            name: "A".to_string(),
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
}

#[derive(Debug, Clone)]
pub enum AudioSource {
    Audio(Arc<AudioTrack>),
    VideoWithAudio(Arc<VideoTrack>),
}

impl AudioSource {
    pub fn metadata(&self) -> &Arc<Metadata> {
        match self {
            AudioSource::Audio(track) => &track.track.metadata,
            AudioSource::VideoWithAudio(track) => &track.track.metadata,
        }
    }

    pub fn duration(&self) -> Duration {
        match self {
            AudioSource::Audio(track) => track.track.duration,
            AudioSource::VideoWithAudio(track) => track.track.duration,
        }
    }

    pub fn segments(&self) -> &[Arc<Segment>] {
        match self {
            AudioSource::Audio(track) => &track.track.segments,
            AudioSource::VideoWithAudio(track) => &track.track.segments,
        }
    }
}

#[derive(Debug, Clone)]
struct AudioSourceInfo {
    pub segments: Vec<SegmentSourceInfo>,
    pub muted: bool,
}

#[derive(Debug, Clone)]
struct SegmentSourceInfo {
    pub path: PathBuf,
    pub stream_index: usize,
    pub channels: u16,
    pub sample_rate: u32,
    pub segment: Arc<Segment>,
}

#[derive(Debug)]
pub struct UnifiedAudioTracksMixerIterator {
    pub sources: Vec<AudioSource>,
    pub timeline_offset: Duration,
    pub cache_duration: Duration,
    pub max_cache_duration: Duration, // 最大缓存时间
    pub channels: u16,
    pub sample_rate: u32,

    cache_samples: Vec<f32>,
    request_samples_duration: Duration, // 每次 next 调用返回的音频时长
    next_cache_timeline_offset: Duration, // 下一次要加载的位置，基于时间轴
    receiver: Receiver<(Vec<f32>, Duration)>,
    sender: Sender<(Vec<f32>, Duration)>,
    is_loading: Arc<AtomicBool>,

    reached_end: bool,
    end_timeline_offset: Duration,
    remained_timeline_duration: Duration,
}

impl UnifiedAudioTracksMixerIterator {
    pub fn new(
        sources: Vec<AudioSource>,
        timeline_offset: Duration,
        cache_duration: Duration,
        max_cache_duration: Duration,
        channels: u16,
        sample_rate: u32,
        request_samples_duration: Duration,
    ) -> Result<Self> {
        let end_timeline_offset = sources
            .iter()
            .flat_map(|source| source.segments().iter())
            .filter(|seg| !seg.hiding)
            .map(|seg| seg.timeline_offset + seg.duration)
            .max()
            .unwrap_or(Duration::ZERO);

        if channels == 0 {
            return Err(Error::InvalidConfig(
                "channels must be greater than 0".into(),
            ));
        }
        if sample_rate == 0 {
            return Err(Error::InvalidConfig(
                "sample_rate must be greater than 0".into(),
            ));
        }
        if request_samples_duration == Duration::ZERO {
            return Err(Error::InvalidConfig(
                "request_samples_duration must be greater than 0".into(),
            ));
        }
        if max_cache_duration <= cache_duration {
            return Err(Error::InvalidConfig(
                "max_cache_duration must be greater than cache_duration".into(),
            ));
        }

        let (sender, receiver) = channel::unbounded();
        let remained_timeline_duration = end_timeline_offset.saturating_sub(timeline_offset);

        let mut iter = Self {
            sources,
            timeline_offset,
            cache_duration,
            cache_samples: Vec::new(),
            max_cache_duration,
            channels,
            sample_rate,
            request_samples_duration,
            next_cache_timeline_offset: timeline_offset,
            sender,
            receiver,
            is_loading: Arc::new(AtomicBool::new(false)),
            reached_end: false,
            end_timeline_offset,
            remained_timeline_duration,
        };

        // 只有当存在有效的时间范围时才预加载
        if remained_timeline_duration > Duration::ZERO {
            iter.start_background_loader();
        } else {
            iter.reached_end = true;
        }
        Ok(iter)
    }

    fn start_background_loader(&mut self) {
        if self.reached_end || self.next_cache_timeline_offset >= self.end_timeline_offset {
            return;
        }

        if self
            .is_loading
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            log::trace!("audio background loader already running, skipping");
            return;
        }

        let source_infos: Vec<AudioSourceInfo> = self
            .sources
            .iter()
            .map(|source| {
                let segments = source.segments();

                let segment_infos: Vec<SegmentSourceInfo> = segments
                    .iter()
                    .filter_map(|segment| {
                        let metadata = &segment.metadata;
                        metadata.audios.first().map(|audio| SegmentSourceInfo {
                            path: metadata.path.clone(),
                            stream_index: audio.index,
                            channels: audio.channels,
                            sample_rate: audio.sample_rate,
                            segment: segment.clone(),
                        })
                    })
                    .collect();

                let muted =
                    matches!(source, AudioSource::VideoWithAudio(video_track) if video_track.muted);

                AudioSourceInfo {
                    segments: segment_infos,
                    muted,
                }
            })
            .filter(|info| !info.segments.is_empty())
            .collect();

        let next_cache_timeline_offset = self.next_cache_timeline_offset;
        let remaining_duration = self
            .end_timeline_offset
            .saturating_sub(next_cache_timeline_offset);
        let cache_duration = self.cache_duration.min(remaining_duration);
        let channels = self.channels;
        let sample_rate = self.sample_rate;
        let is_loading = self.is_loading.clone();
        let sender = self.sender.clone();

        log::debug!(
            "audio background loading: {:?} -> {:?}",
            next_cache_timeline_offset,
            next_cache_timeline_offset + cache_duration
        );

        thread::spawn(move || {
            // 计算所有 segment 的最大结束时间
            let max_segment_end = source_infos
                .iter()
                .flat_map(|info| info.segments.iter())
                .filter(|seg| !seg.segment.hiding)
                .map(|seg| seg.segment.timeline_offset + seg.segment.duration)
                .max()
                .unwrap_or(Duration::ZERO);

            load_and_handle_samples(
                &source_infos,
                next_cache_timeline_offset,
                cache_duration,
                channels,
                sample_rate,
                sender.clone(),
            );

            // 判断是否已到达所有 segment 的结束位置
            // 如果下一个缓存位置已经到达或超过最大结束时间，标记为结束
            let next_end_offset = next_cache_timeline_offset.saturating_add(cache_duration);
            let is_last_load =
                next_end_offset >= max_segment_end.saturating_sub(Duration::from_micros(1));

            if is_last_load {
                log::debug!(
                    "Audio reached end: next_end_offset={:?}, max_segment_end={:?}",
                    next_end_offset,
                    max_segment_end
                );
                _ = sender.send((vec![], Duration::ZERO));
            }

            is_loading.store(false, Ordering::SeqCst);
        });
    }

    fn refill_cache(&mut self) {
        while let Ok((chunk, actual_duration)) = self.receiver.try_recv() {
            // 空的 chunk 且 actual_duration 为 ZERO 表示到达末尾
            if chunk.is_empty() && actual_duration == Duration::ZERO {
                self.reached_end = true;
            } else {
                // 即使 chunk 为空（在 gap 区域），也要更新时间位置
                self.cache_samples.extend(chunk);
                self.next_cache_timeline_offset += actual_duration;
            }
        }
    }

    fn wait_for_data(&mut self, wait_time: Duration) -> bool {
        if self.cache_samples.is_empty() && self.reached_end {
            return false;
        }

        let cache_duration = Duration::from_secs_f64(
            (self.cache_samples.len() as f64) / self.channels as f64 / self.sample_rate as f64,
        );

        if cache_duration >= self.max_cache_duration {
            return true;
        }

        if cache_duration < self.max_cache_duration * 3 / 4 {
            self.start_background_loader();
        }

        // 没有缓存，并且还没到时间轴末尾，等待缓存准备好
        if self.cache_samples.is_empty() && !self.reached_end {
            let now = Instant::now();

            loop {
                if !self.receiver.is_empty() {
                    return true;
                }

                if now.elapsed() > wait_time {
                    break;
                }

                std::thread::sleep(Duration::from_millis(5));
            }

            log::warn!("Wait of video frame timeout: {:?}", wait_time);
            return false;
        }

        // 如果缓存为空且已到达末尾，直接返回false
        if self.cache_samples.is_empty() {
            return false;
        }

        true
    }
}

impl Iterator for UnifiedAudioTracksMixerIterator {
    type Item = AudioSamples;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remained_timeline_duration == Duration::ZERO {
            return None;
        }

        self.refill_cache();

        // 如果已经到达音频末尾且缓存为空，直接返回None
        if self.reached_end && self.cache_samples.is_empty() {
            return None;
        }

        if !self.wait_for_data(Duration::from_secs(5)) {
            return None;
        }

        self.refill_cache();

        if self.cache_samples.is_empty() {
            return None;
        }

        let chunk_size = (self.request_samples_duration.as_millis()
            * self.sample_rate as u128
            * self.channels as u128
            / 1000)
            .min(self.cache_samples.len() as u128) as usize;

        let samples: Vec<f32> = self.cache_samples.drain(0..chunk_size).collect();
        let sample_duration = Duration::from_millis(
            samples.len() as u64 * 1000 / self.channels as u64 / self.sample_rate as u64,
        );

        self.remained_timeline_duration = self
            .remained_timeline_duration
            .saturating_sub(sample_duration);

        Some(AudioSamples {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
        })
    }
}

fn load_and_handle_samples(
    source_infos: &[AudioSourceInfo],
    request_timeline_offset: Duration,
    request_duration: Duration,
    output_channels: u16,
    output_sample_rate: u32,
    sender: Sender<(Vec<f32>, Duration)>,
) {
    let request_start_time = Instant::now();

    let target_sample_count = (request_duration.as_secs_f64()
        * output_sample_rate as f64
        * output_channels as f64) as usize;

    // 计算所有 segment 的最大结束时间
    let max_end_time = source_infos
        .iter()
        .flat_map(|info| info.segments.iter())
        .filter(|seg_info| !seg_info.segment.hiding && !seg_info.segment.audio_muted)
        .map(|seg_info| seg_info.segment.timeline_offset + seg_info.segment.duration)
        .max()
        .unwrap_or(Duration::ZERO);

    // 如果请求范围已经超过所有 segment，直接返回空
    if request_timeline_offset >= max_end_time {
        _ = sender.send((vec![], Duration::ZERO));
        return;
    }

    let all_segment_samples = extract_all_segment_samples(
        source_infos,
        request_timeline_offset,
        request_duration,
        output_sample_rate,
        output_channels,
    );

    let all_segment_samples = resample_samples(all_segment_samples, target_sample_count);

    let all_segment_samples =
        apply_filters_to_segments(all_segment_samples, output_channels, output_sample_rate);

    let mixed = mix_and_normalize_samples(all_segment_samples);

    // 如果混合结果为空（在 gap 区域），创建静音样本
    let mixed = if mixed.is_empty() && target_sample_count > 0 {
        vec![0.0_f32; target_sample_count]
    } else {
        mixed
    };

    // 计算实际的音频时长
    let actual_duration = Duration::from_secs_f64(
        (mixed.len() as f64) / (output_sample_rate * output_channels as u32) as f64,
    );

    log::debug!(
        "Audio background loader completed: {} samples, duration {:?} in {}ms",
        mixed.len(),
        actual_duration,
        request_start_time.elapsed().as_millis()
    );

    _ = sender.send((mixed, actual_duration));
}

fn extract_all_segment_samples(
    source_infos: &[AudioSourceInfo],
    request_timeline_offset: Duration,
    request_duration: Duration,
    output_sample_rate: u32,
    output_channels: u16,
) -> Vec<SegmentSamples> {
    let mut all_segment_samples = Vec::new();
    let request_start_time = request_timeline_offset;
    let request_end_time = request_timeline_offset + request_duration;

    let target_sample_count = (request_duration.as_secs_f64()
        * output_sample_rate as f64
        * output_channels as f64) as usize;

    // 计算所有段的最大结束时间
    let max_end_time = source_infos
        .iter()
        .flat_map(|info| info.segments.iter())
        .filter(|seg_info| !seg_info.segment.hiding && !seg_info.segment.audio_muted)
        .map(|seg_info| seg_info.segment.timeline_offset + seg_info.segment.duration)
        .max()
        .unwrap_or(Duration::ZERO);

    // 如果请求的时间范围已经超出所有段的结束时间，返回空表示到达末尾
    if request_start_time >= max_end_time {
        return Vec::new();
    }

    for source_info in source_infos.iter() {
        if source_info.muted {
            continue;
        }

        for seg_info in &source_info.segments {
            let segment = &seg_info.segment;

            if segment.hiding || segment.audio_muted {
                continue;
            }

            let segment_start = segment.timeline_offset;
            let segment_end = segment_start + segment.duration;

            let overlap_start = request_start_time.max(segment_start);
            let overlap_end = request_end_time.min(segment_end);

            if overlap_start >= overlap_end {
                continue;
            }

            let overlap_duration = overlap_end - overlap_start;

            // Calculate sample counts for pre-padding, content, and post-padding
            let pre_sample_count = ((overlap_start - request_start_time).as_secs_f64()
                * output_sample_rate as f64
                * output_channels as f64) as usize;
            let content_sample_count = (overlap_duration.as_secs_f64()
                * output_sample_rate as f64
                * output_channels as f64) as usize;
            let post_sample_count = ((request_end_time - overlap_end).as_secs_f64()
                * output_sample_rate as f64
                * output_channels as f64) as usize;
            let total_sample_count = pre_sample_count + content_sample_count + post_sample_count;

            let mut segment_samples = Vec::with_capacity(total_sample_count);

            // pre-padding (gap) - 使用 None 表示间隙
            segment_samples.extend(repeat_n(None, pre_sample_count));

            // Extract content samples if there's any overlap
            if content_sample_count > 0 {
                match extract_segment_audio(
                    &seg_info.path,
                    seg_info.stream_index,
                    segment,
                    overlap_start,
                    overlap_duration,
                    seg_info.channels,
                    seg_info.sample_rate,
                    output_channels,
                    output_sample_rate,
                ) {
                    Ok(segment_data) => {
                        let extracted_samples = segment_data.samples;
                        if extracted_samples.len() != content_sample_count {
                            let resampled = if extracted_samples.is_empty() {
                                vec![Some(0.0_f32); content_sample_count]
                            } else {
                                resample_audio_to_target_samples_option(
                                    &extracted_samples,
                                    output_channels,
                                    content_sample_count as u32 / output_channels as u32,
                                )
                            };
                            segment_samples.extend(resampled.into_iter());
                        } else {
                            segment_samples.extend(extracted_samples.into_iter());
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to extract audio segment [{:.3}s, {:.3}s): {:?}, using silent samples",
                            overlap_start.as_secs_f64(),
                            overlap_end.as_secs_f64(),
                            e
                        );
                        // 即使出错，内容区域仍然用 Some(0.0) 表示有数据
                        segment_samples.extend(repeat_n(Some(0.0_f32), content_sample_count));
                    }
                }
            }

            // post-padding (gap) - 使用 None 表示间隙
            segment_samples.extend(repeat_n(None, post_sample_count));

            // 确保样本数量正确，填补任何缺失的部分（用 None 填充）
            let diff = total_sample_count.saturating_sub(segment_samples.len());
            if diff > 0 {
                segment_samples.extend(repeat_n(None, diff));
            }

            all_segment_samples.push(SegmentSamples {
                from_segment: Some(segment.clone()),
                samples: segment_samples,
                relative_timeline_offset: overlap_start - segment.timeline_offset,
            });
        }
    }

    // 如果没有找到任何段，但请求的时间范围在时间轴内，返回间隙样本
    if all_segment_samples.is_empty() && target_sample_count > 0 {
        let gap_samples: Vec<Option<f32>> = repeat_n(None, target_sample_count).collect();

        all_segment_samples.push(SegmentSamples {
            from_segment: None,
            samples: gap_samples,
            relative_timeline_offset: Duration::ZERO,
        });
    }

    all_segment_samples
}

pub fn mix_and_normalize_samples(all_segment_samples: Vec<SegmentSamples>) -> Vec<f32> {
    let target_count = all_segment_samples
        .first()
        .map(|s| s.samples.len())
        .unwrap_or(0);

    if target_count == 0 {
        return Vec::new();
    }

    let mut mixed = vec![0.0_f32; target_count];

    for seg_samples in all_segment_samples {
        for (i, sample_opt) in seg_samples.samples.iter().enumerate() {
            if i < mixed.len()
                && let Some(sample) = sample_opt
            {
                // None = gap, 不参与混合
                mixed[i] += sample;
            }
        }
    }

    // 移除平均化，直接归一化，保留各音轨原始音量比例
    normalize_samples(&mut mixed);
    mixed
}

pub fn resample_samples(
    all_segment_samples: Vec<SegmentSamples>,
    target_sample_count: usize,
) -> Vec<SegmentSamples> {
    let mut segment_samples = Vec::with_capacity(all_segment_samples.len());

    for seg_samples in all_segment_samples.into_iter() {
        if seg_samples.samples.is_empty() {
            continue;
        }

        // NOTE: 上层应该已经进行重采样，保证了seg_samples大小都一致了，所以这里channel = 1，问题不大
        let resampled = resample_audio_to_target_samples_option(
            &seg_samples.samples,
            1,
            target_sample_count as u32,
        );

        segment_samples.push(SegmentSamples {
            from_segment: seg_samples.from_segment,
            samples: resampled,
            relative_timeline_offset: seg_samples.relative_timeline_offset,
        });
    }

    segment_samples
}

pub fn normalize_samples(samples: &mut [f32]) {
    let max_amplitude = samples
        .iter()
        .map(|&s| s.abs())
        .fold(0.0_f32, |a, b| a.max(b));

    if max_amplitude > 1.0 {
        let scale = 1.0 / max_amplitude;
        for sample in samples.iter_mut() {
            *sample *= scale;
        }
    }
}

pub fn apply_filters_to_segments(
    all_segment_samples: Vec<SegmentSamples>,
    output_channels: u16,
    output_sample_rate: u32,
) -> Vec<SegmentSamples> {
    all_segment_samples
        .into_iter()
        .map(|mut seg_samples| {
            match &seg_samples.from_segment {
                Some(segment) if !segment.audio_filters.is_empty() => {
                    let config = AudioFilterConfig {
                        channels: output_channels,
                        sample_rate: output_sample_rate,
                    };

                    let samples = std::mem::take(&mut seg_samples.samples);
                    // 克隆一份用于错误恢复
                    let samples_clone = samples.clone();
                    match apply_segment_filters_option(
                        config,
                        samples,
                        segment.clone(),
                        seg_samples.relative_timeline_offset,
                    ) {
                        Ok(filtered_samples) => seg_samples.samples = filtered_samples,
                        Err(e) => {
                            log::warn!("Audio filter error: {:?}, using original samples", e);
                            seg_samples.samples = samples_clone;
                        }
                    }
                }
                _ => {}
            }

            seg_samples
        })
        .collect()
}

fn apply_segment_filters_option(
    config: AudioFilterConfig,
    samples: Vec<Option<f32>>,
    segment: Arc<Segment>,
    relative_timeline_offset: Duration,
) -> Result<Vec<Option<f32>>> {
    // 只对 Some 值应用滤镜，保留 None 位置, 提取 Some 值及其位置
    let indexed_samples: Vec<(usize, f32)> = samples
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| opt.map(|v| (i, v)))
        .collect();

    if indexed_samples.is_empty() {
        return Ok(samples);
    }

    let audio_only: Vec<f32> = indexed_samples.iter().map(|(_, v)| *v).collect();
    let chunk_duration = Duration::from_secs_f64(
        audio_only.len() as f64 / (config.sample_rate as f64 * config.channels as f64),
    );

    let mut audio_data = AudioData {
        config,
        samples: audio_only,
        from_segment: segment.clone(),
        relative_timeline_offset,
        chunk_duration,
    };

    for filter in &segment.audio_filters {
        if filter.enabled() {
            filter.inner.apply(&mut audio_data)?;
        }
    }

    // 将滤镜后的数据放回原位置
    let mut result = samples;
    for (i, filtered_value) in indexed_samples.iter().zip(audio_data.samples.iter()) {
        result[i.0] = Some(*filtered_value);
    }

    Ok(result)
}

pub fn extract_segment_audio(
    path: &Path,
    stream_index: usize,
    segment: &Arc<Segment>,
    extract_timeline_offset: Duration, // 从这个时间戳开始提取（相对于时间轴）
    extract_duration: Duration,        // 提取时长
    source_channels: u16,
    source_sample_rate: u32,
    output_channels: u16,
    output_sample_rate: u32,
) -> Result<SegmentSamples> {
    ensure_file_exists!(path);

    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;
    let mut input_ctx = ffmpeg::format::input(path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    // 查找指定的音频流
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

    let source_format = decoder.format();
    let time_base = stream.time_base();

    let effective_speed = segment.playback_speed * segment.global_speed;
    let relative_start = extract_timeline_offset
        .checked_sub(segment.timeline_offset)
        .unwrap_or(Duration::ZERO)
        .as_secs_f64();
    // Scale relative timeline offset by speed for source position
    let source_position =
        segment.source_offset.as_secs_f64() + relative_start * effective_speed as f64;

    // 计算源文件中的结束时间（用于与 packet_time 比较）
    let target_end_source_time = Duration::from_secs_f64(
        source_position + extract_duration.as_secs_f64() * effective_speed as f64,
    );

    // Seek 到实际提取位置之前 1 秒，避免从头开始查找
    // FFmpeg format-level seek expects AV_TIME_BASE (microseconds), not stream time base.
    // Using stream time base causes seek to go to wrong position for MP3/AAC etc.
    let seek_timestamp =
        ((source_position - 1.0).max(0.0) * ffmpeg::sys::AV_TIME_BASE as f64) as i64;

    if seek_timestamp > 0
        && let Err(e) = input_ctx.seek(seek_timestamp, ..)
    {
        log::warn!(
            "{} seek to {:?} ({:.3}s) failed: {e}",
            path.display(),
            seek_timestamp,
            seek_timestamp as f64 / ffmpeg::sys::AV_TIME_BASE as f64
        );
    }
    decoder.flush();

    let mut decoded_data = Vec::new();

    // 记录解码数据的起始时间，用于后续正确切片
    // seek 位置可能不精确，所以我们用第一个有效 packet 的时间作为参考
    let mut decode_start_time: Option<Duration> = None;

    // 安全阈值：当没有 PTS 时，根据已解码的样本数量判断是否应该停止
    // 需要解码从 seek 位置到 target_end_source_time 的所有数据
    let max_decoded_samples = ((target_end_source_time.as_secs_f64() + 1.0)
        * source_sample_rate as f64
        * source_channels as f64) as usize;

    for (stream, packet) in input_ctx.packets() {
        if stream.index() != stream_index {
            continue;
        }

        // 尝试获取 PTS 用于时间跟踪和终止判断
        // 对于 MP3/AAC 等 format，seek 后某些 packet 可能没有 PTS，
        // 但仍然需要发送给解码器以保证解码输出正确
        let packet_time = packet.pts().map(|pts| {
            Duration::from_secs_f64(
                (pts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64)
                    .max(0.0),
            )
        });

        if let Some(pt) = packet_time {
            // 记录第一个有效 packet 的时间作为解码起始时间
            if decode_start_time.is_none() {
                decode_start_time = Some(pt);
            }

            if pt > target_end_source_time {
                break;
            }
        } else if decoded_data.len() >= max_decoded_samples {
            // 没有 PTS 的 packet：如果已解码足够多的样本，则停止
            break;
        }

        if let Err(e) = decoder.send_packet(&packet) {
            log::warn!("Error sending packet to decoder: {:?}", e);
            continue;
        }

        let mut decoded_frame = ffmpeg::frame::Audio::empty();
        loop {
            match decoder.receive_frame(&mut decoded_frame) {
                Ok(_) => {
                    if decoded_frame.samples() > 0 {
                        extract_samples_from_frame(
                            &decoded_frame,
                            source_format,
                            source_channels,
                            &mut decoded_data,
                        )?;
                    }
                }
                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    log::warn!("Error receiving frame: {:?}", e);
                    break;
                }
            }
        }
    }

    // Drain any remaining frames from the decoder
    _ = decoder.send_eof();
    let mut decoded_frame = ffmpeg::frame::Audio::empty();
    loop {
        match decoder.receive_frame(&mut decoded_frame) {
            Ok(_) => {
                if decoded_frame.samples() > 0 {
                    extract_samples_from_frame(
                        &decoded_frame,
                        source_format,
                        source_channels,
                        &mut decoded_data,
                    )?;
                }
            }
            Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
            Err(_) => break,
        }
    }
    decoder.flush();

    // 计算 source_position 相对于解码起始时间的偏移
    // 这样才能正确地从 decoded_data 中切片
    let decode_start = match decode_start_time {
        Some(t) => t.as_secs_f64(),
        None => {
            log::warn!(
                "No packets found for audio extraction, source_position={:.3}s",
                source_position
            );
            // 如果没有解码任何数据，返回空
            return Ok(SegmentSamples {
                from_segment: Some(segment.clone()),
                samples: Vec::new(),
                relative_timeline_offset: extract_timeline_offset
                    .checked_sub(segment.timeline_offset)
                    .unwrap_or(Duration::ZERO),
            });
        }
    };

    let relative_start_sample =
        ((source_position - decode_start).max(0.0) * source_sample_rate as f64) as usize;
    let duration_samples = (extract_duration.as_secs_f64()
        * effective_speed as f64
        * source_sample_rate as f64) as usize;
    let start_byte = relative_start_sample * source_channels as usize;
    let end_byte = (relative_start_sample + duration_samples) * source_channels as usize;

    let segment_data = if start_byte < decoded_data.len() {
        let actual_end = end_byte.min(decoded_data.len());
        decoded_data[start_byte..actual_end].to_vec()
    } else {
        Vec::new()
    };

    let raw_samples =
        if source_channels != output_channels || source_sample_rate != output_sample_rate {
            resample_audio_with_channel(
                &segment_data,
                source_sample_rate,
                source_channels,
                output_sample_rate,
                output_channels,
            )
            .map_err(|e| Error::FFmpeg(format!("Resample error: {:?}", e)))?
        } else {
            segment_data
        };

    // Apply speed adjustment: use pitch_shift to preserve pitch
    // For speed != 1.0, stretch/compress audio while keeping pitch unchanged
    // stretch_ratio = 1.0 / speed: faster speed means smaller stretch_ratio (compress)
    let final_samples = if effective_speed != 1.0 {
        let stretch_ratio = 1.0 / effective_speed;
        time_stretch_preserving_pitch(
            &raw_samples,
            stretch_ratio,
            output_channels,
            output_sample_rate,
        )
    } else {
        raw_samples
    };

    // Ensure output sample count matches expected timeline duration
    // Phase vocoder may produce slightly fewer samples due to frame alignment
    let target_sample_count = (extract_duration.as_secs_f64()
        * output_sample_rate as f64
        * output_channels as f64) as usize;

    let samples: Vec<Option<f32>> = if final_samples.len() < target_sample_count {
        let padding_needed = target_sample_count - final_samples.len();
        final_samples
            .into_iter()
            .chain(std::iter::repeat_n(0.0, padding_needed))
            .map(Some)
            .collect()
    } else {
        final_samples.into_iter().map(Some).collect()
    };

    let relative_timeline_offset = extract_timeline_offset
        .checked_sub(segment.timeline_offset)
        .unwrap_or(Duration::ZERO);

    Ok(SegmentSamples {
        from_segment: Some(segment.clone()),
        samples,
        relative_timeline_offset,
    })
}

pub(crate) fn extract_samples_from_frame(
    frame: &ffmpeg::frame::Audio,
    format: ffmpeg::format::Sample,
    channels: u16,
    output: &mut Vec<f32>,
) -> Result<()> {
    let nb_samples = frame.samples();
    let channels = channels as usize;

    match format {
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let samples = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const f32, nb_samples * channels)
            };
            output.extend_from_slice(samples);
        }
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar) => {
            for sample_idx in 0..nb_samples {
                for ch in 0..channels {
                    let channel_data = frame.data(ch);
                    let samples = unsafe {
                        std::slice::from_raw_parts(channel_data.as_ptr() as *const f32, nb_samples)
                    };
                    output.push(samples[sample_idx]);
                }
            }
        }
        ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let samples = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i16, nb_samples * channels)
            };
            const SCALE: f32 = 1.0 / (i16::MAX as f32 + 1.0);
            output.extend(samples.iter().map(|&s| s as f32 * SCALE));
        }
        ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Planar) => {
            const SCALE: f32 = 1.0 / (i16::MAX as f32 + 1.0);
            for sample_idx in 0..nb_samples {
                for ch in 0..channels {
                    let channel_data = frame.data(ch);
                    let samples = unsafe {
                        std::slice::from_raw_parts(channel_data.as_ptr() as *const i16, nb_samples)
                    };
                    output.push(samples[sample_idx] as f32 * SCALE);
                }
            }
        }
        ffmpeg::format::Sample::I32(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let samples = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i32, nb_samples * channels)
            };
            const SCALE: f32 = 1.0 / (i32::MAX as f32 + 1.0);
            output.extend(samples.iter().map(|&s| s as f32 * SCALE));
        }
        ffmpeg::format::Sample::I32(ffmpeg::format::sample::Type::Planar) => {
            const SCALE: f32 = 1.0 / (i32::MAX as f32 + 1.0);
            for sample_idx in 0..nb_samples {
                for ch in 0..channels {
                    let channel_data = frame.data(ch);
                    let samples = unsafe {
                        std::slice::from_raw_parts(channel_data.as_ptr() as *const i32, nb_samples)
                    };
                    output.push(samples[sample_idx] as f32 * SCALE);
                }
            }
        }
        _ => {
            return Err(Error::FFmpeg(format!(
                "Unsupported sample format: {:?}",
                format
            )));
        }
    }

    Ok(())
}
