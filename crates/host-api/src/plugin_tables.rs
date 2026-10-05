//! 从插件目录读进 Engine 选项的表：`/` 模式的指令表、K 模式的短语表、当前方案选中的辅助码表和 `@` 模式的名单。
//!
//! 它们都是设置页在输入进程运行期间写的文件：导入的指令表包、短语表包、辅助码表包和 `mentions.json`。 A session remembers the modification time and length of every file it read, checks them again when a field gains focus and when preferences are applied, and reads the files only when something moved. Nothing is read for a mode that is switched off.

use msime_client_core::plugins::{
    command_table, helpcode_pack, kind_directory, mentions, phrase_table, PluginKind, MANIFEST_FILE,
};
use msime_client_core::preferences::PluginPreferences;
use msime_engine::host::{
    load_helpcode_keymap, CommandTableEntry, EngineOptions, HelpcodeKeymap, MentionEntry,
    QuickPhraseEntry, SharedKeymap,
};
use msime_engine::SchemeType;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

/// What a file looked like when it was read; `None` when it was not there.
type FileStamp = Option<(Option<SystemTime>, u64)>;

fn stamp(path: &Path) -> FileStamp {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    Some((metadata.modified().ok(), metadata.len()))
}

/// 包目录里每个文件的名字和戳，按名字排序：数据文件的名字写在清单里，盖整个目录的戳就不必先读清单。目录不在时为空。
fn directory_stamp(directory: &Path) -> Vec<(String, FileStamp)> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut stamps: Vec<_> = entries
        .flatten()
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                stamp(&entry.path()),
            )
        })
        .collect();
    stamps.sort();
    stamps
}

/// 选中的辅助码表包：包 id、回退的方案名和包目录的戳。
type HelpcodeStamp = (String, String, Vec<(String, FileStamp)>);

/// The files a session's Engine options were filled from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginTables {
    /// The enabled command-table ids with each manifest's stamp, while the `/` mode is on.
    commands: Option<Vec<(String, FileStamp)>>,
    /// `mentions.json`'s stamp, while the `@` mode is on.
    mentions: Option<FileStamp>,
    /// K 模式打开时，启用的短语表 id 与各自清单的戳。
    phrases: Option<Vec<(String, FileStamp)>>,
    /// 辅助码打开、当前方案是全拼或双拼并选了辅助码表包时，包 id、回退用的 `helpcode_schema` 与包目录的戳。回退方案也算在戳里：包坏掉时用的是它，换了方案就要重新决定用哪张表。
    helpcode: Option<HelpcodeStamp>,
}

impl PluginTables {
    /// Stamp the files `options` would be filled from. Cheap: a few `stat`s, and none for a mode that is off or a session without a plugins directory.
    pub(crate) fn stamp(
        root: Option<&Path>,
        options: &EngineOptions,
        plugins: &PluginPreferences,
    ) -> Self {
        let Some(root) = root else {
            return Self::default();
        };
        let manifests = |kind: PluginKind, enabled: &[String]| -> Vec<(String, FileStamp)> {
            let directory = kind_directory(root, kind);
            enabled
                .iter()
                .map(|id| (id.clone(), stamp(&directory.join(id).join(MANIFEST_FILE))))
                .collect()
        };
        Self {
            commands: options
                .local_command
                .then(|| manifests(PluginKind::CommandTable, &plugins.command_tables)),
            mentions: options
                .local_mention
                .then(|| stamp(&mentions::document_path(root))),
            phrases: options
                .local_quick_phrase
                .then(|| manifests(PluginKind::PhraseTable, &plugins.phrase_tables)),
            helpcode: Self::helpcode_pack(options, plugins).map(|id| {
                let directory = kind_directory(root, PluginKind::Helpcode).join(id);
                (
                    id.to_owned(),
                    options.helpcode_schema.clone(),
                    directory_stamp(&directory),
                )
            }),
        }
    }

    /// 当前方案选中的辅助码表包：只有全拼和双拼有辅助码，辅助码关闭时不读包。
    fn helpcode_pack<'a>(
        options: &EngineOptions,
        plugins: &'a PluginPreferences,
    ) -> Option<&'a str> {
        if !options.helpcode {
            return None;
        }
        let pack = match SchemeType::from_u8(options.scheme)? {
            SchemeType::Quanpin => &plugins.helpcode_pack_quanpin,
            SchemeType::Shuangpin => &plugins.helpcode_pack_shuangpin,
            _ => return None,
        };
        (!pack.is_empty()).then_some(pack.as_str())
    }

    pub(crate) fn commands_differ(&self, previous: &Self) -> bool {
        self.commands != previous.commands
    }

    pub(crate) fn mentions_differ(&self, previous: &Self) -> bool {
        self.mentions != previous.mentions
    }

    pub(crate) fn phrases_differ(&self, previous: &Self) -> bool {
        self.phrases != previous.phrases
    }

    pub(crate) fn helpcode_differs(&self, previous: &Self) -> bool {
        self.helpcode != previous.helpcode
    }

    /// 选中的辅助码表包的码表；没选包时为 `None`。包载入失败时也是 `None`，Engine 于是退回方案原来的 `schema`，设置页把这个包报告为未找到；但回退的表本身也读不出来时（典型是已被删掉的 `custom/<stem>`）给一张空表：`None` 会让 Engine 去读那张表，`Session::new` 因此失败，偏好就再也应用不上。
    pub(crate) fn helpcode_table(
        &self,
        root: Option<&Path>,
        options: &EngineOptions,
    ) -> Option<SharedKeymap> {
        let (Some(root), Some((id, schema, _))) = (root, &self.helpcode) else {
            return None;
        };
        let error = match helpcode_pack::load_codes(root, id) {
            Ok(codes) => return Some(Arc::new(HelpcodeKeymap::from_codes(codes))),
            Err(error) => error,
        };
        eprintln!("msime: helpcode pack {id} unavailable, falling back to {schema}: {error}");
        match load_helpcode_keymap(Path::new(&options.resources), schema) {
            Ok(_) => None,
            Err(error) => {
                eprintln!(
                    "msime: helpcode schema {schema} unavailable, using an empty table: {error}"
                );
                Some(Arc::new(HelpcodeKeymap::default()))
            }
        }
    }

    /// 启用的短语表合并后的行；K 模式关闭时为空。
    pub(crate) fn quick_phrase_table(&self, root: Option<&Path>) -> Vec<QuickPhraseEntry> {
        let (Some(root), Some(phrases)) = (root, &self.phrases) else {
            return Vec::new();
        };
        let enabled: Vec<String> = phrases.iter().map(|(id, _)| id.clone()).collect();
        phrase_table::enabled_phrases(root, &enabled)
            .into_iter()
            .map(|row| QuickPhraseEntry {
                key: row.key,
                text: row.text,
            })
            .collect()
    }

    /// The merged rows of the enabled command tables; empty while the `/` mode is off.
    pub(crate) fn command_table(&self, root: Option<&Path>) -> Vec<CommandTableEntry> {
        let (Some(root), Some(commands)) = (root, &self.commands) else {
            return Vec::new();
        };
        let enabled: Vec<String> = commands.iter().map(|(id, _)| id.clone()).collect();
        command_table::enabled_commands(root, &enabled)
            .into_iter()
            .map(|row| CommandTableEntry {
                trigger: row.trigger,
                title: row.title,
                template: row.template,
            })
            .collect()
    }

    /// The saved name list; empty while the `@` mode is off or when the document does not load, which the settings page reports. A Chinese name saved without a key is given the reading of the Engine's dictionary in `options`.
    pub(crate) fn mention_entries(
        &self,
        root: Option<&Path>,
        options: &EngineOptions,
    ) -> Vec<MentionEntry> {
        let (Some(root), Some(_)) = (root, &self.mentions) else {
            return Vec::new();
        };
        let entries = mentions::MentionStore::new(root).load().unwrap_or_default();
        entries
            .into_iter()
            .map(|entry| {
                let key = if entry.key.is_empty() && !entry.text.is_ascii() {
                    msime_engine::host::hanzi_to_pinyin(options, &entry.text)
                } else {
                    entry.key
                };
                MentionEntry {
                    text: entry.text,
                    key,
                }
            })
            .collect()
    }

    /// Fill `options` from the files that moved since `previous` was stamped.
    pub(crate) fn fill(&self, previous: &Self, root: Option<&Path>, options: &mut EngineOptions) {
        if self.commands_differ(previous) {
            options.command_table = self.command_table(root);
        }
        if self.mentions_differ(previous) {
            options.mention_entries = self.mention_entries(root, options);
        }
        if self.phrases_differ(previous) {
            options.quick_phrase_table = self.quick_phrase_table(root);
        }
        if self.helpcode_differs(previous) {
            options.helpcode_table = self.helpcode_table(root, options);
        }
    }
}
