use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

pub const DHASH_BITS: u32 = 127;

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn dhash_from_bytes(bytes: &[u8]) -> Option<u128> {
    let img = image::load_from_memory(bytes).ok()?;
    let gray = img
        .resize_exact(9, 8, image::imageops::FilterType::Lanczos3)
        .to_luma8();

    let mut hash = 0u128;

    for y in 0..8 {
        for x in 0..8 {
            let left = gray.get_pixel(x, y)[0];
            let right = gray.get_pixel(x + 1, y)[0];
            hash <<= 1;
            if left > right {
                hash |= 1;
            }
        }
    }

    for x in 0..9 {
        for y in 0..7 {
            let top = gray.get_pixel(x, y)[0];
            let bottom = gray.get_pixel(x, y + 1)[0];
            hash <<= 1;
            if top > bottom {
                hash |= 1;
            }
        }
    }

    Some(hash)
}

pub fn dhash_from_file(path: &Path) -> Option<u128> {
    let bytes = std::fs::read(path).ok()?;
    dhash_from_bytes(&bytes)
}

pub fn hamming_distance(a: u128, b: u128) -> u32 {
    (a ^ b).count_ones()
}
