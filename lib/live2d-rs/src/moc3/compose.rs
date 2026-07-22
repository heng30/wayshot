use crate::core::{
    Vector2, WarpInterpolation, rotation_deformer_transform_point, warp_deformer_transform_target,
};

const ROTATION_PROBE_ITERATIONS: usize = 10;
const ROTATION_PROBE_STEP_WARP_PARENT: f32 = -0.1;
const ROTATION_PROBE_STEP_ROTATION_PARENT: f32 = -10.0;
const DEFAULT_MULTIPLY_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
const DEFAULT_SCREEN_COLOR: [f32; 3] = [0.0, 0.0, 0.0];

pub(super) fn parent_rotation_angle(
    composed: &[Option<ComposedDeformer>],
    parent_index: i32,
    origin_world: Vector2,
    translation: Vector2,
) -> Option<f32> {
    let step = match parent_deformer(composed, parent_index) {
        Some(ComposedDeformer::Rotation(_)) => ROTATION_PROBE_STEP_ROTATION_PARENT,
        _ => ROTATION_PROBE_STEP_WARP_PARENT,
    };

    let mut scale = 1.0f32;
    let mut direction = Vector2::default();
    for _ in 0..ROTATION_PROBE_ITERATIONS {
        let offset = step * scale;

        let forward = apply_composed_parent(
            composed,
            parent_index,
            Vector2::new(translation.x(), translation.y() + offset),
        )?;
        let dx = forward.x() - origin_world.x();
        let dy = forward.y() - origin_world.y();
        if dx != 0.0 || dy != 0.0 {
            direction = Vector2::new(dx, dy);
            break;
        }

        let backward = apply_composed_parent(
            composed,
            parent_index,
            Vector2::new(translation.x(), translation.y() - offset),
        )?;
        let dx = backward.x() - origin_world.x();
        let dy = backward.y() - origin_world.y();
        if dx != 0.0 || dy != 0.0 {
            direction = Vector2::new(-dx, -dy);
            break;
        }

        scale *= 0.1;
    }

    Some(wrap_angle(
        direction.y().atan2(direction.x()) - step.atan2(0.0),
    ))
}

fn parent_deformer(
    composed: &[Option<ComposedDeformer>],
    parent_index: i32,
) -> Option<&ComposedDeformer> {
    let index = usize::try_from(parent_index).ok()?;
    composed.get(index)?.as_ref()
}

fn wrap_angle(mut angle: f32) -> f32 {
    use std::f32::consts::PI;
    const TWO_PI: f32 = 2.0 * PI;
    while angle < -PI {
        angle += TWO_PI;
    }
    while angle > PI {
        angle -= TWO_PI;
    }
    angle
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ComposedDeformer {
    Warp(ComposedWarp),
    Rotation(ComposedRotation),
}

impl ComposedDeformer {
    fn scale_accum(&self) -> f32 {
        match self {
            Self::Warp(warp) => warp.scale_accum,
            Self::Rotation(rotation) => rotation.scale_accum,
        }
    }

    fn opacity_accum(&self) -> f32 {
        match self {
            Self::Warp(warp) => warp.opacity_accum,
            Self::Rotation(rotation) => rotation.opacity_accum,
        }
    }

    fn colors(&self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::Warp(warp) => (warp.multiply_color, warp.screen_color),
            Self::Rotation(rotation) => (rotation.multiply_color, rotation.screen_color),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ComposedWarp {
    pub(super) grid: Vec<Vector2>,
    pub(super) cols: usize,
    pub(super) rows: usize,
    pub(super) scale_accum: f32,
    pub(super) opacity_accum: f32,
    pub(super) multiply_color: [f32; 3],
    pub(super) screen_color: [f32; 3],
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) struct ComposedRotation {
    pub(super) origin: Vector2,
    pub(super) angle_degrees: f32,
    pub(super) scale: f32,
    pub(super) flip_x: bool,
    pub(super) flip_y: bool,
    pub(super) scale_accum: f32,
    pub(super) opacity_accum: f32,
    pub(super) multiply_color: [f32; 3],
    pub(super) screen_color: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComposedDeformers {
    deformers: Vec<ComposedDeformer>,
}

impl ComposedDeformers {
    pub(super) fn new(deformers: Vec<ComposedDeformer>) -> Self {
        Self { deformers }
    }

    pub(super) fn transform_vertices(
        &self,
        parent_deformer_index: i32,
        vertices: &mut [Vector2],
    ) -> Option<()> {
        if parent_deformer_index < 0 {
            return Some(());
        }
        let index = usize::try_from(parent_deformer_index).ok()?;
        for vertex in vertices {
            *vertex = apply_one(self.deformers.get(index)?, *vertex)?;
        }
        Some(())
    }

    pub(super) fn deformer_opacity(&self, parent_deformer_index: i32) -> f32 {
        if parent_deformer_index < 0 {
            return 1.0;
        }
        let index = match usize::try_from(parent_deformer_index) {
            Ok(value) => value,
            Err(_) => return 1.0,
        };
        self.deformers
            .get(index)
            .map(ComposedDeformer::opacity_accum)
            .unwrap_or(1.0)
    }

    pub(super) fn deformer_colors(&self, parent_deformer_index: i32) -> ([f32; 3], [f32; 3]) {
        if parent_deformer_index < 0 {
            return (DEFAULT_MULTIPLY_COLOR, DEFAULT_SCREEN_COLOR);
        }
        let index = match usize::try_from(parent_deformer_index) {
            Ok(value) => value,
            Err(_) => return (DEFAULT_MULTIPLY_COLOR, DEFAULT_SCREEN_COLOR),
        };
        self.deformers
            .get(index)
            .map(ComposedDeformer::colors)
            .unwrap_or((DEFAULT_MULTIPLY_COLOR, DEFAULT_SCREEN_COLOR))
    }
}

pub(super) fn apply_one(deformer: &ComposedDeformer, point: Vector2) -> Option<Vector2> {
    match deformer {
        ComposedDeformer::Warp(warp) => warp_deformer_transform_target(
            point,
            &warp.grid,
            warp.cols,
            warp.rows,
            WarpInterpolation::Quad,
        ),
        ComposedDeformer::Rotation(rotation) => Some(rotation_deformer_transform_point(
            point,
            rotation.angle_degrees,
            rotation.scale,
            rotation.origin,
            rotation.flip_x,
            rotation.flip_y,
        )),
    }
}

pub(super) fn apply_composed_parent(
    composed: &[Option<ComposedDeformer>],
    parent_index: i32,
    point: Vector2,
) -> Option<Vector2> {
    if parent_index < 0 {
        return Some(point);
    }
    let index = usize::try_from(parent_index).ok()?;
    let parent = composed.get(index)?.as_ref()?;
    apply_one(parent, point)
}

pub(super) fn parent_scale_accum(composed: &[Option<ComposedDeformer>], parent_index: i32) -> f32 {
    parent_deformer(composed, parent_index)
        .map(ComposedDeformer::scale_accum)
        .unwrap_or(1.0)
}

pub(super) fn parent_opacity_accum(
    composed: &[Option<ComposedDeformer>],
    parent_index: i32,
) -> f32 {
    parent_deformer(composed, parent_index)
        .map(ComposedDeformer::opacity_accum)
        .unwrap_or(1.0)
}

pub(super) fn parent_colors(
    composed: &[Option<ComposedDeformer>],
    parent_index: i32,
) -> ([f32; 3], [f32; 3]) {
    parent_deformer(composed, parent_index)
        .map(ComposedDeformer::colors)
        .unwrap_or((DEFAULT_MULTIPLY_COLOR, DEFAULT_SCREEN_COLOR))
}
