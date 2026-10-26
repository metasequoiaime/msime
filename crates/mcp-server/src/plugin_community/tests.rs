use super::*;
use crate::plugins::{self as local, BuiltinPaths, CreatePluginRequest};
use msime_client_core::preferences::Preferences;
use msime_client_services::account::{
    AccountChallenge, AccountProfile, AccountSessionStorage, AccountTokens, AccountUser,
    SavedAccountSession,
};
use msime_client_services::plugins::community::{
    CommunityPlugin, CommunityPluginDownload, CommunityPluginPage, CommunityPluginPublishRequest,
};
use msime_client_services::plugins::PluginKind;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 合成的访问令牌：结果和错误里都不该出现它。
const ACCESS_TOKEN: &str = "a1a2a3a4a5a6a7a8a9a0b1b2b3b4b5b6b7b8b9b0c1c2c3c4c5c6c7c8c9c0d1d2";

#[derive(Default)]
struct FakeState {
    /// 每次请求的名字，按顺序。
    calls: Vec<String>,
    /// 每次请求带的令牌。
    bearers: Vec<String>,
    published: Vec<CommunityPluginPublishRequest>,
    listed: Vec<CommunityPlugin>,
}

/// 只在内存里应答的账号服务器，测试不联网。
#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<FakeState>>);

impl FakeApi {
    fn record(&self, call: &str, bearer: Option<&str>) {
        let mut state = self.0.lock().unwrap();
        state.calls.push(call.to_owned());
        state.bearers.push(bearer.unwrap_or_default().to_owned());
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }
}

fn item(id: Uuid, kind: PluginKind, plugin_id: &str, name: &str) -> CommunityPlugin {
    CommunityPlugin {
        id,
        kind,
        plugin_id: plugin_id.into(),
        name: name.into(),
        description: String::new(),
        author: "Fixture".into(),
        version: "1.0.0".into(),
        license: "CC0-1.0".into(),
        size: 100,
        sha256: "0".repeat(64),
        downloads: 3,
        rating_count: 0,
        rating_average: 0.0,
        owned: true,
        my_rating: 0,
        created_at: "2026-10-26T00:00:00Z".into(),
        moderation: None,
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

impl CommunityPluginApi for FakeApi {
    fn community_plugins(
        &self,
        _: usize,
        _: &str,
        _: Option<PluginKind>,
        mine: bool,
        token: Option<&str>,
    ) -> Result<CommunityPluginPage, AccountError> {
        self.record(if mine { "list_mine" } else { "list" }, token);
        Ok(CommunityPluginPage {
            plugins: self.0.lock().unwrap().listed.clone(),
            has_more: false,
            skipped: 0,
        })
    }
    fn community_plugin(&self, _: Uuid, _: Option<&str>) -> Result<CommunityPlugin, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn publish_community_plugin(
        &self,
        request: &CommunityPluginPublishRequest,
        token: &str,
    ) -> Result<CommunityPlugin, AccountError> {
        self.record("publish", Some(token));
        self.0.lock().unwrap().published.push(request.clone());
        Ok(CommunityPlugin {
            version: request.version.clone(),
            ..item(request.id, request.kind, &request.plugin_id, &request.name)
        })
    }
    fn download_community_plugin(
        &self,
        _: Uuid,
        _: &str,
    ) -> Result<CommunityPluginDownload, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rate_community_plugin(&self, _: Uuid, _: u8, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn delete_community_plugin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        self.record(&format!("delete {id}"), Some(token));
        Ok(())
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

/// 一个状态目录（装着一张合成短语表）和一个账号会话目录，`signed_in` 决定会话目录里有没有会话。
struct Fixture {
    state: tempfile::TempDir,
    account: tempfile::TempDir,
    api: FakeApi,
}

impl Fixture {
    fn new(signed_in: bool) -> Self {
        let state = tempfile::tempdir().unwrap();
        let account = tempfile::tempdir().unwrap();
        local::create(
            state.path(),
            &Preferences::default(),
            &CreatePluginRequest {
                manifest: "schema_version = 1\nkind = \"phrase_table\"\nid = \"signature\"\nname = \"邮件签名\"\nversion = \"1.0.0\"\nlicense = \"CC0-1.0\"\ndescription = \"合成的签名短语\"\n[[phrases]]\nkey = \"qm\"\ntext = \"合成签名\"\n".into(),
                files: BTreeMap::new(),
                texts: BTreeMap::new(),
                replace: false,
            },
        )
        .unwrap();
        if signed_in {
            FileAccountSessionStorage::new(account.path(), AccountSessionFileLayout::Native)
                .save(&saved_session())
                .unwrap();
        }
        Self {
            state,
            account,
            api: FakeApi::default(),
        }
    }

    fn community(&self) -> PluginCommunity<FakeApi> {
        community_at(
            self.api.clone(),
            self.account.path(),
            AccountSessionFileLayout::Native,
        )
    }

    fn publish(&self, request: PublishPluginRequest) -> Result<PublishedPlugin, String> {
        publish(self.state.path(), &self.community(), &request)
    }
}

fn request(kind: &str, id: &str) -> PublishPluginRequest {
    PublishPluginRequest {
        kind: kind.into(),
        id: id.into(),
        name: None,
        description: None,
    }
}

#[test]
fn publishing_without_a_session_asks_the_user_to_sign_in() {
    let fixture = Fixture::new(false);
    let error = fixture
        .publish(request("phrase_table", "signature"))
        .unwrap_err();
    assert_eq!(error, SIGN_IN);
    assert!(error.contains("settings app"));
    // 没登录时一个请求都不发。
    assert!(fixture.api.calls().is_empty());
    assert_eq!(list(&fixture.community()).unwrap_err(), SIGN_IN);
    assert_eq!(
        delete(
            &fixture.community(),
            &CommunityPluginRequest {
                id: Uuid::new_v4().to_string()
            }
        )
        .unwrap_err(),
        SIGN_IN
    );
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn an_installed_data_pack_is_published_as_the_settings_dialog_sends_it() {
    let fixture = Fixture::new(true);
    let published = fixture
        .publish(request("phrase_table", "signature"))
        .unwrap();
    assert_eq!(published.kind, "phrase_table");
    assert_eq!(published.plugin_id, "signature");
    // 标题和简介默认取清单里的。
    assert_eq!(published.name, "邮件签名");
    assert_eq!(published.version, "1.0.0");
    crate::assert_required_fields_present::<PublishedPlugin>(
        &serde_json::to_value(&published).unwrap(),
    );
    let state = fixture.api.0.lock().unwrap();
    assert_eq!(state.calls, ["publish"]);
    assert_eq!(state.bearers, [ACCESS_TOKEN]);
    let sent = &state.published[0];
    assert_eq!(sent.kind, PluginKind::PhraseTable);
    assert_eq!(sent.plugin_id, "signature");
    assert_eq!(sent.description, "合成的签名短语");
    assert_eq!(sent.id.hyphenated().to_string(), published.id);
    assert!(!sent.archive.is_empty());
    // 令牌不出现在结果里。
    assert!(!serde_json::to_string(&published)
        .unwrap()
        .contains(ACCESS_TOKEN));
}

#[test]
fn the_listing_text_can_be_given_and_is_checked() {
    let fixture = Fixture::new(true);
    let mut custom = request("phrase_table", "signature");
    custom.name = Some("  我的签名  ".into());
    custom.description = Some("给邮件用".into());
    assert_eq!(fixture.publish(custom).unwrap().name, "我的签名");
    let mut long = request("phrase_table", "signature");
    long.name = Some("长".repeat(33));
    assert!(fixture
        .publish(long)
        .unwrap_err()
        .contains("1 to 32 characters"));
    let mut empty = request("phrase_table", "signature");
    empty.name = Some("  ".into());
    assert!(fixture
        .publish(empty)
        .unwrap_err()
        .contains("1 to 32 characters"));
}

#[test]
fn only_the_users_own_data_packs_are_published() {
    let fixture = Fixture::new(true);
    for (kind, id, expected) in [
        (
            "phrase_table",
            "latex-symbols",
            "ships with the input method",
        ),
        ("sound", "default", "ships with the input method"),
        (
            "phrase_table",
            "missing",
            "no phrase_table pack missing is installed",
        ),
        ("phrase_table", "../signature", "no phrase_table pack"),
        ("theme", "signature", "is not a plugin kind"),
    ] {
        let error = fixture.publish(request(kind, id)).unwrap_err();
        assert!(error.contains(expected), "{kind}/{id}: {error}");
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        let error = fixture.publish(request("script", "uuid")).unwrap_err();
        assert!(
            error.contains("script plugin cannot be published"),
            "{error}"
        );
    }
    // 包不合分享规则时在本机拒绝，不联网。
    std::fs::write(
        fixture
            .state
            .path()
            .join("plugins/phrase_table/signature/extra.wav"),
        b"RIFF",
    )
    .unwrap();
    let error = fixture
        .publish(request("phrase_table", "signature"))
        .unwrap_err();
    assert!(error.contains("sharing rules"), "{error}");
    assert!(fixture.api.calls().is_empty());
    // 列表照常看得到它在 issues 里。
    let listed = local::list(
        fixture.state.path(),
        &BuiltinPaths::default(),
        &Preferences::default(),
    );
    assert_eq!(listed.issues.len(), 1);
}

#[test]
fn the_accounts_own_plugins_are_listed_and_deleted() {
    let fixture = Fixture::new(true);
    let id = Uuid::new_v4();
    fixture.api.0.lock().unwrap().listed =
        vec![item(id, PluginKind::PhraseTable, "signature", "邮件签名")];
    let listed = list(&fixture.community()).unwrap();
    assert_eq!(
        listed,
        CommunityPluginList {
            plugins: vec![CommunityPluginView {
                id: id.hyphenated().to_string(),
                kind: "phrase_table".into(),
                plugin_id: "signature".into(),
                name: "邮件签名".into(),
                version: "1.0.0".into(),
                downloads: 3,
                created_at: "2026-10-26T00:00:00Z".into(),
            }]
        }
    );
    let value = serde_json::to_value(&listed).unwrap();
    crate::assert_required_fields_present::<CommunityPluginList>(&value);
    crate::assert_required_fields_present::<CommunityPluginView>(&value["plugins"][0]);

    let deleted = delete(
        &fixture.community(),
        &CommunityPluginRequest {
            id: format!(" {} ", id.hyphenated()),
        },
    )
    .unwrap();
    assert_eq!(deleted.id, id.hyphenated().to_string());
    assert_eq!(
        fixture.api.calls(),
        ["list_mine".to_owned(), format!("delete {id}")]
    );
    let error = delete(
        &fixture.community(),
        &CommunityPluginRequest {
            id: "not-an-id".into(),
        },
    )
    .unwrap_err();
    assert!(error.contains("list_community_plugins"), "{error}");
    // 本机装着的那份不动。
    assert!(fixture
        .state
        .path()
        .join("plugins/phrase_table/signature")
        .is_dir());
}

#[test]
fn community_refusals_say_what_to_do() {
    assert_eq!(community_problem("community_unauthorized"), SIGN_IN);
    assert!(community_problem("community_conflict").contains("20 published packs"));
    assert!(community_problem("community_rate_limited").contains("10 times an hour"));
    assert!(pack_problem(PluginFailure {
        code: "plugin_community_too_large",
        detail: None
    })
    .contains("8 MB"));
    assert_eq!(
        pack_problem(PluginFailure {
            code: "community_unauthorized",
            detail: None
        }),
        SIGN_IN
    );
}
