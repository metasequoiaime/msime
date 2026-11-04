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

/// 宿主列给设置页的自定义表：用户目录（状态根下的 `helpcodes/custom`）和资源目录两处合起来，按主干排序；同一主干两边都有时取用户目录那份，与 Engine 读表的顺序（`msime_engine::load_helpcode_keymap_in`）一致。`user_root` 不是绝对路径时只看资源目录。
pub fn list_custom_helpcode_schemas_in(
    resources: &Path,
    user_root: Option<&Path>,
) -> Vec<CustomHelpcodeSchema> {
    let mut schemas = user_root
        .filter(|path| path.is_absolute())
        .map(list_custom_helpcode_schemas)
        .unwrap_or_default();
    for schema in list_custom_helpcode_schemas(resources) {
        if !schemas
            .iter()
            .any(|listed| listed.file_stem == schema.file_stem)
        {
            schemas.push(schema);
        }
    }
    schemas.sort_by(|left, right| left.file_stem.cmp(&right.file_stem));
    schemas
}

/// 把资源目录里的自定义表复制进用户目录 `user_root/helpcodes/custom`，返回复制了几张。
///
/// 资源目录是安装包或词库代次：Windows 升级时删掉整个 `server` 目录，macOS 换掉整个应用包，在线词库更新把 `resources` 换成新代次，放在旧目录里的表就再也找不到了，偏好里的 `custom/<stem>` 只剩一张空表。宿主准备或刷新运行时配置时用它把还在的表搬到状态根下，之后 Engine 先读那里。用户目录里已有同一主干的表时不动它，源文件也不删；只复制按设置页同样规则列得出来、不超过 1 MiB 的普通文件，写成小写 `.txt`。经临时文件发布、不替换已有文件，几个进程同时搬也只有一份落地。任何一张读写失败都只是少复制一张，不影响准备流程。
pub fn adopt_custom_helpcodes(user_root: &Path, sources: &[&Path]) -> usize {
    if !user_root.is_absolute() {
        return 0;
    }
    let destination = custom_helpcode_directory(user_root);
    let mut adopted = 0;
    for source in sources {
        if !source.is_absolute() || *source == user_root {
            continue;
        }
        for schema in list_custom_helpcode_schemas(source) {
            if custom_helpcode_path(user_root, &schema.schema).is_some() {
                continue;
            }
            let Some(path) = custom_helpcode_path(source, &schema.schema) else {
                continue;
            };
            let Some(bytes) = read_table(&path) else {
                continue;
            };
            if copy_table(&destination, &schema.file_stem, &bytes).is_ok() {
                adopted += 1;
            }
        }
    }
    adopted
}

fn copy_table(directory: &Path, stem: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if !crate::storage::create_directory_and_check(directory)? {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut staged = tempfile::Builder::new()
        .prefix(".adopt-")
        .tempfile_in(directory)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist_noclobber(directory.join(format!("{stem}.txt")))
        .map_err(|error| error.error)?;
    Ok(())
}

const MAX_HELPCODE_BYTES: u64 = 1024 * 1024;

fn read_table(path: &Path) -> Option<Vec<u8>> {
    let file = crate::storage::open_private_file_in(path).ok()?;
    crate::bounded_io::read_bounded(file, MAX_HELPCODE_BYTES).ok()
}

fn read_display_names(path: &Path) -> (String, String) {
    let Some(bytes) = read_table(path) else {
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

    /// 用户目录和资源目录合起来列出；同一主干取用户目录那份。
    #[test]
    fn lists_user_tables_before_resource_tables_of_the_same_stem() {
        let resources = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let legacy = custom_helpcode_directory(resources.path());
        let user = custom_helpcode_directory(state.path());
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(legacy.join("both.txt"), "# name: 旧\n你=ab\n").unwrap();
        fs::write(legacy.join("legacy.txt"), "你=ab\n").unwrap();
        fs::write(user.join("both.txt"), "# name: 新\n你=ab\n").unwrap();
        fs::write(user.join("mine.txt"), "你=ab\n").unwrap();

        let schemas = list_custom_helpcode_schemas_in(resources.path(), Some(state.path()));
        let listed: Vec<_> = schemas
            .iter()
            .map(|schema| (schema.schema.as_str(), schema.name.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                ("custom/both", "新"),
                ("custom/legacy", ""),
                ("custom/mine", ""),
            ]
        );
        assert_eq!(
            list_custom_helpcode_schemas_in(resources.path(), Some(Path::new("relative"))),
            list_custom_helpcode_schemas(resources.path())
        );
    }

    /// 资源目录里的表复制进状态根：已有的不覆盖、源文件留着、超过 1 MiB 的不搬、再搬一次什么也不做；源就是状态根时跳过。
    #[test]
    fn adopts_resource_tables_into_the_user_root_once() {
        let resources = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let legacy = custom_helpcode_directory(resources.path());
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("mine.TXT"), "# name: 我的\n你=ab\n").unwrap();
        fs::write(legacy.join("kept.txt"), "你=cd\n").unwrap();
        fs::write(legacy.join("huge.txt"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
        let user = custom_helpcode_directory(state.path());
        fs::create_dir_all(&user).unwrap();
        fs::write(user.join("kept.txt"), "你=ef\n").unwrap();

        assert_eq!(
            adopt_custom_helpcodes(state.path(), &[resources.path(), state.path()]),
            1
        );
        assert_eq!(
            fs::read_to_string(user.join("mine.txt")).unwrap(),
            "# name: 我的\n你=ab\n"
        );
        assert_eq!(
            fs::read_to_string(user.join("kept.txt")).unwrap(),
            "你=ef\n"
        );
        assert!(!user.join("huge.txt").exists());
        assert!(legacy.join("mine.TXT").is_file());
        assert_eq!(adopt_custom_helpcodes(state.path(), &[resources.path()]), 0);
        assert_eq!(
            adopt_custom_helpcodes(Path::new("relative"), &[resources.path()]),
            0
        );
        let leftovers: Vec<_> = fs::read_dir(&user)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
