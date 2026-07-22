#[derive(Debug, Copy, Clone, PartialEq)]
pub struct KeyformAxisInterval {
    left_index: usize,
    t: f32,
}

impl KeyformAxisInterval {
    pub fn left_index(&self) -> usize {
        self.left_index
    }

    pub fn t(&self) -> f32 {
        self.t
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct KeyformAxis {
    left_index: usize,
    t: f32,
    stride: usize,
}

impl KeyformAxis {
    pub fn new(left_index: usize, t: f32, stride: usize) -> Self {
        Self {
            left_index,
            t,
            stride,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct KeyformRuntimeSlot {
    flat_index: usize,
    weight: f32,
}

impl KeyformRuntimeSlot {
    pub fn flat_index(&self) -> usize {
        self.flat_index
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }
}

pub fn compute_keyform_axis_interval(keys: &[f32], value: f32) -> Option<KeyformAxisInterval> {
    let first = *keys.first()?;
    if value <= first {
        return Some(KeyformAxisInterval {
            left_index: 0,
            t: 0.0,
        });
    }

    let last_index = keys.len() - 1;
    if value >= keys[last_index] {
        return Some(KeyformAxisInterval {
            left_index: last_index,
            t: 0.0,
        });
    }

    for index in 0..last_index {
        let left = keys[index];
        let right = keys[index + 1];
        if left <= value && value <= right {
            return Some(KeyformAxisInterval {
                left_index: index,
                t: (value - left) / (right - left),
            });
        }
    }

    Some(KeyformAxisInterval {
        left_index: last_index,
        t: 0.0,
    })
}

pub fn expand_keyform_runtime_slots(axes: &[KeyformAxis]) -> Vec<KeyformRuntimeSlot> {
    let active_count = axes.iter().filter(|axis| axis.t != 0.0).count();
    let slot_count = 1usize << active_count;
    let mut slots = Vec::with_capacity(slot_count);

    for mask in 0..slot_count {
        let mut flat_index = 0usize;
        let mut weight = 1.0f32;
        let mut bit = 0usize;

        for axis in axes {
            if axis.t == 0.0 {
                flat_index += axis.left_index * axis.stride;
                continue;
            }

            let use_right = ((mask >> bit) & 1) != 0;
            bit += 1;

            if use_right {
                flat_index += (axis.left_index + 1) * axis.stride;
                weight *= axis.t;
            } else {
                flat_index += axis.left_index * axis.stride;
                weight *= 1.0 - axis.t;
            }
        }

        slots.push(KeyformRuntimeSlot { flat_index, weight });
    }

    slots
}
