pub mod camera_client;
pub mod camera_info;
pub mod image_composition;

pub use camera_client::{CameraClient, CameraConfig, PixelFormat};
pub use camera_info::CameraInfo;
pub use image::{Rgba, RgbaImage};
pub use image_composition::{Shape, ShapeCircle, ShapeRectangle, mix_images};

pub type CameraResult<T> = Result<T, CameraError>;

#[derive(thiserror::Error, Debug)]
pub enum CameraError {
    #[error("Failed to query cameras: {0}")]
    QueryError(String),

    #[error("Failed to initialize camera: {0}")]
    InitializationError(String),

    #[error("Failed to start camera: {0}")]
    StartError(String),

    #[error("Failed to stop camera: {0}")]
    StopError(String),

    #[error("No frame available")]
    NoFrameAvailable,

    #[error("Invalid camera index: {0}")]
    InvalidCameraIndex(usize),

    #[error("Invalid pixel format")]
    InvalidPixelFormat,

    #[error("Image processing error: {0}")]
    ImageError(String),

    #[error("Fast image buffer error: {0}")]
    FastImageBufferError(#[from] fast_image_resize::ImageBufferError),

    #[error("Fast image resize error: {0}")]
    FastImageResizeError(#[from] fast_image_resize::ResizeError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Image error: {0}")]
    ImageLibraryError(#[from] image::ImageError),

    #[error("Camera error: {0}")]
    NokhwaError(#[from] nokhwa::NokhwaError),
}

pub fn init() {
    #[cfg(target_os = "macos")]
    nokhwa_initialize(|granted| {
        log::info!("User said {} for nokhwa", granted);
    });
}

pub fn rgb_to_rgba(rgb_image: &image::RgbImage) -> RgbaImage {
    use image::{Rgba, RgbaImage};

    let (width, height) = rgb_image.dimensions();
    let mut rgba_img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = rgb_image.get_pixel(x, y);
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            rgba_img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    rgba_img
}
