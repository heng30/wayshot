use crate::{Error, Result};

use super::{Moc3Header, parse::read_u32};

const OFFSET_TABLE_START: usize = 0x40;
const OFFSET_COUNT: usize = 160;
const U32_SIZE: usize = 4;
const OFFSET_TABLE_END: usize = OFFSET_TABLE_START + OFFSET_COUNT * U32_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moc3SectionOffsets {
    offsets: [u32; OFFSET_COUNT],
}

impl Moc3SectionOffsets {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let table_len = OFFSET_COUNT * U32_SIZE;
        if bytes.len() < OFFSET_TABLE_START + table_len {
            return Err(invalid_offsets("section offset table is incomplete"));
        }

        let mut offsets = [0; OFFSET_COUNT];
        for (index, offset) in offsets.iter_mut().enumerate() {
            *offset = read_u32(
                bytes,
                OFFSET_TABLE_START + index * U32_SIZE,
                header.endianness(),
            );
        }

        validate_required_offset(bytes, offsets[0], "count info")?;
        validate_required_offset(bytes, offsets[1], "canvas info")?;

        for (index, offset) in offsets.iter().copied().enumerate().skip(2) {
            validate_optional_offset(bytes, offset, index)?;
        }

        Ok(Self { offsets })
    }

    pub fn count_info_offset(&self) -> u32 {
        self.offsets[0]
    }

    pub fn canvas_info_offset(&self) -> u32 {
        self.offsets[1]
    }

    pub fn section_offsets(&self) -> &[u32; OFFSET_COUNT] {
        &self.offsets
    }

    pub fn section_offset(&self, index: usize) -> Option<u32> {
        self.offsets.get(index).copied()
    }
}

fn validate_required_offset(bytes: &[u8], offset: u32, name: &'static str) -> Result<()> {
    validate_offset(bytes, offset, name)
}

fn validate_optional_offset(bytes: &[u8], offset: u32, index: usize) -> Result<()> {
    if offset == 0 {
        return Ok(());
    }

    if usize::try_from(offset).ok() == Some(bytes.len()) {
        return Ok(());
    }

    validate_offset(bytes, offset, format!("section {index}"))
}

fn validate_offset(bytes: &[u8], offset: u32, name: impl std::fmt::Display) -> Result<()> {
    let offset = usize::try_from(offset)
        .map_err(|_| invalid_offsets(format!("{name} offset does not fit in platform usize")))?;

    if offset >= bytes.len() {
        return Err(invalid_offsets(format!("{name} offset is outside file")));
    }

    if offset < OFFSET_TABLE_END {
        return Err(invalid_offsets(format!(
            "{name} offset points into the header or offset table"
        )));
    }

    if offset % U32_SIZE != 0 {
        return Err(invalid_offsets(format!(
            "{name} offset is not 4-byte aligned"
        )));
    }

    Ok(())
}

fn invalid_offsets(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
