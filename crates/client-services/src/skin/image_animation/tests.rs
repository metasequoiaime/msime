use super::*;

fn gif(repeat: Option<image::codecs::gif::Repeat>, frames: usize, size: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).unwrap();
        }
        for at in 0..frames {
            encoder
                .encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(size, size, image::Rgba([at as u8, 20, 40, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(if at == 0 { 10 } else { 40 }, 1),
                ))
                .unwrap();
        }
    }
    bytes
}

#[test]
fn gif_repeat_is_complete_plays_and_short_delays_match_windows() {
    use image::codecs::gif::Repeat;
    for (repeat, plays) in [
        (None, 1),
        (Some(Repeat::Finite(2)), 3),
        (Some(Repeat::Infinite), 0),
    ] {
        let animation = decode(&gif(repeat, 2, 2)).unwrap();
        assert_eq!(animation.play_count, plays);
        assert_eq!(animation.delays_ms, [100, 40]);
        assert_eq!(animation.frames.len(), 2);
        assert_eq!(&animation.frames[1].rgba[..4], &[1, 20, 40, 255]);
    }
    assert_eq!(
        normalize_delay(image::Delay::from_numer_denom_ms(21, 2)),
        11
    );
    assert_eq!(
        normalize_delay(image::Delay::from_numer_denom_ms(20, 2)),
        100
    );
}

#[test]
fn apng_excludes_separate_default_image_and_composes_subframes() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(3, 2).unwrap();
        encoder.set_sep_def_img(true).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[0, 0, 255, 255].repeat(2))
            .unwrap();
        writer.set_frame_delay(1, 100).unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255].repeat(2))
            .unwrap();
        writer.set_frame_dimension(1, 1).unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.set_frame_delay(3, 100).unwrap();
        writer.set_blend_op(png::BlendOp::Source).unwrap();
        writer.set_dispose_op(png::DisposeOp::Previous).unwrap();
        writer.write_image_data(&[0, 255, 0, 255]).unwrap();
        writer.set_frame_position(0, 0).unwrap();
        writer.write_image_data(&[255, 255, 0, 255]).unwrap();
        writer.finish().unwrap();
    }
    let animation = decode(&bytes).unwrap();
    assert_eq!(animation.play_count, 2);
    assert_eq!(animation.delays_ms, [100, 30, 30]);
    assert_eq!(animation.frames[0].rgba, [255, 0, 0, 255].repeat(2));
    assert_eq!(animation.frames[1].rgba, [255, 0, 0, 255, 0, 255, 0, 255]);
    assert_eq!(animation.frames[2].rgba, [255, 255, 0, 255, 255, 0, 0, 255]);
    assert_eq!(
        image_frame::decode(&bytes).unwrap().rgba,
        [0, 0, 255, 255].repeat(2)
    );
}

#[test]
fn webp_uses_wic_timing_and_static_formats_stay_still() {
    use crate::skin::webp_preflight::tests::{animated, static_images};
    let images = static_images();
    let animation = decode(&animated(&images)).unwrap();
    assert_eq!(animation.play_count, 0);
    assert_eq!(animation.delays_ms, [100; 3]);
    assert_eq!(animation.frames[0].rgba.len(), 4 * 3 * 4);
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let image = image::RgbImage::from_pixel(2, 3, image::Rgb([20, 40, 60]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        let animation = decode(bytes.get_ref()).unwrap();
        assert_eq!(animation.frames.len(), 1);
        assert_eq!(animation.delays_ms, [0]);
        assert_eq!(animation.play_count, 1);
    }
}

#[test]
fn late_corruption_and_frame_and_pixel_budgets_reject_whole_animation() {
    let mut bytes = gif(None, 2, 2);
    bytes.truncate(bytes.len() - 4);
    assert!(decode(&bytes).is_err());
    assert!(decode(&gif(None, 121, 1)).is_err());
    assert!(decode(&gif(None, 2, 2001)).is_err());
    assert!(decode(b"broken").is_err());
    assert!(decode(&vec![0; MAX_RESOURCE_BYTES + 1]).is_err());
}
