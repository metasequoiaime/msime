//! Files the settings page exports, written by the host into the user's Downloads folder.
//!
//! The page builds the document and names it; the host decides the folder. The WKWebView behind the macOS settings window cancels every download it has no handler for, so a download link there writes nothing, and the Windows source's outcome - the file lands in Downloads - has to be produced here instead.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Longest file name, in bytes, that APFS, ext4 and NTFS all accept.
const NAME_MAX_BYTES: usize = 255;
/// The page exports at most a million rows of one dictionary; this is well above that document and still refuses a runaway one.
const CONTENTS_MAX_BYTES: usize = 64 * 1024 * 1024;
/// How many " (n)" variants are tried before giving up, as a browser would after the same number of repeats.
const MAX_DUPLICATES: u32 = 999;

/// Check a page-supplied file name and return it trimmed.
///
/// The name is a single path component: separators, NUL and other control characters, and a leading dot (hidden files, `.` and `..`) are refused rather than rewritten, so the file lands only in the folder the host chose and only under the name the page asked for, or a numbered variant of it. `:` is refused as well, because the Finder shows it as `/`.
pub(crate) fn sanitize_name(name: &str) -> Result<&str, &'static str> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > NAME_MAX_BYTES
        || name.starts_with('.')
        || name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':'))
        || msime_client_core::has_disallowed_control_with_options(name, false)
    {
        return Err("export_name");
    }
    Ok(name)
}

/// The name a browser would give the `index`-th copy: `name.txt`, then `name (2).txt`, `name (3).txt`.
fn numbered_name(name: &str, index: u32) -> String {
    if index <= 1 {
        return name.to_owned();
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{} ({index}){}", &name[..dot], &name[dot..]),
        _ => format!("{name} ({index})"),
    }
}

/// Write `contents` into `directory` under `name`, or under the first free numbered variant of it, and return the absolute path written.
///
/// An existing file is never replaced: each candidate is created with `create_new`, so a name taken between the check and the write moves on to the next number instead of overwriting.
pub(crate) fn save(directory: &Path, name: &str, contents: &str) -> Result<PathBuf, &'static str> {
    let name = sanitize_name(name)?;
    if contents.len() > CONTENTS_MAX_BYTES {
        return Err("export_too_large");
    }
    super::atomic_file::create_directory_and_check(directory).map_err(|_| "storage")?;
    let directory = std::path::absolute(directory).map_err(|_| "storage")?;
    for index in 1..=MAX_DUPLICATES {
        let candidate = numbered_name(name, index);
        if candidate.len() > NAME_MAX_BYTES {
            return Err("export_name");
        }
        let path = directory.join(candidate);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("storage"),
        };
        if file
            .write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .is_err()
        {
            drop(file);
            // A half-written export reads as a complete one with rows missing, so it is removed rather than left behind.
            let _ = std::fs::remove_file(&path);
            return Err("storage");
        }
        return Ok(path);
    }
    Err("export_name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_one_component_and_trimmed() {
        assert_eq!(sanitize_name("  水杉用户词库.txt "), Ok("水杉用户词库.txt"));
        assert_eq!(
            sanitize_name("水杉IME-拼音用户词库.txt"),
            Ok("水杉IME-拼音用户词库.txt")
        );
        for refused in [
            "",
            "   ",
            ".",
            "..",
            ".hidden.txt",
            "../escape.txt",
            "nested/name.txt",
            "nested\\name.txt",
            "colon:name.txt",
            "line\nbreak.txt",
            "nul\0.txt",
        ] {
            assert_eq!(sanitize_name(refused), Err("export_name"), "{refused:?}");
        }
        assert_eq!(
            sanitize_name(&"a".repeat(NAME_MAX_BYTES + 1)),
            Err("export_name")
        );
    }

    #[test]
    fn numbered_names_follow_the_browser_pattern() {
        assert_eq!(numbered_name("words.txt", 1), "words.txt");
        assert_eq!(numbered_name("words.txt", 2), "words (2).txt");
        assert_eq!(numbered_name("words.backup.txt", 3), "words.backup (3).txt");
        assert_eq!(numbered_name("words", 2), "words (2)");
    }

    #[test]
    fn a_taken_name_gets_the_next_free_number_and_nothing_is_overwritten() {
        let state = tempfile::tempdir().unwrap();
        let downloads = state.path().join("Downloads");
        let first = save(&downloads, "words.txt", "first\n").unwrap();
        assert!(first.is_absolute());
        assert_eq!(
            first,
            std::path::absolute(downloads.join("words.txt")).unwrap()
        );
        std::fs::write(downloads.join("words (2).txt"), "someone else's\n").unwrap();
        let third = save(&downloads, "words.txt", "\u{feff}third\n").unwrap();
        assert_eq!(
            third,
            std::path::absolute(downloads.join("words (3).txt")).unwrap()
        );
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "first\n");
        assert_eq!(
            std::fs::read_to_string(downloads.join("words (2).txt")).unwrap(),
            "someone else's\n"
        );
        assert_eq!(std::fs::read_to_string(&third).unwrap(), "\u{feff}third\n");
    }

    #[test]
    fn a_refused_name_writes_nothing() {
        let state = tempfile::tempdir().unwrap();
        assert_eq!(save(state.path(), "../words.txt", "x"), Err("export_name"));
        assert_eq!(std::fs::read_dir(state.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_downloads_directory_without_writing_outside_it() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        symlink(outside.path(), &downloads).unwrap();

        assert_eq!(save(&downloads, "words.txt", "synthetic\n"), Err("storage"));
        assert!(!outside.path().join("words.txt").exists());
    }
}
