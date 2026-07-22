use crate::{
    Result,
    filters::{
        interpolation::{get_color_at_time, get_float_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
        video::border::blend_pixel,
    },
    tracks::video_frame_cache::VideoImage,
};
use image::Rgba;
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GridFilter {
    pub rows: u32,
    pub columns: u32,
    pub line_color: [u8; 4],
    pub line_size: u32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for GridFilter {
    fn default() -> Self {
        Self {
            rows: 3,
            columns: 3,
            line_color: [255, 255, 255, 255],
            line_size: 2,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl GridFilter {
    pub const NAME: &'static str = "grid";

    pub fn new(rows: u32, columns: u32, line_color: [u8; 4], line_size: u32) -> Self {
        Self {
            rows,
            columns,
            line_color,
            line_size,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("line_size", "Line Size", 0.0, 1000.0, 0.0),
            AnimatableProperty::color("line_color", "Line Color", 255, 255, 255, 255),
        ]
    }
}

impl VideoFilter for GridFilter {
    crate::impl_default_video_filter!(GridFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_height = data
            .frames
            .first()
            .and_then(|f| {
                if let VideoImage::Image { buffer, .. } = f {
                    Some(buffer.height())
                } else {
                    None
                }
            })
            .unwrap_or(1080);

        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let line_size = self
            .keyframe_tracks
            .get_track("line_size")
            .map(|track| get_float_at_time(track, time_ms, self.line_size as f32) as u32)
            .unwrap_or(self.line_size);

        let line_color = self
            .keyframe_tracks
            .get_track("line_color")
            .map(|track| {
                get_color_at_time(
                    track,
                    time_ms,
                    (
                        self.line_color[0],
                        self.line_color[1],
                        self.line_color[2],
                        self.line_color[3],
                    ),
                )
            })
            .map(|c| [c.0, c.1, c.2, c.3])
            .unwrap_or(self.line_color);

        let rows = self.rows.max(1);
        let columns = self.columns.max(1);
        let scaled_line_size = scale_pixel_for_height(line_size, frame_height);
        let border_color = Rgba(line_color);
        let half = (scaled_line_size as i32 + 1) / 2;

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                let width = buffer.width();
                let height = buffer.height();

                buffer
                    .par_enumerate_pixels_mut()
                    .for_each(|(px, py, pixel)| {
                        let mut on_line = false;

                        for i in 1..rows {
                            let line_y = (height * i / rows) as i32;
                            if (py as i32 - line_y).abs() <= half {
                                on_line = true;
                                break;
                            }
                        }

                        if !on_line {
                            for i in 1..columns {
                                let line_x = (width * i / columns) as i32;
                                if (px as i32 - line_x).abs() <= half {
                                    on_line = true;
                                    break;
                                }
                            }
                        }

                        if on_line {
                            blend_pixel(pixel, border_color, line_color[3] as f32 / 255.0);
                        }
                    });
            }
        }
        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        let mut updated = false;

        if let Some(track) = tracks.get_track("line_size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "line_size",
                time_ms,
                KeyframeValue::Float(self.line_size as f32),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("line_color")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "line_color",
                time_ms,
                KeyframeValue::Color(
                    self.line_color[0],
                    self.line_color[1],
                    self.line_color[2],
                    self.line_color[3],
                ),
            );
            updated = true;
        }

        updated
    }
}
