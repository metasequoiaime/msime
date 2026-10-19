//! 在 WebP 解码器分配位图之前核对容器、每帧位流尺寸和总像素预算。

use super::catalog::{MAX_ANIMATION_FRAMES, MAX_IMAGE_SIDE};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Invalid,
    Limit,
}

struct Chunks<'a>(&'a [u8]);

impl<'a> Iterator for Chunks<'a> {
    type Item = Result<(&'a [u8], &'a [u8]), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let data = std::mem::take(&mut self.0);
        let Some(header) = data.get(..8) else {
            return Some(Err(Error::Invalid));
        };
        let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let Some(end) = size.checked_add(8) else {
            return Some(Err(Error::Invalid));
        };
        let Some(padded) = end.checked_add(size % 2) else {
            return Some(Err(Error::Invalid));
        };
        let Some(rest) = data.get(padded..) else {
            return Some(Err(Error::Invalid));
        };
        self.0 = rest;
        Some(Ok((&header[..4], &data[8..end])))
    }
}

fn uint24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16
}

fn bitstream_dimensions(kind: &[u8], bytes: &[u8]) -> Result<(u32, u32), Error> {
    let dimensions = match kind {
        b"VP8 " if bytes.len() >= 10 && bytes[0] & 1 == 0 && bytes[3..6] == [0x9d, 1, 0x2a] => (
            u32::from(u16::from_le_bytes(bytes[6..8].try_into().unwrap()) & 0x3fff),
            u32::from(u16::from_le_bytes(bytes[8..10].try_into().unwrap()) & 0x3fff),
        ),
        b"VP8L" if bytes.len() >= 5 && bytes[0] == 0x2f => {
            let header = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            if header >> 29 != 0 {
                return Err(Error::Invalid);
            }
            ((header & 0x3fff) + 1, ((header >> 14) & 0x3fff) + 1)
        }
        _ => return Err(Error::Invalid),
    };
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(Error::Invalid);
    }
    if dimensions.0 > MAX_IMAGE_SIDE || dimensions.1 > MAX_IMAGE_SIDE {
        return Err(Error::Limit);
    }
    Ok(dimensions)
}

pub(super) fn check(
    bytes: &[u8],
    dimensions: (u32, u32),
    remaining_pixels: u64,
) -> Result<u32, Error> {
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(Error::Invalid);
    }
    if dimensions.0 > MAX_IMAGE_SIDE || dimensions.1 > MAX_IMAGE_SIDE {
        return Err(Error::Limit);
    }
    if bytes.len() < 12
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WEBP"
        || u64::from(u32::from_le_bytes(bytes[4..8].try_into().unwrap())) + 8 != bytes.len() as u64
    {
        return Err(Error::Invalid);
    }
    let frame_pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    let mut count = 0u32;
    let mut static_image = false;
    for chunk in Chunks(&bytes[12..]) {
        let (kind, body) = chunk?;
        match kind {
            b"ANMF" => {
                if body.len() < 16 || static_image {
                    return Err(Error::Invalid);
                }
                let frame = (uint24(&body[6..]) + 1, uint24(&body[9..]) + 1);
                if uint24(body) * 2 + frame.0 > dimensions.0
                    || uint24(&body[3..]) * 2 + frame.1 > dimensions.1
                {
                    return Err(Error::Invalid);
                }
                let mut alpha = false;
                let mut found = false;
                for subchunk in Chunks(&body[16..]) {
                    let (kind, bits) = subchunk?;
                    match kind {
                        b"ALPH" if !alpha && !found => alpha = true,
                        b"VP8 " | b"VP8L" if !found => {
                            let actual = bitstream_dimensions(kind, bits)?;
                            if actual != frame || (alpha && kind != b"VP8 ") {
                                return Err(Error::Invalid);
                            }
                            found = true;
                        }
                        // 解码器把 ALPH 后的块直接当成 VP8，不能跳过未知块再验后面的位流。
                        _ => return Err(Error::Invalid),
                    }
                }
                if !found {
                    return Err(Error::Invalid);
                }
                count += 1;
            }
            b"VP8 " | b"VP8L" => {
                if static_image || count != 0 || bitstream_dimensions(kind, body)? != dimensions {
                    return Err(Error::Invalid);
                }
                static_image = true;
                count = 1;
            }
            _ => {}
        }
        if count > MAX_ANIMATION_FRAMES
            || frame_pixels.saturating_mul(u64::from(count.max(1))) > remaining_pixels
        {
            return Err(Error::Limit);
        }
    }
    if count == 0 {
        return Err(Error::Invalid);
    }
    Ok(count)
}

#[cfg(test)]
#[path = "webp_preflight/tests.rs"]
pub(super) mod tests;
