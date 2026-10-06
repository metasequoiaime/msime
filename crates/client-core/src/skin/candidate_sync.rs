//! Keeping the skin root and the signed-in user's candidate-skin library in step.
//!
//! Every installed package the sharing rules accept is uploaded to the user's library as a private package, so it survives a reinstall and follows the account to another device, and every package in the library is installed locally. A small state file beside the skin root remembers, per package, which library row it was last synced with, the [`request_digest`] of that row's content and the digest of the local content at that moment. With both sides compared against it a run can tell which side changed: a local edit is uploaded, a change made on another device is downloaded, and a package removed on one side is removed on the other. A package with no remembered state is never deleted anywhere; when both sides hold it and differ, the local copy wins.
//!
//! Packages installed from someone else's publication are left alone: they are recorded by [`record_install`], and uploading them would copy another author's work into this library.

use super::candidate_community::{
    self, pack_as, request_digest, BackendCandidateSkinCommunityService, CandidateSkinCategory,
    CandidateSkinCommunityApi, CandidateSkinItem, CandidateSkinPackage,
    CandidateSkinPublishRequest, CandidateSkinReplaceRequest, CandidateSkinSyncEntry,
    CandidateSkinVisibility, PackedSkin,
};
use super::catalog;
use crate::account::{AccountApi, AccountError, AccountSessionStorage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

/// The state file's name; hosts keep it in the directory that holds the skin root.
pub const STATE_FILE: &str = "candidate-skin-sync.json";
const MAX_STATE_BYTES: u64 = 1 << 20;
const RATE_LIMITED: &str = "account_rate_limited";
const LIBRARY_LIMIT: &str = "candidate_skin_library_limit";
const PACKAGE: &str = "candidate_skin_package";
const STORAGE: &str = "storage";
/// A public package removed locally: taking it down would clear its downloads and ratings, so that stays a choice made in the gallery.
const PUBLIC_KEPT: &str = "candidate_skin_public_kept";

/// One run at a time in this process, and no state-file update from [`record_install`] or [`forget`] in the middle of one, since a run writes back the state it read.
static SYNC_RUNS: Mutex<()> = Mutex::new(());

fn lock_runs() -> MutexGuard<'static, ()> {
    SYNC_RUNS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The account side of a sync run: the calls it makes, as the signed-in user.
pub trait CandidateSkinSyncRemote {
    /// The signed-in user's id, or `None` when signed out.
    fn user_id(&self) -> Result<Option<String>, AccountError>;
    /// The generation of the signed-in session, when the remote can bind a run to one.
    fn session_generation(&self) -> Result<Option<u64>, AccountError> {
        Ok(None)
    }
    /// Hold the account generation stable while applying a local side effect.
    fn with_session_generation<T, F>(
        &self,
        _generation: u64,
        _user_id: &str,
        operation: F,
    ) -> Result<T, AccountError>
    where
        F: FnOnce() -> Result<T, AccountError>,
    {
        if self.user_id()?.as_deref() != Some(_user_id)
            || self.session_generation()? != Some(_generation)
        {
            return Err(AccountError::Cancelled);
        }
        operation()
    }
    fn sync_list(&self) -> Result<Vec<CandidateSkinSyncEntry>, AccountError>;
    fn detail(&self, id: Uuid) -> Result<CandidateSkinItem, AccountError>;
    fn publish(
        &self,
        request: &CandidateSkinPublishRequest,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn replace(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn set_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn set_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn download(&self, id: Uuid) -> Result<CandidateSkinPackage, AccountError>;
    fn unpublish(&self, id: Uuid) -> Result<(), AccountError>;
}

impl<A, S> CandidateSkinSyncRemote for BackendCandidateSkinCommunityService<A, S>
where
    A: AccountApi + CandidateSkinCommunityApi,
    S: AccountSessionStorage,
{
    fn user_id(&self) -> Result<Option<String>, AccountError> {
        BackendCandidateSkinCommunityService::user_id(self)
    }
    fn session_generation(&self) -> Result<Option<u64>, AccountError> {
        BackendCandidateSkinCommunityService::session_generation(self)
    }
    fn with_session_generation<T, F>(
        &self,
        generation: u64,
        user_id: &str,
        operation: F,
    ) -> Result<T, AccountError>
    where
        F: FnOnce() -> Result<T, AccountError>,
    {
        BackendCandidateSkinCommunityService::with_session_generation(
            self, generation, user_id, operation,
        )
    }
    fn sync_list(&self) -> Result<Vec<CandidateSkinSyncEntry>, AccountError> {
        BackendCandidateSkinCommunityService::sync_list(self)
    }
    fn detail(&self, id: Uuid) -> Result<CandidateSkinItem, AccountError> {
        BackendCandidateSkinCommunityService::detail(self, id)
    }
    fn publish(
        &self,
        request: &CandidateSkinPublishRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        BackendCandidateSkinCommunityService::publish(self, request)
    }
    fn replace(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        BackendCandidateSkinCommunityService::replace(self, id, request)
    }
    fn set_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
    ) -> Result<CandidateSkinItem, AccountError> {
        BackendCandidateSkinCommunityService::set_visibility(self, id, visibility)
    }
    fn set_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
    ) -> Result<CandidateSkinItem, AccountError> {
        BackendCandidateSkinCommunityService::set_category(self, id, category)
    }
    fn download(&self, id: Uuid) -> Result<CandidateSkinPackage, AccountError> {
        BackendCandidateSkinCommunityService::download(self, id)
    }
    fn unpublish(&self, id: Uuid) -> Result<(), AccountError> {
        BackendCandidateSkinCommunityService::unpublish(self, id)
    }
}

fn ensure_session<R: CandidateSkinSyncRemote>(
    remote: &R,
    user_id: &str,
    generation: Option<u64>,
) -> Result<(), AccountError> {
    let Some(expected_generation) = generation else {
        return Ok(());
    };
    let current_user = remote.user_id()?.ok_or(AccountError::Cancelled)?;
    let current_generation = remote
        .session_generation()?
        .ok_or(AccountError::Cancelled)?;
    if current_user != user_id || current_generation != expected_generation {
        return Err(AccountError::Cancelled);
    }
    Ok(())
}

fn with_session_effect<R, T, F>(
    remote: &R,
    user_id: &str,
    generation: Option<u64>,
    operation: F,
) -> Result<T, AccountError>
where
    R: CandidateSkinSyncRemote,
    F: FnOnce() -> Result<T, AccountError>,
{
    let Some(generation) = generation else {
        return operation();
    };
    remote.with_session_generation(generation, user_id, operation)
}

/// A package the run left as it was, and why: a `candidate_skin_*` rule code for a package the sharing rules refuse, or an `account_*` code for a request that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateSkinSyncSkip {
    pub package_id: String,
    pub code: &'static str,
}

/// What one run did, each list by package id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CandidateSkinSyncReport {
    pub uploaded: Vec<String>,
    pub downloaded: Vec<String>,
    pub deleted_local: Vec<String>,
    pub deleted_cloud: Vec<String>,
    pub skipped: Vec<CandidateSkinSyncSkip>,
    /// Set when the server refused further uploads for now (`account_rate_limited`) or the library is full (`candidate_skin_library_limit`). Uploads stop for the rest of the run; downloads and deletions go on.
    pub stopped: Option<&'static str>,
}

impl CandidateSkinSyncReport {
    /// Whether the run changed the skin root, so the host has to rescan it.
    pub fn changed_local(&self) -> bool {
        !self.downloaded.is_empty() || !self.deleted_local.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SyncedPackage {
    cloud_id: Uuid,
    /// The row's `request_sha256` when the package was last in step.
    cloud_digest: String,
    /// [`content_digest`] of the local folder at that moment.
    local_digest: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SyncState {
    /// The account the packages were synced with. Another account's state says nothing about this one's library.
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    packages: BTreeMap<String, SyncedPackage>,
    /// The publication each package was last installed from out of the gallery.
    #[serde(default)]
    installed: BTreeMap<String, Uuid>,
}

/// A state file that cannot be read is treated as empty: without state a run never deletes anything, it only uploads, downloads or compares.
fn load_state(path: &Path) -> SyncState {
    open_state_file(path)
        .and_then(|file| crate::bounded_io::read_bounded(file, MAX_STATE_BYTES).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn open_state_file(path: &Path) -> Option<File> {
    // 同步状态参与本地删除决策，不能跟随外部符号链接读取。
    crate::storage::reject_symlink(path).ok()?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path).ok()
}

/// Write the state by rename, so a reader never sees half of it.
fn save_state(path: &Path, state: &SyncState) -> Result<(), &'static str> {
    crate::storage::reject_symlink(path).map_err(|_| STORAGE)?;
    let directory = path.parent().ok_or(STORAGE)?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|_| STORAGE)?;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| STORAGE)?;
    file.write_all(&bytes).map_err(|_| STORAGE)?;
    file.as_file().sync_all().map_err(|_| STORAGE)?;
    file.persist(path).map_err(|_| STORAGE)?;
    Ok(())
}

/// The digest of a package's content alone, the manifest and the images, independent of the listing name and description.
fn content_digest(
    manifest: &str,
    files: &BTreeMap<String, String>,
) -> Result<String, &'static str> {
    request_digest("", "", manifest, files)
}

/// The folders in `root` the catalog would list, whether or not their manifest is valid. A package missing from this set is deleted from the library when it was synced before, so a root or entry that cannot be read fails the run instead of reading as empty; only a root that does not exist yet is empty.
fn local_packages(root: &Path) -> std::io::Result<BTreeSet<String>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => return Err(error),
    };
    let mut packages = BTreeSet::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Ok(name) = entry.file_name().into_string() {
            if catalog::is_external_id(&name) {
                packages.insert(name);
            }
        }
    }
    Ok(packages)
}

/// Record that `package_id` was just installed from the gallery publication `publication`. A later run leaves the package alone unless that publication is one of the user's own, and compares it afresh if it is, since the install replaced whatever was synced before.
pub fn record_install(
    state_path: &Path,
    package_id: &str,
    publication: Uuid,
) -> Result<(), &'static str> {
    let _run = lock_runs();
    let mut state = load_state(state_path);
    state.packages.remove(package_id);
    state.installed.insert(package_id.to_owned(), publication);
    save_state(state_path, &state)
}

/// Take the publication `publication` out of the library and forget the row it was synced with. The local folder is then uploaded again as a new private package by the next run rather than deleted to match. Both happen under the run lock: a run that listed the library between the two would find the row gone while its state still named it, and delete the local folder.
pub fn unpublish(
    state_path: &Path,
    remote: &impl CandidateSkinSyncRemote,
    publication: Uuid,
) -> Result<(), AccountError> {
    let _run = lock_runs();
    match remote.unpublish(publication) {
        Ok(()) | Err(AccountError::NotFound) => {}
        Err(error) => return Err(error),
    }
    let mut state = load_state(state_path);
    state
        .packages
        .retain(|_, synced| synced.cloud_id != publication);
    save_state(state_path, &state).map_err(|_| AccountError::Storage)?;
    Ok(())
}

/// The library row each package was last synced with, by package id.
pub fn synced_packages(state_path: &Path) -> BTreeMap<String, Uuid> {
    let _run = lock_runs();
    load_state(state_path)
        .packages
        .into_iter()
        .map(|(id, synced)| (id, synced.cloud_id))
        .collect()
}

/// Why [`publish`] failed: a package rule, or the account request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateSkinPublishError {
    Package(&'static str),
    Account(AccountError),
}

/// Publish the installed package `package_id` to the library under `name` and `description` with `visibility` and, when given, `category`. A package sync already keeps in the library is updated in place, content, visibility and category, so publishing never leaves a second row of the same package behind; any other package is created as the publication `publication`. The state is updated either way, so the next run sees the package as in step.
#[allow(clippy::too_many_arguments)]
pub fn publish(
    root: &Path,
    state_path: &Path,
    remote: &impl CandidateSkinSyncRemote,
    package_id: &str,
    publication: Uuid,
    name: String,
    description: String,
    visibility: CandidateSkinVisibility,
    category: Option<CandidateSkinCategory>,
) -> Result<CandidateSkinItem, CandidateSkinPublishError> {
    let _run = lock_runs();
    let packed =
        pack_as(root, package_id, visibility).map_err(CandidateSkinPublishError::Package)?;
    let local_digest = content_digest(&packed.manifest, &packed.files)
        .map_err(CandidateSkinPublishError::Package)?;
    let cloud_digest = request_digest(&name, &description, &packed.manifest, &packed.files)
        .map_err(CandidateSkinPublishError::Package)?;
    let user = remote
        .user_id()
        .map_err(CandidateSkinPublishError::Account)?
        .ok_or(CandidateSkinPublishError::Account(
            AccountError::Unauthorized,
        ))?;
    let generation = remote
        .session_generation()
        .map_err(CandidateSkinPublishError::Account)?;
    ensure_session(remote, &user, generation).map_err(CandidateSkinPublishError::Account)?;
    let mut state = load_state(state_path);
    if state.user_id != user {
        state.user_id = user.clone();
        state.packages.clear();
    }
    // A package sync has not recorded yet may still have a row, from another device or a run that has not finished; updating that row keeps the library at one row per package.
    let existing = match state.packages.get(package_id) {
        Some(synced) => Some(synced.cloud_id),
        None => {
            ensure_session(remote, &user, generation)
                .map_err(CandidateSkinPublishError::Account)?;
            match remote.sync_list() {
                Ok(rows) => rows
                    .into_iter()
                    .find(|row| row.package_id == package_id)
                    .map(|row| row.id),
                // A server that predates the library has no listing, and every package there is a new publication.
                Err(AccountError::NotFound) => None,
                Err(error) => return Err(CandidateSkinPublishError::Account(error)),
            }
        }
    };
    let item = match existing {
        Some(id) => match update_in_place(
            remote,
            id,
            &name,
            &description,
            &packed,
            visibility,
            category,
            &user,
            generation,
        ) {
            Err(AccountError::NotFound) => None,
            result => Some(result.map_err(CandidateSkinPublishError::Account)?),
        },
        None => None,
    };
    let item = match item {
        Some(item) => item,
        None => {
            ensure_session(remote, &user, generation)
                .map_err(CandidateSkinPublishError::Account)?;
            let item = remote
                .publish(&CandidateSkinPublishRequest::new(
                    publication,
                    name,
                    description,
                    packed,
                    visibility,
                    category,
                ))
                .map_err(CandidateSkinPublishError::Account)?;
            ensure_session(remote, &user, generation)
                .map_err(CandidateSkinPublishError::Account)?;
            item
        }
    };
    with_session_effect(remote, &user, generation, || {
        state.installed.remove(package_id);
        state.packages.insert(
            package_id.to_owned(),
            SyncedPackage {
                cloud_id: item.id,
                cloud_digest,
                local_digest,
            },
        );
        save_state(state_path, &state).map_err(|_| AccountError::Storage)?;
        Ok(())
    })
    .map_err(CandidateSkinPublishError::Account)?;
    Ok(item)
}

#[allow(clippy::too_many_arguments)]
fn update_in_place(
    remote: &impl CandidateSkinSyncRemote,
    id: Uuid,
    name: &str,
    description: &str,
    packed: &PackedSkin,
    visibility: CandidateSkinVisibility,
    category: Option<CandidateSkinCategory>,
    user_id: &str,
    generation: Option<u64>,
) -> Result<CandidateSkinItem, AccountError> {
    // A private package may carry no license, so the content goes up before the row is made public, and the server's license check sees the new content.
    ensure_session(remote, user_id, generation)?;
    let item = remote.replace(
        id,
        &CandidateSkinReplaceRequest {
            name: name.to_owned(),
            description: description.to_owned(),
            manifest: packed.manifest.clone(),
            files: packed.files.clone(),
        },
    )?;
    ensure_session(remote, user_id, generation)?;
    let item = if item.visibility == visibility {
        item
    } else {
        ensure_session(remote, user_id, generation)?;
        remote.set_visibility(id, visibility)?
    };
    // 分类不在替换请求里，同步按私有方式存入库中的行由这次发布补上分类。
    match category {
        Some(category) if item.category != Some(category) => {
            ensure_session(remote, user_id, generation)?;
            let item = remote.set_category(id, category)?;
            ensure_session(remote, user_id, generation)?;
            Ok(item)
        }
        _ => Ok(item),
    }
}

/// Bring the skin root `root` and the signed-in user's library in step, remembering the outcome in `state_path`.
///
/// Signed out, or signed out or switched during the run, is `Unauthorized` or `Cancelled`; a failed library listing is its own error. Every other failure only skips the package it concerns.
pub fn sync_candidate_skins(
    root: &Path,
    state_path: &Path,
    remote: &impl CandidateSkinSyncRemote,
) -> Result<CandidateSkinSyncReport, AccountError> {
    let _run = lock_runs();
    if crate::storage::reject_symlink(root).is_err() {
        return Err(AccountError::Storage);
    }
    let user = remote.user_id()?.ok_or(AccountError::Unauthorized)?;
    let generation = remote.session_generation()?;
    let mut state = load_state(state_path);
    if state.user_id != user {
        state.user_id = user.clone();
        state.packages.clear();
    }
    let rows = remote.sync_list()?;
    let owned: BTreeSet<Uuid> = rows.iter().map(|row| row.id).collect();
    // Two rows of one package (a publication made before sync knew about it) resolve to the newest. The server lists newest first; `updated_at` is not compared as text, since RFC 3339 with a variable fraction does not sort that way.
    let mut newest: BTreeMap<String, CandidateSkinSyncEntry> = BTreeMap::new();
    for row in rows {
        newest.entry(row.package_id.clone()).or_insert(row);
    }
    let local = local_packages(root).map_err(|_| AccountError::Storage)?;
    // A root that is gone while packages were synced from it is more likely moved or unmounted than emptied on purpose; reading it as empty would delete every one of them from the library.
    if !root.is_dir() && !state.packages.is_empty() {
        return Err(AccountError::Storage);
    }
    let ids: BTreeSet<String> = local
        .iter()
        .cloned()
        .chain(newest.keys().cloned())
        .chain(state.packages.keys().cloned())
        .chain(state.installed.keys().cloned())
        .collect();
    let mut run = Run {
        root,
        state_path,
        remote,
        user_id: user,
        generation,
        state,
        owned,
        report: CandidateSkinSyncReport::default(),
    };
    run.ensure_session()?;
    for id in &ids {
        run.package(id, local.contains(id), newest.get(id))?;
    }
    run.with_local_effect(|run| {
        save_state(run.state_path, &run.state).map_err(|_| AccountError::Storage)?;
        Ok(())
    })?;
    Ok(run.report)
}

struct Run<'a, R: CandidateSkinSyncRemote> {
    root: &'a Path,
    state_path: &'a Path,
    remote: &'a R,
    user_id: String,
    generation: Option<u64>,
    state: SyncState,
    /// Every row the user owns, so a gallery install can be told apart from someone else's.
    owned: BTreeSet<Uuid>,
    report: CandidateSkinSyncReport,
}

/// The local side of one package: its packed content and that content's digest.
struct Local {
    packed: PackedSkin,
    digest: String,
}

/// Which kind of request failed, since a refusal means something different for each.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Request {
    Create,
    Replace,
    Other,
}

impl<R: CandidateSkinSyncRemote> Run<'_, R> {
    fn ensure_session(&self) -> Result<(), AccountError> {
        ensure_session(self.remote, &self.user_id, self.generation)
    }

    fn with_local_effect<T, F>(&mut self, operation: F) -> Result<T, AccountError>
    where
        F: FnOnce(&mut Self) -> Result<T, AccountError>,
    {
        let remote = self.remote;
        let user_id = self.user_id.clone();
        with_session_effect(remote, &user_id, self.generation, || operation(self))
    }

    fn skip(&mut self, package_id: &str, code: &'static str) {
        self.report.skipped.push(CandidateSkinSyncSkip {
            package_id: package_id.to_owned(),
            code,
        });
    }

    fn uploads_stopped(&self) -> bool {
        self.report.stopped.is_some()
    }

    /// Record a failed request. A lost or switched session ends the run; a refused upload stops uploading; anything else skips the package.
    fn fail(
        &mut self,
        package_id: &str,
        error: AccountError,
        request: Request,
    ) -> Result<(), AccountError> {
        match (error, request) {
            (error @ (AccountError::Unauthorized | AccountError::Cancelled), _) => Err(error),
            (AccountError::RateLimited, Request::Create | Request::Replace) => {
                self.report.stopped = Some(RATE_LIMITED);
                Ok(())
            }
            (AccountError::Conflict, Request::Create) => {
                self.report.stopped = Some(LIBRARY_LIMIT);
                Ok(())
            }
            (error, _) => {
                self.skip(package_id, error.code());
                Ok(())
            }
        }
    }

    fn record(
        &mut self,
        package_id: &str,
        cloud_id: Uuid,
        cloud_digest: String,
        local_digest: String,
    ) -> Result<(), AccountError> {
        self.with_local_effect(|run| {
            run.record_unchecked(package_id, cloud_id, cloud_digest, local_digest);
            Ok(())
        })
    }

    fn record_unchecked(
        &mut self,
        package_id: &str,
        cloud_id: Uuid,
        cloud_digest: String,
        local_digest: String,
    ) {
        self.state.packages.insert(
            package_id.to_owned(),
            SyncedPackage {
                cloud_id,
                cloud_digest,
                local_digest,
            },
        );
        let _ = save_state(self.state_path, &self.state);
    }

    fn package(
        &mut self,
        id: &str,
        present: bool,
        cloud: Option<&CandidateSkinSyncEntry>,
    ) -> Result<(), AccountError> {
        self.ensure_session()?;
        if !present {
            self.state.installed.remove(id);
        }
        // Remembered state describes only the row it was made for.
        let saved = self
            .state
            .packages
            .get(id)
            .filter(|saved| cloud.is_none_or(|row| row.id == saved.cloud_id))
            .cloned();
        if present && saved.is_none() {
            let foreign = self
                .state
                .installed
                .get(id)
                .is_some_and(|publication| !self.owned.contains(publication));
            if foreign {
                return Ok(());
            }
        }
        let local = if present {
            let packed = match pack_as(self.root, id, CandidateSkinVisibility::Private) {
                Ok(packed) => packed,
                Err(code) => {
                    self.skip(id, code);
                    return Ok(());
                }
            };
            match content_digest(&packed.manifest, &packed.files) {
                Ok(digest) => Some(Local { packed, digest }),
                Err(code) => {
                    self.skip(id, code);
                    return Ok(());
                }
            }
        } else {
            None
        };
        match (local, cloud, saved) {
            (Some(local), None, None) => self.create(id, local),
            (Some(local), None, Some(saved)) => {
                if local.digest == saved.local_digest {
                    self.delete_local(id)
                } else {
                    self.create(id, local)
                }
            }
            (Some(local), Some(row), None) => self.compare(id, local, row),
            (Some(local), Some(row), Some(saved)) => {
                if local.digest != saved.local_digest {
                    self.replace(id, local, row)
                } else if row.request_sha256 != saved.cloud_digest {
                    self.download(id, row)
                } else {
                    Ok(())
                }
            }
            (None, Some(row), Some(saved)) => {
                if row.request_sha256 != saved.cloud_digest {
                    self.download(id, row)
                } else if row.visibility == CandidateSkinVisibility::Public {
                    self.skip(id, PUBLIC_KEPT);
                    Ok(())
                } else {
                    self.delete_cloud(id, row)
                }
            }
            (None, Some(row), None) => self.download(id, row),
            (None, None, _) => {
                self.state.packages.remove(id);
                Ok(())
            }
        }
    }

    /// Upload a package the library does not hold, as a new private package listed under its manifest name and description.
    fn create(&mut self, id: &str, local: Local) -> Result<(), AccountError> {
        if self.uploads_stopped() {
            return Ok(());
        }
        let Local { packed, digest } = local;
        let name = packed.suggested_name.clone();
        let description = packed.suggested_description.clone();
        let request = CandidateSkinPublishRequest::new(
            Uuid::new_v4(),
            name,
            description,
            packed,
            CandidateSkinVisibility::Private,
            None,
        );
        let cloud_digest = match request_digest(
            &request.name,
            &request.description,
            &request.manifest,
            &request.files,
        ) {
            Ok(value) => value,
            Err(code) => {
                self.skip(id, code);
                return Ok(());
            }
        };
        match self.remote.publish(&request) {
            Ok(item) => {
                self.record(id, item.id, cloud_digest, digest)?;
                self.report.uploaded.push(id.to_owned());
                Ok(())
            }
            Err(error) => self.fail(id, error, Request::Create),
        }
    }

    /// Both sides hold the package and nothing is remembered about it: the row's own name and description with the local content give the digest the row would have if they matched.
    fn compare(
        &mut self,
        id: &str,
        local: Local,
        row: &CandidateSkinSyncEntry,
    ) -> Result<(), AccountError> {
        let item = match self.remote.detail(row.id) {
            Ok(item) => item,
            Err(error) => return self.fail(id, error, Request::Other),
        };
        match request_digest(
            &item.name,
            &item.description,
            &local.packed.manifest,
            &local.packed.files,
        ) {
            Ok(digest) if digest == row.request_sha256 => {
                self.record(id, row.id, digest, local.digest)?;
                Ok(())
            }
            Ok(_) => self.put(id, local, row, item),
            Err(code) => {
                self.skip(id, code);
                Ok(())
            }
        }
    }

    fn replace(
        &mut self,
        id: &str,
        local: Local,
        row: &CandidateSkinSyncEntry,
    ) -> Result<(), AccountError> {
        if self.uploads_stopped() {
            return Ok(());
        }
        match self.remote.detail(row.id) {
            Ok(item) => self.put(id, local, row, item),
            Err(error) => self.fail(id, error, Request::Other),
        }
    }

    /// Replace the row's content with the local package, keeping the listing name and description the row already has.
    fn put(
        &mut self,
        id: &str,
        local: Local,
        row: &CandidateSkinSyncEntry,
        item: CandidateSkinItem,
    ) -> Result<(), AccountError> {
        if self.uploads_stopped() {
            return Ok(());
        }
        // A public row must keep an asset license; the private pack above did not check for one.
        if row.visibility == CandidateSkinVisibility::Public {
            if let Err(code) = pack_as(self.root, id, CandidateSkinVisibility::Public) {
                self.skip(id, code);
                return Ok(());
            }
        }
        let request = CandidateSkinReplaceRequest {
            name: item.name,
            description: item.description,
            manifest: local.packed.manifest,
            files: local.packed.files,
        };
        let cloud_digest = match request_digest(
            &request.name,
            &request.description,
            &request.manifest,
            &request.files,
        ) {
            Ok(value) => value,
            Err(code) => {
                self.skip(id, code);
                return Ok(());
            }
        };
        match self.remote.replace(row.id, &request) {
            Ok(_) => {
                self.record(id, row.id, cloud_digest, local.digest)?;
                self.report.uploaded.push(id.to_owned());
                Ok(())
            }
            Err(error) => self.fail(id, error, Request::Replace),
        }
    }

    fn download(&mut self, id: &str, row: &CandidateSkinSyncEntry) -> Result<(), AccountError> {
        let package = match self.remote.download(row.id) {
            Ok(package) => package,
            Err(error) => return self.fail(id, error, Request::Other),
        };
        if package.package_id != id {
            self.skip(id, PACKAGE);
            return Ok(());
        }
        // The server re-encodes images, so the local digest is taken from the bytes actually installed.
        let installed = self.with_local_effect(|run| {
            match candidate_community::install(run.root, &package, true)
                .and_then(|_| content_digest(&package.manifest, &package.files))
            {
                Ok(local_digest) => {
                    run.state.installed.remove(id);
                    run.record_unchecked(id, row.id, row.request_sha256.clone(), local_digest);
                    run.report.downloaded.push(id.to_owned());
                    Ok(())
                }
                Err(code) => {
                    run.skip(id, code);
                    Ok(())
                }
            }
        });
        installed?;
        Ok(())
    }

    /// Remove a local package whose library row is gone. It is first renamed out of the catalog's sight, so a failure part-way never leaves a half-deleted skin listed.
    fn delete_local(&mut self, id: &str) -> Result<(), AccountError> {
        self.with_local_effect(|run| {
            let removed = {
                let _writes = super::folder_import::lock_skin_root();
                let aside = run.root.join(format!(".removed-{id}"));
                let _ = remove_entry(&aside);
                if fs::rename(run.root.join(id), &aside).is_err() {
                    false
                } else {
                    match remove_entry(&aside) {
                        Ok(()) => true,
                        Err(_) => {
                            let _ = fs::rename(&aside, run.root.join(id));
                            false
                        }
                    }
                }
            };
            if removed {
                run.state.packages.remove(id);
                let _ = save_state(run.state_path, &run.state);
                run.report.deleted_local.push(id.to_owned());
            } else {
                run.skip(id, STORAGE);
            }
            Ok(())
        })
    }

    fn delete_cloud(&mut self, id: &str, row: &CandidateSkinSyncEntry) -> Result<(), AccountError> {
        match self.remote.unpublish(row.id) {
            Ok(()) | Err(AccountError::NotFound) => self.with_local_effect(|run| {
                run.state.packages.remove(id);
                let _ = save_state(run.state_path, &run.state);
                run.report.deleted_cloud.push(id.to_owned());
                Ok(())
            }),
            Err(error) => self.fail(id, error, Request::Other),
        }
    }
}

fn remove_entry(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests;
