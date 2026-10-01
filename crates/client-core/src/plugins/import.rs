//! Installing a pack the user picked: a folder, or a `.zip` archive of one.
//!
//! `validate` runs the same steps without installing anything, for the `msime-pack` tool an author checks a pack with before publishing it: both go through `stage` and `check_staged`, so a pack the tool accepts is a pack import accepts.
//!
//! The pack is copied or extracted into a staging directory beside the kind directories, checked there by the same rules `scan` lists packs by, and only then renamed into `<root>/<kind>/<id>`, replacing an installed pack of that id whole (`skin::folder_import::replace_directory`). A failed or interrupted import therefore never leaves a directory that looks installed, and never a pack that is half one version and half another.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use super::{
    is_builtin, kind_directory, load_directory, PluginError, PluginSummary, MAX_PACK_FILES,
};

/// Serializes every write to the plugins root in this process. The file lock below does the same across processes (the settings window and an input process both install and remove packs), but a lock on a file is not something every platform promises to hold between two threads of one process.
static PLUGIN_ROOT_WRITES: Mutex<()> = Mutex::new(());

/// The file in the plugins root that writers of packs lock, the way `mentions.lock` guards the name list.
pub(crate) const LOCK_FILE: &str = "packs.lock";

/// A staging, set-aside or replaced directory younger than this is left for a later sweep. The shared lock already keeps two writers apart, but it is advisory, and on a file system that does not honour it (some network and FUSE mounts accept the call and lock nothing) a sweep that went by name alone would delete another process's import while it is being written. An import or removal finishes in seconds.
pub(crate) const LEFTOVER_AGE: Duration = Duration::from_secs(60 * 60);

/// Both plugins-root write locks, released together when dropped.
pub(crate) struct RootWrites {
    _file: File,
    _process: MutexGuard<'static, ()>,
}

/// Hold the plugins-root write locks: this process's, then the one every process shares through `LOCK_FILE`. `root` must exist. A write that panicked leaves nothing the next one relies on, since each import sweeps leftovers first, so a poisoned lock is taken over.
pub(super) fn lock_plugin_root(root: &Path) -> io::Result<RootWrites> {
    let process = PLUGIN_ROOT_WRITES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let file = crate::file_lock::open_lock_file(root.join(LOCK_FILE))?;
    crate::file_lock::exclusive(&file)?;
    Ok(RootWrites {
        _file: file,
        _process: process,
    })
}

/// Bytes of an archive the picker may hand over: a music pack at its limits plus notices and zip overhead.
pub const MAX_ARCHIVE_BYTES: u64 = 80 * 1024 * 1024;
/// Members of an archive, macOS resource forks and a wrapping folder included.
const MAX_ARCHIVE_MEMBERS: usize = 64;
/// Bytes `zip` may read while it parses an archive's directory: the end records it searches for (behind a comment of up to 64 KiB) and a directory entry for each of `MAX_ARCHIVE_MEMBERS` members, with room for long names and extra fields. `zip` allocates every entry a directory declares before the member count can be checked, and a zip64 directory in an archive of `MAX_ARCHIVE_BYTES` can declare over a million; this budget refuses one after a few thousand.
const MAX_DIRECTORY_READ_BYTES: u64 = 128 * 1024 + MAX_ARCHIVE_MEMBERS as u64 * 4 * 1024;
/// Bytes of one copied or extracted file, the largest any kind allows.
const MAX_FILE_BYTES: u64 = super::music_pack::MAX_TRACK_BYTES;
/// Bytes of a whole copied or extracted pack.
const MAX_TOTAL_BYTES: u64 = super::music_pack::MAX_PACK_BYTES + 2 * 1024 * 1024;

/// Install the pack at `source` - a folder, or a `.zip` file holding the pack's files either at its top level or inside one folder - under `root`, and return it as `scan` now lists it.
///
/// Names starting with a dot (`.DS_Store`, `.git`) and a zip's `__MACOSX` folder are left behind, as the file managers that add them intend; anything else the pack may not contain - a subdirectory, a symbolic link, a file its manifest does not account for - refuses the whole import.
pub fn import(source: &Path, root: &Path) -> Result<PluginSummary, PluginError> {
    let archive = is_archive(source)?;
    if !crate::storage::create_directory_and_check(root).map_err(|_| PluginError::Storage)? {
        return Err(PluginError::Storage);
    }
    let _writes = lock_plugin_root(root)?;
    sweep_leftovers(root, SystemTime::now());
    let staging = Staging(root.join(format!(".staging-{}", uuid::Uuid::new_v4().simple())));
    fs::create_dir(&staging.0)?;
    stage(source, archive, &staging.0)?;
    let mut summary = check_staged(&staging.0)?;
    let kind = summary.kind();
    let directory = kind_directory(root, kind);
    if !crate::storage::create_directory_and_check(&directory).map_err(|_| PluginError::Storage)? {
        return Err(PluginError::Storage);
    }
    let target = directory.join(&summary.id);
    // A stray file or link where the pack goes is not a pack anyone could have chosen; it is replaced like one.
    if fs::symlink_metadata(&target).is_ok_and(|metadata| !metadata.is_dir()) {
        fs::remove_file(&target)?;
    }
    // Named uniquely, so a backup a crashed import left behind, too young yet for the sweep, cannot stand in the way of this one.
    let backup = directory.join(format!(
        ".replaced-{}-{}",
        summary.id,
        uuid::Uuid::new_v4().simple()
    ));
    crate::skin::folder_import::replace_directory(&staging.0, &target, &backup)
        .map_err(|_| PluginError::Storage)?;
    summary.directory = target;
    Ok(summary)
}

/// Check the pack at `source` - a folder or a `.zip` file, as `import` takes them - by exactly the rules `import` installs it by, without a plugins root and without installing anything. The files are copied or extracted into a temporary directory first, as `import` stages them, so an archive is held to the same member, size and layout rules. The summary's `directory` is `source`.
pub fn validate(source: &Path) -> Result<PluginSummary, PluginError> {
    let archive = is_archive(source)?;
    let staging = tempfile::Builder::new().prefix("msime-pack-").tempdir()?;
    stage(source, archive, staging.path())?;
    let mut summary = check_staged(staging.path())?;
    summary.directory = source.to_path_buf();
    Ok(summary)
}

/// Whether `source` is a `.zip` file (true) or a folder (false); anything else is not a pack source.
fn is_archive(source: &Path) -> Result<bool, PluginError> {
    let metadata = fs::symlink_metadata(source).map_err(|_| PluginError::UnsupportedSource)?;
    if metadata.is_dir() {
        Ok(false)
    } else if metadata.is_file()
        && source
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        Ok(true)
    } else {
        Err(PluginError::UnsupportedSource)
    }
}

/// Copy or extract the pack's files from `source` into the empty directory `staging`.
fn stage(source: &Path, archive: bool, staging: &Path) -> Result<(), PluginError> {
    if archive {
        extract(source, staging)
    } else {
        copy_folder(source, staging)
    }
}

/// The staged pack, checked by the rules `scan` lists packs by, and refused when its id is one the bundle ships.
fn check_staged(staging: &Path) -> Result<PluginSummary, PluginError> {
    let summary = load_directory(staging).map_err(PluginError::Invalid)?;
    if is_builtin(summary.kind(), &summary.id) {
        return Err(PluginError::Reserved);
    }
    Ok(summary)
}

/// Removes the staging directory however the import ends; after a successful rename there is nothing left to remove.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Staging, set-aside and replaced directories a crashed import or removal left behind, once they are `LEFTOVER_AGE` old by their modification time at `now`. One whose age cannot be read is left alone.
pub(crate) fn sweep_leftovers(root: &Path, now: SystemTime) {
    let mut directories = vec![root.to_path_buf()];
    directories.extend(super::PluginKind::ALL.map(|kind| kind_directory(root, kind)));
    for directory in directories {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if [".staging-", ".replaced-", ".old-"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
                && entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .is_ok_and(|modified| {
                        now.duration_since(modified)
                            .is_ok_and(|age| age >= LEFTOVER_AGE)
                    })
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Running totals of one copy or extraction.
#[derive(Default)]
struct Budget {
    files: usize,
    bytes: u64,
}

impl Budget {
    fn take(&mut self, bytes: u64) -> Result<(), PluginError> {
        self.files += 1;
        self.bytes += bytes;
        if self.files > MAX_PACK_FILES || bytes > MAX_FILE_BYTES || self.bytes > MAX_TOTAL_BYTES {
            return Err(PluginError::Invalid("插件太大".into()));
        }
        Ok(())
    }
}

fn copy_folder(source: &Path, staging: &Path) -> Result<(), PluginError> {
    let mut budget = Budget::default();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| PluginError::Invalid("有文件名不是 UTF-8 编码".into()))?;
        if name.starts_with('.') {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(PluginError::Invalid(format!("{name} 是符号链接")));
        }
        if kind.is_dir() {
            return Err(PluginError::Invalid(format!("{name} 是子文件夹")));
        }
        if !kind.is_file() {
            return Err(PluginError::Invalid(format!("{name} 不是普通文件")));
        }
        let input = File::open(entry.path())?;
        let size = input.metadata()?.len();
        budget.take(size)?;
        write_member(staging, &name, input, size)?;
    }
    Ok(())
}

fn extract(source: &Path, staging: &Path) -> Result<(), PluginError> {
    let file = File::open(source)?;
    if file.metadata()?.len() > MAX_ARCHIVE_BYTES {
        return Err(PluginError::Archive("压缩包太大".into()));
    }
    let directory = zip::ZipArchive::new(ReadBudget {
        inner: file,
        left: MAX_DIRECTORY_READ_BYTES,
    })
    .map_err(archive_error)?;
    if directory.len() > MAX_ARCHIVE_MEMBERS {
        return Err(PluginError::Archive("压缩包里的文件太多".into()));
    }
    // The directory is known to be small now; members are read without the budget, each bounded by `Budget` instead.
    let mut archive = zip::ZipArchive::new(directory.into_inner().inner).map_err(archive_error)?;
    // Every file member's path, as plain components, with the members to leave behind already dropped.
    let mut members: Vec<(usize, Vec<String>)> = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let member = archive.by_index_raw(index).map_err(archive_error)?;
        let path = member
            .enclosed_name()
            .ok_or_else(|| PluginError::Archive(format!("{} 指向了压缩包之外", member.name())))?;
        let components = plain_components(&path)
            .ok_or_else(|| PluginError::Archive(format!("{} 不是普通的文件路径", member.name())))?;
        if components.first().is_some_and(|first| first == "__MACOSX")
            || components.iter().any(|part| part.starts_with('.'))
        {
            continue;
        }
        if member.is_symlink() {
            return Err(PluginError::Invalid(format!(
                "{} 是符号链接",
                member.name()
            )));
        }
        if member.is_dir() {
            if components.len() > 1 {
                return Err(PluginError::Invalid(format!(
                    "{} 是子文件夹",
                    member.name()
                )));
            }
            continue;
        }
        members.push((index, components));
    }
    // Either every file sits at the top level, or every file sits in the same single folder, which is what zipping a pack's folder produces.
    let wrapper = match members.first() {
        Some((_, first)) if first.len() == 2 => Some(first[0].clone()),
        _ => None,
    };
    let mut budget = Budget::default();
    for (index, components) in members {
        let name = match (&wrapper, components.as_slice()) {
            (None, [name]) => name.clone(),
            (Some(wrapper), [folder, name]) if folder == wrapper => name.clone(),
            _ => {
                return Err(PluginError::Invalid(format!(
                    "{} 是子文件夹",
                    components.join("/")
                )))
            }
        };
        let member = archive.by_index(index).map_err(archive_error)?;
        let size = member.size();
        budget.take(size)?;
        write_member(staging, &name, member, size)?;
    }
    Ok(())
}

/// A reader that fails once it has read `left` bytes, so parsing an archive's directory cannot run past `MAX_DIRECTORY_READ_BYTES`. Seeking is free.
struct ReadBudget<R> {
    inner: R,
    left: u64,
}

impl<R: Read> Read for ReadBudget<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.left = self
            .left
            .checked_sub(read as u64)
            .ok_or_else(|| io::Error::other(DirectoryTooLarge))?;
        Ok(read)
    }
}

/// `ReadBudget` ran out: the archive's directory declares far more members than a pack may have.
#[derive(Debug)]
struct DirectoryTooLarge;

impl std::fmt::Display for DirectoryTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("压缩包的目录太大，里面的文件远多于插件允许的数量")
    }
}

impl std::error::Error for DirectoryTooLarge {}

/// A `zip` failure as the reason the settings page shows. `zip`'s own messages are English and name format internals, so they become one sentence; the budget this module sets keeps its own.
fn archive_error(error: zip::result::ZipError) -> PluginError {
    if let zip::result::ZipError::Io(io) = &error {
        if let Some(budget) = io
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<DirectoryTooLarge>())
        {
            return PluginError::Archive(budget.to_string());
        }
    }
    PluginError::Archive("不是有效的 zip 文件，或用了不支持的压缩方式".into())
}

impl<R: Seek> Seek for ReadBudget<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

/// A member path as plain components, or `None` if it has anything but.
fn plain_components(path: &Path) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then_some(parts)
}

/// Write one file of the pack into staging, refusing a name the pack may not use or a second file of the same name, and stopping at the declared size even if the reader has more: a zip's recorded size is the archive's claim, not a bound.
fn write_member(
    staging: &Path,
    name: &str,
    input: impl Read,
    declared: u64,
) -> Result<(), PluginError> {
    if !super::valid_file_name(name) {
        return Err(PluginError::Invalid(format!("{name} 不是有效的文件名")));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(staging.join(name))
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => PluginError::Invalid(format!("{name} 出现了两次")),
            _ => PluginError::Io(error),
        })?;
    let copied = io::copy(&mut input.take(declared.saturating_add(1)), &mut output)?;
    if copied != declared {
        return Err(PluginError::Invalid(format!("{name} 的大小与记录的不符")));
    }
    output.flush()?;
    Ok(())
}
