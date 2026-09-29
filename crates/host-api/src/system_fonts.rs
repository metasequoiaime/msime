//! Installed family names only; never expose font files or paths to the webview.
#[cfg(target_os = "macos")]
mod aliases_macos;
#[cfg(windows)]
mod aliases_windows;

use msime_client_core::is_bounded_text;

/// Resolve display-only CSS names without changing stored font preferences.
///
/// A preference can hold a face name where a stylesheet needs a family, because the picker is not the
/// only way a name gets in there: a file written by an older build, a migrated profile, an edit by
/// hand. The hosts whose native text layer accepts both - DirectWrite and CoreText - render such a
/// name correctly in the candidate window while the settings preview beside it silently falls back,
/// so both ask their own font system what family the name belongs to. The remaining hosts have only
/// family names to begin with.
pub fn resolve_css_families(names: Vec<String>) -> Result<Vec<String>, &'static str> {
    if names.len() > 33
        || names
            .iter()
            .any(|name| name.is_empty() || !is_bounded_text(name, 128))
    {
        return Err("font_family");
    }
    #[cfg(windows)]
    return Ok(names
        .into_iter()
        .map(|name| aliases_windows::resolve(&name).unwrap_or(name))
        .collect());
    #[cfg(target_os = "macos")]
    return Ok(names
        .into_iter()
        .map(|name| aliases_macos::resolve(&name).unwrap_or(name))
        .collect());
    #[cfg(not(any(windows, target_os = "macos")))]
    Ok(names)
}

/// OpenHarmony reports `target_os = "linux"`, so every Linux gate in this file has to exclude it explicitly. It ships no fontconfig, and an extension ability cannot spawn `fc-list` from its sandbox, so claiming support here would offer the shared UI a font picker that only ever returns an error.
/// The HarmonyOS settings page does list installed families, but through ArkUI's own
/// `font.getSystemFontList()` rather than this module; alias resolution has no equivalent there.
pub fn supported() -> bool {
    cfg!(any(
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos")),
        windows
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn list() -> Result<Vec<String>, &'static str> {
    Err("unsupported")
}

#[cfg(windows)]
mod windows_catalog;

#[cfg(windows)]
pub fn list() -> Result<Vec<String>, &'static str> {
    windows_catalog::list()
}

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
pub fn list() -> Result<Vec<String>, &'static str> {
    linux_catalog::list()
}

#[cfg(all(target_os = "linux", target_env = "ohos"))]
pub fn list() -> Result<Vec<String>, &'static str> {
    Err("unsupported")
}

#[cfg(any(all(target_os = "linux", not(target_env = "ohos")), test))]
fn parse_catalog(output: &[u8], max_families: usize) -> Result<Vec<String>, &'static str> {
    use std::collections::BTreeSet;

    let output = std::str::from_utf8(output).map_err(|_| "font_catalog")?;
    let mut names = BTreeSet::new();
    for line in output.lines() {
        for family in line.split(',') {
            let family = family.trim();
            if !family.is_empty() && is_bounded_text(family, 128) {
                names.insert(family.to_owned());
            }
            if names.len() > max_families {
                return Err("font_catalog_limit");
            }
        }
    }
    let mut result = Vec::with_capacity(names.len());
    result.extend(names);
    Ok(result)
}

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux_catalog {
    use super::parse_catalog;
    use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
    use std::io::{ErrorKind, Read};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const MAX_FAMILIES: usize = 16_384;
    const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
    const CATALOG_TIMEOUT: Duration = Duration::from_secs(10);

    fn read_output(
        program: &str,
        arguments: &[&str],
        max_bytes: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>, &'static str> {
        let mut child = Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "font_catalog")?;
        let result = (|| {
            let mut output = child.stdout.take().ok_or("font_catalog")?;
            let flags = fcntl_getfl(&output).map_err(|_| "font_catalog")?;
            fcntl_setfl(&output, flags | OFlags::NONBLOCK).map_err(|_| "font_catalog")?;
            let deadline = Instant::now() + timeout;
            let mut bytes = Vec::with_capacity(max_bytes.min(8192));
            let mut buffer = [0; 8192];
            let mut eof = false;
            loop {
                if !eof {
                    let remaining = max_bytes
                        .saturating_add(1)
                        .saturating_sub(bytes.len())
                        .min(buffer.len());
                    if remaining == 0 {
                        return Err("font_catalog");
                    }
                    match output.read(&mut buffer[..remaining]) {
                        Ok(0) => eof = true,
                        Ok(count) => {
                            bytes.extend_from_slice(&buffer[..count]);
                            if bytes.len() > max_bytes {
                                return Err("font_catalog");
                            }
                            continue;
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                        Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => return Err("font_catalog"),
                    }
                }
                if eof {
                    if let Some(status) = child.try_wait().map_err(|_| "font_catalog")? {
                        return status.success().then_some(bytes).ok_or("font_catalog");
                    }
                }
                if Instant::now() >= deadline {
                    return Err("font_catalog");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })();
        if result.is_err() {
            let _ = child.kill();
        }
        let _ = child.wait();
        result
    }

    pub(super) fn list() -> Result<Vec<String>, &'static str> {
        let output = read_output(
            "fc-list",
            &[":", "family"],
            MAX_OUTPUT_BYTES,
            CATALOG_TIMEOUT,
        )?;
        parse_catalog(&output, MAX_FAMILIES)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn output_capture_is_bounded_and_times_out() {
            assert_eq!(
                read_output("/bin/sh", &["-c", "printf abc"], 3, Duration::from_secs(1)).unwrap(),
                b"abc"
            );
            assert!(
                read_output("/bin/sh", &["-c", "printf abcd"], 3, Duration::from_secs(1)).is_err()
            );
            assert!(
                read_output("/bin/sh", &["-c", "sleep 1"], 3, Duration::from_millis(20)).is_err()
            );
        }
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::parse_catalog;

    #[test]
    fn parses_bounded_utf8_families_in_sorted_order() {
        let names = parse_catalog("Zulu,别名\nAlpha\nAlpha\nBad\tName\n".as_bytes(), 3).unwrap();
        assert_eq!(names, ["Alpha", "Zulu", "别名"]);
        assert!(parse_catalog(b"A\nB\n", 1).is_err());
        assert!(parse_catalog(&[0xff], 3).is_err());
    }
}

#[cfg(target_os = "macos")]
pub fn list() -> Result<Vec<String>, &'static str> {
    macos::list()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::collections::BTreeSet;
    use std::ffi::{c_char, c_void, CStr};

    const MAX_FAMILIES: usize = 16_384;
    const MAX_BYTES: usize = 128;
    type CFRef = *const c_void;

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTFontManagerCopyAvailableFontFamilyNames() -> CFRef;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(array: CFRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFRef, index: isize) -> CFRef;
        fn CFStringGetCString(string: CFRef, buffer: *mut c_char, size: isize, encoding: u32)
            -> u8;
        fn CFRelease(value: CFRef);
    }

    struct OwnedArray(CFRef);
    impl Drop for OwnedArray {
        fn drop(&mut self) {
            // SAFETY: this guard exclusively owns the non-null Copy-rule array.
            unsafe { CFRelease(self.0) };
        }
    }

    fn insert(names: &mut BTreeSet<String>, name: &str) {
        if !name.is_empty() && name.len() <= MAX_BYTES {
            names.insert(name.to_owned());
        }
    }

    pub(super) fn list() -> Result<Vec<String>, &'static str> {
        // SAFETY: CoreText takes no arguments and returns an owned CFArray of CFStrings.
        let array = unsafe { CTFontManagerCopyAvailableFontFamilyNames() };
        if array.is_null() {
            return Err("font_catalog");
        }
        let array = OwnedArray(array);
        // SAFETY: the owned array remains alive throughout enumeration.
        let count = unsafe { CFArrayGetCount(array.0) };
        if count < 0 || count as usize > MAX_FAMILIES {
            return Err("font_catalog_limit");
        }
        let mut names = BTreeSet::new();
        for index in 0..count {
            // SAFETY: index is in bounds; the borrowed CFString is retained by array.
            let name = unsafe { CFArrayGetValueAtIndex(array.0, index) };
            if name.is_null() {
                continue;
            }
            let mut buffer = [0 as c_char; MAX_BYTES + 1];
            // SAFETY: buffer has the specified capacity; name is a live CFString.
            // kCFStringEncodingUTF8. Failed conversions (including long names) are skipped.
            let converted = unsafe {
                CFStringGetCString(name, buffer.as_mut_ptr(), buffer.len() as isize, 0x08000100)
            };
            if converted != 0 {
                // SAFETY: successful CFStringGetCString guarantees a terminating NUL.
                if let Ok(name) = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str() {
                    insert(&mut names, name);
                }
            }
        }
        let mut result = Vec::with_capacity(names.len());
        result.extend(names);
        Ok(result)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn bounds_unicode_and_deduplication() {
            let mut names = BTreeSet::new();
            insert(&mut names, "");
            insert(&mut names, &"a".repeat(129));
            insert(&mut names, &"字".repeat(43));
            assert!(names.is_empty());
            insert(&mut names, &"a".repeat(128));
            insert(&mut names, "示例字体");
            insert(&mut names, "示例字体");
            assert_eq!(names.len(), 2);
        }

        #[test]
        fn native_catalog_is_bounded_sorted_and_nonempty() {
            let names = list().expect("native catalog unavailable");
            // Do not print installed names, even on assertion failure.
            assert!(!names.is_empty());
            assert!(names.len() <= MAX_FAMILIES);
            assert!(names
                .iter()
                .all(|name| !name.is_empty() && name.len() <= MAX_BYTES));
            assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }
}
