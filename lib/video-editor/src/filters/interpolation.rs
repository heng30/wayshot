use crate::filters::keyframe::{KeyframeValue, PropertyTrack};

pub fn interpolate_linear(t: f32, a: f32, b: f32) -> f32 {
    a + (b - a) * t
}

pub fn interpolate_values(
    t: f32,
    from: &KeyframeValue,
    to: &KeyframeValue,
) -> Option<KeyframeValue> {
    match (from, to) {
        (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
            let value = interpolate_linear(t, *a, *b);
            Some(KeyframeValue::Float(value))
        }
        (KeyframeValue::Float2(ax, ay), KeyframeValue::Float2(bx, by)) => {
            let vx = interpolate_linear(t, *ax, *bx);
            let vy = interpolate_linear(t, *ay, *by);
            Some(KeyframeValue::Float2(vx, vy))
        }
        (KeyframeValue::Color(ar, ag, ab, aa), KeyframeValue::Color(br, bg, bb, ba)) => {
            let r = interpolate_linear(t, *ar as f32, *br as f32) as u8;
            let g = interpolate_linear(t, *ag as f32, *bg as f32) as u8;
            let b = interpolate_linear(t, *ab as f32, *bb as f32) as u8;
            let a = interpolate_linear(t, *aa as f32, *ba as f32) as u8;
            Some(KeyframeValue::Color(r, g, b, a))
        }
        (KeyframeValue::Bool(_), KeyframeValue::Bool(_)) => Some(from.clone()),
        _ => None,
    }
}

pub fn evaluate_track_at_time(track: &PropertyTrack, time_ms: i64) -> Option<KeyframeValue> {
    if track.is_empty() {
        return None;
    }

    let keyframes = &track.keyframes;

    if time_ms <= keyframes[0].time_ms {
        return Some(keyframes[0].value.clone());
    }

    if time_ms >= keyframes[keyframes.len() - 1].time_ms {
        return Some(keyframes[keyframes.len() - 1].value.clone());
    }

    // Find the keyframes surrounding the current time
    for i in 0..keyframes.len() - 1 {
        let from_kf = &keyframes[i];
        let to_kf = &keyframes[i + 1];

        if time_ms >= from_kf.time_ms && time_ms < to_kf.time_ms {
            // Calculate interpolation factor (0.0 to 1.0)
            let duration_ms = to_kf.time_ms - from_kf.time_ms;
            if duration_ms == 0 {
                return Some(to_kf.value.clone());
            }

            let t = (time_ms - from_kf.time_ms) as f32 / duration_ms as f32;
            let t = t.clamp(0.0, 1.0);

            return interpolate_values(t, &from_kf.value, &to_kf.value);
        }
    }

    None
}

pub fn get_float_at_time(track: &PropertyTrack, time_ms: i64, default: f32) -> f32 {
    evaluate_track_at_time(track, time_ms)
        .and_then(|v| v.as_float())
        .unwrap_or(default)
}

pub fn get_float2_at_time(
    track: &PropertyTrack,
    time_ms: i64,
    default_x: f32,
    default_y: f32,
) -> (f32, f32) {
    evaluate_track_at_time(track, time_ms)
        .and_then(|v| v.as_float2())
        .unwrap_or((default_x, default_y))
}

pub fn get_bool_at_time(track: &PropertyTrack, time_ms: i64, default: bool) -> bool {
    evaluate_track_at_time(track, time_ms)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

pub fn get_color_at_time(
    track: &PropertyTrack,
    time_ms: i64,
    default: (u8, u8, u8, u8),
) -> (u8, u8, u8, u8) {
    evaluate_track_at_time(track, time_ms)
        .and_then(|v| v.as_color())
        .unwrap_or(default)
}
