pub mod audio;
pub mod global;
pub mod interpolation;
pub mod keyframe;
pub mod subtitle;
pub mod text;
pub mod traits;
pub mod video;

pub use global::*;
pub use interpolation::*;
pub use keyframe::*;
pub use traits::*;

use std::time::Duration;

pub fn progress_ratio_from_offset(frame_time_offset: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    let ratio = frame_time_offset.as_secs_f32() / duration.as_secs_f32();
    ratio.clamp(0.0, 1.0)
}
