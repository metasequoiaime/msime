//! Plugin packs: sound packs, background music, command tables and typing-effect parameters, as validated data.
//!
//! Nothing in a pack runs. The kinds are a closed set, each with a fixed manifest shape that this module parses in full, and a pack that asks for any permission is refused, so a third party can supply samples, tracks and text templates and nothing else. Hosts play the audio, hand the command rows to the Engine and draw their own built-in effects with an effect pack's parameters; all of them only ever see a pack this module has accepted.
//!
//! A pack lives in `<root>/<kind>/<id>/`, where `root` is the `plugins` directory under the host's state root and `kind` is `sound`, `music`, `command_table` or `effect`. The directory holds `plugin.toml` and the flat files it names, plus optional text notices, and nothing else: no subdirectories, no symbolic links, no file the manifest does not account for. Built-in sound packs ship inside each platform's bundle rather than under `root`, in a directory the host names (`resources/sound-packs` in the repository), and are listed beside the installed ones.
//!
//! `mentions.json` beside the kind directories is the @ mode's name list, kept by `mentions`. It lives here rather than in the preferences document because that document is the one hosts copy and account sync reads from, and a contact list belongs to neither.

pub mod command_table;
pub mod community;
pub mod effect_pack;
mod failure;
mod import;
pub mod mentions;
pub mod music_pack;
pub mod sound_pack;

pub use failure::{remove_named, PluginFailure};
pub use import::{import, validate};

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
/// The sound pack a fresh profile selects.
pub const DEFAULT_SOUND_PACK: &str = "default";
/// The melody pack a fresh profile selects.
pub const DEFAULT_MELODY_PACK: &str = "twinkle";

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
}

impl PluginKind {
    pub const ALL: [Self; 4] = [Self::Sound, Self::Music, Self::CommandTable, Self::Effect];

    /// The manifest's `kind` and the directory under the plugins root.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sound => "sound",
            Self::Music => "music",
            Self::CommandTable => "command_table",
            Self::Effect => "effect",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Whether `id` of `kind` names a pack the bundle ships. Only sound and music packs are built in: an effect has nothing to ship, since the styles themselves are in the hosts and `effect_style` already selects one without a pack.
pub fn is_builtin(kind: PluginKind, id: &str) -> bool {
    match kind {
        PluginKind::Sound => BUILTIN_SOUND_PACKS.contains(&id),
        PluginKind::Music => BUILTIN_MUSIC_PACKS.contains(&id),
        PluginKind::CommandTable | PluginKind::Effect => false,
    }
}

/// The typing effect a host draws on keys and commits. Closed: every style is built into the hosts, and an effect pack (`PluginKind::Effect`) only selects one of them and tunes it within `effect_pack`'s bounds, so no pack can supply a style of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStyle {
    #[default]
    Off,
    /// A brief flash on the host's own candidate surface.
    Flash,
    /// Sparks where the host can draw them, a flash where it cannot.
    Sparks,
    /// Sparks, a shake and a combo that grows the effect as it climbs.
    PowerMode,
}

impl EffectStyle {
    /// The style's number in `msime_client_typing_effect`'s answer.
    pub fn code(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Flash => 1,
            Self::Sparks => 2,
            Self::PowerMode => 3,
        }
    }
}

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

/// `<root>/<kind>`.
pub fn kind_directory(root: &Path, kind: PluginKind) -> PathBuf {
    root.join(kind.as_str())
}

/// Every pack under `root`, and the built-in sound packs under `builtin_sounds` when the host has them, with what is wrong with each folder that is not a pack. Folders whose name starts with a dot are install and removal leftovers and are not reported.
pub fn scan(root: &Path, builtin_sounds: Option<&Path>) -> PluginCatalog {
    let mut catalog = PluginCatalog::default();
    if let Some(builtin) = builtin_sounds {
        scan_builtin(builtin, &mut catalog);
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
            Ok(file_type) if file_type.is_dir() => load_installed(directory, &folder, kind, true),
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
                load_installed(directory, &folder, kind, builtin)
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

/// Validate one pack by the rules `scan` lists packs by, without reading the rest of the root. Hosts resolve the selected pack through this. A built-in sound or music pack id is read from `builtin_sounds`, never from `root`.
pub fn load_package(
    root: &Path,
    builtin_sounds: Option<&Path>,
    kind: PluginKind,
    id: &str,
) -> Result<PluginSummary, String> {
    if !safe_id(id) {
        return Err("插件 id 无效".into());
    }
    if is_builtin(kind, id) {
        let builtin = builtin_sounds.ok_or("内置音效包不可用")?;
        return load_installed(builtin, id, kind, true);
    }
    load_installed(&kind_directory(root, kind), id, kind, false)
}

/// A pack in `<directory>/<folder>`, whose manifest must name `folder` as its id and `kind` as its kind.
fn load_installed(
    directory: &Path,
    folder: &str,
    kind: PluginKind,
    builtin: bool,
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
    let mut summary = load_directory(&package)?;
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

/// A plain file in a pack directory, by name, with its size.
pub(crate) type PackFiles = BTreeMap<String, u64>;

/// Parse and check the pack in `directory` whatever its folder is called. `import` runs this on the staged copy before anything is installed, and every load runs it again.
pub(crate) fn load_directory(directory: &Path) -> Result<PluginSummary, String> {
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
    if table.get("schema_version").and_then(Value::as_integer) != Some(1) {
        return Err("不支持这个 schema_version".into());
    }
    let kind = table
        .get("kind")
        .and_then(Value::as_str)
        .and_then(PluginKind::parse)
        .ok_or("kind 不是已知的插件类型")?;
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
    match table.get("permissions") {
        None => {}
        Some(Value::Array(items)) if items.is_empty() => {}
        Some(Value::Array(_)) => return Err("插件不能申请权限，permissions 必须为空".into()),
        Some(_) => return Err("permissions 必须是数组".into()),
    }
    let kind_keys: &[&str] = match kind {
        PluginKind::Sound => &sound_pack::MANIFEST_KEYS,
        PluginKind::Music => &music_pack::MANIFEST_KEYS,
        PluginKind::CommandTable => &command_table::MANIFEST_KEYS,
        PluginKind::Effect => &effect_pack::MANIFEST_KEYS,
    };
    if let Some(key) = table
        .keys()
        .find(|key| !COMMON_KEYS.contains(&key.as_str()) && !kind_keys.contains(&key.as_str()))
    {
        return Err(format!("plugin.toml 里有未知的键 {key}"));
    }
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
    };
    check_files(directory, &files, &audio, limits)?;
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
    name.len() <= 64
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && crate::skin::catalog::safe_resource(name, 64)
        && !name.contains('/')
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

/// Every file is the manifest, an audio file the manifest names, or a notice; every named audio file is there, within `limits`, and starts the way its format does.
fn check_files(
    directory: &Path,
    files: &PackFiles,
    audio: &[String],
    limits: AudioLimits,
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
    for (name, size) in files {
        if name == MANIFEST_FILE || distinct.contains(&name.as_str()) {
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
    let Ok(mut file) = fs::File::open(directory.join(name)) else {
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
    let file = fs::File::open(&path)?;
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

/// Delete an installed pack. A built-in id is refused; removing a pack that is not installed succeeds.
pub fn remove(root: &Path, kind: PluginKind, id: &str) -> Result<(), PluginError> {
    if !safe_id(id) {
        return Err(PluginError::Invalid("插件 id 无效".into()));
    }
    if is_builtin(kind, id) {
        return Err(PluginError::Reserved);
    }
    let directory = kind_directory(root, kind);
    crate::storage::reject_symlink(&directory).map_err(|_| PluginError::Storage)?;
    if fs::symlink_metadata(&directory).is_err() {
        // No kind directory, so no pack of this kind is installed, and there is nothing to lock.
        return Ok(());
    }
    let _writes = import::lock_plugin_root(root)?;
    let target = directory.join(id);
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() {
        // Never a pack, so there is nothing to follow: a link or a stray file goes as itself.
        fs::remove_file(&target)?;
        return Ok(());
    }
    // Renamed aside first, so a deletion interrupted halfway never leaves a directory that still looks like the pack. The leading dot keeps it out of `scan`, and the next import sweeps it.
    let aside = directory.join(format!(".old-{id}-{}", uuid::Uuid::new_v4().simple()));
    fs::rename(&target, &aside)?;
    fs::remove_dir_all(&aside)?;
    Ok(())
}

#[cfg(test)]
mod tests;
