pub mod camera_client;
pub mod camera_info;
pub mod image_composition;

pub use camera_client::{CameraClient, CameraConfig, PixelFormat};
pub use camera_info::{CameraInfo, query_available_cameras, query_camera_id, query_first_camera};
pub use image::{ImageBuffer, Rgb, Rgba, RgbaImage};
pub use image_composition::{
    MixPositionWithPadding, Shape, ShapeBase, ShapeCircle, ShapeRectangle, mix_images,
    mix_images_rgb,
};

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
