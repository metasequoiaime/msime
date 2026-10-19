use super::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

pub(in crate::skin) fn static_images() -> Vec<Vec<u8>> {
    let mut lossless = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        3,
        image::Rgba([70, 140, 210, 100]),
    ))
    .write_to(&mut lossless, image::ImageFormat::WebP)
    .unwrap();
    // Pillow 编码的同色合成图，分别覆盖有损图和 ALPH + VP8。
    vec![
        lossless.into_inner(),
        BASE64.decode("UklGRjgAAABXRUJQVlA4ICwAAADwAQCdASoEAAMAAUAmJaACdLoB+AAEyAAA/hMf+cSx1O7uz/+YJX5BuuQAAA==").unwrap(),
        BASE64.decode("UklGRlwAAABXRUJQVlA4WAoAAAAQAAAAAwAAAgAAQUxQSAoAAAABB1CyiAhERP8DVlA4ICwAAADwAQCdASoEAAMAAUAmJaACdLoB+AAEyAAA/hMf+cSx1O7uz/+YJX5BuuQAAA==").unwrap(),
    ]
}

fn chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
    let mut output = kind.to_vec();
    output.extend((body.len() as u32).to_le_bytes());
    output.extend(body);
    output.resize(output.len() + body.len() % 2, 0);
    output
}

fn container(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut output = b"RIFF".to_vec();
    output.extend((4 + chunks.iter().map(Vec::len).sum::<usize>() as u32).to_le_bytes());
    output.extend(b"WEBP");
    for chunk in chunks {
        output.extend(chunk);
    }
    output
}

pub(in crate::skin) fn animated(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut chunks = vec![
        chunk(b"VP8X", &[0x12, 0, 0, 0, 3, 0, 0, 2, 0, 0]),
        chunk(b"ANIM", &[0; 6]),
    ];
    for frame in frames {
        let mut body = vec![0; 16];
        body[6] = 3;
        body[9] = 2;
        body[12] = 70;
        body[15] = 2;
        for part in Chunks(&frame[12..]) {
            let (kind, bits) = part.unwrap();
            if matches!(kind, b"ALPH" | b"VP8 " | b"VP8L") {
                body.extend(chunk(kind, bits));
            }
        }
        chunks.push(chunk(b"ANMF", &body));
    }
    container(&chunks)
}

pub(in crate::skin) fn with_inner_dimensions(
    mut bytes: Vec<u8>,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let offset = bytes
        .windows(4)
        .position(|part| matches!(part, b"VP8 " | b"VP8L"))
        .unwrap();
    if &bytes[offset..offset + 4] == b"VP8L" {
        let bits = (u32::from(width) - 1) | ((u32::from(height) - 1) << 14) | (1 << 28);
        bytes[offset + 9..offset + 13].copy_from_slice(&bits.to_le_bytes());
    } else {
        bytes[offset + 14..offset + 16].copy_from_slice(&width.to_le_bytes());
        bytes[offset + 16..offset + 18].copy_from_slice(&height.to_le_bytes());
    }
    bytes
}

#[test]
fn all_frame_geometries_are_checked_before_any_pixel_decode() {
    let images = static_images();
    for bytes in &images {
        assert_eq!(check(bytes, (4, 3), 12), Ok(1));
        assert_eq!(check(bytes, (4, 3), 11), Err(Error::Limit));
        let large = with_inner_dimensions(bytes.clone(), 16383, 16383);
        assert_eq!(check(&animated(&[large]), (4, 3), 12), Err(Error::Limit));
        let mismatch = with_inner_dimensions(bytes.clone(), 5, 3);
        assert_eq!(
            check(&animated(&[mismatch]), (4, 3), 12),
            Err(Error::Invalid)
        );
    }
    assert_eq!(check(&animated(&images), (4, 3), 36), Ok(3));
    let late = with_inner_dimensions(images[1].clone(), 16383, 16383);
    assert_eq!(
        check(&animated(&[images[0].clone(), late]), (4, 3), 24),
        Err(Error::Limit)
    );
}

#[test]
fn frame_count_and_full_canvas_budget_are_preflighted() {
    let frame = static_images().remove(0);
    let bytes = animated(&vec![frame.clone(); MAX_ANIMATION_FRAMES as usize]);
    assert_eq!(check(&bytes, (4, 3), u64::MAX), Ok(MAX_ANIMATION_FRAMES));
    assert_eq!(
        check(&bytes, (4, 3), 12 * u64::from(MAX_ANIMATION_FRAMES) - 1),
        Err(Error::Limit)
    );
    let bytes = animated(&vec![frame; MAX_ANIMATION_FRAMES as usize + 1]);
    assert_eq!(check(&bytes, (4, 3), u64::MAX), Err(Error::Limit));
}

#[test]
fn alpha_cannot_hide_an_unchecked_bitstream_in_an_unknown_chunk() {
    let images = static_images();
    let mut bytes = animated(&[images[2].clone()]);
    let offset = bytes.windows(4).position(|part| part == b"VP8 ").unwrap();
    // ALPH 后第一块即使改成未知标签，解码器仍把正文作为 VP8 解码。
    bytes[offset..offset + 4].copy_from_slice(b"JUNK");
    assert_eq!(check(&bytes, (4, 3), 12), Err(Error::Invalid));
}

#[test]
fn malformed_chunk_lengths_and_duplicate_drawing_data_are_rejected() {
    let image = static_images().remove(0);
    let mut duplicate = image.clone();
    duplicate.extend(&image[12..]);
    let size = (duplicate.len() - 8) as u32;
    duplicate[4..8].copy_from_slice(&size.to_le_bytes());
    assert_eq!(check(&duplicate, (4, 3), 24), Err(Error::Invalid));
    for cut in [0, 8, 12, image.len() - 1] {
        assert_eq!(check(&image[..cut], (4, 3), 12), Err(Error::Invalid));
    }
    let mut missing_pad = container(&[chunk(b"JUNK", &[0])]);
    missing_pad.pop();
    let size = (missing_pad.len() - 8) as u32;
    missing_pad[4..8].copy_from_slice(&size.to_le_bytes());
    assert_eq!(check(&missing_pad, (4, 3), 12), Err(Error::Invalid));
    let huge_chunk = container(&[vec![b'V', b'P', b'8', b'L', 255, 255, 255, 255]]);
    assert_eq!(check(&huge_chunk, (4, 3), 12), Err(Error::Invalid));
}
