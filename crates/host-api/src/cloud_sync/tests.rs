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
    /// 云端词库现在的条数（`data_summary` 报的）；默认有词，模拟云端被清空时设成 0。
    dictionary_items: i64,
    /// 下一次 PUT 常用语之前，另一台设备先写了这一份：这次 PUT 以 409 失败。
    phrases_written_elsewhere: Option<AccountPhrases>,
    failures: HashMap<&'static str, VecDeque<AccountFailure>>,
    calls: Vec<&'static str>,
    snapshot_uploads: usize,
    /// 每次上传的快照原文，按上传顺序。
    uploaded_snapshots: Vec<String>,
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
            dictionary_items: 1,
            phrases_written_elsewhere: None,
            failures: HashMap::new(),
            calls: Vec::new(),
            snapshot_uploads: 0,
            uploaded_snapshots: Vec::new(),
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
        // 假快照是「修订号 词数」：词数跟 `data_summary` 报的条数一致。
        let cloud = self.cloud();
        std::fs::write(
            destination,
            format!("{} {}", cloud.dictionary_revision, cloud.dictionary_items),
        )
        .unwrap();
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
        cloud
            .uploaded_snapshots
            .push(std::fs::read_to_string(snapshot).unwrap());
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
    /// 本机设置的修订号，像共享偏好那样每写一次加一。
    settings_revision: u64,
    /// 下一次写回设置之前，另一个进程里用户改了这一项（读本机和写回之间的修改）。
    edit_before_apply: Option<(String, AccountPreferenceValue)>,
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
    /// 激活队列里有等键盘激活的快照；交给激活队列时置上，像真的队列一样。
    snapshot_pending: bool,
    /// 键盘激活了的快照请求；交给激活队列时按顺序编号。
    activated: Vec<String>,
    exported: usize,
    replaced: Vec<(String, i64)>,
    merged: usize,
    /// 合并时返回的错误，比如云端的词这一轮没排完。
    merge_failure: Option<String>,
    /// 合并没做完时这一轮有没有往导入队列里排了新的词。
    merge_queued: bool,
    /// 合并没做完时云端快照里已经在本机的词数。
    merge_present: Option<usize>,
    /// 交给激活队列时返回的错误，比如键盘还没发布过本机词库版本。
    replace_failure: Option<String>,
    cancelled: Vec<String>,
}

#[derive(Default)]
struct FakeLocal(Mutex<Local>);

impl FakeLocal {
    fn local(&self) -> std::sync::MutexGuard<'_, Local> {
        self.0.lock().unwrap()
    }

    /// 键盘处理了最近交给激活队列的快照：`applied` 为假时是没激活就结束了（冲突、失败）。
    fn finish_activation(&self, applied: bool) {
        let mut local = self.local();
        local.snapshot_pending = false;
        if applied {
            let id = format!("snapshot-{}", local.replaced.len());
            local.activated.push(id);
        }
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
            revision: local.settings_revision,
        })
    }
    fn apply_settings(
        &self,
        document: &AccountPreferences,
        _: &AccountPreferenceSchema,
        read_revision: u64,
    ) -> Result<SettingsApplied, String> {
        let mut local = self.local();
        if let Some((key, value)) = local.edit_before_apply.take() {
            local.settings.insert(key, value);
            local.settings_revision += 1;
        }
        if local.settings_revision != read_revision {
            return Err(SETTINGS_CHANGED.to_owned());
        }
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
        local.settings_revision += 1;
        Ok(SettingsApplied {
            writes: HostWrites::default(),
            deferred: false,
        })
    }
    fn commit_settings(
        &self,
        document: &AccountPreferences,
        saved_native: &IosKeyboardPreferences,
        read_revision: u64,
    ) -> Result<(), String> {
        let mut local = self.local();
        if let Some(code) = local.commit_failure.take() {
            return Err(code);
        }
        if local.settings_revision != read_revision {
            return Err("conflict".to_owned());
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
    fn dictionary(&self, awaited: Option<&str>) -> Result<DictionaryLocal, String> {
        let local = self.local();
        let activation = match awaited {
            _ if local.snapshot_pending => SnapshotActivation::Waiting,
            Some(id) if !local.activated.iter().any(|activated| activated == id) => {
                SnapshotActivation::Lost
            }
            _ => SnapshotActivation::Idle,
        };
        Ok(DictionaryLocal {
            has_words: local.words,
            pending: local.pending,
            activation,
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
    ) -> Result<String, String> {
        assert!(snapshot.exists());
        let mut local = self.local();
        if let Some(reason) = local.replace_failure.clone() {
            return Err(reason);
        }
        local.replaced.push((account_id.to_owned(), revision));
        local.snapshot_pending = true;
        Ok(format!("snapshot-{}", local.replaced.len()))
    }
    fn merge_dictionary(&self, _: &Path) -> Result<(), MergeStop> {
        let mut local = self.local();
        local.merged += 1;
        match local.merge_failure.clone() {
            None => Ok(()),
            Some(reason) => Err(MergeStop {
                reason,
                queued: local.merge_queued,
                present: local.merge_present,
            }),
        }
    }
    fn cancel_dictionary(&self, account_id: &str) -> Result<(), String> {
        self.local().cancelled.push(account_id.to_owned());
        Ok(())
    }
    fn snapshot_revision(&self, snapshot: &Path) -> Result<i64, String> {
        std::fs::read_to_string(snapshot)
            .ok()
            .and_then(|text| text.split(' ').next()?.parse().ok())
            .ok_or_else(|| "dictionary_invalid".to_owned())
    }
    fn snapshot_has_words(&self, snapshot: &Path) -> Result<bool, String> {
        std::fs::read_to_string(snapshot)
            .ok()
            .and_then(|text| text.split(' ').nth(1)?.parse::<i64>().ok())
            .map(|items| items > 0)
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

    /// 登录同一账号的另一台设备：共用这个云端，同步状态和本机各是各的。
    fn another_device(&self) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let state_directory = directory.path().to_path_buf();
        let storage = SharedStorage(Arc::new(Mutex::new(Some(saved(USER, 'a')))));
        let session = BackendAccountSession::new(self.api.clone(), storage.clone());
        Self {
            store: SyncStateStore::new(&state_directory),
            _directory: directory,
            state_directory,
            api: self.api.clone(),
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

#[test]
fn a_cloud_cleared_after_the_read_asks_instead_of_deleting_local_phrases() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["甲", "乙"]);
    fixture.local.add_phrase("丙");
    store_mark(&fixture, Section::Phrases);
    // 这一轮读完常用语、写之前，另一台设备「删除云端数据」。
    fixture.api.cloud().phrases_written_elsewhere = Some(AccountPhrases {
        revision: 2,
        phrases: Vec::new(),
    });
    let asked = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 60_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert_eq!(fixture.local.texts(), ["甲", "乙", "丙"]);
    assert!(fixture.api.cloud_texts().is_empty());
    assert_eq!(fixture.api.calls(), ["phrases", "put_phrases", "phrases"]);
    let state = fixture.state();
    assert!(state.sections.phrases.cleared);
    assert!(state.sections.phrases.base.is_none());
    // 选了合并：没有旧基线，本机的全部传回云端。
    let merged = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        phrases_only(),
        NOW + 70_000,
    );
    assert_eq!(merged["outcome"], "done", "{merged}");
    assert_eq!(fixture.api.cloud_texts(), ["甲", "乙", "丙"]);
    assert_eq!(fixture.local.texts(), ["甲", "乙", "丙"]);
    assert!(!fixture.state().sections.phrases.cleared);
}

#[test]
fn a_cloud_cleared_after_another_section_wrote_keeps_its_progress_and_asks_next_round() {
    let fixture = Fixture::new();
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.local.add_phrase("甲");
    fixture.enable();
    let synced = fixture.run(Trigger::Enable, None, settings_and_phrases(), NOW);
    assert_eq!(synced["outcome"], "done", "{synced}");
    fixture.local.local().settings = [("input.schema".to_owned(), string("wubi"))].into();
    fixture.local.add_phrase("乙");
    store_mark(&fixture, Section::Settings);
    store_mark(&fixture, Section::Phrases);
    // 设置先写上了云端；常用语写的时候 409，重读到的是被别的设备清空的云端。
    fixture.api.cloud().phrases_written_elsewhere = Some(AccountPhrases {
        revision: 2,
        phrases: Vec::new(),
    });
    let result = fixture.run(Trigger::Manual, None, settings_and_phrases(), NOW + 60_000);
    assert_eq!(result["outcome"], "done", "{result}");
    assert_eq!(result["sections"]["settings"]["outcome"], "synced");
    assert_eq!(result["sections"]["phrases"]["outcome"], "needs_choice");
    assert_eq!(result["sections"]["phrases"]["reason"], "cloud_cleared");
    assert_eq!(
        fixture.api.cloud().preferences.settings,
        [("input.schema".to_owned(), string("wubi"))].into()
    );
    assert_eq!(fixture.local.texts(), ["甲", "乙"]);
    assert!(fixture.api.cloud_texts().is_empty());
    let state = fixture.state();
    assert_eq!(state.sections.settings.cursor, Some(2));
    assert!(!state.sections.settings.dirty());
    assert!(state.sections.phrases.cleared);
    assert!(state.sections.phrases.base.is_none());
    assert_eq!(
        result["status"]["status_line"],
        "云端数据已被清空，请选择合并、使用云端或关闭同步"
    );
    // 下一轮读完云端就问，什么都不写。
    let asked = fixture.run(Trigger::Manual, None, settings_and_phrases(), NOW + 120_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert_eq!(fixture.local.texts(), ["甲", "乙"]);
}

#[test]
fn a_setting_changed_elsewhere_during_the_round_is_merged_next_round_rather_than_overwritten() {
    let capabilities = Capabilities {
        settings: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.enable();
    let first = fixture.run(Trigger::Enable, None, capabilities, NOW);
    assert_eq!(
        first["sections"]["settings"]["outcome"], "synced",
        "{first}"
    );
    let before = fixture.state().sections.settings.clone();
    assert_eq!(before.cursor, Some(1));
    // 别的设备改了主题；这一轮下载期间，另一个进程里用户把方案改成了五笔。
    {
        let mut cloud = fixture.api.cloud();
        cloud.preferences.revision = 2;
        cloud
            .preferences
            .settings
            .insert("platform.android.theme".to_owned(), string("ink"));
    }
    fixture.local.local().edit_before_apply = Some(("input.schema".to_owned(), string("wubi")));
    let raced = fixture.run(Trigger::Manual, None, capabilities, NOW + 60_000);
    assert_eq!(
        raced["sections"]["settings"]["reason"], SETTINGS_CHANGED,
        "{raced}"
    );
    assert_eq!(
        fixture.local.local().settings.get("input.schema"),
        Some(&string("wubi"))
    );
    let state = fixture.state();
    assert_eq!(state.sections.settings.cursor, before.cursor);
    assert_eq!(state.sections.settings.base, before.base);
    assert!(state.sections.settings.dirty());
    assert_eq!(state.failure, None);
    // 下一轮按三方合并：用户的五笔和云端的主题都留下。
    let merged = fixture.run(Trigger::Manual, None, capabilities, NOW + 120_000);
    assert_eq!(
        merged["sections"]["settings"]["outcome"], "synced",
        "{merged}"
    );
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.settings.get("input.schema"), Some(&string("wubi")));
    assert_eq!(
        cloud.settings.get("platform.android.theme"),
        Some(&string("ink"))
    );
    let local = fixture.local.local().settings.clone();
    assert_eq!(local.get("input.schema"), Some(&string("wubi")));
    assert_eq!(local.get("platform.android.theme"), Some(&string("ink")));
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
fn a_huge_retry_after_blocks_a_manual_run_for_at_most_a_day() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["一"]);
    fixture.local.add_phrase("二");
    fixture.api.fail(
        "put_phrases",
        AccountFailure::from_http_response(
            429,
            br#"{"error":{"code":"rate_limit_exceeded"}}"#,
            Some("31536000"),
        ),
    );
    let limited = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 10_000);
    assert_eq!(
        limited["sections"]["phrases"]["outcome"], "failed",
        "{limited}"
    );
    let cap = NOW + 10_000 + policy::MAX_BACKOFF_MS;
    assert_eq!(fixture.state().not_before_ms, Some(cap));
    assert_eq!(limited["status"]["not_before_ms"], cap);
    let blocked = fixture.run(Trigger::Manual, None, phrases_only(), cap - 1);
    assert_eq!(blocked["reason"], "backoff");
    let retried = fixture.run(Trigger::Manual, None, phrases_only(), cap);
    assert_eq!(
        retried["sections"]["phrases"]["outcome"], "synced",
        "{retried}"
    );
    assert_eq!(fixture.state().not_before_ms, None);
}

#[test]
fn a_backoff_recorded_before_the_clock_went_back_does_not_block() {
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["一"]);
    fixture.local.add_phrase("二");
    // 时钟比现在快一年时收到的 429：记下的退避离纠正后的时间一年多。
    fixture
        .store
        .update(|state| state.not_before_ms = Some(NOW + 365 * 24 * 3_600_000))
        .unwrap();
    let status = status(&fixture.session, &fixture.store, NOW + 10_000).unwrap();
    assert_eq!(status["not_before_ms"], Value::Null);
    let run = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 10_000);
    assert_eq!(run["sections"]["phrases"]["outcome"], "synced", "{run}");
    assert_eq!(fixture.state().not_before_ms, None);
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
        "云端数据已被清空，请选择合并、使用云端或关闭同步"
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
fn a_cloud_cleared_in_a_round_that_already_has_a_choice_does_not_use_the_old_base() {
    for choice in [Choice::Merge, Choice::UseCloud] {
        let fixture = Fixture::new();
        fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
        fixture.local.add_phrase("甲");
        fixture.local.add_phrase("乙");
        fixture.enable();
        let synced = fixture.run(Trigger::Enable, None, settings_and_phrases(), NOW);
        assert_eq!(synced["outcome"], "done", "{synced}");
        assert!(fixture.state().sections.phrases.base.is_some());
        // 另一台设备「删除云端数据」，而这一轮已经带着（旧界面发来的）选择。
        {
            let mut cloud = fixture.api.cloud();
            cloud.preferences = AccountPreferences {
                revision: 2,
                settings: BTreeMap::new(),
            };
            cloud.phrases = AccountPhrases {
                revision: 2,
                phrases: Vec::new(),
            };
        }
        let result = fixture.run(
            Trigger::Choice,
            Some(choice),
            settings_and_phrases(),
            NOW + 60_000,
        );
        assert_eq!(result["outcome"], "done", "{choice:?} {result}");
        assert_eq!(fixture.local.texts(), ["甲", "乙"], "{choice:?}");
        assert_eq!(fixture.api.cloud_texts(), ["甲", "乙"], "{choice:?}");
        assert_eq!(
            fixture.local.local().settings,
            [("input.schema".to_owned(), string("quanpin"))].into(),
            "{choice:?}"
        );
        assert_eq!(
            fixture.api.cloud().preferences.settings,
            [("input.schema".to_owned(), string("quanpin"))].into(),
            "{choice:?}"
        );
        let state = fixture.state();
        assert!(!state.sections.phrases.cleared && !state.sections.settings.cleared);
    }
}

#[test]
fn a_host_without_the_skin_library_keeps_the_cloud_library() {
    let fixture = Fixture::new();
    let library =
        json!([{ "id": "android", "name": "android", "design": {}, "updated_at": 1 }]).to_string();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            (policy::SKINS_KEY.to_owned(), string(&library)),
        ]
        .into(),
    };
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.enable();
    let merged = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        settings_and_phrases(),
        NOW,
    );
    assert_eq!(merged["outcome"], "done", "{merged}");
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.revision, 3);
    assert_eq!(
        cloud.settings.get(policy::SKINS_KEY),
        Some(&string(&library))
    );
    assert_eq!(cloud.settings.get("input.schema"), Some(&string("quanpin")));
    // 皮肤库不写回不同步它的本机。
    assert!(!fixture
        .local
        .local()
        .settings
        .contains_key(policy::SKINS_KEY));
    // 之后本机改了设置再上传（有基线）：云端的皮肤库仍然留着。
    fixture
        .local
        .local()
        .settings
        .insert("input.schema".to_owned(), string("shuangpin"));
    store_mark(&fixture, Section::Settings);
    let uploaded = fixture.run(Trigger::Manual, None, settings_and_phrases(), NOW + 60_000);
    assert_eq!(
        uploaded["sections"]["settings"]["outcome"], "synced",
        "{uploaded}"
    );
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.revision, 4);
    assert_eq!(
        cloud.settings.get("input.schema"),
        Some(&string("shuangpin"))
    );
    assert_eq!(
        cloud.settings.get(policy::SKINS_KEY),
        Some(&string(&library))
    );
}

#[test]
fn an_emptied_dictionary_cloud_never_replaces_local_words() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 首次同步，云端在「删除云端数据」以后只剩一条 reset：当作空的云端，直接上传本机，「使用云端」也不清空本机。
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 3;
        cloud.dictionary_reset = true;
        cloud.dictionary_items = 0;
    }
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(first["outcome"], "done", "{first}");
    assert_eq!(first["sections"]["dictionary"]["outcome"], "synced");
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);
    assert_eq!(fixture.state().sections.dictionary.cursor, Some(4));

    // 同步过以后云端被清空：问用户；选「使用云端」时按合并处理，不拿空快照替换本机。
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 5;
        cloud.dictionary_reset = true;
        cloud.dictionary_items = 0;
    }
    let asked = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    let chosen = fixture.run(
        Trigger::Choice,
        Some(Choice::UseCloud),
        dictionary,
        NOW + 120_000,
    );
    assert_eq!(chosen["outcome"], "done", "{chosen}");
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.local.local().merged, 1);
    let state = fixture.state();
    assert!(!state.sections.dictionary.cleared);
    assert!(
        state.sections.dictionary.dirty(),
        "local words are uploaded next round"
    );
}

#[test]
fn use_cloud_never_empties_local_words_when_the_cloud_dictionary_is_empty_without_a_reset() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 新设备首次同步，云端在云词库面板里加过词又全删掉：改动都不是 reset，词库条数为 0。
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 3;
        cloud.dictionary_reset = false;
        cloud.dictionary_items = 0;
    }
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(first["outcome"], "done", "{first}");
    assert_eq!(first["sections"]["dictionary"]["outcome"], "synced");
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);
    assert!(fixture.api.calls().contains(&"data_summary"));
    // 云端有词时照常问、照常按「使用云端」下载。
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 3;
        cloud.dictionary_items = 2;
    }
    fixture.enable();
    let asked = fixture.run(Trigger::Enable, None, dictionary, NOW);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    let chosen = fixture.run(
        Trigger::Choice,
        Some(Choice::UseCloud),
        dictionary,
        NOW + 1_000,
    );
    assert_eq!(chosen["outcome"], "done", "{chosen}");
    assert_eq!(fixture.local.local().replaced, [(USER.to_owned(), 3)]);
}

#[test]
fn an_empty_snapshot_never_replaces_local_words_even_when_the_summary_is_unreadable() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 新设备首次同步，云端词库其实是空的，但读汇总失败了：云端按有词处理，问用户，用户选了「使用云端」。
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 3;
        cloud.dictionary_items = 0;
    }
    fixture.api.fail(
        "data_summary",
        AccountFailure::from_http_response(503, b"{}", None),
    );
    fixture.enable();
    let chosen = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(chosen["outcome"], "done", "{chosen}");
    assert_eq!(
        chosen["sections"]["dictionary"]["outcome"], "synced",
        "{chosen}"
    );
    assert!(fixture.api.calls().contains(&"data_summary"));
    // 下载到的快照一个词都没有：不交给激活队列替换本机，改按合并处理，本机的词留着、标成待上传。
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.local.local().merged, 1);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(3));
    assert!(state.sections.dictionary.dirty());
    assert_eq!(state.sections.dictionary.choice, None);
    // 下一轮把本机的词传回云端。
    let uploaded = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(uploaded["outcome"], "done", "{uploaded}");
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);
    assert_eq!(fixture.state().sections.dictionary.cursor, Some(4));

    // 同步过以后云端变成空的（云词库面板里删光，改动都不是 reset），本机没改：照样不拿空快照替换本机的词。
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 6;
        cloud.dictionary_reset = false;
        cloud.dictionary_items = 0;
    }
    let later = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
    assert_eq!(
        later["sections"]["dictionary"]["outcome"], "synced",
        "{later}"
    );
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.local.local().merged, 2);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(6));
    assert!(state.sections.dictionary.dirty());
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
fn an_unreadable_local_skin_library_does_not_delete_the_cloud_library() {
    let fixture = Fixture::new();
    let library =
        json!([{ "id": "cloud", "name": "cloud", "design": {}, "updated_at": 1 }]).to_string();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            (policy::SKINS_KEY.to_owned(), string(&library)),
        ]
        .into(),
    };
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.local.local().skins = Some("{\"not\":\"an array\"}".into());
    fixture.enable();
    let capabilities = Capabilities {
        settings: true,
        skins_library: true,
        ..Capabilities::default()
    };
    let merged = fixture.run(Trigger::Choice, Some(Choice::Merge), capabilities, NOW);
    assert_eq!(merged["outcome"], "done", "{merged}");
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.revision, 3);
    assert_eq!(cloud.settings.get("input.schema"), Some(&string("quanpin")));
    assert_eq!(
        cloud.settings.get(policy::SKINS_KEY),
        Some(&string(&library))
    );
    assert_eq!(
        merged["sections"]["skins"]["reason"], LOCAL_SKINS_UNREADABLE,
        "{merged}"
    );
}

#[test]
fn an_unreadable_cloud_skin_library_is_never_replaced_by_the_local_one() {
    let capabilities = Capabilities {
        settings: true,
        skins_library: true,
        ..Capabilities::default()
    };
    // 云端的皮肤库不是 JSON 数组（可能是更新的版本写的）：合并时不拿本机的库整份替换它。
    let unreadable = json!({ "format": 2, "designs": [] }).to_string();
    let local_library =
        json!([{ "id": "local", "name": "local", "design": {}, "updated_at": 5 }]).to_string();
    let fixture = Fixture::new();
    fixture.api.cloud().preferences = AccountPreferences {
        revision: 2,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            (policy::SKINS_KEY.to_owned(), string(&unreadable)),
        ]
        .into(),
    };
    fixture.local.local().settings = [("input.schema".to_owned(), string("quanpin"))].into();
    fixture.local.local().skins = Some(local_library.clone());
    fixture.enable();
    let merged = fixture.run(Trigger::Choice, Some(Choice::Merge), capabilities, NOW);
    assert_eq!(merged["outcome"], "done", "{merged}");
    assert_eq!(merged["sections"]["settings"]["outcome"], "synced");
    assert_eq!(
        merged["sections"]["skins"]["reason"], CLOUD_SKINS_UNREADABLE,
        "{merged}"
    );
    let cloud = fixture.api.cloud().preferences.clone();
    assert_eq!(cloud.revision, 3);
    assert_eq!(cloud.settings.get("input.schema"), Some(&string("quanpin")));
    assert_eq!(
        cloud.settings.get(policy::SKINS_KEY),
        Some(&string(&unreadable))
    );
    // 之后本机改了皮肤、云端没变（上传）：照样不替换。
    store_mark(&fixture, Section::Skins);
    let uploaded = fixture.run(Trigger::Manual, None, capabilities, NOW + 60_000);
    assert_eq!(
        uploaded["sections"]["skins"]["reason"], CLOUD_SKINS_UNREADABLE,
        "{uploaded}"
    );
    assert_eq!(
        fixture
            .api
            .cloud()
            .preferences
            .settings
            .get(policy::SKINS_KEY),
        Some(&string(&unreadable))
    );
    assert!(!fixture.state().sections.skins.dirty());
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
    fixture.local.finish_activation(true);
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
fn a_downloaded_snapshot_blocks_uploads_until_the_keyboard_activates_it() {
    let fixture = Fixture::new();
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 新设备选「使用云端」：快照交给激活队列，打开开关时的待上传标记照旧留着。
    let downloaded = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(
        downloaded["sections"]["dictionary"]["outcome"], "synced",
        "{downloaded}"
    );
    assert_eq!(fixture.local.local().replaced, [(USER.to_owned(), 4)]);
    assert!(fixture.state().sections.dictionary.dirty());
    // 键盘还没激活快照：不能把激活前的本机词库整份传上去。
    let blocked = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(
        blocked["sections"]["dictionary"]["reason"], "dictionary_pending",
        "{blocked}"
    );
    assert_eq!(fixture.local.local().exported, 0);
    assert_eq!(fixture.api.cloud().snapshot_uploads, 0);
    assert_eq!(fixture.api.cloud().dictionary_revision, 4);
    assert!(fixture.state().sections.dictionary.dirty());
    // 激活以后照常上传。
    fixture.local.finish_activation(true);
    let uploaded = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
    assert_eq!(
        uploaded["sections"]["dictionary"]["outcome"], "synced",
        "{uploaded}"
    );
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);
}

#[test]
fn a_downloaded_snapshot_that_never_activates_is_merged_instead_of_uploaded_over_the_cloud() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 新设备打过几个字，选「使用云端」：快照交给激活队列，可激活时本机版本已经变了，请求以冲突结束。
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let downloaded = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(
        downloaded["sections"]["dictionary"]["outcome"], "synced",
        "{downloaded}"
    );
    assert_eq!(
        fixture.state().sections.dictionary.activation.as_deref(),
        Some("snapshot-1")
    );
    fixture.local.finish_activation(false);
    // 下一轮云端相对游标没变、本机有标记，原来会上传没激活过的本机词库，把云端整份换掉；现在把云端的词并进本机。
    let merged = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(
        merged["sections"]["dictionary"]["outcome"], "synced",
        "{merged}"
    );
    assert_eq!(fixture.local.local().exported, 0);
    assert_eq!(fixture.api.cloud().snapshot_uploads, 0);
    assert_eq!(fixture.local.local().merged, 1);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.activation, None);
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert!(state.sections.dictionary.dirty());
    // 并进来以后照常整份上传，带着云端的词。
    let uploaded = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
    assert_eq!(
        uploaded["sections"]["dictionary"]["outcome"], "synced",
        "{uploaded}"
    );
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);

    // 本机没有词时没激活的快照重新下载一次，改记新的请求。
    let fixture = Fixture::new();
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    fixture.local.finish_activation(false);
    let again = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(
        again["sections"]["dictionary"]["outcome"], "synced",
        "{again}"
    );
    assert_eq!(fixture.local.local().replaced.len(), 2);
    assert_eq!(fixture.api.cloud().snapshot_uploads, 0);
    assert_eq!(
        fixture.state().sections.dictionary.activation.as_deref(),
        Some("snapshot-2")
    );
    // 还在等激活时云端又变了：不下载、不合并，记着的请求留着。
    fixture.api.cloud().dictionary_revision = 6;
    let waiting = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
    assert_eq!(
        waiting["sections"]["dictionary"]["reason"], "dictionary_pending",
        "{waiting}"
    );
    assert_eq!(fixture.local.local().replaced.len(), 2);
    assert_eq!(fixture.local.local().merged, 0);
    assert_eq!(
        fixture.state().sections.dictionary.activation.as_deref(),
        Some("snapshot-2")
    );
}

#[test]
fn an_unfinished_merge_keeps_the_cursor_and_the_choice_for_the_next_round() {
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.local.local().merge_failure = Some(MERGE_PENDING.to_owned());
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let partial = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(
        partial["sections"]["dictionary"]["outcome"], "failed",
        "{partial}"
    );
    assert_eq!(partial["sections"]["dictionary"]["reason"], MERGE_PENDING);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, None);
    assert_eq!(state.sections.dictionary.choice, Some(Choice::Merge));
    assert_eq!(state.pending_choice, None);
    assert_eq!(
        partial["status"]["status_line"],
        policy::local_message(MERGE_PENDING)
    );
    // 下一轮不再问选择，接着合并；排完以后推进游标、用掉选择，等键盘应用后再整份上传。
    fixture.local.local().merge_failure = None;
    let merged = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(merged["outcome"], "done", "{merged}");
    assert_eq!(fixture.local.local().merged, 2);
    assert_eq!(fixture.api.cloud().snapshot_uploads, 0);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert_eq!(state.sections.dictionary.choice, None);
    assert!(state.sections.dictionary.dirty());

    // 云端有本机导入不了的词、或者被别的失败请求挡住时也一样：不推进、不上传，选择留着，状态行说清楚为什么停下。
    for reason in [MERGE_REJECTED, MERGE_BLOCKED] {
        let fixture = Fixture::new();
        fixture.local.local().words = true;
        fixture.local.local().merge_failure = Some(reason.to_owned());
        fixture.api.cloud().dictionary_revision = 4;
        fixture.enable();
        let stopped = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
        assert_eq!(stopped["sections"]["dictionary"]["reason"], reason);
        assert_eq!(
            stopped["status"]["status_line"],
            policy::local_message(reason)
        );
        let state = fixture.state();
        assert_eq!(state.sections.dictionary.cursor, None);
        assert_eq!(state.sections.dictionary.choice, Some(Choice::Merge));
        let again = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
        assert_eq!(again["sections"]["dictionary"]["reason"], reason, "{again}");
        assert_eq!(fixture.api.cloud().snapshot_uploads, 0);
    }
}

fn snapshot_downloads(fixture: &Fixture) -> usize {
    fixture
        .api
        .calls()
        .iter()
        .filter(|call| **call == "snapshot_download")
        .count()
}

#[test]
fn an_active_trigger_after_an_unfinished_merge_is_throttled_and_backs_off() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.local.local().merge_failure = Some(MERGE_REJECTED.to_owned());
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let stopped = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(
        stopped["sections"]["dictionary"]["reason"], MERGE_REJECTED,
        "{stopped}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.choice, Some(Choice::Merge));
    assert_eq!(state.pending_choice, None);
    assert_eq!(
        state.sections.dictionary.stall,
        Some(Stall {
            rounds: 1,
            until_ms: NOW + policy::STALL_BASE_MS,
            present: None,
        })
    );
    // 紧接着回到前台：留下的选择不让这一轮跳过节流；过了 30 秒，词库的待上传标记也因为退避不再把节流缩短。都不重新下载快照。
    for offset in [1_000, 2_000, 40_000, 120_000] {
        let skipped = fixture.run(Trigger::Active, None, dictionary, NOW + offset);
        assert_eq!(skipped["reason"], "throttled", "{skipped}");
    }
    assert_eq!(snapshot_downloads(&fixture), 0);
    // 退避到期：自动重试一次，还是没做完，这一轮没有排进新的词，退避翻倍。
    let retried = fixture.run(
        Trigger::Active,
        None,
        dictionary,
        NOW + policy::STALL_BASE_MS,
    );
    assert_eq!(
        retried["sections"]["dictionary"]["reason"], MERGE_REJECTED,
        "{retried}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    let stall = fixture.state().sections.dictionary.stall.unwrap();
    assert_eq!(stall.rounds, 2);
    assert_eq!(
        stall.until_ms,
        NOW + policy::STALL_BASE_MS + policy::stall_ms(2)
    );
    // 节流到期了，词库还在退避里：这一轮跳过词库，失败原因留着，不算同步成功。
    let later = NOW + 2 * policy::STALL_BASE_MS;
    let skipped = fixture.run(Trigger::Active, None, dictionary, later);
    assert_eq!(
        skipped["sections"]["dictionary"]["reason"], DICTIONARY_BACKOFF,
        "{skipped}"
    );
    assert_eq!(
        skipped["status"]["status_line"],
        policy::local_message(MERGE_REJECTED)
    );
    assert_eq!(snapshot_downloads(&fixture), 0);
    // 手动同步不等退避。这一轮排进了新的词：合并在往前走，退避从头数。
    {
        let mut local = fixture.local.local();
        local.merge_failure = Some(MERGE_PENDING.to_owned());
        local.merge_queued = true;
    }
    let progressed = fixture.run(Trigger::Manual, None, dictionary, later + 1_000);
    assert_eq!(
        progressed["sections"]["dictionary"]["reason"], MERGE_PENDING,
        "{progressed}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    assert_eq!(
        fixture.state().sections.dictionary.stall,
        Some(Stall {
            rounds: 1,
            until_ms: later + 1_000 + policy::STALL_BASE_MS,
            present: None,
        })
    );
    // 合并做完：退避和留下的选择都清掉。
    fixture.local.local().merge_failure = None;
    let merged = fixture.run(Trigger::Manual, None, dictionary, later + 2_000);
    assert_eq!(
        merged["sections"]["dictionary"]["outcome"], "synced",
        "{merged}"
    );
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.stall, None);
    assert_eq!(state.sections.dictionary.choice, None);
    assert_eq!(state.sections.dictionary.cursor, Some(4));
}

#[test]
fn a_persistent_local_dictionary_failure_backs_off_instead_of_redownloading() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.api.cloud().dictionary_revision = 3;
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(first["outcome"], "done", "{first}");
    assert!(fixture.state().sections.dictionary.dirty());
    assert_eq!(snapshot_downloads(&fixture), 1);
    // 云端又变了，本机带着待上传标记，每一轮都要合并；本机词库这时一直读写失败。
    fixture.api.cloud().dictionary_revision = 5;
    fixture.local.local().merge_failure = Some("dictionary_storage".to_owned());
    let start = NOW + 60_000;
    let failed = fixture.run(Trigger::Manual, None, dictionary, start);
    assert_eq!(
        failed["sections"]["dictionary"]["reason"], "dictionary_storage",
        "{failed}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    let state = fixture.state();
    assert_eq!(
        state.sections.dictionary.stall,
        Some(Stall {
            rounds: 1,
            until_ms: start + policy::STALL_BASE_MS,
            present: None,
        })
    );
    assert_eq!(state.sections.dictionary.cursor, Some(3));
    assert_eq!(
        failed["status"]["status_line"],
        policy::local_message("dictionary_storage")
    );
    // 满 30 秒回到前台：词库在退避里，它的待上传标记不把节流缩短，不重下整份快照。
    for offset in [40_000, 120_000] {
        let skipped = fixture.run(Trigger::Active, None, dictionary, start + offset);
        assert_eq!(skipped["reason"], "throttled", "{skipped}");
    }
    assert_eq!(snapshot_downloads(&fixture), 0);
    // 退避到期自动重试一次，还是失败：没有进展，退避翻倍。
    let expired = start + policy::STALL_BASE_MS;
    let retried = fixture.run(Trigger::Active, None, dictionary, expired);
    assert_eq!(
        retried["sections"]["dictionary"]["reason"], "dictionary_storage",
        "{retried}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    let stall = fixture.state().sections.dictionary.stall.unwrap();
    assert_eq!(
        (stall.rounds, stall.until_ms),
        (2, expired + policy::stall_ms(2))
    );
    // 手动同步、打开开关和做选择都不等退避。
    let manual = fixture.run(Trigger::Manual, None, dictionary, expired + 1_000);
    assert_eq!(
        manual["sections"]["dictionary"]["reason"], "dictionary_storage",
        "{manual}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    assert_eq!(fixture.state().sections.dictionary.stall.unwrap().rounds, 3);
    let enabled = fixture.run(Trigger::Enable, None, dictionary, expired + 2_000);
    assert_eq!(
        enabled["sections"]["dictionary"]["reason"], "dictionary_storage",
        "{enabled}"
    );
    assert_eq!(snapshot_downloads(&fixture), 1);
    // 词库恢复以后合并做完：退避清掉。
    fixture.local.local().merge_failure = None;
    let merged = fixture.run(Trigger::Manual, None, dictionary, expired + 3_000);
    assert_eq!(
        merged["sections"]["dictionary"]["outcome"], "synced",
        "{merged}"
    );
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.stall, None);
    assert_eq!(state.sections.dictionary.cursor, Some(5));

    // 联网失败不进这个退避，照旧只看 `Retry-After`。
    fixture.api.cloud().dictionary_revision = 7;
    fixture.api.fail(
        "snapshot_download",
        AccountFailure::from_http_response(503, b"{}", None),
    );
    let offline = fixture.run(Trigger::Manual, None, dictionary, expired + 4_000);
    assert_eq!(
        offline["sections"]["dictionary"]["outcome"], "failed",
        "{offline}"
    );
    assert_eq!(fixture.state().sections.dictionary.stall, None);
}

#[test]
fn requeueing_without_progress_doubles_the_backoff_and_applied_words_restart_it() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    {
        let mut local = fixture.local.local();
        local.words = true;
        local.merge_failure = Some(MERGE_PENDING.to_owned());
        local.merge_present = Some(10);
    }
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(
        first["sections"]["dictionary"]["reason"], MERGE_PENDING,
        "{first}"
    );
    assert_eq!(
        fixture.state().sections.dictionary.stall,
        Some(Stall {
            rounds: 1,
            until_ms: NOW + policy::STALL_BASE_MS,
            present: Some(10),
        })
    );
    // 键盘一直拿不到词库：同一批词每一轮都重排，没有排进新的词，本机也没多出云端的词。退避翻倍，不从 5 分钟重新数。
    let mut now = NOW;
    for rounds in [2, 3, 4] {
        now += 1_000;
        let again = fixture.run(Trigger::Manual, None, dictionary, now);
        assert_eq!(
            again["sections"]["dictionary"]["reason"], MERGE_PENDING,
            "{again}"
        );
        assert_eq!(
            fixture.state().sections.dictionary.stall,
            Some(Stall {
                rounds,
                until_ms: now + policy::stall_ms(rounds),
                present: Some(10),
            })
        );
    }
    // 键盘应用了排着的几个词：云端的词在本机的多了，合并在往前走，退避从头数。
    fixture.local.local().merge_present = Some(12);
    now += 1_000;
    fixture.run(Trigger::Manual, None, dictionary, now);
    assert_eq!(
        fixture.state().sections.dictionary.stall,
        Some(Stall {
            rounds: 1,
            until_ms: now + policy::STALL_BASE_MS,
            present: Some(12),
        })
    );
    // 中间一轮本机读写失败、没走到比对：沿用记下的词数，下一轮仍然拿它比。
    {
        let mut local = fixture.local.local();
        local.merge_failure = Some("dictionary_storage".to_owned());
        local.merge_present = None;
    }
    now += 1_000;
    fixture.run(Trigger::Manual, None, dictionary, now);
    let stall = fixture.state().sections.dictionary.stall.unwrap();
    assert_eq!((stall.rounds, stall.present), (2, Some(12)));
    {
        let mut local = fixture.local.local();
        local.merge_failure = Some(MERGE_PENDING.to_owned());
        local.merge_present = Some(12);
    }
    now += 1_000;
    fixture.run(Trigger::Manual, None, dictionary, now);
    assert_eq!(fixture.state().sections.dictionary.stall.unwrap().rounds, 3);
}

#[test]
fn a_choice_made_while_busy_reaches_a_dictionary_in_its_merge_backoff() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    fixture.local.local().words = true;
    fixture.api.cloud().dictionary_revision = 3;
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
    assert_eq!(first["outcome"], "done", "{first}");
    // 词库还在早先一次没做完的合并的退避里，这时云端被清空了：手动同步问用户。
    fixture
        .store
        .update(|state| {
            state.sections.dictionary.stall = Some(Stall {
                rounds: 4,
                until_ms: NOW + policy::MAX_STALL_MS,
                present: None,
            });
        })
        .unwrap();
    {
        let mut cloud = fixture.api.cloud();
        cloud.dictionary_revision = 5;
        cloud.dictionary_reset = true;
        cloud.dictionary_items = 0;
    }
    let asked = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(asked["reason"], "cloud_cleared", "{asked}");
    let merged_before = fixture.local.local().merged;
    // 用户选的时候另一个进程恰好在跑一轮：选择记成 pending_choice。
    let lease = fixture
        .store
        .update(|state| state.begin_run(USER, NOW + 61_000, None))
        .unwrap();
    let busy = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        dictionary,
        NOW + 62_000,
    );
    assert_eq!(busy["reason"], "busy", "{busy}");
    fixture
        .store
        .update(|state| state.release(&lease.id))
        .unwrap();
    // 下一次回到前台：退避还没到期，词库照样用上这个选择，不丢掉、不再问。
    let resumed = fixture.run(
        Trigger::Active,
        None,
        dictionary,
        NOW + 62_000 + policy::THROTTLE_MS,
    );
    assert_eq!(
        resumed["sections"]["dictionary"]["outcome"], "synced",
        "{resumed}"
    );
    assert_eq!(fixture.local.local().merged, merged_before + 1);
    let state = fixture.state();
    assert_eq!(state.pending_choice, None);
    assert!(!state.sections.dictionary.cleared);
}

#[test]
fn a_choice_kept_for_the_dictionary_is_not_applied_to_a_cleared_phrase_cloud() {
    let capabilities = Capabilities {
        phrases: true,
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    fixture.api.cloud().phrases = cloud_phrases(3, &["云端"]);
    fixture.local.add_phrase("本机");
    fixture.local.local().words = true;
    fixture.local.local().merge_failure = Some(MERGE_PENDING.to_owned());
    fixture.api.cloud().dictionary_revision = 4;
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::Merge), capabilities, NOW);
    assert_eq!(first["sections"]["phrases"]["outcome"], "synced", "{first}");
    assert_eq!(first["sections"]["dictionary"]["reason"], MERGE_PENDING);
    assert_eq!(
        fixture.state().sections.dictionary.choice,
        Some(Choice::Merge)
    );
    // 之后别处「删除云端数据」清空了常用语：给词库留下的「合并」不能替用户回答这一次，要问。
    fixture.api.cloud().phrases = AccountPhrases {
        revision: 9,
        phrases: Vec::new(),
    };
    let asked = fixture.run(Trigger::Manual, None, capabilities, NOW + 60_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert!(fixture.api.cloud_texts().is_empty());
    assert_eq!(fixture.local.texts(), ["本机", "云端"]);
    assert_eq!(
        fixture.state().sections.dictionary.choice,
        Some(Choice::Merge)
    );
}

#[test]
fn a_snapshot_the_keyboard_cannot_take_yet_keeps_the_first_run_choice() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    // 新装以后还没打开过键盘就打开了同步：键盘没发布过本机词库版本，快照交不给激活队列。
    let fixture = Fixture::new();
    fixture.api.cloud().dictionary_revision = 4;
    fixture.local.local().replace_failure = Some(DICTIONARY_UNPUBLISHED.to_owned());
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(
        first["sections"]["dictionary"]["reason"], DICTIONARY_UNPUBLISHED,
        "{first}"
    );
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, None);
    // 本机这时没有自己的词，两个选择没有区别：留成「合并」，以后键盘学到的词不会被云端快照整份替换。
    assert_eq!(state.sections.dictionary.choice, Some(Choice::Merge));
    assert!(state.sections.dictionary.stall.is_some());
    // 还没打开过键盘：退避过后的自动同步照样下载，不再问一次选择。
    let again = fixture.run(
        Trigger::Active,
        None,
        dictionary,
        NOW + policy::STALL_BASE_MS,
    );
    assert_eq!(
        again["sections"]["dictionary"]["reason"], DICTIONARY_UNPUBLISHED,
        "{again}"
    );
    assert_eq!(
        fixture.state().sections.dictionary.choice,
        Some(Choice::Merge)
    );
    // 键盘打开过、学到了词以后，退避过后的自动同步把云端的词并进来，不拿快照替换本机的词，也不再问一次选择。
    {
        let mut local = fixture.local.local();
        local.replace_failure = None;
        local.words = true;
    }
    let resumed = fixture.run(
        Trigger::Active,
        None,
        dictionary,
        NOW + policy::STALL_BASE_MS + policy::stall_ms(2),
    );
    assert_eq!(resumed["outcome"], "done", "{resumed}");
    assert!(fixture.local.local().replaced.is_empty());
    assert_eq!(fixture.local.local().merged, 1);
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert_eq!(state.sections.dictionary.choice, None);
    assert_eq!(state.sections.dictionary.stall, None);

    // 本机已经有自己的词、用户明确选了「使用云端」：快照交不给激活队列时这个选择原样留着，键盘可用以后照用户选的下载。
    let fixture = Fixture::new();
    fixture.api.cloud().dictionary_revision = 4;
    {
        let mut local = fixture.local.local();
        local.words = true;
        local.replace_failure = Some(DICTIONARY_UNPUBLISHED.to_owned());
    }
    fixture.enable();
    let first = fixture.run(Trigger::Choice, Some(Choice::UseCloud), dictionary, NOW);
    assert_eq!(
        first["sections"]["dictionary"]["reason"], DICTIONARY_UNPUBLISHED,
        "{first}"
    );
    assert_eq!(
        fixture.state().sections.dictionary.choice,
        Some(Choice::UseCloud)
    );
    fixture.local.local().replace_failure = None;
    let resumed = fixture.run(Trigger::Manual, None, dictionary, NOW + 1_000);
    assert_eq!(resumed["outcome"], "done", "{resumed}");
    assert_eq!(fixture.local.local().replaced, [(USER.to_owned(), 4)]);
}

#[test]
fn legacy_state_is_imported_after_status_bound_the_same_account() {
    let fixture = Fixture::new();
    // 宿主先读了一次状态：对账把登录的账号写进来了，开关没开、没有游标。
    let before = status(&fixture.session, &fixture.store, NOW).unwrap();
    assert_eq!(before["account_id"], USER);
    let ios = |account: &str| {
        json!({
            "version": 1,
            "enabled": true,
            "accountID": account,
            "cursors": { "settings": "4", "phrases": "2" },
            "generations": {},
            "cleans": {},
        })
    };
    let other = import_legacy(
        &fixture.store,
        Some(ios("synthetic-other")),
        None,
        None,
        NOW,
    )
    .unwrap();
    assert_eq!(other["imported"], false);
    let imported = import_legacy(&fixture.store, Some(ios(USER)), None, None, NOW).unwrap();
    assert_eq!(imported["imported"], true, "{imported}");
    let status = status(&fixture.session, &fixture.store, NOW).unwrap();
    assert_eq!(status["enabled"], true);
    assert_eq!(status["sections"]["settings"]["cursor"], 4);
}

#[test]
fn legacy_state_never_turns_sync_back_on_after_the_user_turned_it_off() {
    let ios = json!({
        "version": 1,
        "enabled": true,
        "accountID": USER,
        "cursors": {},
        "generations": {},
        "cleans": {},
    });
    // 导入以后用户关掉同步，下次启动宿主又带来同一份旧状态。
    let fixture = Fixture::new();
    let imported = import_legacy(&fixture.store, Some(ios.clone()), None, None, NOW).unwrap();
    assert_eq!(imported["imported"], true, "{imported}");
    assert_eq!(imported["status"]["enabled"], true);
    set_enabled(&fixture.session, &fixture.store, false, NOW + 1).unwrap();
    let again = import_legacy(&fixture.store, Some(ios.clone()), None, None, NOW + 2).unwrap();
    assert_eq!(again["imported"], false, "{again}");
    assert_eq!(again["status"]["enabled"], false);
    // 退出再登录同一个账号也不再导入。
    sign_out(&fixture.store, &fixture.local, NOW + 3).unwrap();
    let signed_out = import_legacy(&fixture.store, Some(ios.clone()), None, None, NOW + 4).unwrap();
    assert_eq!(signed_out["imported"], false, "{signed_out}");
    assert_eq!(
        status(&fixture.session, &fixture.store, NOW + 5).unwrap()["enabled"],
        false
    );
    // 先在新界面打开、还没跑完第一轮就关掉，然后才导入。
    let fixture = Fixture::new();
    status(&fixture.session, &fixture.store, NOW).unwrap();
    set_enabled(&fixture.session, &fixture.store, true, NOW + 1).unwrap();
    set_enabled(&fixture.session, &fixture.store, false, NOW + 2).unwrap();
    let late = import_legacy(&fixture.store, Some(ios), None, None, NOW + 3).unwrap();
    assert_eq!(late["imported"], false, "{late}");
    assert_eq!(
        status(&fixture.session, &fixture.store, NOW + 4).unwrap()["enabled"],
        false
    );
}

#[test]
fn a_legacy_android_snapshot_that_never_activated_is_merged_instead_of_uploaded() {
    use msime_client_core::cloud::snapshot_queue::local_version;
    use sha2::{Digest, Sha256};
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let android = |cursor: &str| AndroidLegacy {
        sync_v1: json!({
            "enabled": true,
            "account_id": USER,
            "login_kind": "email",
            "binding_generation": 1,
            "cursor_dictionary": cursor,
            "gen_dictionary": 2,
            "clean_dictionary": 1,
        }),
        cloud_sync_status: Value::Null,
    };
    // 旧的 Android 最后一轮下载了云端修订号 4 的快照，游标记成 4；激活时本机词库已经变了，请求以冲突结束。
    let fixture = Fixture::new();
    let queue_directory = fixture.state_directory.join("queue");
    let queue = DictionarySnapshotQueue::new(queue_directory.clone()).unwrap();
    let published = local_version(None, &"0".repeat(64)).unwrap();
    queue.publish_local_version(&published).unwrap();
    let file = fixture.state_directory.join("download.ndjson");
    std::fs::write(&file, "synthetic snapshot\n").unwrap();
    let sha256 = hex::encode(Sha256::digest(std::fs::read(&file).unwrap()));
    let request = queue
        .enqueue(&file, USER, 4, &published, &sha256, true)
        .unwrap();
    {
        let lease = queue.acquire_worker_lease().unwrap();
        assert!(queue.claim(&lease, Some(USER)).unwrap().is_some());
        let typed = local_version(None, &"1".repeat(64)).unwrap();
        let applied = queue
            .complete(request, &lease, &typed, false, Some(USER), || {
                unreachable!("a conflicting request is not applied")
            })
            .unwrap();
        assert!(!applied);
    }
    let imported = import_legacy(
        &fixture.store,
        None,
        Some(android("4")),
        Some(&queue_directory),
        NOW,
    )
    .unwrap();
    assert_eq!(imported["imported"], true, "{imported}");
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert_eq!(
        state.sections.dictionary.activation,
        Some(request.to_string())
    );
    // 第一轮：云端相对游标没变、本机有标记。那份快照没激活，本机缺着云端的词：改按合并补上，不整份上传。
    fixture.local.local().words = true;
    fixture.api.cloud().dictionary_revision = 4;
    let merged = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
    assert_eq!(
        merged["sections"]["dictionary"]["outcome"], "synced",
        "{merged}"
    );
    assert_eq!(fixture.local.local().merged, 1);
    assert_eq!(fixture.api.cloud().snapshot_uploads, 0);

    // 没给队列目录：不知道有没有没激活的快照，词库游标不导入，下一轮词库按首次同步处理。
    let fixture = Fixture::new();
    import_legacy(&fixture.store, None, Some(android("4")), None, NOW).unwrap();
    let state = fixture.state();
    assert!(state.enabled);
    assert_eq!(state.sections.dictionary.cursor, None);
    // 队列里没有没应用的请求：游标照常导入，不记等激活的快照。
    let fixture = Fixture::new();
    let empty = fixture.state_directory.join("empty-queue");
    import_legacy(&fixture.store, None, Some(android("4")), Some(&empty), NOW).unwrap();
    let state = fixture.state();
    assert_eq!(state.sections.dictionary.cursor, Some(4));
    assert_eq!(state.sections.dictionary.activation, None);
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
    let imported = import_legacy(&fixture.store, Some(ios.clone()), None, None, NOW).unwrap();
    assert_eq!(imported["imported"], true);
    assert_eq!(imported["status"]["sections"]["settings"]["cursor"], 4);
    assert_eq!(imported["status"]["sections"]["phrases"]["dirty"], true);
    let again = import_legacy(&fixture.store, Some(ios), None, None, NOW).unwrap();
    assert_eq!(again["imported"], false);
    assert_eq!(
        import_legacy(&fixture.store, None, None, None, NOW),
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
    let applied = local
        .apply_settings(
            &document,
            &fields,
            local.export_settings().unwrap().revision,
        )
        .unwrap();
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
fn writing_settings_back_never_overwrites_a_change_made_after_they_were_read() {
    use msime_client_core::preferences::InputScheme;
    let directory = tempfile::tempdir().unwrap();
    let local = host_local(directory.path(), HostInputs::default());
    let exported = local.export_settings().unwrap();
    assert_eq!(exported.settings["input.schema"], string("quanpin"));
    // 读本机和写回之间，另一个进程里用户把方案改成了双拼。
    let store = PreferencesStore::new(directory.path());
    let current = store.load().unwrap();
    let mut edited = current.preferences.clone();
    edited.scheme = InputScheme::Shuangpin;
    store.save(current.revision, edited).unwrap();
    let fields = schema(&[("input.schema", "string")]);
    let document = AccountPreferences {
        revision: 3,
        settings: [("input.schema".to_owned(), string("wubi"))].into(),
    };
    assert_eq!(
        local
            .apply_settings(&document, &fields, exported.revision)
            .err(),
        Some(SETTINGS_CHANGED.to_owned())
    );
    assert_eq!(
        store.load().unwrap().preferences.scheme,
        InputScheme::Shuangpin
    );
    // 按刚读到的修订号照常写回。
    let fresh = local.export_settings().unwrap();
    local
        .apply_settings(&document, &fields, fresh.revision)
        .unwrap();
    assert_eq!(store.load().unwrap().preferences.scheme, InputScheme::Wubi);
}

#[test]
fn the_harmony_mapping_uses_its_own_namespace_over_shared_preferences() {
    use msime_client_services::account::harmony_settings::HARMONY_FEEDBACK_KEYS;
    let directory = tempfile::tempdir().unwrap();
    let feedback = HostKeyboardFeedback {
        sound_enabled: true,
        haptics_enabled: true,
        haptic_strength: "medium".into(),
    };
    let local = host_local(
        directory.path(),
        HostInputs {
            harmony: true,
            feedback: Some(feedback.clone()),
            ..HostInputs::default()
        },
    );
    let exported = local.export_settings().unwrap();
    assert_eq!(exported.settings["input.schema"], string("quanpin"));
    assert_eq!(
        exported.settings[HARMONY_FEEDBACK_KEYS[0]],
        AccountPreferenceValue::Boolean(true)
    );
    assert_eq!(
        exported.settings["platform.harmony.keyboard_layout"],
        string("twenty_six_key")
    );
    assert!(exported
        .settings
        .keys()
        .all(|key| !key.starts_with("platform.android.")));
    assert!(exported.skins.is_none());
    let fields = schema(&[
        ("input.schema", "string"),
        ("platform.harmony.keyboard_layout", "string"),
        ("platform.android.keyboard_layout", "string"),
        (HARMONY_FEEDBACK_KEYS[0], "boolean"),
    ]);
    let document = AccountPreferences {
        revision: 3,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            (
                "platform.android.keyboard_layout".to_owned(),
                string("handwriting"),
            ),
            (
                HARMONY_FEEDBACK_KEYS[0].to_owned(),
                AccountPreferenceValue::Boolean(false),
            ),
        ]
        .into(),
    };
    let applied = local
        .apply_settings(
            &document,
            &fields,
            local.export_settings().unwrap().revision,
        )
        .unwrap();
    assert!(!applied.deferred);
    assert_eq!(
        applied.writes,
        HostWrites {
            feedback: Some(HostKeyboardFeedback {
                sound_enabled: false,
                ..feedback
            }),
            ..HostWrites::default()
        }
    );
    let saved = PreferencesStore::new(directory.path()).load().unwrap();
    let saved = serde_json::to_value(saved).unwrap();
    assert_eq!(saved["preferences"]["scheme"], "wubi");
    // Android 的布局键不归鸿蒙。
    assert_eq!(
        saved["preferences"]["touch_keyboard_layout"],
        "twenty_six_key"
    );
}

/// 鸿蒙同步过一次以后切到账号装不下的方案（粤拼）或 14 键：这两项都不导出，云端留着上次的全拼和 26 键。这一轮上传之后状态机会把合并结果写回本机，写回时不能拿云端的旧值把用户刚做的改动改回去，否则每一轮同步都会撤销一次。
#[test]
fn a_harmony_upload_keeps_the_scheme_and_layout_it_left_out() {
    use msime_client_core::preferences::{InputScheme, TouchKeyboardLayout};
    for name in ["cantonese", "fourteen_key"] {
        let fixture = Fixture::new();
        fixture.api.cloud().schema = vec![
            ("input.schema", "string"),
            ("platform.harmony.keyboard_layout", "string"),
        ];
        let directory = tempfile::tempdir().unwrap();
        let local = host_local(
            directory.path(),
            HostInputs {
                harmony: true,
                ..HostInputs::default()
            },
        );
        let settings_only = || RunRequest {
            trigger: Trigger::Manual,
            choice: None,
            capabilities: Capabilities {
                settings: true,
                ..Capabilities::default()
            },
            settings_locked: false,
            excluded_phrase_texts: Vec::new(),
        };
        fixture.enable();
        let first = run(
            &fixture.session,
            &fixture.store,
            &local,
            settings_only(),
            NOW,
        )
        .unwrap();
        assert_eq!(
            first["sections"]["settings"]["outcome"], "synced",
            "{name}: {first}"
        );
        let previous = fixture.api.cloud().preferences.settings.clone();
        assert_eq!(previous["input.schema"], string("quanpin"), "{name}");
        assert_eq!(
            previous["platform.harmony.keyboard_layout"],
            string("twenty_six_key"),
            "{name}"
        );

        let store = PreferencesStore::new(directory.path());
        let snapshot = store.load().unwrap();
        let mut changed = snapshot.preferences.clone();
        if name == "cantonese" {
            changed.scheme = InputScheme::Cantonese;
        } else {
            changed.touch_keyboard_layout = TouchKeyboardLayout::FourteenKey;
        }
        store.save(snapshot.revision, changed.clone()).unwrap();
        store_mark(&fixture, Section::Settings);
        let second = run(
            &fixture.session,
            &fixture.store,
            &local,
            settings_only(),
            NOW + 600_000,
        )
        .unwrap();
        assert_eq!(
            second["sections"]["settings"]["outcome"], "synced",
            "{name}: {second}"
        );
        let saved = store.load().unwrap().preferences;
        assert_eq!(
            (saved.scheme, saved.touch_keyboard_layout),
            (changed.scheme, changed.touch_keyboard_layout),
            "{name}"
        );
        assert!(saved == changed, "{name}");
        // 没导出的键在云端还是原来的值。
        assert_eq!(fixture.api.cloud().preferences.settings, previous, "{name}");
    }
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
    let applied = local
        .apply_settings(&same, &fields, local.export_settings().unwrap().revision)
        .unwrap();
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
    let applied = local
        .apply_settings(&wubi, &fields, local.export_settings().unwrap().revision)
        .unwrap();
    assert!(applied.deferred);
    let requested = applied.writes.ios_native.unwrap();
    assert_eq!(
        serde_json::to_value(&requested).unwrap()["inputScheme"],
        "wubi"
    );
    local
        .commit_settings(&wubi, &requested, local.export_settings().unwrap().revision)
        .unwrap();
    let saved = PreferencesStore::new(directory.path()).load().unwrap();
    let saved = serde_json::to_value(saved).unwrap();
    assert_eq!(saved["preferences"]["scheme"], "wubi");
    assert_eq!(saved["preferences"]["wubi_profile"], "wubi98");
}

#[test]
fn committing_ios_settings_never_overwrites_a_change_made_after_they_were_read() {
    use msime_client_core::preferences::{FrequencyMode, InputScheme};
    let directory = tempfile::tempdir().unwrap();
    let local = host_local(
        directory.path(),
        HostInputs {
            ios_native: Some(native()),
            ..HostInputs::default()
        },
    );
    let exported = local.export_settings().unwrap();
    assert_eq!(exported.settings["input.frequency_mode"], string("promote"));
    let fields = schema(&[
        ("input.schema", "string"),
        ("input.wubi_schema", "string"),
        ("input.frequency_mode", "string"),
    ]);
    // 合并出来的文档：方案用云端的五笔，词频方式是读本机时的值。
    let wubi = AccountPreferences {
        revision: 3,
        settings: [
            ("input.schema".to_owned(), string("wubi")),
            ("input.wubi_schema".to_owned(), string("wubi98")),
            ("input.frequency_mode".to_owned(), string("promote")),
        ]
        .into(),
    };
    let applied = local
        .apply_settings(&wubi, &fields, exported.revision)
        .unwrap();
    assert!(applied.deferred);
    let requested = applied.writes.ios_native.unwrap();
    // 宿主写键盘设置的时候，用户在应用的设置页里把词频方式改成了不调频。
    let store = PreferencesStore::new(directory.path());
    let current = store.load().unwrap();
    let mut edited = current.preferences.clone();
    edited.frequency.mode = FrequencyMode::Disabled;
    store.save(current.revision, edited).unwrap();
    assert_eq!(
        local
            .commit_settings(&wubi, &requested, exported.revision)
            .err(),
        Some("conflict".to_owned())
    );
    let kept = store.load().unwrap().preferences;
    assert_eq!(kept.frequency.mode, FrequencyMode::Disabled);
    assert_ne!(kept.scheme, InputScheme::Wubi);
}

#[test]
fn an_ios_commit_after_a_local_settings_change_is_voided() {
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
    assert_eq!(pending["outcome"], "pending_commit", "{pending}");
    let run_id = pending["run_id"].as_str().unwrap().to_owned();
    // 宿主写键盘设置的时候，另一个进程里用户改了共享偏好。
    fixture.local.local().settings_revision += 1;
    assert_eq!(
        commit(
            &fixture.session,
            &fixture.store,
            &fixture.local,
            &run_id,
            Some(&native()),
            NOW + 1,
        ),
        Err("conflict".to_owned())
    );
    assert!(fixture.local.local().committed.is_empty());
    let state = fixture.state();
    assert!(state.run.is_none());
    assert_eq!(state.sections.settings.cursor, None);
    assert!(state.sections.settings.dirty());
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
    // 有没有词与合并时看到的相同：词条全被删除记录盖掉的快照算没有词。
    assert_eq!(local.snapshot_has_words(&file), Ok(true));
    let emptied = directory.path().join("emptied.ndjson");
    let text: String = [&lines[0], &lines[2], &lines[3], &lines[4]]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(&emptied, text).unwrap();
    assert_eq!(local.snapshot_has_words(&emptied), Ok(false));
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

#[test]
fn a_word_edited_in_the_cloud_panel_is_merged_rather_than_dropped() {
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let work = directory.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let file = work.join("download.ndjson");
    // 云词库面板把「你好」改成「你号」：服务端保留词条 id，旧词写成删除的覆盖层，新词写成生效的覆盖层，三条记录共用 id `x`。
    let lines = [
        json!({"type": "header", "format": "msime-dictionary-snapshot", "version": 1, "revision": 2}),
        json!({"type": "entry", "data": {"id": "x", "kind": "pinyin", "code": "ni'hao", "word": "你号", "weight": 3, "revision": 2, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "overlay", "deleted": true, "data": {"id": "x", "kind": "pinyin", "code": "ni'hao", "word": "你好", "weight": 3, "revision": 1, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "overlay", "deleted": false, "data": {"id": "x", "kind": "pinyin", "code": "ni'hao", "word": "你号", "weight": 3, "revision": 2, "updated_at": "2026-09-01T00:00:00Z"}}),
        json!({"type": "footer", "sha256": "0".repeat(64)}),
    ];
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(&file, text).unwrap();
    let edited = policy::Word {
        kind: "pinyin".into(),
        key: "ni'hao".into(),
        value: "你号".into(),
        weight: 3,
    };
    assert_eq!(snapshot_words(&file).unwrap(), [edited]);
    // 改后的词还不在本机：排进导入队列，合并没完，不能整份上传把它从云端删掉。
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_PENDING.to_owned())
    );
    let store = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    let queued: Vec<String> = store
        .read()
        .unwrap()
        .requests
        .iter()
        .filter_map(|request| request.replacement.as_ref().map(|word| word.value.clone()))
        .collect();
    assert_eq!(queued, ["你号"]);
    drain_personal_queue(&options);
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Ok(())
    );
}

/// 合成的云端快照：`count` 个两字拼音词，外加一个重复的词。不合规的词见 `cloud_words_that_never_reach_this_device_keep_the_merge_from_finishing`。
fn cloud_snapshot(file: &Path, count: usize) {
    // 合成的主词库只有 `tbl_2_n`：第一个音节都以 n 开头。
    const FIRST: [(&str, &str); 16] = [
        ("ni", "你"),
        ("na", "拿"),
        ("nu", "怒"),
        ("ne", "呢"),
        ("nan", "南"),
        ("nai", "奶"),
        ("nao", "脑"),
        ("nei", "内"),
        ("nen", "嫩"),
        ("neng", "能"),
        ("nin", "您"),
        ("niu", "牛"),
        ("nian", "年"),
        ("niao", "鸟"),
        ("nong", "农"),
        ("nuo", "诺"),
    ];
    const SECOND: [(&str, &str); 16] = [
        ("ni", "你"),
        ("hao", "好"),
        ("shui", "水"),
        ("shan", "杉"),
        ("ma", "马"),
        ("da", "大"),
        ("la", "拉"),
        ("ka", "卡"),
        ("ta", "他"),
        ("pa", "怕"),
        ("fa", "发"),
        ("ha", "哈"),
        ("sa", "撒"),
        ("wa", "瓦"),
        ("ya", "压"),
        ("zha", "扎"),
    ];
    let entry = |id: String, code: String, word: String| json!({"type": "entry", "data": {"id": id, "kind": "pinyin", "code": code, "word": word, "weight": 7, "revision": 1, "updated_at": "2026-09-01T00:00:00Z"}});
    let mut lines =
        vec![json!({"type": "header", "format": "msime-dictionary-snapshot", "version": 1})];
    for index in 0..count {
        let (first, first_word) = FIRST[index / SECOND.len()];
        let (second, second_word) = SECOND[index % SECOND.len()];
        lines.push(entry(
            format!("w{index}"),
            format!("{first}'{second}"),
            format!("{first_word}{second_word}"),
        ));
    }
    lines.push(entry("dup".into(), "ni'ni".into(), "你你".into()));
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(file, text).unwrap();
}

/// 像键盘那样把个人词库队列里排着的请求都写进 Engine。
fn drain_personal_queue(options: &Value) {
    let request = options.to_string();
    for _ in 0..200 {
        let synced = crate::dictionary::personal_dictionary_sync_json(request.as_bytes()).unwrap();
        if synced["pending_count"] == 0 {
            let store = PersonalDictionaryStore::new(
                Path::new(options["preferences_directory"].as_str().unwrap())
                    .join("PersonalDictionary"),
            );
            let state = store.read().unwrap();
            assert!(
                state
                    .requests
                    .iter()
                    .all(|request| request.status == PersonalWordRequestStatus::Applied),
                "{:?}",
                state
                    .requests
                    .iter()
                    .find(|request| request.status != PersonalWordRequestStatus::Applied)
            );
            return;
        }
    }
    panic!("the personal dictionary queue did not drain");
}

/// Android 本地备份恢复的大批词先进命名词库的待发送队列，由键盘空闲时一批批送进个人词库导入队列。个人词库队列已经清空、待发送队列里还有词时，这些词还不在本机快照里，词库不能上传（#8201 迁移计划 5d，与 Android 原来的 Java 驱动相同）。
#[test]
fn words_still_in_the_collections_outbox_block_the_dictionary_upload() {
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let file = directory.path().join("backup.ndjson");
    cloud_snapshot(&file, 200);
    let words: Vec<PersonalWord> = snapshot_words(&file)
        .unwrap()
        .iter()
        .filter_map(import_word)
        .collect();
    let collections = DictionaryCollectionsStore::new(
        directory.path(),
        PersonalDictionaryStore::new(directory.path().join("PersonalDictionary")),
    );
    let queued = collections.queue_words(words).unwrap().queued.unwrap();
    assert!(queued > policy::PERSONAL_IMPORT_BATCH, "{queued}");
    drain_personal_queue(&options);
    let personal = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    assert_eq!(personal.read().unwrap().pending_count(), 0);
    let outbox = collections.load().unwrap().outbox_pending;
    assert!(outbox > 0, "the outbox kept words for the next idle flush");
    let probe = local.dictionary(None).unwrap();
    assert_eq!(probe.pending, outbox);
    assert!(policy::upload_blocked(
        probe.pending,
        probe.activation == SnapshotActivation::Waiting
    ));
}

#[test]
fn merging_more_cloud_words_than_the_queue_holds_continues_over_several_rounds() {
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let work = directory.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let file = work.join("download.ndjson");
    cloud_snapshot(&file, 200);
    let store = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    // 第一轮只排得下 128 个：这一类失败，没排进去的词不能当成合并完了。排进了新的词，合并在往前走。
    let pending = |queued, present| {
        Err(MergeStop {
            reason: MERGE_PENDING.to_owned(),
            queued,
            present: Some(present),
        })
    };
    assert_eq!(local.merge_dictionary(&file), pending(true, 0));
    assert_eq!(store.read().unwrap().pending_count(), 128);
    assert!(!work.join("merge-local.ndjson").exists());
    // 键盘还没应用：再合并一次不重复排、不报冲突，仍然没完，也不能上传；这一轮什么都没排，退避要加长。
    assert_eq!(local.merge_dictionary(&file), pending(false, 0));
    assert_eq!(store.read().unwrap().requests.len(), 128);
    let probe = local.dictionary(None).unwrap();
    assert_eq!(probe.pending, 128);
    assert!(policy::upload_blocked(
        probe.pending,
        probe.activation == SnapshotActivation::Waiting
    ));
    // 键盘应用以后，下一轮跳过已经在本机的 128 个词，把剩下的 72 个排上；排着的词还没进本机，合并仍然没完。
    drain_personal_queue(&options);
    assert_eq!(local.merge_dictionary(&file), pending(true, 128));
    assert_eq!(store.read().unwrap().pending_count(), 72);
    drain_personal_queue(&options);
    // 全都在本机了：合并完，什么也不排。
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Ok(())
    );
    assert_eq!(store.read().unwrap().pending_count(), 0);
    let count = local
        .dictionary_call(json!({ "operation": "count", "user_only": true }))
        .unwrap();
    assert_eq!(count["count"], 200, "{count}");
}

#[test]
fn a_downloaded_snapshot_is_reported_waiting_activated_or_lost() {
    use msime_client_core::cloud::snapshot_queue::local_version;
    use sha2::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let queue_directory = directory.path().join("queue");
    let local = HostLocal {
        options: Some(options),
        snapshot_queue: Some(queue_directory.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let activation = |awaited: Option<&str>| local.dictionary(awaited).unwrap().activation;
    assert_eq!(activation(None), SnapshotActivation::Idle);
    let queue = DictionarySnapshotQueue::new(queue_directory).unwrap();
    let published = local_version(None, &"0".repeat(64)).unwrap();
    queue.publish_local_version(&published).unwrap();
    let file = directory.path().join("download.ndjson");
    std::fs::write(&file, "synthetic snapshot\n").unwrap();
    let sha256 = hex::encode(Sha256::digest(std::fs::read(&file).unwrap()));
    let first = queue
        .enqueue(&file, USER, 4, &published, &sha256, true)
        .unwrap();
    let first_id = first.to_string();
    assert_eq!(activation(Some(&first_id)), SnapshotActivation::Waiting);
    assert_eq!(local.dictionary(None).unwrap().pending, 0);
    // 激活时本机版本已经变了：请求以冲突结束，宿主读过终态以后请求本身也不在了。
    let typed = local_version(None, &"1".repeat(64)).unwrap();
    {
        let lease = queue.acquire_worker_lease().unwrap();
        assert!(queue.claim(&lease, Some(USER)).unwrap().is_some());
        let applied = queue
            .complete(first, &lease, &typed, false, Some(USER), || {
                unreachable!("a conflicting request is not applied")
            })
            .unwrap();
        assert!(!applied);
    }
    assert_eq!(activation(Some(&first_id)), SnapshotActivation::Lost);
    assert_eq!(activation(None), SnapshotActivation::Idle);
    queue.take_state().unwrap();
    assert_eq!(activation(Some(&first_id)), SnapshotActivation::Lost);
    // 下一份激活成功：本机版本换成以它的 id 为代次的版本，之后键盘发布的新版本也沿用这个代次。
    let second = queue
        .enqueue(&file, USER, 5, &typed, &sha256, true)
        .unwrap();
    let second_id = second.to_string();
    let activated = local_version(Some(&second_id), &"2".repeat(64)).unwrap();
    {
        let lease = queue.acquire_worker_lease().unwrap();
        assert!(queue.claim(&lease, Some(USER)).unwrap().is_some());
        let applied = queue
            .complete(second, &lease, &typed, false, Some(USER), || {
                Ok(activated.clone())
            })
            .unwrap();
        assert!(applied);
    }
    assert_eq!(activation(Some(&second_id)), SnapshotActivation::Idle);
    queue.take_state().unwrap();
    let typed_again = local_version(Some(&second_id), &"3".repeat(64)).unwrap();
    queue.publish_local_version(&typed_again).unwrap();
    assert_eq!(activation(Some(&second_id)), SnapshotActivation::Idle);
    assert_eq!(activation(Some(&first_id)), SnapshotActivation::Lost);
    // 退出登录取消了排着的快照：不再等它，记着的那份算没激活。
    let third = queue
        .enqueue(&file, USER, 6, &typed_again, &sha256, true)
        .unwrap();
    assert_eq!(activation(None), SnapshotActivation::Waiting);
    local.cancel_dictionary(USER).unwrap();
    assert_eq!(
        activation(Some(&third.to_string())),
        SnapshotActivation::Lost
    );
}

/// 合成的云端快照，每个词是 (种类, 编码, 词, 权重)。
fn snapshot_with(file: &Path, words: &[(&str, &str, &str, i64)]) {
    let mut lines =
        vec![json!({"type": "header", "format": "msime-dictionary-snapshot", "version": 1})];
    for (index, (kind, code, word, weight)) in words.iter().enumerate() {
        lines.push(json!({"type": "entry", "data": {"id": format!("w{index}"), "kind": kind, "code": code, "word": word, "weight": weight, "revision": 1, "updated_at": "2026-09-01T00:00:00Z"}}));
    }
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(file, text).unwrap();
}

/// 像键盘那样应用排着的请求，但每个都以 `error` 失败。
fn fail_personal_queue(store: &PersonalDictionaryStore, error: &str) {
    store
        .synchronize(
            |_| Err(error.to_owned()),
            |_| {
                Ok(
                    msime_client_services::dictionary::personal::PersonalWordPage {
                        entries: Vec::new(),
                        has_more: false,
                    },
                )
            },
        )
        .unwrap();
}

#[test]
fn cloud_words_that_never_reach_this_device_keep_the_merge_from_finishing() {
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let work = directory.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let file = work.join("download.ndjson");
    let store = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    // 别的设备的英文学习把权重抬过了新词的上限，服务端也收权重 0：照样导进来，只是权重收进范围。
    snapshot_with(
        &file,
        &[
            ("english", "hello", "hello", 200_000_000),
            ("pinyin", "ni'hao", "你好", 0),
        ],
    );
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_PENDING.to_owned())
    );
    let weights: Vec<i64> = store
        .read()
        .unwrap()
        .requests
        .iter()
        .filter_map(|request| request.replacement.as_ref().map(|word| word.weight))
        .collect();
    assert_eq!(weights, [MAX_IMPORT_WEIGHT, 1]);
    // 键盘还没应用：词都排上了，但还不在本机，合并仍然没完。
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_PENDING.to_owned())
    );
    assert_eq!(store.read().unwrap().requests.len(), 2);
    drain_personal_queue(&options);
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Ok(())
    );

    // 旧版本留下的带数字的快捷短语编码、带控制字符的词，本机新词的规则不收：不能当成合并完了。
    for word in [
        ("quick", "dz1", "地址", 1),
        ("pinyin", "ni'huai", "你坏\u{7}", 1),
    ] {
        snapshot_with(&file, &[word]);
        assert_eq!(
            local.merge_dictionary(&file).map_err(|stop| stop.reason),
            Err(MERGE_REJECTED.to_owned()),
            "{word:?}"
        );
        assert_eq!(store.read().unwrap().pending_count(), 0);
    }

    // 键盘应用时暂时拿不到词库：重排一次，合并仍然没完。重排的是上一轮排过的词，不算排进了新的词，本机也没多出云端的词：退避照样翻倍。
    let words = [
        ("pinyin", "ni'ma", "你马", 3),
        ("pinyin", "na'ma", "拿马", 3),
        ("pinyin", "nu'ma", "怒马", 3),
    ];
    snapshot_with(&file, &words);
    let pending = |queued| {
        Err(MergeStop {
            reason: MERGE_PENDING.to_owned(),
            queued,
            present: Some(0),
        })
    };
    assert_eq!(local.merge_dictionary(&file), pending(true));
    for _ in 0..2 {
        fail_personal_queue(&store, "dictionary maintenance busy");
        assert_eq!(store.read().unwrap().pending_count(), 0);
        assert_eq!(local.merge_dictionary(&file), pending(false));
        assert_eq!(store.read().unwrap().pending_count(), 3);
        assert_eq!(store.read().unwrap().requests.len(), 3 + 2);
    }
    // 键盘拒了这些词（例如本机方案存不了中文词）：再排还会被拒，这一类停下来说明原因，不上传。
    fail_personal_queue(&store, crate::NO_CHINESE_DICTIONARY);
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_REJECTED.to_owned())
    );
    assert_eq!(store.read().unwrap().pending_count(), 0);
    let probe = local.dictionary(None).unwrap();
    assert!(!policy::upload_blocked(
        probe.pending,
        probe.activation == SnapshotActivation::Waiting
    ));

    // 用户自己加的词没保存成功、和云端的词是同一个：队列不收，要用户处理。
    let directory = tempfile::tempdir().unwrap();
    let options = crate::tests::host_options_with_main_dictionary(directory.path());
    let local = HostLocal {
        options: Some(options),
        ..host_local(directory.path(), HostInputs::default())
    };
    let file = directory.path().join("download.ndjson");
    let store = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    let mine = crate::dictionary::personal_import_word("pinyin", "ni'ma", "你马", 3).unwrap();
    store.enqueue(None, Some(mine), "user-add".into()).unwrap();
    fail_personal_queue(&store, "dictionary edit rejected");
    snapshot_with(&file, &words[..1]);
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_BLOCKED.to_owned())
    );
    assert_eq!(store.read().unwrap().requests.len(), 1);

    // 队列被别的失败请求占满，又没有排着的请求能腾出位置：等下去也排不进云端的词。
    store.dismiss_failure("user-add").unwrap();
    let letters = |index: usize| {
        let letter = |value: usize| char::from(b'a' + (value % 26) as u8);
        format!("x{}{}", letter(index / 26), letter(index))
    };
    let failed: Vec<PersonalWord> = (0..policy::PERSONAL_IMPORT_BATCH)
        .map(|index| {
            let word = letters(index);
            crate::dictionary::personal_import_word("english", &word, &word, 5).unwrap()
        })
        .collect();
    store.enqueue_import(failed, "user-import".into()).unwrap();
    while store.read().unwrap().pending_count() > 0 {
        fail_personal_queue(&store, "dictionary edit rejected");
    }
    assert_eq!(
        local.merge_dictionary(&file).map_err(|stop| stop.reason),
        Err(MERGE_BLOCKED.to_owned())
    );
    assert_eq!(store.read().unwrap().pending_count(), 0);
}

/// 常用语同步过 `甲`、`乙` 以后另一台设备「删除云端数据」，这一轮问了 `cloud_cleared`。
fn asked_after_the_cloud_was_cleared(fixture: &Fixture) {
    synced_phrases(fixture, &["甲", "乙"]);
    fixture.api.cloud().phrases = AccountPhrases {
        revision: 2,
        phrases: Vec::new(),
    };
    let asked = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 60_000);
    assert_eq!(asked["reason"], "cloud_cleared", "{asked}");
    assert_eq!(asked["status"]["status_line"], policy::CLOUD_CLEARED_LINE);
    fixture.api.calls();
}

#[test]
fn a_cleared_cloud_can_turn_sync_off_without_uploading_or_deleting_anything() {
    let fixture = Fixture::new();
    asked_after_the_cloud_was_cleared(&fixture);
    fixture.local.add_phrase("丙");
    let disabled = disable(&fixture.session, &fixture.store, NOW + 70_000).unwrap();
    assert_eq!(disabled["outcome"], "disabled", "{disabled}");
    let shown = &disabled["status"];
    assert_eq!(shown["enabled"], false);
    assert_eq!(shown["disabled_on_cloud_cleared"], true);
    assert_eq!(shown["sections"]["phrases"]["cleared"], true);
    assert_eq!(shown["status_line"], policy::DISABLED_ON_CLOUD_CLEARED_LINE);
    // 不联网：云端还是空的，本机一条都没少。
    assert!(fixture.api.calls().is_empty());
    assert!(fixture.api.cloud_texts().is_empty());
    assert_eq!(fixture.local.texts(), ["甲", "乙", "丙"]);
    // 不持有登录的进程看到的一样。
    let peeked = peek(&fixture.store, NOW + 70_000).unwrap();
    assert_eq!(peeked["disabled_on_cloud_cleared"], true);
    assert_eq!(
        peeked["status_line"],
        policy::DISABLED_ON_CLOUD_CLEARED_LINE
    );
    // 关着就不再同步，记不了改动。
    let skipped = fixture.run(Trigger::Manual, None, phrases_only(), NOW + 80_000);
    assert_eq!(skipped["reason"], "disabled", "{skipped}");
    assert!(
        !mark_dirty(&fixture.store, Section::Phrases).unwrap()["marked"]
            .as_bool()
            .unwrap()
    );
    assert!(fixture.api.calls().is_empty());
    // 用户以后自己重新打开：第一轮读完云端先问，不把本机悄悄传回被清空的云端。
    fixture.enable();
    let reopened = status(&fixture.session, &fixture.store, NOW + 90_000).unwrap();
    assert_eq!(reopened["disabled_on_cloud_cleared"], false);
    assert_eq!(reopened["status_line"], policy::CLOUD_CLEARED_LINE);
    let asked = fixture.run(Trigger::Enable, None, phrases_only(), NOW + 90_000);
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert_eq!(fixture.api.calls(), ["phrases"]);
    assert!(fixture.api.cloud_texts().is_empty());
}

#[test]
fn turning_sync_off_on_a_cleared_cloud_does_not_wait_for_a_round_and_is_never_stored() {
    let fixture = Fixture::new();
    asked_after_the_cloud_was_cleared(&fixture);
    // 另一轮还在跑时用户先点了「合并」：记成 pending_choice。
    fixture
        .store
        .update(|state| {
            state.begin_run(USER, NOW + 61_000, None);
            state.sections.dictionary.choice = Some(Choice::UseCloud);
        })
        .unwrap();
    let running = fixture.state().run.unwrap();
    let busy = fixture.run(
        Trigger::Choice,
        Some(Choice::Merge),
        phrases_only(),
        NOW + 62_000,
    );
    assert_eq!(busy["reason"], "busy", "{busy}");
    assert_eq!(fixture.state().pending_choice, Some(Choice::Merge));
    // 接着又点了「关闭同步」：不等那一轮，马上关掉，那一轮因此提交不了；它不记成任何选择，记着的旧选择也丢掉。
    let disabled = disable(&fixture.session, &fixture.store, NOW + 63_000).unwrap();
    assert_eq!(disabled["outcome"], "disabled", "{disabled}");
    assert_eq!(disabled["status"]["pending_choice"], Value::Null);
    let state = fixture.state();
    assert!(!state.enabled);
    assert_eq!(state.pending_choice, None);
    assert_eq!(state.sections.dictionary.choice, None);
    assert!(!state.lease_matches(&running.id, USER));
    assert!(fixture.api.calls().is_empty());
    // 重新打开、那一轮的租约过期以后手动同步：记着的「合并」没有留下来替用户做决定，照样先问。
    fixture.enable();
    let asked = fixture.run(
        Trigger::Manual,
        None,
        phrases_only(),
        NOW + 61_000 + policy::LEASE_MS,
    );
    assert_eq!(asked["outcome"], "needs_choice", "{asked}");
    assert_eq!(asked["reason"], "cloud_cleared");
    assert!(fixture.api.cloud_texts().is_empty());
    assert_eq!(fixture.local.texts(), ["甲", "乙"]);
}

#[test]
fn turning_sync_off_only_answers_a_cleared_cloud() {
    // 同步过、云端没有被清空：不关，开关和进度都不动。
    let fixture = Fixture::new();
    synced_phrases(&fixture, &["甲"]);
    let before = fixture.state();
    let refused = disable(&fixture.session, &fixture.store, NOW + 60_000).unwrap();
    assert_eq!(refused["outcome"], "skipped", "{refused}");
    assert_eq!(refused["reason"], "not_cloud_cleared");
    assert_eq!(refused["status"]["enabled"], true);
    assert_eq!(fixture.state(), before);
    // 首次同步的提示（云端已有数据）也没有这个回答。
    let fixture = Fixture::new();
    fixture.api.cloud().phrases = cloud_phrases(3, &["云端"]);
    fixture.local.add_phrase("本机");
    fixture.enable();
    let asked = fixture.run(Trigger::Enable, None, phrases_only(), NOW);
    assert_eq!(asked["reason"], "first_run", "{asked}");
    let refused = disable(&fixture.session, &fixture.store, NOW + 1_000).unwrap();
    assert_eq!(refused["reason"], "not_cloud_cleared", "{refused}");
    assert!(fixture.state().enabled);
    // 已经关着、或者已经退出时同样不动。
    let fixture = Fixture::new();
    asked_after_the_cloud_was_cleared(&fixture);
    set_enabled(&fixture.session, &fixture.store, false, NOW + 70_000).unwrap();
    let refused = disable(&fixture.session, &fixture.store, NOW + 71_000).unwrap();
    assert_eq!(refused["reason"], "disabled", "{refused}");
    assert_eq!(refused["status"]["disabled_on_cloud_cleared"], false);
    *fixture.storage.0.lock().unwrap() = None;
    let refused = disable(&fixture.session, &fixture.store, NOW + 72_000).unwrap();
    assert_eq!(refused["reason"], "signed_out", "{refused}");
}

/// `host_options_with_main_dictionary` 加上随包的英文词库：清除学习记录要从资源目录把两份词库都复制回来。
fn resettable_host_options(root: &Path) -> Value {
    let options = crate::tests::host_options_with_main_dictionary(root);
    rusqlite::Connection::open(root.join("resources").join("msime-english.db"))
        .unwrap()
        .execute_batch("CREATE TABLE english(key TEXT,value TEXT,weight INTEGER);")
        .unwrap();
    options
}

fn add_user_word(options: &Value, key: &str, value: &str, request_id: &str) {
    let request = json!({"options": options, "action": {"operation": "edit", "previous": null, "replacement": {"kind": "pinyin", "key": key, "value": value, "weight": 100}, "request_id": request_id}});
    let edited = dictionary_request_json(request.to_string().as_bytes()).unwrap();
    assert_eq!(edited["applied"], true, "{edited}");
}

fn reset_learned_data(options: &Value) -> Value {
    let request = json!({"options": options, "action": {"operation": "reset"}});
    dictionary_request_json(request.to_string().as_bytes()).unwrap()
}

fn user_words(options: &Value) -> Vec<String> {
    let request = json!({"options": options, "action": {"operation": "list", "offset": 0, "limit": 100, "user_only": true}});
    let listed = dictionary_request_json(request.to_string().as_bytes()).unwrap();
    listed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["value"].as_str().unwrap().to_owned())
        .collect()
}

/// 快照里每个词一条 `entry`。
fn snapshot_entries(snapshot: &str) -> Vec<String> {
    snapshot
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|record| record["type"] == "entry")
        .map(|record| record["data"]["word"].as_str().unwrap().to_owned())
        .collect()
}

/// 设置页「清除学习记录」：重置以后宿主给词库分类打上待上传标记，下一轮同步把清除后的一份整份传上去，替换云端那份。不打标记时两边都没变，这一轮什么也不传，云端留着清除前的词。
#[test]
fn a_learned_data_reset_marked_for_upload_replaces_the_cloud_copy() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let options = resettable_host_options(directory.path());
    add_user_word(&options, "ni'hao", "你好", "seed-word");
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    let round = |trigger: Trigger, now: u64| {
        run(
            &fixture.session,
            &fixture.store,
            &local,
            RunRequest {
                trigger,
                choice: None,
                capabilities: dictionary,
                settings_locked: false,
                excluded_phrase_texts: Vec::new(),
            },
            now,
        )
        .unwrap()
    };
    // 首次同步，云端没有词：上传本机这一份。
    fixture.api.cloud().dictionary_items = 0;
    fixture.enable();
    let first = round(Trigger::Enable, NOW);
    assert_eq!(
        first["sections"]["dictionary"]["outcome"], "synced",
        "{first}"
    );
    assert_eq!(
        snapshot_entries(&fixture.api.cloud().uploaded_snapshots[0]),
        ["你好"]
    );

    let reset = reset_learned_data(&options);
    assert_eq!(reset, json!({"reset": true, "collections_requeued": 0}));
    assert!(user_words(&options).is_empty());
    let unmarked = round(Trigger::Manual, NOW + 60_000);
    assert_eq!(unmarked["outcome"], "done", "{unmarked}");
    assert_eq!(fixture.api.cloud().snapshot_uploads, 1);

    assert_eq!(
        dictionary_reset(&fixture.store, &local).unwrap()["marked"],
        true
    );
    let marked = round(Trigger::Manual, NOW + 120_000);
    assert_eq!(
        marked["sections"]["dictionary"]["outcome"], "synced",
        "{marked}"
    );
    let cloud = fixture.api.cloud();
    assert_eq!(cloud.snapshot_uploads, 2);
    assert!(snapshot_entries(&cloud.uploaded_snapshots[1]).is_empty());
    drop(cloud);
    assert!(!fixture.state().sections.dictionary.dirty());
}

/// 清除学习记录清掉用户自己的词，已启用命名词库的词重新排队、送回词库，停用的不送；之后上传的那一份里只有它们。
#[test]
fn a_learned_data_reset_keeps_the_words_of_enabled_named_dictionaries() {
    let directory = tempfile::tempdir().unwrap();
    let options = resettable_host_options(directory.path());
    let collections = DictionaryCollectionsStore::new(
        directory.path(),
        PersonalDictionaryStore::new(directory.path().join("PersonalDictionary")),
    );
    let pinyin = msime_client_services::dictionary::personal::PersonalWordKind::Pinyin;
    let word = |key: &str, value: &str| PersonalWord {
        kind: pinyin,
        key: key.into(),
        value: value.into(),
        weight: 10_000,
    };
    let enabled = collections.create("常用", pinyin).unwrap().collections[0]
        .id
        .to_string();
    collections
        .add_words(&enabled, vec![word("ni'hao", "你好")])
        .unwrap();
    let view = collections.create("停用", pinyin).unwrap();
    let idle = view
        .collections
        .iter()
        .find(|item| item.name == "停用")
        .unwrap()
        .id
        .to_string();
    collections
        .add_words(&idle, vec![word("niu'nai", "牛奶")])
        .unwrap();
    drain_personal_queue(&options);
    // 「删除」要等「加入」应用过才送，所以先排空一次再停用。
    collections.set_enabled(&idle, false).unwrap();
    drain_personal_queue(&options);
    add_user_word(&options, "nai'cha", "奶茶", "own-word");
    let mut before = user_words(&options);
    before.sort();
    assert_eq!(before, ["你好", "奶茶"]);

    let reset = reset_learned_data(&options);
    assert_eq!(reset, json!({"reset": true, "collections_requeued": 1}));
    assert!(user_words(&options).is_empty());
    let local = HostLocal {
        options: Some(options.clone()),
        ..host_local(directory.path(), HostInputs::default())
    };
    // 送回去之前不能上传：那样云端会丢掉命名词库的词。
    assert!(local.dictionary(None).unwrap().pending > 0);

    drain_personal_queue(&options);
    assert_eq!(user_words(&options), ["你好"]);
    assert_eq!(local.dictionary(None).unwrap().pending, 0);
    let file = directory.path().join("export.ndjson");
    local.export_dictionary(&file).unwrap();
    assert_eq!(
        snapshot_entries(&std::fs::read_to_string(&file).unwrap()),
        ["你好"]
    );
}

/// 清除学习记录时还有一份下载下来的云端快照在等键盘激活：重置以后本机版本变了，它以冲突结束、不会激活。只打待上传标记时下一轮把它当成没激活的快照，按合并把云端清除前的词并回本机；`dictionary_reset` 取消它、忘掉这条记录，下一轮整份上传清除后的这一份。
#[test]
fn a_learned_data_reset_forgets_a_snapshot_still_waiting_for_activation() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    for forget in [false, true] {
        let fixture = Fixture::new();
        fixture.local.local().words = true;
        fixture.enable();
        let uploaded = fixture.run(Trigger::Enable, None, dictionary, NOW);
        assert_eq!(uploaded["outcome"], "done", "{uploaded}");
        // 另一台设备改了云端：下载下来交给激活队列，键盘还没激活。
        fixture.api.cloud().dictionary_revision = 5;
        let downloaded = fixture.run(Trigger::Manual, None, dictionary, NOW + 60_000);
        assert_eq!(
            fixture.local.local().replaced,
            [(USER.to_owned(), 5)],
            "{downloaded}"
        );
        assert!(fixture.state().sections.dictionary.activation.is_some());

        if forget {
            let reset = dictionary_reset(&fixture.store, &fixture.local).unwrap();
            assert_eq!(reset["marked"], true);
            assert_eq!(fixture.local.local().cancelled, [USER]);
            assert_eq!(fixture.state().sections.dictionary.activation, None);
        } else {
            store_mark(&fixture, Section::Dictionary);
        }
        fixture.local.finish_activation(false);
        let next = fixture.run(Trigger::Manual, None, dictionary, NOW + 120_000);
        assert_eq!(
            next["sections"]["dictionary"]["outcome"], "synced",
            "{next}"
        );
        let merged = fixture.local.local().merged;
        let uploads = fixture.api.cloud().snapshot_uploads;
        if forget {
            assert_eq!((merged, uploads), (0, 2), "the cleared copy is uploaded");
            assert!(!fixture.state().sections.dictionary.dirty());
        } else {
            assert_eq!((merged, uploads), (1, 1), "the old cloud words come back");
        }
    }
}

/// 两台设备登录同一账号、词库都同步过。A 清除学习记录以后整份上传（服务端整份恢复快照时写一条 reset），B 下一轮：A 上传的快照是空的（没有命名词库）时，云端看起来和「删除云端数据」一样被清空了，B 停下来问，不替换也不上传；快照里有词（命名词库的词）时，没有本机改动的 B 照常下载，用这一份整份替换自己的词库。确认框因此说登录同一账号的其他设备会跟着换掉，或者提示选择。
#[test]
fn a_learned_data_reset_reaches_the_other_devices_on_the_account() {
    let dictionary = Capabilities {
        dictionary: true,
        ..Capabilities::default()
    };
    for items in [0, 3] {
        let a = Fixture::new();
        let b = a.another_device();
        a.local.local().words = true;
        a.enable();
        let first = a.run(Trigger::Enable, None, dictionary, NOW);
        assert_eq!(a.api.cloud().snapshot_uploads, 1, "{first}");
        b.local.local().words = true;
        b.enable();
        let merged = b.run(Trigger::Choice, Some(Choice::Merge), dictionary, NOW);
        assert_eq!(merged["outcome"], "done", "{merged}");
        // B 合并完、本机的词也传上去以后两边一致：这里直接清掉 B 的标记，免得多一轮上传。
        b.store
            .update(|state| {
                let section = &mut state.sections.dictionary;
                section.clean = section.generation;
            })
            .unwrap();

        a.local.local().words = items > 0;
        assert_eq!(
            dictionary_reset(&a.store, &a.local).unwrap()["marked"],
            true
        );
        let cleared = a.run(Trigger::Manual, None, dictionary, NOW + 60_000);
        assert_eq!(
            cleared["sections"]["dictionary"]["outcome"], "synced",
            "{cleared}"
        );
        {
            let mut cloud = a.api.cloud();
            assert_eq!(cloud.snapshot_uploads, 2);
            cloud.dictionary_reset = true;
            cloud.dictionary_items = items;
        }

        let other = b.run(Trigger::Manual, None, dictionary, NOW + 120_000);
        if items == 0 {
            assert_eq!(other["outcome"], "needs_choice", "{other}");
            assert_eq!(other["reason"], "cloud_cleared");
            assert!(b.local.local().replaced.is_empty());
        } else {
            assert_eq!(
                other["sections"]["dictionary"]["outcome"], "synced",
                "{other}"
            );
            assert_eq!(b.local.local().replaced, [(USER.to_owned(), 2)]);
        }
        assert_eq!(a.api.cloud().snapshot_uploads, 2);
    }
}

/// 本地备份恢复排下、还没合并的输入记录和输入习惯（含键盘认领了还没合并完的和失败计数）在清除时一起删掉，不会在键盘下次空闲时把清除前的学习并回来。
#[test]
fn a_learned_data_reset_drops_learning_still_waiting_to_be_merged() {
    let directory = tempfile::tempdir().unwrap();
    let options = resettable_host_options(directory.path());
    let names = [
        "pending-learning-merge.ndjson",
        "pending-learning-merge.claimed.ndjson",
        "pending-learning-merge.attempts",
        "pending-habits-merge.ndjson",
        "pending-habits-merge.claimed.ndjson",
        "pending-habits-merge.attempts",
    ];
    for name in names {
        std::fs::write(directory.path().join(name), "synthetic\n").unwrap();
    }
    let reset = reset_learned_data(&options);
    assert_eq!(reset, json!({"reset": true, "collections_requeued": 0}));
    for name in names {
        assert!(!directory.path().join(name).exists(), "{name}");
    }
}

/// 命名词库的索引坏了、重新排队失败时，重置已经做完：照样回答 `reset:true`，宿主据此打云同步标记，并告诉用户已添加词库的词没能写回，而不是说清除失败。
#[test]
fn a_learned_data_reset_still_succeeds_when_named_dictionaries_cannot_be_requeued() {
    let directory = tempfile::tempdir().unwrap();
    let options = resettable_host_options(directory.path());
    add_user_word(&options, "ni'hao", "你好", "seed-word");
    let collections = directory.path().join("DictionaryCollections");
    std::fs::create_dir_all(&collections).unwrap();
    std::fs::write(collections.join("index.json"), "not json").unwrap();
    let reset = reset_learned_data(&options);
    assert_eq!(
        reset,
        json!({
            "reset": true,
            "collections_requeued": null,
            "collections_error": "learned-data reset collections unavailable",
        })
    );
    assert!(user_words(&options).is_empty());
}
