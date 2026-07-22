pub const LAMA_DOWNLOAD_URL: &str =
    "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx";

/// Core configuration for subtitle removal.
#[derive(Debug, Clone)]
pub struct Config {
    // --- Mask generation ---
    /// Expand mask rectangles by this many pixels to avoid text edge residue.
    pub subtitle_area_deviation_pixel: u32,

    // --- Timeline expansion ---
    /// Expand subtitle frame ranges backward by this many frames.
    pub timeline_backward_frames: u32,
    /// Expand subtitle frame ranges forward by this many frames.
    pub timeline_forward_frames: u32,

    // --- Hardware ---
    /// Enable hardware acceleration (GPU/DirectML).
    pub hardware_acceleration: bool,

    // --- Paths ---
    /// Directory containing model files.
    pub model_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            subtitle_area_deviation_pixel: 10,
            timeline_backward_frames: 3,
            timeline_forward_frames: 3,
            hardware_acceleration: true,
            model_dir: String::new(),
        }
    }
}
