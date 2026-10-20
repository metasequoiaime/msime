use super::*;
use crate::skins::{self, CreateSkinRequest};
use msime_client_services::account::{
    AccountChallenge, AccountProfile, AccountSessionStorage, AccountTokens, AccountUser,
    SavedAccountSession,
};
use msime_client_services::skin::candidate_community::{
    CandidateSkinItem, CandidateSkinLicense, CandidateSkinPackage, CandidateSkinPage,
    CandidateSkinPreview, CandidateSkinPublishRequest, CandidateSkinReplaceRequest,
    CandidateSkinSyncEntry,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";
/// 合成的访问令牌：结果和错误里都不该出现它。
const ACCESS_TOKEN: &str = "a1a2a3a4a5a6a7a8a9a0b1b2b3b4b5b6b7b8b9b0c1c2c3c4c5c6c7c8c9c0d1d2";
const PACKAGE: &str = "sunset";

#[derive(Default)]
struct FakeState {
    rows: Vec<CandidateSkinSyncEntry>,
    /// 每次请求的名字和参数，按顺序。
    calls: Vec<String>,
    /// 每次请求带的令牌。
    bearers: Vec<String>,
    published: Vec<CandidateSkinPublishRequest>,
    refuse_visibility: Option<AccountError>,
}

/// 只在内存里应答的账号服务器，测试不联网。
#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<FakeState>>);

impl FakeApi {
    fn with_rows(rows: Vec<CandidateSkinSyncEntry>) -> Self {
        let api = Self::default();
        api.0.lock().unwrap().rows = rows;
        api
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }

    fn record(&self, call: String, bearer: &str) {
        let mut state = self.0.lock().unwrap();
        state.calls.push(call);
        state.bearers.push(bearer.to_owned());
    }

    fn row(&self, id: Uuid) -> Option<CandidateSkinSyncEntry> {
        self.0
            .lock()
            .unwrap()
            .rows
            .iter()
            .find(|row| row.id == id)
            .cloned()
    }
}

fn item(id: Uuid, visibility: CandidateSkinVisibility) -> CandidateSkinItem {
    CandidateSkinItem {
        id,
        package_id: PACKAGE.into(),
        name: "Sunset".into(),
        description: String::new(),
        author: "Fixture".into(),
        version: "1.0".into(),
        license: CandidateSkinLicense {
            code: String::new(),
            assets: "CC0-1.0".into(),
            source: String::new(),
        },
        size: 70,
        file_count: 1,
        downloads: 0,
        rating_count: 0,
        rating_average: 0.0,
        owned: true,
        my_rating: 0,
        created_at: "2026-10-20T00:00:00Z".into(),
        visibility,
        updated_at: "2026-10-20T00:00:00Z".into(),
        request_sha256: String::new(),
        moderation: None,
        category: Some(CandidateSkinCategory::Other),
    }
}

fn row(id: Uuid, visibility: CandidateSkinVisibility) -> CandidateSkinSyncEntry {
    CandidateSkinSyncEntry {
        id,
        package_id: PACKAGE.into(),
        request_sha256: "0".repeat(64),
        visibility,
        updated_at: "2026-10-20T00:00:00Z".into(),
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        Ok(HashMap::new())
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        panic!("the server never signs in")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        panic!("the server never signs in")
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

impl CandidateSkinCommunityApi for FakeApi {
    fn candidate_skins(
        &self,
        _: usize,
        _: &str,
        _: bool,
        _: Option<CandidateSkinCategory>,
        _: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn candidate_skin(&self, _: Uuid, _: Option<&str>) -> Result<CandidateSkinItem, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn candidate_skin_preview(
        &self,
        _: Uuid,
        _: Option<&str>,
    ) -> Result<CandidateSkinPreview, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn publish_candidate_skin(
        &self,
        request: &CandidateSkinPublishRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.record("publish".into(), token);
        let mut state = self.0.lock().unwrap();
        state.published.push(request.clone());
        state.rows.push(row(request.id, request.visibility));
        let mut published = item(request.id, request.visibility);
        published.category = Some(request.category.unwrap_or_default());
        Ok(published)
    }
    fn download_candidate_skin(
        &self,
        _: Uuid,
        _: &str,
    ) -> Result<CandidateSkinPackage, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rate_candidate_skin(&self, _: Uuid, _: u8, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn unpublish_candidate_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        self.record(format!("unpublish {id}"), token);
        let mut state = self.0.lock().unwrap();
        let before = state.rows.len();
        state.rows.retain(|row| row.id != id);
        if state.rows.len() == before {
            return Err(AccountError::NotFound);
        }
        Ok(())
    }
    fn candidate_skin_sync_list(
        &self,
        token: &str,
    ) -> Result<Vec<CandidateSkinSyncEntry>, AccountError> {
        self.record("sync_list".into(), token);
        Ok(self.0.lock().unwrap().rows.clone())
    }
    fn replace_candidate_skin(
        &self,
        id: Uuid,
        _: &CandidateSkinReplaceRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.record(format!("replace {id}"), token);
        let row = self.row(id).ok_or(AccountError::NotFound)?;
        Ok(item(id, row.visibility))
    }
    fn set_candidate_skin_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.record(format!("visibility {id} {visibility:?}"), token);
        if let Some(error) = self.0.lock().unwrap().refuse_visibility.clone() {
            return Err(error);
        }
        let mut state = self.0.lock().unwrap();
        let row = state
            .rows
            .iter_mut()
            .find(|row| row.id == id)
            .ok_or(AccountError::NotFound)?;
        row.visibility = visibility;
        Ok(item(id, visibility))
    }
    fn set_candidate_skin_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.record(format!("category {id} {}", category.as_str()), token);
        let row = self.row(id).ok_or(AccountError::NotFound)?;
        let mut changed = item(id, row.visibility);
        changed.category = Some(category);
        Ok(changed)
    }
}

/// 一台装了 `sunset`、设置应用登录过（`signed_in`）或没登录过的电脑：状态目录、会话目录和接在会话文件上的社区服务。
struct Computer {
    state: tempfile::TempDir,
    account: tempfile::TempDir,
    api: FakeApi,
}

impl Computer {
    fn new(signed_in: bool, api: FakeApi) -> Self {
        let state = tempfile::tempdir().unwrap();
        let account = tempfile::tempdir().unwrap();
        skins::create(state.path(), None, &skin_request(PACKAGE, "")).unwrap();
        if signed_in {
            // 设置应用登录后写下的会话文件。
            FileAccountSessionStorage::new(account.path(), AccountSessionFileLayout::Native)
                .save(&saved_session())
                .unwrap();
        }
        Self {
            state,
            account,
            api,
        }
    }

    fn service(&self) -> BackendCandidateSkinCommunityService<FakeApi, FileAccountSessionStorage> {
        service_at(
            self.api.clone(),
            self.account.path(),
            AccountSessionFileLayout::Native,
        )
    }

    fn publish(&self, request: PublishRequest) -> Result<PublishedSkin, String> {
        publish(self.state.path(), &self.service(), &request)
    }

    fn manifest(&self) -> String {
        std::fs::read_to_string(
            self.state
                .path()
                .join("skins")
                .join(PACKAGE)
                .join("skin.toml"),
        )
        .unwrap()
    }
}

fn saved_session() -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: ACCESS_TOKEN.into(),
            refresh_token: "e1e2e3e4e5e6e7e8e9e0f1f2f3f4f5f6f7f8f9f0a1a2a3a4a5a6a7a8a9a0b1b2"
                .into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: "fixture-user".into(),
                display_name: "Fixture".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000,
        session_id: None,
    }
}

fn skin_request(id: &str, extra: &str) -> CreateSkinRequest {
    CreateSkinRequest {
        package_id: id.into(),
        manifest: format!(
            "schema_version = 1\nid = '{id}'\nname = 'Sunset'\nversion = '1.0'\nbase = 'night'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['dark']\n[candidate_window]\nmin_width_dip = 200\n{extra}"
        ),
        images: BTreeMap::from([("preview.png".into(), PNG.into())]),
        image_files: BTreeMap::new(),
        replace: true,
    }
}

fn request(visibility: Visibility) -> PublishRequest {
    PublishRequest {
        package_id: PACKAGE.into(),
        visibility,
        category: None,
        name: None,
        description: None,
        license_assets: None,
    }
}

#[test]
fn without_the_settings_apps_session_nothing_is_sent_or_changed() {
    let computer = Computer::new(false, FakeApi::default());
    let before = computer.manifest();
    let error = computer
        .publish(PublishRequest {
            license_assets: Some("CC0-1.0".into()),
            ..request(Visibility::Public)
        })
        .unwrap_err();
    assert!(error.contains("sign in"), "{error}");
    assert!(error.contains("settings app"), "{error}");
    // 没登录时连清单里的授权都不写。
    assert_eq!(computer.manifest(), before);
    assert_eq!(list(&computer.service()).unwrap_err(), error);
    let visibility = set_visibility(
        &computer.service(),
        &VisibilityRequest {
            id: Uuid::new_v4().to_string(),
            visibility: Visibility::Private,
        },
    )
    .unwrap_err();
    assert_eq!(visibility, error);
    assert!(computer.api.calls().is_empty());
    // 不建会话文件：服务器不登录。
    assert!(!computer
        .account
        .path()
        .join("account-session.json")
        .exists());
}

#[test]
fn a_private_publish_creates_an_entry_and_a_public_one_replaces_it() {
    let computer = Computer::new(true, FakeApi::default());
    let published = computer.publish(request(Visibility::Private)).unwrap();
    assert!(!published.replaced);
    assert_eq!(published.visibility, Visibility::Private);
    assert_eq!(published.package_id, PACKAGE);
    // 没给标题时用清单的 name。
    assert_eq!(computer.api.0.lock().unwrap().published[0].name, "Sunset");
    assert_eq!(computer.api.calls(), ["sync_list", "publish"]);
    let value = serde_json::to_value(&published).unwrap();
    crate::assert_required_fields_present::<PublishedSkin>(&value);
    // 设置应用读的同步状态记下了它，所以它不会被设置应用当成新包再传一行。
    assert!(skins::list(computer.state.path()).skins[0].synced);

    // 公开发布同一款皮肤：替换已有的那一行，不新建；没有授权时先写进清单。
    let refused = computer.publish(request(Visibility::Public)).unwrap_err();
    assert!(refused.contains("license_assets"), "{refused}");
    assert_eq!(computer.api.calls().len(), 2, "refused before any request");
    let public = computer
        .publish(PublishRequest {
            category: Some(Category::Nature),
            name: Some(" Sunset Glow ".into()),
            license_assets: Some(" CC-BY-4.0 ".into()),
            ..request(Visibility::Public)
        })
        .unwrap();
    assert!(public.replaced);
    assert_eq!(public.id, published.id);
    assert_eq!(public.visibility, Visibility::Public);
    assert_eq!(public.category, Some(Category::Nature));
    assert!(computer.manifest().contains("assets = \"CC-BY-4.0\""));
    let id = &published.id;
    assert_eq!(
        computer.api.calls()[2..],
        [
            format!("replace {id}"),
            format!("visibility {id} Public"),
            format!("category {id} nature"),
        ]
    );
    // 每个请求都带着设置应用保存的那份会话的令牌，而令牌不出现在结果里。
    assert!(computer
        .api
        .0
        .lock()
        .unwrap()
        .bearers
        .iter()
        .all(|bearer| bearer == ACCESS_TOKEN));
    assert!(!serde_json::to_string(&public)
        .unwrap()
        .contains(ACCESS_TOKEN));
}

#[test]
fn a_row_the_settings_app_synced_is_replaced_rather_than_duplicated() {
    let existing = Uuid::new_v4();
    let api = FakeApi::with_rows(vec![row(existing, CandidateSkinVisibility::Private)]);
    // 同步状态里没有它（例如在另一台电脑上同步的），靠 package_id 在库里找到。
    let computer = Computer::new(true, api);
    let published = computer.publish(request(Visibility::Private)).unwrap();
    assert!(published.replaced);
    assert_eq!(published.id, existing.hyphenated().to_string());
    assert_eq!(
        computer.api.calls(),
        ["sync_list".to_owned(), format!("replace {existing}")]
    );
    assert!(computer.api.0.lock().unwrap().published.is_empty());
}

#[test]
fn a_license_the_manifest_already_declares_is_kept() {
    let computer = Computer::new(true, FakeApi::default());
    skins::create(
        computer.state.path(),
        None,
        &skin_request(PACKAGE, "[license]\nassets = 'CC0-1.0'\n"),
    )
    .unwrap();
    let error = computer
        .publish(PublishRequest {
            license_assets: Some("CC-BY-4.0".into()),
            ..request(Visibility::Public)
        })
        .unwrap_err();
    assert!(error.contains("already declares"), "{error}");
    assert!(computer.api.calls().is_empty());
    // 给的和清单里的一样，或者不给，都照常发布。
    computer
        .publish(PublishRequest {
            license_assets: Some("CC0-1.0".into()),
            ..request(Visibility::Public)
        })
        .unwrap();
    assert!(computer.manifest().contains("assets = 'CC0-1.0'"));
}

#[test]
fn what_cannot_be_published_is_refused_before_any_request() {
    let computer = Computer::new(true, FakeApi::default());
    for (package_id, expected) in [
        ("night", "cannot be published"),
        ("missing", "no skin missing is installed"),
    ] {
        let error = computer
            .publish(PublishRequest {
                package_id: package_id.into(),
                ..request(Visibility::Private)
            })
            .unwrap_err();
        assert!(error.contains(expected), "{package_id}: {error}");
    }
    let error = computer
        .publish(PublishRequest {
            name: Some("x".repeat(33)),
            ..request(Visibility::Private)
        })
        .unwrap_err();
    assert!(error.contains("1 to 32 characters"), "{error}");
    let error = computer
        .publish(PublishRequest {
            description: Some("x".repeat(281)),
            ..request(Visibility::Private)
        })
        .unwrap_err();
    assert!(error.contains("at most 280"), "{error}");
    // 没有预览的皮肤不能分享。
    std::fs::write(
        computer
            .state
            .path()
            .join("skins")
            .join(PACKAGE)
            .join("skin.toml"),
        computer.manifest().replace("preview = 'preview.png'\n", ""),
    )
    .unwrap();
    let error = computer.publish(request(Visibility::Private)).unwrap_err();
    assert!(error.contains("has no preview"), "{error}");
    assert!(computer.api.calls().is_empty());
}

#[test]
fn visibility_and_category_must_be_named_exactly() {
    let parse = |value: serde_json::Value| serde_json::from_value::<PublishRequest>(value);
    assert!(parse(serde_json::json!({ "package_id": PACKAGE })).is_err());
    assert!(parse(serde_json::json!({ "package_id": PACKAGE, "visibility": "unlisted" })).is_err());
    assert!(parse(
        serde_json::json!({ "package_id": PACKAGE, "visibility": "private", "category": "anime" })
    )
    .is_err());
    let parsed = parse(
        serde_json::json!({ "package_id": PACKAGE, "visibility": "public", "category": "guofeng" }),
    )
    .unwrap();
    assert_eq!(parsed.visibility, Visibility::Public);
    assert_eq!(parsed.category, Some(Category::Guofeng));
}

#[test]
fn the_library_is_listed_and_an_entry_made_private_or_deleted() {
    let public = Uuid::new_v4();
    let private = Uuid::new_v4();
    let api = FakeApi::with_rows(vec![
        row(public, CandidateSkinVisibility::Public),
        row(private, CandidateSkinVisibility::Private),
    ]);
    let computer = Computer::new(true, api);
    let service = computer.service();
    let listed = list(&service).unwrap();
    assert_eq!(listed.skins.len(), 2);
    assert_eq!(listed.skins[0].id, public.hyphenated().to_string());
    assert_eq!(listed.skins[0].visibility, Visibility::Public);
    let value = serde_json::to_value(&listed).unwrap();
    crate::assert_required_fields_present::<CommunitySkinList>(&value);
    crate::assert_required_fields_present::<CommunitySkinView>(&value["skins"][0]);

    let changed = set_visibility(
        &service,
        &VisibilityRequest {
            id: public.to_string(),
            visibility: Visibility::Private,
        },
    )
    .unwrap();
    assert_eq!(changed.visibility, Visibility::Private);
    crate::assert_required_fields_present::<VisibilityChanged>(
        &serde_json::to_value(&changed).unwrap(),
    );
    let error = set_visibility(
        &service,
        &VisibilityRequest {
            id: "sunset".into(),
            visibility: Visibility::Private,
        },
    )
    .unwrap_err();
    assert!(error.contains("not a community id"), "{error}");

    // 转为公开被拒（服务端要求已上传的清单带授权）时，指向能一起上传授权的发布工具。
    computer.api.0.lock().unwrap().refuse_visibility = Some(AccountError::Invalid);
    let error = set_visibility(
        &service,
        &VisibilityRequest {
            id: private.to_string(),
            visibility: Visibility::Public,
        },
    )
    .unwrap_err();
    assert!(error.contains("publish_candidate_skin"), "{error}");

    let deleted = delete(
        computer.state.path(),
        &service,
        &CommunitySkinRequest {
            id: private.to_string(),
        },
    )
    .unwrap();
    assert_eq!(deleted.id, private.hyphenated().to_string());
    crate::assert_required_fields_present::<DeletedCommunitySkin>(
        &serde_json::to_value(&deleted).unwrap(),
    );
    assert!(computer.api.row(private).is_none());
    // 本机那款皮肤留着。
    assert_eq!(skins::list(computer.state.path()).skins.len(), 1);
}

#[test]
fn server_refusals_read_in_plain_words() {
    for (error, expected) in [
        (AccountError::Unauthorized, "settings app"),
        (
            AccountError::Conflict,
            "at most 100 skins, at most 20 of them public",
        ),
        (AccountError::RateLimited, "10 public publishes an hour"),
        (AccountError::Invalid, "more than 3 images"),
        (AccountError::BlockedContent, "does not allow some words"),
        (AccountError::NotFound, "list_community_skins"),
    ] {
        let text = account_problem(error.clone());
        assert!(text.contains(expected), "{error:?}: {text}");
    }
}

#[test]
fn the_session_is_where_the_settings_app_keeps_it() {
    let state = Path::new("/state");
    let edition = Edition::full();
    let env = |name: &str| match name {
        "HOME" => Some(OsString::from("/home/someone")),
        "LOCALAPPDATA" => Some(OsString::from("C:\\Users\\someone\\AppData\\Local")),
        _ => None,
    };
    let (directory, layout) = session_location(state, edition, env).unwrap();
    if cfg!(target_os = "macos") {
        assert_eq!(
            directory,
            Path::new("/home/someone/Library/Application Support")
                .join(&edition.macos().unwrap().settings_bundle_id)
        );
        assert_eq!(layout, AccountSessionFileLayout::Apple);
        assert!(session_location(state, edition, |_| None)
            .unwrap_err()
            .contains("HOME"));
    } else if cfg!(windows) {
        assert!(directory
            .ends_with(Path::new(&edition.windows().unwrap().user_data_directory).join("account")));
        assert_eq!(layout, AccountSessionFileLayout::Native);
    } else {
        assert_eq!(directory, state);
        assert_eq!(layout, AccountSessionFileLayout::Native);
    }
}
