//! Python string semantics the source formats were written against. The artifacts must stay row-identical to the ones the Python pipeline shipped, so line splitting, whitespace and letter tests follow `str.splitlines`, `str.strip`, `str.split()` and `str.isalpha` rather than their nearest Rust equivalents, which differ on a handful of control and combining characters.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use unicode_general_category::{get_general_category, GeneralCategory};

/// `str.isspace`: Unicode `White_Space` plus the four ASCII information separators.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`.
pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `str.strip(chars)`.
pub fn strip_chars<'a>(text: &'a str, chars: &str) -> &'a str {
    text.trim_matches(|c| chars.contains(c))
}

/// `str.split()` without arguments.
pub fn split_whitespace(text: &str) -> impl Iterator<Item = &str> {
    text.split(is_space).filter(|part| !part.is_empty())
}

/// `str.isalpha()`: non-empty and every character in a letter category.
pub fn is_alpha(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| {
            matches!(
                get_general_category(c),
                GeneralCategory::UppercaseLetter
                    | GeneralCategory::LowercaseLetter
                    | GeneralCategory::TitlecaseLetter
                    | GeneralCategory::ModifierLetter
                    | GeneralCategory::OtherLetter
            )
        })
}

/// `str.splitlines()`: every Unicode line boundary Python recognises, with `\r\n` as one.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        let boundary = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !boundary {
            continue;
        }
        lines.push(&text[start..index]);
        let mut end = index + c.len_utf8();
        if c == '\r' {
            if let Some(&(next, '\n')) = chars.peek() {
                chars.next();
                end = next + 1;
            }
        }
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Iterating a file opened in text mode: universal newlines (`\r\n`, `\r`, `\n`), terminators removed.
pub fn universal_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(&text[start..index]);
                start = index + 1;
            }
            b'\r' => {
                lines.push(&text[start..index]);
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Reads a UTF-8 file strictly, as `open(encoding="utf-8")` does. A byte-order mark is kept: only the readers the Python pipeline opened with `utf-8-sig` drop it, through [`without_bom`].
pub fn read(path: &Path) -> Result<String> {
    let mut file = open_source(path).with_context(|| format!("opening {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("reading {}", path.display()))?;
    String::from_utf8(bytes).with_context(|| format!("{} is not UTF-8", path.display()))
}

fn open_source(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "dictionary source is not a regular file",
        ));
    }
    Ok(file)
}

pub fn without_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn read_rejects_a_symlinked_source() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.txt");
        std::fs::write(&target, "synthetic outside source\n").unwrap();
        let linked = checkout.path().join("source.txt");
        symlink(&target, &linked).unwrap();

        assert!(read(&linked).is_err());
    }

    #[test]
    fn whitespace_includes_the_information_separators() {
        assert_eq!(strip("\u{1f} a \u{3000}"), "a");
        assert_eq!(
            split_whitespace(" a\u{1c}b  c ").collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(strip("\u{feff}a"), "\u{feff}a");
    }

    #[test]
    fn letters_exclude_marks_and_digits() {
        assert!(is_alpha("café"));
        assert!(!is_alpha("e\u{301}"));
        assert!(!is_alpha("a1"));
        assert!(!is_alpha(""));
    }

    #[test]
    fn splitlines_matches_python() {
        assert_eq!(
            splitlines("a\r\nb\rc\nd\u{2028}e\u{0c}"),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(splitlines("a\n\nb"), ["a", "", "b"]);
        assert!(splitlines("").is_empty());
    }

    #[test]
    fn universal_lines_only_split_on_carriage_returns_and_newlines() {
        assert_eq!(
            universal_lines("a\r\nb\rc\nd\u{2028}e"),
            ["a", "b", "c", "d\u{2028}e"]
        );
        assert_eq!(universal_lines("a\n"), ["a"]);
    }
}
