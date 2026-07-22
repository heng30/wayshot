pub mod col_sample;
pub mod cutpoint;
pub mod error;
pub mod stitch;
pub mod template;
pub mod util;

pub use error::StitchError;
pub use stitch::{stitch, Algorithm, StitchConfig, StitchOutcome, Stitcher};