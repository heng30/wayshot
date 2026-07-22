use std::path::{Path, PathBuf};

use crate::{
    ExpressionManager, ModelRuntime, MotionPlayer,
    assets::{DecodedTexture, load_model_runtime},
    core::Matrix44,
    expression::load_expression,
    moc3::{Moc3DrawableBlendMode, Moc3DrawableMesh},
    motion::load_motion,
};

/// Headless Live2D renderer. Pure CPU — no GPU, no window.
/// Renders a model to RGBA image buffers at a specified frame rate.
pub struct Live2dRenderer {
    runtime: ModelRuntime,
    textures: Vec<DecodedTexture>,
    model_dir: Option<PathBuf>,
    motions: Vec<PathBuf>,
    expressions: Vec<PathBuf>,
    motion_player: Option<MotionPlayer>,
    expression_manager: ExpressionManager,
    width: u32,
    height: u32,
    background: [u8; 4],
    model_view_fill: f32,
}

impl Live2dRenderer {
    /// Create a new renderer targeting the given output resolution.
    pub fn new(model_path: impl AsRef<Path>, width: u32, height: u32) -> Result<Self, RenderError> {
        Self::new_with_options(model_path, width, height, Options::default())
    }

    /// Create a renderer with custom options.
    pub fn new_with_options(
        model_path: impl AsRef<Path>,
        width: u32,
        height: u32,
        options: Options,
    ) -> Result<Self, RenderError> {
        let model_path = model_path.as_ref();
        let loaded = load_model_runtime(model_path)?;
        let model_dir = loaded.model_dir().map(Path::to_path_buf);
        let runtime = loaded.runtime().clone();
        let textures = loaded.textures().to_vec();

        let motions = motion_path_bufs(&runtime, loaded.model_dir());
        let expressions = expression_path_bufs(&runtime, loaded.model_dir());

        Ok(Self {
            runtime,
            textures,
            model_dir,
            motions,
            expressions,
            motion_player: None,
            expression_manager: ExpressionManager::new(),
            width,
            height,
            background: options.background,
            model_view_fill: options.model_view_fill,
        })
    }

    /// Set the background color (RGBA, straight alpha).
    pub fn set_background(&mut self, bg: [u8; 4]) {
        self.background = bg;
    }

    /// Set how much of the canvas the model fills.
    pub fn set_model_view_fill(&mut self, fill: f32) {
        self.model_view_fill = fill;
    }

    // --- Animation control ---

    /// Play a motion from a .motion3.json file.
    pub fn play_motion(&mut self, path: impl AsRef<Path>) -> Result<(), RenderError> {
        let motion = load_motion(path)?;
        self.motion_player = Some(MotionPlayer::new(motion));
        Ok(())
    }

    /// Play an expression from a .exp3.json file.
    pub fn play_expression(&mut self, path: impl AsRef<Path>) -> Result<(), RenderError> {
        let expression = load_expression(path)?;
        self.expression_manager.play(expression);
        Ok(())
    }

    /// Stop the current motion.
    pub fn stop_motion(&mut self) {
        self.motion_player = None;
    }

    /// Stop all active expressions.
    pub fn stop_expressions(&mut self) {
        self.expression_manager.stop_all();
    }

    /// Set a parameter value by ID.
    pub fn set_parameter(&mut self, id: &str, value: f32) -> bool {
        self.runtime.set_parameter(id, value)
    }

    /// Access the model runtime.
    pub fn runtime(&self) -> &ModelRuntime {
        &self.runtime
    }

    /// Access the model runtime (mutable).
    pub fn runtime_mut(&mut self) -> &mut ModelRuntime {
        &mut self.runtime
    }

    // --- Rendering ---

    /// Advance animation by `delta_seconds` and render one frame.
    /// Returns straight-alpha RGBA pixel data (`width * height * 4` bytes).
    pub fn render_frame(&mut self, delta_seconds: f32) -> Vec<u8> {
        self.advance_animation(delta_seconds);
        self.render_meshes()
    }

    /// Render the current state without advancing animation.
    pub fn render_static(&mut self) -> Vec<u8> {
        self.runtime.update_meshes();
        self.render_meshes()
    }

    /// Render the frame at the given absolute time and fps.
    ///
    /// **Idempotent**: the same `(fps, current_time)` always produces the
    /// same RGBA output regardless of call history or order.
    ///
    /// The method rebuilds animation state from scratch each call:
    /// 1. Reset runtime parameters and part opacities to defaults
    /// 2. Seek motion to `frame_time` and apply
    /// 3. Seek expressions to `frame_time` and apply
    /// 4. Apply parameter overrides, pose, and mesh update
    /// 5. Rasterize
    ///
    /// `current_time` is snapped to the nearest frame boundary for the given
    /// `fps`, so e.g. at 30fps both `t=0.03` and `t=0.01` produce frame 0.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use live2d_rs::Live2dRenderer;
    ///
    /// let mut renderer = Live2dRenderer::new("model.model3.json", 512, 512).unwrap();
    /// renderer.play_motion("idle.motion3.json").unwrap();
    ///
    /// let fps = 30.0;
    /// // These two calls produce identical output:
    /// let frame_a = renderer.render_at(fps, 1.0);
    /// let frame_b = renderer.render_at(fps, 1.0);
    /// assert_eq!(frame_a, frame_b);
    ///
    /// // Rendering a later frame then going back also works:
    /// let frame_c = renderer.render_at(fps, 0.0);
    /// let frame_d = renderer.render_at(fps, 2.0);
    /// // frame_c and frame_d are different frames, but each is deterministic
    /// ```
    pub fn render_at(&mut self, fps: f32, current_time: f32) -> Vec<u8> {
        let fps = fps.max(1.0);
        let current_time = current_time.max(0.0);
        let frame_duration = 1.0 / fps;

        // Snap to frame boundary for deterministic output.
        let frame_time = (current_time / frame_duration).floor() * frame_duration;

        // Reset runtime from scratch.
        self.runtime.reset_parameters();
        self.runtime.reset_part_opacities();

        // Seek motion to absolute time and apply.
        if let Some(player) = self.motion_player.as_mut() {
            player.seek_to(frame_time);
            player.apply(&mut self.runtime);
        }

        // Seek all active expressions to the same time and apply.
        for player in &mut self.expression_manager.players {
            player.seek_to(frame_time);
        }
        self.expression_manager.apply(&mut self.runtime);

        self.runtime.apply_parameter_overrides();
        self.runtime.apply_pose(frame_duration);
        self.runtime.update_meshes();
        self.render_meshes()
    }

    /// Change the output resolution.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    // --- Accessors ---

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn model_dir(&self) -> Option<&Path> {
        self.model_dir.as_deref()
    }

    pub fn motion_paths(&self) -> &[PathBuf] {
        &self.motions
    }

    pub fn expression_paths(&self) -> &[PathBuf] {
        &self.expressions
    }

    // --- Private ---

    fn advance_animation(&mut self, delta_seconds: f32) {
        self.runtime.reset_parameters();
        self.runtime.reset_part_opacities();

        if let Some(player) = self.motion_player.as_mut() {
            player.tick(delta_seconds);
            player.apply(&mut self.runtime);
            if player.is_finished() {
                self.motion_player = None;
            }
        }

        self.expression_manager.tick(delta_seconds);
        self.expression_manager.apply(&mut self.runtime);

        self.runtime.apply_parameter_overrides();
        self.runtime.apply_pose(delta_seconds);
        self.runtime.update_meshes();
    }

    fn render_meshes(&self) -> Vec<u8> {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return Vec::new();
        }

        let meshes = self.runtime.meshes();

        // Compute model bounds and transform.
        let bounds = ModelBounds::from_drawables(meshes);
        let transform = match bounds {
            Some(b) => fit_model_matrix(b, self.width, self.height, self.model_view_fill),
            None => Matrix44::identity(),
        };

        // Build draw order.
        let draw_order = draw_order_indices(meshes);

        // Prepare framebuffer — premultiplied alpha floating-point.
        let mut fb = vec![0.0f32; w * h * 4];

        // Fill background.
        let bg = [
            self.background[0] as f32 / 255.0,
            self.background[1] as f32 / 255.0,
            self.background[2] as f32 / 255.0,
            self.background[3] as f32 / 255.0,
        ];
        for pixel in fb.chunks_exact_mut(4) {
            pixel[0] = bg[0] * bg[3];
            pixel[1] = bg[1] * bg[3];
            pixel[2] = bg[2] * bg[3];
            pixel[3] = bg[3];
        }

        // Render each mesh in draw order.
        for &mesh_index in &draw_order {
            let mesh = &meshes[mesh_index];
            if mesh.opacity() <= 0.0 || mesh.vertices().is_empty() || mesh.indices().is_empty() {
                continue;
            }

            let tex_info = self
                .textures
                .get(mesh.texture_index() as usize)
                .map(|t| (t.rgba(), t.width(), t.height()));

            let multiply = mesh.multiply_color();
            let screen = mesh.screen_color();
            let opacity = mesh.opacity();
            let blend_mode = mesh.blend_mode();

            // Transform vertices to screen space.
            let transformed: Vec<[f32; 2]> = mesh
                .vertices()
                .iter()
                .map(|v| {
                    let [x, y] = v.position();
                    let sx = transform.transform_x(x);
                    let sy = transform.transform_y(y);
                    // NDC [-1,1] -> pixel coords [0, width/height]
                    let px = (sx + 1.0) * 0.5 * self.width as f32;
                    let py = (1.0 - sy) * 0.5 * self.height as f32; // flip Y
                    [px, py]
                })
                .collect();

            // Rasterize triangles.
            let indices = mesh.indices();
            for tri in indices.chunks_exact(3) {
                let i0 = tri[0] as usize;
                let i1 = tri[1] as usize;
                let i2 = tri[2] as usize;
                if i0 >= transformed.len() || i1 >= transformed.len() || i2 >= transformed.len() {
                    continue;
                }

                let v0 = &transformed[i0];
                let v1 = &transformed[i1];
                let v2 = &transformed[i2];
                let uv0 = mesh.vertices()[i0].uv();
                let uv1 = mesh.vertices()[i1].uv();
                let uv2 = mesh.vertices()[i2].uv();

                rasterize_triangle(
                    &mut fb, w, h, v0, v1, v2, &uv0, &uv1, &uv2, tex_info, multiply, screen,
                    opacity, blend_mode,
                );
            }
        }

        // Convert premultiplied-alpha float framebuffer to straight-alpha u8.
        let mut rgba = vec![0u8; w * h * 4];
        for (dst, src) in rgba.chunks_exact_mut(4).zip(fb.chunks_exact(4)) {
            let a = src[3];
            if a <= 0.0 {
                dst[0] = 0;
                dst[1] = 0;
                dst[2] = 0;
                dst[3] = 0;
            } else {
                let inv_a = 1.0 / a;
                dst[0] = (src[0] * inv_a * 255.0).clamp(0.0, 255.0) as u8;
                dst[1] = (src[1] * inv_a * 255.0).clamp(0.0, 255.0) as u8;
                dst[2] = (src[2] * inv_a * 255.0).clamp(0.0, 255.0) as u8;
                dst[3] = (a * 255.0).clamp(0.0, 255.0) as u8;
            }
        }

        rgba
    }
}

/// Renderer options.
pub struct Options {
    /// Background color (RGBA, straight alpha). Default: transparent black.
    pub background: [u8; 4],
    /// How much of the canvas the model fills. Default: 1.85.
    pub model_view_fill: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            background: [0, 0, 0, 0],
            model_view_fill: 1.85,
        }
    }
}

/// Error type for the renderer.
#[derive(Debug)]
pub enum RenderError {
    ModelLoad(crate::assets::AssetLoadError),
    MotionLoad(crate::motion::MotionLoadError),
    ExpressionLoad(crate::expression::ExpressionLoadError),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelLoad(e) => write!(f, "failed to load model: {e}"),
            Self::MotionLoad(e) => write!(f, "failed to load motion: {e}"),
            Self::ExpressionLoad(e) => write!(f, "failed to load expression: {e}"),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ModelLoad(e) => Some(e),
            Self::MotionLoad(e) => Some(e),
            Self::ExpressionLoad(e) => Some(e),
        }
    }
}

impl From<crate::assets::AssetLoadError> for RenderError {
    fn from(e: crate::assets::AssetLoadError) -> Self {
        Self::ModelLoad(e)
    }
}

impl From<crate::motion::MotionLoadError> for RenderError {
    fn from(e: crate::motion::MotionLoadError) -> Self {
        Self::MotionLoad(e)
    }
}

impl From<crate::expression::ExpressionLoadError> for RenderError {
    fn from(e: crate::expression::ExpressionLoadError) -> Self {
        Self::ExpressionLoad(e)
    }
}

// ---------------------------------------------------------------------------
// CPU triangle rasterizer
// ---------------------------------------------------------------------------

fn rasterize_triangle(
    fb: &mut [f32],
    fb_w: usize,
    fb_h: usize,
    v0: &[f32; 2],
    v1: &[f32; 2],
    v2: &[f32; 2],
    uv0: &[f32; 2],
    uv1: &[f32; 2],
    uv2: &[f32; 2],
    texture: Option<(&[u8], u32, u32)>,
    multiply: [f32; 3],
    screen: [f32; 3],
    opacity: f32,
    blend_mode: Moc3DrawableBlendMode,
) {
    // Bounding box.
    let min_x = v0[0].min(v1[0]).min(v2[0]).floor() as i32;
    let max_x = v0[0].max(v1[0]).max(v2[0]).ceil() as i32;
    let min_y = v0[1].min(v1[1]).min(v2[1]).floor() as i32;
    let max_y = v0[1].max(v1[1]).max(v2[1]).ceil() as i32;

    let min_x = min_x.max(0);
    let min_y = min_y.max(0);
    let max_x = max_x.min(fb_w as i32 - 1);
    let max_y = max_y.min(fb_h as i32 - 1);

    if min_x > max_x || min_y > max_y {
        return;
    }

    // Edge function for barycentric coordinates.
    let area = edge_function(v0, v1, v2);
    if area.abs() < 1e-6 {
        return; // Degenerate triangle.
    }
    let inv_area = 1.0 / area;

    for py in min_y..=max_y {
        for px in min_x..max_x {
            let p = [px as f32 + 0.5, py as f32 + 0.5];

            let w0 = edge_function(&p, v1, v2) * inv_area;
            let w1 = edge_function(&p, v2, v0) * inv_area;
            let w2 = edge_function(&p, v0, v1) * inv_area;

            // Inside test (with slight tolerance for edge pixels).
            if w0 < -0.001 || w1 < -0.001 || w2 < -0.001 {
                continue;
            }

            // Interpolate UV.
            let u = w0 * uv0[0] + w1 * uv1[0] + w2 * uv2[0];
            let v = w0 * uv0[1] + w1 * uv1[1] + w2 * uv2[1];

            // Sample texture.
            let tex_color = sample_texture(texture, u, v);

            // Apply multiply/screen color + opacity (same as the WGSL shader).
            // rgb = sample.rgb * multiply
            // rgb = rgb + screen - rgb * screen  (screen blend)
            // alpha = sample.a * opacity
            // output = vec4(rgb * alpha, alpha)   (premultiplied)
            let mut r = tex_color[0] * multiply[0];
            let mut g = tex_color[1] * multiply[1];
            let mut b = tex_color[2] * multiply[2];
            r = r + screen[0] - r * screen[0];
            g = g + screen[1] - g * screen[1];
            b = b + screen[2] - b * screen[2];
            let a = tex_color[3] * opacity;

            // Premultiplied source color.
            let sr = r * a;
            let sg = g * a;
            let sb = b * a;
            let sa = a;

            let idx = (py as usize * fb_w + px as usize) * 4;
            let dr = fb[idx];
            let dg = fb[idx + 1];
            let db = fb[idx + 2];
            let da = fb[idx + 3];

            // Blend with destination (all premultiplied alpha).
            let (or, og, ob, oa) = match blend_mode {
                Moc3DrawableBlendMode::Normal => {
                    // src*1 + dst*(1-srcAlpha)
                    let out_a = sa + da * (1.0 - sa);
                    if out_a <= 0.0 {
                        (0.0, 0.0, 0.0, 0.0)
                    } else {
                        (
                            sr + dr * (1.0 - sa),
                            sg + dg * (1.0 - sa),
                            sb + db * (1.0 - sa),
                            out_a,
                        )
                    }
                }
                Moc3DrawableBlendMode::Additive => {
                    // src*1 + dst*1 (additive blend)
                    (sr + dr, sg + dg, sb + db, sa + da)
                }
                Moc3DrawableBlendMode::Multiplicative => {
                    // src*dst + dst*(1-srcAlpha)
                    let out_a = da;
                    (
                        sr * dr + dr * (1.0 - sa),
                        sg * dg + dg * (1.0 - sa),
                        sb * db + db * (1.0 - sa),
                        out_a,
                    )
                }
            };

            fb[idx] = or;
            fb[idx + 1] = og;
            fb[idx + 2] = ob;
            fb[idx + 3] = oa;
        }
    }
}

/// Edge function: returns twice the signed area of triangle (a, b, c).
fn edge_function(a: &[f32; 2], b: &[f32; 2], c: &[f32; 2]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

/// Bilinear texture sample. Returns [R, G, B, A] in [0,1].
fn sample_texture(texture: Option<(&[u8], u32, u32)>, u: f32, v: f32) -> [f32; 4] {
    let Some((tex, tw, th)) = texture else {
        // No texture — white.
        return [1.0, 1.0, 1.0, 1.0];
    };

    if tex.len() < 4 || tw == 0 || th == 0 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    // Clamp UV to [0, 1].
    let u = u.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);

    // Bilinear interpolation.
    let x = u * (tw as f32 - 1.0);
    let y = v * (th as f32 - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(tw - 1);
    let y1 = (y0 + 1).min(th - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = tex_pixel(tex, tw, x0, y0);
    let p10 = tex_pixel(tex, tw, x1, y0);
    let p01 = tex_pixel(tex, tw, x0, y1);
    let p11 = tex_pixel(tex, tw, x1, y1);

    let r = p00[0] * (1.0 - fx) * (1.0 - fy)
        + p10[0] * fx * (1.0 - fy)
        + p01[0] * (1.0 - fx) * fy
        + p11[0] * fx * fy;
    let g = p00[1] * (1.0 - fx) * (1.0 - fy)
        + p10[1] * fx * (1.0 - fy)
        + p01[1] * (1.0 - fx) * fy
        + p11[1] * fx * fy;
    let b = p00[2] * (1.0 - fx) * (1.0 - fy)
        + p10[2] * fx * (1.0 - fy)
        + p01[2] * (1.0 - fx) * fy
        + p11[2] * fx * fy;
    let a = p00[3] * (1.0 - fx) * (1.0 - fy)
        + p10[3] * fx * (1.0 - fy)
        + p01[3] * (1.0 - fx) * fy
        + p11[3] * fx * fy;

    [r, g, b, a]
}

fn tex_pixel(tex: &[u8], tw: u32, x: u32, y: u32) -> [f32; 4] {
    let idx = (y * tw + x) as usize * 4;
    if idx + 3 >= tex.len() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    [
        tex[idx] as f32 / 255.0,
        tex[idx + 1] as f32 / 255.0,
        tex[idx + 2] as f32 / 255.0,
        tex[idx + 3] as f32 / 255.0,
    ]
}

// ---------------------------------------------------------------------------
// Draw order
// ---------------------------------------------------------------------------

fn draw_order_indices(meshes: &[Moc3DrawableMesh]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..meshes.len()).collect();
    indices.sort_by(|&a, &b| {
        let ra = meshes[a].render_order();
        let rb = meshes[b].render_order();
        ra.cmp(&rb).then_with(|| a.cmp(&b))
    });
    indices
}

// ---------------------------------------------------------------------------
// Model bounds & transform
// ---------------------------------------------------------------------------

#[derive(Debug, Copy, Clone, PartialEq)]
struct ModelBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl ModelBounds {
    fn from_drawables(drawables: &[Moc3DrawableMesh]) -> Option<Self> {
        let mut bounds: Option<Self> = None;
        for vertex in drawables.iter().flat_map(|d| d.vertices()) {
            let [x, y] = vertex.position();
            bounds = Some(match bounds {
                Some(b) => Self {
                    min_x: b.min_x.min(x),
                    min_y: b.min_y.min(y),
                    max_x: b.max_x.max(x),
                    max_y: b.max_y.max(y),
                },
                None => Self {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                },
            });
        }
        bounds.filter(|b| b.width() > 0.0 && b.height() > 0.0)
    }

    fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(self) -> f32 {
        self.max_y - self.min_y
    }
}

fn fit_model_matrix(bounds: ModelBounds, width: u32, height: u32, view_fill: f32) -> Matrix44 {
    let aspect = width as f32 / height as f32;
    let fit_x = view_fill / (bounds.width() * aspect);
    let fit_y = view_fill / bounds.height();
    let scale_y = fit_x.min(fit_y);
    let scale_x = scale_y / aspect;

    // Center the model's origin (0,0) on the canvas, not the bounding box center.
    // Live2D models are authored with their origin at the visual anchor point
    // (e.g. waist for standing characters), so centering on the bounding box
    // geometric center would push the model off-screen vertically.
    let mut matrix = Matrix44::identity();
    matrix.scale(scale_x, scale_y);
    matrix
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn motion_path_bufs(runtime: &ModelRuntime, model_dir: Option<&Path>) -> Vec<PathBuf> {
    let Some(model_dir) = model_dir else {
        return Vec::new();
    };
    runtime
        .model()
        .motions()
        .values()
        .flatten()
        .map(|reference| model_dir.join(reference.file()))
        .collect()
}

fn expression_path_bufs(runtime: &ModelRuntime, model_dir: Option<&Path>) -> Vec<PathBuf> {
    let Some(model_dir) = model_dir else {
        return Vec::new();
    };
    runtime
        .model()
        .expressions()
        .iter()
        .map(|reference| model_dir.join(reference.file()))
        .collect()
}
