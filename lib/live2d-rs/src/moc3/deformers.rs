use crate::{Result, core::Vector2};

use super::{
    Moc3CountInfo, Moc3Header, Moc3SectionOffsets,
    compose::{
        ComposedDeformer, ComposedDeformers, ComposedRotation, ComposedWarp, apply_composed_parent,
        parent_colors, parent_opacity_accum, parent_rotation_angle, parent_scale_accum,
    },
    keyform_bindings::{Moc3KeyformBindings, Moc3KeyformSlot},
    parse::{
        invalid_moc3, read_bool_section, read_f32_section, read_f32_section_or_default,
        read_i32_section, read_i32_section_or_default, to_usize,
    },
};

const DEFORMER_PARENT_DEFORMER_INDICES_SLOT: usize = 16;
const DEFORMER_TYPES_SLOT: usize = 17;
const DEFORMER_SPECIFIC_INDICES_SLOT: usize = 18;
const WARP_KEYFORM_BINDING_BAND_INDICES_SLOT: usize = 19;
const WARP_KEYFORM_BEGIN_INDICES_SLOT: usize = 20;
const WARP_KEYFORM_COUNTS_SLOT: usize = 21;
const WARP_VERTEX_COUNTS_SLOT: usize = 22;
const WARP_ROWS_SLOT: usize = 23;
const WARP_COLS_SLOT: usize = 24;
const ROTATION_KEYFORM_BINDING_BAND_INDICES_SLOT: usize = 25;
const ROTATION_KEYFORM_BEGIN_INDICES_SLOT: usize = 26;
const ROTATION_KEYFORM_COUNTS_SLOT: usize = 27;
const ROTATION_BASE_ANGLES_SLOT: usize = 28;
const WARP_KEYFORM_OPACITIES_SLOT: usize = 59;
const WARP_KEYFORM_POSITION_BEGIN_INDICES_SLOT: usize = 60;
const ROTATION_KEYFORM_OPACITIES_SLOT: usize = 61;
const ROTATION_KEYFORM_ANGLES_SLOT: usize = 62;
const ROTATION_KEYFORM_ORIGIN_XS_SLOT: usize = 63;
const ROTATION_KEYFORM_ORIGIN_YS_SLOT: usize = 64;
const ROTATION_KEYFORM_SCALES_SLOT: usize = 65;
const ROTATION_KEYFORM_REFLECT_XS_SLOT: usize = 66;
const ROTATION_KEYFORM_REFLECT_YS_SLOT: usize = 67;
const KEYFORM_POSITION_XYS_SLOT: usize = 71;
const WARP_KEYFORM_COLOR_BEGIN_INDICES_SLOT: usize = 105;
const ROTATION_KEYFORM_COLOR_BEGIN_INDICES_SLOT: usize = 106;
const KEYFORM_MULTIPLY_COLOR_SLOTS: [usize; 3] = [108, 109, 110];
const KEYFORM_SCREEN_COLOR_SLOTS: [usize; 3] = [111, 112, 113];

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Moc3DeformerKind {
    Warp,
    Rotation,
}

impl Moc3DeformerKind {
    fn from_raw(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Warp),
            1 => Some(Self::Rotation),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct InterpolatedRotation {
    angle_degrees: f32,
    translation: Vector2,
    scale: f32,
    flip_x: bool,
    flip_y: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Moc3Deformers {
    parent_deformer_indices: Vec<i32>,
    deformer_kinds: Vec<Moc3DeformerKind>,
    specific_indices: Vec<i32>,
    warp_keyform_binding_band_indices: Vec<i32>,
    warp_keyform_begin_indices: Vec<i32>,
    warp_keyform_counts: Vec<i32>,
    warp_vertex_counts: Vec<i32>,
    warp_rows: Vec<i32>,
    warp_cols: Vec<i32>,
    warp_keyform_opacities: Vec<f32>,
    rotation_keyform_binding_band_indices: Vec<i32>,
    rotation_keyform_begin_indices: Vec<i32>,
    rotation_keyform_counts: Vec<i32>,
    rotation_base_angles: Vec<f32>,
    warp_keyform_position_begin_indices: Vec<i32>,
    rotation_keyform_angles: Vec<f32>,
    rotation_keyform_origin_xs: Vec<f32>,
    rotation_keyform_origin_ys: Vec<f32>,
    rotation_keyform_scales: Vec<f32>,
    rotation_keyform_reflect_xs: Vec<bool>,
    rotation_keyform_reflect_ys: Vec<bool>,
    rotation_keyform_opacities: Vec<f32>,
    keyform_position_xys: Vec<f32>,
    warp_keyform_color_begin_indices: Vec<i32>,
    rotation_keyform_color_begin_indices: Vec<i32>,
    keyform_multiply_colors: [Vec<f32>; 3],
    keyform_screen_colors: [Vec<f32>; 3],
}

impl Moc3Deformers {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;
        let endianness = header.endianness();
        let deformer_count = to_usize(counts.deformers(), "deformer count")?;
        let warp_count = to_usize(counts.warp_deformers(), "warp deformer count")?;
        let rotation_count = to_usize(counts.rotation_deformers(), "rotation deformer count")?;
        let warp_keyform_count = to_usize(
            counts.warp_deformer_keyforms(),
            "warp deformer keyform count",
        )?;
        let rotation_keyform_count = to_usize(
            counts.rotation_deformer_keyforms(),
            "rotation deformer keyform count",
        )?;
        let keyform_multiply_color_count = to_usize(
            counts.keyform_multiply_colors(),
            "keyform multiply color count",
        )?;
        let keyform_screen_color_count =
            to_usize(counts.keyform_screen_colors(), "keyform screen color count")?;

        let deformer_types = read_i32_section(
            bytes,
            &offsets,
            DEFORMER_TYPES_SLOT,
            deformer_count,
            endianness,
        )?;
        let deformer_kinds = deformer_types
            .iter()
            .copied()
            .map(|value| {
                Moc3DeformerKind::from_raw(value)
                    .ok_or_else(|| invalid_moc3(format!("unsupported deformer type {value}")))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            parent_deformer_indices: read_i32_section(
                bytes,
                &offsets,
                DEFORMER_PARENT_DEFORMER_INDICES_SLOT,
                deformer_count,
                endianness,
            )?,
            deformer_kinds,
            specific_indices: read_i32_section(
                bytes,
                &offsets,
                DEFORMER_SPECIFIC_INDICES_SLOT,
                deformer_count,
                endianness,
            )?,
            warp_keyform_binding_band_indices: read_i32_section(
                bytes,
                &offsets,
                WARP_KEYFORM_BINDING_BAND_INDICES_SLOT,
                warp_count,
                endianness,
            )?,
            warp_keyform_begin_indices: read_i32_section(
                bytes,
                &offsets,
                WARP_KEYFORM_BEGIN_INDICES_SLOT,
                warp_count,
                endianness,
            )?,
            warp_keyform_counts: read_i32_section(
                bytes,
                &offsets,
                WARP_KEYFORM_COUNTS_SLOT,
                warp_count,
                endianness,
            )?,
            warp_vertex_counts: read_i32_section(
                bytes,
                &offsets,
                WARP_VERTEX_COUNTS_SLOT,
                warp_count,
                endianness,
            )?,
            warp_rows: read_i32_section(bytes, &offsets, WARP_ROWS_SLOT, warp_count, endianness)?,
            warp_cols: read_i32_section(bytes, &offsets, WARP_COLS_SLOT, warp_count, endianness)?,
            warp_keyform_opacities: read_f32_section_or_default(
                bytes,
                &offsets,
                WARP_KEYFORM_OPACITIES_SLOT,
                warp_keyform_count,
                endianness,
                1.0,
            )?,
            rotation_keyform_binding_band_indices: read_i32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_BINDING_BAND_INDICES_SLOT,
                rotation_count,
                endianness,
            )?,
            rotation_keyform_begin_indices: read_i32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_BEGIN_INDICES_SLOT,
                rotation_count,
                endianness,
            )?,
            rotation_keyform_counts: read_i32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_COUNTS_SLOT,
                rotation_count,
                endianness,
            )?,
            rotation_base_angles: read_f32_section(
                bytes,
                &offsets,
                ROTATION_BASE_ANGLES_SLOT,
                rotation_count,
                endianness,
            )?,
            warp_keyform_position_begin_indices: read_i32_section(
                bytes,
                &offsets,
                WARP_KEYFORM_POSITION_BEGIN_INDICES_SLOT,
                warp_keyform_count,
                endianness,
            )?,
            rotation_keyform_angles: read_f32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_ANGLES_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_origin_xs: read_f32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_ORIGIN_XS_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_origin_ys: read_f32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_ORIGIN_YS_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_scales: read_f32_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_SCALES_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_reflect_xs: read_bool_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_REFLECT_XS_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_reflect_ys: read_bool_section(
                bytes,
                &offsets,
                ROTATION_KEYFORM_REFLECT_YS_SLOT,
                rotation_keyform_count,
                endianness,
            )?,
            rotation_keyform_opacities: read_f32_section_or_default(
                bytes,
                &offsets,
                ROTATION_KEYFORM_OPACITIES_SLOT,
                rotation_keyform_count,
                endianness,
                1.0,
            )?,
            keyform_position_xys: read_f32_section(
                bytes,
                &offsets,
                KEYFORM_POSITION_XYS_SLOT,
                to_usize(counts.keyform_positions(), "keyform position count")?,
                endianness,
            )?,
            warp_keyform_color_begin_indices: read_i32_section_or_default(
                bytes,
                &offsets,
                WARP_KEYFORM_COLOR_BEGIN_INDICES_SLOT,
                warp_count,
                endianness,
                -1,
            )?,
            rotation_keyform_color_begin_indices: read_i32_section_or_default(
                bytes,
                &offsets,
                ROTATION_KEYFORM_COLOR_BEGIN_INDICES_SLOT,
                rotation_count,
                endianness,
                -1,
            )?,
            keyform_multiply_colors: read_color_channels(
                bytes,
                &offsets,
                KEYFORM_MULTIPLY_COLOR_SLOTS,
                keyform_multiply_color_count,
                endianness,
                1.0,
            )?,
            keyform_screen_colors: read_color_channels(
                bytes,
                &offsets,
                KEYFORM_SCREEN_COLOR_SLOTS,
                keyform_screen_color_count,
                endianness,
                0.0,
            )?,
        })
    }

    pub(super) fn compose(
        &self,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<ComposedDeformers> {
        let count = self.deformer_kinds.len();
        let mut order: Vec<usize> = (0..count).collect();
        order.sort_by_key(|&idx| self.deformer_depth(idx));

        let mut composed: Vec<Option<ComposedDeformer>> = vec![None; count];
        for idx in order {
            let parent = *self.parent_deformer_indices.get(idx)?;
            let specific = usize::try_from(*self.specific_indices.get(idx)?).ok()?;
            let composed_deformer = match *self.deformer_kinds.get(idx)? {
                Moc3DeformerKind::Warp => {
                    let mut grid =
                        self.interpolated_warp_grid(specific, bindings, parameter_values)?;
                    let cols = usize::try_from(*self.warp_cols.get(specific)?).ok()?;
                    let rows = usize::try_from(*self.warp_rows.get(specific)?).ok()?;
                    for point in &mut grid {
                        *point = apply_composed_parent(&composed, parent, *point)?;
                    }
                    let scale_accum = parent_scale_accum(&composed, parent);
                    let opacity =
                        self.interpolated_warp_opacity(specific, bindings, parameter_values)?;
                    let opacity_accum = opacity * parent_opacity_accum(&composed, parent);
                    let (multiply_color, screen_color) = compose_colors(
                        self.interpolated_warp_colors(specific, bindings, parameter_values)?,
                        parent_colors(&composed, parent),
                    );
                    ComposedDeformer::Warp(ComposedWarp {
                        grid,
                        cols,
                        rows,
                        scale_accum,
                        opacity_accum,
                        multiply_color,
                        screen_color,
                    })
                }
                Moc3DeformerKind::Rotation => {
                    let rotation =
                        self.interpolated_rotation(specific, bindings, parameter_values)?;
                    let origin = apply_composed_parent(&composed, parent, rotation.translation)?;
                    let parent_angle =
                        parent_rotation_angle(&composed, parent, origin, rotation.translation)?;
                    let scale_accum = parent_scale_accum(&composed, parent);
                    let opacity =
                        self.interpolated_rotation_opacity(specific, bindings, parameter_values)?;
                    let opacity_accum = opacity * parent_opacity_accum(&composed, parent);
                    let (multiply_color, screen_color) = compose_colors(
                        self.interpolated_rotation_colors(specific, bindings, parameter_values)?,
                        parent_colors(&composed, parent),
                    );
                    ComposedDeformer::Rotation(ComposedRotation {
                        origin,
                        angle_degrees: rotation.angle_degrees + parent_angle.to_degrees(),
                        scale: rotation.scale * scale_accum,
                        flip_x: rotation.flip_x,
                        flip_y: rotation.flip_y,
                        scale_accum: rotation.scale * scale_accum,
                        opacity_accum,
                        multiply_color,
                        screen_color,
                    })
                }
            };
            *composed.get_mut(idx)? = Some(composed_deformer);
        }

        Some(ComposedDeformers::new(
            composed.into_iter().collect::<Option<Vec<_>>>()?,
        ))
    }

    fn deformer_depth(&self, index: usize) -> usize {
        let mut depth = 0usize;
        let mut current = index;
        loop {
            let parent = self
                .parent_deformer_indices
                .get(current)
                .copied()
                .unwrap_or(-1);
            if parent < 0 {
                break;
            }
            current = match usize::try_from(parent) {
                Ok(value) => value,
                Err(_) => break,
            };
            depth += 1;
            if depth > self.parent_deformer_indices.len() {
                break;
            }
        }
        depth
    }

    fn warp_keyform_slots(
        &self,
        warp_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<Vec<Moc3KeyformSlot>> {
        let keyform_count = usize::try_from(*self.warp_keyform_counts.get(warp_index)?).ok()?;
        bindings.keyform_slots(
            *self.warp_keyform_binding_band_indices.get(warp_index)?,
            keyform_count,
            parameter_values,
        )
    }

    fn rotation_keyform_slots(
        &self,
        rotation_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<Vec<Moc3KeyformSlot>> {
        let keyform_count =
            usize::try_from(*self.rotation_keyform_counts.get(rotation_index)?).ok()?;
        bindings.keyform_slots(
            *self
                .rotation_keyform_binding_band_indices
                .get(rotation_index)?,
            keyform_count,
            parameter_values,
        )
    }

    fn interpolated_warp_grid(
        &self,
        warp_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<Vec<Vector2>> {
        let slots = self.warp_keyform_slots(warp_index, bindings, parameter_values)?;
        let begin = usize::try_from(*self.warp_keyform_begin_indices.get(warp_index)?).ok()?;
        let vertex_count = usize::try_from(*self.warp_vertex_counts.get(warp_index)?).ok()?;
        let mut grid = vec![Vector2::default(); vertex_count];

        for slot in slots {
            let keyform_index = begin.checked_add(slot.local_index)?;
            let source = self.warp_grid(warp_index, keyform_index)?;
            if source.len() != grid.len() {
                return None;
            }
            for (target, source) in grid.iter_mut().zip(source) {
                *target = Vector2::new(
                    target.x() + source.x() * slot.weight,
                    target.y() + source.y() * slot.weight,
                );
            }
        }

        Some(grid)
    }

    fn interpolated_rotation(
        &self,
        rotation_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<InterpolatedRotation> {
        let slots = self.rotation_keyform_slots(rotation_index, bindings, parameter_values)?;
        let begin =
            usize::try_from(*self.rotation_keyform_begin_indices.get(rotation_index)?).ok()?;
        let mut angle = 0.0f32;
        let mut translation = Vector2::default();
        let mut scale = 0.0f32;
        let mut flip_x = 0.0f32;
        let mut flip_y = 0.0f32;

        for slot in slots {
            let keyform_index = begin.checked_add(slot.local_index)?;
            angle += *self.rotation_keyform_angles.get(keyform_index)? * slot.weight;
            translation = Vector2::new(
                translation.x()
                    + *self.rotation_keyform_origin_xs.get(keyform_index)? * slot.weight,
                translation.y()
                    + *self.rotation_keyform_origin_ys.get(keyform_index)? * slot.weight,
            );
            scale += *self.rotation_keyform_scales.get(keyform_index)? * slot.weight;
            flip_x += u8::from(*self.rotation_keyform_reflect_xs.get(keyform_index)?) as f32
                * slot.weight;
            flip_y += u8::from(*self.rotation_keyform_reflect_ys.get(keyform_index)?) as f32
                * slot.weight;
        }

        Some(InterpolatedRotation {
            angle_degrees: *self.rotation_base_angles.get(rotation_index)? + angle,
            translation,
            scale,
            flip_x: interpolate_bool(flip_x),
            flip_y: interpolate_bool(flip_y),
        })
    }

    fn interpolated_warp_opacity(
        &self,
        warp_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<f32> {
        let slots = self.warp_keyform_slots(warp_index, bindings, parameter_values)?;
        let begin = usize::try_from(*self.warp_keyform_begin_indices.get(warp_index)?).ok()?;
        let mut opacity = 0.0f32;
        for slot in slots {
            let keyform_index = begin.checked_add(slot.local_index)?;
            opacity += *self.warp_keyform_opacities.get(keyform_index)? * slot.weight;
        }
        Some(opacity)
    }

    fn interpolated_warp_colors(
        &self,
        warp_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<([f32; 3], [f32; 3])> {
        let slots = self.warp_keyform_slots(warp_index, bindings, parameter_values)?;
        let begin = *self.warp_keyform_color_begin_indices.get(warp_index)?;
        interpolate_colors(
            begin,
            &slots,
            &self.keyform_multiply_colors,
            &self.keyform_screen_colors,
        )
    }

    fn interpolated_rotation_opacity(
        &self,
        rotation_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<f32> {
        let slots = self.rotation_keyform_slots(rotation_index, bindings, parameter_values)?;
        let begin =
            usize::try_from(*self.rotation_keyform_begin_indices.get(rotation_index)?).ok()?;
        let mut opacity = 0.0f32;
        for slot in slots {
            let keyform_index = begin.checked_add(slot.local_index)?;
            opacity += *self.rotation_keyform_opacities.get(keyform_index)? * slot.weight;
        }
        Some(opacity)
    }

    fn interpolated_rotation_colors(
        &self,
        rotation_index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<([f32; 3], [f32; 3])> {
        let slots = self.rotation_keyform_slots(rotation_index, bindings, parameter_values)?;
        let begin = *self
            .rotation_keyform_color_begin_indices
            .get(rotation_index)?;
        interpolate_colors(
            begin,
            &slots,
            &self.keyform_multiply_colors,
            &self.keyform_screen_colors,
        )
    }

    fn warp_grid(&self, warp_index: usize, keyform_index: usize) -> Option<Vec<Vector2>> {
        let start = usize::try_from(
            *self
                .warp_keyform_position_begin_indices
                .get(keyform_index)?,
        )
        .ok()?;
        let vertex_count = usize::try_from(*self.warp_vertex_counts.get(warp_index)?).ok()?;
        let len = vertex_count.checked_mul(2)?;
        let values = self
            .keyform_position_xys
            .get(start..start.checked_add(len)?)?;

        Some(
            values
                .chunks_exact(2)
                .map(|xy| Vector2::new(xy[0], xy[1]))
                .collect(),
        )
    }
}

fn interpolate_bool(value: f32) -> bool {
    (value + 0.001).trunc() != 0.0
}

fn read_color_channels(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slots: [usize; 3],
    count: usize,
    endianness: super::Endianness,
    default: f32,
) -> Result<[Vec<f32>; 3]> {
    Ok([
        read_f32_section_or_default(bytes, offsets, slots[0], count, endianness, default)?,
        read_f32_section_or_default(bytes, offsets, slots[1], count, endianness, default)?,
        read_f32_section_or_default(bytes, offsets, slots[2], count, endianness, default)?,
    ])
}

fn interpolate_colors(
    begin: i32,
    slots: &[Moc3KeyformSlot],
    multiply_colors: &[Vec<f32>; 3],
    screen_colors: &[Vec<f32>; 3],
) -> Option<([f32; 3], [f32; 3])> {
    if begin < 0 {
        return Some(([1.0, 1.0, 1.0], [0.0, 0.0, 0.0]));
    }
    let begin = usize::try_from(begin).ok()?;
    let mut multiply = [0.0; 3];
    let mut screen = [0.0; 3];
    for slot in slots {
        let color_index = begin.checked_add(slot.local_index)?;
        for channel in 0..3 {
            multiply[channel] += multiply_colors[channel].get(color_index).copied()? * slot.weight;
            screen[channel] += screen_colors[channel].get(color_index).copied()? * slot.weight;
        }
    }
    Some((multiply, screen))
}

fn compose_colors(
    local: ([f32; 3], [f32; 3]),
    parent: ([f32; 3], [f32; 3]),
) -> ([f32; 3], [f32; 3]) {
    let mut multiply = [0.0; 3];
    let mut screen = [0.0; 3];
    for channel in 0..3 {
        multiply[channel] = local.0[channel] * parent.0[channel];
        screen[channel] =
            local.1[channel] + parent.1[channel] - local.1[channel] * parent.1[channel];
    }
    (multiply, screen)
}
