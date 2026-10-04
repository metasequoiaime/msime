//! On-device speech models: the pinned catalog, and installing, listing and removing models under a host-chosen root.
//!
//! A model lives in `<root>/<id>/`. The directory is recognised as an installed model by `msime-model.json`, the catalog entry copied verbatim, which the installer writes last and only after every file the entry names is in place; the recognizer (`shared/voice/LocalAsr.cpp`) reads nothing else to find its files. Everything is assembled in a staging directory beside the target and renamed into place, so a crash, a cancel or a failed checksum never leaves a directory that looks installed.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

/// The file that marks a directory as an installed model. Kept in step with `local_model_manifest` in `shared/voice/LocalAsr.cpp`.
pub const MANIFEST_FILE: &str = "msime-model.json";

const CATALOG_JSON: &str = include_str!("../../../../resources/local-asr-models.json");

/// Files the catalog ships inside the repository rather than downloading, by their `resource` name.
const EMBEDDED_RESOURCES: &[(&str, &[u8])] = &[(
    "x-asr-zh-en-bpe.vocab",
    include_bytes!("../../../../resources/voice-models/x-asr-zh-en-bpe.vocab"),
)];

const CHUNK: usize = 64 * 1024;
/// Upper bound on archive members, so a hostile archive of empty entries cannot keep the extractor busy indefinitely.
const MAX_ARCHIVE_ENTRIES: usize = 100_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    pub version: u32,
    pub runtime: String,
    pub models: Vec<CatalogModel>,
    /// Top-level archive members (below the archive root) never extracted.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogModel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub kind: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub streaming: bool,
    #[serde(default)]
    pub languages: Vec<String>,
    pub archive: CatalogArchive,
    #[serde(default)]
    pub extra: Vec<CatalogExtra>,
    /// Role -> path relative to the model directory. A path may name a directory, whose whole subtree is kept.
    pub files: BTreeMap<String, String>,
    /// `native` when the recognizer takes hotwords itself, `pinyin` when they are applied afterwards by `voice::hotwords::correct`.
    #[serde(default)]
    pub hotwords: String,
    #[serde(default)]
    pub modeling_unit: String,
    #[serde(default)]
    pub installed_size: u64,
    /// Human-readable memory estimate as the catalog writes it (`约 0.5 GB`).
    #[serde(default)]
    pub memory: String,
    #[serde(default)]
    pub desktop_only: bool,
    pub license: CatalogLicense,
    /// The entry exactly as the catalog has it, written out as `msime-model.json`.
    #[serde(skip)]
    pub manifest: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogArchive {
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    /// The single top-level directory every member sits under.
    pub root: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogExtra {
    /// File name inside the model directory.
    pub name: String,
    /// Download URL; exactly one of `url` and `resource` is set.
    #[serde(default)]
    pub url: Option<String>,
    /// File under `resources/voice-models/`, embedded in this crate.
    #[serde(default)]
    pub resource: Option<String>,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogLicense {
    pub spdx: String,
    pub source: String,
    #[serde(default)]
    pub terms: Option<String>,
    #[serde(default)]
    pub notice: String,
}

/// One catalog model as a settings page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalModelStatus {
    pub id: String,
    pub title: String,
    pub description: String,
    pub languages: Vec<String>,
    pub streaming: bool,
    pub default: bool,
    pub desktop_only: bool,
    pub installed: bool,
    /// `<root>/<id>`, whether or not it is installed yet; this is what `asr_model_path` is set to.
    pub path: String,
    pub installed_size: u64,
    pub archive_size: u64,
    /// Approximate resident memory while recognising, in bytes, parsed from the catalog's text.
    pub memory: u64,
    pub license_spdx: String,
    pub license_source: String,
    /// Link to the license text when it is not the SPDX identifier's standard one; empty otherwise.
    pub license_terms: String,
    pub license_notice: String,
    pub hotwords: String,
}

/// Install progress. `downloaded`/`total` are archive bytes: received while downloading, consumed while extracting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstallProgress {
    /// `download`, `verify`, `extract` or `done`.
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum LocalModelError {
    #[error("local_model_unknown")]
    UnknownModel,
    #[error("local_model_invalid_root")]
    InvalidRoot,
    #[error("local_model_invalid_mirror")]
    InvalidMirror,
    #[error("local_model_cancelled")]
    Cancelled,
    #[error("local_model_network: {0}")]
    Network(String),
    #[error("local_model_http_status: {0}")]
    HttpStatus(u16),
    #[error("local_model_size_mismatch: {0}")]
    SizeMismatch(String),
    #[error("local_model_checksum_mismatch: {0}")]
    ChecksumMismatch(String),
    #[error("local_model_unsafe_archive: {0}")]
    UnsafeArchive(String),
    #[error("local_model_missing_file: {0}")]
    MissingFile(String),
    #[error("local_model_io: {0}")]
    Io(#[from] io::Error),
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let raw: Value =
            serde_json::from_str(CATALOG_JSON).expect("embedded model catalog is JSON");
        let mut catalog: Catalog =
            serde_json::from_value(raw.clone()).expect("embedded model catalog matches its schema");
        for (model, manifest) in catalog
            .models
            .iter_mut()
            .zip(raw["models"].as_array().expect("catalog models").iter())
        {
            model.manifest = manifest.clone();
        }
        catalog
    })
}

pub fn default_model_id() -> &'static str {
    let catalog = catalog();
    catalog
        .models
        .iter()
        .find(|model| model.default)
        .or_else(|| catalog.models.first())
        .map(|model| model.id.as_str())
        .unwrap_or_default()
}

fn find_model(id: &str) -> Result<&'static CatalogModel, LocalModelError> {
    catalog()
        .models
        .iter()
        .find(|model| model.id == id)
        .ok_or(LocalModelError::UnknownModel)
}

/// Every catalog model with whether it is installed under `root`.
pub fn list(root: &Path) -> Vec<LocalModelStatus> {
    let root_valid = check_root(root).is_ok();
    catalog()
        .models
        .iter()
        .map(|model| {
            let path = root.join(&model.id);
            LocalModelStatus {
                id: model.id.clone(),
                title: model.title.clone(),
                description: model.description.clone(),
                languages: model.languages.clone(),
                streaming: model.streaming,
                default: model.default,
                desktop_only: model.desktop_only,
                installed: root_valid
                    && fs::symlink_metadata(&path)
                        .map(|metadata| metadata.is_dir())
                        .unwrap_or(false)
                    && fs::symlink_metadata(path.join(MANIFEST_FILE))
                        .map(|metadata| metadata.is_file())
                        .unwrap_or(false),
                path: path.to_string_lossy().into_owned(),
                installed_size: model.installed_size,
                archive_size: model.archive.size,
                memory: memory_bytes(&model.memory),
                license_spdx: model.license.spdx.clone(),
                license_source: model.license.source.clone(),
                license_terms: model.license.terms.clone().unwrap_or_default(),
                license_notice: model.license.notice.clone(),
                hotwords: model.hotwords.clone(),
            }
        })
        .collect()
}

/// `约 0.5 GB` -> 500000000. The catalog writes the estimate for people; the number is recovered for sorting and formatting.
fn memory_bytes(text: &str) -> u64 {
    let start = match text.find(|ch: char| ch.is_ascii_digit()) {
        Some(start) => start,
        None => return 0,
    };
    let rest = &text[start..];
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(rest.len());
    let value: f64 = rest[..end].parse().unwrap_or(0.0);
    let unit = rest[end..].trim_start().to_ascii_uppercase();
    let scale = if unit.starts_with("GB") || unit.starts_with('G') {
        1e9
    } else if unit.starts_with("MB") || unit.starts_with('M') {
        1e6
    } else {
        1.0
    };
    (value * scale).round() as u64
}

/// Download, verify and install one catalog model into `<root>/<id>`, replacing any previous install. Blocking; run it off the UI thread. `cancel` is polled between chunks.
pub fn install(
    root: &Path,
    id: &str,
    mirror: &str,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    let model = find_model(id)?;
    install_model(root, model, mirror, &HttpFetcher::new()?, progress, cancel)
}

/// Delete an installed model. Only catalog ids are accepted, so the id can never name anything outside `root`. Removing a model that is not installed succeeds.
pub fn remove(root: &Path, id: &str) -> Result<(), LocalModelError> {
    let model = find_model(id)?;
    check_root(root)?;
    let target = root.join(&model.id);
    remove_leftovers(root, &model.id);
    let target_is_dir = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => return Ok(()),
    };
    // Renamed aside first so a deletion interrupted halfway never leaves a directory that still carries its manifest.
    let aside = root.join(format!(".old-{}-{}", model.id, unique_suffix()));
    fs::rename(&target, &aside)?;
    if target_is_dir {
        fs::remove_dir_all(&aside)?;
    } else {
        fs::remove_file(&aside)?;
    }
    Ok(())
}

fn check_root(root: &Path) -> Result<(), LocalModelError> {
    if !root.is_absolute() || root.to_str().is_none() {
        return Err(LocalModelError::InvalidRoot);
    }
    // `create_dir_all` follows an existing root symlink. Model installation
    // publishes staging directories and downloaded files below this path, so
    // accepting one would let a caller redirect the whole install elsewhere.
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LocalModelError::InvalidRoot);
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(LocalModelError::InvalidRoot),
    }
    // 逐个检查所有祖先；只看最近的已存在目录会漏掉“符号链接后面中间目录已存在”的路径。
    let mut ancestors = Vec::with_capacity(ancestor_capacity(root));
    let mut current = Some(root);
    while let Some(path) = current {
        ancestors.push(path);
        current = path.parent();
    }
    for path in ancestors.into_iter().rev() {
        // 系统自己的符号链接（macOS 的 /var、Android 的 /data/user/0）由 msime-path-trust 列出。
        if msime_path_trust::is_trusted_system_alias(path) {
            continue;
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LocalModelError::InvalidRoot);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(LocalModelError::InvalidRoot),
        }
    }
    Ok(())
}

fn ancestor_capacity(root: &Path) -> usize {
    root.components().count()
}

fn unique_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Staging and set-aside directories a crashed or killed install left behind for this id.
fn remove_leftovers(root: &Path, id: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let staging = format!(".staging-{id}-");
    let old = format!(".old-{id}-");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(&staging) || name.starts_with(&old) {
            remove_leftover(&entry.path());
        }
    }
}

fn remove_leftover(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

/// `https://mirror/` + original URL, the prefix form ghproxy-style mirrors take.
fn mirrored(mirror: &str, url: &str) -> String {
    if mirror.is_empty() {
        url.to_owned()
    } else {
        format!("{}/{}", mirror.trim_end_matches('/'), url)
    }
}

/// Where archive and extra bytes come from. Injected so tests never touch the network.
pub(crate) trait Fetcher {
    fn fetch<'a>(&'a self, url: &str) -> Result<Box<dyn Read + 'a>, LocalModelError>;
}

struct HttpFetcher {
    client: reqwest::blocking::Client,
}

impl HttpFetcher {
    fn new() -> Result<Self, LocalModelError> {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            // GitHub release assets redirect to their CDN; a redirect that leaves HTTPS is refused.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else if attempt.url().scheme() != "https" {
                    attempt.error("redirect left https")
                } else {
                    attempt.follow()
                }
            }))
            .connect_timeout(Duration::from_secs(30))
            // In the blocking client this bounds each read rather than the whole transfer, so a stalled connection fails without capping a slow but steady download.
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("msime/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| LocalModelError::Network(error.to_string()))?;
        Ok(Self { client })
    }
}

impl Fetcher for HttpFetcher {
    fn fetch<'a>(&'a self, url: &str) -> Result<Box<dyn Read + 'a>, LocalModelError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| LocalModelError::Network(error.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(LocalModelError::HttpStatus(response.status().as_u16()));
        }
        Ok(Box::new(response))
    }
}

/// Removes the staging directory however the install ends.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn install_model(
    root: &Path,
    model: &CatalogModel,
    mirror: &str,
    fetcher: &dyn Fetcher,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    check_root(root)?;
    if !crate::preferences::valid_model_mirror(mirror) {
        return Err(LocalModelError::InvalidMirror);
    }
    fs::create_dir_all(root)?;
    remove_leftovers(root, &model.id);
    let staging = Staging(root.join(format!(".staging-{}-{}", model.id, unique_suffix())));
    fs::create_dir(&staging.0)?;
    let model_dir = staging.0.join("model");
    fs::create_dir(&model_dir)?;

    let total = model.archive.size;
    let archive = staging.0.join("archive.tar.bz2");
    let digest = {
        let mut output = BufWriter::new(fs::File::create(&archive)?);
        let mut last = 0u64;
        let digest = download(
            fetcher,
            &mirrored(mirror, &model.archive.url),
            total,
            &mut output,
            cancel,
            &mut |downloaded| {
                // About two hundred updates over the whole download is smooth enough for a bar and cheap to deliver across a C or IPC boundary.
                if downloaded == total || downloaded - last >= (total / 200).max(CHUNK as u64) {
                    last = downloaded;
                    progress(InstallProgress {
                        stage: "download",
                        downloaded,
                        total,
                    });
                }
            },
        )?;
        output.flush()?;
        digest
    };
    progress(InstallProgress {
        stage: "verify",
        downloaded: total,
        total,
    });
    if !digest.eq_ignore_ascii_case(&model.archive.sha256) {
        return Err(LocalModelError::ChecksumMismatch(
            model.archive.name.clone(),
        ));
    }

    extract(
        &archive,
        model,
        &catalog().exclude,
        &model_dir,
        progress,
        cancel,
    )?;
    fs::remove_file(&archive)?;

    for extra in &model.extra {
        check_cancel(cancel)?;
        let name = single_component(&extra.name)
            .ok_or_else(|| LocalModelError::UnsafeArchive(extra.name.clone()))?;
        let destination = model_dir.join(name);
        match (&extra.resource, &extra.url) {
            (Some(resource), _) => {
                let bytes = EMBEDDED_RESOURCES
                    .iter()
                    .find(|(name, _)| name == resource)
                    .map(|(_, bytes)| *bytes)
                    .ok_or_else(|| LocalModelError::MissingFile(resource.clone()))?;
                if bytes.len() as u64 != extra.size {
                    return Err(LocalModelError::SizeMismatch(extra.name.clone()));
                }
                if !hex::encode(Sha256::digest(bytes)).eq_ignore_ascii_case(&extra.sha256) {
                    return Err(LocalModelError::ChecksumMismatch(extra.name.clone()));
                }
                fs::write(&destination, bytes)?;
            }
            (None, Some(url)) => {
                let mut output = BufWriter::new(fs::File::create(&destination)?);
                let digest = download(
                    fetcher,
                    &mirrored(mirror, url),
                    extra.size,
                    &mut output,
                    cancel,
                    &mut |_| {},
                )?;
                output.flush()?;
                if !digest.eq_ignore_ascii_case(&extra.sha256) {
                    return Err(LocalModelError::ChecksumMismatch(extra.name.clone()));
                }
            }
            (None, None) => return Err(LocalModelError::MissingFile(extra.name.clone())),
        }
    }

    for relative in model.files.values() {
        let components = relative_components(relative)
            .ok_or_else(|| LocalModelError::UnsafeArchive(relative.clone()))?;
        let path = components
            .iter()
            .fold(model_dir.clone(), |path, part| path.join(part));
        if !path.exists() {
            return Err(LocalModelError::MissingFile(relative.clone()));
        }
    }
    check_cancel(cancel)?;
    write_manifest(&model_dir, &model.manifest)?;
    let target = publish(root, &model.id, &model_dir)?;
    progress(InstallProgress {
        stage: "done",
        downloaded: total,
        total,
    });
    Ok(target)
}

/// 把 `msime-model.json` 写进暂存目录；它总是该目录里最后写入的文件，有它才算安装完整。
fn write_manifest(dir: &Path, manifest: &Value) -> Result<(), LocalModelError> {
    let manifest = serde_json::to_vec_pretty(manifest).map_err(io::Error::other)?;
    let mut file = fs::File::create(dir.join(MANIFEST_FILE))?;
    file.write_all(&manifest)?;
    file.sync_all()?;
    Ok(())
}

/// 把完整的暂存目录移到 `<root>/<id>`：先把旧安装改名挪开，移动失败时再挪回来，成功后删除旧安装。
fn publish(root: &Path, id: &str, staged: &Path) -> Result<PathBuf, LocalModelError> {
    let target = root.join(id);
    let aside = root.join(format!(".old-{}-{}", id, unique_suffix()));
    let replaced = if fs::symlink_metadata(&target).is_ok() {
        fs::rename(&target, &aside)?;
        true
    } else {
        false
    };
    if let Err(error) = fs::rename(staged, &target) {
        if replaced {
            let _ = fs::rename(&aside, &target);
        }
        return Err(error.into());
    }
    if replaced {
        remove_leftover(&aside);
    }
    Ok(target)
}

/// 下载一组固定的文件（名称、URL、长度、SHA-256 都来自仓库里审过的锁文件）到 `<root>/<id>`，写入 `manifest` 作为 `msime-model.json` 并整体发布，替换之前的安装。阻塞调用，不要放在 UI 线程。
pub fn install_files(
    root: &Path,
    id: &str,
    files: &[crate::resources::Artifact],
    manifest: &Value,
    mirror: &str,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    install_files_with(
        root,
        id,
        files,
        manifest,
        mirror,
        &HttpFetcher::new()?,
        progress,
        cancel,
    )
}

/// 不变量：已经发布的文件永远不会被重新打开写入。输入法会内存映射 dict_japanese.dat，原地改写会让正在使用的映射读到半新半旧的内容甚至触发 SIGBUS；所以新文件一律写进暂存目录，再整体改名替换旧目录，旧文件只被改名和删除，已打开的句柄仍能读到原来的字节。
#[allow(clippy::too_many_arguments)]
pub(crate) fn install_files_with(
    root: &Path,
    id: &str,
    files: &[crate::resources::Artifact],
    manifest: &Value,
    mirror: &str,
    fetcher: &dyn Fetcher,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    check_root(root)?;
    if !crate::preferences::valid_model_mirror(mirror) {
        return Err(LocalModelError::InvalidMirror);
    }
    if single_component(id).is_none_or(|single| single != id) || id.starts_with('.') {
        return Err(LocalModelError::UnknownModel);
    }
    fs::create_dir_all(root)?;
    remove_leftovers(root, id);
    let staging = Staging(root.join(format!(".staging-{}-{}", id, unique_suffix())));
    fs::create_dir(&staging.0)?;
    let pack_dir = staging.0.join("model");
    fs::create_dir(&pack_dir)?;

    let total = files
        .iter()
        .fold(0u64, |sum, file| sum.saturating_add(file.size));
    let mut offset = 0u64;
    let mut last = 0u64;
    for file in files {
        check_cancel(cancel)?;
        let name = single_component(&file.name)
            .filter(|single| *single == file.name)
            .ok_or_else(|| LocalModelError::UnsafeArchive(file.name.clone()))?;
        let mut output = BufWriter::new(fs::File::create(pack_dir.join(&name))?);
        let digest = download(
            fetcher,
            &mirrored(mirror, &file.url),
            file.size,
            &mut output,
            cancel,
            &mut |n| {
                let downloaded = offset + n;
                if downloaded == total || downloaded - last >= (total / 200).max(CHUNK as u64) {
                    last = downloaded;
                    progress(InstallProgress {
                        stage: "download",
                        downloaded,
                        total,
                    });
                }
            },
        )?;
        let output = output.into_inner().map_err(|error| error.into_error())?;
        output.sync_all()?;
        if !digest.eq_ignore_ascii_case(&file.sha256) {
            return Err(LocalModelError::ChecksumMismatch(file.name.clone()));
        }
        offset += file.size;
    }
    progress(InstallProgress {
        stage: "verify",
        downloaded: total,
        total,
    });
    write_manifest(&pack_dir, manifest)?;
    check_cancel(cancel)?;
    let target = publish(root, id, &pack_dir)?;
    progress(InstallProgress {
        stage: "done",
        downloaded: total,
        total,
    });
    Ok(target)
}

/// 读取 `<root>/<id>/msime-model.json`。只接受不超过 64 KiB 的普通文件，符号链接、目录或无法解析的内容都视为没有。
pub fn installed_manifest(root: &Path, id: &str) -> Option<Value> {
    if check_root(root).is_err() {
        return None;
    }
    if single_component(id).is_none_or(|single| single != id) || id.starts_with('.') {
        return None;
    }
    let directory = root.join(id);
    if !fs::symlink_metadata(&directory).ok()?.file_type().is_dir() {
        return None;
    }
    let path = directory.join(MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        fs::File::open(&path).ok()?,
        MAX_MANIFEST_BYTES,
        || (),
        |_| (),
    )
    .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// `msime-model.json` 由安装器在本机写出，正常只有几 KB；限制大小，免得被替换的文件占用无界内存。
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

fn check_cancel(cancel: &AtomicBool) -> Result<(), LocalModelError> {
    if cancel.load(Ordering::Relaxed) {
        Err(LocalModelError::Cancelled)
    } else {
        Ok(())
    }
}

/// Stream `url` into `output`, hashing as it goes. Fails as soon as more than `expected` bytes arrive, and at the end on any shortfall. Returns the lowercase hex SHA-256.
fn download(
    fetcher: &dyn Fetcher,
    url: &str,
    expected: u64,
    output: &mut dyn Write,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<String, LocalModelError> {
    if !url.starts_with("https://") {
        return Err(LocalModelError::Network(
            "only https downloads are allowed".into(),
        ));
    }
    check_cancel(cancel)?;
    let mut reader = fetcher.fetch(url)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut downloaded = 0u64;
    loop {
        check_cancel(cancel)?;
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(LocalModelError::Network(error.to_string())),
        };
        downloaded += read as u64;
        if downloaded > expected {
            return Err(LocalModelError::SizeMismatch(url_file_name(url)));
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
        progress(downloaded);
    }
    if downloaded != expected {
        return Err(LocalModelError::SizeMismatch(url_file_name(url)));
    }
    Ok(hex::encode(hasher.finalize()))
}

fn url_file_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or_default().to_owned()
}

/// A path as plain, relative components: no root, prefix, `..`, empty or separator-bearing parts. `None` for anything else.
fn relative_components(path: &str) -> Option<Vec<String>> {
    let mut parts = Vec::with_capacity(relative_component_capacity(path));
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                let part = part.to_str()?;
                // A backslash or colon is an ordinary character on Unix and a separator or drive on Windows; refuse it on both so an archive extracts the same everywhere.
                if part.is_empty() || part.contains(['\\', ':']) {
                    return None;
                }
                parts.push(part.to_owned());
            }
            _ => return None,
        }
    }
    Some(parts)
}

fn relative_component_capacity(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    path.bytes()
        .filter(|byte| matches!(*byte, b'/' | b'\\'))
        .count()
        .saturating_add(1)
}

fn single_component(name: &str) -> Option<String> {
    let parts = relative_components(name)?;
    match parts.as_slice() {
        [single] => Some(single.clone()),
        _ => None,
    }
}

/// Counts bytes read from the compressed archive, for extraction progress.
struct Counting<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

fn copy_link_with_budget(
    source: &Path,
    destination: &Path,
    written: &mut u64,
    budget: u64,
) -> Result<(), LocalModelError> {
    let size = fs::metadata(source)?.len();
    let next = written
        .checked_add(size)
        .ok_or_else(|| LocalModelError::UnsafeArchive("archive expands too far".into()))?;
    if next > budget {
        return Err(LocalModelError::UnsafeArchive(
            "archive expands too far".into(),
        ));
    }
    fs::copy(source, destination)?;
    *written = next;
    Ok(())
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.count.set(self.count.get() + read as u64);
        Ok(read)
    }
}

/// Unpack the members the model needs from `archive` into `model_dir`. Members outside `archive.root`, or with absolute or `..` paths, or links that resolve outside the model, fail the install; members the model does not name are skipped. Links inside the model are materialised as copies so the result needs no symlink support on Windows.
fn extract(
    archive: &Path,
    model: &CatalogModel,
    exclude: &[String],
    model_dir: &Path,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<(), LocalModelError> {
    let total = model.archive.size;
    let consumed = Rc::new(Cell::new(0u64));
    let reader = Counting {
        inner: fs::File::open(archive)?,
        count: consumed.clone(),
    };
    let decoder = bzip2::read::MultiBzDecoder::new(BufReader::with_capacity(CHUNK, reader));
    let mut tar = tar::Archive::new(decoder);
    let mut needed = Vec::with_capacity(model.files.len());
    needed.extend(
        model
            .files
            .values()
            .filter_map(|path| relative_components(path)),
    );
    let is_needed = |relative: &[String]| {
        needed
            .iter()
            .any(|wanted| relative.len() >= wanted.len() && relative[..wanted.len()] == wanted[..])
    };
    // Generous against the catalog's estimate, only there to stop a decompression bomb filling the disk.
    let budget = model
        .installed_size
        .saturating_mul(2)
        .saturating_add(256 * 1024 * 1024);
    let mut written = 0u64;
    // (destination, source), both model-relative: links are copied once every regular file is out.
    let mut pending: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut last_report = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    progress(InstallProgress {
        stage: "extract",
        downloaded: 0,
        total,
    });
    let entries = tar.entries().map_err(unsafe_archive)?;
    for (index, entry) in entries.enumerate() {
        check_cancel(cancel)?;
        if index >= MAX_ARCHIVE_ENTRIES {
            return Err(LocalModelError::UnsafeArchive("too many members".into()));
        }
        let mut entry = entry.map_err(unsafe_archive)?;
        let raw_path = entry.path().map_err(unsafe_archive)?.into_owned();
        let display = raw_path.to_string_lossy().into_owned();
        let parts = raw_path
            .to_str()
            .and_then(relative_components)
            .ok_or_else(|| LocalModelError::UnsafeArchive(display.clone()))?;
        let relative = match parts.split_first() {
            Some((first, rest)) if *first == model.archive.root => rest.to_vec(),
            _ => return Err(LocalModelError::UnsafeArchive(display)),
        };
        if relative.is_empty()
            || exclude.iter().any(|excluded| *excluded == relative[0])
            || !is_needed(&relative)
        {
            continue;
        }
        let destination = relative
            .iter()
            .fold(model_dir.to_path_buf(), |path, part| path.join(part));
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(&destination)?;
        } else if kind.is_file() || kind.is_contiguous() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = BufWriter::new(fs::File::create(&destination)?);
            loop {
                check_cancel(cancel)?;
                let read = match entry.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(unsafe_archive(error)),
                };
                written += read as u64;
                if written > budget {
                    return Err(LocalModelError::UnsafeArchive(
                        "archive expands too far".into(),
                    ));
                }
                output.write_all(&buffer[..read])?;
                let now = consumed.get().min(total);
                if now - last_report >= (total / 200).max(CHUNK as u64) {
                    last_report = now;
                    progress(InstallProgress {
                        stage: "extract",
                        downloaded: now,
                        total,
                    });
                }
            }
            output.flush()?;
        } else if kind.is_symlink() || kind.is_hard_link() {
            let target = entry
                .link_name()
                .map_err(unsafe_archive)?
                .and_then(|target| target.to_str().map(str::to_owned))
                .ok_or_else(|| LocalModelError::UnsafeArchive(display.clone()))?;
            let source = if kind.is_symlink() {
                // Relative to the link's own directory.
                resolve_inside(&relative[..relative.len() - 1], &target)
            } else {
                // A hard link names another member by its archive path.
                resolve_inside(&[], &target).and_then(|parts| match parts.split_first() {
                    Some((first, rest)) if *first == model.archive.root => Some(rest.to_vec()),
                    _ => None,
                })
            }
            .ok_or_else(|| LocalModelError::UnsafeArchive(format!("{display} -> {target}")))?;
            pending.push((relative, source));
        }
        // Device nodes, FIFOs and the like are never part of a model and are skipped.
    }
    for (destination, source) in pending {
        let join = |parts: &[String]| {
            parts
                .iter()
                .fold(model_dir.to_path_buf(), |path, part| path.join(part))
        };
        let (from, to) = (join(&source), join(&destination));
        if !from.is_file() {
            return Err(LocalModelError::UnsafeArchive(format!(
                "{} -> {}",
                destination.join("/"),
                source.join("/")
            )));
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_link_with_budget(&from, &to, &mut written, budget)?;
    }
    progress(InstallProgress {
        stage: "extract",
        downloaded: total,
        total,
    });
    Ok(())
}

/// `target` resolved against `base` (both model-relative), or `None` if it is absolute or climbs above the model directory.
fn resolve_inside(base: &[String], target: &str) -> Option<Vec<String>> {
    let mut parts = base.to_vec();
    for component in Path::new(target).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(part) => {
                let part = part.to_str()?;
                if part.is_empty() || part.contains(['\\', ':']) {
                    return None;
                }
                parts.push(part.to_owned());
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then_some(parts)
}

fn unsafe_archive(error: io::Error) -> LocalModelError {
    LocalModelError::UnsafeArchive(error.to_string())
}

#[cfg(test)]
mod tests;
