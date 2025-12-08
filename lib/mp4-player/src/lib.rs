use thiserror::Error;

pub mod metadata;
pub mod player;
pub mod video_decoder;

pub use player::{Config, DecodedVideoFrame, Mp4Player, VideoFrame};

pub type Result<T> = std::result::Result<T, MP4PlayerError>;

#[derive(Error, Debug)]
pub enum MP4PlayerError {
    #[error("Failed to open MP4 file: {0}")]
    FileOpenError(#[from] std::io::Error),

    #[error("MP4 parsing error: {0}")]
    ParseError(#[from] mp4::Error),

    #[error("Channel communication error: {0}")]
    ChannelError(String),

    #[error("Tracker error: {0}")]
    TrackError(String),

    #[error("Frame parsing error: {0}")]
    FrameError(String),

    #[error("Mp4 Player stop error: {0}")]
    PlayerStopError(String),
}

pub fn yuv420_to_rgb(
    yuv_frame: &openh264::decoder::DecodedYUV,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    use openh264::formats::YUVSource;
    use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix, yuv420_to_rgb};

    let y_plane = yuv_frame.y();
    let u_plane = yuv_frame.u();
    let v_plane = yuv_frame.v();
    let y_plane_len = y_plane.len();
    let u_plane_len = u_plane.len();
    let v_plane_len = v_plane.len();

    let height_usize = height as usize;
    let yuv_planar_image = YuvPlanarImage {
        y_plane,
        y_stride: (y_plane_len / height_usize) as u32, // Calculate actual stride from data length
        u_plane,
        u_stride: (u_plane_len / (height_usize / 2)) as u32, // U plane stride for 420 format
        v_plane,
        v_stride: (v_plane_len / (height_usize / 2)) as u32, // V plane stride for 420 format
        width,
        height,
    };

    let mut rgb_data = vec![0u8; (width * height * 3) as usize];
    yuv420_to_rgb(
        &yuv_planar_image,
        &mut rgb_data,
        width * 3,                // RGB stride (3 bytes per pixel)
        YuvRange::Limited,        // TV range (16-235) - matches encoder
        YuvStandardMatrix::Bt601, // BT.601 standard - matches encoder
    )
    .map_err(|e| MP4PlayerError::FrameError(format!("YUV to RGB conversion failed: {:?}", e)))?;

    Ok(rgb_data)
}
