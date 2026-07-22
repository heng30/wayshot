const REPEAT_Q_THRESHOLD: f32 = 8_388_608.0;

pub fn clamp_parameter_value(value: f32, minimum: f32, maximum: f32) -> f32 {
    value.max(minimum).min(maximum)
}

pub fn core_repeat_fold(value: f32, minimum: f32, repeat_step: f32) -> f32 {
    let q = (value - minimum) / repeat_step;
    let mut n = q.trunc() as i32;

    if q.abs() < REPEAT_Q_THRESHOLD && n as f32 > q {
        n -= 1;
    }

    (q - n as f32) * repeat_step + minimum
}

pub fn parameter_dirty(old_cached: f32, new_value: f32) -> bool {
    old_cached != new_value
}
