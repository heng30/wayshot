use crate::{CameraError, CameraResult, rgb_to_rgba, rgba_to_rgb};
use derivative::Derivative;
use derive_setters::Setters;
use image::{RgbImage, RgbaImage, imageops};
use nokhwa::{
    CallbackCamera,
    pixel_format::{RgbAFormat, RgbFormat},
    utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelFormat {
    #[default]
    RGBA,
    RGB,
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct CameraConfig {
    #[derivative(Default(value = "None"))]
    #[setters[strip_option]]
    pub fps: Option<u32>,

    #[derivative(Default(value = "None"))]
    #[setters[strip_option]]
    pub width: Option<u32>,

    #[derivative(Default(value = "None"))]
    #[setters[strip_option]]
    pub height: Option<u32>,

    #[derivative(Default(value = "PixelFormat::RGBA"))]
    pub pixel_format: PixelFormat,

    #[derivative(Default(value = "false"))]
    pub mirror_horizontal: bool,
}

pub struct CameraClient {
    camera: Option<CallbackCamera>,
    is_running: Arc<AtomicBool>,
    pixel_format: PixelFormat,
    mirror_horizontal: bool,
}

impl CameraClient {
    pub fn new(camera_index: CameraIndex, config: CameraConfig) -> CameraResult<Self> {
        let pixel_format = config.pixel_format;
        let mirror_horizontal = config.mirror_horizontal;
        let format_type = RequestedFormatType::AbsoluteHighestFrameRate;
        let format = match pixel_format {
            PixelFormat::RGBA => RequestedFormat::new::<RgbAFormat>(format_type),
            PixelFormat::RGB => RequestedFormat::new::<RgbFormat>(format_type),
        };

        let mut camera = CallbackCamera::new(camera_index, format, move |_| {})
            .map_err(|e| CameraError::InitializationError(e.to_string()))?;

        if let Some(fps) = config.fps
            && let Err(e) = camera.set_frame_rate(fps)
        {
            log::warn!("camera set frame rate ({fps}) failed: {e}");
        }

        if let Some(w) = config.width
            && let Some(h) = config.height
            && let Err(e) = camera.set_resolution(Resolution::new(w, h))
        {
            log::warn!("camera set resolution ({w} x {h}) failed: {e}");
        }

        Ok(Self {
            camera: Some(camera),
            is_running: Arc::new(AtomicBool::new(false)),
            pixel_format,
            mirror_horizontal,
        })
    }

    pub fn start(&mut self) -> CameraResult<()> {
        if let Some(ref mut camera) = self.camera {
            camera
                .open_stream()
                .map_err(|e| CameraError::StartError(e.to_string()))?;
            self.is_running.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            Err(CameraError::InitializationError(
                "Camera not initialized".to_string(),
            ))
        }
    }

    pub fn stop(&mut self) -> CameraResult<()> {
        if let Some(ref mut camera) = self.camera {
            camera
                .stop_stream()
                .map_err(|e| CameraError::StopError(e.to_string()))?;
            self.is_running.store(false, Ordering::Relaxed);
            Ok(())
        } else {
            Err(CameraError::StopError("Camera not initialized".to_string()))
        }
    }

    pub fn last_frame_rgba(&self) -> CameraResult<RgbaImage> {
        match self.camera {
            Some(ref c) => {
                let buffer = c.last_frame()?;

                let mut image = match self.pixel_format {
                    PixelFormat::RGBA => buffer.decode_image::<RgbAFormat>()?,
                    PixelFormat::RGB => {
                        if let Ok(rgb_image) = buffer.decode_image::<RgbFormat>() {
                            rgb_to_rgba(rgb_image)
                        } else {
                            return Err(CameraError::NoFrameAvailable);
                        }
                    }
                };

                if self.mirror_horizontal {
                    imageops::flip_horizontal_in_place(&mut image);
                }

                Ok(image)
            }
            None => Err(CameraError::InitializationError("No camera".to_string())),
        }
    }

    pub fn last_frame_rgb(&self) -> CameraResult<RgbImage> {
        match self.camera {
            Some(ref c) => {
                let buffer = c.last_frame()?;

                let mut image = match self.pixel_format {
                    PixelFormat::RGB => buffer.decode_image::<RgbFormat>()?,
                    PixelFormat::RGBA => {
                        if let Ok(rgba_image) = buffer.decode_image::<RgbAFormat>() {
                            rgba_to_rgb(rgba_image)
                        } else {
                            return Err(CameraError::NoFrameAvailable);
                        }
                    }
                };

                if self.mirror_horizontal {
                    imageops::flip_horizontal_in_place(&mut image);
                }

                Ok(image)
            }
            None => Err(CameraError::InitializationError("No camera".to_string())),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn frame_rate(&self) -> u32 {
        self.camera
            .as_ref()
            .map_or(24, |c| c.frame_rate().unwrap_or(24))
    }
}

impl Drop for CameraClient {
    fn drop(&mut self) {
        if self.is_running() {
            _ = self.stop();
        }
    }
}
