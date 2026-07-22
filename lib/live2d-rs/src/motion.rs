use std::{fs, path::Path};

use crate::{
    json::{
        Motion3, apply_motion_fade, motion_fade_in_weight, motion_fade_out_weight,
        parameter_curve_fade_weight,
    },
    runtime::ModelRuntime,
};

const PARAMETER_TARGET: &str = "Parameter";
const PART_OPACITY_TARGET: &str = "PartOpacity";

#[derive(Debug, Clone)]
pub struct MotionPlayer {
    motion: Motion3,
    time: f32,
    weight: f32,
    finished: bool,
}

impl MotionPlayer {
    pub fn new(motion: Motion3) -> Self {
        Self {
            motion,
            time: 0.0,
            weight: 1.0,
            finished: false,
        }
    }

    pub fn motion(&self) -> &Motion3 {
        &self.motion
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight.clamp(0.0, 1.0);
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn restart(&mut self) {
        self.time = 0.0;
        self.finished = false;
    }

    /// Seek to an absolute time. Sets the internal time and derives
    /// the `finished` flag (non-looping motions finish when time >= duration).
    pub fn seek_to(&mut self, time: f32) {
        self.time = time.max(0.0);
        let duration = self.motion.meta().duration();
        if duration <= 0.0 {
            self.finished = false;
            return;
        }
        if self.motion.meta().is_looping() {
            self.time %= duration;
            self.finished = false;
        } else if self.time >= duration {
            self.time = duration;
            self.finished = true;
        } else {
            self.finished = false;
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        if self.finished {
            return;
        }

        self.time += delta_seconds.max(0.0);
        let duration = self.motion.meta().duration();
        if duration <= 0.0 {
            return;
        }

        if self.motion.meta().is_looping() {
            self.time %= duration;
        } else if self.time >= duration {
            self.time = duration;
            self.finished = true;
        }
    }

    pub fn apply(&self, runtime: &mut ModelRuntime) {
        let duration = self.motion.meta().duration();
        let end_time = if self.motion.meta().is_looping() {
            -1.0
        } else {
            duration
        };
        let fade_in = motion_fade_in_weight(self.time, 0.0, 0.0);
        let fade_out = motion_fade_out_weight(self.time, end_time, 0.0);

        for curve in self.motion.curves() {
            let Some(sampled) = curve.sample(self.time) else {
                continue;
            };
            let curve_weight = parameter_curve_fade_weight(
                self.weight,
                fade_in,
                fade_out,
                curve.fade_in_time(),
                curve.fade_out_time(),
                self.time,
                0.0,
                end_time,
            );

            match curve.target() {
                PARAMETER_TARGET => {
                    let Some(index) = runtime.parameter_index(curve.id()) else {
                        continue;
                    };
                    let Some(current) = runtime.parameter_value_by_index(index) else {
                        continue;
                    };
                    let value = apply_motion_fade(current, sampled, curve_weight);
                    runtime.set_parameter_by_index(index, value);
                }
                PART_OPACITY_TARGET => {
                    let Some(index) = runtime.part_index(curve.id()) else {
                        continue;
                    };
                    let value = apply_motion_fade(1.0, sampled, curve_weight);
                    runtime.set_part_opacity_by_index(index, value);
                }
                _ => {}
            }
        }
    }
}

pub fn load_motion(path: impl AsRef<Path>) -> Result<Motion3, MotionLoadError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| MotionLoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Motion3::from_json_str(&source).map_err(MotionLoadError::Parse)
}

#[derive(Debug)]
pub enum MotionLoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse(crate::Error),
}

impl std::fmt::Display for MotionLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "failed to read {path}: {source}"),
            Self::Parse(error) => write!(formatter, "failed to parse motion3: {error}"),
        }
    }
}

impl std::error::Error for MotionLoadError {}
