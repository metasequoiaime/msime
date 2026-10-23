//! 官方词库独立更新。网络只发生在设置宿主的后台任务，输入宿主只读取已校验的资源代次。

use crate::voice::local_models::{self, Fetched, Fetcher, LocalModelError};
use crate::{file_lock, resources, storage};
use resources::{Artifact, ResourceSet, ResourceStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str =
    "https://api.github.com/repos/metasequoiaime/msime-dictionary/releases?per_page=100";
const DOWNLOAD: &str = "https://github.com/metasequoiaime/msime-dictionary/releases/download/";
const MANIFEST: &str = "msime-dictionary-manifest.json";
const INTERVAL: u64 = 24 * 60 * 60;
const DOCUMENT_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("dictionary_update_invalid")]
    Invalid,
    #[error("dictionary_update_network")]
    Network,
    #[error("dictionary_update_incompatible")]
    Incompatible,
    #[error("dictionary_update_integrity")]
    Integrity,
    #[error("dictionary_update_storage")]
    Storage,
}

impl From<io::Error> for UpdateError {
    fn from(_: io::Error) -> Self {
        Self::Storage
    }
}

/// 本安装包的资源边界；远端只能更新这些文件，模型等其他仓库资源维持安装包固定的版本。
#[derive(Clone)]
pub struct UpdateContext {
    pub state_root: PathBuf,
    pub resources: PathBuf,
    pub baseline: ResourceSet,
    pub on_demand: Vec<String>,
    pub languages: Option<PathBuf>,
    pub format: i32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub baseline_generation: String,
    pub version: String,
    pub desktop: ResourceSet,
    pub languages: Option<ResourceSet>,
    pub bundled_resources: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Installed {
    active: Option<Selection>,
    previous: Option<Selection>,
}

const LOADED: &str = "loaded.json";

/// 输入宿主最近一次建会话时用的资源目录（规范化路径）。设置宿主和输入宿主常是两个进程，`status` 靠它判断已选中的代次是否还要重新打开输入法才生效。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Loaded {
    resources: PathBuf,
}

/// 输入宿主建会话时调用。从没装过更新时不建目录也不写；内容没变时不重写，避免每次弹出键盘都写一次盘。
pub fn record_loaded(state: &Path, resources: &Path) -> Result<(), UpdateError> {
    let folder = root(state);
    if !folder.is_dir() {
        return Ok(());
    }
    let resources = std::fs::canonicalize(resources)?;
    let path = folder.join(LOADED);
    if read::<Loaded>(&path).is_ok_and(|loaded| loaded.resources == resources) {
        return Ok(());
    }
    write(&path, &Loaded { resources })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    enabled: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateStatus {
    pub enabled: bool,
    pub version: String,
    pub phase: String,
    pub checked_at: u64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
    pub can_rollback: bool,
    pub pending_restart: bool,
}

fn root(state: &Path) -> PathBuf {
    state.join("dictionary-updates")
}
fn store(state: &Path) -> ResourceStore {
    ResourceStore::new(root(state).join("generations"))
}
pub fn directory(state: &Path, set: &ResourceSet) -> Result<PathBuf, UpdateError> {
    msime_path_trust::reject_symlinked_components(state)?;
    Ok(root(&std::fs::canonicalize(state)?)
        .join("generations")
        .join(set.generation().map_err(|_| UpdateError::Invalid)?))
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, UpdateError> {
    msime_path_trust::reject_symlinked_components(path)?;
    let file = file_lock::open_private_file(path)?;
    let mut bytes = Vec::new();
    file.take(DOCUMENT_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > DOCUMENT_LIMIT {
        return Err(UpdateError::Invalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| UpdateError::Invalid)
}

fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), UpdateError> {
    let bytes = serde_json::to_vec(value).map_err(|_| UpdateError::Invalid)?;
    file_lock::replace_private_file(path, &bytes)?;
    Ok(())
}

fn version(tag: &str) -> Option<[u64; 3]> {
    let parts: Vec<_> = tag.strip_prefix("dict-v")?.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some([
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ])
}

pub fn baseline_version(baseline: &ResourceSet) -> String {
    baseline
        .artifacts
        .iter()
        .filter_map(|file| file.url.strip_prefix(DOWNLOAD)?.split('/').next())
        .filter(|tag| version(tag).is_some())
        .max_by_key(|tag| version(tag))
        .unwrap_or("随应用安装")
        .to_owned()
}

/// 发布包可以不带、改由资源包提供的桌面词库文件属于哪个资源包：日文词典那一组是 `japanese`，五笔码表是 `wubi`，其余文件随包，为 `None`。更新只在本机已有这份文件（随包或资源包已装）时才下载它的新版本。
fn on_demand_pack(name: &str) -> Option<crate::resource_packs::ResourcePack> {
    use crate::resource_packs::ResourcePack;
    if resources::ON_DEMAND_JAPANESE_ARTIFACTS.contains(&name) {
        Some(ResourcePack::Japanese)
    } else if resources::ON_DEMAND_WUBI_ARTIFACTS.contains(&name) {
        Some(ResourcePack::Wubi)
    } else {
        None
    }
}

fn validate_selection(selection: &Selection, baseline: &ResourceSet) -> Result<(), UpdateError> {
    if selection.baseline_generation != baseline.generation().map_err(|_| UpdateError::Invalid)?
        || version(&selection.version).is_none()
        || version(&selection.version) < version(&baseline_version(baseline))
        || !selection.bundled_resources.is_absolute()
    {
        return Err(UpdateError::Invalid);
    }
    selection
        .desktop
        .validate()
        .map_err(|_| UpdateError::Invalid)?;
    for file in &selection.desktop.artifacts {
        let pinned = baseline
            .artifacts
            .iter()
            .find(|p| p.name == file.name)
            .ok_or(UpdateError::Invalid)?;
        if pinned.url.starts_with(DOWNLOAD) {
            if file.url != format!("{DOWNLOAD}{}/{}", selection.version, file.name) {
                return Err(UpdateError::Invalid);
            }
        } else if file.url != pinned.url || file.sha256 != pinned.sha256 || file.size != pinned.size
        {
            return Err(UpdateError::Invalid);
        }
    }
    // 允许省去按需下载的日文词典和五笔码表，不能省去其他随包资源。
    if baseline.artifacts.iter().any(|p| {
        on_demand_pack(&p.name).is_none()
            && !selection.desktop.artifacts.iter().any(|a| a.name == p.name)
    }) {
        return Err(UpdateError::Invalid);
    }
    if let Some(languages) = &selection.languages {
        languages.validate().map_err(|_| UpdateError::Invalid)?;
        let pinned: ResourceSet = serde_json::from_str(include_str!(
            "../../../resources/language-dictionaries.lock.json"
        ))
        .map_err(|_| UpdateError::Invalid)?;
        if languages.artifacts.len() != pinned.artifacts.len()
            || languages.artifacts.iter().any(|a| {
                !pinned.artifacts.iter().any(|p| p.name == a.name)
                    || a.url != format!("{DOWNLOAD}{}/{}", selection.version, a.name)
            })
        {
            return Err(UpdateError::Invalid);
        }
    }
    Ok(())
}

/// 不联网；调用方在准备 Engine 代次前再校验文件字节，校验失败保留随包词库。
pub fn selected(state: &Path, baseline: &ResourceSet) -> Option<Selection> {
    let installed: Installed = read(&root(state).join("installed.json")).ok()?;
    let active = installed.active?;
    validate_selection(&active, baseline).ok()?;
    msime_path_trust::reject_symlinked_components(&directory(state, &active.desktop).ok()?).ok()?;
    Some(active)
}

/// 原安装包目录即使回退到基线仍保留，避免拿下载目录去校验旧锁。
pub fn bundled_resources(state: &Path, baseline: &ResourceSet) -> Option<PathBuf> {
    let installed: Installed = read(&root(state).join("installed.json")).ok()?;
    installed
        .active
        .iter()
        .chain(installed.previous.iter())
        .find(|s| validate_selection(s, baseline).is_ok())
        .map(|s| s.bundled_resources.clone())
}

/// 已有输入会话的快照仍按它自己的代次校验，不能被后台刚发布的新选择误判成旧锁。
pub fn set_for_directory(state: &Path, baseline: &ResourceSet, path: &Path) -> Option<ResourceSet> {
    let installed: Installed = read(&root(state).join("installed.json")).ok()?;
    if let Some(selection) = installed
        .active
        .iter()
        .chain(installed.previous.iter())
        .find(|s| {
            validate_selection(s, baseline).is_ok()
                && directory(state, &s.desktop).ok().as_deref() == Some(path)
        })
    {
        return Some(selection.desktop.clone());
    }
    let generation = path.file_name()?.to_str()?;
    if generation.len() != 64 || !generation.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let selection: Selection = read(
        &root(state)
            .join("generations")
            .join(format!("{generation}.json")),
    )
    .ok()?;
    validate_selection(&selection, baseline).ok()?;
    (directory(state, &selection.desktop).ok()?.as_path() == path).then_some(selection.desktop)
}

/// 已发布的可选词库；只做身份和长度检查，完整摘要在后台安装与宿主准备时验证。
pub fn installed_file(state: &Path, baseline: &ResourceSet, name: &str) -> Option<PathBuf> {
    let selection = selected(state, baseline)?;
    for set in std::iter::once(&selection.desktop).chain(selection.languages.iter()) {
        if set.artifacts.iter().any(|a| a.name == name) {
            return generation_file(&directory(state, set).ok()?, set, name);
        }
    }
    None
}

/// `set` 的代次目录 `directory`（[`directory`] 的结果）里的 `name`：路径上没有符号链接，是普通文件，长度与 `set` 记录的一致。`set` 里没有这个文件时为 `None`。和 [`installed_file`] 一样只做身份和长度检查；已经拿到选择记录和代次目录的调用方用它查同一代次里的多个文件，不必每个文件重读一次选择记录。
pub fn generation_file(directory: &Path, set: &ResourceSet, name: &str) -> Option<PathBuf> {
    let artifact = set.artifacts.iter().find(|a| a.name == name)?;
    let path = directory.join(name);
    msime_path_trust::reject_symlinked_components(&path).ok()?;
    let metadata = std::fs::metadata(&path).ok()?;
    (metadata.is_file() && metadata.len() == artifact.size).then_some(path)
}

pub fn due(context: &UpdateContext) -> bool {
    automatic_check_due(&status(context), now())
}

fn automatic_check_due(status: &UpdateStatus, now: u64) -> bool {
    // 墙上时钟回拨时旧节流失效，开始检查后会保存修正后的时间。
    status.enabled
        && now
            .checked_sub(status.checked_at)
            .is_none_or(|elapsed| elapsed >= INTERVAL)
}

pub fn status(context: &UpdateContext) -> UpdateStatus {
    let mut status: UpdateStatus =
        read(&root(&context.state_root).join("status.json")).unwrap_or_default();
    // 上次进程退出后不让界面永远停在下载中；活动任务仍持有跨进程锁。
    if matches!(status.phase.as_str(), "checking" | "downloading") {
        if let Ok(lock) =
            file_lock::open_private_lock_file(root(&context.state_root).join("update.lock"))
        {
            if let Ok(Some(_idle)) = file_lock::ScopedFileLock::try_exclusive(lock) {
                status.phase = "interrupted".into();
                status.error = Some("dictionary_update_interrupted".into());
            }
        }
    }
    status.enabled = read::<Config>(&root(&context.state_root).join("config.json"))
        .unwrap_or_default()
        .enabled;
    let active = selected(&context.state_root, &context.baseline);
    status.version = active
        .as_ref()
        .map(|s| s.version.clone())
        .unwrap_or_else(|| baseline_version(&context.baseline));
    // 设置宿主传入的是自己的随包资源，不是输入法正在用的那份；输入宿主建会话时记下的目录才说明新代次是否已经生效。没有记录时（还没建过会话、写不进状态目录）退回比较调用方自己的资源。
    let recorded = read::<Loaded>(&root(&context.state_root).join(LOADED))
        .ok()
        .map(|loaded| loaded.resources);
    let loaded = recorded.as_deref().unwrap_or(&context.resources);
    let downloaded = std::fs::canonicalize(&context.state_root)
        .map(|state| {
            loaded.starts_with(root(&state)) || loaded.starts_with(root(&context.state_root))
        })
        .unwrap_or(false);
    status.pending_restart = active
        .as_ref()
        .and_then(|s| directory(&context.state_root, &s.desktop).ok())
        .map(|p| p != loaded)
        .unwrap_or((recorded.is_some() || status.phase == "rolled_back") && downloaded);
    status.can_rollback = active.is_some();
    status
}

pub fn configure(context: &UpdateContext, enabled: bool) -> Result<UpdateStatus, UpdateError> {
    storage::create_directory_and_check(&root(&context.state_root))?;
    write(
        &root(&context.state_root).join("config.json"),
        &Config { enabled },
    )?;
    Ok(status(context))
}

pub fn rollback(context: &UpdateContext) -> Result<UpdateStatus, UpdateError> {
    let folder = root(&context.state_root);
    storage::create_directory_and_check(&folder)?;
    let _lock = file_lock::ScopedFileLock::exclusive(file_lock::open_private_lock_file(
        folder.join("update.lock"),
    )?)?;
    let mut installed: Installed = read(&folder.join("installed.json"))?;
    if let Some(previous) = &installed.previous {
        validate_selection(previous, &context.baseline)?;
    }
    let current = installed.active.take();
    installed.active = installed.previous.take();
    installed.previous = current;
    // 回退同时关闭自动更新，避免下一次检查立即装回同一版。
    write(&folder.join("config.json"), &Config { enabled: false })?;
    write(&folder.join("installed.json"), &installed)?;
    let mut result = status(context);
    result.phase = "rolled_back".into();
    result.pending_restart = true;
    result.error = None;
    write(&folder.join("status.json"), &result)?;
    Ok(result)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}

fn artifact(release: &Release, name: &str) -> Result<Artifact, UpdateError> {
    let matches: Vec<_> = release.assets.iter().filter(|a| a.name == name).collect();
    if matches.len() != 1 {
        return Err(UpdateError::Invalid);
    }
    let asset = matches[0];
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or(UpdateError::Integrity)?;
    let expected = format!("{DOWNLOAD}{}/{name}", release.tag_name);
    if asset.browser_download_url != expected {
        return Err(UpdateError::Invalid);
    }
    Ok(Artifact {
        name: name.to_owned(),
        url: expected,
        sha256: sha256.to_owned(),
        size: asset.size,
    })
}

fn client() -> Result<reqwest::blocking::Client, UpdateError> {
    // GitHub 的版本接口和附件都是 https 公网地址；按用户的网络代理设置走。
    crate::proxy::configure_blocking(reqwest::blocking::Client::builder())
        .https_only(true)
        .user_agent("msime-dictionary-updater")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|_| UpdateError::Network)
}

trait Transport {
    fn open(&self, url: &str, timeout: Duration) -> Result<Box<dyn Read>, UpdateError>;
}

struct Http(reqwest::blocking::Client);
impl Transport for Http {
    fn open(&self, url: &str, timeout: Duration) -> Result<Box<dyn Read>, UpdateError> {
        self.0
            .get(url)
            .timeout(timeout)
            .send()
            .and_then(|r| r.error_for_status())
            .map(|response| Box::new(response) as Box<dyn Read>)
            .map_err(|_| UpdateError::Network)
    }
}

fn fetch(transport: &dyn Transport, url: &str, limit: u64) -> Result<Vec<u8>, UpdateError> {
    let mut bytes = Vec::new();
    transport
        .open(url, Duration::from_secs(30))?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| UpdateError::Network)?;
    if bytes.len() as u64 > limit {
        return Err(UpdateError::Invalid);
    }
    Ok(bytes)
}

struct UpdateFetcher<'a> {
    transport: &'a dyn Transport,
    timeout: Duration,
}

impl Fetcher for UpdateFetcher<'_> {
    fn fetch<'a>(&'a self, url: &str, _: u64) -> Result<Fetched<'a>, LocalModelError> {
        self.transport
            .open(url, self.timeout)
            .map(|reader| Fetched { reader, offset: 0 })
            .map_err(|_| LocalModelError::Network("dictionary update".into()))
    }
}

/// 复用资源包的逐文件换源与校验；匿名暂存文件随句柄关闭删除，不把大词库放进内存。
fn download(
    transport: &dyn Transport,
    artifact: &Artifact,
    state: &Path,
    progress: &mut dyn FnMut(u64),
) -> Result<File, UpdateError> {
    let mut output = tempfile::tempfile_in(root(state))?;
    // CDN 只缓存不可变附件，版本 API 始终直连；选择记录继续保留 GitHub 原地址。
    let mirrors = if artifact.url.starts_with(DOWNLOAD) {
        crate::resource_packs::PROJECT_MIRRORS
    } else {
        &[]
    };
    local_models::download_from_mirrors(
        &UpdateFetcher {
            transport,
            timeout: Duration::from_secs(if artifact.name == MANIFEST {
                30
            } else {
                30 * 60
            }),
        },
        mirrors,
        &artifact.url,
        artifact.size,
        &artifact.sha256,
        &mut output,
        &AtomicBool::new(false),
        progress,
    )
    .map_err(|error| match error {
        LocalModelError::Network(_) | LocalModelError::HttpStatus(_) => UpdateError::Network,
        LocalModelError::Io(_) => UpdateError::Storage,
        _ => UpdateError::Integrity,
    })?;
    output.rewind()?;
    Ok(output)
}

#[derive(Deserialize)]
struct ProductManifest {
    manifest_version: u32,
    profile: String,
    format_version: i32,
    custom_dictionary_commit: String,
    engine_compatibility: Compatibility,
    licensing: Licensing,
    files: BTreeMap<String, ProductFile>,
}
#[derive(Deserialize)]
struct Compatibility {
    dictionary_format: i32,
    japanese_model_magic: String,
}
#[derive(Deserialize)]
struct Licensing {
    includes_unlicensed_inputs: bool,
}
#[derive(Deserialize)]
struct ProductFile {
    sha256: String,
    size: u64,
}

fn plan(
    context: &UpdateContext,
    release: &Release,
    bytes: &[u8],
) -> Result<Selection, UpdateError> {
    let manifest: ProductManifest =
        serde_json::from_slice(bytes).map_err(|_| UpdateError::Invalid)?;
    if manifest.manifest_version != 1
        || manifest.profile != "desktop"
        || manifest.format_version != context.format
        || manifest.engine_compatibility.dictionary_format != context.format
        || manifest.engine_compatibility.japanese_model_magic != "MSJPDT1"
        || manifest.licensing.includes_unlicensed_inputs
    {
        return Err(UpdateError::Incompatible);
    }
    let mut desktop = context.baseline.clone();
    desktop.source_commit = manifest.custom_dictionary_commit.clone();
    desktop.artifacts.retain(|a| {
        !context.on_demand.contains(&a.name)
            || context.resources.join(&a.name).is_file()
            || on_demand_pack(&a.name).is_some_and(|pack| {
                crate::resource_packs::installed_file(&context.state_root, pack, &a.name).is_some()
            })
    });
    for file in &mut desktop.artifacts {
        if !file.url.starts_with(DOWNLOAD) {
            continue;
        }
        let incoming = artifact(release, &file.name)?;
        if file.name != MANIFEST {
            let described = manifest.files.get(&file.name).ok_or(UpdateError::Invalid)?;
            if described.sha256 != incoming.sha256 || described.size != incoming.size {
                return Err(UpdateError::Integrity);
            }
        }
        *file = incoming;
    }
    let languages = if context.languages.as_ref().is_some_and(|p| p.is_dir()) {
        let pinned: ResourceSet = serde_json::from_str(include_str!(
            "../../../resources/language-dictionaries.lock.json"
        ))
        .map_err(|_| UpdateError::Invalid)?;
        Some(ResourceSet {
            source_commit: manifest.custom_dictionary_commit,
            artifacts: pinned
                .artifacts
                .iter()
                .map(|p| artifact(release, &p.name))
                .collect::<Result<_, _>>()?,
        })
    } else {
        None
    };
    let bundled_resources = bundled_resources(&context.state_root, &context.baseline)
        .unwrap_or_else(|| context.resources.clone());
    let selection = Selection {
        baseline_generation: context
            .baseline
            .generation()
            .map_err(|_| UpdateError::Invalid)?,
        version: release.tag_name.clone(),
        desktop,
        languages,
        bundled_resources,
    };
    validate_selection(&selection, &context.baseline)?;
    Ok(selection)
}

fn install(
    context: &UpdateContext,
    set: &ResourceSet,
    transport: &dyn Transport,
    status: &mut UpdateStatus,
) -> Result<(), UpdateError> {
    store(&context.state_root)
        .install(set, |file| {
            // 复用当前资源的字节；ResourceStore 始终重新校验，损坏缓存不能被带进新代次。
            let active = selected(&context.state_root, &context.baseline);
            let candidates = [
                Some(context.resources.join(&file.name)),
                context.languages.as_ref().map(|p| p.join(&file.name)),
                active
                    .and_then(|s| directory(&context.state_root, &s.desktop).ok())
                    .map(|p| p.join(&file.name)),
            ];
            for path in candidates.into_iter().flatten() {
                if msime_path_trust::reject_symlinked_components(&path).is_err() {
                    continue;
                }
                if let Ok(mut source) = File::open(&path) {
                    if source.metadata()?.len() != file.size {
                        continue;
                    }
                    let mut digest = Sha256::new();
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        let count = source.read(&mut buffer)?;
                        if count == 0 {
                            break;
                        }
                        digest.update(&buffer[..count]);
                    }
                    if hex::encode(digest.finalize()) == file.sha256 {
                        return Ok(Box::new(File::open(path)?) as Box<dyn Read>);
                    }
                }
            }
            let offset = status.downloaded_bytes;
            let mut reported = offset;
            let source = download(transport, file, &context.state_root, &mut |count| {
                status.downloaded_bytes = offset + count.min(file.size);
                if status.downloaded_bytes.abs_diff(reported) >= 1024 * 1024 || count == file.size {
                    reported = status.downloaded_bytes;
                    let _ = write(&root(&context.state_root).join("status.json"), status);
                }
            })
            .map_err(io::Error::other)?;
            Ok(Box::new(source) as Box<dyn Read>)
        })
        .map_err(|_| UpdateError::Integrity)?;
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 在后台线程执行。跨进程取锁，自动检查每天至多一次；手动检查不受节流与开关影响。
pub fn check(context: &UpdateContext, automatic: bool) -> Result<UpdateStatus, UpdateError> {
    check_with(context, automatic, &Http(client()?))
}

fn check_with(
    context: &UpdateContext,
    automatic: bool,
    transport: &dyn Transport,
) -> Result<UpdateStatus, UpdateError> {
    let folder = root(&context.state_root);
    storage::create_directory_and_check(&folder)?;
    let Some(_lock) = file_lock::ScopedFileLock::try_exclusive(file_lock::open_private_lock_file(
        folder.join("update.lock"),
    )?)?
    else {
        return Ok(status(context));
    };
    let mut status = status(context);
    let checked_at = now();
    if automatic && !automatic_check_due(&status, checked_at) {
        return Ok(status);
    }
    status.phase = "checking".into();
    status.checked_at = checked_at;
    status.error = None;
    status.downloaded_bytes = 0;
    status.total_bytes = 0;
    write(&folder.join("status.json"), &status)?;
    let result = (|| {
        let bytes = fetch(transport, API, 16 * 1024 * 1024)?;
        let mut releases: Vec<Release> =
            serde_json::from_slice(&bytes).map_err(|_| UpdateError::Invalid)?;
        releases.retain(|r| !r.draft && !r.prerelease && version(&r.tag_name).is_some());
        releases.sort_by_key(|r| std::cmp::Reverse(version(&r.tag_name)));
        let current = version(&status.version);
        let active = selected(&context.state_root, &context.baseline);
        let mut candidate = None;
        let mut incompatible = false;
        for release in releases.iter().filter(|r| {
            version(&r.tag_name) > current || (active.is_some() && version(&r.tag_name) == current)
        }) {
            let descriptor = artifact(release, MANIFEST)?;
            if descriptor.size > DOCUMENT_LIMIT {
                return Err(UpdateError::Invalid);
            }
            let mut manifest = Vec::new();
            download(transport, &descriptor, &context.state_root, &mut |_| {})?
                .take(DOCUMENT_LIMIT + 1)
                .read_to_end(&mut manifest)?;
            match plan(context, release, &manifest) {
                Err(UpdateError::Incompatible) => incompatible = true,
                Err(error) => return Err(error),
                Ok(selection) => {
                    let unchanged = active.as_ref().is_some_and(|old| {
                        old.desktop.generation().ok() == selection.desktop.generation().ok()
                            && old.languages.as_ref().and_then(|s| s.generation().ok())
                                == selection
                                    .languages
                                    .as_ref()
                                    .and_then(|s| s.generation().ok())
                    });
                    if unchanged {
                        incompatible = false;
                        break;
                    }
                    candidate = Some(selection);
                    break;
                }
            }
        }
        let Some(selection) = candidate else {
            if incompatible {
                return Err(UpdateError::Incompatible);
            }
            status.phase = "current".into();
            return Ok(());
        };
        status.phase = "downloading".into();
        status.total_bytes = selection
            .desktop
            .artifacts
            .iter()
            .chain(selection.languages.iter().flat_map(|s| &s.artifacts))
            .map(|a| a.size)
            .sum();
        write(&folder.join("status.json"), &status)?;
        install(context, &selection.desktop, transport, &mut status)?;
        if let Some(languages) = &selection.languages {
            install(context, languages, transport, &mut status)?;
        }
        // 下载期间关闭开关时不发布，既有代次保持可用。
        if automatic
            && !read::<Config>(&folder.join("config.json"))
                .unwrap_or_default()
                .enabled
        {
            status.phase = "cancelled".into();
            return Ok(());
        }
        // 保留每个资源代次的清单，长寿命旧会话做快照时仍能按自己的资源验证。
        let generation = selection
            .desktop
            .generation()
            .map_err(|_| UpdateError::Invalid)?;
        write(
            &folder
                .join("generations")
                .join(format!("{generation}.json")),
            &selection,
        )?;
        let old: Installed = read(&folder.join("installed.json")).unwrap_or_default();
        write(
            &folder.join("installed.json"),
            &Installed {
                active: Some(selection),
                previous: old.active,
            },
        )?;
        status.phase = "ready".into();
        Ok(())
    })();
    if let Err(error) = &result {
        status.phase = "failed".into();
        status.error = Some(error.to_string());
    }
    write(&folder.join("status.json"), &status)?;
    result?;
    Ok(self::status(context))
}

#[cfg(test)]
mod tests;
