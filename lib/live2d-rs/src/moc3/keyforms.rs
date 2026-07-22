use crate::Result;

use super::{
    Moc3CountInfo, Moc3Header, Moc3SectionOffsets,
    parse::{
        invalid_moc3, nonnegative_range_len, read_f32_section, read_f32_section_or_default,
        read_i32_section, read_i32_section_or_default, to_usize, validate_art_mesh_range,
    },
};

const KEYFORM_BEGIN_INDICES_SLOT: usize = 35;
const KEYFORM_COUNTS_SLOT: usize = 36;
const VERTEX_COUNTS_SLOT: usize = 43;
const ART_MESH_KEYFORM_OPACITIES_SLOT: usize = 68;
const ART_MESH_KEYFORM_DRAW_ORDERS_SLOT: usize = 69;
const KEYFORM_POSITION_BEGIN_INDICES_SLOT: usize = 70;
const KEYFORM_POSITION_XYS_SLOT: usize = 71;
const ART_MESH_KEYFORM_COLOR_BEGIN_INDICES_SLOT: usize = 107;
const KEYFORM_MULTIPLY_COLOR_SLOTS: [usize; 3] = [108, 109, 110];
const KEYFORM_SCREEN_COLOR_SLOTS: [usize; 3] = [111, 112, 113];

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Moc3ArtMeshKeyformInfo {
    opacity: f32,
    draw_order: f32,
    position_begin_index: i32,
    multiply_color: [f32; 3],
    screen_color: [f32; 3],
}

impl Moc3ArtMeshKeyformInfo {
    pub fn new(opacity: f32, draw_order: f32, position_begin_index: i32) -> Self {
        Self::with_colors(
            opacity,
            draw_order,
            position_begin_index,
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        )
    }

    pub fn with_colors(
        opacity: f32,
        draw_order: f32,
        position_begin_index: i32,
        multiply_color: [f32; 3],
        screen_color: [f32; 3],
    ) -> Self {
        Self {
            opacity,
            draw_order,
            position_begin_index,
            multiply_color,
            screen_color,
        }
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn draw_order(&self) -> f32 {
        self.draw_order
    }

    pub fn position_begin_index(&self) -> i32 {
        self.position_begin_index
    }

    pub fn multiply_color(&self) -> [f32; 3] {
        self.multiply_color
    }

    pub fn screen_color(&self) -> [f32; 3] {
        self.screen_color
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Moc3ArtMeshKeyforms {
    keyform_begin_indices: Vec<i32>,
    keyform_counts: Vec<i32>,
    vertex_counts: Vec<i32>,
    keyforms: Vec<Moc3ArtMeshKeyformInfo>,
    position_xys: Vec<f32>,
}

impl Moc3ArtMeshKeyforms {
    pub fn from_parts(
        keyform_begin_indices: Vec<i32>,
        keyform_counts: Vec<i32>,
        vertex_counts: Vec<i32>,
        keyforms: Vec<Moc3ArtMeshKeyformInfo>,
        position_xys: Vec<f32>,
    ) -> Result<Self> {
        if keyform_begin_indices.len() != keyform_counts.len()
            || keyform_begin_indices.len() != vertex_counts.len()
        {
            return Err(invalid_moc3(
                "art mesh keyform metadata lengths do not match",
            ));
        }

        for mesh_index in 0..keyform_begin_indices.len() {
            validate_keyform_ranges(
                mesh_index,
                keyform_begin_indices[mesh_index],
                keyform_counts[mesh_index],
                vertex_counts[mesh_index],
                &keyforms,
                position_xys.len(),
            )?;
        }

        Ok(Self {
            keyform_begin_indices,
            keyform_counts,
            vertex_counts,
            keyforms,
            position_xys,
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;
        let art_mesh_count = to_usize(counts.art_meshes(), "art mesh count")?;
        let art_mesh_keyform_count =
            to_usize(counts.art_mesh_keyforms(), "art mesh keyform count")?;

        let keyform_begin_indices = read_i32_section(
            bytes,
            &offsets,
            KEYFORM_BEGIN_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let keyform_counts = read_i32_section(
            bytes,
            &offsets,
            KEYFORM_COUNTS_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let vertex_counts = read_i32_section(
            bytes,
            &offsets,
            VERTEX_COUNTS_SLOT,
            art_mesh_count,
            header.endianness(),
        )?;
        let opacities = read_f32_section(
            bytes,
            &offsets,
            ART_MESH_KEYFORM_OPACITIES_SLOT,
            art_mesh_keyform_count,
            header.endianness(),
        )?;
        let draw_orders = read_f32_section(
            bytes,
            &offsets,
            ART_MESH_KEYFORM_DRAW_ORDERS_SLOT,
            art_mesh_keyform_count,
            header.endianness(),
        )?;
        let position_begin_indices = read_i32_section(
            bytes,
            &offsets,
            KEYFORM_POSITION_BEGIN_INDICES_SLOT,
            art_mesh_keyform_count,
            header.endianness(),
        )?;
        let position_xys = read_f32_section(
            bytes,
            &offsets,
            KEYFORM_POSITION_XYS_SLOT,
            to_usize(counts.keyform_positions(), "keyform position count")?,
            header.endianness(),
        )?;
        let art_mesh_keyform_color_begin_indices = read_i32_section_or_default(
            bytes,
            &offsets,
            ART_MESH_KEYFORM_COLOR_BEGIN_INDICES_SLOT,
            art_mesh_count,
            header.endianness(),
            -1,
        )?;
        let read_color_channels =
            |slots: [usize; 3], count: usize, default: f32| -> Result<[Vec<f32>; 3]> {
                Ok([
                    read_f32_section_or_default(
                        bytes,
                        &offsets,
                        slots[0],
                        count,
                        header.endianness(),
                        default,
                    )?,
                    read_f32_section_or_default(
                        bytes,
                        &offsets,
                        slots[1],
                        count,
                        header.endianness(),
                        default,
                    )?,
                    read_f32_section_or_default(
                        bytes,
                        &offsets,
                        slots[2],
                        count,
                        header.endianness(),
                        default,
                    )?,
                ])
            };
        let multiply_colors = read_color_channels(
            KEYFORM_MULTIPLY_COLOR_SLOTS,
            to_usize(
                counts.keyform_multiply_colors(),
                "keyform multiply color count",
            )?,
            1.0,
        )?;
        let screen_colors = read_color_channels(
            KEYFORM_SCREEN_COLOR_SLOTS,
            to_usize(counts.keyform_screen_colors(), "keyform screen color count")?,
            0.0,
        )?;

        let mut keyforms = (0..art_mesh_keyform_count)
            .map(|i| {
                Moc3ArtMeshKeyformInfo::with_colors(
                    opacities[i],
                    draw_orders[i],
                    position_begin_indices[i],
                    [1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0],
                )
            })
            .collect::<Vec<_>>();
        for mesh_index in 0..art_mesh_count {
            let keyform_begin = usize::try_from(keyform_begin_indices[mesh_index])
                .map_err(|_| invalid_moc3("art mesh keyform begin index is negative"))?;
            let keyform_count = usize::try_from(keyform_counts[mesh_index])
                .map_err(|_| invalid_moc3("art mesh keyform count is negative"))?;
            let color_begin = art_mesh_keyform_color_begin_indices[mesh_index];
            if color_begin < 0 {
                continue;
            }
            let color_begin = usize::try_from(color_begin)
                .map_err(|_| invalid_moc3("art mesh color begin index is too large"))?;
            for local_index in 0..keyform_count {
                let keyform_index = keyform_begin
                    .checked_add(local_index)
                    .ok_or_else(|| invalid_moc3("art mesh keyform index overflows"))?;
                let color_index = color_begin
                    .checked_add(local_index)
                    .ok_or_else(|| invalid_moc3("art mesh color index overflows"))?;
                let keyform = keyforms.get_mut(keyform_index).ok_or_else(|| {
                    invalid_moc3("art mesh keyform color index is outside keyforms")
                })?;
                keyform.multiply_color = color_at(&multiply_colors, color_index, [1.0, 1.0, 1.0]);
                keyform.screen_color = color_at(&screen_colors, color_index, [0.0, 0.0, 0.0]);
            }
        }

        Self::from_parts(
            keyform_begin_indices,
            keyform_counts,
            vertex_counts,
            keyforms,
            position_xys,
        )
    }

    pub fn keyforms(&self) -> &[Moc3ArtMeshKeyformInfo] {
        &self.keyforms
    }

    pub fn position_xys(&self) -> &[f32] {
        &self.position_xys
    }

    pub fn art_mesh_keyforms(&self, mesh_index: usize) -> Option<&[Moc3ArtMeshKeyformInfo]> {
        let start = usize::try_from(*self.keyform_begin_indices.get(mesh_index)?).ok()?;
        let len = usize::try_from(*self.keyform_counts.get(mesh_index)?).ok()?;
        self.keyforms.get(start..start.checked_add(len)?)
    }

    pub fn art_mesh_keyform_positions(
        &self,
        mesh_index: usize,
        local_keyform_index: usize,
    ) -> Option<&[f32]> {
        let keyform = self
            .art_mesh_keyforms(mesh_index)?
            .get(local_keyform_index)?;
        let vertex_count = *self.vertex_counts.get(mesh_index)?;
        let start = usize::try_from(keyform.position_begin_index).ok()?;
        let len = usize::try_from(vertex_count).ok()?.checked_mul(2)?;
        self.position_xys.get(start..start.checked_add(len)?)
    }
}

fn color_at(channels: &[Vec<f32>; 3], index: usize, default: [f32; 3]) -> [f32; 3] {
    [
        channels[0].get(index).copied().unwrap_or(default[0]),
        channels[1].get(index).copied().unwrap_or(default[1]),
        channels[2].get(index).copied().unwrap_or(default[2]),
    ]
}

fn validate_keyform_ranges(
    mesh_index: usize,
    keyform_begin_index: i32,
    keyform_count: i32,
    vertex_count: i32,
    keyforms: &[Moc3ArtMeshKeyformInfo],
    position_count: usize,
) -> Result<()> {
    let keyform_len = nonnegative_range_len(keyform_count, 1, "art mesh keyform count")?;
    validate_art_mesh_range(
        keyform_begin_index,
        keyform_len,
        keyforms.len(),
        mesh_index,
        "keyform",
    )?;

    let keyform_begin_index = usize::try_from(keyform_begin_index).map_err(|_| {
        invalid_moc3(format!(
            "art mesh {mesh_index} keyform begin index is too large"
        ))
    })?;
    let position_len = nonnegative_range_len(vertex_count, 2, "vertex count")?;

    for keyform in keyforms.iter().skip(keyform_begin_index).take(keyform_len) {
        validate_art_mesh_range(
            keyform.position_begin_index,
            position_len,
            position_count,
            mesh_index,
            "keyform position",
        )?;
    }

    Ok(())
}
