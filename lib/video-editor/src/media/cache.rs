use crate::Result;
use ffmpeg_next as ffmpeg;
use image::{ImageBuffer, Rgb};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::BufReader,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaThumbnail {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_time: Option<f64>, // Time in seconds for video thumbnails
}

impl MediaThumbnail {
    pub fn new(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            path,
            width,
            height,
            frame_time: None,
        }
    }

    pub fn with_frame_time(mut self, time: f64) -> Self {
        self.frame_time = Some(time);
        self
    }

    pub fn is_expired_on_disk(&self, max_age: Duration) -> bool {
        if !self.path.exists() {
            return true;
        }

        match fs::metadata(&self.path).and_then(|m| m.modified()) {
            Ok(modified) => modified.elapsed().unwrap_or(Duration::ZERO) > max_age,
            Err(_) => true,
        }
    }

    pub fn file_modified_time(&self) -> Option<SystemTime> {
        self.path
            .exists()
            .then(|| fs::metadata(&self.path).ok())
            .flatten()
            .and_then(|m| m.modified().ok())
    }
}

#[derive(Debug, Clone)]
pub struct MediaCache {
    pub cache_dir: PathBuf,
    thumbnail_size: (u32, u32),
    max_cache_age: Duration,
}

impl MediaCache {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            cache_dir,
            thumbnail_size: (160, 90), // Default thumbnail size
            max_cache_age: Duration::from_secs(86400), // 24 hours
        })
    }

    pub fn with_thumbnail_size(mut self, width: u32, height: u32) -> Self {
        self.thumbnail_size = (width, height);
        self
    }

    pub fn with_max_age(mut self, age: Duration) -> Self {
        self.max_cache_age = age;
        self
    }

    // Calculate dimensions that fit within target bounds while preserving aspect ratio.
    fn calculate_scaled_dimensions(original: (u32, u32), target: (u32, u32)) -> (u32, u32) {
        let original_ratio = original.0 as f64 / original.1 as f64;
        let target_ratio = target.0 as f64 / target.1 as f64;

        if original_ratio > target_ratio {
            // Original is wider, scale based on width
            let height = (target.0 as f64 / original_ratio) as u32;
            (target.0, height.max(1))
        } else {
            // Original is taller or equal, scale based on height
            let width = (target.1 as f64 * original_ratio) as u32;
            (width.max(1), target.1)
        }
    }

    pub fn get_thumbnail_path(&self, file_path: &Path) -> PathBuf {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let mtime = fs::metadata(file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut hasher = DefaultHasher::new();
        file_path.hash(&mut hasher);
        let hash = hasher.finish();
        self.cache_dir
            .join(format!("thumb_{:x}_{}_{}.jpg", hash, mtime, file_name))
    }

    pub fn is_thumbnail_current(&self, file_path: &Path, cached_path: &Path) -> bool {
        if !cached_path.exists() {
            return false;
        }

        let expected_path = self.get_thumbnail_path(file_path);
        cached_path == expected_path
    }

    pub fn get_thumbnail(&self, file_path: &Path) -> Option<MediaThumbnail> {
        let thumbnail_path = self.get_thumbnail_path(file_path);

        if !thumbnail_path.exists() {
            return None;
        }

        let thumbnail =
            MediaThumbnail::new(thumbnail_path, self.thumbnail_size.0, self.thumbnail_size.1);

        if thumbnail.is_expired_on_disk(self.max_cache_age) {
            return None;
        }

        Some(thumbnail)
    }

    pub fn generate_thumbnail(&mut self, file_path: &Path) -> Result<MediaThumbnail> {
        let thumbnail_path = self.get_thumbnail_path(file_path);

        if let Some(parent) = thumbnail_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Try FFmpeg first, fall back to image crate for formats FFmpeg can't decode
        // (e.g. animated WebP where FFmpeg reports width=0, height=0)
        let result = self
            .generate_thumbnail_from_video(file_path, &thumbnail_path)
            .or_else(|_| self.generate_thumbnail_from_image(file_path, &thumbnail_path));

        let (actual_width, actual_height) = result?;
        let frame_time = self.extract_frame_time(file_path).ok();

        let thumbnail = MediaThumbnail::new(thumbnail_path, actual_width, actual_height)
            .with_frame_time(frame_time.unwrap_or(0.0));

        Ok(thumbnail)
    }

    fn generate_thumbnail_from_video(
        &self,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(u32, u32)> {
        ffmpeg::init()
            .map_err(|e| crate::Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

        let mut input_ctx = ffmpeg::format::input(file_path)
            .map_err(|e| crate::Error::FFmpeg(format!("Failed to open input: {}", e)))?;

        let video_stream = input_ctx
            .streams()
            .find(|s| s.parameters().medium() == ffmpeg::media::Type::Video)
            .ok_or_else(|| crate::Error::InvalidFile("No video stream found".to_string()))?;

        let stream_index = video_stream.index();

        let codec_par = video_stream.parameters();
        let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par)
            .and_then(|ctx| ctx.decoder().video())
            .map_err(|e| crate::Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

        let width = decoder.width();
        let height = decoder.height();

        if width == 0 || height == 0 {
            return Err(crate::Error::InvalidFile(
                "Invalid video dimensions".to_string(),
            ));
        }

        let duration = input_ctx.duration();
        let seek_pos = duration / 4; // Seek to 25% of video
        let actual_size = Self::calculate_scaled_dimensions((width, height), self.thumbnail_size);

        if seek_pos > 0 {
            input_ctx
                .seek(seek_pos, ..seek_pos)
                .map_err(|e| crate::Error::FFmpeg(format!("Failed to seek: {}", e)))?;
        }

        let (decoded_frame, scaler) =
            Self::decode_first_frame(&mut input_ctx, stream_index, &mut decoder, actual_size)?;

        let decoded_frame =
            decoded_frame.ok_or_else(|| crate::Error::FFmpeg("No decoded frame".to_string()))?;

        let mut rgb_frame = ffmpeg::frame::Video::empty();
        scaler?
            .run(&decoded_frame, &mut rgb_frame)
            .map_err(|e| crate::Error::FFmpeg(format!("Scaling failed: {}", e)))?;

        Self::ffmpeg_frame_to_image(&rgb_frame)?.save(output_path)?;

        log::debug!(
            "Generated thumbnail: {} -> {:?}",
            file_path.display(),
            output_path
        );

        Ok(actual_size)
    }

    fn decode_first_frame(
        input_ctx: &mut ffmpeg::format::context::Input,
        stream_index: usize,
        decoder: &mut ffmpeg::decoder::Video,
        thumbnail_size: (u32, u32), // Actual calculated dimensions preserving aspect ratio
    ) -> Result<(
        Option<ffmpeg::frame::Video>,
        Result<ffmpeg::software::scaling::Context>,
    )> {
        for (stream, packet) in input_ctx.packets() {
            if stream.index() != stream_index {
                continue;
            }

            let mut decoded = ffmpeg::frame::Video::empty();
            decoder
                .send_packet(&packet)
                .map_err(|e| crate::Error::FFmpeg(format!("Send packet error: {}", e)))?;

            match decoder.receive_frame(&mut decoded) {
                Ok(_) => {
                    let scaler = ffmpeg::software::scaling::Context::get(
                        decoder.format(),
                        decoder.width(),
                        decoder.height(),
                        ffmpeg::format::Pixel::RGB24,
                        thumbnail_size.0,
                        thumbnail_size.1,
                        ffmpeg::software::scaling::Flags::BILINEAR,
                    )
                    .map_err(|e| crate::Error::FFmpeg(format!("Failed to create scaler: {}", e)));

                    return Ok((Some(decoded), scaler));
                }
                Err(ffmpeg::Error::Other { .. }) => continue,
                Err(e) => return Err(crate::Error::FFmpeg(format!("Decode error: {}", e))),
            }
        }

        let mut decoded = ffmpeg::frame::Video::empty();
        decoder
            .send_eof()
            .map_err(|e| crate::Error::FFmpeg(format!("Send EOF error: {}", e)))?;

        match decoder.receive_frame(&mut decoded) {
            Ok(_) => {
                let scaler = ffmpeg::software::scaling::Context::get(
                    decoder.format(),
                    decoder.width(),
                    decoder.height(),
                    ffmpeg::format::Pixel::RGB24,
                    thumbnail_size.0,
                    thumbnail_size.1,
                    ffmpeg::software::scaling::Flags::BILINEAR,
                )
                .map_err(|e| crate::Error::FFmpeg(format!("Failed to create scaler: {}", e)));

                Ok((Some(decoded), scaler))
            }
            Err(_) => Err(crate::Error::FFmpeg("No frame decoded".to_string())),
        }
    }

    fn ffmpeg_frame_to_image(frame: &ffmpeg::frame::Video) -> Result<image::RgbImage> {
        let width = frame.width();
        let height = frame.height();

        let format = frame.format();
        if format != ffmpeg::format::Pixel::RGB24 {
            return Err(crate::Error::FFmpeg(format!(
                "Expected RGB24, got {:?}",
                format
            )));
        }

        let data = frame.data(0);
        let stride = frame.stride(0);

        let mut img = ImageBuffer::new(width, height);
        for y in 0..height {
            let row_offset = y as usize * stride;
            for x in 0..width {
                let pixel_offset = row_offset + (x as usize * 3);
                if pixel_offset + 2 < data.len() {
                    let r = data[pixel_offset];
                    let g = data[pixel_offset + 1];
                    let b = data[pixel_offset + 2];
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }

        Ok(img)
    }

    /// Generate thumbnail using the `image` crate directly.
    /// This is a fallback for formats that FFmpeg cannot decode properly,
    /// such as animated WebP (FFmpeg skips ANIM/ANMF chunks, reporting width=0, height=0).
    fn generate_thumbnail_from_image(
        &self,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(u32, u32)> {
        let file = fs::File::open(file_path)?;
        let reader = BufReader::new(file);

        let img = image::ImageReader::new(reader)
            .with_guessed_format()
            .map_err(|e| crate::Error::InvalidFile(format!("Failed to guess image format: {}", e)))?
            .decode()
            .map_err(|e| crate::Error::InvalidFile(format!("Failed to decode image: {}", e)))?;

        let (orig_width, orig_height) = (img.width(), img.height());
        if orig_width == 0 || orig_height == 0 {
            return Err(crate::Error::InvalidFile(
                "Invalid image dimensions".to_string(),
            ));
        }

        let actual_size =
            Self::calculate_scaled_dimensions((orig_width, orig_height), self.thumbnail_size);

        let thumbnail = img.resize_exact(
            actual_size.0,
            actual_size.1,
            image::imageops::FilterType::Triangle,
        );
        let thumbnail_rgb = thumbnail.to_rgb8();
        thumbnail_rgb.save(output_path)?;

        log::debug!(
            "Generated thumbnail (image crate): {} -> {:?}",
            file_path.display(),
            output_path
        );

        Ok(actual_size)
    }

    fn extract_frame_time(&self, file_path: &Path) -> Result<f64> {
        ffmpeg::init()
            .map_err(|e| crate::Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

        let input_ctx = ffmpeg::format::input(file_path)
            .map_err(|e| crate::Error::FFmpeg(format!("Failed to open input: {}", e)))?;

        let duration_us = input_ctx.duration() as f64;
        let frame_time = (duration_us / 4.0) / 1_000_000.0;

        Ok(frame_time)
    }

    pub fn get_or_generate_thumbnail(&mut self, file_path: &Path) -> Result<MediaThumbnail> {
        if let Some(thumbnail) = self.get_thumbnail(file_path) {
            return Ok(thumbnail);
        }

        self.generate_thumbnail(file_path)
    }

    /// Public alias for get_thumbnail_path, for use by MediaList to check cache validity.
    pub fn get_current_thumbnail_path(&self, file_path: &Path) -> PathBuf {
        self.get_thumbnail_path(file_path)
    }

    pub fn clear_thumbnail_cache(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("jpg")
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("thumb_"))
                        == Some(true)
                {
                    _ = fs::remove_file(&path);
                }
            }
        }
    }

    pub fn cleanup_cache(&mut self) -> Result<()> {
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) != Some("jpg")
                    || path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.starts_with("thumb_"))
                        .unwrap_or(true)
                {
                    continue;
                }

                let should_remove = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|modified| {
                        modified.elapsed().unwrap_or(Duration::ZERO) > self.max_cache_age
                    })
                    .unwrap_or(true);

                if should_remove {
                    _ = fs::remove_file(&path);
                    log::debug!("Removed expired thumbnail: {:?}", path);
                }
            }
        }

        Ok(())
    }

    pub fn cache_size(&self) -> u64 {
        fs::read_dir(&self.cache_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        entry.path().extension().and_then(|s| s.to_str()) == Some("jpg")
                            && entry
                                .path()
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.starts_with("thumb_"))
                                == Some(true)
                    })
                    .count() as u64
            })
            .unwrap_or(0)
    }
}
