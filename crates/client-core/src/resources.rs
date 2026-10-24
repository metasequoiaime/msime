//! Install trusted, pinned resource sets without replacing active generations.
//! Transport is injected by the host; filenames, lengths and hashes come from a
//! reviewed product lock, never from an untrusted downloaded manifest alone.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub name: String,
    /// HTTPS download location.
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSet {
    /// Actual source of the data, independently of the Engine code revision.
    pub source_commit: String,
    pub artifacts: Vec<Artifact>,
}

/// 发布包可以不内置、改为按需下载的桌面词库文件（macOS、Android 和 iOS 的发布包都这样做，见 [`on_demand_artifacts`]）。三者作为一个整体出现或缺席：日文词典与它的两份许可文本（Mozc 词典说明里的 IPAdic/ICOT 条款、Mozc 的 BSD 许可）必须同时在场，只缺一部分时按原规则校验失败。
pub const ON_DEMAND_JAPANESE_ARTIFACTS: [&str; 3] = [
    "msime-japanese.dat",
    "msime-mozc_dictionary_oss_README.txt",
    "msime-mozc_LICENSE.txt",
];

/// macOS 发布包可以不内置、改为按需下载的五笔码表（见 [`on_demand_artifacts`]）。拼音用户用不到它：五笔反查编码只在五笔方案里显示，设置页的五笔查码只在五笔可用时出现；缺少它时五笔方案像缺词库的粤拼一样回退到别的方案，日志里的五笔行在准备代次时跳过。五笔是默认方案的版本（五笔版）的发布包始终带着它，这由打包脚本保证，运行时的校验只容许它缺席。
pub const ON_DEMAND_WUBI_ARTIFACTS: [&str; 1] = ["msime-wubi.db"];

/// 目标系统 `target_os`（取值同 `std::env::consts::OS`）的发布包可以不内置的资源文件，按组列出：每组整体出现或缺席（见 [`ResourceSet::as_shipped_in_groups`]）。macOS 是日文词典那一组和五笔码表，Android 和 iOS 是日文词典那一组，其余平台照旧全部内置，返回空列表。iOS 的发布构建以 `MSIME_IOS_OMIT_ON_DEMAND=1` 暂存资源（#7687），键盘扩展的 `EngineResources` 不带这一组；这里不列 iOS 时，运行时按完整清单校验，发布包里的键盘一启动就以 `ExistingGeneration`（缺这三个文件）报「输入运行时准备失败」。
///
/// 按参数判断而不是只写 `cfg!`，测试在任何一台主机上都能检查每个目标的规则；宿主按 `std::env::consts::OS` 取本平台的那一份。
pub const fn on_demand_artifacts(target_os: &str) -> &'static [&'static [&'static str]] {
    if same_text(target_os, "macos") {
        &[&ON_DEMAND_JAPANESE_ARTIFACTS, &ON_DEMAND_WUBI_ARTIFACTS]
    } else if same_text(target_os, "android") || same_text(target_os, "ios") {
        &[&ON_DEMAND_JAPANESE_ARTIFACTS]
    } else {
        &[]
    }
}

/// 常量求值里比较两段文本；`str` 的 `==` 不能在 const fn 里用。
const fn same_text(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("invalid pinned resource set")]
    InvalidManifest,
    #[error("resource length or digest mismatch")]
    Integrity,
    /// Carries what was actually found. This is the one resource error a host cannot reproduce off
    /// the device -- it fires on a directory the host did not stage itself, most often an app bundle
    /// whose contents differ from the staging machine's. Without the listing there is nothing left
    /// to read anywhere on the device.
    #[error("existing resource generation has unexpected files: {0}")]
    ExistingGeneration(String),
    #[error("resource storage or transport failed: {0}")]
    Io(#[from] std::io::Error),
}

impl ResourceSet {
    pub fn validate(&self) -> Result<(), ResourceError> {
        let hex = crate::is_lower_hex;
        if !hex(&self.source_commit, 40) || self.artifacts.is_empty() || self.artifacts.len() > 128
        {
            return Err(ResourceError::InvalidManifest);
        }
        let mut names = HashSet::with_capacity(self.artifacts.len());
        for artifact in &self.artifacts {
            // A flat, portable resource layout. Reject aliases, traversal and device names.
            let stem = artifact
                .name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
                || (stem.len() == 4
                    && (stem.starts_with("com") || stem.starts_with("lpt"))
                    && stem.as_bytes()[3].is_ascii_digit());
            if artifact.name.is_empty()
                || artifact.name.len() > 128
                || artifact.name.starts_with('.')
                || artifact.name.ends_with('.')
                || !crate::is_ascii_identifier_with_dots(&artifact.name)
                || reserved
                || !names.insert(artifact.name.to_ascii_lowercase())
                || !hex(&artifact.sha256, 64)
                || artifact.size > 2 * 1024 * 1024 * 1024
                || !artifact.url.starts_with("https://")
            {
                return Err(ResourceError::InvalidManifest);
            }
        }
        Ok(())
    }

    pub fn generation(&self) -> Result<String, ResourceError> {
        self.validate()?;
        let encoded = serde_json::to_vec(self).map_err(|_| ResourceError::InvalidManifest)?;
        Ok(hex::encode(Sha256::digest(encoded)))
    }

    /// 只保留 `names` 中列出的文件，顺序与锁文件一致，source_commit 不变。
    pub fn only(&self, names: &[&str]) -> ResourceSet {
        self.filtered(|name| names.contains(&name))
    }

    /// 去掉 `names` 中列出的文件，是 [`ResourceSet::only`] 的补集。
    pub fn without(&self, names: &[&str]) -> ResourceSet {
        self.filtered(|name| !names.contains(&name))
    }

    /// 目录实际按哪一份清单发货。`on_demand` 中的文件全部不存在（连符号链接也没有）时，说明这是不内置按需文件的发布包，返回去掉它们的子集；其余情况（列表为空、部分存在、是符号链接、读取出错）一律返回完整清单，让 `verify` 像以前一样报告缺一半或文件损坏。
    pub fn as_shipped_in(&self, directory: &Path, on_demand: &[&str]) -> ResourceSet {
        let all_absent = !on_demand.is_empty()
            && on_demand.iter().all(|name| {
                matches!(
                    fs::symlink_metadata(directory.join(name)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound
                )
            });
        if all_absent {
            self.without(on_demand)
        } else {
            self.clone()
        }
    }

    /// [`ResourceSet::as_shipped_in`] 逐组应用：每组文件全部不存在时去掉这一组，组与组互不影响。
    pub fn as_shipped_in_groups(&self, directory: &Path, groups: &[&[&str]]) -> ResourceSet {
        groups.iter().fold(self.clone(), |set, group| {
            set.as_shipped_in(directory, group)
        })
    }

    fn filtered(&self, keep: impl Fn(&str) -> bool) -> ResourceSet {
        ResourceSet {
            source_commit: self.source_commit.clone(),
            artifacts: self
                .artifacts
                .iter()
                .filter(|artifact| keep(&artifact.name))
                .cloned()
                .collect(),
        }
    }
}

/// Remove stages an installer left when it was killed mid-download. Only called under the
/// exclusive `resources.lock`, so none of them can still be in use. Only real directories are
/// removed, and a failure never stops the install.
#[cfg(unix)]
fn sweep_abandoned_stages(directory: &File) {
    let Ok(entries) = rustix::fs::Dir::read_from(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_bytes();
        if name.starts_with(b"incoming-") && entry.file_type().is_dir() {
            let name = std::ffi::OsStr::from_bytes(name);
            let _ = crate::storage::remove_private_tree_at(directory, name);
        }
    }
}

#[cfg(not(unix))]
fn sweep_abandoned_stages(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let is_stage = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("incoming-"));
        if is_stage && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

pub struct ResourceStore {
    root: PathBuf,
}

#[cfg(unix)]
struct ResourceInstallLock {
    _file: File,
}

#[cfg(unix)]
impl Drop for ResourceInstallLock {
    fn drop(&mut self) {
        // 复制或继承的描述符不能把互斥延长到资源安装结束之后。
        let _ = crate::file_lock::unlock(&self._file);
    }
}

#[cfg(unix)]
struct ResourceStage {
    path: PathBuf,
    parent: File,
    name: OsString,
}

#[cfg(unix)]
impl ResourceStage {
    fn create(root: &Path, parent: &File) -> std::io::Result<Self> {
        loop {
            let name = OsString::from(format!("incoming-{}", uuid::Uuid::new_v4().simple()));
            match rustix::fs::mkdirat(parent, &name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    return Ok(Self {
                        path: root.join(&name),
                        parent: parent.try_clone()?,
                        name,
                    })
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn open_directory(&self) -> std::io::Result<File> {
        crate::storage::open_private_directory_at(&self.parent, &self.name)
    }
}

#[cfg(unix)]
impl Drop for ResourceStage {
    fn drop(&mut self) {
        let _ = crate::storage::remove_private_tree_at(&self.parent, &self.name);
    }
}

#[cfg(any(not(unix), test))]
use crate::file_lock::create_private_file;

#[cfg(unix)]
fn create_private_file_at(directory: &File, name: &str) -> std::io::Result<File> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?;
    Ok(descriptor.into())
}

#[cfg(unix)]
fn publish_generation(
    directory: &File,
    staging_name: &str,
    generation: &str,
) -> std::io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        rustix::fs::renameat_with(
            directory,
            std::ffi::OsStr::new(staging_name),
            directory,
            std::ffi::OsStr::new(generation),
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(Into::into)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    {
        rustix::fs::renameat(
            directory,
            std::ffi::OsStr::new(staging_name),
            directory,
            std::ffi::OsStr::new(generation),
        )
        .map_err(Into::into)
    }
}

impl ResourceStore {
    /// root is an application-owned directory, separate from user learning data.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[cfg(unix)]
    fn ensure_root_still_current(&self, opened: &File) -> std::io::Result<()> {
        use std::os::unix::fs::MetadataExt;

        let current = crate::storage::open_private_directory(&self.root)?;
        let opened = opened.metadata()?;
        let current = current.metadata()?;
        if opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err(std::io::Error::other(
                "resource root changed during installation",
            ));
        }
        Ok(())
    }

    pub fn install(
        &self,
        specification: &ResourceSet,
        mut fetch: impl FnMut(&Artifact) -> Result<Box<dyn Read>, std::io::Error>,
    ) -> Result<PathBuf, ResourceError> {
        let generation = specification.generation()?;
        crate::storage::create_directory_and_check(&self.root)?;
        #[cfg(unix)]
        let root_directory = crate::storage::open_private_directory(&self.root)?;
        #[cfg(unix)]
        let lock = crate::file_lock::open_private_lock_file_at_raw(
            &root_directory,
            std::ffi::OsStr::new("resources.lock"),
        )?;
        #[cfg(not(unix))]
        let lock = crate::file_lock::open_lock_file(self.root.join("resources.lock"))?;
        crate::file_lock::exclusive(&lock)?;
        #[cfg(unix)]
        let _lock = ResourceInstallLock { _file: lock };
        #[cfg(unix)]
        sweep_abandoned_stages(&root_directory);
        #[cfg(not(unix))]
        sweep_abandoned_stages(&self.root);
        let destination = self.root.join(&generation);
        if fs::symlink_metadata(&destination).is_ok() {
            self.verify(&destination, specification)?;
            #[cfg(unix)]
            self.ensure_root_still_current(&root_directory)?;
            return Ok(destination);
        }
        #[cfg(unix)]
        let stage = ResourceStage::create(&self.root, &root_directory)?;
        #[cfg(not(unix))]
        let stage = tempfile::Builder::new()
            .prefix("incoming-")
            .tempdir_in(&self.root)?;
        #[cfg(unix)]
        let stage_directory = stage.open_directory()?;
        for artifact in &specification.artifacts {
            let mut source = fetch(artifact)?;
            #[cfg(unix)]
            let mut output = create_private_file_at(&stage_directory, &artifact.name)?;
            #[cfg(not(unix))]
            let mut output = create_private_file(&stage.path().join(&artifact.name))?;
            copy_verified(source.as_mut(), &mut output, artifact)?;
            output.sync_all()?;
        }
        // Published directories are complete. Existing generations are never overwritten.
        #[cfg(unix)]
        {
            self.ensure_root_still_current(&root_directory)?;
            publish_generation(
                &root_directory,
                stage
                    .path()
                    .file_name()
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "stage has no name")
                    })?
                    .to_str()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "stage name is not UTF-8",
                        )
                    })?,
                &generation,
            )?;
        }
        #[cfg(not(unix))]
        fs::rename(stage.path(), &destination)?;
        #[cfg(unix)]
        self.ensure_root_still_current(&root_directory)?;
        Ok(destination)
    }

    /// Check that `directory` holds exactly the artifacts `specification` pins, each with its pinned length and SHA-256, and nothing else. One exception: a real `helpcodes/` directory is let through for the Engine's helpcode tables. `verify` never writes.
    pub fn verify(
        &self,
        directory: &Path,
        specification: &ResourceSet,
    ) -> Result<(), ResourceError> {
        crate::storage::reject_symlink(directory)?;
        specification.validate()?;
        let kind = fs::symlink_metadata(directory)?.file_type();
        if !kind.is_dir() {
            return Err(ResourceError::ExistingGeneration(format!(
                "{} is not a directory ({})",
                directory.display(),
                describe(kind)
            )));
        }
        let mut expected = HashSet::with_capacity(specification.artifacts.len());
        expected.extend(specification.artifacts.iter().map(|a| a.name.as_str()));
        let mut count = 0;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(ResourceError::ExistingGeneration(format!(
                    "non-UTF-8 entry in {}",
                    directory.display()
                )));
            };
            // The Engine reads its helpcode tables from `helpcodes/` under this same directory. They are an Engine asset rather than part of the pinned dictionary release, so a host that ships them puts them here; only the directory itself is let through, and every pinned file is still checked below.
            if name == HELPCODE_DIRECTORY && kind.is_dir() {
                continue;
            }
            if !kind.is_file() {
                return Err(ResourceError::ExistingGeneration(format!(
                    "{name} in {} is a {}, not a file",
                    directory.display(),
                    describe(kind)
                )));
            }
            if !expected.contains(name) {
                return Err(ResourceError::ExistingGeneration(format!(
                    "{name} in {} is not in the pinned resource set",
                    directory.display()
                )));
            }
            count += 1;
        }
        if count != expected.len() {
            let mut missing = Vec::with_capacity(expected.len());
            missing.extend(
                expected
                    .iter()
                    .filter(|name| !directory.join(name).is_file())
                    .copied(),
            );
            missing.sort_unstable();
            return Err(ResourceError::ExistingGeneration(format!(
                "{} holds {count} of the {} pinned resources, missing: {}",
                directory.display(),
                expected.len(),
                missing.join(", ")
            )));
        }
        for artifact in &specification.artifacts {
            let mut input = crate::storage::open_private_file_in(&directory.join(&artifact.name))?;
            copy_verified(&mut input, &mut std::io::sink(), artifact)?;
        }
        Ok(())
    }
}

/// Where the Engine looks for helpcode tables, relative to the resource directory (`helpcodes/…` in its asset contract).
const HELPCODE_DIRECTORY: &str = "helpcodes";

/// Verification markers are generated locally and contain only the pinned
/// artifact names and metadata. Keep a corrupt or replaced marker from
/// allocating without bound before it is discarded as a cache miss.
const MAX_MARKER_BYTES: u64 = 64 * 1024;

fn describe(kind: std::fs::FileType) -> &'static str {
    if kind.is_dir() {
        "directory"
    } else if kind.is_symlink() {
        "symlink"
    } else if kind.is_file() {
        "file"
    } else {
        "special file"
    }
}

fn copy_verified(
    input: &mut dyn Read,
    output: &mut dyn Write,
    artifact: &Artifact,
) -> Result<(), ResourceError> {
    let mut hash = Sha256::new();
    let mut remaining = artifact.size;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let limit = buffer.len().min((remaining + 1) as usize);
        let count = input.read(&mut buffer[..limit])?;
        if count == 0 {
            break;
        }
        if count as u64 > remaining {
            return Err(ResourceError::Integrity);
        }
        remaining -= count as u64;
        hash.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
    }
    if remaining != 0 || hex::encode(hash.finalize()) != artifact.sha256 {
        return Err(ResourceError::Integrity);
    }
    Ok(())
}

/// A record that one resource directory was verified, so the next start need not hash it again.
///
/// `verify` reads every artifact to recompute its SHA-256. That is the right thing to do once,
/// and the wrong thing to do on every launch: the desktop set is 169 MB, which costs about half a
/// second of hashing before the first keystroke can be served, every time the Server process
/// starts.
///
/// What the marker cannot do is replace the hashes. It records the identity of the *set* and, per
/// file, the size and modification time the verified bytes had. A file whose size or mtime moved is
/// re-hashed; so is one that is missing, and so is the whole set when the specification changes. A
/// replacement crafted to keep both size and mtime would be accepted, which is the trade: the
/// resources sit in the installation directory, so writing there already requires the privileges
/// that hashing at launch would not have stopped anyway.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerifiedMarker {
    /// The specification's generation digest, so a different resource set never matches.
    pub generation: String,
    /// Absolute path of the directory these files were verified in.
    pub directory: String,
    /// `(name, size, modified-nanoseconds)` per artifact, sorted by name.
    pub files: Vec<(String, u64, u128)>,
    /// Every entry in the resource directory, including the Engine-owned `helpcodes` directory.
    /// The fast path must notice an unpinned file appearing after the initial verification; the
    /// full verifier rejects such files, so a marker that does not record the directory shape
    /// would silently skip that check on the next launch.
    pub entries: Vec<String>,
}

impl VerifiedMarker {
    /// Describe `directory` as it is right now, or `None` when any artifact cannot be read.
    pub fn describe(
        directory: &Path,
        specification: &ResourceSet,
    ) -> Result<Option<Self>, ResourceError> {
        crate::storage::reject_symlink(directory)?;
        let mut expected = HashSet::with_capacity(specification.artifacts.len());
        expected.extend(
            specification
                .artifacts
                .iter()
                .map(|artifact| artifact.name.as_str()),
        );
        let Ok(directory_entries) = fs::read_dir(directory) else {
            return Ok(None);
        };
        let mut entries = Vec::with_capacity(specification.artifacts.len() + 1);
        for entry in directory_entries {
            let Ok(entry) = entry else {
                return Ok(None);
            };
            let Ok(kind) = entry.file_type() else {
                return Ok(None);
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Ok(None);
            };
            // Keep the same exception as `verify`: Engine helpcode tables are installed beside
            // the pinned artifacts, but the directory itself must be a real directory.
            if name == HELPCODE_DIRECTORY {
                if !kind.is_dir() {
                    return Ok(None);
                }
            } else if !expected.contains(name.as_str()) || !kind.is_file() {
                return Ok(None);
            }
            entries.push(name);
        }
        entries.sort();
        let mut files = Vec::with_capacity(specification.artifacts.len());
        for artifact in &specification.artifacts {
            // `metadata` follows symlinks. The full verifier rejects them, so use
            // `symlink_metadata` here and force a marker miss instead of letting a symlinked
            // artifact inherit the target file's size and mtime.
            let Ok(metadata) = fs::symlink_metadata(directory.join(&artifact.name)) else {
                return Ok(None);
            };
            if !metadata.file_type().is_file() {
                return Ok(None);
            }
            let Ok(modified) = metadata.modified() else {
                return Ok(None);
            };
            let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) else {
                return Ok(None);
            };
            files.push((
                artifact.name.clone(),
                metadata.len(),
                since_epoch.as_nanos(),
            ));
        }
        files.sort();
        Ok(Some(Self {
            generation: specification.generation()?,
            directory: directory.to_string_lossy().into_owned(),
            files,
            entries,
        }))
    }

    /// Read a marker previously written by [`VerifiedMarker::write`].
    ///
    /// A marker that is absent, unreadable or not the shape this version writes is simply a miss:
    /// the caller hashes, and writes a fresh one.
    pub fn read(path: &Path) -> Option<Self> {
        if path
            .parent()
            .is_some_and(|parent| crate::storage::reject_symlink(parent).is_err())
        {
            return None;
        }
        let metadata = fs::symlink_metadata(path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return None;
        }
        let bytes = crate::bounded_io::read_bounded_file_with(
            crate::storage::open_private_file_in(path).ok()?,
            MAX_MARKER_BYTES,
            || (),
            |_| (),
        )
        .ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write(&self, path: &Path) -> Result<(), ResourceError> {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(ResourceError::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "resource marker is not a regular file",
                )));
            }
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        if !crate::storage::create_directory_and_check(parent)? {
            return Err(ResourceError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "marker parent is not a real directory",
            )));
        }
        let encoded = serde_json::to_vec(self).map_err(|_| ResourceError::InvalidManifest)?;
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(parent)?;
            let name = path.file_name().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "marker has no name")
            })?;
            crate::storage::write_private_file_at(&directory, name, &encoded)?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            temporary.write_all(&encoded)?;
            temporary.as_file().sync_all()?;
            temporary
                .persist(path)
                .map(|_| ())
                .map_err(|error| error.error)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn specification() -> ResourceSet {
        ResourceSet {
            source_commit: "a".repeat(40),
            artifacts: vec![Artifact {
                name: "msime-pinyin.db".into(),
                url: "https://example.invalid/msime-pinyin.db".into(),
                sha256: hex::encode(Sha256::digest(b"fixture")),
                size: 7,
            }],
        }
    }
    fn source(bytes: &[u8]) -> Box<dyn Read> {
        Box::new(Cursor::new(bytes.to_vec()))
    }

    #[cfg(unix)]
    #[test]
    fn staged_resource_file_creation_rejects_a_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.bin");
        fs::write(&target, b"keep").unwrap();
        let path = directory.path().join("staged.bin");
        symlink(&target, &path).unwrap();

        assert!(create_private_file(&path).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn generation_publish_stays_in_an_open_resource_directory() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("resources");
        let outside = parent.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let stage = root.join("incoming-fixture");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("artifact"), b"synthetic").unwrap();
        let directory = crate::storage::open_private_directory(&root).unwrap();
        let moved = parent.path().join("moved");
        fs::rename(&root, &moved).unwrap();
        symlink(&outside, &root).unwrap();

        publish_generation(&directory, "incoming-fixture", "generation").unwrap();

        assert!(moved.join("generation/artifact").is_file());
        assert!(!outside.join("generation").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    #[test]
    fn generation_publish_does_not_replace_a_generation_created_after_the_check() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("incoming-fixture");
        let generation = root.path().join("generation");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("artifact"), b"new").unwrap();
        fs::create_dir(&generation).unwrap();
        let directory = crate::storage::open_private_directory(root.path()).unwrap();

        assert_eq!(
            publish_generation(&directory, "incoming-fixture", "generation")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert!(generation.is_dir());
        assert!(!generation.join("artifact").exists());
        assert_eq!(fs::read(stage.join("artifact")).unwrap(), b"new");
    }

    #[cfg(unix)]
    #[test]
    fn stage_creation_stays_in_an_open_resource_directory() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("resources");
        let outside = parent.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let directory = crate::storage::open_private_directory(&root).unwrap();
        let moved = parent.path().join("moved");
        fs::rename(&root, &moved).unwrap();
        symlink(&outside, &root).unwrap();

        let stage = ResourceStage::create(&root, &directory).unwrap();

        assert!(stage.path().starts_with(&root));
        assert!(moved.join(stage.path().file_name().unwrap()).is_dir());
        assert!(!outside.join(stage.path().file_name().unwrap()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn install_rejects_replaced_root_without_publishing() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("resources");
        let outside = parent.path().join("outside");
        let moved = parent.path().join("moved");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let spec = specification();
        let generation = spec.generation().unwrap();
        let store = ResourceStore::new(&root);

        let result = store.install(&spec, |_| {
            let stage_name = fs::read_dir(&root)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?
                .into_iter()
                .find(|name| name.to_string_lossy().starts_with("incoming-"))
                .expect("created stage");
            fs::create_dir(outside.join(&stage_name))?;
            fs::rename(&root, &moved)?;
            symlink(&outside, &root)?;
            Ok(source(b"fixture"))
        });

        assert!(matches!(result, Err(ResourceError::Io(_))));

        assert!(!moved.join(generation).exists());
        assert!(!fs::read_dir(&outside)
            .unwrap()
            .any(|entry| { entry.unwrap().path().join("msime-pinyin.db").exists() }));
    }

    #[cfg(unix)]
    #[test]
    fn abandoned_stage_sweep_stays_in_an_open_resource_directory() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("resources");
        let outside = parent.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(root.join("incoming-stale")).unwrap();
        fs::write(root.join("incoming-stale/file"), b"synthetic").unwrap();
        let directory = crate::storage::open_private_directory(&root).unwrap();
        let moved = parent.path().join("moved");
        fs::rename(&root, &moved).unwrap();
        symlink(&outside, &root).unwrap();

        sweep_abandoned_stages(&directory);

        assert!(!moved.join("incoming-stale").exists());
        assert!(outside.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_finished_resource_install_releases_the_lock_while_a_copy_remains_open() {
        let root = tempfile::tempdir().unwrap();
        let directory = crate::storage::open_private_directory(root.path()).unwrap();
        let file = crate::file_lock::open_private_lock_file_at_raw(
            &directory,
            std::ffi::OsStr::new("resources.lock"),
        )
        .unwrap();
        crate::file_lock::exclusive(&file).unwrap();
        let held = ResourceInstallLock { _file: file };
        let inherited = held._file.try_clone().unwrap();
        let contender =
            crate::file_lock::open_private_lock_file(root.path().join("resources.lock")).unwrap();
        assert!(!crate::file_lock::try_exclusive(&contender).unwrap());

        drop(held);
        assert!(
            crate::file_lock::try_exclusive(&contender).unwrap(),
            "资源安装结束后不能由旧描述符副本继续占锁"
        );
        drop(inherited);
        let next =
            crate::file_lock::open_private_lock_file(root.path().join("resources.lock")).unwrap();
        assert!(!crate::file_lock::try_exclusive(&next).unwrap());
        crate::file_lock::unlock(&contender).unwrap();
        assert!(crate::file_lock::try_exclusive(&next).unwrap());
        crate::file_lock::unlock(&next).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn resource_install_holds_the_lock_until_success_or_failure_returns() {
        for outcome in ["success", "fetch-error", "integrity-error"] {
            let root = tempfile::tempdir().unwrap();
            let store = ResourceStore::new(root.path());
            let contender =
                crate::file_lock::open_private_lock_file(root.path().join("resources.lock"))
                    .unwrap();
            let result = store.install(&specification(), |_| {
                assert!(!crate::file_lock::try_exclusive(&contender).unwrap());
                match outcome {
                    "fetch-error" => Err(std::io::Error::other("synthetic fetch error")),
                    "integrity-error" => Ok(source(b"invalid")),
                    _ => Ok(source(b"fixture")),
                }
            });
            assert_eq!(result.is_ok(), outcome == "success");
            assert!(crate::file_lock::try_exclusive(&contender).unwrap());
            assert!(!fs::read_dir(root.path()).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("incoming-")));
            crate::file_lock::unlock(&contender).unwrap();
            if result.is_ok() {
                store
                    .install(&specification(), |_| panic!("缓存命中不应下载"))
                    .unwrap();
                assert!(crate::file_lock::try_exclusive(&contender).unwrap());
                crate::file_lock::unlock(&contender).unwrap();
            }
        }
    }

    fn fixture_artifact(name: &str, bytes: &[u8]) -> Artifact {
        Artifact {
            name: name.into(),
            url: format!("https://example.invalid/{name}"),
            sha256: hex::encode(Sha256::digest(bytes)),
            size: bytes.len() as u64,
        }
    }

    /// 在现有夹具上追加三个按需下载的文件，夹在核心文件中间，用来检查顺序保持不变。
    fn desktop_specification() -> ResourceSet {
        let mut set = specification();
        set.artifacts.push(fixture_artifact(
            ON_DEMAND_JAPANESE_ARTIFACTS[0],
            b"japanese",
        ));
        set.artifacts
            .push(fixture_artifact(ON_DEMAND_JAPANESE_ARTIFACTS[1], b"readme"));
        set.artifacts.push(fixture_artifact(
            ON_DEMAND_JAPANESE_ARTIFACTS[2],
            b"license",
        ));
        set.artifacts
            .push(fixture_artifact("msime-english.db", b"english"));
        set
    }

    fn write_core(directory: &Path) {
        fs::write(directory.join("msime-pinyin.db"), b"fixture").unwrap();
        fs::write(directory.join("msime-english.db"), b"english").unwrap();
    }

    fn names(set: &ResourceSet) -> Vec<&str> {
        set.artifacts.iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn only_and_without_partition_the_set_in_lock_order() {
        let spec = desktop_specification();
        let on_demand = spec.only(&ON_DEMAND_JAPANESE_ARTIFACTS);
        let core = spec.without(&ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(
            names(&on_demand),
            [
                "msime-japanese.dat",
                "msime-mozc_dictionary_oss_README.txt",
                "msime-mozc_LICENSE.txt"
            ]
        );
        assert_eq!(names(&core), ["msime-pinyin.db", "msime-english.db"]);
        assert_eq!(on_demand.source_commit, spec.source_commit);
        assert_eq!(core.source_commit, spec.source_commit);
        assert!(on_demand.validate().is_ok() && core.validate().is_ok());
    }

    #[test]
    fn a_core_only_directory_ships_and_verifies_the_subset() {
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        let spec = desktop_specification();
        let shipped = spec.as_shipped_in(directory.path(), &ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(names(&shipped), ["msime-pinyin.db", "msime-english.db"]);
        let store = ResourceStore::new(directory.path());
        assert!(store.verify(directory.path(), &shipped).is_ok());
        assert_ne!(spec.generation().unwrap(), shipped.generation().unwrap());
    }

    #[test]
    fn a_half_present_pair_is_verified_against_the_full_set() {
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        fs::write(directory.path().join("msime-japanese.dat"), b"japanese").unwrap();
        let spec = desktop_specification();
        let shipped = spec.as_shipped_in(directory.path(), &ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(names(&shipped), names(&spec));
        let error = ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .unwrap_err();
        assert!(
            matches!(&error, ResourceError::ExistingGeneration(message) if message.contains("msime-mozc_dictionary_oss_README.txt") && message.contains("msime-mozc_LICENSE.txt")),
            "{error}"
        );
    }

    #[test]
    fn a_complete_directory_ships_the_full_set() {
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        fs::write(directory.path().join("msime-japanese.dat"), b"japanese").unwrap();
        fs::write(
            directory
                .path()
                .join("msime-mozc_dictionary_oss_README.txt"),
            b"readme",
        )
        .unwrap();
        fs::write(directory.path().join("msime-mozc_LICENSE.txt"), b"license").unwrap();
        let spec = desktop_specification();
        let shipped = spec.as_shipped_in(directory.path(), &ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(names(&shipped), names(&spec));
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .is_ok());
    }

    /// 用户词库代际取自完整锁文件的 generation，裁剪发货清单不能改变它。
    #[test]
    fn the_full_generation_is_unchanged() {
        let lock: ResourceSet = serde_json::from_str(include_str!(
            "../../../resources/desktop-dictionary.lock.json"
        ))
        .unwrap();
        let before = lock.generation().unwrap();
        let empty = tempfile::tempdir().unwrap();
        let _ = lock.only(&ON_DEMAND_JAPANESE_ARTIFACTS);
        let _ = lock.without(&ON_DEMAND_JAPANESE_ARTIFACTS);
        let shipped = lock.as_shipped_in(empty.path(), &ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(lock.generation().unwrap(), before);
        assert_eq!(lock.artifacts.len(), 12);
        assert_eq!(shipped.artifacts.len(), 9);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_on_demand_file_still_fails_verification() {
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("msime-japanese.dat");
        fs::write(&target, b"japanese").unwrap();
        std::os::unix::fs::symlink(&target, directory.path().join("msime-japanese.dat")).unwrap();
        let spec = desktop_specification();
        let shipped = spec.as_shipped_in(directory.path(), &ON_DEMAND_JAPANESE_ARTIFACTS);
        assert_eq!(names(&shipped), names(&spec));
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .is_err());
    }

    /// macOS、Android 和 iOS 的发布包可以不带日文词典组，macOS 还可以不带五笔码表；Linux、Windows 以及其他目标照旧要求带齐。
    #[test]
    fn only_macos_android_and_ios_ship_without_the_japanese_group() {
        assert_eq!(
            on_demand_artifacts("macos"),
            [
                ON_DEMAND_JAPANESE_ARTIFACTS.as_slice(),
                ON_DEMAND_WUBI_ARTIFACTS.as_slice()
            ]
        );
        assert_eq!(
            on_demand_artifacts("android"),
            [ON_DEMAND_JAPANESE_ARTIFACTS.as_slice()]
        );
        assert_eq!(
            on_demand_artifacts("ios"),
            [ON_DEMAND_JAPANESE_ARTIFACTS.as_slice()]
        );
        for target in [
            "linux", "windows", "", "androi", "android ", "ios ", "macos ",
        ] {
            assert!(on_demand_artifacts(target).is_empty(), "{target:?}");
        }
        // Android 上不带日文词典组的资源目录按子集校验通过。
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        let spec = desktop_specification();
        let shipped = spec.as_shipped_in_groups(directory.path(), on_demand_artifacts("android"));
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .is_ok());
        let linux = spec.as_shipped_in_groups(directory.path(), on_demand_artifacts("linux"));
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &linux)
            .is_err());
    }

    /// 按组裁剪：五笔码表和日文词典组各自整体缺席或在场，互不影响；macOS 上两组都缺、只缺一组的目录都按各自的子集校验通过，Android 不容许缺五笔码表。
    #[test]
    fn each_on_demand_group_is_left_out_on_its_own() {
        let mut spec = desktop_specification();
        spec.artifacts
            .push(fixture_artifact(ON_DEMAND_WUBI_ARTIFACTS[0], b"wubi"));
        let macos = on_demand_artifacts("macos");
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        let shipped = spec.as_shipped_in_groups(directory.path(), macos);
        assert_eq!(names(&shipped), ["msime-pinyin.db", "msime-english.db"]);
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .is_ok());
        // 五笔版那样带着五笔码表、不带日文词典：五笔码表照常校验。
        fs::write(directory.path().join("msime-wubi.db"), b"wubi").unwrap();
        let shipped = spec.as_shipped_in_groups(directory.path(), macos);
        assert_eq!(
            names(&shipped),
            ["msime-pinyin.db", "msime-english.db", "msime-wubi.db"]
        );
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &shipped)
            .is_ok());
        fs::write(directory.path().join("msime-wubi.db"), b"tampered").unwrap();
        assert!(ResourceStore::new(directory.path())
            .verify(
                directory.path(),
                &spec.as_shipped_in_groups(directory.path(), macos)
            )
            .is_err());
        // Android 只容许缺日文词典组：缺五笔码表的目录按完整清单校验失败。
        fs::remove_file(directory.path().join("msime-wubi.db")).unwrap();
        let android = spec.as_shipped_in_groups(directory.path(), on_demand_artifacts("android"));
        assert!(names(&android).contains(&"msime-wubi.db"));
        assert!(ResourceStore::new(directory.path())
            .verify(directory.path(), &android)
            .is_err());
    }

    #[test]
    fn an_empty_on_demand_list_keeps_the_full_set() {
        let directory = tempfile::tempdir().unwrap();
        write_core(directory.path());
        let spec = desktop_specification();
        assert_eq!(
            names(&spec.as_shipped_in(directory.path(), &[])),
            names(&spec)
        );
    }
    #[test]
    fn an_artifact_needs_an_https_url() {
        let with = |url: &str| {
            let mut set = specification();
            set.artifacts[0].url = url.into();
            set.validate()
        };
        assert!(with("https://example.invalid/a").is_ok());
        assert!(with("").is_err());
        assert!(with("http://example.invalid/a").is_err());
    }

    #[test]
    fn publishes_complete_generation_and_verifies_cached_bytes() {
        let root = tempfile::tempdir().unwrap();
        let store = ResourceStore::new(root.path());
        let spec = specification();
        let path = store.install(&spec, |_| Ok(source(b"fixture"))).unwrap();
        assert_eq!(fs::read(path.join("msime-pinyin.db")).unwrap(), b"fixture");
        assert_eq!(
            store
                .install(&spec, |_| panic!("must not fetch cached resources"))
                .unwrap(),
            path
        );
        fs::write(path.join("msime-pinyin.db"), b"damaged").unwrap();
        assert!(matches!(
            store.install(&spec, |_| panic!("must not overwrite active resources")),
            Err(ResourceError::Integrity)
        ));
    }
    #[test]
    fn rejects_truncated_oversized_and_wrong_digest_without_publishing() {
        for bytes in [b"short".as_slice(), b"fixture-extra", b"invalid"] {
            let root = tempfile::tempdir().unwrap();
            let store = ResourceStore::new(root.path());
            let spec = specification();
            assert!(matches!(
                store.install(&spec, |_| Ok(source(bytes))),
                Err(ResourceError::Integrity)
            ));
            assert!(!root.path().join(spec.generation().unwrap()).exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn install_rejects_a_symlinked_root() {
        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = parent.path().join("resources");
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        let store = ResourceStore::new(&root);

        assert!(store
            .install(&specification(), |_| Ok(source(b"fixture")))
            .is_err());
        assert!(!outside.path().join("resources.lock").exists());
        assert!(outside.path().read_dir().unwrap().next().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn verify_rejects_a_symlinked_ancestor() {
        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = outside.path().join("resources");
        fs::create_dir(&outside_root).unwrap();
        fs::write(outside_root.join("msime-pinyin.db"), b"fixture").unwrap();
        let linked = parent.path().join("linked");
        msime_path_trust::untrusted_symlink(outside.path(), &linked).unwrap();
        let directory = linked.join("resources");
        let store = ResourceStore::new(&directory);

        let error = store.verify(&directory, &specification()).unwrap_err();

        assert!(matches!(error, ResourceError::Io(_)));
    }

    #[cfg(unix)]
    #[test]
    fn marker_description_rejects_a_symlinked_directory() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("msime-pinyin.db"), b"fixture").unwrap();
        let linked = parent.path().join("resources");
        symlink(outside.path(), &linked).unwrap();

        let error = VerifiedMarker::describe(&linked, &specification()).unwrap_err();

        assert!(matches!(error, ResourceError::Io(_)));
    }
    #[test]
    fn stages_an_interrupted_install_left_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let store = ResourceStore::new(root.path());
        let spec = specification();
        let stale = root.path().join("incoming-abandoned");
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join("msime-pinyin.db"), b"fix").unwrap();
        let path = store.install(&spec, |_| Ok(source(b"fixture"))).unwrap();
        assert!(!stale.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
        // A cached install sweeps too, and a file of that name is left alone.
        fs::create_dir(&stale).unwrap();
        fs::write(root.path().join("incoming-note"), b"").unwrap();
        assert_eq!(
            store
                .install(&spec, |_| panic!("must not fetch cached resources"))
                .unwrap(),
            path
        );
        assert!(!stale.exists());
        assert!(root.path().join("incoming-note").is_file());
    }
    #[test]
    fn failed_upgrade_preserves_previous_generation() {
        let root = tempfile::tempdir().unwrap();
        let store = ResourceStore::new(root.path());
        let mut spec = specification();
        let old = store.install(&spec, |_| Ok(source(b"fixture"))).unwrap();
        spec.source_commit = "b".repeat(40);
        assert!(store
            .install(&spec, |_| Err(std::io::Error::other("offline")))
            .is_err());
        assert_eq!(fs::read(old.join("msime-pinyin.db")).unwrap(), b"fixture");
    }
    #[test]
    fn verification_admits_the_engine_helpcode_directory_only() {
        let root = tempfile::tempdir().unwrap();
        let store = ResourceStore::new(root.path());
        let spec = specification();
        let path = store.install(&spec, |_| Ok(source(b"fixture"))).unwrap();
        fs::create_dir(path.join("helpcodes")).unwrap();
        fs::write(path.join("helpcodes/helpcode.txt"), b"a=aa").unwrap();
        assert!(store.verify(&path, &spec).is_ok());
        // A file by that name, or any other extra directory, is still not in the pinned set.
        fs::remove_dir_all(path.join("helpcodes")).unwrap();
        fs::write(path.join("helpcodes"), b"").unwrap();
        assert!(store.verify(&path, &spec).is_err());
        fs::remove_file(path.join("helpcodes")).unwrap();
        fs::create_dir(path.join("extra")).unwrap();
        assert!(store.verify(&path, &spec).is_err());
    }

    #[test]
    fn rejects_path_aliases_and_duplicate_names() {
        for name in [
            "../secret",
            "a/b",
            "a\\b",
            "CON",
            "nul.db",
            "msime-pinyin.db.",
            ".hidden",
        ] {
            let mut spec = specification();
            spec.artifacts[0].name = name.into();
            assert!(spec.validate().is_err());
        }
        let mut spec = specification();
        let mut duplicate = spec.artifacts[0].clone();
        duplicate.name = "MSIME-PINYIN.DB".into();
        spec.artifacts.push(duplicate);
        assert!(spec.validate().is_err());
    }

    /// What the marker is allowed to skip, and what it must not.
    ///
    /// The point of recording a verification is to not hash 169 MB at every launch. The point of
    /// recording it *this* way is that anything which could mean different bytes puts the hashing
    /// back: a different resource set, a file that grew or shrank, a file written again, a file
    /// that is no longer there.
    #[test]
    fn a_recorded_verification_only_matches_the_files_it_recorded() {
        let directory = tempfile::tempdir().unwrap();
        let spec = specification();
        fs::write(directory.path().join("msime-pinyin.db"), b"fixture").unwrap();

        let recorded = VerifiedMarker::describe(directory.path(), &spec)
            .unwrap()
            .expect("every artifact is present");
        assert_eq!(
            VerifiedMarker::describe(directory.path(), &spec).unwrap(),
            Some(recorded.clone()),
            "an untouched directory describes identically, which is what lets the hashing be skipped"
        );

        // A different resource set never matches, even over the same bytes.
        let mut other = specification();
        other.source_commit = "b".repeat(40);
        assert_ne!(
            VerifiedMarker::describe(directory.path(), &other).unwrap(),
            Some(recorded.clone()),
            "the generation is part of the record"
        );

        // 同长度改写后显式推进修改时间，避免模拟器时钟或文件系统精度吞掉固定等待。
        let artifact = directory.path().join("msime-pinyin.db");
        let modified = fs::metadata(&artifact).unwrap().modified().unwrap();
        fs::write(&artifact, b"FIXTURE").unwrap();
        fs::File::options()
            .write(true)
            .open(&artifact)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(modified + std::time::Duration::from_secs(2)),
            )
            .unwrap();
        assert_ne!(
            VerifiedMarker::describe(directory.path(), &spec).unwrap(),
            Some(recorded.clone()),
            "a rewritten file is re-hashed even when its size is unchanged"
        );

        // A different length is caught whatever the clock did.
        fs::write(
            directory.path().join("msime-pinyin.db"),
            b"fixture-and-more",
        )
        .unwrap();
        let grown = VerifiedMarker::describe(directory.path(), &spec)
            .unwrap()
            .expect("still present");
        assert_ne!(grown.files[0].1, recorded.files[0].1);

        // A missing artifact is not describable, so there is nothing to compare and it is hashed.
        fs::remove_file(directory.path().join("msime-pinyin.db")).unwrap();
        assert_eq!(
            VerifiedMarker::describe(directory.path(), &spec).unwrap(),
            None
        );
    }

    #[test]
    fn marker_misses_unpinned_entries_and_symlinked_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let spec = specification();
        fs::write(directory.path().join("msime-pinyin.db"), b"fixture").unwrap();
        assert!(VerifiedMarker::describe(directory.path(), &spec)
            .unwrap()
            .is_some());

        // Resource verification rejects files outside the pinned set. The marker fast path must
        // therefore stop matching when one appears after the initial verification.
        fs::write(directory.path().join("unexpected.db"), b"fixture").unwrap();
        assert_eq!(
            VerifiedMarker::describe(directory.path(), &spec).unwrap(),
            None
        );

        fs::remove_file(directory.path().join("unexpected.db")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target = directory.path().join("target.db");
            fs::write(&target, b"fixture").unwrap();
            fs::remove_file(directory.path().join("msime-pinyin.db")).unwrap();
            symlink(&target, directory.path().join("msime-pinyin.db")).unwrap();
            assert_eq!(
                VerifiedMarker::describe(directory.path(), &spec).unwrap(),
                None
            );
        }
    }

    /// A marker that cannot be read is a miss, not a failure.
    #[test]
    fn an_unusable_marker_falls_back_to_hashing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("verified-resources.json");
        assert_eq!(VerifiedMarker::read(&path), None, "absent");
        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(VerifiedMarker::read(&path), None, "unparseable");
        fs::write(&path, br#"{"generation":"a"}"#).unwrap();
        assert_eq!(VerifiedMarker::read(&path), None, "an older or newer shape");

        let spec = specification();
        let resources = directory.path().join("resources");
        fs::create_dir(&resources).unwrap();
        fs::write(resources.join("msime-pinyin.db"), b"fixture").unwrap();
        let marker = VerifiedMarker::describe(&resources, &spec)
            .unwrap()
            .unwrap();
        marker.write(&path).unwrap();
        assert_eq!(
            VerifiedMarker::read(&path),
            Some(marker.clone()),
            "round trips"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = directory.path().join("outside-marker.json");
            fs::write(&outside, b"keep outside").unwrap();
            fs::remove_file(&path).unwrap();
            symlink(&outside, &path).unwrap();
            assert_eq!(
                VerifiedMarker::read(&path),
                None,
                "a symlinked marker is a cache miss"
            );
            assert!(
                marker.write(&path).is_err(),
                "a symlinked marker is not overwritten"
            );
            assert_eq!(fs::read(&outside).unwrap(), b"keep outside");
            fs::remove_file(&path).unwrap();
        }

        fs::write(&path, vec![b' '; MAX_MARKER_BYTES as usize + 1]).unwrap();
        assert_eq!(
            VerifiedMarker::read(&path),
            None,
            "oversized markers are cache misses"
        );
    }

    #[cfg(unix)]
    #[test]
    fn marker_write_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        fs::create_dir(&resources).unwrap();
        fs::write(resources.join("msime-pinyin.db"), b"fixture").unwrap();
        let marker = VerifiedMarker::describe(&resources, &specification())
            .unwrap()
            .unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();

        assert!(marker
            .write(&linked.join("verified-resources.json"))
            .is_err());
        assert!(!outside.path().join("verified-resources.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn marker_read_ignores_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        fs::create_dir(&resources).unwrap();
        fs::write(resources.join("msime-pinyin.db"), b"fixture").unwrap();
        let marker = VerifiedMarker::describe(&resources, &specification())
            .unwrap()
            .unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("verified-resources.json");
        marker.write(&outside_path).unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();

        assert_eq!(
            VerifiedMarker::read(&linked.join("verified-resources.json")),
            None
        );
    }
}
