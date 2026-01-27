use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::Path;

/// Video frame data
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Pixel format
    pub pixel_format: String,

    /// Frame data (raw RGB bytes)
    pub data: Vec<u8>,

    /// Presentation timestamp in seconds
    pub pts: f64,

    /// Frame number
    pub frame_number: usize,
}

/// Extract a single frame at a specific time
///
/// # Arguments
///
/// * `video_path` - Path to the video file
/// * `time` - Time in seconds
///
/// # Returns
///
/// Returns `VideoFrame` containing the frame data
///
/// # Example
///
/// ```no_run
/// use video_utils::video_frame::extract_frame_at_time;
///
/// let frame = extract_frame_at_time("video.mp4", 5.0).unwrap();
/// println!("Extracted frame: {}x{}", frame.width, frame.height);
/// ```
pub fn extract_frame_at_time<P: AsRef<Path>>(
    video_path: P,
    time: f64,
) -> Result<VideoFrame> {
    let video_path = video_path.as_ref();
    let path_str = video_path.to_string_lossy().to_string();

    if !video_path.exists() {
        return Err(Error::IO(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path_str),
        )));
    }

    log::info!("Extracting frame at {:.2} seconds from {}", time, path_str);

    // Initialize FFmpeg
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input
    let mut input_ctx = ffmpeg::format::input(&path_str)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    // Find video stream
    let video_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found in input file".to_string()))?;

    let video_stream_index = video_stream.index();
    let time_base = video_stream.time_base();
    let codec_par = video_stream.parameters();

    // Seek to the specified time
    let seek_timestamp = (time * 10000.0) as i64; // Convert to AV_TIME_BASE
    input_ctx
        .seek(seek_timestamp, ..)
        .map_err(|e| Error::FFmpeg(format!("Failed to seek: {}", e)))?;
    let decoder_context = ffmpeg::codec::context::Context::from_parameters(codec_par)
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?;

    let mut decoder = decoder_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create video decoder: {}", e)))?;

    let width = decoder.width();
    let height = decoder.height();

    log::debug!("Video info: {}x{}", width, height);

    // Create scaler to convert to RGB24 (3 bytes per pixel: R, G, B)
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::RGB24,
        width,
        height,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .map_err(|e| Error::FFmpeg(format!("Failed to create scaler: {}", e)))?;

    // Decode frames
    let mut decoded_frame = ffmpeg::frame::Video::empty();
    let mut rgb_frame = ffmpeg::frame::Video::empty();
    let mut packet = ffmpeg::Packet::empty();

    for (stream, mut packet) in input_ctx.packets() {
        if stream.index() != video_stream_index {
            continue;
        }

        decoder.send_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Decoder send failed: {}", e)))?;

        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            // Check if this is the frame we want
            if let Some(pts) = decoded_frame.pts() {
                let frame_time = pts as f64 * time_base.numerator() as f64
                    / time_base.denominator() as f64;

                if frame_time >= time {
                    // Convert to RGB8
                    scaler.run(&decoded_frame, &mut rgb_frame)
                        .map_err(|e| Error::FFmpeg(format!("Scaler run failed: {}", e)))?;

                    // Extract data
                    let stride = rgb_frame.stride(0);
                    let data = rgb_frame.data(0);
                    let data_size = stride * height as usize;

                    if !data.is_empty() && data_size > 0 {
                        let mut frame_data = vec![0u8; data_size];
                        frame_data.copy_from_slice(&data[..data_size]);

                        return Ok(VideoFrame {
                            width,
                            height,
                            pixel_format: "rgb24".to_string(),
                            data: frame_data,
                            pts: frame_time,
                            frame_number: 0,
                        });
                    }
                }
            }
        }
    }

    Err(Error::FFmpeg("Failed to extract frame at specified time".to_string()))
}

/// Extract multiple frames at regular intervals
///
/// # Arguments
///
/// * `video_path` - Path to the video file
/// * `start_time` - Start time in seconds
/// * `end_time` - End time in seconds
/// * `interval` - Interval between frames in seconds
///
/// # Returns
///
/// Returns a vector of `VideoFrame` objects
///
/// # Example
///
/// ```no_run
/// use video_utils::video_frame::extract_frames_interval;
///
/// let frames = extract_frames_interval("video.mp4", 0.0, 10.0, 1.0).unwrap();
/// println!("Extracted {} frames", frames.len());
/// ```
pub fn extract_frames_interval<P: AsRef<Path>>(
    video_path: P,
    start_time: f64,
    end_time: f64,
    interval: f64,
) -> Result<Vec<VideoFrame>> {
    let video_path = video_path.as_ref();
    let path_str = video_path.to_string_lossy().to_string();

    if !video_path.exists() {
        return Err(Error::IO(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path_str),
        )));
    }

    log::info!(
        "Extracting frames from {} ({:.2}s to {:.2}s, interval: {:.2}s)",
        path_str,
        start_time,
        end_time,
        interval
    );

    // Initialize FFmpeg
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input
    let mut input_ctx = ffmpeg::format::input(&path_str)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    // Find video stream
    let video_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found in input file".to_string()))?;

    let video_stream_index = video_stream.index();
    let time_base = video_stream.time_base();

    // Create decoder
    let codec_par = video_stream.parameters();
    let decoder_context = ffmpeg::codec::context::Context::from_parameters(codec_par)
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?;

    let mut decoder = decoder_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create video decoder: {}", e)))?;

    let width = decoder.width();
    let height = decoder.height();

    // Create scaler to convert to RGB24 (3 bytes per pixel: R, G, B)
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::RGB24,
        width,
        height,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .map_err(|e| Error::FFmpeg(format!("Failed to create scaler: {}", e)))?;

    let mut frames = Vec::new();
    let mut next_frame_time = start_time;
    let mut frame_count = 0;

    // Seek to start time
    let seek_timestamp = (start_time * 10000.0) as i64;
    input_ctx
        .seek(seek_timestamp, ..)
        .map_err(|e| Error::FFmpeg(format!("Failed to seek: {}", e)))?;

    // Decode frames
    let mut decoded_frame = ffmpeg::frame::Video::empty();
    let mut rgb_frame = ffmpeg::frame::Video::empty();
    let mut packet = ffmpeg::Packet::empty();

    for (stream, mut packet) in input_ctx.packets() {
        if stream.index() != video_stream_index {
            continue;
        }

        decoder.send_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Decoder send failed: {}", e)))?;

        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            if let Some(pts) = decoded_frame.pts() {
                let frame_time = pts as f64 * time_base.numerator() as f64
                    / time_base.denominator() as f64;

                if frame_time > end_time {
                    break;
                }

                // Extract frame at the specified interval
                if frame_time >= next_frame_time {
                    scaler.run(&decoded_frame, &mut rgb_frame)
                        .map_err(|e| Error::FFmpeg(format!("Scaler run failed: {}", e)))?;

                    let stride = rgb_frame.stride(0);
                    let data = rgb_frame.data(0);
                    let data_size = stride * height as usize;

                    if !data.is_empty() && data_size > 0 {
                        let mut frame_data = vec![0u8; data_size];
                        frame_data.copy_from_slice(&data[..data_size]);

                        frames.push(VideoFrame {
                            width,
                            height,
                            pixel_format: "rgb24".to_string(),
                            data: frame_data,
                            pts: frame_time,
                            frame_number: frame_count,
                        });

                        frame_count += 1;
                        next_frame_time += interval;

                        log::debug!(
                            "Extracted frame {} at {:.2}s",
                            frame_count,
                            frame_time
                        );
                    }
                }
            }
        }
    }

    log::info!("Extracted {} frames", frames.len());

    Ok(frames)
}

/// Extract all frames from the video (at 1 fps interval)
///
/// # Arguments
///
/// * `video_path` - Path to the video file
///
/// # Returns
///
/// Returns a vector of `VideoFrame` objects
pub fn extract_all_frames<P: AsRef<Path>>(video_path: P) -> Result<Vec<VideoFrame>> {
    let video_path = video_path.as_ref();

    // Get metadata first
    let metadata = super::metadata::get_metadata(video_path)?;

    let duration = metadata.duration;
    let fps = 25.0; // Default to 25 fps
    let interval = 1.0 / fps;

    extract_frames_interval(video_path, 0.0, duration, interval)
}

/// Save frame as image file (PNG, JPG, etc.)
///
/// # Arguments
///
/// * `frame` - The video frame to save
/// * `output_path` - Path where to save the image
///
/// # Returns
///
/// Returns `Ok(())` on success
pub fn save_frame_as_image<P: AsRef<Path>>(frame: &VideoFrame, output_path: P) -> Result<()> {
    let output_path = output_path.as_ref();
    let path_str = output_path.to_string_lossy().to_string();

    // Create image from RGB data
    let img: image::RgbImage = image::RgbImage::from_raw(frame.width, frame.height, frame.data.clone())
        .ok_or_else(|| Error::InvalidConfig("Failed to create image from frame data".to_string()))?;

    // Save image
    img.save(output_path)
        .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to save image: {}", e))))?;

    log::info!("Saved frame to {}", path_str);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_frame_at_time() {
        // Test requires actual video file
        // let frame = extract_frame_at_time("test.mp4", 5.0).unwrap();
        // assert_eq!(frame.width, 1920);
    }
}
