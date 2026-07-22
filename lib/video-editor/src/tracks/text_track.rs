use super::{
    segment::Segment, track::InnerTrack, unified_mixer::UnifiedFrameText,
    video_frame_cache::VideoImage, video_track::LayerFrame,
};
use crate::{
    Result,
    filters::{
        keyframe::{KeyframeTracks, KeyframeValue},
        subtitle::{renderer::create_text_image, style::SubtitleStyle},
    },
    metadata::Metadata,
};
use image::{Rgba, RgbaImage, imageops};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};
use std::{path::PathBuf, sync::Arc, time::Duration};

#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct TextElement {
    pub text: String,
    pub style: SubtitleStyle,

    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub position: (f32, f32),

    #[derivative(Default(value = "1.0"))]
    pub opacity: f32,

    #[derivative(Default(value = "0.0"))]
    pub rotation: f32,

    pub keyframe_tracks: KeyframeTracks,
}

impl TextElement {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn with_style(mut self, style: SubtitleStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = (x, y);
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn get_value_at_time(&self, property_name: &str, time_ms: i64) -> Option<KeyframeValue> {
        self.keyframe_tracks
            .get_track(property_name)
            .and_then(|track| {
                if track.keyframes.is_empty() {
                    return None;
                }

                let idx = track.keyframes.partition_point(|k| k.time_ms <= time_ms);
                if idx == 0 {
                    return Some(track.keyframes[0].value.clone());
                }

                if idx >= track.keyframes.len() {
                    return Some(track.keyframes.last().unwrap().value.clone());
                }

                let prev = &track.keyframes[idx - 1];
                let next = &track.keyframes[idx];

                let t = if next.time_ms == prev.time_ms {
                    0.0
                } else {
                    (time_ms - prev.time_ms) as f32 / (next.time_ms - prev.time_ms) as f32
                };

                match (&prev.value, &next.value) {
                    (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
                        Some(KeyframeValue::Float(a + (b - a) * t))
                    }
                    (KeyframeValue::Float2(ax, ay), KeyframeValue::Float2(bx, by)) => Some(
                        KeyframeValue::Float2(ax + (bx - ax) * t, ay + (by - ay) * t),
                    ),
                    (
                        KeyframeValue::Color(ar, ag, ab, aa),
                        KeyframeValue::Color(br, bg, bb, ba),
                    ) => Some(KeyframeValue::Color(
                        (*ar as f32 + (*br as f32 - *ar as f32) * t) as u8,
                        (*ag as f32 + (*bg as f32 - *ag as f32) * t) as u8,
                        (*ab as f32 + (*bb as f32 - *ab as f32) * t) as u8,
                        (*aa as f32 + (*ba as f32 - *aa as f32) * t) as u8,
                    )),
                    (KeyframeValue::Bool(a), KeyframeValue::Bool(_)) => {
                        Some(KeyframeValue::Bool(*a))
                    }
                    _ => Some(prev.value.clone()),
                }
            })
    }
}

#[derive(Debug, Clone, Default)]
pub struct TextTrack {
    pub name: String,
    pub hiding: bool,
    pub locked: bool,
    pub track: InnerTrack,
}

impl TextTrack {
    pub fn new() -> Self {
        let metadata = Arc::new(Metadata {
            path: PathBuf::from("text://"),
            ..Default::default()
        });
        Self {
            name: "T".to_string(),
            hiding: false,
            locked: false,
            track: InnerTrack::new(metadata, Duration::ZERO, vec![]),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_hiding(&mut self, hiding: bool) {
        self.hiding = hiding;
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn is_hiding(&self) -> bool {
        self.hiding
    }

    pub fn add_segment(
        &mut self,
        element: TextElement,
        offset: Duration,
        duration: Duration,
        global_speed: f32,
    ) {
        let metadata = Arc::new(Metadata {
            path: PathBuf::from(format!("text://{}", uuid::Uuid::new_v4())),
            ..Default::default()
        });
        let segment = Arc::new(
            Segment::new(offset, duration, metadata, global_speed).with_text_element(element),
        );
        self.track.segments.push(segment);
        self.update_duration();
    }

    pub(crate) fn update_duration(&mut self) {
        self.track.duration = self
            .track
            .segments
            .iter()
            .map(|seg| seg.timeline_offset + seg.duration)
            .max()
            .unwrap_or(Duration::ZERO);
    }

    pub fn render_at_time(
        &self,
        time: Duration,
        width: u32,
        height: u32,
    ) -> Result<Option<RgbaImage>> {
        if self.hiding {
            return Ok(None);
        }

        let active_segments: Vec<_> = self
            .track
            .segments
            .iter()
            .filter(|seg| time >= seg.timeline_offset && time < seg.timeline_offset + seg.duration)
            .collect();

        if active_segments.is_empty() {
            return Ok(None);
        }

        let mut img = RgbaImage::new(width, height);

        for segment in active_segments {
            if let Some(element) = &segment.text_element {
                let relative_time = time.saturating_sub(segment.timeline_offset);
                let time_ms = relative_time.as_millis() as i64;
                render_text_element(&mut img, element, time_ms)?;
            }
        }

        Ok(Some(img))
    }
}

fn render_text_element(img: &mut RgbaImage, element: &TextElement, time_ms: i64) -> Result<()> {
    let position = element
        .get_value_at_time("position", time_ms)
        .and_then(|v| v.as_float2())
        .unwrap_or(element.position);

    let opacity = element
        .get_value_at_time("opacity", time_ms)
        .and_then(|v| v.as_float())
        .unwrap_or(element.opacity);

    let rotation = element
        .get_value_at_time("rotation", time_ms)
        .and_then(|v| v.as_float())
        .map(|v| v.to_radians())
        .unwrap_or(element.rotation.to_radians());

    let scaled_style = element
        .style
        .scaled_for_resolution(img.width(), img.height());

    let text_img = create_text_image(&element.text, &scaled_style)?;

    let final_img = if rotation != 0.0 {
        apply_rotation(&text_img, rotation)
    } else {
        text_img
    };

    let px = (position.0 * img.width() as f32).round() as i32 - final_img.width() as i32 / 2;
    let py = (position.1 * img.height() as f32).round() as i32 - final_img.height() as i32 / 2;

    blend_image(img, &final_img, px, py, opacity);

    Ok(())
}

fn blend_image(dest: &mut RgbaImage, src: &RgbaImage, offset_x: i32, offset_y: i32, opacity: f32) {
    for y in 0..src.height() {
        for x in 0..src.width() {
            let dest_x = offset_x + x as i32;
            let dest_y = offset_y + y as i32;

            if dest_x < 0
                || dest_y < 0
                || dest_x >= dest.width() as i32
                || dest_y >= dest.height() as i32
            {
                continue;
            }

            let src_pixel = src.get_pixel(x, y);
            let dest_pixel = dest.get_pixel_mut(dest_x as u32, dest_y as u32);

            // Apply opacity to source alpha
            let src_alpha = (src_pixel[3] as f32 / 255.0) * opacity;
            let dst_alpha = dest_pixel[3] as f32 / 255.0;

            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

            if out_alpha > 0.0 {
                dest_pixel[0] = ((src_pixel[0] as f32 * src_alpha
                    + dest_pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                dest_pixel[1] = ((src_pixel[1] as f32 * src_alpha
                    + dest_pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                dest_pixel[2] = ((src_pixel[2] as f32 * src_alpha
                    + dest_pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                dest_pixel[3] = (out_alpha * 255.0) as u8;
            }
        }
    }
}

fn apply_rotation(src: &RgbaImage, theta: f32) -> RgbaImage {
    let w = src.width() as f32;
    let h = src.height() as f32;

    let cos_theta = theta.cos().abs();
    let sin_theta = theta.sin().abs();

    // Calculate rotated bounding box
    let rotated_w = w * cos_theta + h * sin_theta;
    let rotated_h = w * sin_theta + h * cos_theta;

    // Canvas must fit both original AND rotated dimensions
    // This prevents negative offsets when placing the original image
    // before rotation (e.g., 90° rotation of wide text would clip)
    let expanded_width = w.max(rotated_w).ceil() as u32;
    let expanded_height = h.max(rotated_h).ceil() as u32;

    let mut expanded_canvas =
        RgbaImage::from_pixel(expanded_width, expanded_height, Rgba([0, 0, 0, 0]));

    let offset_x = (expanded_width as i64 - src.width() as i64) / 2;
    let offset_y = (expanded_height as i64 - src.height() as i64) / 2;
    imageops::overlay(&mut expanded_canvas, src, offset_x, offset_y);

    rotate_about_center::<Rgba<u8>>(
        &expanded_canvas,
        theta,
        Interpolation::Bilinear,
        imageproc::geometric_transformations::Border::Constant(Rgba([0, 0, 0, 0])),
    )
}

#[derive(Debug, Clone)]
pub struct TextSource {
    pub track_index: usize,
    pub track: Arc<TextTrack>,
}

impl TextSource {
    pub fn duration(&self) -> Duration {
        self.track.track.duration
    }

    pub fn segments(&self) -> &[Arc<Segment>] {
        &self.track.track.segments
    }
}

#[derive(Debug)]
pub struct UnifiedTextTracksCompositorIterator {
    pub timeline_offset: Duration,
    all_segments: Vec<UnifiedFrameText>,
}

impl UnifiedTextTracksCompositorIterator {
    pub fn new(sources: Vec<TextSource>, timeline_offset: Duration) -> Self {
        let mut all_segments: Vec<UnifiedFrameText> = Vec::new();

        for source in &sources {
            let track_index = source.track_index;

            if source.track.track.segments.is_empty() || source.track.hiding {
                continue;
            }

            for (segment_index, segment) in source.track.track.segments.iter().enumerate() {
                if segment.hiding {
                    continue;
                }

                let segment_end = segment.timeline_offset + segment.duration;
                if segment_end <= timeline_offset {
                    continue;
                }

                if let Some(element) = &segment.text_element {
                    all_segments.push(UnifiedFrameText {
                        element: element.clone(),
                        segment: segment.clone(),
                        segment_index,
                        track_index,
                    });
                }
            }
        }

        all_segments.sort_by(|a, b| {
            b.track_index
                .cmp(&a.track_index)
                .then_with(|| a.segment.timeline_offset.cmp(&b.segment.timeline_offset))
        });

        Self {
            timeline_offset,
            all_segments,
        }
    }

    pub fn get_text_at(&self, timestamp: Duration) -> Vec<UnifiedFrameText> {
        self.all_segments
            .iter()
            .filter(|frame_text| {
                let segment_start = frame_text.segment.timeline_offset;
                let segment_end = segment_start + frame_text.segment.duration;
                timestamp >= segment_start && timestamp < segment_end
            })
            .cloned()
            .collect()
    }
}

pub fn create_text_layer_frame(
    element: &TextElement,
    segment: Arc<Segment>,
    segment_index: usize,
    track_index: usize,
    time: Duration,
    output_width: u32,
    output_height: u32,
) -> Result<LayerFrame> {
    let relative_time = time.saturating_sub(segment.timeline_offset);
    let time_ms = relative_time.as_millis() as i64;

    let position = element
        .get_value_at_time("position", time_ms)
        .and_then(|v| v.as_float2())
        .unwrap_or(element.position);

    let opacity = element
        .get_value_at_time("opacity", time_ms)
        .and_then(|v| v.as_float())
        .unwrap_or(element.opacity);

    let rotation = element
        .get_value_at_time("rotation", time_ms)
        .and_then(|v| v.as_float())
        .map(|v| v.to_radians())
        .unwrap_or(element.rotation.to_radians());

    let scaled_style = element
        .style
        .scaled_for_resolution(output_width, output_height);

    let text_img = create_text_image(&element.text, &scaled_style)?;

    // original_image: 未经位置和旋转变换的原始文字图片
    let origin_video_image = VideoImage::image(text_img.clone());

    let final_img = if rotation != 0.0 {
        apply_rotation(&text_img, rotation)
    } else {
        text_img
    };

    let mut img = RgbaImage::new(output_width, output_height);

    // Calculate position (center the text at the given position)
    let px = (position.0 * output_width as f32).round() as i32 - final_img.width() as i32 / 2;
    let py = (position.1 * output_height as f32).round() as i32 - final_img.height() as i32 / 2;

    blend_image(&mut img, &final_img, px, py, opacity);

    let video_image = VideoImage::image(img);
    Ok(LayerFrame::new(
        origin_video_image,
        video_image,
        Some((segment_index, segment)),
        track_index,
    ))
}
