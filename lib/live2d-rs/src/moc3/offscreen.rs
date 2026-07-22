use crate::Result;

use super::{
    Moc3CountInfo, Moc3Header, Moc3Ids, Moc3SectionOffsets, Moc3Version,
    parse::{read_i32_section, to_usize},
};

const PART_PARENT_PART_INDICES_SLOT: usize = 9;
const DRAWABLE_PARENT_PART_INDICES_SLOT: usize = 39;
const PART_OFFSCREEN_INDICES_SLOT: usize = 149;
const OFFSCREEN_OWNER_PART_INDICES_SLOT: usize = 155;
const EFFECT_PART_ID: &str = "PartEffect";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moc3OffscreenInfo {
    part_parent_indices: Vec<i32>,
    drawable_parent_part_indices: Vec<i32>,
    part_offscreen_indices: Vec<i32>,
    offscreen_owner_part_indices: Vec<i32>,
}

impl Moc3OffscreenInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;
        let endianness = header.endianness();
        let part_count = to_usize(counts.parts(), "part count")?;
        let drawable_count = to_usize(counts.art_meshes(), "art mesh count")?;

        let part_parent_indices = read_i32_section(
            bytes,
            &offsets,
            PART_PARENT_PART_INDICES_SLOT,
            part_count,
            endianness,
        )?;
        let drawable_parent_part_indices = read_i32_section(
            bytes,
            &offsets,
            DRAWABLE_PARENT_PART_INDICES_SLOT,
            drawable_count,
            endianness,
        )?;
        let part_offscreen_indices = if header.version() == Moc3Version::V5_3_0 {
            read_i32_section(
                bytes,
                &offsets,
                PART_OFFSCREEN_INDICES_SLOT,
                part_count,
                endianness,
            )?
        } else {
            vec![-1; part_count]
        };
        let offscreen_count = match part_offscreen_indices
            .iter()
            .copied()
            .filter(|index| *index >= 0)
            .max()
        {
            Some(index) => usize::try_from(index).map_or(0, |index| index + 1),
            None => 0,
        };
        let offscreen_owner_part_indices = if offscreen_count == 0 {
            Vec::new()
        } else {
            read_i32_section(
                bytes,
                &offsets,
                OFFSCREEN_OWNER_PART_INDICES_SLOT,
                offscreen_count,
                endianness,
            )?
        };

        Ok(Self {
            part_parent_indices,
            drawable_parent_part_indices,
            part_offscreen_indices,
            offscreen_owner_part_indices,
        })
    }

    pub fn offscreen_count(&self) -> usize {
        self.offscreen_owner_part_indices.len()
    }

    pub fn part_offscreen_index(&self, part_index: usize) -> Option<i32> {
        self.part_offscreen_indices.get(part_index).copied()
    }

    pub fn part_offscreen_indices(&self) -> &[i32] {
        &self.part_offscreen_indices
    }

    pub fn offscreen_owner_part_index(&self, offscreen_index: usize) -> Option<i32> {
        self.offscreen_owner_part_indices
            .get(offscreen_index)
            .copied()
    }

    pub fn drawable_parent_part_index(&self, drawable_index: usize) -> Option<i32> {
        self.drawable_parent_part_indices
            .get(drawable_index)
            .copied()
    }

    pub(crate) fn effect_source_drawable_indices(&self, ids: &Moc3Ids) -> Vec<usize> {
        if self.offscreen_count() == 0 {
            return Vec::new();
        }

        let Some(effect_part_index) = ids.parts().iter().position(|id| id == EFFECT_PART_ID) else {
            return Vec::new();
        };

        self.drawable_parent_part_indices
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(drawable_index, parent_part_index)| {
                self.is_part_descendant_of(parent_part_index, effect_part_index)
                    .then_some(drawable_index)
            })
            .collect()
    }

    fn is_part_descendant_of(&self, part_index: i32, ancestor_index: usize) -> bool {
        let mut part_index = part_index;
        let mut guard = 0usize;

        while part_index >= 0 {
            let Ok(index) = usize::try_from(part_index) else {
                return false;
            };
            if index == ancestor_index {
                return true;
            }
            let Some(parent_index) = self.part_parent_indices.get(index).copied() else {
                return false;
            };
            part_index = parent_index;
            guard += 1;
            if guard > self.part_parent_indices.len() {
                return false;
            }
        }

        false
    }
}
