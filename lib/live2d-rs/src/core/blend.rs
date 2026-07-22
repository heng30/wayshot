#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BlendSlot {
    Skip,
    Single {
        base: usize,
        index: usize,
        weight: f32,
        final_weight: f32,
    },
    Pair {
        base: usize,
        index0: usize,
        weight0: f32,
        index1: usize,
        weight1: f32,
        final_weight: f32,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rgb {
    r: f32,
    g: f32,
    b: f32,
}

impl Rgb {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn r(&self) -> f32 {
        self.r
    }

    pub fn g(&self) -> f32 {
        self.g
    }

    pub fn b(&self) -> f32 {
        self.b
    }
}

pub fn blend_scalar_slots(slots: &[BlendSlot], source_values: &[f32], initial: f32) -> Option<f32> {
    let mut out = initial;

    for slot in slots {
        match *slot {
            BlendSlot::Skip => {}
            BlendSlot::Single {
                base,
                index,
                weight,
                final_weight,
            } => {
                let value = *source_values.get(base.checked_add(index)?)?;
                out += value * weight * final_weight;
            }
            BlendSlot::Pair {
                base,
                index0,
                weight0,
                index1,
                weight1,
                final_weight,
            } => {
                let value0 = *source_values.get(base.checked_add(index0)?)?;
                let value1 = *source_values.get(base.checked_add(index1)?)?;
                out += (value0 * weight0 + value1 * weight1) * final_weight;
            }
        }
    }

    Some(out)
}

pub fn blend_scalar_slots_clamped(
    slots: &[BlendSlot],
    source_values: &[f32],
    initial: f32,
    minimum: f32,
    maximum: f32,
) -> Option<f32> {
    Some(blend_scalar_slots(slots, source_values, initial)?.clamp(minimum, maximum))
}

pub fn multiply_rgb(local: Rgb, parent: Rgb) -> Rgb {
    Rgb::new(local.r * parent.r, local.g * parent.g, local.b * parent.b)
}

pub fn screen_rgb(local: Rgb, parent: Rgb) -> Rgb {
    Rgb::new(
        clamp01(local.r + parent.r - local.r * parent.r),
        clamp01(local.g + parent.g - local.g * parent.g),
        clamp01(local.b + parent.b - local.b * parent.b),
    )
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
