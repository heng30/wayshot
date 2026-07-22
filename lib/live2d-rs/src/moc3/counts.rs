use crate::{Error, Result};

use super::{Moc3Header, Moc3SectionOffsets, parse::read_u32};

const U32_SIZE: usize = 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Moc3CountInfo {
    parts: u32,
    deformers: u32,
    warp_deformers: u32,
    rotation_deformers: u32,
    art_meshes: u32,
    parameters: u32,
    part_keyforms: u32,
    warp_deformer_keyforms: u32,
    rotation_deformer_keyforms: u32,
    art_mesh_keyforms: u32,
    keyform_positions: u32,
    parameter_binding_indices: u32,
    keyform_bindings: u32,
    parameter_bindings: u32,
    keys: u32,
    uvs: u32,
    position_indices: u32,
    drawable_masks: u32,
    draw_order_groups: u32,
    draw_order_group_objects: u32,
    glue: u32,
    glue_info: u32,
    glue_keyforms: u32,
    keyform_multiply_colors: u32,
    keyform_screen_colors: u32,
}

impl Moc3CountInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let offset = usize::try_from(offsets.count_info_offset())
            .map_err(|_| invalid_counts("count info offset does not fit in platform usize"))?;
        let word_count = header.version().count_info_word_count();
        let required_len = word_count * U32_SIZE;

        if bytes.len().saturating_sub(offset) < required_len {
            return Err(invalid_counts("count info table is incomplete"));
        }

        let read = |index| read_u32(bytes, offset + index * U32_SIZE, header.endianness());

        Ok(Self {
            parts: read(0),
            deformers: read(1),
            warp_deformers: read(2),
            rotation_deformers: read(3),
            art_meshes: read(4),
            parameters: read(5),
            part_keyforms: read(6),
            warp_deformer_keyforms: read(7),
            rotation_deformer_keyforms: read(8),
            art_mesh_keyforms: read(9),
            keyform_positions: read(10),
            parameter_binding_indices: read(11),
            keyform_bindings: read(12),
            parameter_bindings: read(13),
            keys: read(14),
            uvs: read(15),
            position_indices: read(16),
            drawable_masks: read(17),
            draw_order_groups: read(18),
            draw_order_group_objects: read(19),
            glue: read(20),
            glue_info: read(21),
            glue_keyforms: read(22),
            keyform_multiply_colors: if word_count > 23 { read(23) } else { 0 },
            keyform_screen_colors: if word_count > 24 { read(24) } else { 0 },
        })
    }

    pub fn parts(&self) -> u32 {
        self.parts
    }

    pub fn deformers(&self) -> u32 {
        self.deformers
    }

    pub fn warp_deformers(&self) -> u32 {
        self.warp_deformers
    }

    pub fn rotation_deformers(&self) -> u32 {
        self.rotation_deformers
    }

    pub fn art_meshes(&self) -> u32 {
        self.art_meshes
    }

    pub fn parameters(&self) -> u32 {
        self.parameters
    }

    pub fn part_keyforms(&self) -> u32 {
        self.part_keyforms
    }

    pub fn warp_deformer_keyforms(&self) -> u32 {
        self.warp_deformer_keyforms
    }

    pub fn rotation_deformer_keyforms(&self) -> u32 {
        self.rotation_deformer_keyforms
    }

    pub fn art_mesh_keyforms(&self) -> u32 {
        self.art_mesh_keyforms
    }

    pub fn keyform_positions(&self) -> u32 {
        self.keyform_positions
    }

    pub fn parameter_binding_indices(&self) -> u32 {
        self.parameter_binding_indices
    }

    pub fn keyform_bindings(&self) -> u32 {
        self.keyform_bindings
    }

    pub fn parameter_bindings(&self) -> u32 {
        self.parameter_bindings
    }

    pub fn keys(&self) -> u32 {
        self.keys
    }

    pub fn uvs(&self) -> u32 {
        self.uvs
    }

    pub fn position_indices(&self) -> u32 {
        self.position_indices
    }

    pub fn drawable_masks(&self) -> u32 {
        self.drawable_masks
    }

    pub fn draw_order_groups(&self) -> u32 {
        self.draw_order_groups
    }

    pub fn draw_order_group_objects(&self) -> u32 {
        self.draw_order_group_objects
    }

    pub fn glue(&self) -> u32 {
        self.glue
    }

    pub fn glue_info(&self) -> u32 {
        self.glue_info
    }

    pub fn glue_keyforms(&self) -> u32 {
        self.glue_keyforms
    }

    pub fn keyform_multiply_colors(&self) -> u32 {
        self.keyform_multiply_colors
    }

    pub fn keyform_screen_colors(&self) -> u32 {
        self.keyform_screen_colors
    }
}

fn invalid_counts(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
