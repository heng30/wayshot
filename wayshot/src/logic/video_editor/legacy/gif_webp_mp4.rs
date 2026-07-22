/// Shared logic: encode animation frames into an MP4 file.
/// `frame_iter` yields the remaining frames after the first frame has already been consumed.
/// `first_frame` is the already-decoded first frame.
/// `mp4_path` is the output file path.
/// `format_name` is used in error/log messages (e.g. "GIF", "WebP").
/// `reopen` is a closure that re-opens the source file and returns a new `Frames` iterator
/// for extra loops when the animation duration is shorter than the minimum.
fn convert_animation_to_mp4(
    mut frame_iter: image::Frames<'_>,
    first_frame: image::Frame,
    mp4_path: PathBuf,
    format_name: &str,
    reopen: impl Fn() -> Result<image::Frames<'static>, String>,
) -> Result<PathBuf, String> {
    let first_buffer = first_frame.buffer();
    let (orig_width, orig_height) = first_buffer.dimensions();

    // H.264 requires width and height to be even numbers.
    // Round up to the nearest even dimension.
    let width = (orig_width + 1) & !1;
    let height = (orig_height + 1) & !1;
    let need_resize = orig_width != width || orig_height != height;

    // Use a fixed fps for the encoder. Each frame will be repeated
    // according to its delay to match the original timing:
    //   repeat_count = max(1, delay_ms * fps / 1000)
    let fps: u32 = 25;

    let config = Mp4ProcessorConfigBuilder::default()
        .save_path(mp4_path.clone())
        .video_config(VideoConfig { width, height, fps })
        .channel_size(1024)
        .build()
        .map_err(|e| format!("Failed to build MP4 config: {}", e))?;

    let mut processor = Mp4Processor::new(config);
    let video_sender = processor.h264_sender();

    let encoder_config = VideoEncoderConfig::new(width, height).with_fps(fps);
    let mut encoder = video_encoder::new(encoder_config)
        .map_err(|e| format!("Failed to create video encoder: {}", e))?;

    let headers = encoder
        .headers()
        .map_err(|e| format!("Failed to get encoder headers: {}", e))?;

    let processor_thread = std::thread::spawn(move || {
        if let Err(e) = processor.run_processing_loop(Some(headers)) {
            log::warn!("MP4 processing error: {}", e);
        }
    });

    // Helper: resize frame to even dimensions if needed, then convert to RGB
    let prepare_frame = |rgba: RgbaImage| -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        if need_resize {
            let src_image =
                FastImage::from_vec_u8(orig_width, orig_height, rgba.into_raw(), PixelType::U8x4)
                    .expect("create fast image from rgba");
            let mut dst_buf = vec![0u8; (width * height * 4) as usize];
            let mut dst_image =
                FastImage::from_slice_u8(width, height, &mut dst_buf, PixelType::U8x4)
                    .expect("create dst fast image");
            Resizer::new()
                .resize(&src_image, &mut dst_image, None)
                .expect("resize frame");
            // Convert RGBA → RGB
            let rgb_buf: Vec<u8> = dst_buf
                .chunks_exact(4)
                .flat_map(|px| [&px[0], &px[1], &px[2]])
                .copied()
                .collect();
            image::ImageBuffer::from_raw(width, height, rgb_buf)
                .expect("create rgb buffer from raw")
        } else {
            rgba_to_rgb(&rgba)
        }
    };

    // Encode a single frame's RGBA data repeated according to its delay
    let encode_rgba = |rgba: &RgbaImage,
                       repeat: u32,
                       encoder: &mut Box<dyn VideoEncoder>,
                       video_sender: &mp4m::Sender<VideoFrameType>|
     -> Result<(), String> {
        for _ in 0..repeat {
            let rgb_buffer = prepare_frame(rgba.clone());
            let encoded = encoder
                .encode_frame(rgb_buffer)
                .map_err(|e| format!("Failed to encode frame: {}", e))?;
            if let EncodedFrame::Frame {
                data, is_keyframe, ..
            } = encoded
            {
                video_sender
                    .send(VideoFrameType::Frame {
                        data,
                        is_sync: is_keyframe,
                    })
                    .map_err(|e| format!("Failed to send encoded frame: {}", e))?;
            }
        }
        Ok(())
    };

    // Helper: read all frames from an animation decoder and encode them.
    // Returns the total duration in milliseconds of one loop.
    let encode_frames = |frame_iter: &mut image::Frames<'_>,
                         encoder: &mut Box<dyn VideoEncoder>,
                         video_sender: &mp4m::Sender<VideoFrameType>|
     -> Result<u64, String> {
        let mut total_ms: u64 = 0;
        for frame_result in frame_iter {
            let frame = match frame_result {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Skipping corrupted {} frame: {}", format_name, e);
                    continue;
                }
            };

            let (numer, denom) = frame.delay().numer_denom_ms();
            let delay_ms = numer / denom.max(1);
            let repeat = ((delay_ms as u64 * fps as u64 / 1000).max(1)) as u32;

            let rgba = frame.buffer().clone();
            drop(frame);

            encode_rgba(&rgba, repeat, encoder, video_sender)?;
            total_ms += delay_ms as u64;
        }
        Ok(total_ms)
    };

    // Encode first frame
    let (numer, denom) = first_frame.delay().numer_denom_ms();
    let first_delay_ms = numer / denom.max(1);
    let first_repeat = ((first_delay_ms as u64 * fps as u64 / 1000).max(1)) as u32;

    let first_rgba = first_buffer.clone();
    drop(first_frame);

    encode_rgba(&first_rgba, first_repeat, &mut encoder, &video_sender)?;
    let mut one_loop_ms: u64 = first_delay_ms as u64;

    // Encode remaining frames
    let remaining_ms = encode_frames(&mut frame_iter, &mut encoder, &video_sender)?;
    one_loop_ms += remaining_ms;

    // Animations can be very short (e.g. <1s) due to loop playback.
    // Repeat the frame sequence until reaching at least 5 seconds
    // so the resulting MP4 is usable in a video editor.
    const MIN_DURATION_MS: u64 = 5000;
    let extra_loops_count = if one_loop_ms > 0 && one_loop_ms < MIN_DURATION_MS {
        ((MIN_DURATION_MS - one_loop_ms + one_loop_ms - 1) / one_loop_ms) as usize
    } else {
        0
    };

    for _ in 0..extra_loops_count {
        let mut extra_frame_iter = reopen()?;
        encode_frames(&mut extra_frame_iter, &mut encoder, &video_sender)?;
    }

    let sender_clone = video_sender.clone();
    encoder
        .flush(Box::new(move |data, is_keyframe| {
            if let Err(e) = sender_clone.send(VideoFrameType::Frame {
                data,
                is_sync: is_keyframe,
            }) {
                log::warn!("Failed to send flushed data: {}", e);
            }
        }))
        .map_err(|e| format!("Failed to flush encoder: {}", e))?;

    video_sender
        .send(VideoFrameType::End)
        .map_err(|e| format!("Failed to send end signal: {}", e))?;

    drop(video_sender);

    processor_thread
        .join()
        .map_err(|_| "Processor thread error".to_string())?;

    Ok(mp4_path)
}

/// Prepare cache directory and MP4 output path for animation-to-MP4 conversion.
/// Returns `(mp4_path, true)` if a valid cached version already exists,
/// or `(mp4_path, false)` if the file needs to be generated.
fn prepare_animation_cache(
    source_path: &PathBuf,
    cache_subdir: &str,
    default_stem: &str,
) -> Result<(PathBuf, bool), String> {
    let cache_dir = crate::config::all().cache_dir.join(cache_subdir);
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        return Err(format!("Failed to create cache directory: {}", e));
    }

    let source_metadata = std::fs::metadata(source_path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;

    let mtime = source_metadata
        .modified()
        .map_err(|e| format!("Failed to get modification time: {}", e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Failed to convert modification time: {}", e))?
        .as_secs();

    let original_name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(default_stem);
    let filename = format!("{}_{}.mp4", original_name, mtime);
    let mp4_path = cache_dir.join(&filename);

    if mp4_path.exists() {
        if get_metadata(&mp4_path).is_ok() {
            return Ok((mp4_path, true));
        }
        _ = std::fs::remove_file(&mp4_path);
        log::warn!(
            "Cached MP4 is invalid, regenerating: {}",
            mp4_path.display()
        );
    }

    Ok((mp4_path, false))
}

pub fn convert_gif_to_mp4(gif_path: &PathBuf) -> Result<PathBuf, String> {
    let (mp4_path, cached) = prepare_animation_cache(gif_path, "gif_converted", "gif")?;
    if cached {
        return Ok(mp4_path);
    }

    let file =
        std::fs::File::open(gif_path).map_err(|e| format!("Failed to open GIF file: {}", e))?;
    let reader = BufReader::new(file);
    let decoder =
        GifDecoder::new(reader).map_err(|e| format!("Failed to create GIF decoder: {}", e))?;

    let mut frame_iter = decoder.into_frames();
    let first_frame = frame_iter
        .next()
        .ok_or_else(|| "GIF has no frames".to_string())?
        .map_err(|e| format!("Failed to decode first GIF frame: {}", e))?;

    let gif_path_clone = gif_path.clone();
    convert_animation_to_mp4(frame_iter, first_frame, mp4_path, "GIF", move || {
        let file = std::fs::File::open(&gif_path_clone)
            .map_err(|e| format!("Failed to reopen GIF file: {}", e))?;
        let reader = BufReader::new(file);
        let decoder = GifDecoder::new(reader)
            .map_err(|e| format!("Failed to recreate GIF decoder: {}", e))?;
        Ok(decoder.into_frames())
    })
}

fn is_animated_webp(path: &PathBuf) -> bool {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = BufReader::new(file);
    WebPDecoder::new(reader)
        .map(|decoder| decoder.has_animation())
        .unwrap_or(false)
}

pub fn convert_webp_to_mp4(webp_path: &PathBuf) -> Result<PathBuf, String> {
    let (mp4_path, cached) = prepare_animation_cache(webp_path, "webp_converted", "webp")?;
    if cached {
        return Ok(mp4_path);
    }

    let file =
        std::fs::File::open(webp_path).map_err(|e| format!("Failed to open WebP file: {}", e))?;
    let reader = BufReader::new(file);
    let decoder =
        WebPDecoder::new(reader).map_err(|e| format!("Failed to create WebP decoder: {}", e))?;

    let mut frame_iter = decoder.into_frames();
    let first_frame = frame_iter
        .next()
        .ok_or_else(|| "WebP has no frames".to_string())?
        .map_err(|e| format!("Failed to decode first WebP frame: {}", e))?;

    let webp_path_clone = webp_path.clone();
    convert_animation_to_mp4(frame_iter, first_frame, mp4_path, "WebP", move || {
        let file = std::fs::File::open(&webp_path_clone)
            .map_err(|e| format!("Failed to reopen WebP file: {}", e))?;
        let reader = BufReader::new(file);
        let decoder = WebPDecoder::new(reader)
            .map_err(|e| format!("Failed to recreate WebP decoder: {}", e))?;
        Ok(decoder.into_frames())
    })
}
