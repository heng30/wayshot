use crate::{Error, Result};

use super::{Moc3CountInfo, Moc3SectionOffsets};

const STR64_SIZE: usize = 64;
const PART_IDS_SLOT: usize = 3;
const ART_MESH_IDS_SLOT: usize = 33;
const PARAMETER_IDS_SLOT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moc3Ids {
    parts: Vec<String>,
    art_meshes: Vec<String>,
    parameters: Vec<String>,
}

impl Moc3Ids {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let counts = Moc3CountInfo::parse(bytes)?;

        Ok(Self {
            parts: read_str64_section(bytes, &offsets, PART_IDS_SLOT, counts.parts())?,
            art_meshes: read_str64_section(
                bytes,
                &offsets,
                ART_MESH_IDS_SLOT,
                counts.art_meshes(),
            )?,
            parameters: read_str64_section(
                bytes,
                &offsets,
                PARAMETER_IDS_SLOT,
                counts.parameters(),
            )?,
        })
    }

    pub fn parts(&self) -> &[String] {
        &self.parts
    }

    pub fn art_meshes(&self) -> &[String] {
        &self.art_meshes
    }

    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }
}

fn read_str64_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: u32,
) -> Result<Vec<String>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let offset = offsets
        .section_offset(slot)
        .ok_or_else(|| invalid_ids(format!("section slot {slot} is outside offset table")))?;

    if offset == 0 {
        return Err(invalid_ids(format!("section slot {slot} has no offset")));
    }

    let offset = usize::try_from(offset)
        .map_err(|_| invalid_ids(format!("section slot {slot} offset is too large")))?;
    let count = usize::try_from(count)
        .map_err(|_| invalid_ids(format!("section slot {slot} count is too large")))?;
    let byte_len = count
        .checked_mul(STR64_SIZE)
        .ok_or_else(|| invalid_ids(format!("section slot {slot} size overflows")))?;

    if bytes.len().saturating_sub(offset) < byte_len {
        return Err(invalid_ids(format!("section slot {slot} is incomplete")));
    }

    let mut ids = Vec::with_capacity(count);
    for index in 0..count {
        let start = offset + index * STR64_SIZE;
        let raw = &bytes[start..start + STR64_SIZE];
        let end = raw.iter().position(|byte| *byte == 0).unwrap_or(STR64_SIZE);
        ids.push(String::from_utf8_lossy(&raw[..end]).into_owned());
    }

    Ok(ids)
}

fn invalid_ids(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
