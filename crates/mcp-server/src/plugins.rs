//! AI 助手管理插件包：列出、读回、创建、导入和删除装在 `<state_dir>/plugins` 下的包。
//!
//! 规则全在 client-services 的 `plugins` 里，这里不另写一份：create_plugin 把代理给的清单和文件写进服务器自己的临时目录，再交给设置页导入用的同一个 `import_from_settings`；install_plugin_folder 直接把代理给的文件夹或 .zip 交给它。所以经 MCP 装进来的包与用户在「插件」页导入的包过的是同一套检查，设置页拒绝的包这里也拒绝。脚本插件只能经 install_plugin_folder 导入（代理先用 `msime-pack` 构建好），导入后和设置页导入的一样默认停用，要用户在「插件 → 脚本插件（实验）」里逐项授权才运行；本服务器从不写授权。输入进程读的是同一个插件目录，装好、删掉都不用另发通知。

use msime_client_core::edition::Edition;
use msime_client_core::preferences::{KeySoundMode, Preferences, PreferencesStore};
use msime_client_services::plugins::{
    self, BuiltinDirs, PluginCatalog, PluginContent, PluginError, PluginKind, PluginSummary,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// 状态目录下的插件目录名，与设置应用的 `PluginsState` 和 host-api 的 `preferences_directory/plugins` 相同。
pub const PLUGIN_DIRECTORY: &str = "plugins";
/// 一个本机文件的上限：任何类型允许的最大文件是一首音乐曲目（`music_pack::MAX_TRACK_BYTES`），与导入复制时的单文件上限相同。
const MAX_FILE_BYTES: u64 = plugins::music_pack::MAX_TRACK_BYTES;
/// 全部文件合计的上限，与导入复制时的整包上限相同：一个满额音乐包再加说明文件。
const MAX_TOTAL_BYTES: u64 = plugins::music_pack::MAX_PACK_BYTES + 2 * 1024 * 1024;
/// 内联文本文件的上限。辅助码表最大 1 MiB；更大的单词本写成文件再给路径。
const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// 列出包内容摘要时最多点名几项（触发词、编码、分组）。
const SUMMARY_ITEMS: usize = 8;

pub fn root(state_dir: &Path) -> PathBuf {
    state_dir.join(PLUGIN_DIRECTORY)
}

/// 安装包随带的两个只读目录：内置音效包和官方插件包。按 host-api `PluginRoots::new` 的规则从运行时选项读：`sound_packs` 和 `plugin_packs` 给了就用（只认绝对路径）；没给 `sound_packs` 时取 `resources` 旁边存在的 `sound-packs`，没给 `plugin_packs` 时取内置音效包目录旁边存在的 `plugin-packs`。读不到的目录是 `None`，对应的内置包就列不出来，用户装的包照常列出。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BuiltinPaths {
    sounds: Option<PathBuf>,
    packs: Option<PathBuf>,
}

impl BuiltinPaths {
    pub fn from_options(document: &Value) -> Self {
        let absolute = |key: &str| {
            document
                .get(key)
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let sounds = match document.get("sound_packs").and_then(Value::as_str) {
            Some(_) => absolute("sound_packs"),
            None => absolute("resources")
                .and_then(|resources| Some(resources.parent()?.join("sound-packs")))
                .filter(|path| path.is_dir()),
        };
        let packs = match document.get("plugin_packs").and_then(Value::as_str) {
            Some(_) => absolute("plugin_packs"),
            None => sounds
                .as_deref()
                .and_then(Path::parent)
                .map(|parent| parent.join("plugin-packs"))
                .filter(|path| path.is_dir()),
        };
        Self { sounds, packs }
    }

    fn dirs(&self) -> BuiltinDirs<'_> {
        BuiltinDirs {
            sounds: self.sounds.as_deref(),
            packs: self.packs.as_deref(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginList {
    /// Every loadable pack: the user's own and those that ship with the input method.
    pub plugins: Vec<PluginView>,
    /// Folders under the plugins folder that are not a loadable pack, with the reason the input method gives (in Chinese).
    pub issues: Vec<PluginIssueView>,
    /// Which packs the preferences select or enable now.
    pub selection: PluginSelection,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginView {
    /// sound, music, command_table, effect, phrase_table, helpcode, wordbook, symbol_set or script.
    pub kind: String,
    /// The manifest id, which is also the pack's folder name.
    pub id: String,
    pub name: String,
    pub version: String,
    /// The SPDX licence the manifest declares.
    pub license: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// Ships with the input method rather than installed by the user: it cannot be deleted, replaced or published.
    pub builtin: bool,
    /// One of the official packs the desktop installer ships (the latex-symbols phrase table, the uuid script plugin); always also builtin.
    pub official: bool,
    /// What it holds, in a few words: sample or track counts, the / triggers, the K-mode keys, entry or word counts, symbol groups, or a script's capabilities.
    pub summary: String,
    /// Whether the input method uses it now by the preferences: the selected key sound, melody, music or effect pack (with the sound or music switch on), an enabled command or phrase table, a helpcode table chosen for quanpin or shuangpin; word books and symbol sets are used once installed. Null for a script plugin, which is turned on and granted in 设置 → 插件 → 脚本插件（实验）.
    pub active: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginIssueView {
    pub kind: String,
    /// The folder's name; empty when the kind folder itself is not a folder.
    pub folder: String,
    pub reason: String,
}

/// 偏好里 `plugins.*` 的选用和启用，加上决定指令表和短语表起不起作用的两个快捷模式开关。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginSelection {
    /// 声音与效果 → 按键音 is on.
    pub key_sound_enabled: bool,
    /// keys (the sound pack's samples) or melody (the melody pack's notes).
    pub key_sound_mode: String,
    /// plugins.key_sound.pack: the sound pack played in keys mode.
    pub key_sound_pack: String,
    /// plugins.melody.pack: the sound pack played in melody mode.
    pub melody_pack: String,
    /// 声音与效果 → 背景音乐 is on.
    pub music_enabled: bool,
    pub music_pack: String,
    /// plugins.effect_pack, null for none.
    pub effect_pack: Option<String>,
    /// plugins.command_tables: the enabled command tables, in priority order.
    pub command_tables: Vec<String>,
    /// plugins.phrase_tables: the enabled phrase tables, in priority order.
    pub phrase_tables: Vec<String>,
    /// plugins.helpcode_pack_quanpin, null for the built-in scheme.
    pub helpcode_pack_quanpin: Option<String>,
    /// plugins.helpcode_pack_shuangpin, null for the built-in scheme.
    pub helpcode_pack_shuangpin: Option<String>,
    /// 输入 → 快捷模式 → 指令(/ 模式) is on. Command tables and script / commands do nothing while it is off, and it is off by default.
    pub command_mode: bool,
    /// 输入 → 快捷模式 → 快捷短语 (K mode, Shift+K) is on. Phrase tables do nothing while it is off.
    pub quick_phrase_mode: bool,
}

/// 指定一个包：类型和 id。
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PluginRequest {
    /// sound, music, command_table, effect, phrase_table, helpcode, wordbook, symbol_set or script, as list_plugins gives it.
    pub kind: String,
    /// The pack's id, as list_plugins gives it.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginFiles {
    pub kind: String,
    pub id: String,
    pub builtin: bool,
    /// The whole plugin.toml.
    pub manifest: String,
    /// The absolute path of the pack's folder.
    pub directory: String,
    /// Every file in the folder other than plugin.toml.
    pub files: Vec<PluginFileView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PluginFileView {
    /// The file name in the pack, the key create_plugin's files take.
    pub name: String,
    /// The absolute path on this computer, usable as is as a value of create_plugin's files.
    pub file: String,
    pub bytes: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CreatePluginRequest {
    /// The whole plugin.toml, at most 256 KiB.
    pub manifest: String,
    /// 以本机文件给出的包内文件：键是包里的文件名（清单点名的那个），值是文件的绝对路径。服务器只读这些文件：路径上不能有符号链接，必须是普通文件，超过 16 MiB 的在读之前就拒绝。
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// 直接写在调用里的文本文件：键是包里的文件名，值是 UTF-8 文本，每个最多 1 MiB。适合辅助码表、单词本和说明文件；同一个文件名不能两边都给。
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
    /// Replace an installed pack of the same kind and id whole. Without it an installed pack is left alone and the call refused.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct InstallPluginRequest {
    /// The absolute path of a pack folder, or of a .zip holding the pack's files at its top level or inside one folder.
    pub path: String,
    /// Replace an installed pack of the same kind and id whole. Without it an installed pack is left alone and the call refused.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct InstalledPlugin {
    pub kind: String,
    pub id: String,
    pub name: String,
    /// Whether an installed pack of this kind and id was replaced.
    pub replaced: bool,
    /// What the user has to do for the pack to take effect, to pass on in plain words.
    pub next_step: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeletedPlugin {
    pub kind: String,
    pub id: String,
    /// The preference entries that still name it (plugins.key_sound.pack, plugins.phrase_tables, ...). The input method falls back to the default for each, as after the settings page removes a pack in use.
    pub still_selected_in: Vec<String>,
}

/// 用户装的包、能读到的内置包和偏好里的选用。
pub fn list(state_dir: &Path, builtin: &BuiltinPaths, preferences: &Preferences) -> PluginList {
    let PluginCatalog { packages, issues } = plugins::scan(&root(state_dir), builtin.dirs());
    PluginList {
        plugins: packages
            .iter()
            .map(|package| PluginView {
                kind: package.kind().as_str().to_owned(),
                id: package.id.clone(),
                name: package.name.clone(),
                version: package.version.clone(),
                license: package.license.clone(),
                author: package.author.clone(),
                description: package.description.clone(),
                builtin: package.builtin,
                official: plugins::is_official(package.kind(), &package.id),
                summary: summarize(&package.content),
                active: active(package.kind(), &package.id, preferences),
            })
            .collect(),
        issues: issues
            .into_iter()
            .map(|issue| PluginIssueView {
                kind: issue.kind.as_str().to_owned(),
                folder: issue.folder,
                reason: issue.reason,
            })
            .collect(),
        selection: selection(preferences),
    }
}

/// 读一个包的清单原文和文件，内置包也能读（只读，可以拿来照着改成用户自己的包）。
pub fn get(
    state_dir: &Path,
    builtin: &BuiltinPaths,
    request: &PluginRequest,
) -> Result<PluginFiles, String> {
    let kind = parse_kind(&request.kind)?;
    let id = request.id.as_str();
    let root = root(state_dir);
    let package = plugins::load_package(&root, builtin.dirs(), kind, id).map_err(|reason| {
        let installed = plugins::pack_directory(&root, builtin.dirs(), kind, id)
            .is_some_and(|directory| std::fs::symlink_metadata(directory).is_ok());
        if installed {
            format!("{id} cannot be read as a {} pack ({reason}); list_plugins lists it under issues with the reason", kind.as_str())
        } else {
            not_installed(kind, id)
        }
    })?;
    let directory = package.directory.clone();
    let manifest_file = crate::bounded::open_private(&directory.join(plugins::MANIFEST_FILE))
        .map_err(|_| "plugin.toml cannot be opened")?;
    let manifest = crate::bounded::read(manifest_file, plugins::MAX_MANIFEST_BYTES)
        .map_err(|_| "plugin.toml cannot be read")?;
    let manifest = String::from_utf8(manifest).map_err(|_| "plugin.toml is not UTF-8")?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(|_| "the pack folder cannot be read")? {
        let entry = entry.map_err(|_| "the pack folder cannot be read")?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        // 以点开头的是文件管理器留下的东西，载入时也不算进包里。
        if name.starts_with('.') || name == plugins::MANIFEST_FILE {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|_| "the pack folder cannot be read")?;
        if !metadata.is_file() {
            continue;
        }
        files.push(PluginFileView {
            file: directory.join(&name).display().to_string(),
            name,
            bytes: metadata.len(),
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(PluginFiles {
        kind: kind.as_str().to_owned(),
        id: package.id,
        builtin: package.builtin,
        manifest,
        directory: directory.display().to_string(),
        files,
    })
}

/// 用清单和文件装一个数据类插件包：先在服务器自己的临时目录里拼出包文件夹，再按设置页导入的规则装进插件目录。
pub fn create(
    state_dir: &Path,
    preferences: &Preferences,
    request: &CreatePluginRequest,
) -> Result<InstalledPlugin, String> {
    if request.manifest.len() as u64 > plugins::MAX_MANIFEST_BYTES {
        return Err("plugin.toml may be at most 256 KiB".into());
    }
    if declares_script(&request.manifest) {
        return Err(SCRIPT_BY_FOLDER.into());
    }
    if request.files.len() + request.texts.len() >= plugins::MAX_PACK_FILES {
        return Err(format!(
            "a pack holds at most {} files besides plugin.toml",
            plugins::MAX_PACK_FILES - 1
        ));
    }
    let mut contents: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    let mut total = 0u64;
    for (name, text) in &request.texts {
        check_file_name(name)?;
        if text.len() > MAX_TEXT_BYTES {
            return Err(format!("texts[{name:?}] is larger than 1 MiB; write it to a file and pass its path in files"));
        }
        total += text.len() as u64;
        contents.insert(name, text.as_bytes().to_vec());
    }
    for (name, file) in &request.files {
        check_file_name(name)?;
        if contents.contains_key(name.as_str()) {
            return Err(format!(
                "{name:?} is given in both files and texts; give each file in only one of the two"
            ));
        }
        let bytes = read_local(name, file)?;
        total += bytes.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(
                "the files are larger than 66 MiB together, more than any pack may hold".into(),
            );
        }
        contents.insert(name, bytes);
    }
    let staging = tempfile::Builder::new()
        .prefix("msime-mcp-plugin-")
        .tempdir()
        .map_err(|_| "cannot make a temporary folder to assemble the pack in")?;
    write_new(
        &staging.path().join(plugins::MANIFEST_FILE),
        request.manifest.as_bytes(),
    )?;
    for (name, bytes) in &contents {
        write_new(&staging.path().join(name), bytes)?;
    }
    install(
        state_dir,
        preferences,
        staging.path(),
        request.replace,
        false,
    )
}

/// 导入本机已有的包文件夹或 .zip，脚本插件也在内。
pub fn install_folder(
    state_dir: &Path,
    preferences: &Preferences,
    request: &InstallPluginRequest,
) -> Result<InstalledPlugin, String> {
    let path = Path::new(&request.path);
    if !path.is_absolute() {
        return Err(format!(
            "{:?} is not an absolute path; pass the absolute path of the pack folder or .zip",
            request.path
        ));
    }
    if msime_path_trust::reject_symlinked_components(path).is_err() {
        return Err(format!("{:?} has a symbolic link on the way or cannot be reached; pass the real path of the pack folder or .zip", request.path));
    }
    install(state_dir, preferences, path, request.replace, true)
}

/// 先按导入的规则检查 `source`，得知它的类型和 id，按 `replace` 决定能不能替换已装的同名包，再导入。
fn install(
    state_dir: &Path,
    preferences: &Preferences,
    source: &Path,
    replace: bool,
    allow_script: bool,
) -> Result<InstalledPlugin, String> {
    let checked = plugins::validate(source).map_err(refused)?;
    let kind = checked.kind();
    if kind.is_script() && !allow_script {
        return Err(SCRIPT_BY_FOLDER.into());
    }
    let root = root(state_dir);
    let target = plugins::kind_directory(&root, kind).join(&checked.id);
    let replaced = std::fs::symlink_metadata(&target).is_ok();
    if replaced && !replace {
        return Err(format!("a {} pack {} is already installed; list_plugins shows it. Pass replace: true to replace it whole, or choose another id", kind.as_str(), checked.id));
    }
    let summary = plugins::import_from_settings(source, &root).map_err(refused)?;
    Ok(InstalledPlugin {
        kind: summary.kind().as_str().to_owned(),
        next_step: next_step(&summary, preferences),
        id: summary.id,
        name: summary.name,
        replaced,
    })
}

/// 删掉一个用户装的包。内置包和官方包都拒绝。
pub fn delete(
    state_dir: &Path,
    preferences: &Preferences,
    request: &PluginRequest,
) -> Result<DeletedPlugin, String> {
    let kind = parse_kind(&request.kind)?;
    let id = request.id.as_str();
    if !valid_id(id) {
        return Err(format!("{id:?} is not a plugin id: lowercase ASCII letters, digits, `_`, `-` and `.`, starting with a letter or digit, at most 64 bytes"));
    }
    if plugins::is_builtin(kind, id) {
        return Err(format!(
            "{id} ships with the input method and cannot be deleted"
        ));
    }
    let root = root(state_dir);
    if std::fs::symlink_metadata(plugins::kind_directory(&root, kind).join(id)).is_err() {
        return Err(not_installed(kind, id));
    }
    plugins::remove(&root, kind, id).map_err(refused)?;
    Ok(DeletedPlugin {
        kind: kind.as_str().to_owned(),
        id: id.to_owned(),
        still_selected_in: selected_in(kind, id, preferences),
    })
}

/// 偏好里的选用，`edition` 决定还没有偏好文件时读到的默认值。
pub fn preferences(state_dir: &Path, edition: &'static Edition) -> Result<Preferences, String> {
    PreferencesStore::for_edition(state_dir, edition)
        .load()
        .map(|snapshot| snapshot.preferences)
        .map_err(|error| error.to_string())
}

pub(crate) fn parse_kind(kind: &str) -> Result<PluginKind, String> {
    PluginKind::parse(kind).ok_or_else(|| {
        let kinds: Vec<&str> = PluginKind::ALL.iter().map(|kind| kind.as_str()).collect();
        format!(
            "{kind:?} is not a plugin kind; use one of {}",
            kinds.join(", ")
        )
    })
}

pub(crate) fn not_installed(kind: PluginKind, id: &str) -> String {
    format!(
        "no {} pack {id} is installed; list_plugins shows the installed packs",
        kind.as_str()
    )
}

/// 与 client-services 判定插件文件夹名的 `safe_id` 相同的形状。
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && msime_client_core::is_ascii_lowercase_identifier_with_dots(id)
}

const SCRIPT_BY_FOLDER: &str = "create_plugin makes data packs only; a script plugin (kind = \"script\") carries a WebAssembly module: build the pack folder with msime-pack and import it with install_plugin_folder";

const LOCAL_FILE_RULES: &str = "each files value must be the absolute path of a regular file on this computer that this user can read, at most 16 MiB, with no symbolic link anywhere on the way (system links such as /tmp on macOS excepted); write the file there and pass that path";

/// 包里的文件名只能是一个普通的路径分量：写进临时目录之前就挡住 `..`、分隔符和清单本身，其余命名规则（字符、长度、保留名）由导入检查。
fn check_file_name(name: &str) -> Result<(), String> {
    let mut components = Path::new(name).components();
    let plain = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !name.contains(['/', '\\'])
        && !name.starts_with('.');
    if !plain {
        return Err(format!("{name:?} is not a file name: a pack holds only plain files directly in its folder, named with ASCII letters, digits, `_`, `-` and `.`, starting with a letter or digit"));
    }
    if name.eq_ignore_ascii_case(plugins::MANIFEST_FILE) {
        return Err("plugin.toml is given as manifest, not as a file".into());
    }
    Ok(())
}

/// 读一个代理给的本机文件，规则与 create_candidate_skin 的 image_files 相同：绝对路径，路径上没有符号链接（系统别名除外），普通文件，读之前按大小拒绝，读时再按同一上限截断。
fn read_local(name: &str, file: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(file);
    let problem = |what: &str| format!("files[{name:?}] = {file:?} {what}; {LOCAL_FILE_RULES}");
    if !path.is_absolute() {
        return Err(problem("is not an absolute path"));
    }
    match std::fs::symlink_metadata(path) {
        Err(_) => return Err(problem("does not exist or cannot be reached")),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(problem("is a symbolic link; pass the file it points to"))
        }
        Ok(metadata) if !metadata.is_file() => return Err(problem("is not a regular file")),
        Ok(metadata) if metadata.len() > MAX_FILE_BYTES => {
            return Err(problem("is larger than 16 MiB"))
        }
        Ok(_) => {}
    }
    msime_path_trust::reject_symlinked_components(path)
        .map_err(|_| problem("has a symbolic link on the way"))?;
    let handle = msime_client_core::file_lock::open_private_file_in(path)
        .map_err(|_| problem("cannot be opened"))?;
    let metadata = handle.metadata().map_err(|_| problem("cannot be read"))?;
    if !metadata.is_file() {
        return Err(problem("is not a regular file"));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(problem("is larger than 16 MiB"));
    }
    crate::bounded::read(handle, MAX_FILE_BYTES).map_err(|error| match error {
        crate::bounded::ReadError::TooLarge => problem("is larger than 16 MiB"),
        crate::bounded::ReadError::Io => problem("cannot be read"),
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|_| "cannot write the pack into the temporary folder".to_owned())
}

/// 清单是不是写了 `kind = "script"`。只是为了在拼包之前给出明确的说法：导入检查之后还会按真正解析出的类型再拒一次。
fn declares_script(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        let value = value.split('#').next().unwrap_or_default().trim();
        key.trim() == "kind" && matches!(value, "\"script\"" | "'script'")
    })
}

fn refused(error: PluginError) -> String {
    match error {
        PluginError::Invalid(reason) => format!("the pack was refused: {reason}. create_plugin's description lists the manifest rules"),
        PluginError::Archive(reason) => format!("the .zip was refused: {reason}"),
        PluginError::UnsupportedSource => "the path must be a pack folder or a .zip file".into(),
        PluginError::Reserved => "this id belongs to a pack that ships with the input method; choose another id".into(),
        PluginError::Storage => "the plugins folder in the input method's state directory is a symbolic link or not a folder".into(),
        PluginError::Io(_) => "the pack could not be read or written".into(),
    }
}

fn selection(preferences: &Preferences) -> PluginSelection {
    let plugins = &preferences.plugins;
    let optional = |value: &str| (!value.is_empty()).then(|| value.to_owned());
    PluginSelection {
        key_sound_enabled: plugins.key_sound.enabled,
        key_sound_mode: match plugins.key_sound.mode {
            KeySoundMode::Keys => "keys",
            KeySoundMode::Melody => "melody",
        }
        .into(),
        key_sound_pack: plugins.key_sound.pack.clone(),
        melody_pack: plugins.melody.pack.clone(),
        music_enabled: plugins.music.enabled,
        music_pack: plugins.music.pack.clone(),
        effect_pack: optional(&plugins.effect_pack),
        command_tables: plugins.command_tables.clone(),
        phrase_tables: plugins.phrase_tables.clone(),
        helpcode_pack_quanpin: optional(&plugins.helpcode_pack_quanpin),
        helpcode_pack_shuangpin: optional(&plugins.helpcode_pack_shuangpin),
        command_mode: preferences.local_modes.command,
        quick_phrase_mode: preferences.local_modes.quick_phrase,
    }
}

/// 偏好里点名这个包的条目，不看开关。
fn selected_in(kind: PluginKind, id: &str, preferences: &Preferences) -> Vec<String> {
    let plugins = &preferences.plugins;
    let candidates: Vec<(&str, bool)> = match kind {
        PluginKind::Sound => vec![
            ("plugins.key_sound.pack", plugins.key_sound.pack == id),
            ("plugins.melody.pack", plugins.melody.pack == id),
        ],
        PluginKind::Music => vec![("plugins.music.pack", plugins.music.pack == id)],
        PluginKind::Effect => vec![("plugins.effect_pack", plugins.effect_pack == id)],
        PluginKind::CommandTable => vec![(
            "plugins.command_tables",
            plugins.command_tables.iter().any(|table| table == id),
        )],
        PluginKind::PhraseTable => vec![(
            "plugins.phrase_tables",
            plugins.phrase_tables.iter().any(|table| table == id),
        )],
        PluginKind::Helpcode => vec![
            (
                "plugins.helpcode_pack_quanpin",
                plugins.helpcode_pack_quanpin == id,
            ),
            (
                "plugins.helpcode_pack_shuangpin",
                plugins.helpcode_pack_shuangpin == id,
            ),
        ],
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .filter(|(_, selected)| *selected)
        .map(|(key, _)| key.to_owned())
        .collect()
}

/// 输入法现在按偏好用不用这个包；脚本插件的启用和授权不在偏好里，报 `None`。
fn active(kind: PluginKind, id: &str, preferences: &Preferences) -> Option<bool> {
    let plugins = &preferences.plugins;
    Some(match kind {
        PluginKind::Sound => {
            plugins.key_sound.enabled
                && match plugins.key_sound.mode {
                    KeySoundMode::Keys => plugins.key_sound.pack == id,
                    KeySoundMode::Melody => plugins.melody.pack == id,
                }
        }
        PluginKind::Music => plugins.music.enabled && plugins.music.pack == id,
        PluginKind::Wordbook | PluginKind::SymbolSet => true,
        PluginKind::Effect
        | PluginKind::CommandTable
        | PluginKind::PhraseTable
        | PluginKind::Helpcode => !selected_in(kind, id, preferences).is_empty(),
        #[allow(unreachable_patterns)]
        _ => return None,
    })
}

/// 装好之后用户还要做什么，包才起作用。
fn next_step(summary: &PluginSummary, preferences: &Preferences) -> String {
    let kind = summary.kind();
    if kind.is_script() {
        return "It does not run yet: the user turns it on and grants each capability it asks for in 设置 → 插件 → 脚本插件（实验）. This server never grants capabilities.".into();
    }
    if active(kind, &summary.id, preferences) == Some(true)
        && !matches!(kind, PluginKind::Wordbook | PluginKind::SymbolSet)
    {
        return "It is already in use, so the change takes effect in the input method within a few seconds.".into();
    }
    match &summary.content {
        PluginContent::Sound(pack) if pack.sequence.is_some() => "In 设置 → 插件 → 我的插件, open it and choose 设为按键旋律, which also turns on 按键音 in melody mode.".into(),
        PluginContent::Sound(_) => "In 设置 → 插件 → 我的插件, open it and choose 设为当前音效包, which also turns on 按键音.".into(),
        PluginContent::Music(_) => "In 设置 → 插件 → 我的插件, open it and choose 设为当前音乐包, which also turns on 背景音乐.".into(),
        PluginContent::Effect(_) => "In 设置 → 插件 → 我的插件, open it and choose 使用此特效包. Linux draws no typing effects.".into(),
        PluginContent::CommandTable(_) => {
            let mode = if preferences.local_modes.command { "" } else { " 指令(/ 模式) in 设置 → 输入 → 快捷模式 is off, so turn it on too." };
            format!("In 设置 → 插件 → 我的插件, open it and turn on 启用.{mode} Then, with Chinese punctuation in Quanpin, Shuangpin or Wubi and nothing being typed, type / and a trigger.")
        }
        PluginContent::PhraseTable(_) => {
            let mode = if preferences.local_modes.quick_phrase { "" } else { " 快捷短语 in 设置 → 输入 → 快捷模式 is off, so turn it on too." };
            format!("In 设置 → 插件 → 我的插件, open it and turn on 启用.{mode} Then, in Chinese mode with Quanpin, Shuangpin or Wubi, press Shift+K and type a key.")
        }
        PluginContent::Helpcode(_) => "Choose it as the helpcode scheme in 设置 → 输入 → 辅助码, or open it in 设置 → 插件 → 我的插件 and turn on 用于全拼 or 用于双拼.".into(),
        PluginContent::Wordbook(_) => "It is ready: it shows in 背单词's book list, marked 插件.".into(),
        PluginContent::SymbolSet(_) => "It is ready: its groups show in the symbol panel on Windows, macOS and Linux.".into(),
        #[allow(unreachable_patterns)]
        _ => String::new(),
    }
}

/// 包内容的一句话摘要。脚本插件只在桌面上有这个变体，按它序列化出的字段读，免得这里跟着 client-services 写一遍 cfg。
fn summarize(content: &PluginContent) -> String {
    match content {
        PluginContent::Sound(pack) => match &pack.sequence {
            Some(sequence) => format!(
                "melody of {} notes played on one sample",
                sequence.semitones.len()
            ),
            None => {
                let mut files = pack.files();
                files.sort();
                files.dedup();
                format!("key sounds from {} sample files", files.len())
            }
        },
        PluginContent::Music(pack) => format!("{} music tracks", pack.tracks.len()),
        PluginContent::CommandTable(table) => format!(
            "{} / commands: {}",
            table.commands.len(),
            named(table.commands.iter().map(|row| format!("/{}", row.trigger)))
        ),
        PluginContent::Effect(pack) => format!(
            "{} typing effect at intensity {}",
            serde_json::to_value(pack.style)
                .ok()
                .and_then(|style| style.as_str().map(str::to_owned))
                .unwrap_or_default(),
            pack.intensity
        ),
        PluginContent::PhraseTable(table) => {
            let mut keys: Vec<&str> = Vec::new();
            for row in &table.phrases {
                if !keys.contains(&row.key.as_str()) {
                    keys.push(&row.key);
                }
            }
            format!(
                "{} K-mode phrases under keys {}",
                table.phrases.len(),
                named(keys.into_iter().map(str::to_owned))
            )
        }
        PluginContent::Helpcode(pack) => {
            format!("{} helpcode entries in {}", pack.entries, pack.table)
        }
        PluginContent::Wordbook(pack) => format!("{} words in {}", pack.word_count, pack.file),
        PluginContent::SymbolSet(set) => format!(
            "{} symbol groups: {}",
            set.groups.len(),
            named(set.groups.iter().map(|group| format!(
                "{} ({}, {} items)",
                group.title,
                serde_json::to_value(group.tab)
                    .ok()
                    .and_then(|tab| tab.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                group.items.len()
            )))
        ),
        #[allow(unreachable_patterns)]
        other => script_summary(other),
    }
}

fn script_summary(content: &PluginContent) -> String {
    let value = serde_json::to_value(content).unwrap_or_default();
    let names = |key: &str| -> String {
        value[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().or_else(|| item["trigger"].as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|joined| !joined.is_empty())
            .unwrap_or_else(|| "none".into())
    };
    format!(
        "script plugin; capabilities: {}; hooks: {}; / commands: {}",
        names("permissions"),
        names("hooks"),
        names("commands")
    )
}

/// 最多点名 [`SUMMARY_ITEMS`] 项，多的用省略号。
fn named(items: impl Iterator<Item = String>) -> String {
    let items: Vec<String> = items.collect();
    let mut text = items
        .iter()
        .take(SUMMARY_ITEMS)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > SUMMARY_ITEMS {
        text.push_str(", …");
    }
    text
}

#[cfg(test)]
mod tests;
