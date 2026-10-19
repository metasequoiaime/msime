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
