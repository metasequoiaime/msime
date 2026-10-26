//! 设置页的插件编辑器：普通用户不写 TOML，也能新建和修改短语表、指令表、符号集和单词本。
//!
//! 编辑器与宿主之间只交换一份结构化的 [`AuthoredPack`]。保存时这里把它写成 `plugin.toml`（单词本另写 `words.tsv`），放进暂存目录，再走与导入完全相同的 `import::install_staged`：同一把写锁、同一个 `check_staged`、同样整包替换。所以编辑器保存的包和用户从磁盘导入的包按同一套规则校验，规则只有一份。这里额外挡住的只有 TSV 写不出来的内容（某一列里有制表符或换行、单词以 `#` 开头），否则一个单词会被拆成几行或被当成注释，悄悄变成别的内容。读取时反过来，从已安装的包得到同样的结构。
//!
//! 社区装来的包和导入的包一样是用户插件目录里的普通包，可以照样编辑；保存时包里原有的说明文件（`.txt`、`.md`，例如许可证全文）原样带进新包，不因为改了内容而丢掉署名。

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Cursor};
use std::path::Path;

use super::import::{install_staged, write_member, Existing};
use super::{
    is_builtin, is_notice, kind_directory, list_files, load_package_unread, read_file,
    wordbook_pack, BuiltinDirs, PluginContent, PluginError, PluginKind, PluginSummary,
    MANIFEST_FILE, MAX_MANIFEST_BYTES, MAX_NOTICE_BYTES,
};
use crate::skin::catalog::safe_id;

/// 编辑器生成的 id 的前缀，后面跟 8 位小写十六进制随机数。它同时满足单词本更严的 id 规则（小写字母、数字和 `-`，不超过 59 个字符）。
pub const GENERATED_ID_PREFIX: &str = "my-";
/// 编辑器写出的单词本词表文件名。
pub const WORDS_FILE: &str = "words.tsv";
/// 生成 id 时最多试几次；插件目录里同类型的包远少于 2^32 个，撞上一次都罕见。
const MAX_ID_ATTEMPTS: usize = 32;

/// 编辑器能写的四种类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredKind {
    PhraseTable,
    CommandTable,
    SymbolSet,
    Wordbook,
}

impl AuthoredKind {
    pub fn plugin_kind(self) -> PluginKind {
        match self {
            Self::PhraseTable => PluginKind::PhraseTable,
            Self::CommandTable => PluginKind::CommandTable,
            Self::SymbolSet => PluginKind::SymbolSet,
            Self::Wordbook => PluginKind::Wordbook,
        }
    }

    fn from_plugin_kind(kind: PluginKind) -> Option<Self> {
        match kind {
            PluginKind::PhraseTable => Some(Self::PhraseTable),
            PluginKind::CommandTable => Some(Self::CommandTable),
            PluginKind::SymbolSet => Some(Self::SymbolSet),
            PluginKind::Wordbook => Some(Self::Wordbook),
            _ => None,
        }
    }

    /// 这种类型的内容字段名。
    fn content_field(self) -> &'static str {
        match self {
            Self::PhraseTable => "phrases",
            Self::CommandTable => "commands",
            Self::SymbolSet => "groups",
            Self::Wordbook => "words",
        }
    }
}

/// 短语表的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPhrase {
    pub key: String,
    pub text: String,
}

/// 指令表的一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredCommand {
    pub trigger: String,
    pub title: String,
    pub template: String,
}

/// 符号集的一组。`tab` 原样写进清单，不是 `symbols` 或 `kaomoji` 时由清单校验拒绝。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredGroup {
    pub tab: String,
    pub title: String,
    /// 为空或全是空白时不写进清单。
    #[serde(default)]
    pub keywords: Option<String>,
    pub items: Vec<String>,
}

/// 单词本的一个单词。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredWord {
    pub word: String,
    /// 为空时这一行只写两列。
    #[serde(default)]
    pub phonetic: Option<String>,
    pub gloss: String,
}

/// 编辑器里的一个包：清单的通用字段，加上与 `kind` 对应的那一个内容字段。`id` 为 null 表示新建、由 [`save`] 生成；`author`、`description` 为 null 或全是空白时不写进清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPack {
    pub kind: AuthoredKind,
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub version: String,
    pub license: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phrases: Option<Vec<AuthoredPhrase>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<AuthoredCommand>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<AuthoredGroup>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<AuthoredWord>>,
}

/// 用户插件目录里 `kind` 类的包 `id`，读成编辑器的结构。内置和官方包、编辑器不支持的类型、没装或载入不了的包都是 `plugin_not_editable`，说明写明是哪一种。
pub fn load(root: &Path, kind: &str, id: &str) -> Result<AuthoredPack, PluginError> {
    let authored = PluginKind::parse(kind)
        .and_then(AuthoredKind::from_plugin_kind)
        .ok_or_else(|| PluginError::NotEditable("这种插件不能在设置页编辑".into()))?;
    let kind = authored.plugin_kind();
    if is_builtin(kind, id) {
        return Err(PluginError::NotEditable(
            "随输入法分发的插件不能编辑".into(),
        ));
    }
    // 内置目录一律不给：这里只认用户插件目录里的包。
    let summary = load_package_unread(root, BuiltinDirs::default(), kind, id)
        .map_err(PluginError::NotEditable)?;
    let mut pack = AuthoredPack {
        kind: authored,
        id: Some(summary.id),
        name: summary.name,
        version: summary.version,
        license: summary.license,
        author: summary.author,
        description: summary.description,
        phrases: None,
        commands: None,
        groups: None,
        words: None,
    };
    match summary.content {
        PluginContent::PhraseTable(table) => {
            pack.phrases = Some(
                table
                    .phrases
                    .into_iter()
                    .map(|row| AuthoredPhrase {
                        key: row.key,
                        text: row.text,
                    })
                    .collect(),
            );
        }
        PluginContent::CommandTable(table) => {
            pack.commands = Some(
                table
                    .commands
                    .into_iter()
                    .map(|row| AuthoredCommand {
                        trigger: row.trigger,
                        title: row.title,
                        template: row.template,
                    })
                    .collect(),
            );
        }
        PluginContent::SymbolSet(set) => {
            pack.groups = Some(
                set.groups
                    .into_iter()
                    .map(|group| AuthoredGroup {
                        tab: group.tab.as_str().to_owned(),
                        title: group.title,
                        keywords: (!group.keywords.is_empty()).then_some(group.keywords),
                        items: group.items,
                    })
                    .collect(),
            );
        }
        PluginContent::Wordbook(book) => {
            let entries = wordbook_pack::read_entries(&summary.directory, &book.file)
                .map_err(PluginError::NotEditable)?;
            pack.words = Some(
                entries
                    .into_iter()
                    .map(|entry| AuthoredWord {
                        word: entry.word,
                        phonetic: (!entry.phonetic.is_empty()).then_some(entry.phonetic),
                        gloss: entry.meaning,
                    })
                    .collect(),
            );
        }
        _ => return Err(PluginError::NotEditable("这种插件不能在设置页编辑".into())),
    }
    Ok(pack)
}

/// 把 `pack` 装进 `root`，返回与导入相同的包摘要。`pack.id` 为 null 时生成一个没被占用的 `my-` id。同类型、同 id 的包已经装着时，`replace` 为真就整包替换（原包的说明文件带进新包），为假则是 `plugin_exists`。内容不合规时是导入同样的 `plugin_invalid`，说明是清单校验报告的规则。
pub fn save(root: &Path, pack: &AuthoredPack, replace: bool) -> Result<PluginSummary, PluginError> {
    check_shape(pack)?;
    let kind = pack.kind.plugin_kind();
    if let Some(id) = &pack.id {
        // 路径拼接之前先挡住不是一个普通文件夹名的 id；其余 id 规则（包括单词本更严的那条）由清单校验负责。
        if !safe_id(id) {
            return Err(PluginError::Invalid("插件 id 无效".into()));
        }
        if is_builtin(kind, id) {
            return Err(PluginError::Reserved);
        }
    }
    let existing = if replace {
        Existing::Replace
    } else {
        Existing::Refuse
    };
    install_staged(root, false, existing, |staging, staging_directory| {
        let id = match &pack.id {
            Some(id) => id.clone(),
            None => generate_id(root, kind, random_bytes)?,
        };
        for (name, bytes) in render(pack, &id)? {
            write_member(
                staging,
                name,
                Cursor::new(&bytes),
                bytes.len() as u64,
                staging_directory,
            )?;
        }
        if replace {
            copy_notices(root, kind, &id, staging, staging_directory)?;
        }
        Ok(())
    })
}

/// 恰好有与 `kind` 对应的那一个内容字段。
fn check_shape(pack: &AuthoredPack) -> Result<(), PluginError> {
    let present = [
        ("phrases", pack.phrases.is_some()),
        ("commands", pack.commands.is_some()),
        ("groups", pack.groups.is_some()),
        ("words", pack.words.is_some()),
    ];
    let expected = pack.kind.content_field();
    if present
        .iter()
        .all(|(field, present)| *present == (*field == expected))
    {
        Ok(())
    } else {
        Err(PluginError::Invalid(format!(
            "{} 只能带 {expected} 这一项内容",
            pack.kind.plugin_kind().as_str()
        )))
    }
}

fn random_bytes() -> io::Result<[u8; 4]> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("系统随机数不可用"))?;
    Ok(bytes)
}

/// `my-` 加 8 位小写十六进制，取 `random` 给出的字节；`<root>/<kind>/` 下已经有这个名字时换一个再试。在插件根目录的写锁里调用，所以选中的 id 到安装为止不会被别人占去。
fn generate_id(
    root: &Path,
    kind: PluginKind,
    mut random: impl FnMut() -> io::Result<[u8; 4]>,
) -> Result<String, PluginError> {
    let directory = kind_directory(root, kind);
    for _ in 0..MAX_ID_ATTEMPTS {
        let bytes = random()?;
        let mut id = String::with_capacity(GENERATED_ID_PREFIX.len() + 8);
        id.push_str(GENERATED_ID_PREFIX);
        for byte in bytes {
            id.push_str(&format!("{byte:02x}"));
        }
        if fs::symlink_metadata(directory.join(&id)).is_err() {
            return Ok(id);
        }
    }
    Err(PluginError::Storage)
}

/// 为空或全是空白的可选文本按没写处理。
fn present(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|value| !value.trim().is_empty())
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: u8,
    kind: &'static str,
    id: &'a str,
    name: &'a str,
    version: &'a str,
    license: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phrases: Option<&'a [AuthoredPhrase]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<&'a [AuthoredCommand]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    groups: Option<Vec<ManifestGroup<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wordbook: Option<ManifestWordbook>,
}

#[derive(Serialize)]
struct ManifestGroup<'a> {
    tab: &'a str,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    keywords: Option<&'a str>,
    items: &'a [String],
}

#[derive(Serialize)]
struct ManifestWordbook {
    file: &'static str,
}

/// 包里要写的文件：`plugin.toml`，单词本另有 `words.tsv`。
fn render(pack: &AuthoredPack, id: &str) -> Result<Vec<(&'static str, Vec<u8>)>, PluginError> {
    let manifest = Manifest {
        schema_version: 1,
        kind: pack.kind.plugin_kind().as_str(),
        id,
        name: &pack.name,
        version: &pack.version,
        license: &pack.license,
        author: present(&pack.author),
        description: present(&pack.description),
        phrases: pack.phrases.as_deref(),
        commands: pack.commands.as_deref(),
        groups: pack.groups.as_ref().map(|groups| {
            groups
                .iter()
                .map(|group| ManifestGroup {
                    tab: &group.tab,
                    title: &group.title,
                    keywords: present(&group.keywords),
                    items: &group.items,
                })
                .collect()
        }),
        wordbook: pack
            .words
            .as_ref()
            .map(|_| ManifestWordbook { file: WORDS_FILE }),
    };
    let text = toml::to_string(&manifest)
        .map_err(|error| PluginError::Invalid(format!("plugin.toml 无法生成：{error}")))?;
    // 同一条大小上限，换成编辑器用户看得懂的说法。
    if text.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(PluginError::Invalid(format!(
            "内容太多，生成的 plugin.toml 超过 {} KiB，请减少条目或缩短文字",
            MAX_MANIFEST_BYTES / 1024
        )));
    }
    let mut files = vec![(MANIFEST_FILE, text.into_bytes())];
    if let Some(words) = &pack.words {
        files.push((WORDS_FILE, render_words(words)?));
    }
    Ok(files)
}

/// 单词本的 TSV：一行一个单词，没有音标时两列。列里的制表符和换行会把一个单词拆成别的行，`#` 开头的单词会被读成注释，这两种在写出之前就拒绝。一个单词都没有时只写一行注释，由词表校验报「没有任何单词」。
fn render_words(words: &[AuthoredWord]) -> Result<Vec<u8>, PluginError> {
    if words.is_empty() {
        return Ok(b"# empty\n".to_vec());
    }
    let mut text = String::new();
    for (index, entry) in words.iter().enumerate() {
        let number = index + 1;
        let phonetic = entry.phonetic.as_deref().unwrap_or_default();
        if [entry.word.as_str(), phonetic, entry.gloss.as_str()]
            .iter()
            .any(|column| column.contains(['\t', '\n', '\r']))
        {
            return Err(PluginError::Invalid(format!(
                "第 {number} 个单词含有制表符或换行"
            )));
        }
        if entry.word.starts_with('#') {
            return Err(PluginError::Invalid(format!(
                "第 {number} 个单词「{}」不能以 # 开头",
                entry.word
            )));
        }
        text.push_str(&entry.word);
        text.push('\t');
        if !phonetic.is_empty() {
            text.push_str(phonetic);
            text.push('\t');
        }
        text.push_str(&entry.gloss);
        text.push('\n');
    }
    Ok(text.into_bytes())
}

/// 已安装的同 id 包里的说明文件，原样复制进暂存目录。原包读不了或不是普通文件夹时什么都不带：要换掉的正是它。
fn copy_notices(
    root: &Path,
    kind: PluginKind,
    id: &str,
    staging: &Path,
    staging_directory: Option<&fs::File>,
) -> Result<(), PluginError> {
    let directory = kind_directory(root, kind);
    if crate::storage::reject_symlink(&directory).is_err() {
        return Ok(());
    }
    let installed = directory.join(id);
    if !fs::symlink_metadata(&installed).is_ok_and(|metadata| metadata.is_dir()) {
        return Ok(());
    }
    let Ok(files) = list_files(&installed) else {
        return Ok(());
    };
    for (name, size) in files {
        if !is_notice(&name) || size > MAX_NOTICE_BYTES {
            continue;
        }
        let Ok(bytes) = read_file(&installed, &name, MAX_NOTICE_BYTES) else {
            continue;
        };
        write_member(
            staging,
            &name,
            Cursor::new(&bytes),
            bytes.len() as u64,
            staging_directory,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn base(kind: AuthoredKind) -> AuthoredPack {
        AuthoredPack {
            kind,
            id: None,
            name: "合成插件".into(),
            version: "1.0.0".into(),
            license: "CC0-1.0".into(),
            author: Some("合成作者".into()),
            description: Some("一句合成的介绍".into()),
            phrases: None,
            commands: None,
            groups: None,
            words: None,
        }
    }

    fn phrases() -> AuthoredPack {
        AuthoredPack {
            phrases: Some(vec![
                AuthoredPhrase {
                    key: "dh".into(),
                    text: "合成电话".into(),
                },
                AuthoredPhrase {
                    key: "dz".into(),
                    text: "带 \"引号\" 和 \\ 的地址".into(),
                },
            ]),
            ..base(AuthoredKind::PhraseTable)
        }
    }

    fn commands() -> AuthoredPack {
        AuthoredPack {
            commands: Some(vec![AuthoredCommand {
                trigger: "sig".into(),
                title: "签名".into(),
                template: "{date:%Y-%m-%d} 合成".into(),
            }]),
            ..base(AuthoredKind::CommandTable)
        }
    }

    fn symbols() -> AuthoredPack {
        AuthoredPack {
            groups: Some(vec![
                AuthoredGroup {
                    tab: "symbols".into(),
                    title: "箭头".into(),
                    keywords: Some("jiantou arrow".into()),
                    items: vec!["→".into(), "←".into()],
                },
                AuthoredGroup {
                    tab: "kaomoji".into(),
                    title: "开心".into(),
                    keywords: None,
                    items: vec!["(^_^)".into()],
                },
            ]),
            ..base(AuthoredKind::SymbolSet)
        }
    }

    fn words() -> AuthoredPack {
        AuthoredPack {
            words: Some(vec![
                AuthoredWord {
                    word: "apple".into(),
                    phonetic: Some("/ˈæp.əl/".into()),
                    gloss: "n. 苹果".into(),
                },
                AuthoredWord {
                    word: "banana".into(),
                    phonetic: None,
                    gloss: "n. 香蕉".into(),
                },
            ]),
            ..base(AuthoredKind::Wordbook)
        }
    }

    fn generated(id: &str) -> bool {
        id.len() == GENERATED_ID_PREFIX.len() + 8
            && id.starts_with(GENERATED_ID_PREFIX)
            && id[GENERATED_ID_PREFIX.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    #[test]
    fn each_kind_saves_loads_and_saves_again_to_the_same_manifest() {
        for pack in [phrases(), commands(), symbols(), words()] {
            let root = tempdir().unwrap();
            let kind = pack.kind.plugin_kind();
            let saved = save(root.path(), &pack, false).unwrap();
            assert!(generated(&saved.id), "{}", saved.id);
            assert_eq!(saved.kind(), kind);
            let directory = kind_directory(root.path(), kind).join(&saved.id);
            assert_eq!(saved.directory, directory);
            let manifest = fs::read(directory.join(MANIFEST_FILE)).unwrap();

            let loaded = load(root.path(), kind.as_str(), &saved.id).unwrap();
            assert_eq!(
                loaded,
                AuthoredPack {
                    id: Some(saved.id.clone()),
                    ..pack.clone()
                }
            );
            let again = save(root.path(), &loaded, true).unwrap();
            assert_eq!(again.id, saved.id);
            assert_eq!(fs::read(directory.join(MANIFEST_FILE)).unwrap(), manifest);
            assert_eq!(load(root.path(), kind.as_str(), &saved.id).unwrap(), loaded);
            let leftovers: Vec<_> = fs::read_dir(root.path())
                .unwrap()
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(".staging-"))
                .collect();
            assert!(leftovers.is_empty());
        }
    }

    #[test]
    fn the_summary_is_the_one_import_reports() {
        let root = tempdir().unwrap();
        let saved = save(root.path(), &words(), false).unwrap();
        let PluginContent::Wordbook(book) = &saved.content else {
            panic!("not a wordbook");
        };
        assert_eq!(book.file, WORDS_FILE);
        assert_eq!(book.word_count, 2);
        assert_eq!(book.first_words, ["apple", "banana"]);
        assert_eq!(
            fs::read_to_string(saved.directory.join(WORDS_FILE)).unwrap(),
            "apple\t/ˈæp.əl/\tn. 苹果\nbanana\tn. 香蕉\n"
        );
        let listed = super::super::scan(root.path(), BuiltinDirs::default());
        assert_eq!(listed.packages, [saved]);
    }

    #[test]
    fn blank_optional_fields_are_left_out() {
        let root = tempdir().unwrap();
        let pack = AuthoredPack {
            author: Some("  ".into()),
            description: Some(String::new()),
            groups: Some(vec![AuthoredGroup {
                tab: "symbols".into(),
                title: "点".into(),
                keywords: Some(" ".into()),
                items: vec!["·".into()],
            }]),
            ..base(AuthoredKind::SymbolSet)
        };
        let saved = save(root.path(), &pack, false).unwrap();
        let manifest = fs::read_to_string(saved.directory.join(MANIFEST_FILE)).unwrap();
        assert!(!manifest.contains("author"), "{manifest}");
        assert!(!manifest.contains("description"), "{manifest}");
        assert!(!manifest.contains("keywords"), "{manifest}");
        let loaded = load(root.path(), "symbol_set", &saved.id).unwrap();
        assert_eq!(loaded.author, None);
        assert_eq!(loaded.description, None);
        assert_eq!(loaded.groups.unwrap()[0].keywords, None);
    }

    #[test]
    fn a_chosen_id_is_kept_and_an_installed_one_needs_replace() {
        let root = tempdir().unwrap();
        let pack = AuthoredPack {
            id: Some("my-signatures".into()),
            ..commands()
        };
        assert_eq!(save(root.path(), &pack, false).unwrap().id, "my-signatures");
        let changed = AuthoredPack {
            name: "改过的名字".into(),
            ..pack.clone()
        };
        assert!(matches!(
            save(root.path(), &changed, false),
            Err(PluginError::Exists)
        ));
        assert_eq!(
            load(root.path(), "command_table", "my-signatures")
                .unwrap()
                .name,
            "合成插件"
        );
        assert_eq!(
            save(root.path(), &changed, true).unwrap().name,
            "改过的名字"
        );
    }

    #[test]
    fn generated_ids_skip_taken_names() {
        let root = tempdir().unwrap();
        fs::create_dir_all(
            kind_directory(root.path(), PluginKind::PhraseTable).join("my-00000000"),
        )
        .unwrap();
        let mut draws = [[0u8; 4], [0u8; 4], [0xab, 0xcd, 0x01, 0x2f]].into_iter();
        let id = generate_id(root.path(), PluginKind::PhraseTable, || {
            Ok(draws.next().unwrap())
        })
        .unwrap();
        assert_eq!(id, "my-abcd012f");
        // 每次都撞上时放弃，而不是无休止地试下去。
        assert!(matches!(
            generate_id(root.path(), PluginKind::PhraseTable, || Ok([0; 4])),
            Err(PluginError::Storage)
        ));
    }

    #[test]
    fn content_is_held_to_the_import_rules() {
        let root = tempdir().unwrap();
        let invalid = |pack: &AuthoredPack| match save(root.path(), pack, false) {
            Err(PluginError::Invalid(reason)) => reason,
            other => panic!("{other:?}"),
        };
        let bad_key = AuthoredPack {
            phrases: Some(vec![AuthoredPhrase {
                key: "DH".into(),
                text: "电话".into(),
            }]),
            ..phrases()
        };
        assert_eq!(invalid(&bad_key), "短语编码 DH 必须是 1 到 32 个小写字母");
        let empty = AuthoredPack {
            phrases: Some(Vec::new()),
            ..phrases()
        };
        assert_eq!(invalid(&empty), "短语表的条数不在允许范围内");
        let bad_template = AuthoredPack {
            commands: Some(vec![AuthoredCommand {
                trigger: "x".into(),
                title: "x".into(),
                template: "{clipboard}".into(),
            }]),
            ..commands()
        };
        assert_eq!(invalid(&bad_template), "指令 x 的模板有不认识的占位符");
        let bad_tab = AuthoredPack {
            groups: Some(vec![AuthoredGroup {
                tab: "emoji".into(),
                title: "x".into(),
                keywords: None,
                items: vec!["x".into()],
            }]),
            ..symbols()
        };
        assert_eq!(invalid(&bad_tab), "第 1 组的 tab 只能是 symbols 或 kaomoji");
        let no_name = AuthoredPack {
            name: " ".into(),
            ..phrases()
        };
        assert_eq!(invalid(&no_name), "name 的长度或字符不符合要求");
        let bad_license = AuthoredPack {
            license: "随便用".into(),
            ..phrases()
        };
        assert_eq!(invalid(&bad_license), "license 必须是 SPDX 许可证表达式");
        // 单词本的 id 规则比一般插件严：下划线在别的类型里可以，在这里不行。
        let underscored = AuthoredPack {
            id: Some("my_words".into()),
            ..words()
        };
        assert!(invalid(&underscored).starts_with("单词本的 id 只能由"));
        let empty_book = AuthoredPack {
            words: Some(Vec::new()),
            ..words()
        };
        assert_eq!(invalid(&empty_book), "words.tsv 里没有任何单词");
        assert_eq!(
            invalid(&AuthoredPack {
                id: Some("../escape".into()),
                ..phrases()
            }),
            "插件 id 无效"
        );
        // 什么都没装上，也没有留下暂存目录。
        assert!(super::super::scan(root.path(), BuiltinDirs::default())
            .packages
            .is_empty());
    }

    #[test]
    fn words_a_tsv_line_cannot_hold_are_refused_before_writing() {
        let root = tempdir().unwrap();
        let with = |word: &str, gloss: &str| AuthoredPack {
            words: Some(vec![AuthoredWord {
                word: word.into(),
                phonetic: None,
                gloss: gloss.into(),
            }]),
            ..words()
        };
        // 不拦的话，这一个单词会变成两个合规的单词。
        for pack in [with("foo", "bar\nbaz\tqux"), with("a\tb", "x")] {
            assert!(matches!(
                save(root.path(), &pack, false),
                Err(PluginError::Invalid(reason)) if reason == "第 1 个单词含有制表符或换行"
            ));
        }
        assert!(matches!(
            save(root.path(), &with("#tag", "x"), false),
            Err(PluginError::Invalid(reason)) if reason == "第 1 个单词「#tag」不能以 # 开头"
        ));
    }

    #[test]
    fn the_request_names_exactly_its_own_content() {
        let root = tempdir().unwrap();
        let none = base(AuthoredKind::PhraseTable);
        let both = AuthoredPack {
            commands: commands().commands,
            ..phrases()
        };
        let wrong = AuthoredPack {
            kind: AuthoredKind::Wordbook,
            ..phrases()
        };
        for pack in [none, both, wrong] {
            assert!(matches!(
                save(root.path(), &pack, false),
                Err(PluginError::Invalid(_))
            ));
        }
    }

    #[test]
    fn built_in_unsupported_and_missing_packs_are_not_editable() {
        let root = tempdir().unwrap();
        let not_editable = |kind: &str, id: &str| {
            matches!(
                load(root.path(), kind, id),
                Err(PluginError::NotEditable(_))
            )
        };
        assert!(not_editable("phrase_table", "latex-symbols"));
        assert!(not_editable("sound", "default"));
        assert!(not_editable("helpcode", "anything"));
        assert!(not_editable("nonsense", "anything"));
        assert!(not_editable("phrase_table", "not-installed"));
        assert!(not_editable("phrase_table", "../escape"));
        assert!(matches!(
            save(
                root.path(),
                &AuthoredPack {
                    id: Some("latex-symbols".into()),
                    ..phrases()
                },
                true
            ),
            Err(PluginError::Reserved)
        ));
    }

    #[test]
    fn an_imported_pack_keeps_its_notices_when_edited() {
        let root = tempdir().unwrap();
        let picked = tempdir().unwrap();
        let source = picked.path().join("community");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join(MANIFEST_FILE),
            "schema_version = 1\nkind = \"phrase_table\"\nid = \"shared-phrases\"\nname = \"社区短语\"\nversion = \"2.1\"\nlicense = \"CC-BY-4.0\"\nauthor = \"别人\"\npermissions = []\n\n[[phrases]]\nkey = \"hi\"\ntext = \"你好\"\n",
        )
        .unwrap();
        fs::write(source.join("LICENSE.txt"), "合成的许可证全文").unwrap();
        super::super::import(&source, root.path()).unwrap();

        let mut pack = load(root.path(), "phrase_table", "shared-phrases").unwrap();
        assert_eq!(pack.author.as_deref(), Some("别人"));
        pack.phrases.as_mut().unwrap().push(AuthoredPhrase {
            key: "bye".into(),
            text: "再见".into(),
        });
        let saved = save(root.path(), &pack, true).unwrap();
        let PluginContent::PhraseTable(table) = &saved.content else {
            panic!("not a phrase table");
        };
        assert_eq!(table.phrases.len(), 2);
        assert_eq!(
            fs::read_to_string(saved.directory.join("LICENSE.txt")).unwrap(),
            "合成的许可证全文"
        );
    }

    #[test]
    fn the_wire_shape_round_trips_through_json() {
        let root = tempdir().unwrap();
        let saved = save(root.path(), &words(), false).unwrap();
        let loaded = load(root.path(), "wordbook", &saved.id).unwrap();
        let json = serde_json::to_value(&loaded).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "wordbook",
                "id": saved.id,
                "name": "合成插件",
                "version": "1.0.0",
                "license": "CC0-1.0",
                "author": "合成作者",
                "description": "一句合成的介绍",
                "words": [
                    {"word": "apple", "phonetic": "/ˈæp.əl/", "gloss": "n. 苹果"},
                    {"word": "banana", "phonetic": null, "gloss": "n. 香蕉"}
                ]
            })
        );
        assert_eq!(
            serde_json::from_value::<AuthoredPack>(json).unwrap(),
            loaded
        );
        assert!(serde_json::from_value::<AuthoredPack>(serde_json::json!({
            "kind": "sound", "id": null, "name": "x", "version": "1", "license": "CC0-1.0"
        }))
        .is_err());
    }
}
