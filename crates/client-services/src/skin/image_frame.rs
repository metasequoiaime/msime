//! 把宿主已经安全读取的皮肤图片解码为有界的静态 RGBA 画布。

use super::catalog::{MAX_IMAGE_SIDE, MAX_PACKAGE_PIXELS, MAX_RESOURCE_BYTES};
use image::{ImageFormat, ImageReader};
use std::io::Cursor;

/// 非预乘的 RGBA8；每行恰好 `width * 4` 字节，不携带文件或解码器句柄。
pub struct SkinImageFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// PNG/APNG 取默认图，GIF/WebP 取第一幅完整画布；不播放动画，也不读取文件。
pub fn decode(bytes: &[u8]) -> Result<SkinImageFrame, &'static str> {
    const INVALID: &str = "invalid skin image";
    if bytes.is_empty() || bytes.len() > MAX_RESOURCE_BYTES {
        return Err(INVALID);
    }
    let format = image::guess_format(bytes).map_err(|_| INVALID)?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return Err(INVALID);
    }
    let reader = || {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_SIDE);
        limits.max_image_height = Some(MAX_IMAGE_SIDE);
        limits.max_alloc = Some(64 << 20);
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        reader.limits(limits);
        reader
    };
    let dimensions = reader().into_dimensions().map_err(|_| INVALID)?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > MAX_IMAGE_SIDE
        || dimensions.1 > MAX_IMAGE_SIDE
    {
        return Err(INVALID);
    }
    if format == ImageFormat::WebP {
        super::webp_preflight::check(bytes, dimensions, MAX_PACKAGE_PIXELS).map_err(|_| INVALID)?;
    }
    let rgba = reader().decode().map_err(|_| INVALID)?.into_rgba8();
    Ok(SkinImageFrame {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_formats_decode_to_the_canvas_and_preserve_alpha() {
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::WebP,
        ] {
            let image = image::RgbaImage::from_pixel(7, 5, image::Rgba([40, 100, 160, 128]));
            let mut bytes = Cursor::new(Vec::new());
            let image = image::DynamicImage::ImageRgba8(image);
            if format == ImageFormat::Jpeg {
                image.to_rgb8().write_to(&mut bytes, format).unwrap();
            } else {
                image.write_to(&mut bytes, format).unwrap();
            }
            let frame = decode(bytes.get_ref()).unwrap();
            assert_eq!((frame.width, frame.height, frame.rgba.len()), (7, 5, 140));
            if matches!(format, ImageFormat::Png | ImageFormat::WebP) {
                assert_eq!(&frame.rgba[..4], &[40, 100, 160, 128]);
            }
        }
    }

    #[test]
    fn gif_uses_first_frame_without_advancing() {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            for value in [40, 200] {
                encoder
                    .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                        3,
                        2,
                        image::Rgba([value, 10, 20, 255]),
                    )))
                    .unwrap();
            }
        }
        assert_eq!(&decode(&bytes).unwrap().rgba[..4], &[40, 10, 20, 255]);
    }

    #[test]
    fn apng_preserves_default_image_even_when_excluded_from_animation() {
        for separate in [false, true] {
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .set_animated(if separate { 1 } else { 2 }, 0)
                    .unwrap();
                encoder.set_sep_def_img(separate).unwrap();
                let mut writer = encoder.write_header().unwrap();
                for red in [40, 200] {
                    writer
                        .write_image_data(&[red, 10, 20, 128].repeat(2))
                        .unwrap();
                }
                writer.finish().unwrap();
            }
            assert_eq!(&decode(&bytes).unwrap().rgba[..4], &[40, 10, 20, 128]);
        }
    }

    #[test]
    fn webp_checks_later_frame_geometry_before_decoding_first_frame() {
        use crate::skin::webp_preflight::tests::{animated, static_images, with_inner_dimensions};
        let images = static_images();
        for bytes in &images {
            let frame = decode(bytes).unwrap();
            assert_eq!((frame.width, frame.height), (4, 3));
        }
        let bytes = animated(&images);
        assert_eq!(&decode(&bytes).unwrap().rgba[..4], &[70, 140, 210, 100]);
        let large = with_inner_dimensions(images[1].clone(), 16383, 16383);
        assert!(decode(&animated(&[images[0].clone(), large])).is_err());
    }

    #[test]
    fn invalid_unsupported_and_oversized_images_are_rejected() {
        assert!(decode(&[]).is_err());
        assert!(decode(b"broken").is_err());
        assert!(decode(b"BMunsupported bitmap").is_err());
        assert!(decode(&vec![0; MAX_RESOURCE_BYTES + 1]).is_err());
        let image = image::RgbImage::new(MAX_IMAGE_SIDE + 1, 1);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert!(decode(bytes.get_ref()).is_err());
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::new(2, 2)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let truncated = bytes.get_ref().len() / 2;
        bytes.get_mut().truncate(truncated);
        assert!(decode(bytes.get_ref()).is_err());
    }
}
