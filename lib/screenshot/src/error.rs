/// Errors that can occur during stitching.
#[derive(Debug)]
pub enum StitchError {
    /// No images were provided.
    EmptyInput,
    /// Images have inconsistent widths.
    DifferentWidths {
        first: u32,
        mismatch: u32,
        index: usize,
    },
}

impl std::fmt::Display for StitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StitchError::EmptyInput => write!(f, "no images provided"),
            StitchError::DifferentWidths {
                first,
                mismatch,
                index,
            } => {
                write!(
                    f,
                    "image {index} has width {mismatch}, expected {first}"
                )
            }
        }
    }
}

impl std::error::Error for StitchError {}
