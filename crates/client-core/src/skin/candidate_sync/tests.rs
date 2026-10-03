//! Unit tests for the parent module: every sync rule against an in-memory library and a temporary skin root.

use super::*;
use crate::skin::candidate_community::CandidateSkinLicense;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;

fn manifest(id: &str, name: &str, head: &str, tail: &str) -> String {
    format!("schema_version = 1\nid = '{id}'\nname = '{name}'\ndescription = '{name}的说明'\nversion = '1.0'\nbase = 'paper'\npreview = 'preview.png'\n{head}[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n{tail}")
}

/// Write the package `id` into `root`, its preview varied by `seed` so two versions differ in an image as well as the manifest.
fn write_skin(root: &Path, id: &str, name: &str, seed: u8) {
    write_skin_with(root, id, name, seed, "", "");
}

fn write_skin_with(root: &Path, id: &str, name: &str, seed: u8, head: &str, tail: &str) {
    let skin = root.join(id);
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("skin.toml"), manifest(id, name, head, tail)).unwrap();
    // 安装和发布都会完整解码图片，所以这里是一张真的 PNG，颜色随 `seed` 变化。
    let mut preview = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(2, 2, image::Rgb([seed, 7, 9]))
        .write_to(&mut preview, image::ImageFormat::Png)
        .unwrap();
    fs::write(skin.join("preview.png"), preview.into_inner()).unwrap();
}

struct Row {
    package_id: String,
    name: String,
    description: String,
    manifest: String,
    files: BTreeMap<String, String>,
    visibility: CandidateSkinVisibility,
    updated_at: String,
    digest: String,
    category: Option<CandidateSkinCategory>,
}

/// The user's library as the server keeps it. Downloads return every image with one byte appended, standing in for the server's re-encoding, so a downloaded package never has the uploaded bytes.
struct Library {
    user: RefCell<Option<String>>,
    generation: Cell<u64>,
    relogin_after_list: Cell<bool>,
    relogin_on_publish: Cell<bool>,
    rows: RefCell<BTreeMap<Uuid, Row>>,
    clock: Cell<u32>,
    calls: RefCell<Vec<String>>,
    refuse_publish: RefCell<Option<AccountError>>,
    refuse_replace: RefCell<Option<AccountError>>,
    refuse_download: RefCell<Option<AccountError>>,
}

impl Library {
    fn new() -> Self {
        Self {
            user: RefCell::new(Some("user-1".into())),
            generation: Cell::new(1),
            relogin_after_list: Cell::new(false),
            relogin_on_publish: Cell::new(false),
            rows: RefCell::new(BTreeMap::new()),
            clock: Cell::new(0),
            calls: RefCell::new(Vec::new()),
            refuse_publish: RefCell::new(None),
            refuse_replace: RefCell::new(None),
            refuse_download: RefCell::new(None),
        }
    }

    fn tick(&self) -> String {
        self.clock.set(self.clock.get() + 1);
        format!("2026-10-01T00:00:{:02}Z", self.clock.get())
    }

    /// Put a row straight into the library, as another device would have uploaded it, and return its id.
    fn insert(&self, root: &Path, package_id: &str, name: &str) -> Uuid {
        let packed = pack_as(root, package_id, CandidateSkinVisibility::Private).unwrap();
        let id = Uuid::new_v4();
        let digest = request_digest(name, "", &packed.manifest, &packed.files).unwrap();
        let updated_at = self.tick();
        self.rows.borrow_mut().insert(
            id,
            Row {
                package_id: package_id.into(),
                name: name.into(),
                description: String::new(),
                manifest: packed.manifest,
                files: packed.files,
                visibility: CandidateSkinVisibility::Private,
                updated_at,
                digest,
                category: None,
            },
        );
        id
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow_mut().drain(..).collect()
    }

    fn log(&self, call: &str) {
        self.calls.borrow_mut().push(call.to_owned());
    }

    fn item(&self, id: Uuid) -> Result<CandidateSkinItem, AccountError> {
        let rows = self.rows.borrow();
        let row = rows.get(&id).ok_or(AccountError::NotFound)?;
        Ok(CandidateSkinItem {
            id,
            package_id: row.package_id.clone(),
            name: row.name.clone(),
            description: row.description.clone(),
            author: "Fixture".into(),
            version: "1.0".into(),
            license: CandidateSkinLicense {
                code: String::new(),
                assets: String::new(),
                source: String::new(),
            },
            size: 0,
            file_count: 1,
            downloads: 0,
            rating_count: 0,
            rating_average: 0.0,
            owned: true,
            my_rating: 0,
            created_at: row.updated_at.clone(),
            visibility: row.visibility,
            updated_at: row.updated_at.clone(),
            request_sha256: row.digest.clone(),
            moderation: None,
            category: row.category,
        })
    }

    fn row_names(&self) -> Vec<(String, String)> {
        self.rows
            .borrow()
            .values()
            .map(|row| (row.package_id.clone(), row.name.clone()))
            .collect()
    }
}

impl CandidateSkinSyncRemote for Library {
    fn user_id(&self) -> Result<Option<String>, AccountError> {
        Ok(self.user.borrow().clone())
    }
    fn session_generation(&self) -> Result<Option<u64>, AccountError> {
        Ok(self.user.borrow().as_ref().map(|_| self.generation.get()))
    }
    fn sync_list(&self) -> Result<Vec<CandidateSkinSyncEntry>, AccountError> {
        self.log("list");
        if self.relogin_after_list.replace(false) {
            self.generation.set(self.generation.get() + 1);
        }
        let mut rows: Vec<_> = self
            .rows
            .borrow()
            .iter()
            .map(|(id, row)| CandidateSkinSyncEntry {
                id: *id,
                package_id: row.package_id.clone(),
                request_sha256: row.digest.clone(),
                visibility: row.visibility,
                updated_at: row.updated_at.clone(),
            })
            .collect();
        // The server lists the newest row first.
        rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(rows)
    }
    fn detail(&self, id: Uuid) -> Result<CandidateSkinItem, AccountError> {
        self.log("detail");
        self.item(id)
    }
    fn publish(
        &self,
        request: &CandidateSkinPublishRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.log(&format!("publish {}", request.name));
        if let Some(error) = self.refuse_publish.borrow().clone() {
            return Err(error);
        }
        let package_id = toml::from_str::<toml::Value>(&request.manifest).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let updated_at = self.tick();
        self.rows.borrow_mut().insert(
            request.id,
            Row {
                package_id,
                name: request.name.clone(),
                description: request.description.clone(),
                manifest: request.manifest.clone(),
                files: request.files.clone(),
                visibility: request.visibility,
                updated_at,
                digest: request_digest(
                    &request.name,
                    &request.description,
                    &request.manifest,
                    &request.files,
                )
                .unwrap(),
                category: request.category,
            },
        );
        if self.relogin_on_publish.replace(false) {
            self.generation.set(self.generation.get() + 1);
        }
        self.item(request.id)
    }
    fn replace(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.log(&format!("replace {}", request.name));
        if let Some(error) = self.refuse_replace.borrow().clone() {
            return Err(error);
        }
        let updated_at = self.tick();
        {
            let mut rows = self.rows.borrow_mut();
            let row = rows.get_mut(&id).ok_or(AccountError::NotFound)?;
            row.name = request.name.clone();
            row.description = request.description.clone();
            row.manifest = request.manifest.clone();
            row.files = request.files.clone();
            row.updated_at = updated_at;
            row.digest = request_digest(
                &request.name,
                &request.description,
                &request.manifest,
                &request.files,
            )
            .unwrap();
        }
        self.item(id)
    }
    fn set_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.log("visibility");
        self.rows
            .borrow_mut()
            .get_mut(&id)
            .ok_or(AccountError::NotFound)?
            .visibility = visibility;
        self.item(id)
    }
    fn set_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.log(&format!("category {}", category.as_str()));
        self.rows
            .borrow_mut()
            .get_mut(&id)
            .ok_or(AccountError::NotFound)?
            .category = Some(category);
        self.item(id)
    }
    fn download(&self, id: Uuid) -> Result<CandidateSkinPackage, AccountError> {
        self.log("download");
        if let Some(error) = self.refuse_download.borrow().clone() {
            return Err(error);
        }
        let rows = self.rows.borrow();
        let row = rows.get(&id).ok_or(AccountError::NotFound)?;
        let files = row
            .files
            .iter()
            .map(|(path, data)| {
                let mut bytes = BASE64.decode(data).unwrap();
                bytes.push(0);
                (path.clone(), BASE64.encode(bytes))
            })
            .collect();
        Ok(CandidateSkinPackage {
            id,
            package_id: row.package_id.clone(),
            manifest: row.manifest.clone(),
            files,
        })
    }
    fn unpublish(&self, id: Uuid) -> Result<(), AccountError> {
        self.log("unpublish");
        self.rows
            .borrow_mut()
            .remove(&id)
            .map(|_| ())
            .ok_or(AccountError::NotFound)
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("skins");
        fs::create_dir_all(&root).unwrap();
        let state = directory.path().join(STATE_FILE);
        Self {
            _directory: directory,
            root,
            state,
            library: Library::new(),
        }
    }

    fn sync(&self) -> CandidateSkinSyncReport {
        sync_candidate_skins(&self.root, &self.state, &self.library).unwrap()
    }

    fn installed(&self, id: &str) -> bool {
        self.root.join(id).join("skin.toml").is_file()
    }

    fn manifest_of(&self, id: &str) -> String {
        fs::read_to_string(self.root.join(id).join("skin.toml")).unwrap()
    }
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn a_local_package_is_uploaded_private_under_its_manifest_name_once() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    let report = fixture.sync();
    assert_eq!(report.uploaded, ids(&["sakura"]));
    assert_eq!(fixture.library.calls(), ["list", "publish 樱花"]);
    let rows = fixture.library.rows.borrow();
    let row = rows.values().next().unwrap();
    assert_eq!(row.visibility, CandidateSkinVisibility::Private);
    assert_eq!(row.description, "樱花的说明");
    drop(rows);

    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list"]);
}

#[test]
fn matching_packages_on_both_sides_are_recorded_without_an_upload() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.library.insert(&fixture.root, "sakura", "我的樱花");
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list", "detail"]);
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list"]);
}

#[test]
fn differing_packages_on_both_sides_upload_the_local_one_and_keep_the_listing_name() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.library.insert(&fixture.root, "sakura", "我的樱花");
    write_skin(&fixture.root, "sakura", "樱花", 2);
    let report = fixture.sync();
    assert_eq!(report.uploaded, ids(&["sakura"]));
    assert_eq!(
        fixture.library.calls(),
        ["list", "detail", "replace 我的樱花"]
    );
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
}

#[test]
fn a_local_edit_replaces_the_library_copy() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fixture.library.calls();
    write_skin(&fixture.root, "sakura", "樱花", 2);
    assert_eq!(fixture.sync().uploaded, ids(&["sakura"]));
    assert_eq!(fixture.library.calls(), ["list", "detail", "replace 樱花"]);
    assert_eq!(fixture.library.rows.borrow().len(), 1);
}

#[test]
fn a_change_from_another_device_is_downloaded_and_stays_in_step() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    // Another device edits the package and uploads it.
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "sakura", "新樱花", 2);
    let packed = pack_as(other.path(), "sakura", CandidateSkinVisibility::Private).unwrap();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    fixture
        .library
        .replace(
            id,
            &CandidateSkinReplaceRequest {
                name: "樱花".into(),
                description: "樱花的说明".into(),
                manifest: packed.manifest,
                files: packed.files,
            },
        )
        .unwrap();
    fixture.library.calls();

    let report = fixture.sync();
    assert_eq!(report.downloaded, ids(&["sakura"]));
    assert!(report.changed_local());
    assert!(fixture.manifest_of("sakura").contains("新樱花"));
    // The re-encoded images now on disk are what the next run compares against.
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list", "download", "list"]);
}

#[test]
fn deleting_a_synced_package_locally_removes_it_from_the_library() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fs::remove_dir_all(fixture.root.join("sakura")).unwrap();
    let report = fixture.sync();
    assert_eq!(report.deleted_cloud, ids(&["sakura"]));
    assert!(fixture.library.rows.borrow().is_empty());
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
}

#[test]
fn a_local_deletion_does_not_discard_a_newer_library_copy() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    fixture
        .library
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .digest = "f".repeat(64);
    fs::remove_dir_all(fixture.root.join("sakura")).unwrap();
    assert_eq!(fixture.sync().downloaded, ids(&["sakura"]));
    assert!(fixture.installed("sakura"));
}

#[test]
fn a_package_removed_from_the_library_is_removed_locally_when_unchanged() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fixture.library.rows.borrow_mut().clear();
    let report = fixture.sync();
    assert_eq!(report.deleted_local, ids(&["sakura"]));
    assert!(!fixture.root.join("sakura").exists());
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[test]
fn a_stale_removed_file_does_not_block_local_skin_deletion() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fixture.library.rows.borrow_mut().clear();
    fs::write(
        fixture.root.join(".removed-sakura"),
        b"stale cleanup marker",
    )
    .unwrap();

    let report = fixture.sync();

    assert_eq!(report.deleted_local, ids(&["sakura"]));
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[test]
fn a_package_removed_from_the_library_is_uploaded_again_when_edited() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fixture.library.rows.borrow_mut().clear();
    write_skin(&fixture.root, "sakura", "樱花", 2);
    assert_eq!(fixture.sync().uploaded, ids(&["sakura"]));
    assert!(fixture.installed("sakura"));
    assert_eq!(fixture.library.rows.borrow().len(), 1);
}

#[test]
fn a_library_only_package_is_installed() {
    let fixture = Fixture::new();
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "sakura", "樱花", 1);
    fixture.library.insert(other.path(), "sakura", "樱花");
    assert_eq!(fixture.sync().downloaded, ids(&["sakura"]));
    assert!(fixture.installed("sakura"));
}

#[test]
fn duplicate_rows_resolve_to_the_newest() {
    let fixture = Fixture::new();
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "sakura", "旧樱花", 1);
    fixture.library.insert(other.path(), "sakura", "旧樱花");
    write_skin(other.path(), "sakura", "新樱花", 2);
    fixture.library.insert(other.path(), "sakura", "新樱花");
    assert_eq!(fixture.sync().downloaded, ids(&["sakura"]));
    assert!(fixture.manifest_of("sakura").contains("新樱花"));
}

#[test]
fn a_package_the_sharing_rules_refuse_is_skipped_with_its_code() {
    let fixture = Fixture::new();
    write_skin_with(
        &fixture.root,
        "styled",
        "样式",
        1,
        "toolbar_stylesheet = 'toolbar.css'\n",
        "",
    );
    fs::write(fixture.root.join("styled").join("toolbar.css"), "a{}").unwrap();
    fs::create_dir_all(fixture.root.join("broken")).unwrap();
    let report = fixture.sync();
    assert!(report.uploaded.is_empty());
    let skipped: Vec<_> = report
        .skipped
        .iter()
        .map(|skip| (skip.package_id.as_str(), skip.code))
        .collect();
    assert_eq!(
        skipped,
        [
            ("broken", "candidate_skin_package"),
            ("styled", "candidate_skin_file_type")
        ]
    );
    assert_eq!(fixture.library.calls(), ["list"]);
}

#[test]
fn a_rate_limit_stops_uploads_but_not_downloads() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "alpha", "甲", 1);
    write_skin(&fixture.root, "beta", "乙", 2);
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "gamma", "丙", 3);
    fixture.library.insert(other.path(), "gamma", "丙");
    *fixture.library.refuse_publish.borrow_mut() = Some(AccountError::RateLimited);
    let report = fixture.sync();
    assert_eq!(report.stopped, Some("account_rate_limited"));
    assert!(report.uploaded.is_empty());
    assert_eq!(report.downloaded, ids(&["gamma"]));
    assert_eq!(fixture.library.calls(), ["list", "publish 甲", "download"]);

    *fixture.library.refuse_publish.borrow_mut() = None;
    assert_eq!(fixture.sync().uploaded, ids(&["alpha", "beta"]));
}

#[test]
fn a_full_library_stops_uploads() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "alpha", "甲", 1);
    *fixture.library.refuse_publish.borrow_mut() = Some(AccountError::Conflict);
    assert_eq!(fixture.sync().stopped, Some("candidate_skin_library_limit"));
}

#[test]
fn a_failed_download_skips_only_that_package() {
    let fixture = Fixture::new();
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "gamma", "丙", 3);
    fixture.library.insert(other.path(), "gamma", "丙");
    write_skin(&fixture.root, "alpha", "甲", 1);
    *fixture.library.refuse_download.borrow_mut() = Some(AccountError::Unavailable);
    let report = fixture.sync();
    assert_eq!(report.uploaded, ids(&["alpha"]));
    assert_eq!(
        report.skipped,
        [CandidateSkinSyncSkip {
            package_id: "gamma".into(),
            code: "account_unavailable"
        }]
    );
}

#[test]
fn someone_elses_package_installed_from_the_gallery_is_left_alone() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    record_install(&fixture.state, "sakura", Uuid::new_v4());
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list"]);
    // Once it is gone locally the record goes too, and a package of that name is the user's own again.
    fs::remove_dir_all(fixture.root.join("sakura")).unwrap();
    fixture.sync();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    assert_eq!(fixture.sync().uploaded, ids(&["sakura"]));
}

#[test]
fn the_users_own_package_installed_from_the_gallery_is_compared_afresh() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    record_install(&fixture.state, "sakura", id);
    fixture.library.calls();
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list", "detail"]);
}

#[test]
fn another_accounts_state_never_deletes_anything() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fixture.library.rows.borrow_mut().clear();
    *fixture.library.user.borrow_mut() = Some("user-2".into());
    let report = fixture.sync();
    assert!(report.deleted_local.is_empty());
    assert_eq!(report.uploaded, ids(&["sakura"]));
}

#[test]
fn a_signed_out_run_is_refused_before_any_request() {
    let fixture = Fixture::new();
    *fixture.library.user.borrow_mut() = None;
    assert_eq!(
        sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library),
        Err(AccountError::Unauthorized)
    );
    assert!(fixture.library.calls().is_empty());
}

#[test]
fn a_same_user_relogin_after_listing_cancels_the_sync() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.library.relogin_after_list.set(true);

    assert_eq!(
        sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library),
        Err(AccountError::Cancelled)
    );
    assert!(fixture.library.rows.borrow().is_empty());
    assert!(!fixture.state.exists());
}

#[test]
fn a_same_user_relogin_after_upload_does_not_save_stale_sync_state() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.library.relogin_on_publish.set(true);

    assert_eq!(
        sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library),
        Err(AccountError::Cancelled)
    );
    assert!(!fixture.state.exists());
}

#[test]
fn a_corrupt_state_file_reads_as_empty() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fs::write(&fixture.state, b"{not json").unwrap();
    fixture.library.rows.borrow_mut().clear();
    let report = fixture.sync();
    assert!(report.deleted_local.is_empty());
    assert_eq!(report.uploaded, ids(&["sakura"]));
}

#[test]
fn publishing_a_synced_package_updates_its_row_in_place() {
    let fixture = Fixture::new();
    write_skin_with(
        &fixture.root,
        "sakura",
        "樱花",
        1,
        "",
        "[license]\nassets = 'CC-BY-4.0'\n",
    );
    fixture.sync();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    fixture.library.calls();
    let item = publish(
        &fixture.root,
        &fixture.state,
        &fixture.library,
        "sakura",
        Uuid::new_v4(),
        "公开樱花".into(),
        "说明".into(),
        CandidateSkinVisibility::Public,
        Some(CandidateSkinCategory::Guofeng),
    )
    .unwrap();
    assert_eq!(item.id, id);
    assert_eq!(item.visibility, CandidateSkinVisibility::Public);
    // 同步存入的私有行没有分类，原地更新时由发布补上。
    assert_eq!(item.category, Some(CandidateSkinCategory::Guofeng));
    assert_eq!(
        fixture.library.calls(),
        ["replace 公开樱花", "visibility", "category guofeng"]
    );
    assert_eq!(
        fixture.library.row_names(),
        [("sakura".to_owned(), "公开樱花".to_owned())]
    );
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());

    // 分类已经一致时不再多发一次请求；分类不计入内容摘要，所以同步仍视为一致。
    fixture.library.calls();
    publish(
        &fixture.root,
        &fixture.state,
        &fixture.library,
        "sakura",
        Uuid::new_v4(),
        "公开樱花".into(),
        "说明".into(),
        CandidateSkinVisibility::Public,
        Some(CandidateSkinCategory::Guofeng),
    )
    .unwrap();
    assert_eq!(fixture.library.calls(), ["replace 公开樱花"]);
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
}

#[test]
fn publishing_cancels_when_same_user_relogs_in_before_state_save() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.library.relogin_on_publish.set(true);

    assert_eq!(
        publish(
            &fixture.root,
            &fixture.state,
            &fixture.library,
            "sakura",
            Uuid::new_v4(),
            "樱花".into(),
            String::new(),
            CandidateSkinVisibility::Private,
            None,
        ),
        Err(CandidateSkinPublishError::Account(AccountError::Cancelled))
    );
    assert!(!fixture.state.exists());
}

#[test]
fn publishing_a_new_package_creates_a_row_that_sync_then_knows() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    let publication = Uuid::new_v4();
    assert_eq!(
        publish(
            &fixture.root,
            &fixture.state,
            &fixture.library,
            "sakura",
            publication,
            "樱花".into(),
            String::new(),
            CandidateSkinVisibility::Public,
            None,
        ),
        Err(CandidateSkinPublishError::Package(
            "candidate_skin_license_required"
        ))
    );
    let item = publish(
        &fixture.root,
        &fixture.state,
        &fixture.library,
        "sakura",
        publication,
        "樱花".into(),
        String::new(),
        CandidateSkinVisibility::Private,
        Some(CandidateSkinCategory::Cute),
    )
    .unwrap();
    assert_eq!(item.id, publication);
    assert_eq!(item.category, Some(CandidateSkinCategory::Cute));
    fixture.library.calls();
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
    assert_eq!(fixture.library.calls(), ["list"]);
}

#[test]
fn an_unpublished_row_is_uploaded_again_rather_than_deleted_locally() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    unpublish(&fixture.state, &fixture.library, id).unwrap();
    assert!(synced_packages(&fixture.state).is_empty());
    let report = fixture.sync();
    assert!(report.deleted_local.is_empty());
    assert_eq!(report.uploaded, ids(&["sakura"]));
    assert_eq!(
        synced_packages(&fixture.state).keys().collect::<Vec<_>>(),
        ["sakura"]
    );
}

#[test]
fn a_failed_unpublish_keeps_the_row_remembered() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    assert_eq!(
        unpublish(&fixture.state, &fixture.library, Uuid::new_v4()),
        Err(AccountError::NotFound)
    );
    assert_eq!(
        synced_packages(&fixture.state).keys().collect::<Vec<_>>(),
        ["sakura"]
    );
}

#[test]
fn a_missing_skin_root_deletes_nothing_once_packages_are_remembered() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fs::remove_dir_all(&fixture.root).unwrap();
    fixture.library.calls();
    assert_eq!(
        sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library),
        Err(AccountError::Storage)
    );
    assert!(!fixture.library.calls().contains(&"unpublish".to_owned()));
    assert_eq!(fixture.library.rows.borrow().len(), 1);
}

#[cfg(unix)]
#[test]
fn a_symlinked_skin_root_deletes_nothing_once_packages_are_remembered() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    let outside = tempfile::tempdir().unwrap();
    write_skin(outside.path(), "outside", "外部", 2);
    fs::remove_dir_all(&fixture.root).unwrap();
    symlink(outside.path(), &fixture.root).unwrap();
    fixture.library.calls();

    assert_eq!(
        sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library),
        Err(AccountError::Storage)
    );
    assert!(outside.path().join("outside/skin.toml").is_file());
    assert!(!fixture.library.calls().contains(&"unpublish".to_owned()));
    assert_eq!(fixture.library.rows.borrow().len(), 1);
}

#[cfg(unix)]
#[test]
fn a_symlinked_sync_state_does_not_delete_a_local_skin() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    let packed = pack_as(&fixture.root, "sakura", CandidateSkinVisibility::Private).unwrap();
    let local_digest = content_digest(&packed.manifest, &packed.files).unwrap();
    let external = tempfile::tempdir().unwrap();
    let external_state = external.path().join("state.json");
    let state = SyncState {
        user_id: "user-1".to_owned(),
        packages: BTreeMap::from([(
            "sakura".to_owned(),
            SyncedPackage {
                cloud_id: Uuid::new_v4(),
                cloud_digest: "a".repeat(64),
                local_digest,
            },
        )]),
        installed: BTreeMap::new(),
    };
    let bytes = serde_json::to_vec(&state).unwrap();
    fs::write(&external_state, &bytes).unwrap();
    symlink(&external_state, &fixture.state).unwrap();

    // 外部同步状态不能让同步删除本地皮肤，且外部文件不能被改写。
    let report = fixture.sync();
    assert_eq!(report.uploaded, ids(&["sakura"]));
    assert!(fixture.installed("sakura"));
    assert_eq!(fs::read(&external_state).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn a_symlinked_sync_state_parent_does_not_write_outside() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    let outside = tempfile::tempdir().unwrap();
    let linked_parent = fixture._directory.path().join("linked-state");
    symlink(outside.path(), &linked_parent).unwrap();
    let state_path = linked_parent.join(STATE_FILE);

    // 状态文件父目录是外部链接时，保存不能在外部目录留下同步状态。
    let report = sync_candidate_skins(&fixture.root, &state_path, &fixture.library).unwrap();
    assert_eq!(report.uploaded, ids(&["sakura"]));
    assert!(!outside.path().join(STATE_FILE).exists());
    assert!(fixture.installed("sakura"));
}

#[cfg(unix)]
#[test]
fn an_unreadable_skin_root_deletes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&fixture.root).is_ok();
    let result = sync_candidate_skins(&fixture.root, &fixture.state, &fixture.library);
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o755)).unwrap();
    // A superuser reads the folder regardless of its mode, and then there is nothing to prove.
    if !readable {
        assert_eq!(result, Err(AccountError::Storage));
    }
    assert_eq!(fixture.library.rows.borrow().len(), 1);
}

#[test]
fn a_public_package_deleted_locally_stays_published() {
    let fixture = Fixture::new();
    write_skin(&fixture.root, "sakura", "樱花", 1);
    fixture.sync();
    let id = *fixture.library.rows.borrow().keys().next().unwrap();
    fixture
        .library
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .visibility = CandidateSkinVisibility::Public;
    fs::remove_dir_all(fixture.root.join("sakura")).unwrap();
    let report = fixture.sync();
    assert!(report.deleted_cloud.is_empty());
    assert_eq!(
        report.skipped,
        [CandidateSkinSyncSkip {
            package_id: "sakura".into(),
            code: PUBLIC_KEPT
        }]
    );
    assert_eq!(fixture.library.rows.borrow().len(), 1);
    assert!(!fixture.installed("sakura"));
}

#[test]
fn publishing_updates_a_row_sync_has_not_recorded_yet() {
    let fixture = Fixture::new();
    let other = tempfile::tempdir().unwrap();
    write_skin(other.path(), "sakura", "樱花", 1);
    let id = fixture.library.insert(other.path(), "sakura", "樱花");
    write_skin(&fixture.root, "sakura", "樱花", 2);
    let item = publish(
        &fixture.root,
        &fixture.state,
        &fixture.library,
        "sakura",
        Uuid::new_v4(),
        "新樱花".into(),
        String::new(),
        CandidateSkinVisibility::Private,
        None,
    )
    .unwrap();
    assert_eq!(item.id, id);
    assert_eq!(fixture.library.calls(), ["list", "replace 新樱花"]);
    assert_eq!(
        fixture.library.row_names(),
        [("sakura".to_owned(), "新樱花".to_owned())]
    );
    assert_eq!(fixture.sync(), CandidateSkinSyncReport::default());
}
