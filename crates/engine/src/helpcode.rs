//! Helpcode (auxiliary code) tables and matching (schemes-lang.md §2, `R/common/helpcode_utils.*`). A keymap maps one character to its 1-2 lowercase code letters. Quanpin and shuangpin filter or reorder with it, and the session and host facade annotate candidates with it.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::assets::{CUSTOM_HELPCODE_DIRECTORY, CUSTOM_HELPCODE_PREFIX, HELPCODES};
use crate::diagnostics::UNKNOWN_HELPCODE_SCHEMA;
use crate::error::{EngineError, Result};
use crate::text::{count_han_chars, first_han_char, last_han_char};
use crate::types::WordItem;

/// The shipped tables are below 150 KiB; leave room for larger compatible tables without
/// allowing a user supplied file to make a session allocate without bound.
const MAX_HELPCODE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HelpcodeKeymap {
    codes: HashMap<String, String>,
}

/// Loaded once per session (and per schema change) and shared by every provider.
pub type SharedKeymap = Arc<HelpcodeKeymap>;

impl HelpcodeKeymap {
    pub fn from_codes(codes: HashMap<String, String>) -> Self {
        Self { codes }
    }

    /// The code letters of one character, lowercase.
    pub fn code(&self, character: &str) -> Option<&str> {
        self.codes.get(character).map(String::as_str)
    }

    /// Table size, only asserted by the loader tests here and in host/tests.rs.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.codes.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleHelpcodeMatch {
    None,
    First,
    Last,
    Both,
}

/// The stem of a `custom/<stem>` schema, or `None` when the stem is empty or could name a file outside the custom directory (helpcode_utils.cpp:26-36).
fn custom_schema_stem(schema: &str) -> Option<&str> {
    let stem = schema.strip_prefix(CUSTOM_HELPCODE_PREFIX)?;
    if stem.is_empty()
        || stem.starts_with('.')
        || stem.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || stem.bytes().any(|byte| byte < 0x20)
    {
        return None;
    }
    Some(stem)
}

fn built_in_helpcode_file(schema: &str) -> Option<&'static str> {
    HELPCODES
        .iter()
        .find(|(name, _)| *name == schema)
        .map(|(_, file)| *file)
}

/// A built-in schema name, or `custom/<stem>` with a stem that cannot escape the custom directory (helpcode_utils.cpp:26-41). The file's existence is checked by `load_helpcode_keymap`, not here.
pub fn is_supported_helpcode_schema(schema: &str) -> bool {
    built_in_helpcode_file(schema).is_some() || custom_schema_stem(schema).is_some()
}

/// The table file for a schema under the resource root; `UNKNOWN_HELPCODE_SCHEMA` for an unsupported name. This joins directly rather than through `paths::join_checked`, as helpcode_utils.cpp:37-40 does: `custom_schema_stem` already refuses every separator and a leading `.`, so the stem is one normal component and can never be `..`, which is a stricter guarantee than the `..` refusal would add.
pub fn helpcode_path(resources: &Path, schema: &str) -> Result<PathBuf> {
    if let Some(file) = built_in_helpcode_file(schema) {
        return Ok(resources.join(file));
    }
    match custom_schema_stem(schema) {
        Some(stem) => {
            let mut filename = String::with_capacity(stem.len() + ".txt".len());
            filename.push_str(stem);
            filename.push_str(".txt");
            Ok(resources.join(CUSTOM_HELPCODE_DIRECTORY).join(filename))
        }
        None => Err(EngineError::invalid(UNKNOWN_HELPCODE_SCHEMA)),
    }
}

/// Parse a helpcode table (helpcode_utils.cpp:54-88): BOM and `\r` stripped, `#` lines skipped, `left=right` with the first up-to-two `a-z` letters of `right`, later lines overwrite. A missing custom file fails with `UNKNOWN_HELPCODE_SCHEMA`.
pub fn load_helpcode_keymap(resources: &Path, schema: &str) -> Result<HelpcodeKeymap> {
    let path = helpcode_path(resources, schema)?;
    let built_in = built_in_helpcode_file(schema).is_some();
    if !built_in {
        let Some(directory) = path.parent() else {
            return Err(EngineError::invalid(UNKNOWN_HELPCODE_SCHEMA));
        };
        let directory_is_real = std::fs::symlink_metadata(directory)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false);
        let file_is_real = std::fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false);
        if !directory_is_real || !file_is_real {
            return Err(EngineError::invalid(UNKNOWN_HELPCODE_SCHEMA));
        }
    } else if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        // Resource files are shipped assets; a link here must not make the
        // engine read outside the verified resource generation.
        if !metadata.file_type().is_file() {
            return Ok(HelpcodeKeymap::default());
        }
    }
    let bytes = match File::open(&path) {
        Ok(file) => {
            if file
                .metadata()
                .map(|metadata| metadata.len())
                .unwrap_or(MAX_HELPCODE_BYTES + 1)
                > MAX_HELPCODE_BYTES
            {
                return Ok(HelpcodeKeymap::default());
            }
            let mut bytes = Vec::new();
            // Bound the read again in case the file grows after the metadata check.
            if file
                .take(MAX_HELPCODE_BYTES + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() as u64 > MAX_HELPCODE_BYTES
            {
                return Ok(HelpcodeKeymap::default());
            }
            bytes
        }
        // The C++ reads the table through an `ifstream` that is never checked (helpcode_utils.cpp:57-67), so any open or read failure (a resource tree without the built-in file, which fixtures and hosts rely on for the default `lantian` schema, a directory in its place, no permission, a Windows sharing violation on a custom table being edited, an I/O error) gives an empty table and the session is still created.
        Err(_) => return Ok(HelpcodeKeymap::default()),
    };
    Ok(HelpcodeKeymap::from_codes(parse_helpcode_table(&bytes)))
}

fn parse_helpcode_table(bytes: &[u8]) -> HashMap<String, String> {
    let mut lines: Vec<&[u8]> = bytes.split(|&byte| byte == b'\n').collect();
    // `getline` yields no final empty line after a trailing newline.
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let mut codes = HashMap::with_capacity(lines.len());
    for (number, mut line) in lines.into_iter().enumerate() {
        if number == 0 {
            line = line.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(line);
        }
        line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.first() == Some(&b'#') {
            continue;
        }
        let Some(position) = line.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        if position == 0 {
            continue;
        }
        // The code is the next one or two bytes, so a longer code is cut to two letters and anything not `a-z` rejects the line.
        let right = &line[position + 1..];
        let code = &right[..right.len().min(2)];
        if code.is_empty() || !code.iter().all(u8::is_ascii_lowercase) {
            continue;
        }
        // A key that is not UTF-8 could never equal a candidate word, so dropping it changes no lookup.
        let Ok(key) = std::str::from_utf8(&line[..position]) else {
            continue;
        };
        codes.insert(key.to_owned(), String::from_utf8_lossy(code).into_owned());
    }
    codes
}

/// The annotation shown beside a candidate: a single character's full code, or the first letters of the first and last Han characters' codes; wrapped in parentheses; uppercase entirely when `uppercase_all`, else only the second letter (helpcode_utils.cpp:189-241). Empty when a code is missing.
pub fn compute_helpcodes(word: &str, uppercase_all: bool, keymap: &HelpcodeKeymap) -> String {
    let single = count_han_chars(word) == 1;
    let (first, last) = if single {
        (keymap.code(word), None)
    } else {
        let (Some(first), Some(last)) = (
            keymap.code(first_han_char(word)),
            keymap.code(last_han_char(word)),
        ) else {
            return String::new();
        };
        (Some(first), Some(last))
    };
    let Some(first) = first else {
        return String::new();
    };
    if first.is_empty() {
        return String::new();
    }
    let capacity = if single { first.len() + 2 } else { 4 };
    let mut result = String::with_capacity(capacity);
    result.push('(');
    if single {
        for (index, character) in first.chars().enumerate() {
            result.push(if uppercase_all || index == 1 {
                character.to_ascii_uppercase()
            } else {
                character
            });
        }
    } else {
        let character = first.chars().next().expect("helpcode is non-empty");
        result.push(if uppercase_all {
            character.to_ascii_uppercase()
        } else {
            character
        });
        let last = last.expect("multi-character helpcode has a last code");
        let character = last.chars().next().expect("helpcode is non-empty");
        result.push(character.to_ascii_uppercase());
    }
    result.push(')');
    result
}

/// Longer than one letter, not double mode, last letter uppercase.
pub fn is_quanpin_single_help_mode(pinyin_with_cases: &str) -> bool {
    pinyin_with_cases.len() > 1
        && !is_quanpin_double_help_mode(pinyin_with_cases)
        && pinyin_with_cases
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_uppercase)
}

/// Longer than two letters and the last two uppercase.
pub fn is_quanpin_double_help_mode(pinyin_with_cases: &str) -> bool {
    let bytes = pinyin_with_cases.as_bytes();
    bytes.len() > 2 && bytes[bytes.len() - 2..].iter().all(u8::is_ascii_uppercase)
}

pub fn match_single_helpcode(
    word: &str,
    help_code: &str,
    keymap: &HelpcodeKeymap,
) -> SingleHelpcodeMatch {
    if word.is_empty() || help_code.len() != 1 {
        return SingleHelpcodeMatch::None;
    }
    let letter = help_code.as_bytes()[0];
    let (matches_first, matches_last) = if count_han_chars(word) == 1 {
        let Some(code) = keymap.code(word).map(str::as_bytes) else {
            return SingleHelpcodeMatch::None;
        };
        (code[0] == letter, code.get(1) == Some(&letter))
    } else {
        let initial = |character: &str| keymap.code(character).map(|code| code.as_bytes()[0]);
        (
            initial(first_han_char(word)) == Some(letter),
            initial(last_han_char(word)) == Some(letter),
        )
    };
    match (matches_first, matches_last) {
        (true, true) => SingleHelpcodeMatch::Both,
        (true, false) => SingleHelpcodeMatch::First,
        (false, true) => SingleHelpcodeMatch::Last,
        (false, false) => SingleHelpcodeMatch::None,
    }
}

pub fn matches_double_helpcodes(word: &str, help_codes: &str, keymap: &HelpcodeKeymap) -> bool {
    if word.is_empty() || help_codes.len() != 2 {
        return false;
    }
    let expected = help_codes.as_bytes();
    if count_han_chars(word) == 1 {
        return keymap
            .code(word)
            .is_some_and(|code| code.as_bytes() == expected);
    }
    match (
        keymap.code(first_han_char(word)),
        keymap.code(last_han_char(word)),
    ) {
        (Some(first), Some(last)) => {
            first.as_bytes()[0] == expected[0] && last.as_bytes()[0] == expected[1]
        }
        _ => false,
    }
}

/// First and Both matches, then Last, then the rest, each stable; unchanged unless `help_code` is one letter.
pub fn reorder_candidates_with_single_helpcode(
    candidates: Vec<WordItem>,
    help_code: &str,
    keymap: &HelpcodeKeymap,
) -> Vec<WordItem> {
    if help_code.len() != 1 {
        return candidates;
    }
    let mut first = Vec::with_capacity(candidates.len());
    let mut last = Vec::with_capacity(candidates.len());
    let mut rest = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        match match_single_helpcode(&candidate.word, help_code, keymap) {
            SingleHelpcodeMatch::First | SingleHelpcodeMatch::Both => first.push(candidate),
            SingleHelpcodeMatch::Last => last.push(candidate),
            SingleHelpcodeMatch::None => rest.push(candidate),
        }
    }
    first.extend(last);
    first.extend(rest);
    first
}

/// Only the double-helpcode matches; empty unless `help_codes` is two letters.
pub fn filter_candidates_with_double_helpcodes(
    candidates: Vec<WordItem>,
    help_codes: &str,
    keymap: &HelpcodeKeymap,
) -> Vec<WordItem> {
    if help_codes.len() != 2 {
        return Vec::new();
    }
    candidates
        .into_iter()
        .filter(|candidate| matches_double_helpcodes(&candidate.word, help_codes, keymap))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    fn keymap(entries: &[(&str, &str)]) -> HelpcodeKeymap {
        HelpcodeKeymap::from_codes(
            entries
                .iter()
                .map(|(character, code)| ((*character).to_owned(), (*code).to_owned()))
                .collect(),
        )
    }

    fn items(words: &[&str]) -> Vec<WordItem> {
        words
            .iter()
            .map(|word| WordItem::new("", *word, 1, CandidateSource::Database, ""))
            .collect()
    }

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn schema_names() {
        for schema in [
            "lantian",
            "ziranma",
            "shouyou2_0",
            "shouyouplus",
            "xiaohe",
            "jiajia",
            "custom/mine",
        ] {
            assert!(is_supported_helpcode_schema(schema), "{schema}");
        }
        for schema in [
            "unknown",
            "custom/",
            "custom/.hidden",
            "custom/..",
            "custom/../x",
            "custom/a/b",
            "custom/a\\b",
            "custom/a:b",
            "custom/a\u{1}",
            "Custom/x",
        ] {
            assert!(!is_supported_helpcode_schema(schema), "{schema:?}");
            assert!(
                helpcode_path(Path::new("/res"), schema).is_err(),
                "{schema:?}"
            );
        }
        let root = Path::new("/res");
        assert_eq!(
            helpcode_path(root, "xiaohe").unwrap(),
            root.join("helpcodes/xiaohe_helpcode.txt")
        );
        assert_eq!(
            helpcode_path(root, "custom/mine").unwrap(),
            root.join("helpcodes/custom").join("mine.txt")
        );
        assert_eq!(
            helpcode_path(root, "unknown").unwrap_err().to_string(),
            UNKNOWN_HELPCODE_SCHEMA
        );
    }

    #[test]
    fn tables_parse_the_way_the_engine_reads_them() {
        let parsed = parse_helpcode_table(
            "\u{feff}你=ab\r\n# comment=zz\n好=c\n=de\nno code\n长=xyz\n坏=a1\n坏=q\n私=s\u{e643}\n多字=mn\n末=gh".as_bytes(),
        );
        let expected: HashMap<String, String> = [
            ("你", "ab"),
            ("好", "c"),
            ("长", "xy"),
            ("坏", "q"),
            ("多字", "mn"),
            ("末", "gh"),
        ]
        .into_iter()
        .map(|(key, code)| (key.to_owned(), code.to_owned()))
        .collect();
        assert_eq!(parsed, expected);
    }

    /// helpcode_utils.cpp:57-67 reads through an `ifstream` it never checks, so a table that exists but cannot be read (a directory, no permission, a sharing violation) is an empty table and the session is still created.
    #[test]
    fn an_unreadable_table_is_an_empty_table() {
        let resources = tempfile::tempdir().unwrap();
        let built_in = helpcode_path(resources.path(), "lantian").unwrap();
        std::fs::create_dir_all(&built_in).unwrap();
        assert_eq!(
            load_helpcode_keymap(resources.path(), "lantian")
                .unwrap()
                .len(),
            0
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let custom = resources.path().join("helpcodes/custom");
            std::fs::create_dir_all(&custom).unwrap();
            let file = custom.join("locked.txt");
            std::fs::write(&file, "你=aa\n").unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
            // A process that can read the file anyway (root) has nothing to prove here.
            if std::fs::read(&file).is_err() {
                assert_eq!(
                    load_helpcode_keymap(resources.path(), "custom/locked")
                        .unwrap()
                        .len(),
                    0
                );
            }
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    #[test]
    fn loading_resolves_built_in_and_custom_files() {
        let resources = tempfile::tempdir().unwrap();
        // A missing built-in table is an empty table, as the unchecked C++ stream gives.
        assert_eq!(
            load_helpcode_keymap(resources.path(), "lantian")
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            load_helpcode_keymap(resources.path(), "custom/synthetic")
                .unwrap_err()
                .to_string(),
            UNKNOWN_HELPCODE_SCHEMA
        );
        assert_eq!(
            load_helpcode_keymap(resources.path(), "unknown")
                .unwrap_err()
                .to_string(),
            UNKNOWN_HELPCODE_SCHEMA
        );
        let custom = resources.path().join("helpcodes/custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(custom.join("synthetic.txt"), "\u{feff}你=aa\r\n#你=zz\r\n").unwrap();
        let loaded = load_helpcode_keymap(resources.path(), "custom/synthetic").unwrap();
        assert_eq!(loaded.code("你"), Some("aa"));
        assert_eq!(loaded.len(), 1);
        std::fs::write(resources.path().join("helpcodes/helpcode.txt"), "你=ab\n").unwrap();
        assert_eq!(
            load_helpcode_keymap(resources.path(), "lantian")
                .unwrap()
                .code("你"),
            Some("ab")
        );
    }

    #[test]
    fn oversized_custom_table_is_not_loaded() {
        let resources = tempfile::tempdir().unwrap();
        let custom = resources.path().join("helpcodes/custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            custom.join("oversized.txt"),
            vec![b'x'; MAX_HELPCODE_BYTES as usize + 1],
        )
        .unwrap();

        assert_eq!(
            load_helpcode_keymap(resources.path(), "custom/oversized")
                .unwrap()
                .len(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn custom_tables_reject_symlinked_files_and_directories() {
        use std::os::unix::fs::symlink;

        let resources = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let custom = resources.path().join("helpcodes/custom");
        std::fs::create_dir_all(&custom).unwrap();
        let external = outside.path().join("mine.txt");
        std::fs::write(&external, "你=aa\n").unwrap();
        symlink(&external, custom.join("mine.txt")).unwrap();
        assert_eq!(
            load_helpcode_keymap(resources.path(), "custom/mine")
                .unwrap_err()
                .to_string(),
            UNKNOWN_HELPCODE_SCHEMA
        );

        std::fs::remove_file(custom.join("mine.txt")).unwrap();
        std::fs::remove_dir(&custom).unwrap();
        symlink(outside.path(), &custom).unwrap();
        assert_eq!(
            load_helpcode_keymap(resources.path(), "custom/mine")
                .unwrap_err()
                .to_string(),
            UNKNOWN_HELPCODE_SCHEMA
        );
    }

    // engine-bridge/src/tests.rs:512-545: the carried jiajia table loads whole.
    #[test]
    fn the_carried_jiajia_table_loads_whole() {
        let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let loaded = load_helpcode_keymap(&resources, "jiajia").unwrap();
        assert_eq!(loaded.len(), 7968);
        for (character, code) in [("好", "nz"), ("你", "de"), ("中", "ks"), ("国", "ky")] {
            assert_eq!(loaded.code(character), Some(code));
        }
    }

    #[test]
    fn annotations() {
        let map = keymap(&[("阿", "ek"), ("姨", "nr"), ("一", "y")]);
        let single = compute_helpcodes("阿", false, &map);
        assert_eq!(single, "(eK)");
        assert_eq!(single.capacity(), single.len());
        let uppercase = compute_helpcodes("阿", true, &map);
        assert_eq!(uppercase, "(EK)");
        assert_eq!(uppercase.capacity(), uppercase.len());
        assert_eq!(compute_helpcodes("一", false, &map), "(y)");
        let pair = compute_helpcodes("阿姨", false, &map);
        assert_eq!(pair, "(eN)");
        assert_eq!(pair.capacity(), pair.len());
        assert_eq!(compute_helpcodes("A阿姨B", true, &map), "(EN)");
        assert_eq!(compute_helpcodes("阿好", false, &map), "");
        assert_eq!(compute_helpcodes("好", false, &map), "");
        assert_eq!(compute_helpcodes("", false, &map), "");
    }

    #[test]
    fn help_modes() {
        assert!(is_quanpin_single_help_mode("niH"));
        assert!(!is_quanpin_single_help_mode("H"));
        assert!(!is_quanpin_single_help_mode("niHA"));
        assert!(!is_quanpin_single_help_mode("nih"));
        assert!(is_quanpin_double_help_mode("niHA"));
        assert!(!is_quanpin_double_help_mode("HA"));
        assert!(!is_quanpin_double_help_mode("nihA"));
    }

    // shuangpin_dictionary.cpp:307-316, 377-386: 阿 (ek) and 阿姨 (ek, nr).
    #[test]
    fn matching() {
        let map = keymap(&[("阿", "ek"), ("姨", "nr"), ("一", "y"), ("嗯", "ee")]);
        assert_eq!(
            match_single_helpcode("阿", "e", &map),
            SingleHelpcodeMatch::First
        );
        assert_eq!(
            match_single_helpcode("阿", "k", &map),
            SingleHelpcodeMatch::Last
        );
        assert_eq!(
            match_single_helpcode("嗯", "e", &map),
            SingleHelpcodeMatch::Both
        );
        assert_eq!(
            match_single_helpcode("一", "y", &map),
            SingleHelpcodeMatch::First
        );
        assert_eq!(
            match_single_helpcode("阿姨", "e", &map),
            SingleHelpcodeMatch::First
        );
        assert_eq!(
            match_single_helpcode("阿姨", "n", &map),
            SingleHelpcodeMatch::Last
        );
        assert_eq!(
            match_single_helpcode("阿阿", "e", &map),
            SingleHelpcodeMatch::Both
        );
        assert_eq!(
            match_single_helpcode("阿姨", "k", &map),
            SingleHelpcodeMatch::None
        );
        assert_eq!(
            match_single_helpcode("好", "e", &map),
            SingleHelpcodeMatch::None
        );
        assert_eq!(
            match_single_helpcode("阿", "ek", &map),
            SingleHelpcodeMatch::None
        );
        assert!(matches_double_helpcodes("阿", "ek", &map));
        assert!(!matches_double_helpcodes("一", "yy", &map));
        assert!(matches_double_helpcodes("阿姨", "en", &map));
        assert!(!matches_double_helpcodes("阿姨", "ek", &map));
        assert!(!matches_double_helpcodes("阿好", "eh", &map));
        assert!(!matches_double_helpcodes("阿", "e", &map));
    }

    #[test]
    fn reorder_and_filter() {
        let map = keymap(&[("你", "ab"), ("拟", "ba"), ("泥", "cc"), ("妮", "bb")]);
        let reordered = reorder_candidates_with_single_helpcode(
            items(&["泥", "你", "拟", "妮", "好"]),
            "b",
            &map,
        );
        assert_eq!(words(&reordered), ["拟", "妮", "你", "泥", "好"]);
        let unchanged = reorder_candidates_with_single_helpcode(items(&["泥", "你"]), "ab", &map);
        assert_eq!(words(&unchanged), ["泥", "你"]);
        let filtered =
            filter_candidates_with_double_helpcodes(items(&["泥", "你", "拟"]), "ab", &map);
        assert_eq!(words(&filtered), ["你"]);
        assert!(filter_candidates_with_double_helpcodes(items(&["你"]), "a", &map).is_empty());
    }
}
