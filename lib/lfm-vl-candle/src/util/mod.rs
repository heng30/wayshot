pub mod modules;
pub mod tensor_utils;
pub mod img_utils;
pub mod interpolate;

pub use tensor_utils::*;
pub use img_utils::*;
pub use interpolate::*;

/// Round `num` to the nearest multiple of `factor`.
pub fn round_by_factor(num: u32, factor: u32) -> u32 {
    let round = (num as f32 / factor as f32).round() as u32;
    round * factor
}

/// Floor `num` to the nearest multiple of `factor`.
pub fn floor_by_factor(num: f32, factor: u32) -> u32 {
    let floor = (num / factor as f32).floor() as u32;
    floor * factor
}

/// Ceil `num` to the nearest multiple of `factor`.
pub fn ceil_by_factor(num: f32, factor: u32) -> u32 {
    let ceil = (num / factor as f32).ceil() as u32;
    ceil * factor
}
