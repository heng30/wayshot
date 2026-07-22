use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use live2d_rs::{Live2dRenderer, Options};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

struct CachedRenderer {
    renderer: Option<Live2dRenderer>,
    motion_index: i32,
    expression_index: i32,
}

type CacheKey = (PathBuf, u32, u32);

static RENDERER_CACHE: LazyLock<Mutex<HashMap<CacheKey, CachedRenderer>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn setup_motion_and_expression(
    renderer: &mut Live2dRenderer,
    motion_index: i32,
    expression_index: i32,
) {
    renderer.stop_motion();
    renderer.stop_expressions();

    if motion_index >= 0 {
        let motions: Vec<PathBuf> = renderer.motion_paths().to_vec();
        if (motion_index as usize) < motions.len() {
            let _ = renderer.play_motion(&motions[motion_index as usize]);
        }
    }
    if expression_index >= 0 {
        let expressions: Vec<PathBuf> = renderer.expression_paths().to_vec();
        if (expression_index as usize) < expressions.len() {
            let _ = renderer.play_expression(&expressions[expression_index as usize]);
        }
    }
}

fn get_or_create_renderer(
    model_path: &Path,
    width: u32,
    height: u32,
    motion_index: i32,
    expression_index: i32,
    model_view_fill: f32,
    background: [u8; 4],
) -> Option<Live2dRenderer> {
    let key = (model_path.to_path_buf(), width, height);

    let mut cache = RENDERER_CACHE.lock().unwrap();
    if let Some(cached) = cache.get_mut(&key) {
        if let Some(renderer) = cached.renderer.as_mut() {
            renderer.set_background(background);
            renderer.set_model_view_fill(model_view_fill);
            if cached.motion_index != motion_index || cached.expression_index != expression_index {
                setup_motion_and_expression(renderer, motion_index, expression_index);
                cached.motion_index = motion_index;
                cached.expression_index = expression_index;
            }
            return cached.renderer.take();
        }
    }

    let options = Options {
        background,
        model_view_fill,
    };
    let mut renderer = Live2dRenderer::new_with_options(model_path, width, height, options).ok()?;
    setup_motion_and_expression(&mut renderer, motion_index, expression_index);
    Some(renderer)
}

fn return_renderer(
    model_path: &Path,
    width: u32,
    height: u32,
    renderer: Live2dRenderer,
    motion_index: i32,
    expression_index: i32,
) {
    let key = (model_path.to_path_buf(), width, height);
    let mut cache = RENDERER_CACHE.lock().unwrap();
    cache.insert(
        key,
        CachedRenderer {
            renderer: Some(renderer),
            motion_index,
            expression_index,
        },
    );
}

pub fn resolve_model_dir(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if !p.exists() {
        return None;
    }
    // If it's already a .model3.json file, return directly
    if p.is_file() && path.ends_with(".model3.json") {
        return Some(p.to_path_buf());
    }
    // If it's a directory, find the .model3.json file inside it
    if p.is_dir() {
        let Ok(entries) = std::fs::read_dir(p) else {
            return None;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".model3.json") {
                return Some(entry.path());
            }
        }
    }
    None
}

pub fn model_motion_names(model_dir: &str) -> Vec<String> {
    let resolved = resolve_model_dir(model_dir);
    let path = match resolved {
        Some(p) => p,
        None => return vec![],
    };

    let renderer = match Live2dRenderer::new(&path, 1, 1) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    renderer
        .motion_paths()
        .iter()
        .map(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .collect()
}

pub fn model_expression_names(model_dir: &str) -> Vec<String> {
    let resolved = resolve_model_dir(model_dir);
    let path = match resolved {
        Some(p) => p,
        None => return vec![],
    };

    let renderer = match Live2dRenderer::new(&path, 1, 1) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    renderer
        .expression_paths()
        .iter()
        .map(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct Live2dFilter {
    pub model_dir: String,
    #[derivative(Default(value = "-1"))]
    pub motion_index: i32,
    #[derivative(Default(value = "-1"))]
    pub expression_index: i32,
    #[derivative(Default(value = "1.85"))]
    pub model_view_fill: f32,
    #[derivative(Default(value = "[0, 0, 0, 0]"))]
    pub background: [u8; 4],
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Live2dFilter {
    pub const NAME: &'static str = "live 2d";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![AnimatableProperty::float(
            "model_view_fill",
            "Model View Fill",
            0.5,
            5.0,
            1.85,
        )]
    }

    fn get_model_view_fill_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("model_view_fill")
            .map(|track| get_float_at_time(track, time_ms, self.model_view_fill))
            .unwrap_or(self.model_view_fill)
    }

    fn apply_to_buffer(&self, buffer: &mut image::RgbaImage, time_ms: i64) {
        if self.model_dir.is_empty() {
            return;
        }

        let resolved_path = resolve_model_dir(&self.model_dir);
        let model_path = match resolved_path {
            Some(p) => p,
            None => {
                log::warn!("Live2D model not found: {}", self.model_dir);
                return;
            }
        };

        let width = buffer.width();
        let height = buffer.height();
        let model_view_fill = self.get_model_view_fill_at_time(time_ms);

        let mut renderer = match get_or_create_renderer(
            &model_path,
            width,
            height,
            self.motion_index,
            self.expression_index,
            model_view_fill,
            self.background,
        ) {
            Some(r) => r,
            None => {
                log::warn!("Failed to create Live2D renderer for: {}", self.model_dir);
                return;
            }
        };

        let rgba = if self.motion_index >= 0 || self.expression_index >= 0 {
            let fps = 30.0;
            let current_time = time_ms as f32 / 1000.0;
            renderer.render_at(fps, current_time)
        } else {
            renderer.render_static()
        };

        return_renderer(
            &model_path,
            width,
            height,
            renderer,
            self.motion_index,
            self.expression_index,
        );

        if rgba.len() != (width as usize * height as usize * 4) {
            log::warn!("Live2D render output size mismatch");
            return;
        }

        let model_img = match image::RgbaImage::from_raw(width, height, rgba) {
            Some(img) => img,
            None => return,
        };

        for (dst, src) in buffer.pixels_mut().zip(model_img.pixels()) {
            let sa = src.0[3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let da = dst.0[3] as f32 / 255.0;
            let inv_sa = 1.0 - sa;
            let oa = sa + da * inv_sa;
            if oa > 0.0 {
                dst.0[0] = ((src.0[0] as f32 * sa + dst.0[0] as f32 * da * inv_sa) / oa) as u8;
                dst.0[1] = ((src.0[1] as f32 * sa + dst.0[1] as f32 * da * inv_sa) / oa) as u8;
                dst.0[2] = ((src.0[2] as f32 * sa + dst.0[2] as f32 * da * inv_sa) / oa) as u8;
                dst.0[3] = (oa * 255.0) as u8;
            }
        }
    }
}

impl VideoFilter for Live2dFilter {
    crate::impl_default_video_filter!(Live2dFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_to_buffer(buffer, time_ms);
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
        if let Some(track) = tracks.get_track("model_view_fill")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "model_view_fill",
                time_ms,
                KeyframeValue::Float(self.model_view_fill),
            );
            return true;
        }
        false
    }
}
