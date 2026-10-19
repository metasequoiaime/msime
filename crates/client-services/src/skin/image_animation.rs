//! 有界的完整画布帧与 Windows 皮肤图片播放时序，不读取文件或持有解码器句柄。

use super::catalog::{
    MAX_ANIMATION_FRAMES, MAX_IMAGE_SIDE, MAX_PACKAGE_PIXELS, MAX_RESOURCE_BYTES,
};
use super::image_frame::{self, SkinImageFrame};
use image::{AnimationDecoder, ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

const INVALID: &str = "invalid skin animation";

pub struct SkinImageAnimation {
    pub frames: Vec<SkinImageFrame>,
    pub delays_ms: Vec<u32>,
    /// 完整播放次数；0 为无限循环，有限播放结束后保持末帧。
    pub play_count: u32,
}

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(u64::from(MAX_IMAGE_SIDE).pow(2) * 4 * 3);
    limits
}

fn play_count(value: image::metadata::LoopCount) -> u32 {
    match value {
        image::metadata::LoopCount::Infinite => 0,
        image::metadata::LoopCount::Finite(count) => count.get(),
    }
}

fn normalize_delay(delay: image::Delay) -> u32 {
    let (numerator, denominator) = delay.numer_denom_ms();
    let milliseconds =
        (u64::from(numerator) + u64::from(denominator) / 2) / u64::from(denominator.max(1));
    if milliseconds <= 10 {
        100
    } else {
        milliseconds as u32
    }
}

fn still(bytes: &[u8]) -> Result<SkinImageAnimation, &'static str> {
    Ok(SkinImageAnimation {
        frames: vec![image_frame::decode(bytes)?],
        delays_ms: vec![0],
        play_count: 1,
    })
}

// image 的 GIF LoopCount 不区分“没有扩展”和“无限循环”，且有限值是重复次数。
// 只按 GIF 块边界读取元数据，不能在压缩像素里搜索 NETSCAPE 字串。
fn gif_play_count(bytes: &[u8]) -> Result<u32, &'static str> {
    fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8], &'static str> {
        let (head, rest) = bytes.split_at_checked(length).ok_or(INVALID)?;
        *bytes = rest;
        Ok(head)
    }
    fn block<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], &'static str> {
        let length = usize::from(take(bytes, 1)?[0]);
        take(bytes, length)
    }
    fn palette(bytes: &mut &[u8], packed: u8) -> Result<(), &'static str> {
        if packed & 0x80 != 0 {
            take(bytes, 3 * (2_usize << (packed & 7)))?;
        }
        Ok(())
    }
    let mut bytes = bytes;
    let header = take(&mut bytes, 13)?;
    palette(&mut bytes, header[10])?;
    let mut result = 1;
    loop {
        match take(&mut bytes, 1)?[0] {
            0x3b => return Ok(result),
            0x2c => {
                let descriptor = take(&mut bytes, 9)?;
                palette(&mut bytes, descriptor[8])?;
                take(&mut bytes, 1)?;
                while !block(&mut bytes)?.is_empty() {}
            }
            0x21 => {
                let extension = take(&mut bytes, 1)?[0];
                let application = block(&mut bytes)?;
                if application.is_empty() {
                    continue;
                }
                let repeat =
                    extension == 0xff && matches!(application, b"NETSCAPE2.0" | b"ANIMEXTS1.0");
                let mut first = true;
                loop {
                    let data = block(&mut bytes)?;
                    if data.is_empty() {
                        break;
                    }
                    if repeat && first && data.len() >= 3 && data[0] == 1 {
                        let count = u32::from(u16::from_le_bytes([data[1], data[2]]));
                        result = if count == 0 { 0 } else { count + 1 };
                    }
                    first = false;
                }
            }
            _ => return Err(INVALID),
        }
    }
}

fn collect(
    frames: image::Frames<'_>,
    dimensions: (u32, u32),
    count: u32,
    webp: bool,
) -> Result<SkinImageAnimation, &'static str> {
    let mut result = SkinImageAnimation {
        frames: Vec::new(),
        delays_ms: Vec::new(),
        play_count: count,
    };
    let pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    for frame in frames {
        if result.frames.len() >= MAX_ANIMATION_FRAMES as usize
            || pixels * (result.frames.len() as u64 + 1) > MAX_PACKAGE_PIXELS
        {
            return Err(INVALID);
        }
        let frame = frame.map_err(|_| INVALID)?;
        if frame.buffer().dimensions() != dimensions {
            return Err(INVALID);
        }
        // 与 Windows WIC 多帧回退一致：WebP 没有可用的 WIC 帧时序，固定 100ms。
        let delay = if webp {
            100
        } else {
            normalize_delay(frame.delay())
        };
        result.frames.push(SkinImageFrame {
            width: dimensions.0,
            height: dimensions.1,
            rgba: frame.into_buffer().into_raw(),
        });
        result.delays_ms.push(delay);
    }
    if result.frames.is_empty() {
        return Err(INVALID);
    }
    Ok(result)
}

/// 一次验证并解码整张图片；错误时不返回部分动画。静态 API 的 APNG 默认图语义保持不变。
pub fn decode(bytes: &[u8]) -> Result<SkinImageAnimation, &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_RESOURCE_BYTES {
        return Err(INVALID);
    }
    let format = image::guess_format(bytes).map_err(|_| INVALID)?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Gif | ImageFormat::WebP | ImageFormat::Jpeg
    ) {
        return Err(INVALID);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits());
    let dimensions = reader.into_dimensions().map_err(|_| INVALID)?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > MAX_IMAGE_SIDE
        || dimensions.1 > MAX_IMAGE_SIDE
    {
        return Err(INVALID);
    }
    match format {
        ImageFormat::Png => {
            let (_, count) = super::catalog::png_frames(bytes)?;
            if u64::from(dimensions.0) * u64::from(dimensions.1) * u64::from(count)
                > MAX_PACKAGE_PIXELS
            {
                return Err(INVALID);
            }
            let decoder = image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits())
                .map_err(|_| INVALID)?;
            if !decoder.is_apng().map_err(|_| INVALID)? {
                return still(bytes);
            }
            let decoder = decoder.apng().map_err(|_| INVALID)?;
            let count = play_count(decoder.loop_count());
            collect(decoder.into_frames(), dimensions, count, false)
        }
        ImageFormat::Gif => {
            let count = gif_play_count(bytes)?;
            let mut decoder =
                image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
            decoder.set_limits(limits()).map_err(|_| INVALID)?;
            collect(decoder.into_frames(), dimensions, count, false)
        }
        ImageFormat::WebP => {
            super::webp_preflight::check(bytes, dimensions, MAX_PACKAGE_PIXELS)
                .map_err(|_| INVALID)?;
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
            decoder.set_limits(limits()).map_err(|_| INVALID)?;
            if !decoder.has_animation() {
                return still(bytes);
            }
            collect(decoder.into_frames(), dimensions, 0, true)
        }
        _ => still(bytes),
    }
}

#[cfg(test)]
mod tests;
