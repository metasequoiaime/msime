//! 从插件目录读进 Engine 选项的表：`/` 模式的指令表、K 模式的短语表、当前方案选中的辅助码表和 `@` 模式的名单。
//!
//! 它们都是设置页在输入进程运行期间写的文件：导入的指令表包、短语表包、辅助码表包和 `mentions.json`。会话记下读过的每个文件的修改时间、长度和（Unix 上的）inode，输入框获得焦点和偏好应用时再看一遍，有变动才重新读。关着的模式不读其专属表；联系人名单独立于 `@` 模式装载，供普通拼音匹配。
//!
//! 读出来的表按同样的戳在进程里各留一份（见 [`cached`]）：Android 每次输入框获得焦点都销毁再新建会话，新会话拿 `PluginTables::default()` 对照，每张表都算变了；来源没动时直接用上一个会话读出的那份，不再重读文件，也不再为每个没填拼音的联系人姓名把主词库各张表扫一遍。

use msime_client_core::preferences::PluginPreferences;
use msime_client_services::plugins::{
    command_table, helpcode_pack, kind_directory, mentions, pack_directory, phrase_table,
    BuiltinDirs, PluginKind, MANIFEST_FILE,
};
use msime_engine::host::{
    load_helpcode_keymap, CommandTableEntry, EngineOptions, HelpcodeKeymap, MentionEntry,
    QuickPhraseEntry, SharedKeymap,
};
use msime_engine::SchemeType;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

/// 读文件时它的样子：修改时间、长度和（Unix 上的）inode；文件不在时为 `None`。重新导入会把整个包目录换掉，新清单与旧清单可能长度相同、修改时间又落在同一个计时粒度里（秒级时间戳的文件系统），只有 inode 不同，与 `key_sound::PackStamp` 同理。
type FileStamp = Option<(Option<SystemTime>, u64, u64)>;

fn stamp(path: &Path) -> FileStamp {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&metadata);
    #[cfg(not(unix))]
    let inode = 0;
    Some((metadata.modified().ok(), metadata.len(), inode))
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

/// 进程里最近一次读出的一种表，连同读它时的来源（插件目录和相关文件的戳）。每种表只留最近一份：同一进程里先后建的会话通常是同一套设置。
type Slot<K, V> = Mutex<Option<(K, V)>>;

/// 一组包各自的 id 和戳：启用的表各自清单的戳，或辅助码表包目录里各文件的戳。
type PackStamps = Vec<(String, FileStamp)>;

/// 启用的指令表：插件目录、各清单的戳。
static COMMAND_TABLES: Slot<(PathBuf, PackStamps), Vec<CommandTableEntry>> = Mutex::new(None);
/// 启用的短语表：插件目录、官方插件包目录、各清单的戳。
static PHRASE_TABLES: Slot<(PathBuf, Option<PathBuf>, PackStamps), Vec<QuickPhraseEntry>> =
    Mutex::new(None);
/// 联系人名单：插件目录、`mentions.json` 的戳、查读音的主词库路径和它的戳。主词库是代次里可写的工作副本，换代次或词库变了，没填拼音的姓名就要重新查读音。
static MENTION_ENTRIES: Slot<(PathBuf, FileStamp, PathBuf, FileStamp), Vec<MentionEntry>> =
    Mutex::new(None);
/// 选中的辅助码表包：插件目录、包 id、包目录的戳。只留载入成功的表。
static HELPCODE_TABLES: Slot<(PathBuf, String, PackStamps), Result<SharedKeymap, String>> =
    Mutex::new(None);

/// 放掉进程里为下一个会话留着的四种表（宿主的清缓存、内存告警：`msime_client_reset_cache`、`msime_client_release_process_cache`）。开着的会话各自持有读进 Engine 的那份，不受影响；之后新建的会话照来源重读一遍文件。
pub(crate) fn release_cached_tables() {
    *COMMAND_TABLES
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    *PHRASE_TABLES.lock().unwrap_or_else(PoisonError::into_inner) = None;
    *MENTION_ENTRIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    *HELPCODE_TABLES
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
}

/// `key` 与 `slot` 记下的来源相同时返回记下的表；否则调 `load` 读一遍，它说可以留（第二项为 `true`）时记下。读文件时不持锁。
fn cached<K: PartialEq, V: Clone>(
    slot: &Slot<K, V>,
    key: K,
    load: impl FnOnce() -> (V, bool),
) -> V {
    if let Some((cached_key, value)) = &*slot.lock().unwrap_or_else(PoisonError::into_inner) {
        if *cached_key == key {
            return value.clone();
        }
    }
    let (value, keep) = load();
    if keep {
        *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some((key, value.clone()));
    }
    value
}

/// The files a session's Engine options were filled from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginTables {
    /// The enabled command-table ids with each manifest's stamp, while the `/` mode is on.
    commands: Option<Vec<(String, FileStamp)>>,
    /// 本地名单的文件戳；普通联系人候选不依赖 `@` 模式开关。
    mentions: Option<FileStamp>,
    /// K 模式打开时，启用的短语表 id 与各自清单的戳。
    phrases: Option<Vec<(String, FileStamp)>>,
    /// 辅助码打开、当前方案是全拼或双拼并选了辅助码表包时，包 id、回退用的 `helpcode_schema` 与包目录的戳。回退方案也算在戳里：包坏掉时用的是它，换了方案就要重新决定用哪张表。
    helpcode: Option<HelpcodeStamp>,
    /// 官方插件包目录（`PluginRoots::builtin_packs`）：启用的官方短语表从这里读、在这里盖戳。会话里不变。
    builtin_packs: Option<PathBuf>,
}

impl PluginTables {
    /// 给选项来源盖戳：只做少量 `stat`；未启用的模式专属表和没有插件目录的会话不读取文件。
    pub(crate) fn stamp(
        root: Option<&Path>,
        builtin_packs: Option<&Path>,
        options: &EngineOptions,
        plugins: &PluginPreferences,
    ) -> Self {
        let Some(root) = root else {
            return Self::default();
        };
        // 官方短语表的清单在内置目录里，其余在插件目录里；内置目录没有时它没有戳（读不到）。
        let manifests = |kind: PluginKind, enabled: &[String]| -> Vec<(String, FileStamp)> {
            enabled
                .iter()
                .map(|id| {
                    let manifest =
                        pack_directory(root, BuiltinDirs::packs(builtin_packs), kind, id)
                            .map(|directory| directory.join(MANIFEST_FILE));
                    (id.clone(), manifest.as_deref().and_then(stamp))
                })
                .collect()
        };
        Self {
            commands: options
                .local_command
                .then(|| manifests(PluginKind::CommandTable, &plugins.command_tables)),
            mentions: Some(stamp(&mentions::document_path(root))),
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
            builtin_packs: builtin_packs.map(Path::to_path_buf),
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
        let (Some(root), Some((id, schema, directory))) = (root, &self.helpcode) else {
            return None;
        };
        // 载入失败不记下：下一个会话照旧重试，并照旧报告。
        let loaded = cached(
            &HELPCODE_TABLES,
            (root.to_path_buf(), id.clone(), directory.clone()),
            || {
                let loaded = helpcode_pack::load_codes(root, id)
                    .map(|codes| Arc::new(HelpcodeKeymap::from_codes(codes)));
                let keep = loaded.is_ok();
                (loaded, keep)
            },
        );
        let error = match loaded {
            Ok(table) => return Some(table),
            Err(error) => error,
        };
        crate::diagnostics::report(
            "helpcode pack unavailable, falling back to the scheme's schema",
            &format!("{id} -> {schema}: {error}"),
        );
        match load_helpcode_keymap(Path::new(&options.resources), schema) {
            Ok(_) => None,
            Err(error) => {
                crate::diagnostics::report(
                    "helpcode schema unavailable, using an empty table",
                    &format!("{schema}: {error}"),
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
        let key = (
            root.to_path_buf(),
            self.builtin_packs.clone(),
            phrases.clone(),
        );
        cached(&PHRASE_TABLES, key, || {
            let enabled: Vec<String> = phrases.iter().map(|(id, _)| id.clone()).collect();
            let table =
                phrase_table::enabled_phrases(root, self.builtin_packs.as_deref(), &enabled)
                    .into_iter()
                    .map(|row| QuickPhraseEntry {
                        key: row.key,
                        text: row.text,
                    })
                    .collect();
            (table, true)
        })
    }

    /// The merged rows of the enabled command tables; empty while the `/` mode is off.
    pub(crate) fn command_table(&self, root: Option<&Path>) -> Vec<CommandTableEntry> {
        let (Some(root), Some(commands)) = (root, &self.commands) else {
            return Vec::new();
        };
        cached(
            &COMMAND_TABLES,
            (root.to_path_buf(), commands.clone()),
            || {
                let enabled: Vec<String> = commands.iter().map(|(id, _)| id.clone()).collect();
                let table = command_table::enabled_commands(root, &enabled)
                    .into_iter()
                    .map(|row| CommandTableEntry {
                        trigger: row.trigger,
                        title: row.title,
                        template: row.template,
                    })
                    .collect();
                (table, true)
            },
        )
    }

    /// 本地保存的名单；无插件目录或读取失败时为空。未填拼音的中文姓名用引擎词典补全读音，与 `@` 模式开关无关。
    pub(crate) fn mention_entries(
        &self,
        root: Option<&Path>,
        options: &EngineOptions,
    ) -> Vec<MentionEntry> {
        let (Some(root), Some(document)) = (root, self.mentions) else {
            return Vec::new();
        };
        let dictionary = msime_engine::host::main_dictionary_path(options);
        let dictionary_stamp = stamp(&dictionary);
        let key = (root.to_path_buf(), document, dictionary, dictionary_stamp);
        cached(&MENTION_ENTRIES, key, || {
            // 读不出名单时给空表，不记下：下一个会话再读。
            let Ok(entries) = mentions::MentionStore::new(root).load() else {
                return (Vec::new(), false);
            };
            let names: Vec<&str> = entries
                .iter()
                .map(|entry| {
                    if entry.key.is_empty() && !entry.text.is_ascii() {
                        entry.text.as_str()
                    } else {
                        ""
                    }
                })
                .collect();
            let readings = msime_engine::host::hanzi_to_pinyin_batch(options, &names);
            // 要查读音的姓名一个也没查到，多半是主词库没打开：不记下，下一个会话再查。个别生僻字查不到读音不妨碍记下其余的。
            let mut looked_up = names
                .iter()
                .zip(&readings)
                .filter(|(name, _)| !name.is_empty())
                .peekable();
            let keep =
                looked_up.peek().is_none() || looked_up.any(|(_, reading)| !reading.is_empty());
            let entries = entries
                .into_iter()
                .zip(readings)
                .map(|(entry, reading)| {
                    let key = if entry.key.is_empty() && !entry.text.is_ascii() {
                        reading
                    } else {
                        entry.key
                    };
                    MentionEntry {
                        text: entry.text,
                        key,
                        phone: entry.phone,
                    }
                })
                .collect();
            (entries, keep)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 来源没动时直接用记下的表，不再读；来源变了重读并换下记下的那份；`load` 说不留的结果不记下，下次照样重读。
    #[test]
    fn a_table_is_read_again_only_when_its_source_moved() {
        let slot: Slot<u32, &str> = Mutex::new(None);
        let reads = Cell::new(0);
        let read = |key: u32, value: &'static str, keep: bool| {
            cached(&slot, key, || {
                reads.set(reads.get() + 1);
                (value, keep)
            })
        };
        assert_eq!(read(1, "first", true), "first");
        assert_eq!(read(1, "unused", true), "first");
        assert_eq!(reads.get(), 1);
        assert_eq!(read(2, "second", true), "second");
        assert_eq!(read(1, "first again", true), "first again");
        assert_eq!(reads.get(), 3);
        assert_eq!(read(3, "failed", false), "failed");
        assert_eq!(read(3, "retried", true), "retried");
        assert_eq!(read(3, "unused", true), "retried");
        assert_eq!(reads.get(), 5);
    }
}
