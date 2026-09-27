//! Preserve the reference host's DirectWrite typographic-family resolution.
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Gdi::{DEFAULT_CHARSET, FW_NORMAL, LOGFONTW};

fn preferred_name(names: &IDWriteLocalizedStrings) -> Option<String> {
    // SAFETY: the COM projection retains strings; all indices are obtained
    // from this object, and the output buffer has the reported length + NUL.
    unsafe {
        if names.GetCount() == 0 {
            return None;
        }
        let mut locale = [0u16; 85];
        GetUserDefaultLocaleName(&mut locale);
        let mut index = 0;
        for language in [PCWSTR(locale.as_ptr()), w!("zh-cn"), w!("en-us")] {
            let mut exists = BOOL(0);
            if names
                .FindLocaleName(language, &mut index, &mut exists)
                .is_ok()
                && exists.as_bool()
            {
                break;
            }
            index = 0;
        }
        let length = names.GetStringLength(index).ok()? as usize;
        if length == 0 || length > 128 {
            return None;
        }
        let mut buffer = vec![0; length + 1];
        names.GetString(index, &mut buffer).ok()?;
        let value = String::from_utf16(&buffer[..length]).ok()?;
        super::valid_font_name(value).then_some(value)
    }
}

pub(super) fn resolve(name: &str) -> Option<String> {
    let encoded: Vec<_> = name.encode_utf16().collect();
    let mut probe = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        lfWeight: FW_NORMAL.0 as i32,
        ..Default::default()
    };
    if encoded.is_empty() || encoded.len() >= probe.lfFaceName.len() || encoded.contains(&0) {
        return None;
    }
    probe.lfFaceName[..encoded.len()].copy_from_slice(&encoded);
    // SAFETY: DirectWrite owns its returned interfaces. LOGFONTW is initialized,
    // NUL-terminated and remains live throughout the synchronous call.
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let font = factory
            .GetGdiInterop()
            .ok()?
            .CreateFontFromLOGFONT(&probe)
            .ok()?;
        let mut names = None;
        let mut exists = BOOL(0);
        if font
            .GetInformationalStrings(
                DWRITE_INFORMATIONAL_STRING_TYPOGRAPHIC_FAMILY_NAMES,
                &mut names,
                &mut exists,
            )
            .is_ok()
            && exists.as_bool()
        {
            if let Some(value) = names.as_ref().and_then(preferred_name) {
                return Some(value);
            }
        }
        preferred_name(&font.GetFontFamily().ok()?.GetFamilyNames().ok()?)
    }
}
