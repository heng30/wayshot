use crate::{Error, Result};

use super::{Moc3Header, Moc3SectionOffsets, parse::read_f32};

const CANVAS_INFO_SIZE: usize = 64;
const F32_SIZE: usize = 4;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Moc3CanvasInfo {
    pixels_per_unit: f32,
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
    flags: u8,
}

impl Moc3CanvasInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let header = Moc3Header::parse(bytes)?;
        let offsets = Moc3SectionOffsets::parse(bytes)?;
        let offset = usize::try_from(offsets.canvas_info_offset())
            .map_err(|_| invalid_canvas("canvas info offset does not fit in platform usize"))?;

        if bytes.len().saturating_sub(offset) < CANVAS_INFO_SIZE {
            return Err(invalid_canvas("canvas info is incomplete"));
        }

        Ok(Self {
            pixels_per_unit: read_f32(bytes, offset, header.endianness()),
            origin_x: read_f32(bytes, offset + F32_SIZE, header.endianness()),
            origin_y: read_f32(bytes, offset + F32_SIZE * 2, header.endianness()),
            width: read_f32(bytes, offset + F32_SIZE * 3, header.endianness()),
            height: read_f32(bytes, offset + F32_SIZE * 4, header.endianness()),
            flags: bytes[offset + F32_SIZE * 5],
        })
    }

    pub fn pixels_per_unit(&self) -> f32 {
        self.pixels_per_unit
    }

    pub fn origin_x(&self) -> f32 {
        self.origin_x
    }

    pub fn origin_y(&self) -> f32 {
        self.origin_y
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn reverse_y_coordinate(&self) -> bool {
        self.flags & 1 == 1
    }
}

fn invalid_canvas(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
