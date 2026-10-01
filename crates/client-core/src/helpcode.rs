//! Discovery metadata for user supplied helper-code tables.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// A helper-code table found below `helpcodes/custom`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CustomHelpcodeSchema {
    /// The identifier persisted in preferences and passed to Engine.
    pub schema: String,
    /// The file name without its `.txt` extension.
    pub file_stem: String,
    /// Optional display name from a leading `# name:` comment.
    pub name: String,
    /// Optional English display name from a leading `# name_en:` comment.
    pub name_en: String,
}

/// Return the directory in which user supplied helper-code tables live.
pub fn custom_helpcode_directory(resources: &Path) -> PathBuf {
    resources.join("helpcodes").join("custom")
}

/// Validate and extract the file stem from a custom schema identifier.
///
/// The check deliberately mirrors Engine's boundary: the identifier must stay a single file name
/// below `helpcodes/custom`, so path traversal and platform-specific filename separators are not
/// accepted.
pub fn custom_schema_stem(schema: &str) -> Option<&str> {
    let stem = schema.strip_prefix("custom/")?;
    if stem.is_empty()
        || stem.starts_with('.')
        || stem
            .chars()
            .any(|character| character.is_control() || "/\\:*?\"<>|".contains(character))
    {
        return None;
    }
    Some(stem)
}

/// Whether `schema` names a syntactically valid custom table.
pub fn is_custom_schema(schema: &str) -> bool {
    custom_schema_stem(schema).is_some()
}

/// Return the table path for an available custom schema.
pub fn custom_helpcode_path(resources: &Path, schema: &str) -> Option<PathBuf> {
    let stem = custom_schema_stem(schema)?;
    let directory = custom_helpcode_directory(resources);
    crate::storage::reject_symlink(&directory).ok()?;
    let path = directory.join(format!("{stem}.txt"));
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    metadata.file_type().is_file().then_some(path)
}

/// Discover regular `.txt` files in `helpcodes/custom` and read their optional display metadata.
/// Missing or unreadable directories produce an empty list, matching the Engine's optional asset
/// semantics. Results are ordered by file stem for stable settings UI presentation.
pub fn list_custom_helpcode_schemas(resources: &Path) -> Vec<CustomHelpcodeSchema> {
    let directory = custom_helpcode_directory(resources);
    if crate::storage::reject_symlink(&directory).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut result = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !entry.file_type().ok()?.is_file()
                || !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
            {
                return None;
            }
            let file_stem = path.file_stem()?.to_str()?.to_owned();
            let schema = format!("custom/{file_stem}");
            custom_schema_stem(&schema)?;
            let (name, name_en) = read_display_names(&path);
            Some(CustomHelpcodeSchema {
                schema,
                file_stem,
                name,
                name_en,
            })
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.file_stem.cmp(&right.file_stem));
    result
}

fn read_display_names(path: &Path) -> (String, String) {
    const MAX_HELPCODE_BYTES: u64 = 1024 * 1024;
    let Ok(file) = std::fs::File::open(path) else {
        return (String::new(), String::new());
    };
    let Ok(bytes) = crate::bounded_io::read_bounded(file, MAX_HELPCODE_BYTES) else {
        return (String::new(), String::new());
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return (String::new(), String::new());
    };
    let mut name = String::new();
    let mut name_en = String::new();
    for (index, raw_line) in text.lines().enumerate() {
        let mut line = raw_line.trim_end_matches('\r');
        if index == 0 {
            line = line.strip_prefix('\u{feff}').unwrap_or(line);
        }
        let line = line.trim_matches([' ', '\t']);
        if line.is_empty() {
            continue;
        }
        let Some(comment) = line.strip_prefix('#') else {
            break;
        };
        let comment = comment.trim_matches([' ', '\t']);
        let (key, value) = comment
            .split_once(':')
            .or_else(|| comment.split_once('：'))
            .map(|(key, value)| {
                (
                    key.trim_matches([' ', '\t']),
                    value.trim_matches([' ', '\t']),
                )
            })
            .unwrap_or(("", ""));
        match key.to_ascii_lowercase().as_str() {
            "name" => name = value.to_owned(),
            "name_en" => name_en = value.to_owned(),
            _ => {}
        }
    }
    (name, name_en)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_sorted_tables_and_bom_crlf_display_headers() {
        let resources = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("我的码.txt"),
            "\u{feff}# name： 我的辅助码\r\n# name_en: Mine\r\n你=cb\r\n",
        )
        .unwrap();
        fs::write(directory.join("plain.TXT"), "# comment\n你=ab\n").unwrap();
        fs::write(directory.join("README.md"), "# name: not a schema\n").unwrap();

        assert_eq!(
            list_custom_helpcode_schemas(resources.path()),
            vec![
                CustomHelpcodeSchema {
                    schema: "custom/plain".into(),
                    file_stem: "plain".into(),
                    name: String::new(),
                    name_en: String::new(),
                },
                CustomHelpcodeSchema {
                    schema: "custom/我的码".into(),
                    file_stem: "我的码".into(),
                    name: "我的辅助码".into(),
                    name_en: "Mine".into(),
                },
            ]
        );
    }

    #[test]
    fn rejects_custom_schema_path_escape() {
        for schema in [
            "custom/",
            "custom/../helpcode",
            "custom/a\\b",
            "custom/.hidden",
            "custom/a:b",
        ] {
            assert!(!is_custom_schema(schema), "accepted {schema}");
        }
        assert_eq!(custom_schema_stem("custom/我的码"), Some("我的码"));
    }

    #[test]
    fn reports_only_existing_custom_tables_as_available() {
        let resources = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("mine.txt"), "你=ab\n").unwrap();

        assert_eq!(
            custom_helpcode_path(resources.path(), "custom/mine"),
            Some(directory.join("mine.txt"))
        );
        assert_eq!(
            custom_helpcode_path(resources.path(), "custom/missing"),
            None
        );
        assert_eq!(
            custom_helpcode_path(resources.path(), "custom/../mine"),
            None
        );
    }

    #[test]
    fn oversized_table_has_no_display_metadata() {
        let resources = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("oversized.txt"),
            [b"# name: hidden\n".as_slice(), &vec![b'x'; 1024 * 1024 + 1]].concat(),
        )
        .unwrap();

        let schemas = list_custom_helpcode_schemas(resources.path());
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].name, "");
        assert_eq!(schemas[0].name_en, "");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_custom_tables() {
        use std::os::unix::fs::symlink;

        let resources = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        let external = outside.path().join("mine.txt");
        fs::write(&external, "你=ab\n").unwrap();
        symlink(&external, directory.join("mine.txt")).unwrap();

        assert_eq!(custom_helpcode_path(resources.path(), "custom/mine"), None);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_custom_directory() {
        use std::os::unix::fs::symlink;

        let resources = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(directory.parent().unwrap()).unwrap();
        fs::write(outside.path().join("mine.txt"), "你=ab\n").unwrap();
        symlink(outside.path(), &directory).unwrap();

        assert_eq!(custom_helpcode_path(resources.path(), "custom/mine"), None);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_list_tables_from_a_symlinked_custom_directory() {
        use std::os::unix::fs::symlink;

        let resources = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(directory.parent().unwrap()).unwrap();
        fs::write(outside.path().join("mine.txt"), "你=ab\n").unwrap();
        symlink(outside.path(), &directory).unwrap();

        assert!(list_custom_helpcode_schemas(resources.path()).is_empty());
    }
}
