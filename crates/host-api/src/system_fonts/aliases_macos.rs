//! Resolve a stored face name to the family a stylesheet can match.
//!
//! 参考宿主在 DirectWrite 上做这件事，因为字体偏好里可能存的是字面名（`仓耳今楷05 W03` 而不是 `仓耳今楷05`），而样式表只认字体族、不认字面。macOS 上设置页的预览走 CSS，同样只认族名，所以这里把字面名或 PostScript 名换成它所属的族名再交给预览。候选窗不经过这里：`CandidateFontCache.mm` 只接受本机已激活的字体族名，存着字面名时画系统字体，和预览并不一致；字重由 `candidate_font_weight` 单独选择（#10748），不靠字面名。
//!
//! CoreText never fails a name lookup: an unknown name yields a substituted font rather than null. So
//! the answer is only used when the font that came back is the one that was asked for - by PostScript
//! name, by full name, or because the request was already a family. Anything else is a substitution,
//! and the stored name is left alone.

use std::ffi::{c_char, c_void, CStr};

const MAX_BYTES: usize = 128;
const UTF8: u32 = 0x0800_0100;

type CFRef = *const c_void;

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFRef, size: f64, matrix: *const c_void) -> CFRef;
    fn CTFontCopyFamilyName(font: CFRef) -> CFRef;
    fn CTFontCopyPostScriptName(font: CFRef) -> CFRef;
    fn CTFontCopyFullName(font: CFRef) -> CFRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        allocator: CFRef,
        bytes: *const u8,
        length: isize,
        encoding: u32,
        external: u8,
    ) -> CFRef;
    fn CFStringGetCString(string: CFRef, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFRelease(value: CFRef);
}

/// A Create/Copy-rule reference, released once.
struct Owned(CFRef);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a non-null reference obtained from a Create or Copy call.
        unsafe { CFRelease(self.0) };
    }
}

impl Owned {
    fn new(value: CFRef) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }
}

fn to_string(value: &Owned) -> Option<String> {
    let mut buffer = [0 as c_char; MAX_BYTES + 1];
    // SAFETY: the guard keeps the CFString alive and the buffer has the capacity passed in.
    // Names that do not fit - which are longer than anything this accepts anyway - fail the conversion.
    let converted =
        unsafe { CFStringGetCString(value.0, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) };
    if converted == 0 {
        return None;
    }
    // SAFETY: a successful conversion NUL-terminates within the buffer.
    let text = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().ok()?;
    (!text.is_empty() && super::is_bounded_text(text, 128)).then(|| text.to_owned())
}

fn cf_string(text: &str) -> Option<Owned> {
    // SAFETY: the bytes are borrowed for the duration of the call, which copies them.
    Owned::new(unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            text.as_ptr(),
            text.len() as isize,
            UTF8,
            0,
        )
    })
}

fn names_the_same(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

pub(super) fn resolve(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > MAX_BYTES || name.contains('\0') {
        return None;
    }
    let requested = cf_string(name)?;
    // SAFETY: the requested name stays alive across the call; a null matrix means the identity one.
    let font = Owned::new(unsafe { CTFontCreateWithName(requested.0, 12.0, std::ptr::null()) })?;
    // SAFETY for the three copies below: the font guard owns a live CTFont, and each Copy call
    // returns an owned CFString or null.
    let family =
        Owned::new(unsafe { CTFontCopyFamilyName(font.0) }).and_then(|value| to_string(&value))?;
    let postscript =
        Owned::new(unsafe { CTFontCopyPostScriptName(font.0) }).and_then(|value| to_string(&value));
    let full =
        Owned::new(unsafe { CTFontCopyFullName(font.0) }).and_then(|value| to_string(&value));
    let asked_for_this_font = names_the_same(&family, name)
        || postscript
            .as_deref()
            .is_some_and(|value| names_the_same(value, name))
        || full
            .as_deref()
            .is_some_and(|value| names_the_same(value, name));
    asked_for_this_font.then_some(family)
}
