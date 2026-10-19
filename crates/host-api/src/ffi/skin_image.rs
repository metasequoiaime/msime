//! 皮肤像素在同步回调期间借用，不分配跨 ABI 所有权的缓冲区。

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub type SkinImageCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32) -> i32;

/// 解码一张静态皮肤画布；拒绝或 panic 都返回 0。
/// # Safety
/// `bytes` 指向 `length` 个可读字节，回调与 `context` 在整个调用期间有效。
/// 回调不得抛异常，不得在返回后使用借用的像素；空输入或空回调被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_image_decode(
    bytes: *const u8,
    length: usize,
    on_frame: Option<SkinImageCallback>,
    context: *mut c_void,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(callback) = on_frame else { return 0 };
        if bytes.is_null()
            || length == 0
            || length > msime_client_services::skin::catalog::MAX_RESOURCE_BYTES
        {
            return 0;
        }
        // 安全：调用者保证输入可读，上面先检查长度，不让解码器读取无界切片。
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let Ok(frame) = msime_client_services::skin::image_frame::decode(bytes) else {
            return 0;
        };
        // 安全：借用像素在同步回调返回前保持有效；回调契约由宿主保证。
        i32::from(
            unsafe {
                callback(
                    context,
                    frame.rgba.as_ptr(),
                    frame.rgba.len(),
                    frame.width,
                    frame.height,
                )
            } == 1,
        )
    }))
    .unwrap_or(0)
}

pub type SkinAnimationCallback =
    unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32, u32, u32, u32, u32) -> i32;

/// 整张图片验证后按序同步发布完整画布帧；回调拒绝或 panic 返回 0。
/// # Safety
/// 输入、回调和 context 的有效性及像素借用期限与静态接口一致。
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_animation_decode(
    bytes: *const u8,
    length: usize,
    on_frame: Option<SkinAnimationCallback>,
    context: *mut c_void,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(callback) = on_frame else { return 0 };
        if bytes.is_null()
            || length == 0
            || length > msime_client_services::skin::catalog::MAX_RESOURCE_BYTES
        {
            return 0;
        }
        // 安全：调用者提供可读输入，长度已限制，解码失败不会发布部分帧。
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let Ok(animation) = msime_client_services::skin::image_animation::decode(bytes) else {
            return 0;
        };
        for (index, frame) in animation.frames.iter().enumerate() {
            // 安全：像素在同步回调期间有效，宿主必须在回调内复制。
            let accepted = unsafe {
                callback(
                    context,
                    frame.rgba.as_ptr(),
                    frame.rgba.len(),
                    frame.width,
                    frame.height,
                    index as u32,
                    animation.frames.len() as u32,
                    animation.delays_ms[index],
                    animation.play_count,
                )
            };
            if accepted != 1 {
                return 0;
            }
        }
        1
    }))
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Received {
        calls: usize,
        size: (u32, u32),
        pixels: Vec<u8>,
    }

    unsafe extern "C" fn accept(
        context: *mut c_void,
        rgba: *const u8,
        length: usize,
        width: u32,
        height: u32,
    ) -> i32 {
        let received = unsafe { &mut *context.cast::<Received>() };
        received.calls += 1;
        received.size = (width, height);
        received.pixels = unsafe { std::slice::from_raw_parts(rgba, length) }.to_vec();
        1
    }

    unsafe extern "C" fn reject(_: *mut c_void, _: *const u8, _: usize, _: u32, _: u32) -> i32 {
        0
    }

    #[test]
    fn animation_validates_before_callbacks_and_cancellation_discards_partial_result() {
        // 两帧 1x1 红蓝 GIF，压缩数据和配置均为合成值。
        let mut gif = b"GIF89a".to_vec();
        gif.extend([1, 0, 1, 0, 128, 0, 0, 255, 0, 0, 0, 0, 255]);
        for bits in [0x44, 0x4c] {
            gif.extend([0x21, 0xf9, 4, 4, 1, 0, 0, 0]);
            gif.extend([0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, bits, 1, 0]);
        }
        gif.push(0x3b);
        #[derive(Default)]
        struct Frames {
            pixels: Vec<Vec<u8>>,
            reject: bool,
        }
        unsafe extern "C" fn frame(
            context: *mut c_void,
            rgba: *const u8,
            length: usize,
            width: u32,
            height: u32,
            index: u32,
            count: u32,
            delay: u32,
            plays: u32,
        ) -> i32 {
            let value = unsafe { &mut *context.cast::<Frames>() };
            assert_eq!((width, height, count, delay, plays), (1, 1, 2, 100, 1));
            assert_eq!(index as usize, value.pixels.len());
            value
                .pixels
                .push(unsafe { std::slice::from_raw_parts(rgba, length) }.to_vec());
            i32::from(!value.reject)
        }
        let mut received = Frames::default();
        unsafe {
            let context = (&mut received as *mut Frames).cast();
            assert_eq!(
                msime_client_skin_animation_decode(gif.as_ptr(), gif.len(), Some(frame), context),
                1
            );
            assert_eq!(
                received.pixels,
                [vec![255, 0, 0, 255], vec![0, 0, 255, 255]]
            );
            received.pixels.clear();
            assert_eq!(
                msime_client_skin_animation_decode(
                    gif.as_ptr(),
                    gif.len() - 4,
                    Some(frame),
                    context
                ),
                0
            );
            assert!(received.pixels.is_empty());
            assert_eq!(
                msime_client_skin_animation_decode(
                    std::ptr::null(),
                    gif.len(),
                    Some(frame),
                    context
                ),
                0
            );
            assert_eq!(
                msime_client_skin_animation_decode(gif.as_ptr(), usize::MAX, Some(frame), context),
                0
            );
            assert_eq!(
                msime_client_skin_animation_decode(gif.as_ptr(), gif.len(), None, context),
                0
            );
            received.reject = true;
            assert_eq!(
                msime_client_skin_animation_decode(gif.as_ptr(), gif.len(), Some(frame), context),
                0
            );
            assert_eq!(received.pixels.len(), 1);
            let mut single = Received::default();
            assert_eq!(
                msime_client_skin_image_decode(
                    gif.as_ptr(),
                    gif.len(),
                    Some(accept),
                    (&mut single as *mut Received).cast()
                ),
                1
            );
            assert_eq!(single.calls, 1);
        }
    }

    #[test]
    fn callback_is_synchronous_and_failure_never_publishes_pixels() {
        // 合成的 1x1 黑色 PNG，与用户资源无关。
        use base64::Engine;
        let png = base64::engine::general_purpose::STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNgYGAAAAAEAAH2FzhVAAAAAElFTkSuQmCC"
        ).unwrap();
        let mut received = Received::default();
        let context = (&mut received as *mut Received).cast();
        unsafe {
            assert_eq!(
                msime_client_skin_image_decode(png.as_ptr(), png.len(), Some(accept), context),
                1
            );
            assert_eq!(received.calls, 1);
            assert_eq!(received.size, (1, 1));
            assert_eq!(received.pixels, [0, 0, 0, 255]);
            assert_eq!(
                msime_client_skin_image_decode(png.as_ptr(), png.len(), Some(reject), context),
                0
            );
            assert_eq!(
                msime_client_skin_image_decode(png.as_ptr(), png.len(), None, context),
                0
            );
            assert_eq!(
                msime_client_skin_image_decode(std::ptr::null(), 4, Some(accept), context),
                0
            );
            assert_eq!(
                msime_client_skin_image_decode(png.as_ptr(), usize::MAX, Some(accept), context),
                0
            );
            assert_eq!(
                msime_client_skin_image_decode(b"broken".as_ptr(), 6, Some(accept), context),
                0
            );
        }
        assert_eq!(received.calls, 1);
    }
}
