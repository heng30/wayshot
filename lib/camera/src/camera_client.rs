use crate::{CameraError, CameraResult, rgb_to_rgba};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
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
}

pub struct CameraClient {
    camera: Option<CallbackCamera>,
    is_running: Arc<AtomicBool>,
    pixel_format: PixelFormat,
}

impl CameraClient {
    pub fn new(camera_index: CameraIndex, config: CameraConfig) -> CameraResult<Self> {
        let pixel_format = config.pixel_format;
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

    pub fn last_frame(&self) -> CameraResult<RgbaImage> {
        match self.camera {
            Some(ref c) => {
                let buffer = c.last_frame()?;

                match self.pixel_format {
                    PixelFormat::RGBA => Ok(buffer.decode_image::<RgbAFormat>()?),
                    PixelFormat::RGB => {
                        if let Ok(rgb_image) = buffer.decode_image::<RgbFormat>() {
                            Ok(rgb_to_rgba(&rgb_image))
                        } else {
                            Err(CameraError::NoFrameAvailable)
                        }
                    }
                }
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
}

impl Drop for CameraClient {
    fn drop(&mut self) {
        if self.is_running() {
            _ = self.stop();
        }
    }
}
