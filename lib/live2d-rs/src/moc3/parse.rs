use crate::{Error, Result};

use super::{Endianness, Moc3SectionOffsets};

pub(super) fn read_i16(bytes: &[u8], offset: usize, endianness: Endianness) -> i16 {
    let raw = read_array(bytes, offset);
    match endianness {
        Endianness::Little => i16::from_le_bytes(raw),
        Endianness::Big => i16::from_be_bytes(raw),
    }
}

pub(super) fn read_u16(bytes: &[u8], offset: usize, endianness: Endianness) -> u16 {
    let raw = read_array(bytes, offset);
    match endianness {
        Endianness::Little => u16::from_le_bytes(raw),
        Endianness::Big => u16::from_be_bytes(raw),
    }
}

pub(super) fn read_i32(bytes: &[u8], offset: usize, endianness: Endianness) -> i32 {
    let raw = read_array(bytes, offset);
    match endianness {
        Endianness::Little => i32::from_le_bytes(raw),
        Endianness::Big => i32::from_be_bytes(raw),
    }
}

pub(super) fn read_u32(bytes: &[u8], offset: usize, endianness: Endianness) -> u32 {
    let raw = read_array(bytes, offset);
    match endianness {
        Endianness::Little => u32::from_le_bytes(raw),
        Endianness::Big => u32::from_be_bytes(raw),
    }
}

pub(super) fn read_f32(bytes: &[u8], offset: usize, endianness: Endianness) -> f32 {
    let raw = read_array(bytes, offset);
    match endianness {
        Endianness::Little => f32::from_le_bytes(raw),
        Endianness::Big => f32::from_be_bytes(raw),
    }
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    bytes[offset..offset + N]
        .try_into()
        .expect("caller validated fixed-width moc3 read")
}

pub(super) fn read_i32_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
) -> Result<Vec<i32>> {
    read_section(bytes, offsets, slot, count, 4, |bytes, offset| {
        read_i32(bytes, offset, endianness)
    })
}

pub(super) fn read_i32_section_or_default(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
    default: i32,
) -> Result<Vec<i32>> {
    match offsets.section_offset(slot) {
        Some(0) | None => Ok(vec![default; count]),
        Some(_) => read_i32_section(bytes, offsets, slot, count, endianness),
    }
}

pub(super) fn read_i16_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
) -> Result<Vec<i16>> {
    read_section(bytes, offsets, slot, count, 2, |bytes, offset| {
        read_i16(bytes, offset, endianness)
    })
}

pub(super) fn read_u16_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
) -> Result<Vec<u16>> {
    read_section(bytes, offsets, slot, count, 2, |bytes, offset| {
        read_u16(bytes, offset, endianness)
    })
}

pub(super) fn read_f32_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
) -> Result<Vec<f32>> {
    read_section(bytes, offsets, slot, count, 4, |bytes, offset| {
        read_f32(bytes, offset, endianness)
    })
}

pub(super) fn read_f32_section_or_default(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
    default: f32,
) -> Result<Vec<f32>> {
    match offsets.section_offset(slot) {
        Some(0) | None => Ok(vec![default; count]),
        Some(_) => read_f32_section(bytes, offsets, slot, count, endianness),
    }
}

pub(super) fn read_u8_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
) -> Result<Vec<u8>> {
    read_section(bytes, offsets, slot, count, 1, |bytes, offset| {
        bytes[offset]
    })
}

pub(super) fn read_bool_section(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    endianness: Endianness,
) -> Result<Vec<bool>> {
    read_i32_section(bytes, offsets, slot, count, endianness)
        .map(|values| values.into_iter().map(|value| value == 1).collect())
}

pub(super) fn read_section<T>(
    bytes: &[u8],
    offsets: &Moc3SectionOffsets,
    slot: usize,
    count: usize,
    element_size: usize,
    read: impl Fn(&[u8], usize) -> T,
) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let offset = offsets
        .section_offset(slot)
        .ok_or_else(|| invalid_moc3(format!("section slot {slot} is outside offset table")))?;
    if offset == 0 {
        return Err(invalid_moc3(format!("section slot {slot} has no offset")));
    }

    let offset = usize::try_from(offset)
        .map_err(|_| invalid_moc3(format!("section slot {slot} offset is too large")))?;
    let byte_len = count
        .checked_mul(element_size)
        .ok_or_else(|| invalid_moc3(format!("section slot {slot} size overflows")))?;
    if bytes.len().saturating_sub(offset) < byte_len {
        return Err(invalid_moc3(format!("section slot {slot} is incomplete")));
    }

    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(read(bytes, offset + index * element_size));
    }

    Ok(values)
}

pub(super) fn to_usize(value: u32, name: &'static str) -> Result<usize> {
    usize::try_from(value).map_err(|_| invalid_moc3(format!("{name} is too large")))
}

pub(super) fn nonnegative_range_len(value: i32, scale: usize, name: &'static str) -> Result<usize> {
    if value < 0 {
        return Err(invalid_moc3(format!("{name} is negative")));
    }

    usize::try_from(value)
        .ok()
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| invalid_moc3(format!("{name} range size overflows")))
}

pub(super) fn validate_art_mesh_range(
    begin: i32,
    len: usize,
    source_len: usize,
    mesh_index: usize,
    name: &'static str,
) -> Result<()> {
    if begin < 0 {
        return Err(invalid_moc3(format!(
            "art mesh {mesh_index} {name} begin index is negative"
        )));
    }

    let begin = usize::try_from(begin)
        .map_err(|_| invalid_moc3(format!("art mesh {mesh_index} {name} begin is too large")))?;
    let end = begin
        .checked_add(len)
        .ok_or_else(|| invalid_moc3(format!("art mesh {mesh_index} {name} range overflows")))?;

    if end > source_len {
        return Err(invalid_moc3(format!(
            "art mesh {mesh_index} {name} range is outside section"
        )));
    }

    Ok(())
}

pub(super) fn validate_count_range(
    begin: i32,
    count: i32,
    source_len: usize,
    index: usize,
    entity: &'static str,
    name: &'static str,
) -> Result<()> {
    if begin < 0 || count < 0 {
        return Err(invalid_moc3(format!(
            "{entity} {index} {name} range is negative"
        )));
    }
    let begin = usize::try_from(begin)
        .map_err(|_| invalid_moc3(format!("{entity} {index} {name} begin is too large")))?;
    let count = usize::try_from(count)
        .map_err(|_| invalid_moc3(format!("{entity} {index} {name} count is too large")))?;
    let end = begin
        .checked_add(count)
        .ok_or_else(|| invalid_moc3(format!("{entity} {index} {name} range overflows")))?;
    if end > source_len {
        return Err(invalid_moc3(format!(
            "{entity} {index} {name} range is outside section"
        )));
    }

    Ok(())
}

pub(super) fn invalid_moc3(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
