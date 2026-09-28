//! DirectWrite family names match Chromium CSS; GDI is only a fallback.
use std::collections::BTreeSet;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::Graphics::{DirectWrite::*, Gdi::*};

const MAX_FAMILIES: usize = 16_384;

#[derive(Default)]
struct Catalog {
    names: BTreeSet<String>,
    overflow: bool,
}

impl Catalog {
    fn insert(&mut self, value: &[u16]) {
        let Ok(name) = String::from_utf16(value) else {
            return;
        };
        if !super::valid_font_name(&name) || name.starts_with('@') {
            return;
        }
        if self.names.len() == MAX_FAMILIES && !self.names.contains(&name) {
            self.overflow = true;
            return;
        }
        self.names.insert(name);
    }
}

fn directwrite(catalog: &mut Catalog) -> windows::core::Result<()> {
    // SAFETY: owned COM projections retain collections/names for each call;
    // all output buffers are sized from bounded lengths reported by DirectWrite.
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let mut collection = None;
        factory.GetSystemFontCollection(&mut collection, false)?;
        let Some(collection) = collection else {
            return Ok(());
        };
        let count = collection.GetFontFamilyCount();
        if count as usize > MAX_FAMILIES {
            catalog.overflow = true;
            return Ok(());
        }
        let mut locale = [0u16; 85];
        GetUserDefaultLocaleName(&mut locale);
        for family in 0..count {
            let Ok(names) = collection
                .GetFontFamily(family)
                .and_then(|f| f.GetFamilyNames())
            else {
                continue;
            };
            if names.GetCount() == 0 {
                continue;
            }
            let mut index = 0;
            for language in [PCWSTR(locale.as_ptr()), w!("zh-cn"), w!("en-us")] {
                let mut found = BOOL(0);
                if names
                    .FindLocaleName(language, &mut index, &mut found)
                    .is_ok()
                    && found.as_bool()
                {
                    break;
                }
                index = 0;
            }
            let Ok(length) = names.GetStringLength(index) else {
                continue;
            };
            if length > 128 {
                continue;
            }
            let mut buffer = vec![0; length as usize + 1];
            if names.GetString(index, &mut buffer).is_ok() {
                catalog.insert(&buffer[..length as usize]);
            }
        }
    }
    Ok(())
}

unsafe extern "system" fn collect(
    font: *const LOGFONTW,
    _: *const TEXTMETRICW,
    _: u32,
    data: LPARAM,
) -> i32 {
    if font.is_null() {
        return 1;
    }
    // SAFETY: EnumFontFamiliesExW calls synchronously with a live LOGFONTW and
    // the exclusive Catalog pointer supplied by list below.
    let (font, catalog) = unsafe { (&*font, &mut *(data.0 as *mut Catalog)) };
    let end = font
        .lfFaceName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(font.lfFaceName.len());
    catalog.insert(&font.lfFaceName[..end]);
    i32::from(!catalog.overflow)
}

pub(super) fn list() -> Result<Vec<String>, &'static str> {
    let mut catalog = Catalog::default();
    let _ = directwrite(&mut catalog);
    if catalog.names.is_empty() && !catalog.overflow {
        // SAFETY: screen DC is released after synchronous enumeration; callback
        // borrows the stack catalog only until enumeration returns.
        unsafe {
            let dc = GetDC(None);
            if dc.0.is_null() {
                return Err("font_catalog");
            }
            let probe = LOGFONTW {
                lfCharSet: DEFAULT_CHARSET,
                ..Default::default()
            };
            EnumFontFamiliesExW(
                dc,
                &probe,
                Some(collect),
                LPARAM(&mut catalog as *mut Catalog as isize),
                0,
            );
            ReleaseDC(None, dc);
        }
    }
    if catalog.overflow {
        return Err("font_catalog_limit");
    }
    if catalog.names.is_empty() {
        return Err("font_catalog");
    }
    Ok(catalog.names.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_bounded_deduplicated_and_validated() {
        let mut catalog = Catalog::default();
        for name in [
            "Synthetic",
            "Synthetic",
            "示例字体",
            "@Vertical",
            "",
            "bad\nname",
        ] {
            catalog.insert(&name.encode_utf16().collect::<Vec<_>>());
        }
        catalog.insert(&[0xd800]);
        catalog.insert(&[b'x' as u16; 129]);
        assert_eq!(
            catalog.names.into_iter().collect::<Vec<_>>(),
            ["Synthetic", "示例字体"]
        );
    }
}
