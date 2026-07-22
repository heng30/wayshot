use crate::{
    Result,
    core::{Vector2, affect_art_mesh_pair},
};

use super::{
    Moc3CountInfo, Moc3DrawableMesh, Moc3DrawableVertex, Moc3Header, Moc3KeyformBindings,
    Moc3SectionOffsets,
    parse::{
        invalid_moc3, read_f32_section, read_i32_section, read_u16_section, to_usize,
        validate_count_range,
    },
};

const GLUE_BINDING_INDICES_SLOT: usize = 91;
const GLUE_KEYFORM_BEGIN_INDICES_SLOT: usize = 92;
const GLUE_KEYFORM_COUNTS_SLOT: usize = 93;
const GLUE_ART_MESH_INDICES_A_SLOT: usize = 94;
const GLUE_ART_MESH_INDICES_B_SLOT: usize = 95;
const GLUE_INFO_BEGIN_INDICES_SLOT: usize = 96;
const GLUE_INFO_COUNTS_SLOT: usize = 97;
const GLUE_INFO_WEIGHTS_SLOT: usize = 98;
const GLUE_INFO_POSITION_INDICES_SLOT: usize = 99;
const GLUE_KEYFORM_INTENSITIES_SLOT: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct Moc3Glues {
    binding_indices: Vec<i32>,
    keyform_begin_indices: Vec<i32>,
    keyform_counts: Vec<i32>,
    art_mesh_indices_a: Vec<i32>,
    art_mesh_indices_b: Vec<i32>,
    info_begin_indices: Vec<i32>,
    info_counts: Vec<i32>,
    info_weights: Vec<f32>,
    info_position_indices: Vec<u16>,
    keyform_intensities: Vec<f32>,
}

impl Moc3Glues {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        binding_indices: Vec<i32>,
        keyform_begin_indices: Vec<i32>,
        keyform_counts: Vec<i32>,
        art_mesh_indices_a: Vec<i32>,
        art_mesh_indices_b: Vec<i32>,
        info_begin_indices: Vec<i32>,
        info_counts: Vec<i32>,
        info_weights: Vec<f32>,
        info_position_indices: Vec<u16>,
        keyform_intensities: Vec<f32>,
    ) -> Result<Self> {
        let glue_count = binding_indices.len();
        if keyform_begin_indices.len() != glue_count
            || keyform_counts.len() != glue_count
            || art_mesh_indices_a.len() != glue_count
            || art_mesh_indices_b.len() != glue_count
            || info_begin_indices.len() != glue_count
            || info_counts.len() != glue_count
            || info_weights.len() != info_position_indices.len()
        {
            return Err(invalid_moc3("glue metadata lengths do not match"));
        }

        for index in 0..glue_count {
            validate_count_range(
                info_begin_indices[index],
                info_counts[index],
                info_weights.len(),
                index,
                "glue",
                "info",
            )?;
            validate_count_range(
                keyform_begin_indices[index],
                keyform_counts[index],
                keyform_intensities.len(),
                index,
                "glue",
                "keyform",
            )?;
        }

        Ok(Self {
            binding_indices,
            keyform_begin_indices,
            keyform_counts,
            art_mesh_indices_a,
            art_mesh_indices_b,
            info_begin_indices,
            info_counts,
            info_weights,
            info_position_indices,
            keyform_intensities,
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;
        let endianness = header.endianness();
        let glue_count = to_usize(counts.glue(), "glue count")?;
        let glue_info_count = to_usize(counts.glue_info(), "glue info count")?;
        let glue_keyform_count = to_usize(counts.glue_keyforms(), "glue keyform count")?;

        Self::from_parts(
            read_i32_section(
                bytes,
                &offsets,
                GLUE_BINDING_INDICES_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_KEYFORM_BEGIN_INDICES_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_KEYFORM_COUNTS_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_ART_MESH_INDICES_A_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_ART_MESH_INDICES_B_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_INFO_BEGIN_INDICES_SLOT,
                glue_count,
                endianness,
            )?,
            read_i32_section(
                bytes,
                &offsets,
                GLUE_INFO_COUNTS_SLOT,
                glue_count,
                endianness,
            )?,
            read_f32_section(
                bytes,
                &offsets,
                GLUE_INFO_WEIGHTS_SLOT,
                glue_info_count,
                endianness,
            )?,
            read_u16_section(
                bytes,
                &offsets,
                GLUE_INFO_POSITION_INDICES_SLOT,
                glue_info_count,
                endianness,
            )?,
            read_f32_section(
                bytes,
                &offsets,
                GLUE_KEYFORM_INTENSITIES_SLOT,
                glue_keyform_count,
                endianness,
            )?,
        )
    }

    pub fn len(&self) -> usize {
        self.binding_indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.binding_indices.is_empty()
    }

    pub(crate) fn apply(
        &self,
        meshes: &mut [Moc3DrawableMesh],
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<()> {
        for index in 0..self.len() {
            let info_count = usize::try_from(*self.info_counts.get(index)?).ok()?;
            if info_count == 0 {
                continue;
            }
            if info_count % 2 != 0 {
                return None;
            }

            let mesh_a = usize::try_from(*self.art_mesh_indices_a.get(index)?).ok()?;
            let mesh_b = usize::try_from(*self.art_mesh_indices_b.get(index)?).ok()?;
            let intensity = self.interpolate_intensity(index, bindings, parameter_values)?;
            let info_begin = usize::try_from(*self.info_begin_indices.get(index)?).ok()?;
            let info_end = info_begin.checked_add(info_count)?;
            let weights = self.info_weights.get(info_begin..info_end)?;
            let position_indices = self.info_position_indices.get(info_begin..info_end)?;

            apply_glue_to_mesh_pair(meshes, mesh_a, mesh_b, weights, position_indices, intensity)?;
        }

        Some(())
    }

    fn interpolate_intensity(
        &self,
        index: usize,
        bindings: &Moc3KeyformBindings,
        parameter_values: &[f32],
    ) -> Option<f32> {
        let keyform_count = usize::try_from(*self.keyform_counts.get(index)?).ok()?;
        let slots = bindings.keyform_slots(
            *self.binding_indices.get(index)?,
            keyform_count,
            parameter_values,
        )?;
        let begin = usize::try_from(*self.keyform_begin_indices.get(index)?).ok()?;
        let mut intensity = 0.0f32;
        for slot in slots {
            let keyform_index = begin.checked_add(slot.local_index)?;
            intensity += *self.keyform_intensities.get(keyform_index)? * slot.weight;
        }
        Some(intensity)
    }
}

fn apply_glue_to_mesh_pair(
    meshes: &mut [Moc3DrawableMesh],
    mesh_a: usize,
    mesh_b: usize,
    weights: &[f32],
    position_indices: &[u16],
    intensity: f32,
) -> Option<()> {
    if mesh_a == mesh_b {
        return None;
    }

    let (mesh_a, mesh_b) = mutable_pair(meshes, mesh_a, mesh_b)?;

    for pair in (0..position_indices.len()).step_by(2) {
        let index_a = usize::from(*position_indices.get(pair)?);
        let index_b = usize::from(*position_indices.get(pair + 1)?);
        let weight_a = *weights.get(pair)?;
        let weight_b = *weights.get(pair + 1)?;

        let vertex_a = *mesh_a.vertices().get(index_a)?;
        let vertex_b = *mesh_b.vertices().get(index_b)?;
        let (position_a, position_b) = affect_art_mesh_pair(
            vertex_position(vertex_a),
            vertex_position(vertex_b),
            weight_a,
            weight_b,
            intensity,
        );
        mesh_a.vertices_mut()[index_a] = vertex_with_position(vertex_a, position_a);
        mesh_b.vertices_mut()[index_b] = vertex_with_position(vertex_b, position_b);
    }

    Some(())
}

fn mutable_pair<T>(slice: &mut [T], a: usize, b: usize) -> Option<(&mut T, &mut T)> {
    if a < b {
        let (left, right) = slice.split_at_mut(b);
        Some((left.get_mut(a)?, right.get_mut(0)?))
    } else {
        let (left, right) = slice.split_at_mut(a);
        Some((right.get_mut(0)?, left.get_mut(b)?))
    }
}

fn vertex_position(vertex: Moc3DrawableVertex) -> Vector2 {
    Vector2::new(vertex.position()[0], vertex.position()[1])
}

fn vertex_with_position(vertex: Moc3DrawableVertex, position: Vector2) -> Moc3DrawableVertex {
    Moc3DrawableVertex::new([position.x(), position.y()], vertex.uv())
}
