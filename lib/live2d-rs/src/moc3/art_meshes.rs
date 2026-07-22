use crate::Result;

use super::{
    Moc3CountInfo, Moc3Header, Moc3SectionOffsets,
    parse::{
        invalid_moc3, nonnegative_range_len, read_f32_section, read_i16_section, read_i32_section,
        read_i32_section_or_default, read_u8_section, to_usize, validate_art_mesh_range,
    },
};

const ART_MESH_KEYFORM_BINDING_BAND_INDICES_SLOT: usize = 34;
const ART_MESH_PARENT_DEFORMER_INDICES_SLOT: usize = 40;
const TEXTURE_INDICES_SLOT: usize = 41;
const DRAWABLE_FLAGS_SLOT: usize = 42;
const VERTEX_COUNTS_SLOT: usize = 43;
const UV_BEGIN_INDICES_SLOT: usize = 44;
const POSITION_INDEX_BEGIN_INDICES_SLOT: usize = 45;
const POSITION_INDEX_COUNTS_SLOT: usize = 46;
const MASK_BEGIN_INDICES_SLOT: usize = 47;
const MASK_COUNTS_SLOT: usize = 48;
const UV_XYS_SLOT: usize = 78;
const POSITION_INDICES_SLOT: usize = 79;
const DRAWABLE_MASKS_SLOT: usize = 80;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Moc3ArtMeshInfo {
    texture_index: i32,
    drawable_flags: u8,
    position_index_count: i32,
    uv_begin_index: i32,
    position_index_begin_index: i32,
    vertex_count: i32,
    mask_begin_index: i32,
    mask_count: i32,
}

impl Moc3ArtMeshInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        texture_index: i32,
        drawable_flags: u8,
        position_index_count: i32,
        uv_begin_index: i32,
        position_index_begin_index: i32,
        vertex_count: i32,
        mask_begin_index: i32,
        mask_count: i32,
    ) -> Self {
        Self {
            texture_index,
            drawable_flags,
            position_index_count,
            uv_begin_index,
            position_index_begin_index,
            vertex_count,
            mask_begin_index,
            mask_count,
        }
    }

    pub fn texture_index(&self) -> i32 {
        self.texture_index
    }

    pub fn drawable_flags(&self) -> u8 {
        self.drawable_flags
    }

    pub fn position_index_count(&self) -> i32 {
        self.position_index_count
    }

    pub fn uv_begin_index(&self) -> i32 {
        self.uv_begin_index
    }

    pub fn position_index_begin_index(&self) -> i32 {
        self.position_index_begin_index
    }

    pub fn vertex_count(&self) -> i32 {
        self.vertex_count
    }

    pub fn mask_begin_index(&self) -> i32 {
        self.mask_begin_index
    }

    pub fn mask_count(&self) -> i32 {
        self.mask_count
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Moc3ArtMeshes {
    meshes: Vec<Moc3ArtMeshInfo>,
    keyform_binding_band_indices: Vec<i32>,
    parent_deformer_indices: Vec<i32>,
    render_orders: Vec<i32>,
    uv_xys: Vec<f32>,
    position_indices: Vec<i16>,
    drawable_masks: Vec<i32>,
}

impl Moc3ArtMeshes {
    pub fn from_parts(
        meshes: Vec<Moc3ArtMeshInfo>,
        uv_xys: Vec<f32>,
        position_indices: Vec<i16>,
        drawable_masks: Vec<i32>,
    ) -> Result<Self> {
        let mesh_count = meshes.len();
        Self::from_parts_with_hierarchy(
            meshes,
            vec![0; mesh_count],
            vec![-1; mesh_count],
            uv_xys,
            position_indices,
            drawable_masks,
        )
    }

    pub fn from_parts_with_hierarchy(
        meshes: Vec<Moc3ArtMeshInfo>,
        keyform_binding_band_indices: Vec<i32>,
        parent_deformer_indices: Vec<i32>,
        uv_xys: Vec<f32>,
        position_indices: Vec<i16>,
        drawable_masks: Vec<i32>,
    ) -> Result<Self> {
        let render_orders = default_render_orders(meshes.len());
        Self::from_parts_with_hierarchy_and_render_orders(
            meshes,
            keyform_binding_band_indices,
            parent_deformer_indices,
            render_orders,
            uv_xys,
            position_indices,
            drawable_masks,
        )
    }

    pub fn from_parts_with_hierarchy_and_render_orders(
        meshes: Vec<Moc3ArtMeshInfo>,
        keyform_binding_band_indices: Vec<i32>,
        parent_deformer_indices: Vec<i32>,
        render_orders: Vec<i32>,
        uv_xys: Vec<f32>,
        position_indices: Vec<i16>,
        drawable_masks: Vec<i32>,
    ) -> Result<Self> {
        if keyform_binding_band_indices.len() != meshes.len()
            || parent_deformer_indices.len() != meshes.len()
            || render_orders.len() != meshes.len()
        {
            return Err(invalid_moc3(
                "art mesh hierarchy metadata lengths do not match",
            ));
        }

        for (index, mesh) in meshes.iter().copied().enumerate() {
            validate_mesh_ranges(
                index,
                mesh,
                uv_xys.len(),
                position_indices.len(),
                drawable_masks.len(),
            )?;
        }

        Ok(Self {
            meshes,
            keyform_binding_band_indices,
            parent_deformer_indices,
            render_orders,
            uv_xys,
            position_indices,
            drawable_masks,
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;
        let art_mesh_count = to_usize(counts.art_meshes(), "art mesh count")?;
        let render_orders = default_render_orders(art_mesh_count);

        let keyform_binding_band_indices = read_i32_section_or_default(
            bytes,
            &offsets,
            ART_MESH_KEYFORM_BINDING_BAND_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
            0,
        )?;
        let texture_indices = read_i32_section(
            bytes,
            &offsets,
            TEXTURE_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let drawable_flags = read_u8_section(bytes, &offsets, DRAWABLE_FLAGS_SLOT, art_mesh_count)?;
        let vertex_counts = read_i32_section(
            bytes,
            &offsets,
            VERTEX_COUNTS_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let uv_begin_indices = read_i32_section(
            bytes,
            &offsets,
            UV_BEGIN_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let position_index_begin_indices = read_i32_section(
            bytes,
            &offsets,
            POSITION_INDEX_BEGIN_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let position_index_counts = read_i32_section(
            bytes,
            &offsets,
            POSITION_INDEX_COUNTS_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let mask_begin_indices = read_i32_section(
            bytes,
            &offsets,
            MASK_BEGIN_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let mask_counts = read_i32_section(
            bytes,
            &offsets,
            MASK_COUNTS_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let parent_deformer_indices = read_i32_section_or_default(
            bytes,
            &offsets,
            ART_MESH_PARENT_DEFORMER_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
            -1,
        )?;
        let uv_xys = read_f32_section(
            bytes,
            &offsets,
            UV_XYS_SLOT,
            to_usize(counts.uvs(), "uv count")?,
            header.endianness(),
        )?;
        let position_indices = read_i16_section(
            bytes,
            &offsets,
            POSITION_INDICES_SLOT,
            to_usize(counts.position_indices(), "position index count")?,
            header.endianness(),
        )?;
        let drawable_masks = read_i32_section(
            bytes,
            &offsets,
            DRAWABLE_MASKS_SLOT,
            to_usize(counts.drawable_masks(), "drawable mask count")?,
            header.endianness(),
        )?;

        let mut meshes = Vec::with_capacity(art_mesh_count);
        for index in 0..art_mesh_count {
            meshes.push(Moc3ArtMeshInfo::new(
                texture_indices[index],
                drawable_flags[index],
                position_index_counts[index],
                uv_begin_indices[index],
                position_index_begin_indices[index],
                vertex_counts[index],
                mask_begin_indices[index],
                mask_counts[index],
            ));
        }

        Self::from_parts_with_hierarchy_and_render_orders(
            meshes,
            keyform_binding_band_indices,
            parent_deformer_indices,
            render_orders,
            uv_xys,
            position_indices,
            drawable_masks,
        )
    }

    pub fn meshes(&self) -> &[Moc3ArtMeshInfo] {
        &self.meshes
    }

    pub fn keyform_binding_band_indices(&self) -> &[i32] {
        &self.keyform_binding_band_indices
    }

    pub fn parent_deformer_indices(&self) -> &[i32] {
        &self.parent_deformer_indices
    }

    pub fn render_orders(&self) -> &[i32] {
        &self.render_orders
    }

    pub fn uv_xys(&self) -> &[f32] {
        &self.uv_xys
    }

    pub fn position_indices(&self) -> &[i16] {
        &self.position_indices
    }

    pub fn drawable_masks(&self) -> &[i32] {
        &self.drawable_masks
    }

    pub fn art_mesh_uvs(&self, index: usize) -> Option<&[f32]> {
        let mesh = self.meshes.get(index)?;
        let start = usize::try_from(mesh.uv_begin_index).ok()?;
        let len = usize::try_from(mesh.vertex_count).ok()?.checked_mul(2)?;
        self.uv_xys.get(start..start.checked_add(len)?)
    }

    pub fn art_mesh_position_indices(&self, index: usize) -> Option<&[i16]> {
        let mesh = self.meshes.get(index)?;
        let start = usize::try_from(mesh.position_index_begin_index).ok()?;
        let len = usize::try_from(mesh.position_index_count).ok()?;
        self.position_indices.get(start..start.checked_add(len)?)
    }

    pub fn art_mesh_masks(&self, index: usize) -> Option<&[i32]> {
        let mesh = self.meshes.get(index)?;
        let start = usize::try_from(mesh.mask_begin_index).ok()?;
        let len = usize::try_from(mesh.mask_count).ok()?;
        self.drawable_masks.get(start..start.checked_add(len)?)
    }

    pub fn art_mesh_keyform_binding_band_index(&self, index: usize) -> Option<i32> {
        self.keyform_binding_band_indices.get(index).copied()
    }

    pub fn art_mesh_parent_deformer_index(&self, index: usize) -> Option<i32> {
        self.parent_deformer_indices.get(index).copied()
    }

    pub fn art_mesh_render_order(&self, index: usize) -> Option<i32> {
        self.render_orders.get(index).copied()
    }
}

fn default_render_orders(mesh_count: usize) -> Vec<i32> {
    (0..mesh_count)
        .map(|index| i32::try_from(index).unwrap_or(i32::MAX))
        .collect()
}

fn validate_mesh_ranges(
    index: usize,
    mesh: Moc3ArtMeshInfo,
    uv_count: usize,
    position_index_count: usize,
    drawable_mask_count: usize,
) -> Result<()> {
    let uv_len = nonnegative_range_len(mesh.vertex_count, 2, "vertex count")?;
    validate_art_mesh_range(mesh.uv_begin_index, uv_len, uv_count, index, "uv")?;

    let position_len = nonnegative_range_len(mesh.position_index_count, 1, "position index count")?;
    validate_art_mesh_range(
        mesh.position_index_begin_index,
        position_len,
        position_index_count,
        index,
        "position index",
    )?;

    let mask_len = nonnegative_range_len(mesh.mask_count, 1, "mask count")?;
    validate_art_mesh_range(
        mesh.mask_begin_index,
        mask_len,
        drawable_mask_count,
        index,
        "mask",
    )
}
