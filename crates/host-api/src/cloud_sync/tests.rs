//! 驱动的测试：假的后端按服务端的比较并交换规则回答，本机是内存里的假实现，会话在内存里（多进程共享，每次访问都重新读）。不联网，只用合成数据。覆盖首次同步、冲突、删除、退出登录、退避、状态读写往返、并发和过期提交、schema 外的键、合并后超限、`use_cloud` 遇 revision 0、云端被清空、跑到一半换账号和词库的几条路径。

use super::*;
use msime_client_services::account::{
    AccountChallenge, AccountDataSection, AccountDataSummary, AccountDictionarySnapshotRestore,
    AccountPreferenceField, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const USER: &str = "synthetic-user";
const NOW: u64 = 1_800_000_000_000;

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

fn saved(user: &str, access: char) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: token(access),
            refresh_token: token('b'),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: user.into(),
                display_name: "合成用户".into(),
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
        session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
    }
}

/// 多个进程共用的会话存储：每次访问都重新读，测试可以在中途换掉里面的账号。
#[derive(Clone)]
struct SharedStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl AccountSessionStorage for SharedStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
    fn shared_across_processes(&self) -> bool {
        true
    }
}

fn conflict() -> AccountFailure {
    AccountFailure::from_http_response(409, br#"{"error":{"code":"revision_conflict"}}"#, None)
}

/// 云端。设置、常用语和词库都按修订号比较并交换。
struct Cloud {
    schema: Vec<(&'static str, &'static str)>,
    preferences: AccountPreferences,
    phrases: AccountPhrases,
    dictionary_revision: i64,
    dictionary_reset: bool,
    dictionary_items: i64,
    /// 下一次 PUT 常用语之前，另一台设备先写了这一份：这次 PUT 以 409 失败。
    phrases_written_elsewhere: Option<AccountPhrases>,
    failures: HashMap<&'static str, VecDeque<AccountFailure>>,
    calls: Vec<&'static str>,
    snapshot_uploads: usize,
    /// 读常用语时把会话换成另一个账号，模拟跑到一半在别处换了号。
    switch_account_on_phrases: Option<SharedStorage>,
}

impl Default for Cloud {
    /// 新用户：设置和常用语都是 revision 0 的空文档，词库没有改动。
    fn default() -> Self {
        Self {
            schema: Vec::new(),
            preferences: AccountPreferences {
                revision: 0,
                settings: BTreeMap::new(),
            },
            phrases: AccountPhrases {
                revision: 0,
                phrases: Vec::new(),
            },
            dictionary_revision: 0,
            dictionary_reset: false,
            dictionary_items: 0,
            phrases_written_elsewhere: None,
            failures: HashMap::new(),
            calls: Vec::new(),
            snapshot_uploads: 0,
            switch_account_on_phrases: None,
        }
    }
}

#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<Cloud>>);

impl FakeApi {
    fn cloud(&self) -> std::sync::MutexGuard<'_, Cloud> {
        self.0.lock().unwrap()
    }

    fn call(&self, operation: &'static str) -> Result<(), AccountFailure> {
        let mut cloud = self.cloud();
        cloud.calls.push(operation);
        match cloud
            .failures
            .get_mut(operation)
            .and_then(VecDeque::pop_front)
        {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    fn fail(&self, operation: &'static str, failure: AccountFailure) {
        self.cloud()
            .failures
            .entry(operation)
            .or_default()
            .push_back(failure);
    }

    fn calls(&self) -> Vec<&'static str> {
        std::mem::take(&mut self.cloud().calls)
    }

    fn cloud_texts(&self) -> Vec<String> {
        self.cloud()
            .phrases
            .phrases
            .iter()
            .map(|phrase| phrase.text.clone())
            .collect()
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        unreachable!("providers")
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        unreachable!("challenge")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        unreachable!("login")
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unauthorized)
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        unreachable!("profile")
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        unreachable!("rename")
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        unreachable!("logout")
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        unreachable!("delete_account")
    }
    fn preference_schema_detailed(
        &self,
        _: &str,
    ) -> Result<AccountPreferenceSchema, AccountFailure> {
        self.call("schema")?;
        Ok(AccountPreferenceSchema {
            fields: self
                .cloud()
                .schema
                .iter()
                .map(|(key, kind)| {
                    (
                        (*key).to_owned(),
                        AccountPreferenceField {
                            value_type: (*kind).to_owned(),
                        },
                    )
                })
                .collect(),
            maximum_bytes: 1_048_576,
            update_mode: "replace".into(),
            revision_required: true,
        })
    }
    fn preferences_detailed(&self, _: &str) -> Result<AccountPreferences, AccountFailure> {
        self.call("preferences")?;
        Ok(self.cloud().preferences.clone())
    }
    fn put_preferences_detailed(
        &self,
        preferences: &AccountPreferences,
        _: &str,
    ) -> Result<AccountPreferences, AccountFailure> {
        self.call("put_preferences")?;
        let mut cloud = self.cloud();
        if preferences.revision != cloud.preferences.revision {
            return Err(conflict());
        }
        cloud.preferences = AccountPreferences {
            revision: preferences.revision + 1,
            settings: preferences.settings.clone(),
        };
        Ok(cloud.preferences.clone())
    }
    fn phrases(&self, _: &str) -> Result<AccountPhrases, AccountFailure> {
        self.call("phrases")?;
        let switch = self.cloud().switch_account_on_phrases.take();
        if let Some(storage) = switch {
            *storage.0.lock().unwrap() = Some(saved("someone-else", 'e'));
        }
        Ok(self.cloud().phrases.clone())
    }
    fn put_phrases(
        &self,
        phrases: &AccountPhrases,
        _: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        self.call("put_phrases")?;
        let mut cloud = self.cloud();
        if let Some(elsewhere) = cloud.phrases_written_elsewhere.take() {
            cloud.phrases = elsewhere;
        }
        if phrases.revision != cloud.phrases.revision {
            return Err(conflict());
        }
        assert!(
            msime_client_services::account::valid_phrases(&phrases.phrases),
            "the driver only uploads lists the server accepts"
        );
        cloud.phrases = AccountPhrases {
            revision: phrases.revision + 1,
            phrases: phrases.phrases.clone(),
        };
        Ok(cloud.phrases.clone())
    }
    fn dictionary_probe(
        &self,
        after: i64,
        _: &str,
    ) -> Result<AccountDictionaryProbe, AccountFailure> {
        self.call("dictionary_probe")?;
        let cloud = self.cloud();
        let changed = cloud.dictionary_revision > after;
        Ok(AccountDictionaryProbe {
            changed,
            revision: if changed {
                cloud.dictionary_revision
            } else {
                after
            },
            reset: changed && cloud.dictionary_reset,
        })
    }
    fn data_summary(&self, _: &str) -> Result<AccountDataSummary, AccountFailure> {
        self.call("data_summary")?;
        Ok(AccountDataSummary {
            bytes: 0,
            sections: vec![AccountDataSection {
                id: "dictionary".into(),
                bytes: 0,
                items: self.cloud().dictionary_items,
            }],
        })
    }
    fn dictionary_snapshot_download(
        &self,
        destination: &Path,
        _: &str,
    ) -> Result<u64, AccountFailure> {
        self.call("snapshot_download")?;
        let revision = self.cloud().dictionary_revision;
        std::fs::write(destination, revision.to_string()).unwrap();
        Ok(1)
    }
    fn dictionary_snapshot_upload(
        &self,
        snapshot: &Path,
        revision: i64,
        _: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountFailure> {
        self.call("snapshot_upload")?;
        assert!(
            snapshot.exists(),
            "the export is uploaded before it is removed"
        );
        let mut cloud = self.cloud();
        if revision != cloud.dictionary_revision {
            return Err(conflict());
        }
        cloud.dictionary_revision += 1;
        cloud.snapshot_uploads += 1;
        Ok(AccountDictionarySnapshotRestore {
            revision: cloud.dictionary_revision,
            reset: true,
        })
    }
}

/// 本机。
#[derive(Default)]
struct Local {
    settings: BTreeMap<String, AccountPreferenceValue>,
    skins: Option<String>,
    applied: Vec<AccountPreferences>,
    /// iOS：应用设置时要改键盘设置，共享偏好留到提交时。
    defer: Option<IosKeyboardPreferences>,
    committed: Vec<(AccountPreferences, IosKeyboardPreferences)>,
    commit_failure: Option<String>,
    phrases: Vec<LocalPhrase>,
    next_id: u32,
    words: bool,
    pending: usize,
    exported: usize,
    replaced: Vec<(String, i64)>,
    merged: usize,
    cancelled: Vec<String>,
}

#[derive(Default)]
struct FakeLocal(Mutex<Local>);

impl FakeLocal {
    fn local(&self) -> std::sync::MutexGuard<'_, Local> {
        self.0.lock().unwrap()
    }

    fn texts(&self) -> Vec<String> {
        self.local()
            .phrases
            .iter()
            .map(|phrase| phrase.text.clone())
            .collect()
    }

    fn add_phrase(&self, text: &str) {
        let mut local = self.local();
        local.next_id += 1;
        let id = format!("local-{}", local.next_id);
        local.phrases.push(LocalPhrase {
            id,
            text: text.into(),
        });
    }
}

impl SyncLocal for FakeLocal {
    fn export_settings(&self) -> Result<LocalSettings, String> {
        let local = self.local();
        Ok(LocalSettings {
            settings: local.settings.clone(),
            skins: local.skins.clone(),
        })
    }
    fn apply_settings(
        &self,
        document: &AccountPreferences,
        _: &AccountPreferenceSchema,
    ) -> Result<SettingsApplied, String> {
        let mut local = self.local();
        local.applied.push(document.clone());
        if let Some(native) = local.defer.clone() {
            return Ok(SettingsApplied {
                writes: HostWrites {
                    ios_native: Some(native),
                    ..HostWrites::default()
                },
                deferred: true,
            });
        }
        local.settings = document.settings.clone();
        Ok(SettingsApplied {
            writes: HostWrites::default(),
            deferred: false,
        })
    }
    fn commit_settings(
        &self,
        document: &AccountPreferences,
        saved_native: &IosKeyboardPreferences,
    ) -> Result<(), String> {
        let mut local = self.local();
        if let Some(code) = local.commit_failure.take() {
            return Err(code);
        }
        local
            .committed
            .push((document.clone(), saved_native.clone()));
        Ok(())
    }
    fn phrases(&self) -> Result<Vec<LocalPhrase>, String> {
        Ok(self.local().phrases.clone())
    }
    fn apply_phrases(
        &self,
        local: &[LocalPhrase],
        target: &[AccountPhrase],
    ) -> Result<BTreeSet<String>, String> {
        let plan = policy::local_plan(local, target);
        self.local()
            .phrases
            .retain(|phrase| !plan.remove.contains(&phrase.id));
        let mut unheld = BTreeSet::new();
        for text in plan.add {
            // 本机最多存 1000 个 UTF-16 单元，与 client-services 的常用语存储相同。
            if text.encode_utf16().count() > 1000 {
                unheld.insert(policy::text_digest(&text));
            } else {
                self.add_phrase(&text);
            }
        }
        Ok(unheld)
    }
    fn dictionary(&self) -> Result<DictionaryLocal, String> {
        let local = self.local();
        Ok(DictionaryLocal {
            has_words: local.words,
            pending: local.pending,
        })
    }
    fn export_dictionary(&self, destination: &Path) -> Result<(), String> {
        self.local().exported += 1;
        std::fs::write(destination, "local snapshot").map_err(|_| "dictionary_storage".to_owned())
    }
    fn replace_dictionary(
        &self,
        snapshot: &Path,
        account_id: &str,
        revision: i64,
    ) -> Result<(), String> {
        assert!(snapshot.exists());
        self.local()
            .replaced
            .push((account_id.to_owned(), revision));
        Ok(())
    }
    fn merge_dictionary(&self, _: &Path) -> Result<(), String> {
        self.local().merged += 1;
        Ok(())
    }
    fn cancel_dictionary(&self, account_id: &str) -> Result<(), String> {
        self.local().cancelled.push(account_id.to_owned());
        Ok(())
    }
    fn snapshot_revision(&self, snapshot: &Path) -> Result<i64, String> {
        std::fs::read_to_string(snapshot)
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| "dictionary_invalid".to_owned())
    }
}

type Session = BackendAccountSession<FakeApi, SharedStorage>;

struct Fixture {
    _directory: tempfile::TempDir,
    state_directory: PathBuf,
    api: FakeApi,
    storage: SharedStorage,
    session: Session,
    store: SyncStateStore,
    local: FakeLocal,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let state_directory = directory.path().to_path_buf();
        let api = FakeApi::default();
        api.cloud().schema = vec![
            ("input.schema", "string"),
            ("platform.android.theme", "string"),
            (policy::SKINS_KEY, "string"),
        ];
        let storage = SharedStorage(Arc::new(Mutex::new(Some(saved(USER, 'a')))));
        let session = BackendAccountSession::new(api.clone(), storage.clone());
        Self {
            store: SyncStateStore::new(&state_directory),
            _directory: directory,
            state_directory,
            api,
            storage,
            session,
            local: FakeLocal::default(),
        }
    }

    fn enable(&self) {
        let value = set_enabled(&self.session, &self.store, true, NOW).unwrap();
        assert_eq!(value["enabled"], true);
    }

    fn run(
        &self,
        trigger: Trigger,
        choice: Option<Choice>,
        capabilities: Capabilities,
        now: u64,
    ) -> Value {
        run(
            &self.session,
            &self.store,
            &self.local,
            RunRequest {
                trigger,
                choice,
                capabilities,
                settings_locked: false,
                excluded_phrase_texts: Vec::new(),
            },
            now,
        )
        .unwrap()
    }

    fn state(&self) -> SyncState {
        self.store.read().unwrap()
    }
}

fn phrases_only() -> Capabilities {
    Capabilities {
        phrases: true,
        ..Capabilities::default()
    }
}

fn settings_and_phrases() -> Capabilities {
    Capabilities {
        settings: true,
        phrases: true,
        ..Capabilities::default()
    }
}

fn string(value: &str) -> AccountPreferenceValue {
    AccountPreferenceValue::String(value.into())
}

fn cloud_phrases(revision: i64, texts: &[&str]) -> AccountPhrases {
    AccountPhrases {
        revision,
        phrases: texts
            .iter()
            .enumerate()
            .map(|(index, text)| AccountPhrase {
                id: format!("cloud-{index}-{revision}"),
                text: (*text).into(),
                group: String::new(),
                position: index as i64,
            })
            .collect(),
    }
}

fn native() -> IosKeyboardPreferences {
    serde_json::from_value(json!({
        "inputScheme": "shuangpin",
        "traditionalChineseOutput": false,
        "soundEnabled": true,
        "hapticsEnabled": false,
        "hapticStrength": "medium",
        "englishSuggestions": true,
        "candidatePaletteFollowsDesktop": false,
        "inlinePreedit": false,
        "hapticsAvailable": true,
        "dictionaryLearning": true,
        "globalTheme": "shuishan",
        "customKeyboardSkin": null,
    }))
    .unwrap()
}

/// 本机与云端都同步过 `texts`，常用语有基线。
fn synced_phrases(fixture: &Fixture, texts: &[&str]) {
    for text in texts {
        fixture.local.add_phrase(text);
    }
    fixture.enable();
    let result = fixture.run(Trigger::Enable, None, phrases_only(), NOW);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(fixture.api.cloud_texts(), texts);
    fixture.api.calls();
}

#[test]
fn first_sync_uploads_to_an_empty_cloud_and_persists_progress() {
    let fixture = Fixture::new();
    fixture.local.local().settings = [
        ("input.schema".to_owned(), string("quanpin")),
        // 不在服务端字段表里：不上传。
        ("platform.ios.global_theme".to_owned(), string("ink")),
        // 隐私键：不上传。
        (
            "platform.android.incognito".to_owned(),
            AccountPreferenceValue::Boolean(true),
        ),
    ]
    .into();
    fixture.local.add_phrase("早上好");
    fixture.enable();
    let result = fixture.run(Trigger::Enable, None, settings_and_phrases(), NOW);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(result["sections"]["settings"]["outcome"], "synced");
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.revision, 1);
    assert_eq!(
        cloud.settings,
        [("input.schema".to_owned(), string("quanpin"))].into()
    );
    assert_eq!(fixture.api.cloud_texts(), ["早上好"]);
    let state = fixture.state();
    assert_eq!(state.sections.settings.cursor, Some(1));
    assert_eq!(state.sections.phrases.cursor, Some(1));
    assert!(!state.sections.settings.dirty() && !state.sections.phrases.dirty());
    assert!(state.sections.settings.base.is_some());
    assert_eq!(state.last_synced_ms, Some(NOW));
    assert!(state.run.is_none());
    assert_eq!(result["status"]["status_line"], "已同步 · 最近 刚刚");
    // 换一个读的对象（另一个进程）读到的是同一份。
    assert_eq!(
        SyncStateStore::new(&fixture.state_directory)
            .read()
            .unwrap(),
        state
    );

    // 两边都没变：5 分钟内回到前台不跑，满 5 分钟跑一轮也不写。
    fixture.api.calls();
    let throttled = fixture.run(Trigger::Active, None, settings_and_phrases(), NOW + 60_000);
    assert_eq!(throttled["reason"], "throttled");
    let quiet = fixture.run(Trigger::Active, None, settings_and_phrases(), NOW + 300_000);
    assert_eq!(quiet["outcome"], "done");
    assert_eq!(quiet["sections"]["phrases"]["outcome"], "unchanged");
    assert_eq!(fixture.api.calls(), ["schema", "preferences", "phrases"]);
}

#[test]
fn first_sync_with_cloud_data_asks_and_merges_after_the_choice() {
    let fixture = Fixture::new();
    fixture.api.cloud().phrases = cloud_phrases(3, &["云端"]);
    fixture.local.add_phrase("本机");
    fixture.enable();
    let asked = fixture.run(Trigger::Enable, None, phrases_only(), NOW);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "first_run");
    assert!(!fixture.api.calls().contains(&"put_phrases"));
    let state = fixture.state();
    assert_eq!(state.prompted_at_ms, Some(NOW));
    assert_eq!(state.sections.phrases.cursor, None);
    // 取消以后回到前台：5 分钟内不再追着问。
    let quiet = fixture.run(Trigger::Active, None, phrases_only(), NOW + 40_000);
    assert_eq!(quiet["reason"], "awaiting_choice", "{quiet}");

    let merged = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        phrases_only(),
        NOW + 50_000,
    );
    assert_eq!(merged["outcome"], "done", "{merged}");
    assert_eq!(fixture.api.cloud_texts(), ["本机", "云端"]);
    assert_eq!(fixture.local.texts(), ["本机", "云端"]);
    assert_eq!(fixture.state().sections.phrases.cursor, Some(4));
}

#[test]
fn a_conflicting_put_is_refetched_and_merged() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["一"]);
    fixture.local.add_phrase("本机新");
    store_mark(&fixture, Section::Phrases);
    fixture.api.cloud().phrases_written_elsewhere = Some(AccountPhrases {
        revision: 2,
        phrases: cloud_phrases(2, &["一", "云端新"]).phrases,
    });
    let result = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 60_000);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(fixture.api.cloud_texts(), ["一", "本机新", "云端新"]);
    assert_eq!(fixture.local.texts(), ["一", "本机新", "云端新"]);
    assert_eq!(
        fixture.api.calls(),
        ["phrases", "put_phrases", "phrases", "put_phrases"]
    );
    assert_eq!(fixture.state().sections.phrases.cursor, Some(3));
}

fn store_mark(fixture: &Fixture, section: Section) {
    assert_eq!(mark_dirty(&fixture.store, section).unwrap()["marked"], true);
}

#[test]
fn deletions_on_either_side_are_not_brought_back() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["甲", "乙", "丙"]);
    // 本机删了「乙」，没有打标记（按内容也能发现）；另一台设备删了「丙」。
    fixture
        .local
        .local()
        .phrases
        .retain(|phrase| phrase.text != "乙");
    fixture.api.cloud().phrases = cloud_phrases(2, &["甲", "乙"]);
    let result = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 60_000);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(fixture.api.cloud_texts(), ["甲"]);
    assert_eq!(fixture.local.texts(), ["甲"]);
}

#[test]
fn sign_out_clears_progress_cancels_snapshots_and_voids_a_pending_commit() {
    let fixture = Fixture::new();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [("input.schema".to_owned(), string("wubi"))].into(),
    };
    fixture.local.local().defer = Some(native());
    fixture.enable();
    let pending = fixture.run(
        Trigger::Choice,
        Some(Choice::UseCloud),
        Capabilities {
            settings: true,
            ..Capabilities::default()
        },
        NOW,
    );
    assert_eq!(pending["outcome"], "pending_commit", "{pending}");
    let run_id = pending["run_id"].as_str().unwrap().to_owned();
    *fixture.storage.0.lock().unwrap() = None;
    let signed_out = sign_out(&fixture.store, &fixture.local, NOW + 1).unwrap();
    assert_eq!(signed_out["enabled"], false);
    assert_eq!(signed_out["account_id"], Value::Null);
    assert_eq!(signed_out["snapshots_cancelled"], true);
    assert_eq!(fixture.local.local().cancelled, [USER]);
    let state = fixture.state();
    assert!(state.sections.settings.cursor.is_none() && state.run.is_none());
    *fixture.storage.0.lock().unwrap() = Some(saved(USER, 'a'));
    let stale = commit(
        &fixture.session,
        &fixture.store,
        &fixture.local,
        &run_id,
        Some(&native()),
        NOW + 2,
    );
    assert_eq!(stale, Err(STALE_RUN.to_owned()));
    assert!(fixture.local.local().committed.is_empty());
    // 再登录同一个账号：进度从头开始，开关是关的。
    let status = status(&fixture.session, &fixture.store, NOW + 3).unwrap();
    assert_eq!(status["account_id"], USER);
    assert_eq!(status["enabled"], false);
}

#[test]
fn retry_after_backs_off_even_a_manual_run() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["一"]);
    fixture.local.add_phrase("二");
    fixture.api.fail(
        "put_phrases",
        AccountFailure::from_http_response(
            429,
            br#"{"error":{"code":"rate_limit_exceeded"}}"#,
            Some("60"),
        ),
    );
    let limited = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 10_000);
    assert_eq!(limited["outcome"], "done", "{limited}");
    assert_eq!(limited["sections"]["phrases"]["outcome"], "failed");
    let state = fixture.state();
    assert_eq!(state.not_before_ms, Some(NOW + 70_000));
    let failure = state.failure.unwrap();
    assert_eq!(failure.message, "同步太频繁，稍后自动重试");
    assert_eq!(failure.status, Some(429));
    assert_eq!(limited["status"]["status_line"], "同步太频繁，稍后自动重试");
    let blocked = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 20_000);
    assert_eq!(blocked["reason"], "backoff");
    let retried = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 70_000);
    assert_eq!(
        retried["sections"]["phrases"]["outcome"], "synced",
        "{retried}"
    );
    let state = fixture.state();
    assert_eq!(state.failure, None);
    assert_eq!(state.not_before_ms, None);
    assert_eq!(fixture.api.cloud_texts(), ["一", "二"]);
}

#[test]
fn host_writes_wait_for_commit_and_a_second_run_waits_for_the_lease() {
    let fixture = Fixture::new();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            // 字段表外的键既不写回本机也不上传。
            ("platform.ios.custom_theme_base".to_owned(), string("ink")),
        ]
        .into(),
    };
    fixture.local.local().defer = Some(native());
    fixture.enable();
    let capabilities = Capabilities {
        settings: true,
        ..Capabilities::default()
    };
    let pending = fixture.run(Trigger::Choice, Some(Choice::UseCloud), capabilities, NOW);
    assert_eq!(pending["outcome"], "pending_commit", "{pending}");
    assert_eq!(
        pending["host_writes"]["ios_native"]["inputScheme"],
        "shuangpin"
    );
    let applied = fixture.local.local().applied.clone();
    assert_eq!(
        applied[0].settings,
        [("input.schema".to_owned(), string("wubi"))].into()
    );
    // 提交之前游标不动，另一轮等租约。
    assert_eq!(fixture.state().sections.settings.cursor, None);
    let busy = fixture.run(Trigger::Manual, None, capabilities, NOW + 1_000);
    assert_eq!(busy["reason"], "busy");
    let run_id = pending["run_id"].as_str().unwrap();
    let committed = commit(
        &fixture.session,
        &fixture.store,
        &fixture.local,
        run_id,
        Some(&native()),
        NOW + 2_000,
    )
    .unwrap();
    assert_eq!(committed["sections"]["settings"]["cursor"], 2);
    assert_eq!(fixture.local.local().committed.len(), 1);
    // 同一个 run_id 不能再提交一次。
    assert_eq!(
        commit(
            &fixture.session,
            &fixture.store,
            &fixture.local,
            run_id,
            Some(&native()),
            NOW + 3_000,
        ),
        Err(STALE_RUN.to_owned())
    );
}

#[test]
fn a_commit_after_another_write_is_stale_and_a_failed_commit_releases_the_lease() {
    let fixture = Fixture::new();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [("input.schema".to_owned(), string("wubi"))].into(),
    };
    fixture.local.local().defer = Some(native());
    fixture.enable();
    let capabilities = Capabilities {
        settings: true,
        ..Capabilities::default()
    };
    let pending = fixture.run(Trigger::Choice, Some(Choice::UseCloud), capabilities, NOW);
    let run_id = pending["run_id"].as_str().unwrap().to_owned();
    // 期间用户关掉了同步：这一轮不能再推进。
    set_enabled(&fixture.session, &fixture.store, false, NOW + 1).unwrap();
    let stale = commit(
        &fixture.session,
        &fixture.store,
        &fixture.local,
        &run_id,
        Some(&native()),
        NOW + 2,
    );
    assert_eq!(stale, Err(STALE_RUN.to_owned()));
    assert_eq!(fixture.state().sections.settings.cursor, None);

    set_enabled(&fixture.session, &fixture.store, true, NOW + 3).unwrap();
    let pending = fixture.run(
        Trigger::Choice,
        Some(Choice::UseCloud),
        capabilities,
        NOW + 4,
    );
    let run_id = pending["run_id"].as_str().unwrap().to_owned();
    // 缺 saved_native 是请求不合法，租约留着，宿主可以重试。
    assert_eq!(
        commit(
            &fixture.session,
            &fixture.store,
            &fixture.local,
            &run_id,
            None,
            NOW + 5
        ),
        Err(INVALID.to_owned())
    );
    assert!(fixture.state().run.is_some());
    fixture.local.local().commit_failure = Some("conflict".into());
    assert_eq!(
        commit(
            &fixture.session,
            &fixture.store,
            &fixture.local,
            &run_id,
            Some(&native()),
            NOW + 6,
        ),
        Err("conflict".to_owned())
    );
    let state = fixture.state();
    assert!(state.run.is_none(), "a failed commit gives the lease back");
    assert_eq!(state.sections.settings.cursor, None);
}

#[test]
fn a_choice_made_while_another_round_runs_is_kept_for_the_next_one() {
    let fixture = Fixture::new();
    fixture.api.cloud().phrases = cloud_phrases(3, &["云端"]);
    fixture.local.add_phrase("本机");
    fixture.enable();
    fixture
        .store
        .update(|state| {
            state.begin_run(USER, NOW, None);
        })
        .unwrap();
    let busy = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        phrases_only(),
        NOW + 1_000,
    );
    assert_eq!(busy["reason"], "busy");
    assert_eq!(fixture.state().pending_choice, Some(Choice::Merge));
    // 那一轮的租约过期以后，回到前台的一轮用记下的选择，不再问。
    let resumed = fixture.run(
        Trigger::Active,
        None,
        phrases_only(),
        NOW + policy::LEASE_MS,
    );
    assert_eq!(resumed["outcome"], "done", "{resumed}");
    assert_eq!(fixture.api.cloud_texts(), ["本机", "云端"]);
    assert_eq!(fixture.state().pending_choice, None);
}

#[test]
fn a_merge_over_the_phrase_limit_fails_without_writing_either_side() {
    let fixture = Fixture::new();
    let many: Vec<String> = (0..300).map(|index| format!("云端 {index}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    fixture.api.cloud().phrases = cloud_phrases(5, &many);
    for index in 0..201 {
        fixture.local.add_phrase(&format!("本机 {index}"));
    }
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.enable();
    let result = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        settings_and_phrases(),
        NOW,
    );
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(
        result["sections"]["phrases"]["reason"],
        "phrases_over_limit"
    );
    assert_eq!(result["sections"]["settings"]["outcome"], "synced");
    assert_eq!(fixture.api.cloud().phrases.revision, 5);
    assert_eq!(fixture.local.texts().len(), 201);
    assert_eq!(
        result["status"]["status_line"],
        policy::local_message("phrases_over_limit")
    );
    assert_eq!(fixture.state().sections.phrases.cursor, None);
}

#[test]
fn merged_phrases_with_colliding_ids_get_new_ids() {
    let fixture = Fixture::new();
    // 本机的 id 恰好与云端另一条的 id 相同。
    fixture.api.cloud().phrases = AccountPhrases {
        revision: 2,
        phrases: vec![AccountPhrase {
            id: "same".into(),
            text: "云端".into(),
            group: String::new(),
            position: 0,
        }],
    };
    fixture.local.local().phrases = vec![LocalPhrase {
        id: "same".into(),
        text: "本机".into(),
    }];
    fixture.enable();
    let result = fixture.run(Trigger::Choice, Some(Choice::Merge), phrases_only(), NOW);
    assert_eq!(result["outcome"], "done", "{result}");
    let cloud = fixture.api.cloud().phrases.clone();
    assert_eq!(cloud.phrases.len(), 2);
    assert_ne!(cloud.phrases[0].id, cloud.phrases[1].id);
}

#[test]
fn use_cloud_on_a_cloud_that_was_never_uploaded_keeps_local_data() {
    let fixture = Fixture::new();
    fixture.api.cloud().phrases = cloud_phrases(0, &["云端"]);
    fixture.local.add_phrase("本机");
    fixture.enable();
    let result = fixture.run(Trigger::Choice, Some(Choice::UseCloud), phrases_only(), NOW);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(fixture.local.texts(), ["本机", "云端"]);
    assert_eq!(fixture.api.cloud_texts(), ["本机", "云端"]);
}

#[test]
fn a_cleared_cloud_asks_instead_of_deleting_local_data() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["甲", "乙"]);
    // 「删除云端数据」：常用语清空，修订号加一。
    fixture.api.cloud().phrases = AccountPhrases {
        revision: 2,
        phrases: Vec::new(),
    };
    let asked = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 60_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert_eq!(fixture.local.texts(), ["甲", "乙"]);
    let state = fixture.state();
    assert!(state.sections.phrases.cleared);
    assert!(state.sections.phrases.base.is_none());
    assert_eq!(
        asked["status"]["status_line"],
        "云端数据已被清空，请选择合并或使用云端"
    );
    // 选了「使用云端」：云端是空的，按合并处理，本机的重新上传。
    let chosen = fixture.run(
        Trigger::Choice,
        Some(Choice::UseCloud),
        phrases_only(),
        NOW + 70_000,
    );
    assert_eq!(chosen["outcome"], "done", "{chosen}");
    assert_eq!(fixture.api.cloud_texts(), ["甲", "乙"]);
    assert!(!fixture.state().sections.phrases.cleared);
}

#[test]
fn an_account_switch_during_the_round_voids_it() {
    let fixture = Fixture::new();
    fixture.local.add_phrase("本机");
    fixture.enable();
    fixture.api.cloud().switch_account_on_phrases = Some(fixture.storage.clone());
    let result = fixture.run(Trigger::Enable, None, phrases_only(), NOW);
    assert_eq!(result["outcome"], "skipped", "{result}");
    assert_eq!(result["reason"], STALE_RUN);
    assert!(fixture.api.cloud().phrases.phrases.is_empty());
    let state = fixture.state();
    assert!(state.run.is_none());
    assert_eq!(state.sections.phrases.cursor, None);
}

#[test]
fn a_skin_library_is_trimmed_to_the_budget_and_merged_with_the_cloud() {
    let fixture = Fixture::new();
    let design = |id: &str, updated_at: u64, size: usize| json!({ "id": id, "name": id, "design": { "pad": "x".repeat(size) }, "updated_at": updated_at });
    fixture.local.local().skins =
        Some(json!([design("local", 2, 500_000), design("older", 1, 400_000)]).to_string());
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.enable();
    let capabilities = Capabilities {
        settings: true,
        skins_library: true,
        ..Capabilities::default()
    };
    let result = fixture.run(Trigger::Enable, None, capabilities, NOW);
    assert_eq!(result["outcome"], "done", "{result}");
    let cloud = fixture.api.cloud().preferences.clone();
    let Some(AccountPreferenceValue::String(library)) = cloud.settings.get(policy::SKINS_KEY)
    else {
        panic!("the library is uploaded: {cloud:?}");
    };
    let items: Vec<Value> = serde_json::from_str(library).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "local");
    let state = fixture.state();
    assert!(state.notices.skins_trimmed);
    assert_eq!(state.sections.skins.cursor, Some(1));
    assert!(!state.sections.skins.dirty());
    assert_eq!(
        result["status"]["status_line"],
        "自定义皮肤太多，只同步了最近的设计"
    );
}

#[test]
fn dictionary_uploads_waits_for_the_queue_downloads_and_detects_a_cleared_cloud() {
    let fixture = Fixture::new();
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    fixture.local.local().words = true;
    fixture.enable();
    // 云端没有词：直接上传本机快照。
    let uploaded = fixture.run(Trigger::Enable, None, dictionary, NOW);
    assert_eq!(
        uploaded["sections"]["dictionary"]["outcome"], "synced",
        "{uploaded}"
    );
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);
    assert_eq!(fixture.state().sections.dictionary.cursor, Some(1));
    assert!(!fixture.state().sections.dictionary.dirty());
    // 导入队列里还有没应用的词：推迟上传，标记留着。
    store_mark(&fixture, Section::Dictionary);
    fixture.local.local().pending = 2;
    let postponed = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(
        postponed["sections"]["dictionary"]["reason"],
        "dictionary_pending"
    );
    assert!(fixture.state().sections.dictionary.dirty());
    // 本机没变、云端变了：下载并交给激活队列，下载以后标记照旧留着（与 Android 相同）。
    fixture.local.local().pending = 0;
    fixture
        .store
        .update(|state| {
            let section = &mut state.sections.dictionary;
            section.clean = section.generation;
        })
        .unwrap();
    fixture.api.cloud().dictionary_revision = 5;
    let downloaded = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
    assert_eq!(
        downloaded["sections"]["dictionary"]["outcome"], "synced",
        "{downloaded}"
    );
    assert_eq!(fixture.local.local().replaced, [(USER.to_owned(), 5)]);
    assert_eq!(fixture.state().sections.dictionary.cursor, Some(5));
    // 「删除云端数据」：第一条新改动是 reset，云端词库为空。
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 6;
        cloud.dictionary_reset = true;
        cloud.dictionary_items = 0;
    }
    let asked = fixture.run(Trigger::Manual, None, dictionary, NOW + 180_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    // 别的设备整份上传快照也写 reset，但云端不空：照常下载。
    fixture
        .store
        .update(|state| state.sections.dictionary.cleared = false)
        .unwrap();
    fixture.api.cloud().dictionary_items = 3;
    let replaced = fixture.run(Trigger::Manual, None, dictionary, NOW + 240_000);
    assert_eq!(replaced["outcome"], "done", "{replaced}");
    assert_eq!(fixture.local.local().replaced.len(), 2);
}

#[test]
fn dictionary_merge_imports_cloud_words_and_marks_for_upload() {
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let merged = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(merged["outcome"], "done", "{merged}");
    assert_eq!(fixture.local.local().merged, 1);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert!(
        state.sections.dictionary.dirty(),
        "merged words are uploaded next round"
    );
}

#[test]
fn legacy_state_is_imported_once() {
    let fixture = Fixture::new();
    let ios = json!({
        "version": 1,
        "enabled": true,
        "accountID": USER,
        "cursors": { "settings": "4", "phrases": "2" },
        "generations": { "phrases": 1 },
        "cleans": {},
    });
    let imported = import_legacy(&fixture.store, Some(ios.clone()), None, NOW).unwrap();
    assert_eq!(imported["imported"], true);
    assert_eq!(imported["status"]["sections"]["settings"]["cursor"], 4);
    assert_eq!(imported["status"]["sections"]["phrases"]["dirty"], true);
    let again = import_legacy(&fixture.store, Some(ios), None, NOW).unwrap();
    assert_eq!(again["imported"], false);
    assert_eq!(
        import_legacy(&fixture.store, None, None, NOW),
        Err(INVALID.to_owned())
    );
    // 导入以后照常对齐账号：会话还是这个账号，进度留着。
    let status = status(&fixture.session, &fixture.store, NOW).unwrap();
    assert_eq!(status["enabled"], true);
    assert_eq!(status["sections"]["settings"]["cursor"], 4);
}

#[test]
fn enabling_needs_a_signed_in_account_and_marking_needs_sync_on() {
    let fixture = Fixture::new();
    assert_eq!(
        mark_dirty(&fixture.store, Section::Settings).unwrap()["marked"],
        false
    );
    *fixture.storage.0.lock().unwrap() = None;
    assert_eq!(
        set_enabled(&fixture.session, &fixture.store, true, NOW),
        Err(SIGNED_OUT.to_owned())
    );
    let skipped = fixture.run(Trigger::Manual, None, phrases_only(), NOW);
    assert_eq!(skipped["reason"], "signed_out");
}

// ---- 本机读写的默认实现 ----

fn schema(fields: &[(&str, &str)]) -> AccountPreferenceSchema {
    AccountPreferenceSchema {
        fields: fields
            .iter()
            .map(|(key, kind)| {
                (
                    (*key).to_owned(),
                    AccountPreferenceField {
                        value_type: (*kind).to_owned(),
                    },
                )
            })
            .collect(),
        maximum_bytes: 1_048_576,
        update_mode: "replace".into(),
        revision_required: true,
    }
}

fn host_local(directory: &Path, inputs: HostInputs) -> HostLocal {
    HostLocal {
        preferences: directory.to_path_buf(),
        options: None,
        snapshot_queue: None,
        inputs,
    }
}

#[test]
fn the_android_mapping_exports_and_applies_through_shared_preferences() {
    use msime_client_services::account::settings_sync::ANDROID_FEEDBACK_KEYS;
    let directory = tempfile::tempdir().unwrap();
    let feedback = HostKeyboardFeedback {
        sound_enabled: true,
        haptics_enabled: true,
        haptic_strength: "medium".into(),
    };
    let local = host_local(
        directory.path(),
        HostInputs {
            feedback: Some(feedback.clone()),
            custom_keyboard_skins: Some("[]".into()),
            ..HostInputs::default()
        },
    );
    let exported = local.export_settings().unwrap();
    assert_eq!(exported.settings["input.schema"], string("quanpin"));
    assert_eq!(
        exported.settings[ANDROID_FEEDBACK_KEYS[0]],
        AccountPreferenceValue::Boolean(true)
    );
    assert_eq!(exported.skins.as_deref(), Some("[]"));
    let fields = schema(&[
        ("input.schema", "string"),
        (ANDROID_FEEDBACK_KEYS[0], "boolean"),
        (ANDROID_FEEDBACK_KEYS[1], "boolean"),
        (ANDROID_FEEDBACK_KEYS[2], "string"),
    ]);
    let document = AccountPreferences {
        revision: 3,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            (
                ANDROID_FEEDBACK_KEYS[0].to_owned(),
                AccountPreferenceValue::Boolean(false),
            ),
        ]
        .into(),
    };
    let applied = local.apply_settings(&document, &fields).unwrap();
    assert!(!applied.deferred);
    // 按键反馈是宿主的值：交给宿主写；共享偏好已经存好。
    assert_eq!(
        applied.writes.feedback,
        Some(HostKeyboardFeedback {
            sound_enabled: false,
            ..feedback
        })
    );
    assert_eq!(
        local.export_settings().unwrap().settings["input.schema"],
        string("wubi")
    );
}

#[test]
fn the_ios_mapping_defers_shared_preferences_until_the_keyboard_settings_are_saved() {
    let directory = tempfile::tempdir().unwrap();
    let local = host_local(
        directory.path(),
        HostInputs {
            ios_native: Some(native()),
            ..HostInputs::default()
        },
    );
    let exported = local.export_settings().unwrap();
    assert_eq!(exported.settings["input.schema"], string("shuangpin"));
    assert!(exported.skins.is_none());
    let fields = schema(&[("input.schema", "string"), ("input.wubi_schema", "string")]);
    // 键盘设置不用变：共享偏好现在就写。
    let same = AccountPreferences {
        revision: 2,
        settings: [("input.schema".to_owned(), string("shuangpin"))].into(),
    };
    let applied = local.apply_settings(&same, &fields).unwrap();
    assert!(!applied.deferred && applied.writes.is_empty());
    // 要换方案：先交给宿主写键盘设置，提交时再按实际保存的值写共享偏好。
    let wubi = AccountPreferences {
        revision: 3,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            ("input.wubi_schema".to_owned(), string("wubi98")),
        ]
        .into(),
    };
    let applied = local.apply_settings(&wubi, &fields).unwrap();
    assert!(applied.deferred);
    let requested = applied.writes.ios_native.unwrap();
    assert_eq!(
        serde_json::to_value(&requested).unwrap()["inputScheme"],
        "wubi"
    );
    local.commit_settings(&wubi, &requested).unwrap();
    let saved = PreferencesStore::new(directory.path()).load().unwrap();
    let saved = serde_json::to_value(saved).unwrap();
    assert_eq!(saved["preferences"]["scheme"], "wubi");
    assert_eq!(saved["preferences"]["wubi_profile"], "wubi98");
}

#[test]
fn local_phrases_are_aligned_by_text_and_long_ones_are_kept_in_the_cloud() {
    let directory = tempfile::tempdir().unwrap();
    let store = CommonPhrasesStore::new(directory.path());
    store.add("保留").unwrap();
    store.add("删掉").unwrap();
    let local = host_local(directory.path(), HostInputs::default());
    let mine = local.phrases().unwrap();
    assert_eq!(mine.len(), 2);
    let long = "长".repeat(1500);
    let target = [
        AccountPhrase {
            id: "a".into(),
            text: "保留".into(),
            group: String::new(),
            position: 0,
        },
        AccountPhrase {
            id: "b".into(),
            text: "新增".into(),
            group: String::new(),
            position: 1,
        },
        AccountPhrase {
            id: "c".into(),
            text: long.clone(),
            group: String::new(),
            position: 2,
        },
    ];
    let unheld = local.apply_phrases(&mine, &target).unwrap();
    assert_eq!(unheld, [policy::text_digest(&long)].into());
    let texts: Vec<String> = local
        .phrases()
        .unwrap()
        .into_iter()
        .map(|phrase| phrase.text)
        .collect();
    assert_eq!(texts, ["保留", "新增"]);
}

#[test]
fn snapshot_words_drop_deleted_entries_and_the_queue_needs_a_published_version() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("snapshot.ndjson");
    let lines = [
        json!({"type": "header", "format": "msime-dictionary-snapshot", "version": 1, "revision": 4}),
        json!({"type": "entry", "data": {"id": "a", "kind": "pinyin", "code": "ni", "word": "你", "weight": 3, "revision": 1, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "entry", "data": {"id": "b", "kind": "quick", "code": "dz", "word": "地址", "weight": 1, "revision": 1, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "overlay", "deleted": true, "data": {"id": "b", "kind": "quick", "code": "dz", "word": "地址", "weight": 0, "revision": 2, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "footer", "sha256": "0".repeat(64)}),
    ];
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(&file, text).unwrap();
    let words = snapshot_words(&file).unwrap();
    assert_eq!(
        words,
        [policy::Word {
            kind: "pinyin".into(),
            key: "ni".into(),
            value: "你".into(),
            weight: 3,
        }]
    );
    // 没有队列目录时退出登录不需要取消什么。
    let local = host_local(directory.path(), HostInputs::default());
    assert_eq!(local.cancel_dictionary(USER), Ok(()));
    let queued = HostLocal {
        snapshot_queue: Some(directory.path().join("queue")),
        ..host_local(directory.path(), HostInputs::default())
    };
    assert_eq!(queued.cancel_dictionary(USER), Ok(()));
    assert_eq!(
        queued.replace_dictionary(&file, USER, 4),
        Err("dictionary_unpublished".to_owned())
    );
    // 下载的快照不合规：不交给队列。
    assert_eq!(
        queued.snapshot_revision(&file),
        Err("dictionary_invalid".to_owned())
    );
}
