#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct InterpolationGroup {
    index: usize,
    offset: usize,
    count: usize,
    out_index: usize,
}

impl InterpolationGroup {
    pub fn new(index: usize, offset: usize, count: usize, out_index: usize) -> Self {
        Self {
            index,
            offset,
            count,
            out_index,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ArrayInterpolationGroup {
    index: usize,
    slot_begin: usize,
    slot_count: usize,
    out_index: usize,
    float_count: usize,
}

impl ArrayInterpolationGroup {
    pub fn new(
        index: usize,
        slot_begin: usize,
        slot_count: usize,
        out_index: usize,
        float_count: usize,
    ) -> Self {
        Self {
            index,
            slot_begin,
            slot_count,
            out_index,
            float_count,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct InterpolationOutput {
    out_index: usize,
    value: f32,
}

impl InterpolationOutput {
    pub fn out_index(&self) -> usize {
        self.out_index
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

pub fn interpolate_float32(values: &[f32], weights: &[f32]) -> Option<f32> {
    if values.len() != weights.len() {
        return None;
    }

    Some(
        values
            .iter()
            .zip(weights)
            .map(|(value, weight)| value * weight)
            .sum(),
    )
}

pub fn interpolate_int32(values: &[f32], weights: &[f32]) -> Option<i32> {
    let value = interpolate_float32(values, weights)?;
    Some((value + 0.001).trunc() as i32)
}

pub fn interpolate_float32_array(
    arrays: &[&[f32]],
    weights: &[f32],
    count: usize,
) -> Option<Vec<f32>> {
    if arrays.len() != weights.len() || arrays.iter().any(|array| array.len() < count) {
        return None;
    }

    let mut out = vec![0.0; count];
    for (array, weight) in arrays.iter().zip(weights) {
        if *weight == 0.0 {
            continue;
        }

        for index in 0..count {
            out[index] += array[index] * weight;
        }
    }

    Some(out)
}

pub fn interpolate_float32_array_grouped(
    arrays: &[&[f32]],
    weights: &[f32],
    groups: &[ArrayInterpolationGroup],
    output_arrays: &mut [Vec<f32>],
    skip_mask: Option<&[bool]>,
) -> Option<()> {
    if arrays.len() != weights.len() {
        return None;
    }

    for group in groups {
        if let Some(mask) = skip_mask
            && !*mask.get(group.index)?
        {
            continue;
        }

        let slot_end = group.slot_begin.checked_add(group.slot_count)?;
        let group_arrays = arrays.get(group.slot_begin..slot_end)?;
        let group_weights = weights.get(group.slot_begin..slot_end)?;
        if group_arrays
            .iter()
            .any(|array| array.len() < group.float_count)
        {
            return None;
        }

        let out = output_arrays.get_mut(group.out_index)?;
        if out.len() < group.float_count {
            return None;
        }

        for value in out.iter_mut().take(group.float_count) {
            *value = 0.0;
        }

        for (array, weight) in group_arrays.iter().zip(group_weights) {
            if *weight == 0.0 {
                continue;
            }

            for index in 0..group.float_count {
                out[index] += array[index] * weight;
            }
        }
    }

    Some(())
}

pub fn interpolate_float32_grouped(
    values: &[f32],
    weights: &[f32],
    groups: &[InterpolationGroup],
    skip_mask: Option<&[bool]>,
) -> Option<Vec<InterpolationOutput>> {
    if values.len() != weights.len() {
        return None;
    }

    let weighted = values
        .iter()
        .zip(weights)
        .map(|(value, weight)| value * weight)
        .collect::<Vec<_>>();
    let mut out = Vec::new();

    for group in groups {
        if let Some(mask) = skip_mask
            && !*mask.get(group.index)?
        {
            continue;
        }

        let end = group.offset.checked_add(group.count)?;
        let values = weighted.get(group.offset..end)?;
        out.push(InterpolationOutput {
            out_index: group.out_index,
            value: values.iter().sum(),
        });
    }

    Some(out)
}
