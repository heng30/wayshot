pub mod danmaku;
pub mod progress_bar;
pub mod rotation;
pub mod speed;
pub mod timer;

pub use danmaku::{
    DanmakuDistributionMode, DanmakuFilter, DanmakuItem, DanmakuSegment, DanmakuStyle,
};
pub use progress_bar::{ProgressBarFilter, ProgressSegment};
pub use rotation::RotationGlobalFilter;
pub use speed::GlobalSpeedFilter;
pub use timer::{TimerFilter, TimerMode, TimerSegment};

pub fn all_filter_names() -> &'static [&'static str] {
    &[
        ProgressBarFilter::NAME,
        RotationGlobalFilter::NAME,
        TimerFilter::NAME,
        GlobalSpeedFilter::NAME,
        DanmakuFilter::NAME,
    ]
}
