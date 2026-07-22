use crate::AnimationInit;
use background_animation::{Animation, AnimationRecordConfig, impl_animation};
use image::{DynamicImage, GenericImageView, RgbaImage};
use std::{path::PathBuf, time::Duration};

pub type Result<T> = std::result::Result<T, ImageScrollError>;

#[derive(thiserror::Error, Debug)]
pub enum ImageScrollError {
    #[error("Image height {actual}px is too small, output requires at least {required}px")]
    ImageTooSmall { actual: u32, required: u32 },
    #[error("Failed to load image: {0}")]
    LoadFailed(String),
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ImageScrollConfig {
    pub image_path: PathBuf,

    /// Scroll speed as percentage of output height per second.
    /// e.g., 0.5 means scroll 50% of viewport height each second.
    /// Default: 0.3 (30% per second)
    #[derivative(Default(value = "0.2"))]
    pub scroll_speed: f32,

    /// Pause duration at the start before scrolling begins (in seconds).
    /// Default: 1.0 second
    #[derivative(Default(value = "1.0"))]
    pub start_pause: f32,

    /// Pause duration at the end after scrolling completes (in seconds).
    /// Default: 1.0 second
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,

    /// Output height (viewport height). Width is automatically set to image width.
    pub output_height: u32,

    /// Frames per second for output video.
    /// Default: 25
    #[derivative(Default(value = "25"))]
    pub fps: u32,

    #[setters(skip)]
    output_width: u32,

    #[setters(skip)]
    image_height: u32,

    #[setters(skip)]
    start_frames: usize,

    #[setters(skip)]
    scroll_frames: usize,

    #[setters(skip)]
    end_frames: usize,

    #[setters(skip)]
    total_frames: usize,

    #[setters(skip)]
    current_frame: usize,

    #[setters(skip)]
    image: Option<RgbaImage>,
}

impl ImageScrollConfig {
    pub fn new(image_path: PathBuf) -> Self {
        Self {
            image_path,
            ..Default::default()
        }
    }

    pub fn load_image(&self) -> Result<DynamicImage> {
        image::open(&self.image_path).map_err(|e| ImageScrollError::LoadFailed(e.to_string()))
    }

    pub fn validate(&self) -> Result<u32> {
        let img = self.load_image()?;
        let (img_width, img_height) = (img.width(), img.height());

        if img_height < self.output_height {
            return Err(ImageScrollError::ImageTooSmall {
                actual: img_height,
                required: self.output_height,
            });
        }

        // x264 encoder requires width divisible by 2
        Ok(img_width - (img_width % 2))
    }

    /// Record the scroll animation to an MP4 file.
    /// This is a convenience method that wraps the configuration and recording process.
    ///
    /// # Example
    /// ```no_run
    /// let mut scroll = ImageScrollConfig::new("image.png".into())
    ///     .with_output_height(1080)
    ///     .with_fps(30)
    ///     .with_scroll_speed(0.2)
    ///     .with_start_pause(2.0)
    ///     .with_end_pause(3.0);
    ///
    /// scroll.record("output.mp4").expect("Failed to record");
    /// ```
    pub fn record(&mut self, output_path: impl Into<PathBuf>) -> crate::Result<()> {
        let width = self
            .validate()
            .map_err(|e| crate::AnimationError::Encoding(e.to_string()))?;
        let config = AnimationRecordConfig::new(
            width,
            self.output_height,
            self.fps,
            Duration::from_secs(3600),
            output_path.into(),
        );
        self.animate_record(config)
    }

    fn calculate_y_offset(&self, frame: usize) -> u32 {
        let image = self.image.as_ref().unwrap();
        let max_y = image.height().saturating_sub(self.output_height);

        if frame < self.start_frames {
            // Phase 0: Start pause - stay at top
            return 0;
        }

        let scroll_frame = frame - self.start_frames;
        if scroll_frame < self.scroll_frames {
            // Phase 1: Scrolling
            let scroll_pixels_per_second = self.scroll_speed * self.output_height as f32;
            let scroll_pixels_per_frame = scroll_pixels_per_second / self.fps as f32;
            let y = (scroll_frame as f32 * scroll_pixels_per_frame) as u32;
            return y.min(max_y);
        }

        // Phase 2: End pause - stay at bottom
        max_y
    }
}

impl AnimationInit for ImageScrollConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.output_width = width;
        self.output_height = height;
        self.fps = fps;
        self.current_frame = 0;

        let image_result = image::open(&self.image_path);
        match image_result {
            Ok(img) => {
                let rgba_img = img.to_rgba8();
                // x264 encoder requires width divisible by 2
                self.output_width = rgba_img.width() - (rgba_img.width() % 2);
                self.image_height = rgba_img.height();

                if self.image_height < self.output_height {
                    log::error!(
                        "Image height {} is less than output height {}",
                        self.image_height,
                        self.output_height
                    );
                    self.image = None;
                    return;
                }

                self.start_frames = (self.start_pause * fps as f32).ceil() as usize;
                self.end_frames = (self.end_pause * fps as f32).ceil() as usize;

                // Scroll frames: distance / (speed * output_height / fps)
                let scroll_distance = (self.image_height - self.output_height) as f32;
                let scroll_pixels_per_second = self.scroll_speed * self.output_height as f32;
                let scroll_pixels_per_frame = scroll_pixels_per_second / fps as f32;
                self.scroll_frames = if scroll_pixels_per_frame > 0.0 {
                    (scroll_distance / scroll_pixels_per_frame).ceil() as usize
                } else {
                    0
                };

                self.total_frames = self.start_frames + self.scroll_frames + self.end_frames;

                self.image = Some(rgba_img);
            }
            Err(e) => {
                log::error!("Failed to load image: {}", e);
                self.image = None;
            }
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, _frames: usize) {
        // Ignore external set_total_frames from macro
        // We calculate total_frames based on scroll parameters
    }
}

impl Iterator for ImageScrollConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        let image = self.image.as_ref()?;
        let y_offset = self.calculate_y_offset(self.current_frame);

        let viewport = image
            .view(0, y_offset, self.output_width, self.output_height)
            .to_image();

        self.current_frame += 1;
        Some(viewport)
    }
}

impl_animation!(ImageScrollConfig);
