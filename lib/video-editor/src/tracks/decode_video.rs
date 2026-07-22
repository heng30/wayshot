use super::{
    TimeToFrameConverter,
    segment::{Segment, convert_frame_to_image, initialize_video_decoder, seek_to_frame},
    video_frame_cache::{FrameCacheKey, VideoImage, get_global_video_cache, render_svg_to_rgba},
    video_track::{
        FilteredLayerImages, LayerFrame, LayerFrames, VideoSourceInfo, apply_segment_video_filters,
        composite_frame,
    },
};
use crate::{
    Error, Result, ensure_file_exists,
    filters::traits::{ImageData, VideoFilterConfig},
};

use crossbeam::channel::{self, Receiver, Sender};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use video_utils::convert::resize_rgba_image_contain;

type SegmentRef = Option<(usize, Arc<Segment>)>;

struct FrameSet {
    frames: Vec<(usize, SegmentRef, FilteredLayerImages)>, // (track_index, segment_index_and_segment, filtered_images)
    relative_timeline_offset: Duration,
}

#[derive(Debug, Clone)]
pub struct DecodeVideoConfig {
    pub request_timeline_offset: Duration,
    pub request_duration: Duration,
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f32,
    pub disable_cache: bool,
}

pub fn decode_frames(
    source_infos: Vec<VideoSourceInfo>,
    config: DecodeVideoConfig,
    sender: Sender<Vec<LayerFrames>>,
) {
    let config_clone = config.clone();
    let request_start_time = Instant::now();
    let (stream_sender, stream_receiver) = channel::unbounded();

    let handle = thread::spawn(move || {
        match extract_segment_frames(source_infos, config_clone, stream_sender) {
            Ok(_) => log::debug!("[decode_frames] extract_segment_frames returned Ok"),
            Err(e) => log::warn!("[decode_frames] extract_segment_frames returned Err: {e}"),
        }
    });

    let total_frames = handle_frames(config, sender, stream_receiver);

    if let Err(e) = handle.join() {
        log::warn!("[decode_frames] extract_segment_frames thread panicked: {e:?}");
    }

    log::debug!(
        "[decode_frames] END: total_frames={}, elapsed={}ms",
        total_frames,
        request_start_time.elapsed().as_millis()
    );
}

fn handle_frames(
    config: DecodeVideoConfig,
    sender: Sender<Vec<LayerFrames>>,
    receiver: Receiver<FrameSet>,
) -> usize {
    let batch_size = 3;
    let mut total_frames = 0;
    let output_width = config.output_width;
    let output_height = config.output_height;
    let mut batch: Vec<FrameSet> = Vec::with_capacity(batch_size);

    while let Ok(frame_set) = receiver.recv() {
        batch.push(frame_set);

        if batch.len() >= batch_size {
            let to_process = std::mem::take(&mut batch);

            let processed: Vec<LayerFrames> = to_process
                .into_par_iter()
                .map(|fs| build_layer_frames(fs, output_width, output_height))
                .collect();

            total_frames += processed.len();

            for layer_frames in processed {
                if let Err(e) = sender.send(vec![layer_frames]) {
                    log::warn!("[handle_frames] receiver dropped after {total_frames} frames: {e}");
                    _ = sender.send(vec![]);
                    return total_frames;
                }
            }
        }
    }

    if !batch.is_empty() {
        let processed: Vec<LayerFrames> = batch
            .into_par_iter()
            .map(|fs| build_layer_frames(fs, output_width, output_height))
            .collect();

        total_frames += processed.len();

        for layer_frames in processed {
            if sender.send(vec![layer_frames]).is_err() {
                break;
            }
        }
    }

    log::debug!("[handle_frames] END: processed {} frames", total_frames);
    _ = sender.send(vec![]); // 没有数据，达到尾部
    total_frames
}

fn build_layer_frames(fs: FrameSet, output_width: u32, output_height: u32) -> LayerFrames {
    let layers: Vec<LayerFrame> = fs
        .frames
        .into_iter()
        .map(|(track_index, from_segment, filtered_images)| {
            let mut layer_frame = LayerFrame::new(
                filtered_images.original_image,
                filtered_images.image,
                from_segment,
                track_index,
            );
            if let Some(img) = filtered_images.image_for_composite {
                layer_frame = layer_frame.with_composite_image(img);
            }
            layer_frame
        })
        .collect();

    let frame_images: Vec<&VideoImage> = layers.iter().map(|l| l.composite_image()).collect();
    let composited_image = composite_single_position(&frame_images, output_width, output_height);

    LayerFrames {
        layers,
        composited_image,
        relative_timeline_offset: fs.relative_timeline_offset,
    }
}

fn extract_segment_frames(
    source_infos: Vec<VideoSourceInfo>,
    config: DecodeVideoConfig,
    sender: Sender<FrameSet>,
) -> Result<()> {
    let output_converter = TimeToFrameConverter::from_f32(config.output_fps);
    let target_frame_count = output_converter.duration_to_frame(config.request_duration);

    if target_frame_count == 0 {
        return Err(crate::Error::InvalidConfig(
            "request duration is ZERO".to_string(),
        ));
    }

    let max_end_time = source_infos
        .iter()
        .flat_map(|info| info.segments.iter())
        .filter(|seg_info| !seg_info.segment.hiding)
        .map(|seg_info| seg_info.segment.timeline_offset + seg_info.segment.duration)
        .max()
        .unwrap_or(Duration::ZERO);

    if config.request_timeline_offset >= max_end_time {
        return Err(crate::Error::InvalidConfig(format!(
            "request start time[{:?}] > max end time[{max_end_time:?}]",
            config.request_timeline_offset
        )));
    }

    let source_count = source_infos.len();
    if source_count == 0 {
        return Ok(());
    }

    let (worker_sender, worker_receiver) = channel::unbounded();
    extract_segment_frames_worker(source_infos, config.clone(), worker_sender);
    reorder_frames(
        config,
        source_count,
        target_frame_count,
        sender,
        worker_receiver,
    );

    Ok(())
}

fn extract_segment_frames_worker(
    source_infos: Vec<VideoSourceInfo>,
    config: DecodeVideoConfig,
    sender: Sender<(usize, usize, SegmentRef, FilteredLayerImages)>,
) {
    let output_converter = TimeToFrameConverter::from_f32(config.output_fps);
    let target_frame_count = output_converter.duration_to_frame(config.request_duration);
    let request_start_time = config.request_timeline_offset;
    let request_end_time = config.request_timeline_offset + config.request_duration;

    for (source_index, source_info) in source_infos.into_iter().enumerate() {
        let track_index = source_info.track_index;
        let sender = sender.clone();
        let config = config.clone();
        let output_converter = output_converter.clone();

        thread::spawn(move || {
            let segments: Vec<_> = source_info
                .segments
                .into_iter()
                .filter(|seg_info| !seg_info.segment.hiding)
                .collect();

            // (start_frame, end_frame, seg_info)
            let mut overlapping_segments: Vec<(usize, usize, _)> = Vec::new();

            for seg_info in segments {
                let segment = &seg_info.segment;
                let segment_start = segment.timeline_offset;
                let segment_end = segment_start + segment.duration;
                let overlap_start = request_start_time.max(segment_start);
                let overlap_end = request_end_time.min(segment_end);

                if overlap_start >= overlap_end {
                    continue;
                }

                let start_frame =
                    output_converter.duration_to_frame(overlap_start - request_start_time);
                let end_frame =
                    output_converter.duration_to_frame(overlap_end - request_start_time);

                overlapping_segments.push((start_frame, end_frame, seg_info));
            }

            if overlapping_segments.is_empty() {
                _ = send_empty_frame(target_frame_count, source_index, track_index, &sender);
                return;
            }

            // Process frame ranges in order, filling gaps with transparent frames
            let mut current_frame = 0usize;
            for (seg_start_frame, seg_end_frame, seg_info) in overlapping_segments {
                let segment = seg_info.segment.clone();
                let is_image = segment.metadata.is_image();

                // FFmpeg cannot decode animated WebP (skips ANIM/ANMF chunks).
                // Route animated WebP segments to the image-crate-based renderer.
                let is_animated_webp = !is_image
                    && segment
                        .metadata
                        .path
                        .extension()
                        .map(|e| e.to_ascii_lowercase() == "webp")
                        .unwrap_or(false);

                if seg_start_frame > current_frame {
                    let gap_frame_count = seg_start_frame - current_frame;
                    if send_empty_frame(gap_frame_count, source_index, track_index, &sender)
                        .is_err()
                    {
                        return;
                    }
                }

                // Process this segment's frames
                let content_frame_count = seg_end_frame - seg_start_frame;

                if content_frame_count > 0 {
                    if is_image {
                        let overlap_start = request_start_time
                            + output_converter.frame_to_duration(seg_start_frame);
                        let relative_offset = overlap_start.saturating_sub(segment.timeline_offset);

                        if let Err(_e) = render_image_segment(
                            segment.clone(),
                            seg_info.segment_index,
                            source_index,
                            track_index,
                            config.output_width,
                            config.output_height,
                            config.output_fps,
                            content_frame_count,
                            relative_offset,
                            sender.clone(),
                        ) {
                            _ = send_empty_frame(
                                content_frame_count,
                                source_index,
                                track_index,
                                &sender,
                            );
                        }
                    } else if is_animated_webp {
                        let overlap_start = request_start_time
                            + output_converter.frame_to_duration(seg_start_frame);
                        let relative_offset = overlap_start.saturating_sub(segment.timeline_offset);

                        if let Err(_e) = render_animated_webp_segment(
                            segment.clone(),
                            seg_info.segment_index,
                            source_index,
                            track_index,
                            config.output_width,
                            config.output_height,
                            config.output_fps,
                            content_frame_count,
                            relative_offset,
                            sender.clone(),
                        ) {
                            _ = send_empty_frame(
                                content_frame_count,
                                source_index,
                                track_index,
                                &sender,
                            );
                        }
                    } else {
                        let config_clone = config.clone();
                        let sender_clone = sender.clone();
                        let source_fps = seg_info.fps.unwrap();
                        let disable_cache = config.disable_cache;

                        let overlap_start = request_start_time
                            + output_converter.frame_to_duration(seg_start_frame);
                        let overlap_end =
                            request_start_time + output_converter.frame_to_duration(seg_end_frame);

                        let effective_speed = segment.playback_speed * segment.global_speed;
                        let source_content_frame_count =
                            ((overlap_end - overlap_start).as_secs_f64()
                                * effective_speed as f64
                                * source_fps as f64)
                                .round() as usize;

                        let segment_start = segment.timeline_offset;
                        let relative_offset = overlap_start.saturating_sub(segment_start);
                        let source_start_time = segment.source_offset
                            + Duration::from_secs_f64(
                                relative_offset.as_secs_f64() * effective_speed as f64,
                            );
                        let extract_start_frame = ((source_start_time.as_secs_f64()
                            * source_fps as f64)
                            .round()) as usize;

                        let segment_clone = segment.clone();
                        let segment_index = seg_info.segment_index;
                        let (frame_sender, frame_receiver) = channel::unbounded();

                        // Relative timeline offset for the first frame of this segment's contribution
                        let relative_segment_timeline_offset_start =
                            request_start_time.saturating_sub(segment_start);

                        thread::spawn(move || {
                            if let Err(e) = extract_frames(
                                segment_clone,
                                extract_start_frame,
                                source_content_frame_count,
                                content_frame_count,
                                source_index,
                                frame_sender,
                                disable_cache,
                            ) {
                                log::warn!(
                                    "Failed to extract video segment [{:.3}s, {:.3}s): {:?}, source_start_frame={}",
                                    overlap_start.as_secs_f64(),
                                    overlap_end.as_secs_f64(),
                                    e,
                                    extract_start_frame
                                );
                            }
                            log::debug!("[extract_frames_thread-{}] END", source_index);
                        });

                        // Wait for all frames from this segment
                        let mut received_count = 0;
                        while let Ok((src_idx, from_seg, frame)) = frame_receiver.recv() {
                            let relative_timeline_offset = relative_segment_timeline_offset_start
                                + output_converter.frame_to_duration(received_count);

                            let filtered_frame = apply_filter_to_frame_for_video(
                                &config_clone,
                                frame,
                                from_seg.clone(),
                                relative_timeline_offset,
                            );

                            if sender_clone
                                .send((
                                    src_idx,
                                    track_index,
                                    Some((segment_index, from_seg)),
                                    filtered_frame,
                                ))
                                .is_err()
                            {
                                return;
                            }
                            received_count += 1;
                        }

                        // Fill any missing frames with transparent
                        if received_count < content_frame_count {
                            let missing = content_frame_count - received_count;
                            log::debug!(
                                "[filter_thread-{}] received {} frames, expected {}, filling {} transparent",
                                source_index,
                                received_count,
                                content_frame_count,
                                missing
                            );
                            _ = send_empty_frame(missing, source_index, track_index, &sender_clone);
                        }
                    }
                }

                current_frame = seg_end_frame;
            }

            // Fill any remaining gap after the last segment
            if current_frame < target_frame_count {
                let remaining = target_frame_count - current_frame;
                _ = send_empty_frame(remaining, source_index, track_index, &sender);
            }
        });
    }
}

fn send_empty_frame(
    count: usize,
    source_index: usize,
    track_index: usize,
    sender: &Sender<(usize, usize, SegmentRef, FilteredLayerImages)>,
) -> Result<()> {
    for _ in 0..count {
        let empty_images = FilteredLayerImages {
            original_image: VideoImage::Empty,
            image: VideoImage::Empty,
            image_for_composite: None,
        };
        let item = (source_index, track_index, None, empty_images);
        sender
            .send(item)
            .map_err(|e| crate::Error::Sender(e.to_string()))?;
    }

    Ok(())
}

fn apply_filter_to_frame_for_video(
    config: &DecodeVideoConfig,
    frame: VideoImage,
    segment: Arc<Segment>,
    relative_timeline_offset: Duration,
) -> FilteredLayerImages {
    let frame = match frame {
        VideoImage::Image { buffer } => {
            if buffer.width() == config.output_width && buffer.height() == config.output_height {
                VideoImage::image(buffer)
            } else {
                match resize_rgba_image_contain(
                    buffer,
                    config.output_width,
                    config.output_height,
                    false,
                ) {
                    Ok(resized_buffer) => VideoImage::image(resized_buffer),
                    Err(e) => {
                        log::warn!("Failed to resize frame: {:?}", e);
                        VideoImage::image(RgbaImage::new(config.output_width, config.output_height))
                    }
                }
            }
        }
        other => other,
    };

    let original_image = frame.clone();
    if segment.video_filters.is_empty() {
        return FilteredLayerImages {
            original_image,
            image: frame,
            image_for_composite: None,
        };
    }

    let filter_config =
        VideoFilterConfig::new(config.output_width, config.output_height, config.output_fps);

    let mut result =
        apply_segment_video_filters(filter_config, frame, segment, relative_timeline_offset);
    result.original_image = original_image;
    result
}

fn extract_frames(
    segment: Arc<Segment>,
    extract_start_frame: usize,
    extract_frames_count: usize,
    request_frame_count: usize,
    source_index: usize,
    sender: Sender<(usize, Arc<Segment>, VideoImage)>, // src_idx,  from_seg, frame
    disable_cache: bool,
) -> Result<()> {
    if request_frame_count == 0 {
        return Ok(());
    }

    let path = &segment.metadata.path;
    ensure_file_exists!(path);

    let video_meta = segment
        .metadata
        .first_video()
        .ok_or_else(|| crate::Error::InvalidConfig("No video stream found in segment".into()))?;

    let stream_index = video_meta.index;
    let converter = TimeToFrameConverter::from_f32(video_meta.fps);

    let sample_indices: Vec<usize> = (0..request_frame_count)
        .map(|i| extract_start_frame + (i * extract_frames_count) / request_frame_count)
        .collect();
    let needed_frame_indices: HashSet<usize> = sample_indices.iter().copied().collect();

    // 解码的时候不是只解码一帧，会解码附近几帧。如果丢弃这些数据比较可惜，缓存起来，也许能够复用
    let mut buffer: HashMap<usize, VideoImage> = HashMap::new();

    let mut decoder_state: Option<(
        ffmpeg::format::context::Input,
        ffmpeg::decoder::Video,
        ffmpeg::Rational,
    )> = None;

    let send_frame = |frame: VideoImage| -> Result<()> {
        sender
            .send((source_index, segment.clone(), frame))
            .map_err(|e| crate::Error::InvalidConfig(format!("Channel send error: {}", e)))?;
        Ok(())
    };

    let decode_result = (|| -> Result<()> {
        for output_pos in 0..request_frame_count {
            let target_frame_idx = sample_indices[output_pos];
            let mut frame_sent = false;

            if let Some(frame) = buffer.remove(&target_frame_idx) {
                send_frame(frame)?;
                // GIF maintains internal canvas state (disposal method, LZW dict).
                // Re-creating the decoder for each target frame prevents flickering
                // caused by stale state after seeking.
                if path
                    .extension()
                    .map(|e| e.to_ascii_lowercase() == "gif")
                    .unwrap_or(false)
                {
                    decoder_state = None;
                }
                continue;
            }

            let cache_key = FrameCacheKey::from_path(path, stream_index, target_frame_idx);
            if let Some(frame) = get_global_video_cache().get(&cache_key) {
                send_frame(frame)?;
                // GIF: re-create decoder to avoid canvas state corruption
                if path
                    .extension()
                    .map(|e| e.to_ascii_lowercase() == "gif")
                    .unwrap_or(false)
                {
                    decoder_state = None;
                }
                continue;
            }

            if decoder_state.is_none() {
                let (mut input_ctx, mut decoder, time_base) =
                    initialize_video_decoder(path, stream_index)?;

                // Seek directly to the target frame - FFmpeg will automatically position
                // at the nearest keyframe, and we decode forward from there.
                let seek_time = converter.frame_to_duration(target_frame_idx);
                seek_to_frame(&mut input_ctx, path, seek_time, time_base);

                // Flush decoder after seek to clear any buffered frames from the previous
                // position. Without this, the decoder may output stale frames with incorrect
                // timestamps, causing frame index mismatches and visual glitches (flickering)
                // especially for formats like GIF that maintain internal canvas state across
                // frames.
                decoder.flush();
                decoder_state = Some((input_ctx, decoder, time_base));
            }

            let (input_ctx, decoder, time_base) = decoder_state.as_mut().unwrap();

            // Decode until we get the target frame
            'decode_loop: for (stream, packet) in input_ctx.packets() {
                if stream.index() != stream_index {
                    continue;
                }

                let packet_time = match packet.dts() {
                    Some(dts) if dts >= 0 => Duration::from_secs_f64(
                        dts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64,
                    ),
                    _ => {
                        _ = decoder.send_packet(&packet);
                        continue;
                    }
                };

                let current_frame_idx = converter.duration_to_frame(packet_time);

                // Stop if we've gone well past our target
                if current_frame_idx > target_frame_idx + 10 {
                    break;
                }

                if let Err(e) = decoder.send_packet(&packet) {
                    log::warn!("Error sending packet: {:?}", e);
                    continue;
                }

                // Receive and process decoded frames
                let mut decoded_frame = ffmpeg::frame::Video::empty();
                loop {
                    match decoder.receive_frame(&mut decoded_frame) {
                        Ok(()) => {
                            let frame_time = match decoded_frame.timestamp() {
                                Some(ts) if ts >= 0 => Duration::from_secs_f64(
                                    ts as f64 * time_base.numerator() as f64
                                        / time_base.denominator() as f64,
                                ),
                                _ => continue,
                            };
                            let frame_idx = converter.duration_to_frame(frame_time);

                            if needed_frame_indices.contains(&frame_idx) {
                                match convert_frame_to_image(&decoded_frame) {
                                    Ok(video_image) => {
                                        if !disable_cache {
                                            let cache_key = FrameCacheKey::from_path(
                                                path,
                                                stream_index,
                                                frame_idx,
                                            );
                                            get_global_video_cache()
                                                .put(cache_key, video_image.clone());
                                        }

                                        if frame_idx == target_frame_idx {
                                            send_frame(video_image)?;
                                            frame_sent = true;
                                            break 'decode_loop;
                                        } else {
                                            buffer.insert(frame_idx, video_image);
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to convert frame {frame_idx}: {e:?}");
                                    }
                                }
                            }
                        }
                        Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                        Err(e) => {
                            log::warn!("Decoder error: {:?}", e);
                            break;
                        }
                    }
                }
            }

            if !frame_sent {
                _ = decoder.send_eof();
                let mut decoded_frame = ffmpeg::frame::Video::empty();
                loop {
                    match decoder.receive_frame(&mut decoded_frame) {
                        Ok(()) => {
                            let frame_time = match decoded_frame.timestamp() {
                                Some(ts) if ts >= 0 => Duration::from_secs_f64(
                                    ts as f64 * time_base.numerator() as f64
                                        / time_base.denominator() as f64,
                                ),
                                _ => continue,
                            };
                            let frame_idx = converter.duration_to_frame(frame_time);

                            if frame_idx == target_frame_idx {
                                if let Ok(video_image) = convert_frame_to_image(&decoded_frame) {
                                    if !disable_cache {
                                        let cache_key =
                                            FrameCacheKey::from_path(path, stream_index, frame_idx);
                                        get_global_video_cache()
                                            .put(cache_key, video_image.clone());
                                    }
                                    send_frame(video_image)?;
                                    frame_sent = true;
                                }
                                break;
                            }
                        }
                        Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                        Err(_) => break,
                    }
                }
                decoder.flush();
            }

            // If frame still not sent, send empty frame for this position
            if !frame_sent {
                send_frame(VideoImage::Empty)?;
            }
        }

        Ok(())
    })();

    // Drain decoder regardless of success/failure.
    // IMPORTANT: bind `input_ctx` to a named variable so it stays alive
    // during the drain loop. Using `_` would drop Input (calling
    // avformat_close_input) BEFORE decoder.flush() runs, which can leave
    // frame-threaded decoder workers referencing freed memory.
    if let Some((input_ctx, mut decoder, _time_base)) = decoder_state {
        let _ = decoder.send_eof();
        let mut decoded_frame = ffmpeg::frame::Video::empty();
        loop {
            match decoder.receive_frame(&mut decoded_frame) {
                Ok(_) => {}
                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                Err(_) => break,
            }
        }
        decoder.flush();
        drop(decoder);
        drop(input_ctx);
    }

    decode_result
}

fn reorder_frames(
    config: DecodeVideoConfig,
    source_count: usize,
    frame_count_per_source: usize,
    sender: Sender<FrameSet>,
    receiver: Receiver<(usize, usize, SegmentRef, FilteredLayerImages)>,
) {
    let mut next_send_index = 0;
    let total_expected = source_count * frame_count_per_source;
    let output_converter = TimeToFrameConverter::from_f32(config.output_fps);
    let mut buffer: Vec<VecDeque<(usize, SegmentRef, FilteredLayerImages)>> =
        vec![VecDeque::with_capacity(frame_count_per_source); source_count];

    let mut received_count = 0;
    for _ in 0..total_expected {
        match receiver.recv() {
            Ok((source_index, track_index, from_segment, filtered_images)) => {
                received_count += 1;
                buffer[source_index].push_back((track_index, from_segment, filtered_images));

                let mut can_send = true;
                for item in buffer.iter() {
                    if item.is_empty() {
                        can_send = false;
                        break;
                    }
                }

                if !can_send {
                    continue;
                }

                // Collect one frame from each source, preserving metadata
                let frames: Vec<(usize, SegmentRef, FilteredLayerImages)> = buffer
                    .iter_mut()
                    .map(|queue| queue.pop_front().unwrap())
                    .collect();

                let relative_timeline_offset = config.request_timeline_offset
                    + output_converter.frame_to_duration(next_send_index);

                let frame_set = FrameSet {
                    frames,
                    relative_timeline_offset,
                };

                if sender.send(frame_set).is_err() {
                    log::warn!("[reorder_frames] receiver dropped, stopping frame extraction");
                    return;
                }

                next_send_index += 1;
            }
            Err(_) => {
                log::warn!(
                    "[reorder_frames] channel closed after {} frames, expected {}",
                    received_count,
                    total_expected
                );
                break;
            }
        }
    }

    log::debug!(
        "[reorder_frames] END: received {} frames, sent {} frames",
        received_count,
        next_send_index
    );
}

pub fn composite_single_position(frames: &[&VideoImage], width: u32, height: u32) -> RgbaImage {
    let mut composited = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]));

    // 从后往前合成：轨道索引大的先绘制（底层），轨道索引小的后绘制（上层）
    for frame in frames.iter().rev() {
        composite_frame(&mut composited, frame);
    }

    composited
}

fn render_animated_webp_segment(
    segment: Arc<Segment>,
    segment_index: usize,
    source_index: usize,
    track_index: usize,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
    frame_counts: usize,
    relative_timeline_offset: Duration,
    sender: Sender<(usize, usize, SegmentRef, FilteredLayerImages)>,
) -> Result<()> {
    let path = &segment.metadata.path;
    let output_converter = TimeToFrameConverter::from_f32(output_fps);
    let config = VideoFilterConfig::new(output_width, output_height, output_fps);

    let video_meta = segment
        .metadata
        .first_video()
        .ok_or_else(|| Error::InvalidConfig("No video metadata for WebP".into()))?;
    let avg_delay_ms = if video_meta.fps > 0.0 {
        (1000.0 / video_meta.fps) as u64
    } else {
        100
    };
    let total_duration_ms = video_meta.duration.as_millis() as u64;
    let source_frame_count = if avg_delay_ms > 0 {
        (total_duration_ms / avg_delay_ms) as usize
    } else {
        1
    };
    if source_frame_count == 0 || total_duration_ms == 0 {
        return Err(Error::InvalidConfig("WebP has zero duration".into()));
    }

    let data = std::fs::read(path)
        .map_err(|e| Error::InvalidFile(format!("Failed to read WebP file: {}", e)))?;

    let mut decoder = webpx::AnimationDecoder::with_options_limits(
        &data,
        webpx::ColorMode::Rgba,
        true,
        &webpx::Limits::none(),
    )
    .map_err(|e| Error::InvalidFile(format!("Failed to create WebP decoder: {e}")))?;

    let mut decoded_source_idx: usize = 0;
    let mut current_resized: Option<(usize, image::RgbaImage)> = None;

    for output_idx in 0..frame_counts {
        let frame_offset =
            relative_timeline_offset + output_converter.frame_to_duration(output_idx);
        let time_ms = frame_offset.as_millis() as u64 % total_duration_ms.max(1);
        let needed_source_idx = (time_ms / avg_delay_ms.max(1)) as usize % source_frame_count;

        if current_resized.as_ref().map(|(idx, _)| *idx) != Some(needed_source_idx) {
            // Loop wrap-around: reset decoder
            if needed_source_idx < decoded_source_idx {
                decoder.reset();
                decoded_source_idx = 0;
            }

            // Skip intermediate frames (must still decode for canvas state,
            // but discard pixel data immediately)
            let gap = needed_source_idx - decoded_source_idx;
            for _ in 0..gap {
                if decoder.next_frame().is_err() {
                    break;
                }
            }

            // Decode the target frame
            current_resized = match decoder.next_frame() {
                Ok(Some(frame)) => {
                    decoded_source_idx = needed_source_idx + 1;
                    let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.data)
                        .unwrap_or_else(|| image::RgbaImage::new(frame.width, frame.height));
                    let resized =
                        resize_rgba_image_contain(rgba, output_width, output_height, false)
                            .map_err(|e| {
                                Error::InvalidFile(format!("Failed to resize WebP frame: {}", e))
                            })?;
                    Some((needed_source_idx, resized))
                }
                _ => None,
            };
        }

        let buffer = match &current_resized {
            Some((_, img)) => img.clone(),
            None => image::RgbaImage::new(output_width, output_height),
        };

        let original_image = VideoImage::image(buffer);
        let image = original_image.clone();

        let filtered_images = if segment.video_filters.is_empty() {
            FilteredLayerImages {
                original_image,
                image,
                image_for_composite: None,
            }
        } else {
            let mut result =
                apply_segment_video_filters(config.clone(), image, segment.clone(), frame_offset);
            result.original_image = original_image;
            result
        };

        if sender
            .send((
                source_index,
                track_index,
                Some((segment_index, segment.clone())),
                filtered_images,
            ))
            .is_err()
        {
            break;
        }
    }

    Ok(())
}

fn render_image_segment(
    segment: Arc<Segment>,
    segment_index: usize,
    source_index: usize,
    track_index: usize,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
    frame_counts: usize,
    relative_timeline_offset: Duration,
    sender: Sender<(usize, usize, SegmentRef, FilteredLayerImages)>,
) -> Result<()> {
    let image_path = &segment.metadata.path;

    let is_svg = image_path
        .extension()
        .map(|e| e.to_ascii_lowercase() == "svg")
        .unwrap_or(false);

    let buffer = if is_svg {
        let rgba = render_svg_to_rgba(image_path)?;
        resize_rgba_image_contain(rgba, output_width, output_height, false)?
    } else {
        // TODO: cache image
        let img = image::open(image_path)
            .map_err(|e| Error::InvalidFile(format!("Failed to load image: {}", e)))?;
        let rgba = img.to_rgba8();
        resize_rgba_image_contain(rgba, output_width, output_height, false)?
    };

    let output_converter = TimeToFrameConverter::from_f32(output_fps);
    let config = VideoFilterConfig::new(output_width, output_height, output_fps);

    for index in 0..frame_counts {
        let frame_offset = relative_timeline_offset + output_converter.frame_to_duration(index);

        let original_image = VideoImage::image(buffer.clone());
        let mut image = original_image.clone();
        let mut image_for_composite: Option<VideoImage> = None;

        for filter in &segment.image_filters {
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

                let mut ui_data = ImageData {
                    config: config.clone(),
                    frames: vec![image],
                    from_segment: segment.clone(),
                    relative_timeline_offset: frame_offset,
                };
                if let Err(e) = filter.inner.apply(&mut ui_data) {
                    log::warn!("Apply image filter: `{}` failed: {e}", filter.inner.name());
                }
                image = ui_data.frames.pop().unwrap_or(VideoImage::Empty);
                image_for_composite = None;
            } else {
                let base = image_for_composite.as_ref().unwrap_or(&image);
                let mut composite_data = ImageData {
                    config: config.clone(),
                    frames: vec![base.clone()],
                    from_segment: segment.clone(),
                    relative_timeline_offset: frame_offset,
                };
                if let Err(e) = filter.inner.apply(&mut composite_data) {
                    log::warn!("Apply image filter: `{}` failed: {e}", filter.inner.name());
                }
                image_for_composite = composite_data.frames.pop();
            }
        }

        let filtered_images = FilteredLayerImages {
            original_image,
            image,
            image_for_composite,
        };

        if sender
            .send((
                source_index,
                track_index,
                Some((segment_index, segment.clone())),
                filtered_images,
            ))
            .is_err()
        {
            break;
        }
    }

    Ok(())
}
