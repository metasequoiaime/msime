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
                installed: root_valid && path.join(MANIFEST_FILE).is_file(),
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
    if !target.exists() {
        return Ok(());
    }
    // Renamed aside first so a deletion interrupted halfway never leaves a directory that still carries its manifest.
    let aside = root.join(format!(".old-{}-{}", model.id, unique_suffix()));
    fs::rename(&target, &aside)?;
    fs::remove_dir_all(&aside)?;
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
    // A not-yet-created root can have several missing components below a
    // replaced app-data directory. Find the nearest existing ancestor and
    // inspect that one; walking farther would reject intentional system
    // aliases such as macOS `/var` even though the nearest real directory
    // already anchors the app-owned path.
    let mut current = root.parent();
    while let Some(path) = current {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LocalModelError::InvalidRoot);
            }
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                current = path.parent();
            }
            Err(_) => return Err(LocalModelError::InvalidRoot),
        }
    }
    Ok(())
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
            let _ = fs::remove_dir_all(entry.path());
        }
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
    let manifest = serde_json::to_vec_pretty(&model.manifest).map_err(io::Error::other)?;
    {
        let mut file = fs::File::create(model_dir.join(MANIFEST_FILE))?;
        file.write_all(&manifest)?;
        file.sync_all()?;
    }

    let target = root.join(&model.id);
    let aside = root.join(format!(".old-{}-{}", model.id, unique_suffix()));
    let replaced = if target.exists() {
        fs::rename(&target, &aside)?;
        true
    } else {
        false
    };
    if let Err(error) = fs::rename(&model_dir, &target) {
        if replaced {
            let _ = fs::rename(&aside, &target);
        }
        return Err(error.into());
    }
    if replaced {
        let _ = fs::remove_dir_all(&aside);
    }
    progress(InstallProgress {
        stage: "done",
        downloaded: total,
        total,
    });
    Ok(target)
}

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
    let mut parts = Vec::new();
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
    let needed: Vec<Vec<String>> = model
        .files
        .values()
        .filter_map(|path| relative_components(path))
        .collect();
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
        written += fs::copy(&from, &to)?;
        if written > budget {
            return Err(LocalModelError::UnsafeArchive(
                "archive expands too far".into(),
            ));
        }
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
