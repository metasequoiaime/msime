//! 插件包：音效包、背景音乐、指令表、打字特效参数、短语表、辅助码表、单词本和符号集，全部是经过校验的数据。
//!
//! 数据类插件包里没有任何会运行的东西。类型是封闭集合，每种类型的清单形状固定、由本模块完整解析，申请任何权限的数据包一律拒绝，所以第三方只能提供音频样本、曲目和文本模板。宿主播放音频、把指令行交给 Engine、用特效包的参数画自己内置的特效；它们看到的都是本模块接受过的包。
//!
//! 唯一的例外是桌面上的脚本插件（`kind = "script"`，见 `script_pack`）：它带一个 core wasm 模块，只在桌面输入法进程里由沙箱解释器按用户逐项授予的能力运行。它只在 macOS、Windows 和非 ohos 的 Linux 上编译，清单必须写 `schema_version = 2`，`permissions` 只对它放行白名单里的能力；授权记在 `script-grants.json`（`script_grants`）。移动端没有这个类型，上面的原则在移动端原样成立。
//!
//! 一个包放在 `<root>/<kind>/<id>/`：`root` 是宿主状态根目录下的 `plugins` 目录，`kind` 是 `PluginKind::as_str` 给出的名字（`sound`、`music`、`command_table`、`effect`、`phrase_table` 等）。目录里只有 `plugin.toml`、清单点名的平铺文件（音频或数据文件）和可选的说明文本，别的一律没有：没有子目录、没有符号链接、没有清单不认账的文件。内置音效包随各平台的安装包分发，不在 `root` 下，而在宿主指定的目录里（仓库中是 `resources/sound-packs`），与已安装的包并列列出。桌面安装包另外随带官方插件包（`plugins/official-packs.json` 登记的脚本插件和短语表），在宿主指定的只读目录里按 `<kind>/<id>/` 排（[`BuiltinDirs::packs`]）；它们的 id 和内置音效包一样保留，载入时永远从那里读，默认停用。
//!
//! `mentions.json` beside the kind directories is the @ mode's name list, kept by `mentions`. It lives here rather than in the preferences document because that document is the one hosts copy and account sync reads from, and a contact list belongs to neither.

pub mod command_table;
pub mod community;
pub mod effect_pack;
mod failure;
pub mod helpcode_pack;
mod import;
pub mod mentions;
pub mod music_pack;
pub mod phrase_table;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub mod script_commands;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub mod script_grants;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub mod script_pack;
pub mod sound_pack;
pub mod symbol_set;
pub mod wordbook_pack;

pub use failure::{remove_named, PluginFailure};
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub use import::load_script_source;
pub use import::{
    import, import_from_settings, import_without_script_page, validate, validate_with, ReservedIds,
};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use toml::Value;

use crate::skin::catalog::{contained, safe_id};

/// The manifest every pack carries.
pub const MANIFEST_FILE: &str = "plugin.toml";
/// Bytes of `plugin.toml`. A command table with every row at its limits fits well inside.
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
/// Entries in a pack directory, the manifest and notices included.
pub const MAX_PACK_FILES: usize = 16;
/// Bytes of one text notice (`*.txt`, `*.md`), which is all a pack may carry besides its manifest and audio.
pub const MAX_NOTICE_BYTES: u64 = 64 * 1024;

/// Sound pack ids the bundle ships: key packs first, then melodies. An installed pack may not take one, so a selected id always names the same pack on every machine.
pub const BUILTIN_SOUND_PACKS: [&str; 9] = [
    "default",
    "twinkle",
    "msime-typewriter",
    "msime-bubble",
    "msime-8bit",
    "msime-woodblock",
    "msime-pentatonic",
    "msime-canon",
    "msime-ode-to-joy",
];
/// Music pack ids the bundle ships, in the same built-in directory as the sound packs, reserved the same way.
pub const BUILTIN_MUSIC_PACKS: [&str; 2] = ["msime-music-lofi", "msime-music-ambient"];
/// 桌面安装包随带的官方脚本插件（`plugins/official-packs.json`）。和内置音效包一样保留：导入的包不能用这些 id，这个 id 永远从内置目录（[`BuiltinDirs::packs`]）读。默认停用，要在授权页逐项授权才运行。
pub const BUILTIN_SCRIPT_PACKS: [&str; 1] = ["uuid"];
/// 桌面安装包随带的官方短语表（`plugins/official-packs.json`），保留方式同 [`BUILTIN_SCRIPT_PACKS`]。不在 `phrase_tables` 偏好里就不进 K 模式。
pub const BUILTIN_PHRASE_TABLES: [&str; 1] = ["latex-symbols"];
/// 定义在 `preferences`（偏好的默认值要用，放在这里会让 `preferences` 依赖插件模块），从这里导回。
pub use crate::preferences::{DEFAULT_MELODY_PACK, DEFAULT_SOUND_PACK};

/// Commit counts at which a host with achievements switched on plays the achievement sample.
pub const ACHIEVEMENT_MILESTONES: [u64; 9] = [
    100, 1_000, 5_000, 10_000, 50_000, 100_000, 500_000, 1_000_000, 10_000_000,
];

/// The milestone a commit count passed on its way from `before` to `after`, the largest when it passed several at once. `None` when it passed none, including when the count went down.
pub fn achievement_milestone(before: u64, after: u64) -> Option<u64> {
    ACHIEVEMENT_MILESTONES
        .iter()
        .rev()
        .copied()
        .find(|milestone| before < *milestone && *milestone <= after)
}

/// What a pack supplies. Closed: a manifest naming anything else is listed as an issue, never loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Key, commit and achievement samples, or one sample a melody is played on.
    Sound,
    /// Background music tracks, streamed by the host rather than decoded up front.
    Music,
    /// `/` commands: a trigger, a title and a template of literal text and clock placeholders.
    CommandTable,
    /// Parameters for one of the hosts' built-in typing effects: a style and a few bounded hints, no files.
    Effect,
    /// K 模式的短语：编码和文本，全部写在清单里。
    PhraseTable,
    /// 辅助码表：一个 `.txt` 数据文件，替换全拼或双拼方案的辅助码。
    Helpcode,
    /// 单词本：一个 `.tsv` 数据文件，作为一本词书出现在背单词里。
    Wordbook,
    /// 符号集：追加到符号面板的符号组和颜文字组，全部写在清单里。
    SymbolSet,
    /// 脚本插件：一个 core wasm 模块，由桌面输入法进程的沙箱解释器按授权运行。只在桌面存在，移动端的 `ALL`、`parse` 和各处穷举都看不到它。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    Script,
}

impl PluginKind {
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    pub const ALL: [Self; 9] = [
        Self::Sound,
        Self::Music,
        Self::CommandTable,
        Self::Effect,
        Self::PhraseTable,
        Self::Helpcode,
        Self::Wordbook,
        Self::SymbolSet,
        Self::Script,
    ];
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    pub const ALL: [Self; 8] = [
        Self::Sound,
        Self::Music,
        Self::CommandTable,
        Self::Effect,
        Self::PhraseTable,
        Self::Helpcode,
        Self::Wordbook,
        Self::SymbolSet,
    ];

    /// The manifest's `kind` and the directory under the plugins root.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sound => "sound",
            Self::Music => "music",
            Self::CommandTable => "command_table",
            Self::Effect => "effect",
            Self::PhraseTable => "phrase_table",
            Self::Helpcode => "helpcode",
            Self::Wordbook => "wordbook",
            Self::SymbolSet => "symbol_set",
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            Self::Script => "script",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// 是不是脚本插件。移动端没有这个类型，任何值的 `as_str` 都不是 `script`，所以恒为 false；按名字比较让这里不必为两种 cfg 各写一份。
    pub fn is_script(self) -> bool {
        self.as_str() == "script"
    }
}

// 移动端的类型集合不变：仍是原来那 8 种，`script` 在那里不是已知类型，`scan` 和遗留目录清扫也不会碰 `script/`。写成编译期断言，任何目标只要编译 client-services 就检查一次，不依赖哪台 CI 机器跑了测试。
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
const _: () = assert!(PluginKind::ALL.len() == 8);
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
const _: () = assert!(PluginKind::ALL.len() == 9);

/// `id` 是不是安装包随带的 `kind` 类包：音效包和背景音乐包（所有平台的安装包都带），以及官方脚本插件和官方短语表（只有桌面安装包带）。保留的 id 在所有平台上都生效：导入的包不能用它，载入时永远从内置目录读。特效包没有内置的：样式本身在宿主里，`effect_style` 不靠包就能选。
pub fn is_builtin(kind: PluginKind, id: &str) -> bool {
    match kind {
        PluginKind::Sound => BUILTIN_SOUND_PACKS.contains(&id),
        PluginKind::Music => BUILTIN_MUSIC_PACKS.contains(&id),
        PluginKind::PhraseTable => BUILTIN_PHRASE_TABLES.contains(&id),
        PluginKind::CommandTable
        | PluginKind::Effect
        | PluginKind::Helpcode
        | PluginKind::Wordbook
        | PluginKind::SymbolSet => false,
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        PluginKind::Script => BUILTIN_SCRIPT_PACKS.contains(&id),
    }
}

/// `id` 是不是官方插件包：内置的、但不是音效包或背景音乐包。它放在 [`BuiltinDirs::packs`] 下的 `<kind>/<id>/`，而不是音效包那个目录。
pub fn is_official(kind: PluginKind, id: &str) -> bool {
    is_builtin(kind, id) && !matches!(kind, PluginKind::Sound | PluginKind::Music)
}

/// 宿主随安装包带的两个内置目录，都是只读的（macOS 在签了名的 bundle 里，Windows 在只有管理员能写的 Program Files 里，Linux 在系统前缀下）。宿主没有时为 `None`，例如移动端和不带官方包的开发构建：这时对应的内置包列不出来，也载不入。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinDirs<'a> {
    /// 内置音效包和背景音乐包：每个包一个 `<id>/` 文件夹（仓库里是 `resources/sound-packs`）。
    pub sounds: Option<&'a Path>,
    /// 官方插件包：按 `<kind>/<id>/` 排，布局同用户的插件目录（构建产物是 `target/plugin-packs`）。
    pub packs: Option<&'a Path>,
}

impl<'a> BuiltinDirs<'a> {
    /// 只有官方插件包目录。
    pub fn packs(packs: Option<&'a Path>) -> Self {
        Self {
            sounds: None,
            packs,
        }
    }
}

/// `kind` 类的包 `id` 所在的上级目录，以及它是不是内置的：内置 id 是内置目录（宿主没有这个目录时报错），其他 id 是 `<root>/<kind>`。
fn pack_parent(
    root: &Path,
    builtin: BuiltinDirs<'_>,
    kind: PluginKind,
    id: &str,
) -> Result<(PathBuf, bool), String> {
    if is_official(kind, id) {
        let packs = builtin.packs.ok_or("内置插件包不可用")?;
        Ok((kind_directory(packs, kind), true))
    } else if is_builtin(kind, id) {
        let sounds = builtin.sounds.ok_or("内置音效包不可用")?;
        Ok((sounds.to_path_buf(), true))
    } else {
        Ok((kind_directory(root, kind), false))
    }
}

/// `kind` 类的包 `id` 的文件夹，按载入时的规则（[`load_package`]）找：内置 id 在内置目录里，其他 id 在 `<root>/<kind>/<id>`。不检查它在不在；内置目录没有时为 `None`。宿主按它盖清单的戳。
pub fn pack_directory(
    root: &Path,
    builtin: BuiltinDirs<'_>,
    kind: PluginKind,
    id: &str,
) -> Option<PathBuf> {
    pack_parent(root, builtin, kind, id)
        .ok()
        .map(|(parent, _)| parent.join(id))
}

/// 定义在 `preferences`（`PluginPreferences::effect_style` 的类型），从这里导回。
pub use crate::preferences::EffectStyle;

/// A combo starts over after the keyboard has been quiet this long, as a melody does.
pub const COMBO_IDLE_RESET_MILLIS: u64 = sound_pack::MELODY_IDLE_RESET_MILLIS;
/// Combo counts that move it up a tier, lowest first.
pub const COMBO_MILESTONES: [u32; 4] = [10, 25, 50, 100];

/// One loadable pack.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The licence the manifest declares, shown beside the pack. Required, and informational: nothing is enforced from it.
    pub license: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// Shipped in the bundle rather than installed; cannot be removed.
    pub builtin: bool,
    /// The pack's directory. Every file the content names is a plain file name inside it.
    #[serde(skip)]
    pub directory: PathBuf,
    #[serde(flatten)]
    pub content: PluginContent,
}

impl PluginSummary {
    pub fn kind(&self) -> PluginKind {
        match self.content {
            PluginContent::Sound(_) => PluginKind::Sound,
            PluginContent::Music(_) => PluginKind::Music,
            PluginContent::CommandTable(_) => PluginKind::CommandTable,
            PluginContent::Effect(_) => PluginKind::Effect,
            PluginContent::PhraseTable(_) => PluginKind::PhraseTable,
            PluginContent::Helpcode(_) => PluginKind::Helpcode,
            PluginContent::Wordbook(_) => PluginKind::Wordbook,
            PluginContent::SymbolSet(_) => PluginKind::SymbolSet,
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            PluginContent::Script(_) => PluginKind::Script,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginContent {
    Sound(sound_pack::SoundPack),
    Music(music_pack::MusicPack),
    CommandTable(command_table::CommandTable),
    Effect(effect_pack::EffectPack),
    PhraseTable(phrase_table::PhraseTable),
    Helpcode(helpcode_pack::HelpcodePack),
    Wordbook(wordbook_pack::WordbookPack),
    SymbolSet(symbol_set::SymbolSet),
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    Script(script_pack::ScriptPack),
}

impl PluginCatalog {
    /// 去掉脚本插件的包和问题行。host-api 的 `msime_client_plugins` 目录命令在请求不带 `script_page` 时经过这里：这样调用的宿主（Android、HarmonyOS）没有授权页。桌面外壳的目录命令和带 `script_page` 的 Windows 原生设置应用列出脚本插件，由页面按 `script_plugins` 决定展示与否；移动端没有脚本插件，原样返回。
    pub fn without_scripts(mut self) -> Self {
        self.packages.retain(|package| !package.kind().is_script());
        self.issues.retain(|issue| !issue.kind.is_script());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginIssue {
    /// The kind directory the folder sits in.
    pub kind: PluginKind,
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct PluginCatalog {
    pub packages: Vec<PluginSummary>,
    pub issues: Vec<PluginIssue>,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// The pack breaks a rule `scan` would list it under; the text says which.
    #[error("plugin_invalid: {0}")]
    Invalid(String),
    /// The picked path is neither a folder nor a `.zip` archive.
    #[error("plugin_unsupported_source")]
    UnsupportedSource,
    /// The archive could not be read, or holds something a pack may not.
    #[error("plugin_archive: {0}")]
    Archive(String),
    /// The id belongs to a pack the bundle ships.
    #[error("plugin_reserved")]
    Reserved,
    /// The plugins root or a kind directory is a symbolic link or not a directory.
    #[error("plugin_storage")]
    Storage,
    #[error("plugin_io: {0}")]
    Io(#[from] std::io::Error),
}

/// 清单的 `kind` 不在 `PluginKind::ALL` 里时的拒绝原因；不接受脚本包的设置导入（`import_without_script_page` 的 release 构建）拒绝脚本包时也用它。
pub(crate) const UNKNOWN_KIND: &str = "kind 不是已知的插件类型";

/// `<root>/<kind>`.
pub fn kind_directory(root: &Path, kind: PluginKind) -> PathBuf {
    root.join(kind.as_str())
}

/// Every pack under `root`, and the built-in packs in `builtin` when the host has them (sound and music packs under `sounds`, official packs under `packs`), with what is wrong with each folder that is not a pack. Folders whose name starts with a dot are install and removal leftovers and are not reported.
pub fn scan(root: &Path, builtin: BuiltinDirs<'_>) -> PluginCatalog {
    let mut catalog = PluginCatalog::default();
    if let Some(sounds) = builtin.sounds {
        scan_builtin(sounds, &mut catalog);
    }
    if let Some(packs) = builtin.packs {
        scan_official(packs, &mut catalog);
    }
    // Do not enumerate through an untrusted root link: even though each pack
    // is checked again below, read_dir would otherwise expose names from
    // outside the host's plugins directory as catalog issues.
    if crate::storage::reject_symlink(root).is_err() {
        return catalog;
    }
    for kind in PluginKind::ALL {
        let directory = kind_directory(root, kind);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() => scan_kind(&directory, kind, false, &mut catalog),
            Ok(_) => catalog.issues.push(PluginIssue {
                kind,
                folder: String::new(),
                reason: "插件类别目录不是文件夹".into(),
            }),
            Err(_) => {}
        }
    }
    catalog.packages.sort_by(|a, b| {
        (a.kind(), !a.builtin, &a.name, &a.id).cmp(&(b.kind(), !b.builtin, &b.name, &b.id))
    });
    catalog
        .issues
        .sort_by(|a, b| (a.kind, &a.folder).cmp(&(b.kind, &b.folder)));
    catalog
}

/// The bundle's built-in packs. They share one directory, so each folder's kind is the one its id is reserved for: a music id is loaded as music, and anything else as a sound pack.
fn scan_builtin(directory: &Path, catalog: &mut PluginCatalog) {
    // Do not enumerate through an untrusted resource link: load_installed
    // rejects each pack below, but read_dir would still expose outside names
    // as catalog issues.
    if crate::storage::reject_symlink(directory).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let folder = entry.file_name().to_string_lossy().into_owned();
        if folder.starts_with('.') {
            continue;
        }
        let kind = if is_builtin(PluginKind::Music, &folder) {
            PluginKind::Music
        } else {
            PluginKind::Sound
        };
        let loaded = match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => {
                load_installed(directory, &folder, kind, true, DataFiles::Summarize)
            }
            _ => Err("不是插件文件夹".to_owned()),
        };
        match loaded {
            Ok(package) => catalog.packages.push(package),
            Err(reason) => catalog.issues.push(PluginIssue {
                kind,
                folder,
                reason,
            }),
        }
    }
}

/// 官方插件包：`packs` 下按 `<kind>/<id>/` 排，布局同用户的插件目录。只认官方 id（[`is_official`]）：别的文件夹不会被 [`load_package`] 从这里读到，列出来就成了一个用不了的「内置」包，所以报为问题。
fn scan_official(packs: &Path, catalog: &mut PluginCatalog) {
    // 同 `scan_builtin`：不经过符号链接列目录。
    if crate::storage::reject_symlink(packs).is_err() {
        return;
    }
    for kind in PluginKind::ALL {
        let directory = kind_directory(packs, kind);
        if crate::storage::reject_symlink(&directory).is_err() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let folder = entry.file_name().to_string_lossy().into_owned();
            if folder.starts_with('.') {
                continue;
            }
            let loaded = match entry.file_type() {
                Ok(_) if !is_official(kind, &folder) => Err("不是随输入法分发的插件".to_owned()),
                Ok(file_type) if file_type.is_dir() => {
                    load_installed(&directory, &folder, kind, true, DataFiles::Summarize)
                }
                _ => Err("不是插件文件夹".to_owned()),
            };
            match loaded {
                Ok(package) => catalog.packages.push(package),
                Err(reason) => catalog.issues.push(PluginIssue {
                    kind,
                    folder,
                    reason,
                }),
            }
        }
    }
}

/// `root` 下某一类型能载入的已安装包，载不入的略过：只要一种类型时不必读整个插件目录。
pub(crate) fn scan_kind_packages(root: &Path, kind: PluginKind) -> Vec<PluginSummary> {
    let directory = kind_directory(root, kind);
    if !fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.is_dir()) {
        return Vec::new();
    }
    let mut catalog = PluginCatalog::default();
    scan_kind(&directory, kind, false, &mut catalog);
    catalog.packages
}

fn scan_kind(directory: &Path, kind: PluginKind, builtin: bool, catalog: &mut PluginCatalog) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let folder = entry.file_name().to_string_lossy().into_owned();
        if folder.starts_with('.') {
            continue;
        }
        let loaded = match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => {
                load_installed(directory, &folder, kind, builtin, DataFiles::Summarize)
            }
            _ => Err("不是插件文件夹".to_owned()),
        };
        match loaded {
            Ok(package) => catalog.packages.push(package),
            Err(reason) => catalog.issues.push(PluginIssue {
                kind,
                folder,
                reason,
            }),
        }
    }
}

/// Validate one pack by the rules `scan` lists packs by, without reading the rest of the root. Hosts resolve the selected pack through this. A built-in sound or music pack id is read from `builtin.sounds` and an official pack id from `builtin.packs`, never from `root`.
pub fn load_package(
    root: &Path,
    builtin: BuiltinDirs<'_>,
    kind: PluginKind,
    id: &str,
) -> Result<PluginSummary, String> {
    if !safe_id(id) {
        return Err("插件 id 无效".into());
    }
    let (parent, builtin) = pack_parent(root, builtin, kind, id)?;
    load_installed(&parent, id, kind, builtin, DataFiles::Summarize)
}

/// 与 [`load_package`] 同样校验一个已安装的包（清单、文件清单、数据文件的存在与大小），但不读数据文件的内容：`content` 里的摘要是占位值。只给自己要完整解析数据文件的调用方用（`wordbook_pack::load_book`、`helpcode_pack::load_codes`），它们的解析就是对内容的校验，这样一个最大 4 MiB 的文件只解析一遍。
pub(crate) fn load_package_unread(
    root: &Path,
    builtin: BuiltinDirs<'_>,
    kind: PluginKind,
    id: &str,
) -> Result<PluginSummary, String> {
    if !safe_id(id) {
        return Err("插件 id 无效".into());
    }
    let (parent, builtin) = pack_parent(root, builtin, kind, id)?;
    load_installed(&parent, id, kind, builtin, DataFiles::Unread)
}

/// 载入包时是否读出数据文件（辅助码表、单词本、脚本模块）的内容。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataFiles {
    /// 读出并严格解析，摘要（条数、预览）写进 `content`；脚本模块读出后按 ABI v1 校验（只编译，不执行），并算出内容哈希。
    Summarize,
    /// 只检查文件存在、大小和扩展名，内容由调用方自己解析。脚本包的内容哈希留空。
    Unread,
    /// 只给脚本包用：读出模块、算出内容哈希，字节随摘要交回调用方，由调用方自己编译。编译就是同一份 ABI 校验，这里不重复做；哈希和交出去的字节出自同一次读取。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    Module,
}

/// A pack in `<directory>/<folder>`, whose manifest must name `folder` as its id and `kind` as its kind.
fn load_installed(
    directory: &Path,
    folder: &str,
    kind: PluginKind,
    builtin: bool,
    data_files: DataFiles,
) -> Result<PluginSummary, String> {
    crate::storage::reject_symlink(directory).map_err(|_| "插件所在目录是符号链接".to_owned())?;
    if !safe_id(folder) {
        return Err("插件 id 无效".into());
    }
    if !builtin && is_builtin(kind, folder) {
        return Err("这个 id 属于内置插件".into());
    }
    let package = directory.join(folder);
    let metadata = fs::symlink_metadata(&package).map_err(|_| "插件文件夹不存在")?;
    if !metadata.is_dir() {
        return Err("不是插件文件夹".into());
    }
    if !contained(directory, &package) {
        return Err("插件指向了所在目录之外".into());
    }
    let mut summary = load_directory_with(&package, data_files)?;
    if summary.id != folder {
        return Err("plugin.toml 里的 id 与文件夹名不一致".into());
    }
    if summary.kind() != kind {
        return Err("plugin.toml 里的 kind 与所在目录不一致".into());
    }
    summary.builtin = builtin;
    Ok(summary)
}

/// The top-level manifest keys every kind shares.
const COMMON_KEYS: [&str; 9] = [
    "schema_version",
    "kind",
    "id",
    "name",
    "version",
    "license",
    "author",
    "description",
    "permissions",
];

/// 数据类插件不能申请任何权限：`permissions` 不写，或者写成空数组。
fn refuse_permissions(table: &toml::map::Map<String, Value>) -> Result<(), String> {
    match table.get("permissions") {
        None => Ok(()),
        Some(Value::Array(items)) if items.is_empty() => Ok(()),
        Some(Value::Array(_)) => Err("插件不能申请权限，permissions 必须为空".into()),
        Some(_) => Err("permissions 必须是数组".into()),
    }
}

/// A plain file in a pack directory, by name, with its size.
pub(crate) type PackFiles = BTreeMap<String, u64>;

/// Parse and check the pack in `directory` whatever its folder is called. `import` runs this on the staged copy before anything is installed, and every load runs it again.
pub(crate) fn load_directory(directory: &Path) -> Result<PluginSummary, String> {
    load_directory_with(directory, DataFiles::Summarize)
}

fn load_directory_with(directory: &Path, data_files: DataFiles) -> Result<PluginSummary, String> {
    let files = list_files(directory)?;
    if !files.contains_key(MANIFEST_FILE) {
        return Err("缺少 plugin.toml".into());
    }
    let bytes = read_file(directory, MANIFEST_FILE, MAX_MANIFEST_BYTES)
        .map_err(|_| "plugin.toml 无法读取或太大".to_owned())?;
    let value: Value =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| "plugin.toml 不是 UTF-8 编码")?)
            .map_err(|_| "plugin.toml 不是有效的 TOML")?;
    let table = value.as_table().ok_or("plugin.toml 的内容必须是一个表")?;
    let schema_version = table.get("schema_version").and_then(Value::as_integer);
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    if schema_version != Some(1) {
        return Err("不支持这个 schema_version".into());
    }
    // 桌面上版本要按类型判断：script 必须写 2，其他类型仍只认 1。既不是 1 也不是 2 时照旧先报版本，和移动端的报错顺序一致。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    if !matches!(schema_version, Some(1 | 2)) {
        return Err("不支持这个 schema_version".into());
    }
    let kind = table
        .get("kind")
        .and_then(Value::as_str)
        .and_then(PluginKind::parse)
        .ok_or(UNKNOWN_KIND)?;
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    script_pack::check_schema_version(kind, schema_version)?;
    let id = required_string(table, "id", 64)?;
    if !safe_id(&id) {
        return Err("插件 id 无效".into());
    }
    let name = required_string(table, "name", 80)?;
    let version = required_string(table, "version", 32)?;
    let license = required_string(table, "license", 64)?;
    if !license
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '(' | ')' | ' '))
    {
        return Err("license 必须是 SPDX 许可证表达式".into());
    }
    let author = optional_string(table, "author", 120)?;
    let description = optional_string(table, "description", 500)?;
    // 权限只对脚本插件放行，而且只认白名单里的能力；数据类插件照旧必须为空。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    let permissions = if kind.is_script() {
        script_pack::parse_permissions(table)?
    } else {
        refuse_permissions(table)?;
        msime_script_runtime::CapabilitySet::EMPTY
    };
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    refuse_permissions(table)?;
    let kind_keys: &[&str] = match kind {
        PluginKind::Sound => &sound_pack::MANIFEST_KEYS,
        PluginKind::Music => &music_pack::MANIFEST_KEYS,
        PluginKind::CommandTable => &command_table::MANIFEST_KEYS,
        PluginKind::Effect => &effect_pack::MANIFEST_KEYS,
        PluginKind::PhraseTable => &phrase_table::MANIFEST_KEYS,
        PluginKind::Helpcode => &helpcode_pack::MANIFEST_KEYS,
        PluginKind::Wordbook => &wordbook_pack::MANIFEST_KEYS,
        PluginKind::SymbolSet => &symbol_set::MANIFEST_KEYS,
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        PluginKind::Script => &script_pack::MANIFEST_KEYS,
    };
    if let Some(key) = table
        .keys()
        .find(|key| !COMMON_KEYS.contains(&key.as_str()) && !kind_keys.contains(&key.as_str()))
    {
        return Err(format!("plugin.toml 里有未知的键 {key}"));
    }
    let mut data: Vec<DataFile> = Vec::with_capacity(data_file_capacity(kind));
    let (content, audio, limits) = match kind {
        PluginKind::Sound => {
            let pack = sound_pack::parse(table)?;
            let audio = pack.files();
            (PluginContent::Sound(pack), audio, sound_pack::LIMITS)
        }
        PluginKind::Music => {
            let pack = music_pack::parse(table)?;
            let audio = pack.tracks.clone();
            (PluginContent::Music(pack), audio, music_pack::LIMITS)
        }
        PluginKind::CommandTable => (
            PluginContent::CommandTable(command_table::parse(table)?),
            Vec::new(),
            AudioLimits::NONE,
        ),
        PluginKind::Effect => (
            PluginContent::Effect(effect_pack::parse(table)?),
            Vec::new(),
            AudioLimits::NONE,
        ),
        PluginKind::PhraseTable => (
            PluginContent::PhraseTable(phrase_table::parse(table)?),
            Vec::new(),
            AudioLimits::NONE,
        ),
        PluginKind::Helpcode => {
            // 码表内容要等文件检查过之后再读，这里先占位，下面换成解析结果。
            let name = helpcode_pack::parse(table)?;
            data.push(DataFile {
                name: name.clone(),
                max_bytes: helpcode_pack::MAX_TABLE_BYTES,
                extension: helpcode_pack::TABLE_EXTENSION,
            });
            (
                PluginContent::Helpcode(helpcode_pack::HelpcodePack {
                    table: name,
                    entries: 0,
                    preview: Vec::new(),
                }),
                Vec::new(),
                AudioLimits::NONE,
            )
        }
        PluginKind::SymbolSet => (
            PluginContent::SymbolSet(symbol_set::parse(table)?),
            Vec::new(),
            AudioLimits::NONE,
        ),
        PluginKind::Wordbook => {
            wordbook_pack::check_identity(&id, &name)?;
            let file = wordbook_pack::parse(table)?;
            data.push(DataFile {
                name: file.clone(),
                max_bytes: wordbook_pack::MAX_FILE_BYTES,
                extension: wordbook_pack::FILE_EXTENSION,
            });
            (
                PluginContent::Wordbook(wordbook_pack::WordbookPack {
                    file,
                    word_count: 0,
                    first_words: Vec::new(),
                }),
                Vec::new(),
                AudioLimits::NONE,
            )
        }
        // 模块按数据文件登记，走同一份文件检查（存在、1 字节到 256 KiB、扩展名 `.wasm`）；内容等文件检查过之后再读。
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        PluginKind::Script => {
            let pack = script_pack::parse(table, permissions)?;
            data.push(script_pack::module_file(&pack));
            (PluginContent::Script(pack), Vec::new(), AudioLimits::NONE)
        }
    };
    check_files(directory, &files, &audio, limits, &data)?;
    let content = match content {
        content if data_files == DataFiles::Unread => content,
        PluginContent::Helpcode(pack) => {
            PluginContent::Helpcode(helpcode_pack::read(directory, &pack.table)?)
        }
        PluginContent::Wordbook(pack) => {
            PluginContent::Wordbook(wordbook_pack::read(directory, &pack.file)?)
        }
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        PluginContent::Script(pack) => {
            PluginContent::Script(script_pack::read(directory, pack, &bytes, data_files)?)
        }
        other => other,
    };
    Ok(PluginSummary {
        id,
        name,
        version,
        license,
        author,
        description,
        builtin: false,
        directory: directory.to_path_buf(),
        content,
    })
}

fn data_file_capacity(kind: PluginKind) -> usize {
    if matches!(kind, PluginKind::Helpcode | PluginKind::Wordbook) || kind.is_script() {
        1
    } else {
        0
    }
}

/// Bounds on the audio files of one kind.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AudioLimits {
    pub files: usize,
    pub file_bytes: u64,
    pub total_bytes: u64,
}

impl AudioLimits {
    const NONE: Self = Self {
        files: 0,
        file_bytes: 0,
        total_bytes: 0,
    };
}

/// 清单点名的一个数据文件（辅助码表、单词表）。它必须存在、大小在 1..=`max_bytes` 之间、扩展名是 `extension`；它不按说明文件处理，也不做签名检查，内容由该类型自己的解析器通过 [`read_file`] 校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFile {
    pub name: String,
    pub max_bytes: u64,
    /// 小写扩展名，不带点。
    pub extension: &'static str,
}

/// The regular files of a pack directory, hidden names aside. Anything else in it - a subdirectory, a symbolic link, a device - refuses the pack, as does a name that is not one plain component or an entry count past `MAX_PACK_FILES`.
fn list_files(directory: &Path) -> Result<PackFiles, String> {
    let mut files = PackFiles::new();
    let entries = fs::read_dir(directory).map_err(|_| "插件文件夹无法读取")?;
    for entry in entries {
        let entry = entry.map_err(|_| "插件文件夹无法读取")?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "有文件名不是 UTF-8 编码")?;
        // What a file manager leaves in a folder it has shown (`.DS_Store`) is not part of the pack. Nothing reads it: every file a host opens is named by the manifest, and those names cannot start with a dot.
        if name.starts_with('.') {
            continue;
        }
        if files.len() == MAX_PACK_FILES {
            return Err("文件太多".into());
        }
        let file_type = entry.file_type().map_err(|_| "插件文件夹无法读取")?;
        if file_type.is_symlink() {
            return Err(format!("{name} 是符号链接"));
        }
        if file_type.is_dir() {
            return Err(format!("{name} 是子文件夹"));
        }
        if !file_type.is_file() {
            return Err(format!("{name} 不是普通文件"));
        }
        if !valid_file_name(&name) {
            return Err(format!("{name} 不是有效的文件名"));
        }
        let size = entry.metadata().map_err(|_| "插件文件夹无法读取")?.len();
        files.insert(name, size);
    }
    Ok(files)
}

/// One plain file name: ASCII letters, digits, `_`, `-` and dots, starting with a letter or digit.
pub(crate) fn valid_file_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit());
    name.len() <= 64
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && crate::skin::catalog::safe_resource(name, 64)
        && !name.contains('/')
        && !reserved
}

/// 数据文件按行切开：整个文件必须是 UTF-8（开头可以有 BOM），按 `\n` 分行，每行去掉一个行尾的 `\r`；其余位置的 `\r` 和别的控制字符一样留给各类型的行规则拒绝。
/// 先校验整个文件的编码，再按需产生从 1 开始的行号和借用行内容。
pub(crate) fn data_lines<'a>(
    bytes: &'a [u8],
    name: &str,
) -> Result<impl Iterator<Item = (usize, &'a str)> + 'a, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{name} 不是 UTF-8 编码"))?;
    Ok(text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .enumerate()
        .map(|(index, line)| (index + 1, line)))
}

/// The lower-case extension of a file name.
fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Whether `name` is an audio file a music pack may name: WAV or Ogg.
pub(crate) fn is_audio(name: &str) -> bool {
    matches!(extension(name).as_str(), "wav" | "ogg")
}

/// Whether `name` is a sample a sound pack may name: WAV only. Hosts decode a key sample whole before they play it, and only a WAV's length can be checked up front, from the frame count its header declares (`sound_pack::sample_frames_allowed`); an Ogg stream has to be decoded to learn how long it runs. HarmonyOS's player already refuses anything else (`KeySoundPolicy.isWav`), so an Ogg sample was silent there while it played on the desktop.
pub(crate) fn is_wav(name: &str) -> bool {
    extension(name) == "wav"
}

fn is_notice(name: &str) -> bool {
    matches!(extension(name).as_str(), "txt" | "md")
}

/// 每个文件要么是清单，要么是清单点名的音频或数据文件，要么是说明文件；点名的音频文件都在、不超出 `limits`、文件头与格式相符；点名的数据文件都在、大小与扩展名符合 [`DataFile`] 的要求。
fn check_files(
    directory: &Path,
    files: &PackFiles,
    audio: &[String],
    limits: AudioLimits,
    data: &[DataFile],
) -> Result<(), String> {
    let mut distinct: Vec<&str> = audio.iter().map(String::as_str).collect();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() > limits.files {
        return Err("音频文件太多".into());
    }
    let mut total = 0u64;
    for name in &distinct {
        let size = *files
            .get(*name)
            .ok_or_else(|| format!("缺少音频文件 {name}"))?;
        if size == 0 || size > limits.file_bytes {
            return Err(format!("{name} 为空或太大"));
        }
        total += size;
        if !audio_signature_matches(directory, name) {
            return Err(format!("{name} 不是 WAV 或 Ogg 音频"));
        }
    }
    if total > limits.total_bytes {
        return Err("音频文件加起来太大".into());
    }
    for file in data {
        if distinct.binary_search(&file.name.as_str()).is_ok() || file.name == MANIFEST_FILE {
            return Err(format!("{} 不能同时用作别的文件", file.name));
        }
        if extension(&file.name) != file.extension {
            return Err(format!("{} 的扩展名必须是 .{}", file.name, file.extension));
        }
        let size = *files
            .get(&file.name)
            .ok_or_else(|| format!("缺少数据文件 {}", file.name))?;
        if size == 0 || size > file.max_bytes {
            return Err(format!("{} 为空或太大", file.name));
        }
    }
    for (name, size) in files {
        if name == MANIFEST_FILE
            || distinct.binary_search(&name.as_str()).is_ok()
            || data.iter().any(|file| file.name == *name)
        {
            continue;
        }
        if !is_notice(name) {
            return Err(format!("{name} 没有在 plugin.toml 里用到"));
        }
        if *size > MAX_NOTICE_BYTES {
            return Err(format!("{name} 太大"));
        }
    }
    Ok(())
}

/// Whether the file begins with the RIFF/WAVE or Ogg header its extension promises. Decoding is the host's, bounded again there; this only keeps a renamed file of some other type from being listed as audio.
fn audio_signature_matches(directory: &Path, name: &str) -> bool {
    let Ok(mut file) = crate::storage::open_private_file_in(&directory.join(name)) else {
        return false;
    };
    let mut header = [0u8; 12];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    match extension(name).as_str() {
        "wav" => &header[..4] == b"RIFF" && &header[8..12] == b"WAVE",
        "ogg" => &header[..4] == b"OggS",
        _ => false,
    }
}

/// A file of a pack directory, at most `maximum` bytes. The type is checked on the open handle too, since the listing may be stale by the time it is read.
fn read_file(directory: &Path, name: &str, maximum: u64) -> std::io::Result<Vec<u8>> {
    let path = directory.join(name);
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let file = crate::storage::open_private_file_in(&path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    crate::bounded_io::read_bounded_file(file, maximum, || {
        std::io::Error::other("file is too large")
    })
}

pub(crate) fn required_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<String, String> {
    let value = table
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} 必须是字符串"))?;
    if value.trim().is_empty() || !crate::text::is_bounded_text(value, max) {
        return Err(format!("{key} 的长度或字符不符合要求"));
    }
    Ok(value.to_owned())
}

pub(crate) fn optional_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, String> {
    if !table.contains_key(key) {
        return Ok(None);
    }
    required_string(table, key, max).map(Some)
}

/// The keys of `table` are all in `allowed`.
pub(crate) fn only_keys(
    table: &toml::map::Map<String, Value>,
    allowed: &[&str],
    what: &str,
) -> Result<(), String> {
    match table.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{what} 里有未知的键 {key}")),
        None => Ok(()),
    }
}

/// Delete an installed pack. A built-in sound or music id is refused; removing a pack that is not installed succeeds.
///
/// 官方插件包的 id 只删 `root` 下同名的影子副本（官方包随安装包分发之前导入过的同 id 包，`scan` 把它报为问题），内置目录永远不碰，也不动这个 id 的授权条目：授权现在归内置的那一份，影子副本的哈希本来就对不上它。
pub fn remove(root: &Path, kind: PluginKind, id: &str) -> Result<(), PluginError> {
    if !safe_id(id) {
        return Err(PluginError::Invalid("插件 id 无效".into()));
    }
    let official = is_official(kind, id);
    if is_builtin(kind, id) && !official {
        return Err(PluginError::Reserved);
    }
    let directory = kind_directory(root, kind);
    crate::storage::reject_symlink(&directory).map_err(|_| PluginError::Storage)?;
    if fs::symlink_metadata(&directory).is_err() {
        // No kind directory, so no pack of this kind is installed, and there is nothing to lock.
        return Ok(());
    }
    let _writes = import::lock_plugin_root(root)?;
    // 脚本包的授权条目在同一把锁里、移走包目录之前去掉：删包失败时授权已经收回，不会反过来留下一条能在重新导入同样内容后复活的授权。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    if kind.is_script() && !official {
        script_grants::removing(root, &_writes, id)?;
    }
    let target = directory.join(id);
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() {
        // Never a pack, so there is nothing to follow: a link or a stray file goes as itself.
        crate::storage::remove_private_file(&target)?;
        return Ok(());
    }
    // Renamed aside first, so a deletion interrupted halfway never leaves a directory that still looks like the pack. The leading dot keeps it out of `scan`, and the next import sweeps it.
    let aside = directory.join(format!(".old-{id}-{}", uuid::Uuid::new_v4().simple()));
    #[cfg(unix)]
    {
        let directory_handle = crate::storage::open_private_directory(&directory)?;
        rustix::fs::renameat(
            &directory_handle,
            std::ffi::OsStr::new(id),
            &directory_handle,
            aside.file_name().expect("generated aside name"),
        )
        .map_err(std::io::Error::from)?;
    }
    #[cfg(not(unix))]
    fs::rename(&target, &aside)?;
    #[cfg(unix)]
    {
        let directory_handle = crate::storage::open_private_directory(&directory)?;
        crate::storage::remove_private_tree_at(
            &directory_handle,
            aside.file_name().expect("generated aside name"),
        )?;
    }
    #[cfg(not(unix))]
    fs::remove_dir_all(&aside)?;
    Ok(())
}

#[cfg(all(
    test,
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
mod script_tests;
#[cfg(test)]
mod tests;
