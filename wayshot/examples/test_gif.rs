use std::io::BufReader;
use fast_image_resize::{PixelType, Resizer, images::Image as FastImage};
use image::{AnimationDecoder, RgbaImage, codecs::gif::GifDecoder};
use mp4m::{Mp4Processor, Mp4ProcessorConfigBuilder, VideoConfig, VideoFrameType};
use video_editor::metadata::get_metadata;
use video_encoder::{EncodedFrame, VideoEncoderConfig};
use video_utils::convert::rgba_to_rgb;

fn convert_gif_to_mp4(gif_path: &std::path::PathBuf) -> Result<std::path::PathBuf, String> {
    let mp4_path = std::path::PathBuf::from("/tmp/test_gif_out.mp4");
    if mp4_path.exists() {
        std::fs::remove_file(&mp4_path).ok();
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

    let first_buffer = first_frame.buffer();
    let (orig_width, orig_height) = first_buffer.dimensions();

    let width = (orig_width + 1) & !1;
    let height = (orig_height + 1) & !1;
    let need_resize = orig_width != width || orig_height != height;

    let fps: u32 = 25;

    println!(
        "orig: {}x{}, even: {}x{}, need_resize: {}",
        orig_width, orig_height, width, height, need_resize
    );

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
            eprintln!("MP4 processing error: {}", e);
        }
    });

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

    let (numer, denom) = first_frame.delay().numer_denom_ms();
    let first_delay_ms = numer / denom.max(1);
    let first_repeat = ((first_delay_ms as u64 * fps as u64 / 1000).max(1)) as u32;

    println!("first frame: delay_ms={}, repeat={}", first_delay_ms, first_repeat);

    let first_rgba = first_buffer.clone();
    drop(first_frame);

    for _ in 0..first_repeat {
        let rgb_buffer = prepare_frame(first_rgba.clone());
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

    for frame_result in frame_iter {
        let frame = match frame_result {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Skipping corrupted GIF frame: {}", e);
                continue;
            }
        };

        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = numer / denom.max(1);
        let repeat = ((delay_ms as u64 * fps as u64 / 1000).max(1)) as u32;

        println!("subsequent frame: delay_ms={}, repeat={}", delay_ms, repeat);

        let rgba = frame.buffer().clone();
        println!("  buffer: {}x{}", rgba.width(), rgba.height());
        drop(frame);

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
    }

    let sender_clone = video_sender.clone();
    encoder
        .flush(Box::new(move |data, is_keyframe| {
            if let Err(e) = sender_clone.send(VideoFrameType::Frame {
                data,
                is_sync: is_keyframe,
            }) {
                eprintln!("Failed to send flushed data: {}", e);
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

    println!("Conversion done: {}", mp4_path.display());

    // Verify the output
    match get_metadata(&mp4_path) {
        Ok(meta) => println!("Metadata: {:?}", meta),
        Err(e) => eprintln!("Failed to get metadata: {}", e),
    }

    Ok(mp4_path)
}

fn main() {
    let gif_path = std::path::PathBuf::from("/data/Code/rust/wayshot/tmp/test.gif");
    match convert_gif_to_mp4(&gif_path) {
        Ok(path) => println!("Success: {}", path.display()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
