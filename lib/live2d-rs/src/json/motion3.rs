use serde::Deserialize;

use crate::{Error, Result};

const FORMAT: &str = "motion3.json";
const SUPPORTED_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Motion3 {
    version: u32,
    meta: MotionMeta,
    curves: Vec<MotionCurve>,
}

impl Motion3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawMotion3 = serde_json::from_str(source).map_err(|error| Error::InvalidJson {
            format: FORMAT,
            message: error.to_string(),
        })?;

        if raw.version != SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion {
                format: FORMAT,
                version: raw.version,
            });
        }

        let are_beziers_restricted = raw.meta.are_beziers_restricted;
        let curves = raw
            .curves
            .into_iter()
            .map(|curve| MotionCurve::from_raw(curve, are_beziers_restricted))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            version: raw.version,
            meta: raw.meta,
            curves,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn meta(&self) -> &MotionMeta {
        &self.meta
    }

    pub fn curves(&self) -> &[MotionCurve] {
        &self.curves
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MotionMeta {
    #[serde(rename = "Duration")]
    duration: f32,
    #[serde(rename = "Fps")]
    fps: f32,
    #[serde(rename = "Loop")]
    loop_motion: bool,
    #[serde(rename = "AreBeziersRestricted", default)]
    are_beziers_restricted: bool,
    #[serde(rename = "CurveCount", default)]
    curve_count: u32,
    #[serde(rename = "TotalSegmentCount", default)]
    total_segment_count: u32,
    #[serde(rename = "TotalPointCount", default)]
    total_point_count: u32,
    #[serde(rename = "UserDataCount", default)]
    user_data_count: u32,
    #[serde(rename = "TotalUserDataSize", default)]
    total_user_data_size: u32,
}

impl MotionMeta {
    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn is_looping(&self) -> bool {
        self.loop_motion
    }

    pub fn are_beziers_restricted(&self) -> bool {
        self.are_beziers_restricted
    }

    pub fn curve_count(&self) -> u32 {
        self.curve_count
    }

    pub fn total_segment_count(&self) -> u32 {
        self.total_segment_count
    }

    pub fn total_point_count(&self) -> u32 {
        self.total_point_count
    }

    pub fn user_data_count(&self) -> u32 {
        self.user_data_count
    }

    pub fn total_user_data_size(&self) -> u32 {
        self.total_user_data_size
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionCurve {
    target: String,
    id: String,
    first_point: MotionPoint,
    segments: Vec<MotionSegment>,
    fade_in_time: Option<f32>,
    fade_out_time: Option<f32>,
    are_beziers_restricted: bool,
}

impl MotionCurve {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn first_point(&self) -> MotionPoint {
        self.first_point
    }

    pub fn segments(&self) -> &[MotionSegment] {
        &self.segments
    }

    pub fn fade_in_time(&self) -> Option<f32> {
        self.fade_in_time
    }

    pub fn fade_out_time(&self) -> Option<f32> {
        self.fade_out_time
    }

    pub fn sample(&self, time: f32) -> Option<f32> {
        if time <= self.first_point.time {
            return Some(self.first_point.value);
        }

        for segment in &self.segments {
            if time < segment.end().time {
                return segment.sample(time, self.are_beziers_restricted);
            }
        }

        self.segments
            .last()
            .map(|segment| segment.end().value)
            .or(Some(self.first_point.value))
    }
}

impl MotionCurve {
    fn from_raw(raw: RawMotionCurve, are_beziers_restricted: bool) -> Result<Self> {
        let (first_point, segments) = parse_segments(&raw.segments)?;

        Ok(Self {
            target: raw.target,
            id: raw.id,
            first_point,
            segments,
            fade_in_time: raw.fade_in_time,
            fade_out_time: raw.fade_out_time,
            are_beziers_restricted,
        })
    }
}

pub fn easing_sine(value: f32) -> f32 {
    if value < 0.0 {
        return 0.0;
    }

    if value > 1.0 {
        return 1.0;
    }

    0.5 - 0.5 * (value * std::f32::consts::PI).cos()
}

pub fn motion_fade_in_weight(
    user_time_seconds: f32,
    fade_in_start_time: f32,
    fade_in_seconds: f32,
) -> f32 {
    if fade_in_seconds <= 0.0 {
        1.0
    } else {
        easing_sine((user_time_seconds - fade_in_start_time) / fade_in_seconds)
    }
}

pub fn motion_fade_out_weight(
    user_time_seconds: f32,
    end_time_seconds: f32,
    fade_out_seconds: f32,
) -> f32 {
    if fade_out_seconds <= 0.0 || end_time_seconds < 0.0 {
        1.0
    } else {
        easing_sine((end_time_seconds - user_time_seconds) / fade_out_seconds)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn parameter_curve_fade_weight(
    motion_weight: f32,
    motion_fade_in: f32,
    motion_fade_out: f32,
    curve_fade_in_seconds: Option<f32>,
    curve_fade_out_seconds: Option<f32>,
    user_time_seconds: f32,
    fade_in_start_time: f32,
    end_time_seconds: f32,
) -> f32 {
    // A negative per-curve fade time is the Cubism sentinel for "this curve has
    // no override; fall back to the motion-level fade", same as an absent value.
    let curve_fade_in_seconds = curve_fade_in_seconds.filter(|seconds| *seconds >= 0.0);
    let curve_fade_out_seconds = curve_fade_out_seconds.filter(|seconds| *seconds >= 0.0);

    if curve_fade_in_seconds.is_none() && curve_fade_out_seconds.is_none() {
        return motion_weight;
    }

    let fade_in = match curve_fade_in_seconds {
        Some(0.0) => 1.0,
        Some(seconds) => easing_sine((user_time_seconds - fade_in_start_time) / seconds),
        None => motion_fade_in,
    };
    let fade_out = match curve_fade_out_seconds {
        Some(0.0) => 1.0,
        Some(_) if end_time_seconds < 0.0 => 1.0,
        Some(seconds) => easing_sine((end_time_seconds - user_time_seconds) / seconds),
        None => motion_fade_out,
    };

    motion_weight * fade_in * fade_out
}

pub fn apply_motion_fade(source_value: f32, target_value: f32, fade_weight: f32) -> f32 {
    source_value + (target_value - source_value) * fade_weight
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MotionPoint {
    pub time: f32,
    pub value: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MotionSegment {
    Linear {
        start: MotionPoint,
        end: MotionPoint,
    },
    Bezier {
        start: MotionPoint,
        control1: MotionPoint,
        control2: MotionPoint,
        end: MotionPoint,
    },
    Stepped {
        start: MotionPoint,
        end: MotionPoint,
    },
    InverseStepped {
        start: MotionPoint,
        end: MotionPoint,
    },
}

impl MotionSegment {
    pub fn end(&self) -> MotionPoint {
        match *self {
            Self::Linear { end, .. }
            | Self::Bezier { end, .. }
            | Self::Stepped { end, .. }
            | Self::InverseStepped { end, .. } => end,
        }
    }

    pub fn sample(&self, time: f32, are_beziers_restricted: bool) -> Option<f32> {
        match *self {
            Self::Linear { start, end } => Some(sample_linear(start, end, time)),
            Self::Stepped { start, .. } => Some(start.value),
            Self::InverseStepped { end, .. } => Some(end.value),
            Self::Bezier {
                start,
                control1,
                control2,
                end,
            } => Some(sample_bezier(
                start,
                control1,
                control2,
                end,
                time,
                are_beziers_restricted,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawMotion3 {
    #[serde(rename = "Version")]
    version: u32,
    #[serde(rename = "Meta")]
    meta: MotionMeta,
    #[serde(rename = "Curves", default)]
    curves: Vec<RawMotionCurve>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawMotionCurve {
    #[serde(rename = "Target")]
    target: String,
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Segments")]
    segments: Vec<f32>,
    #[serde(rename = "FadeInTime", default)]
    fade_in_time: Option<f32>,
    #[serde(rename = "FadeOutTime", default)]
    fade_out_time: Option<f32>,
}

fn parse_segments(values: &[f32]) -> Result<(MotionPoint, Vec<MotionSegment>)> {
    if values.len() < 2 {
        return Err(invalid_segments(
            "segments must start with a time/value point",
        ));
    }

    let first_point = MotionPoint {
        time: values[0],
        value: values[1],
    };
    let mut cursor = 2;
    let mut start = first_point;
    let mut segments = Vec::new();

    while cursor < values.len() {
        let segment_type = segment_type(values[cursor])?;
        cursor += 1;

        let segment = match segment_type {
            0 => {
                let end = read_point(values, &mut cursor)?;
                MotionSegment::Linear { start, end }
            }
            1 => {
                let control1 = read_point(values, &mut cursor)?;
                let control2 = read_point(values, &mut cursor)?;
                let end = read_point(values, &mut cursor)?;
                MotionSegment::Bezier {
                    start,
                    control1,
                    control2,
                    end,
                }
            }
            2 => {
                let end = read_point(values, &mut cursor)?;
                MotionSegment::Stepped { start, end }
            }
            3 => {
                let end = read_point(values, &mut cursor)?;
                MotionSegment::InverseStepped { start, end }
            }
            _ => return Err(invalid_segments("unsupported segment type")),
        };

        start = segment.end();
        segments.push(segment);
    }

    Ok((first_point, segments))
}

fn read_point(values: &[f32], cursor: &mut usize) -> Result<MotionPoint> {
    if values.len().saturating_sub(*cursor) < 2 {
        return Err(invalid_segments("segment point is incomplete"));
    }

    let point = MotionPoint {
        time: values[*cursor],
        value: values[*cursor + 1],
    };
    *cursor += 2;
    Ok(point)
}

fn segment_type(value: f32) -> Result<u32> {
    if value.fract() != 0.0 || !(0.0..=3.0).contains(&value) {
        return Err(invalid_segments("segment type must be 0, 1, 2, or 3"));
    }

    Ok(value as u32)
}

fn sample_linear(start: MotionPoint, end: MotionPoint, time: f32) -> f32 {
    if start.time == end.time {
        return end.value;
    }

    let amount = ((time - start.time) / (end.time - start.time)).max(0.0);
    start.value + (end.value - start.value) * amount
}

fn sample_bezier(
    start: MotionPoint,
    control1: MotionPoint,
    control2: MotionPoint,
    end: MotionPoint,
    time: f32,
    are_beziers_restricted: bool,
) -> f32 {
    let t = if are_beziers_restricted {
        if start.time == end.time {
            1.0
        } else {
            ((time - start.time) / (end.time - start.time)).max(0.0)
        }
    } else {
        solve_bezier_time(start, control1, control2, end, time)
    };

    cubic_bezier_point(start, control1, control2, end, t).value
}

fn cubic_bezier_point(
    start: MotionPoint,
    control1: MotionPoint,
    control2: MotionPoint,
    end: MotionPoint,
    t: f32,
) -> MotionPoint {
    let p01 = lerp_point(start, control1, t);
    let p12 = lerp_point(control1, control2, t);
    let p23 = lerp_point(control2, end, t);
    let p012 = lerp_point(p01, p12, t);
    let p123 = lerp_point(p12, p23, t);
    lerp_point(p012, p123, t)
}

fn lerp_point(a: MotionPoint, b: MotionPoint, t: f32) -> MotionPoint {
    MotionPoint {
        time: a.time + (b.time - a.time) * t,
        value: a.value + (b.value - a.value) * t,
    }
}

fn solve_bezier_time(
    start: MotionPoint,
    control1: MotionPoint,
    control2: MotionPoint,
    end: MotionPoint,
    time: f32,
) -> f32 {
    let a = end.time - 3.0 * control2.time + 3.0 * control1.time - start.time;
    let b = 3.0 * control2.time - 6.0 * control1.time + 3.0 * start.time;
    let c = 3.0 * control1.time - 3.0 * start.time;
    let d = start.time - time;
    cardano_algorithm_for_bezier(a, b, c, d)
}

fn cardano_algorithm_for_bezier(a: f32, b: f32, c: f32, d: f32) -> f32 {
    const EPSILON: f32 = 0.00001;
    const CENTER: f32 = 0.5;
    const THRESHOLD: f32 = CENTER + 0.01;

    if a.abs() < EPSILON {
        return quadratic_equation(b, c, d).clamp(0.0, 1.0);
    }

    let ba = b / a;
    let ca = c / a;
    let da = d / a;
    let p = (3.0 * ca - ba * ba) / 3.0;
    let p3 = p / 3.0;
    let q = (2.0 * ba * ba * ba - 9.0 * ba * ca + 27.0 * da) / 27.0;
    let q2 = q / 2.0;
    let discriminant = q2 * q2 + p3 * p3 * p3;

    if discriminant < 0.0 {
        let mp3 = -p / 3.0;
        let mp33 = mp3 * mp3 * mp3;
        let r = mp33.sqrt();
        let t = -q / (2.0 * r);
        let cos_phi = t.clamp(-1.0, 1.0);
        let phi = cos_phi.acos();
        let crtr = r.cbrt();
        let t1 = 2.0 * crtr;

        let root1 = t1 * (phi / 3.0).cos() - ba / 3.0;
        if (root1 - CENTER).abs() < THRESHOLD {
            return root1.clamp(0.0, 1.0);
        }

        let root2 = t1 * ((phi + 2.0 * std::f32::consts::PI) / 3.0).cos() - ba / 3.0;
        if (root2 - CENTER).abs() < THRESHOLD {
            return root2.clamp(0.0, 1.0);
        }

        let root3 = t1 * ((phi + 4.0 * std::f32::consts::PI) / 3.0).cos() - ba / 3.0;
        return root3.clamp(0.0, 1.0);
    }

    if discriminant == 0.0 {
        let u1 = if q2 < 0.0 { (-q2).cbrt() } else { -q2.cbrt() };
        let root1 = 2.0 * u1 - ba / 3.0;
        if (root1 - CENTER).abs() < THRESHOLD {
            return root1.clamp(0.0, 1.0);
        }

        let root2 = -u1 - ba / 3.0;
        return root2.clamp(0.0, 1.0);
    }

    let sd = discriminant.sqrt();
    let u1 = (sd - q2).cbrt();
    let v1 = (sd + q2).cbrt();
    (u1 - v1 - ba / 3.0).clamp(0.0, 1.0)
}

fn quadratic_equation(a: f32, b: f32, c: f32) -> f32 {
    const EPSILON: f32 = 0.00001;

    if a.abs() < EPSILON {
        if b.abs() < EPSILON {
            return -c;
        }
        return -c / b;
    }

    -(b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a)
}

fn invalid_segments(message: impl Into<String>) -> Error {
    Error::InvalidJson {
        format: FORMAT,
        message: message.into(),
    }
}
