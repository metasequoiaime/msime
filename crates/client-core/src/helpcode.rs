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

/// 查找可用的自定义表：优先精确小写扩展名，再按字典序尝试其余大小写变体。
pub fn custom_helpcode_path(resources: &Path, schema: &str) -> Option<PathBuf> {
    let stem = custom_schema_stem(schema)?;
    let directory = custom_helpcode_directory(resources);
    crate::storage::reject_symlink(&directory).ok()?;
    let mut path = directory.join(format!("{stem}.txt"));
    for extension in ["txt", "TXT", "TXt", "TxT", "Txt", "tXT", "tXt", "txT"] {
        path.set_extension(extension);
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file()) {
            return Some(path);
        }
    }
    None
}

/// 列出 `helpcodes/custom` 下扩展名不分大小写的普通 `.txt` 表并读取显示元数据。
/// 缺失或不可读的目录返回空列表；按主干排序，同一主干优先小写 `.txt`，否则取扩展名字典序最小的文件。
pub fn list_custom_helpcode_schemas(resources: &Path) -> Vec<CustomHelpcodeSchema> {
    let directory = custom_helpcode_directory(resources);
    if crate::storage::reject_symlink(&directory).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut paths = entries
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
            Some(path)
        })
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| {
        left.file_stem()
            .cmp(&right.file_stem())
            .then_with(|| {
                (left.extension() != Some(std::ffi::OsStr::new("txt")))
                    .cmp(&(right.extension() != Some(std::ffi::OsStr::new("txt"))))
            })
            .then_with(|| left.file_name().cmp(&right.file_name()))
    });
    paths.dedup_by(|left, right| left.file_stem() == right.file_stem());
    paths
        .into_iter()
        .filter_map(|path| {
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
        .collect()
}

fn read_display_names(path: &Path) -> (String, String) {
    const MAX_HELPCODE_BYTES: u64 = 1024 * 1024;
    let Ok(file) = crate::storage::open_private_file_in(path) else {
        return (String::new(), String::new());
    };
    let Ok(bytes) = crate::bounded_io::read_bounded(file, MAX_HELPCODE_BYTES) else {
        return (String::new(), String::new());
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return (String::new(), String::new());
    };
    display_names(text)
}

/// 码表开头注释里的显示名：`# name:` 和 `# name_en:`（冒号也可以是全角），遇到第一行非空、非注释的内容就停；没有时为空字符串。开头的 BOM、行尾的 `\r` 和两端的空格、制表符都不算进名字。
#[doc(hidden)]
pub fn display_names(text: &str) -> (String, String) {
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
    fn discovered_extension_variants_resolve_to_readable_tables() {
        let resources = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        for (index, extension) in ["txt", "txT", "tXt", "tXT", "Txt", "TxT", "TXt", "TXT"]
            .into_iter()
            .enumerate()
        {
            let stem = format!("synthetic{index}");
            let contents = format!("# name: {stem}\n你=ab\n");
            fs::write(directory.join(format!("{stem}.{extension}")), &contents).unwrap();
            let path = custom_helpcode_path(resources.path(), &format!("custom/{stem}"))
                .unwrap_or_else(|| panic!("{extension} was listed but could not be resolved"));
            assert_eq!(fs::read_to_string(path).unwrap(), contents);
        }
        assert_eq!(list_custom_helpcode_schemas(resources.path()).len(), 8);
    }

    #[test]
    fn extension_variants_have_one_deterministic_catalog_entry() {
        let resources = tempfile::tempdir().unwrap();
        let directory = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&directory).unwrap();
        let canonical = directory.join("synthetic.txt");
        let upper = directory.join("synthetic.TXT");
        fs::write(&canonical, "# name: canonical\n你=ab\n").unwrap();
        fs::write(&upper, "# name: uppercase\n你=cd\n").unwrap();
        if fs::read(&canonical).unwrap() == fs::read(&upper).unwrap() {
            return;
        }
        let names = || {
            list_custom_helpcode_schemas(resources.path())
                .into_iter()
                .map(|schema| (schema.schema, schema.name))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(), [("custom/synthetic".into(), "canonical".into())]);
        fs::remove_file(canonical).unwrap();
        fs::write(directory.join("synthetic.TxT"), "# name: mixed\n你=ef\n").unwrap();
        assert_eq!(names(), [("custom/synthetic".into(), "uppercase".into())]);
        assert_eq!(
            custom_helpcode_path(resources.path(), "custom/synthetic"),
            Some(upper)
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
        for extension in ["txt", "TXT", "tXt"] {
            let path = directory.join(format!("mine.{extension}"));
            symlink(&external, &path).unwrap();
            assert_eq!(custom_helpcode_path(resources.path(), "custom/mine"), None);
            assert!(list_custom_helpcode_schemas(resources.path()).is_empty());
            fs::remove_file(path).unwrap();
        }
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
