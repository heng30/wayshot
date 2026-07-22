use crate::{DecodeError, types::DanmakuElem};

/// Decode a `DmSegMobileReply` protobuf message from raw bytes.
///
/// Wire format:
/// - field 1 (repeated DanmakuElem): tag = (1 << 3) | 2 = 0x0A, length-delimited
pub fn decode_dm_seg_mobile_reply(data: &[u8]) -> Result<Vec<DanmakuElem>, DecodeError> {
    let mut elems = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, next) = decode_varint(data, pos)?;
        pos = next;
        let wire_type = (tag & 0x07) as u8;
        let field_number = tag >> 3;

        match wire_type {
            0 => {
                // varint
                let (_, next) = decode_varint(data, pos)?;
                pos = next;
            }
            2 => {
                // length-delimited
                let (len, next) = decode_varint(data, pos)?;
                pos = next;
                let len = len as usize;
                if pos + len > data.len() {
                    return Err(DecodeError::UnexpectedEof);
                }
                let payload = &data[pos..pos + len];
                pos += len;

                if field_number == 1 {
                    // DanmakuElem
                    elems.push(decode_danmaku_elem(payload)?);
                }
            }
            1 => {
                // 64-bit fixed
                pos += 8;
            }
            5 => {
                // 32-bit fixed
                pos += 4;
            }
            _ => return Err(DecodeError::UnknownWireType(wire_type)),
        }
    }

    Ok(elems)
}

/// Decode a single `DanmakuElem` protobuf message.
///
/// Field mapping (from bilibili dm.proto):
///   1: int64  id
///   2: int32  progress
///   3: int32  mode
///   4: int32  fontsize
///   5: uint32 color
///   6: string midHash
///   7: string content
///   8: int64  ctime
///   9: string action
///  10: int32  pool
///  11: string idStr
///  12: int32  attr
///  13: int32  weight
///  22: string animation
///  24: int32  colorful (enum)
fn decode_danmaku_elem(data: &[u8]) -> Result<DanmakuElem, DecodeError> {
    let mut elem = DanmakuElem::default();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, next) = decode_varint(data, pos)?;
        pos = next;
        let wire_type = (tag & 0x07) as u8;
        let field_number = tag >> 3;

        match wire_type {
            0 => {
                // varint
                let (value, next) = decode_varint(data, pos)?;
                pos = next;
                match field_number {
                    1 => elem.id = value as i64,
                    2 => elem.progress = value as i32,
                    3 => elem.mode = value as i32,
                    4 => elem.fontsize = value as i32,
                    5 => elem.color = value as u32,
                    8 => elem.ctime = value as i64,
                    10 => elem.pool = value as i32,
                    12 => elem.attr = value as i32,
                    13 => elem.weight = value as i32,
                    24 => elem.colorful = value as i32,
                    _ => {}
                }
            }
            2 => {
                // length-delimited
                let (len, next) = decode_varint(data, pos)?;
                pos = next;
                let len = len as usize;
                if pos + len > data.len() {
                    return Err(DecodeError::UnexpectedEof);
                }
                let payload = &data[pos..pos + len];
                pos += len;

                match field_number {
                    6 => elem.mid_hash = String::from_utf8_lossy(payload).into_owned(),
                    7 => elem.content = String::from_utf8_lossy(payload).into_owned(),
                    9 => elem.action = String::from_utf8_lossy(payload).into_owned(),
                    11 => elem.id_str = String::from_utf8_lossy(payload).into_owned(),
                    22 => elem.animation = Some(String::from_utf8_lossy(payload).into_owned()),
                    _ => {}
                }
            }
            1 => {
                // 64-bit fixed
                pos += 8;
            }
            5 => {
                // 32-bit fixed
                pos += 4;
            }
            _ => return Err(DecodeError::UnknownWireType(wire_type)),
        }
    }

    Ok(elem)
}

/// Decode a varint from the buffer, returning (value, new_position).
fn decode_varint(data: &[u8], mut pos: usize) -> Result<(u64, usize), DecodeError> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        if pos >= data.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        let byte = data[pos];
        pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, pos));
        }
        shift += 7;
        if shift >= 64 {
            return Err(DecodeError::VarintTooLong);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_varint_simple() {
        let data = [0x01];
        let (val, pos) = decode_varint(&data, 0).unwrap();
        assert_eq!(val, 1);
        assert_eq!(pos, 1);
    }

    #[test]
    fn decode_varint_multi_byte() {
        // 300 = 0x12C = binary 1_0010_1100
        // encoded as: 0xAC 0x02
        let data = [0xAC, 0x02];
        let (val, pos) = decode_varint(&data, 0).unwrap();
        assert_eq!(val, 300);
        assert_eq!(pos, 2);
    }

    #[test]
    fn decode_empty_reply() {
        let data: &[u8] = &[];
        let elems = decode_dm_seg_mobile_reply(data).unwrap();
        assert!(elems.is_empty());
    }

    #[test]
    fn decode_single_danmaku_elem() {
        // Build a minimal DmSegMobileReply containing one DanmakuElem
        // DanmakuElem with: id=123, progress=5000, mode=1, fontsize=25, color=0xFFFFFF,
        //                   midHash="abc", content="hello", ctime=1700000000
        let mut elem_buf = Vec::new();

        // field 1 (id=123): tag=0x08, varint 123
        elem_buf.push(0x08);
        elem_buf.push(123);
        // field 2 (progress=5000): tag=0x10, varint 5000
        elem_buf.push(0x10);
        encode_varint(&mut elem_buf, 5000);
        // field 3 (mode=1): tag=0x18, varint 1
        elem_buf.push(0x18);
        elem_buf.push(1);
        // field 7 (content="hello"): tag=0x3A, len=5, "hello"
        elem_buf.push(0x3A);
        elem_buf.push(5);
        elem_buf.extend_from_slice(b"hello");

        // Wrap in DmSegMobileReply: field 1, length-delimited
        let mut reply_buf = Vec::new();
        reply_buf.push(0x0A); // tag for field 1, wire type 2
        encode_varint(&mut reply_buf, elem_buf.len() as u64);
        reply_buf.extend_from_slice(&elem_buf);

        let elems = decode_dm_seg_mobile_reply(&reply_buf).unwrap();
        assert_eq!(elems.len(), 1);
        assert_eq!(elems[0].id, 123);
        assert_eq!(elems[0].progress, 5000);
        assert_eq!(elems[0].mode, 1);
        assert_eq!(elems[0].content, "hello");
    }

    fn encode_varint(buf: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                buf.push(byte);
                break;
            }
            buf.push(byte | 0x80);
        }
    }
}
