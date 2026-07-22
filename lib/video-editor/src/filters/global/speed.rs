use crate::{
    Result,
    filters::traits::{GlobalFilter, GlobalFilterData},
};

/// Global speed filter that affects the entire video project's playback speed.
/// Unlike Timer and ProgressBar filters which process pixels, this filter affects
/// decoding by multiplying the global speed with each segment's playback_speed.
/// The effective speed = segment.playback_speed * global_speed.
#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GlobalSpeedFilter {
    /// Speed multiplier. Range 0.1 ~ 10.0. Default is 1.0.
    #[derivative(Default(value = "1.0"))]
    pub speed: f32,
}

impl GlobalSpeedFilter {
    pub const NAME: &'static str = "global speed";

    pub fn new() -> Self {
        Self::default()
    }
}

impl GlobalFilter for GlobalSpeedFilter {
    crate::impl_default_global_filter!(GlobalSpeedFilter);

    fn apply(&self, _data: &mut GlobalFilterData) -> Result<()> {
        // Global speed is applied at the decoding layer, not here.
        // This filter's speed value is extracted in manager.rs and passed
        // to UnifiedMixerConfig, where it's multiplied with segment.playback_speed.
        Ok(())
    }
}

