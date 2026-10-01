//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;
use std::collections::HashMap;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn token(byte: u8) -> String {
    std::iter::repeat_n(char::from(byte), 64).collect()
}

fn user() -> AccountUser {
    AccountUser {
        id: "fixture-user".into(),
        display_name: "Fixture".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        email: None,
        avatar_url: None,
    }
}

fn tokens(access: u8, refresh: u8, expires_in: u64) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token(refresh),
        token_type: "Bearer".into(),
        expires_in,
        user: user(),
    }
}

#[test]
fn validates_clipboard_boundaries() {
    let valid_id = "0123456789abcdef".repeat(4);
    assert!(validate_clipboard_id(&valid_id).is_ok());
    assert!(validate_clipboard_id(&valid_id.to_uppercase()).is_err());
    assert!(validate_clipboard_id(&format!("{valid_id}0")).is_err());

    assert!(validate_clipboard_search(&"a".repeat(1024)).is_ok());
    assert!(validate_clipboard_search(&"a".repeat(1025)).is_err());
    assert!(validate_clipboard_search("safe\u{7f}query").is_err());

    let valid_text = "😀".repeat(2000);
    assert!(validate_clipboard_text(&valid_text).is_ok());
    assert!(validate_clipboard_text(&format!("{valid_text}😀")).is_err());
    assert!(validate_clipboard_text("\n\r\t").is_err());

    let item = || AccountClipboardItem {
        id: valid_id.clone(),
        text: "fixture clipboard text".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    };
    assert!(validate_clipboard_page(&AccountClipboardPage {
        enabled: true,
        items: vec![item(); 50],
    })
    .is_ok());
    let mut too_many = vec![item(); 50];
    too_many.push(item());
    assert!(validate_clipboard_page(&AccountClipboardPage {
        enabled: true,
        items: too_many,
    })
    .is_err());
}

#[test]
fn validates_chat_catalog_and_request_boundaries() {
    let models = AccountChatModels {
        data: vec![
            AccountChatModel {
                id: "fixture-chat".into(),
            },
            AccountChatModel {
                id: "fixture-fast".into(),
            },
        ],
        default_model: "fixture-chat".into(),
    };
    assert!(validate_chat_models(&models).is_ok());

    let messages = vec![AccountChatMessage {
        role: "user".into(),
        content: "fixture message".into(),
    }];
    assert!(validate_chat_request(&messages, "fixture-chat").is_ok());
    assert!(validate_chat_request(&messages, "").is_err());
    assert!(validate_chat_request(
        &[AccountChatMessage {
            role: "tool".into(),
            content: "x".into(),
        }],
        "fixture-chat",
    )
    .is_err());
    assert!(validate_chat_request(
        &[AccountChatMessage {
            role: "user".into(),
            content: "\n".into(),
        }],
        "fixture-chat",
    )
    .is_err());
    let chat = |content: &str| {
        validate_chat_request(
            &[AccountChatMessage {
                role: "user".into(),
                content: content.into(),
            }],
            "fixture-chat",
        )
    };
    assert!(chat("第一行\n第二行").is_ok());
    assert!(chat("a\r\n\tb").is_ok());
    for rejected in ["  \n", "a\u{0}b", "a\u{1b}[31mb"] {
        assert!(chat(rejected).is_err(), "{rejected:?}");
    }
    assert!(!crate::text::has_disallowed_control("第一段\n\n第二段"));
    assert!(crate::text::has_disallowed_control("a\u{7f}"));

    let mut duplicate = models.clone();
    duplicate.data.push(AccountChatModel {
        id: "fixture-chat".into(),
    });
    assert!(validate_chat_models(&duplicate).is_err());
}

#[test]
fn validates_dictionary_boundaries() {
    let valid_id = "0123456789abcdef".repeat(4);
    assert_eq!(
        crate::cloud::dictionary::dictionary_path(DictionaryKind::Pinyin, 2, "ni hao").unwrap(),
        "/v1/users/me/dictionaries/pinyin?q=ni%20hao&offset=2&limit=100"
    );
    assert_eq!(
        dictionary_catalog_path(DictionaryKind::Pinyin, "nihc", 0, "shuangpin", "xiaohe")
            .unwrap(),
        "/v1/users/me/dictionaries/pinyin/catalog?q=nihc&offset=0&limit=100&scheme=shuangpin&profile=xiaohe"
    );
    assert!(
        crate::cloud::dictionary::dictionary_path(DictionaryKind::Wubi, 1_000_001, "").is_none()
    );
    assert!(dictionary_catalog_path(DictionaryKind::Pinyin, "", 1_000_001, "pinyin", "x").is_err());
    assert!(validate_dictionary_catalog_query("", 0, "", "x").is_err());
    assert!(validate_dictionary_id(&valid_id).is_ok());
    assert!(validate_dictionary_id(&valid_id.to_uppercase()).is_err());

    assert!(validate_dictionary_value(DictionaryKind::Pinyin, "ni' hao", "你好", 1).is_ok());
    assert!(validate_dictionary_value(DictionaryKind::Wubi, "abcd", "字", 0).is_ok());
    assert!(validate_dictionary_value(DictionaryKind::Wubi, "abcde", "字", 0).is_err());
    assert!(validate_dictionary_value(DictionaryKind::Quick, "k2", &"字".repeat(199), 1).is_ok());
    assert!(validate_dictionary_value(DictionaryKind::Quick, "k2", &"字".repeat(200), 1).is_err());
    // Inbound rows tolerate a digit in a quick phrase code; a value this client writes does not.
    assert!(validate_new_dictionary_value(DictionaryKind::Quick, "k2", "字", 1).is_err());
    assert!(validate_new_dictionary_value(DictionaryKind::Quick, "kk", "字", 1).is_ok());
    assert!(validate_dictionary_value(DictionaryKind::English, "hello", "word", 1).is_ok());
    assert!(validate_dictionary_value(DictionaryKind::English, "hello1", "word", 1).is_err());
    assert!(validate_dictionary_import(DictionaryKind::Pinyin, "hans", "你好").is_ok());
    assert!(validate_dictionary_import(DictionaryKind::Wubi, "hans", "你好").is_err());
    assert!(validate_dictionary_import(DictionaryKind::Pinyin, "standard", "bad\u{0001}").is_err());

    let entry = || AccountDictionaryEntry {
        id: valid_id.clone(),
        kind: DictionaryKind::Pinyin,
        code: "ni".into(),
        word: "你".into(),
        weight: 1,
        revision: 2,
    };
    assert!(validate_dictionary_page(
        &AccountDictionaryPage {
            entries: vec![entry(); 100],
            has_more: true,
            offset: 0,
        },
        DictionaryKind::Pinyin
    )
    .is_ok());
    assert!(validate_dictionary_page(
        &AccountDictionaryPage {
            entries: vec![entry(); 101],
            has_more: true,
            offset: 0,
        },
        DictionaryKind::Pinyin
    )
    .is_err());
    assert!(validate_dictionary_catalog_page(
        &AccountDictionaryCatalogPage {
            entries: vec![AccountDictionaryCatalogEntry {
                kind: DictionaryKind::Pinyin,
                code: "ni".into(),
                word: "你".into(),
                weight: 1,
            }],
            has_more: false,
            offset: 0,
            revision: 2,
            normalized: "ni".into(),
        },
        DictionaryKind::Pinyin
    )
    .is_ok());
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl AccountSessionStorage for MemoryStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| AccountError::Storage)
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.0.lock().map_err(|_| AccountError::Storage)? = Some(session.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), AccountError> {
        *self.0.lock().map_err(|_| AccountError::Storage)? = None;
        Ok(())
    }
}

#[derive(Clone)]
struct FakeApi {
    refreshes: Arc<AtomicUsize>,
    reject_refresh: Arc<AtomicBool>,
    refresh_gate: Option<Arc<(Mutex<bool>, Condvar)>>,
    logins: Arc<Mutex<Vec<(String, String)>>>,
}

impl FakeApi {
    fn new() -> Self {
        Self {
            refreshes: Arc::new(AtomicUsize::new(0)),
            reject_refresh: Arc::new(AtomicBool::new(false)),
            refresh_gate: None,
            logins: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

const GOOGLE_FIXTURE_STATE: &str = "fixture-state_0123456789";

fn google_authorization_url(target: &str, state: &str) -> String {
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", "fixture-client.apps.googleusercontent.com")
        .append_pair("redirect_uri", target)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("nonce", "fixture-nonce")
        .append_pair("state", state)
        .append_pair("code_challenge", "fixture-challenge")
        .append_pair("code_challenge_method", "S256");
    url.to_string()
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        Ok(HashMap::from([("email".into(), true)]))
    }

    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        Ok(AccountChallenge {
            challenge_id: "fixture-challenge".into(),
            expires_in: 300,
            nonce: None,
            authorization_url: (provider == "google")
                .then(|| google_authorization_url(target, GOOGLE_FIXTURE_STATE)),
        })
    }

    fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
        self.logins
            .lock()
            .unwrap()
            .push((challenge.into(), credential.into()));
        Ok(tokens(b'a', b'b', 900))
    }

    fn refresh(&self, _refresh_token: &str) -> Result<AccountTokens, AccountError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.refresh_gate {
            let (lock, ready) = &**gate;
            let mut open = lock.lock().map_err(|_| AccountError::Unavailable)?;
            while !*open {
                open = ready.wait(open).map_err(|_| AccountError::Unavailable)?;
            }
        }
        if self.reject_refresh.load(Ordering::SeqCst) {
            Err(AccountError::Unauthorized)
        } else {
            Ok(tokens(b'c', b'd', 900))
        }
    }

    fn profile(&self, _access_token: &str) -> Result<AccountProfile, AccountError> {
        Ok(AccountProfile {
            user: user(),
            identities: vec![AccountProfileIdentity {
                provider: "email".into(),
                subject: "masked-fixture".into(),
            }],
        })
    }

    fn rename(&self, _display_name: &str, _access_token: &str) -> Result<(), AccountError> {
        Ok(())
    }

    fn logout(&self, _access_token: &str, _all: bool) -> Result<(), AccountError> {
        Ok(())
    }

    fn delete_account(&self, _access_token: &str) -> Result<(), AccountError> {
        Ok(())
    }

    fn preference_schema(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountError> {
        if access_token == token(b'a') {
            return Err(AccountError::Unauthorized);
        }
        Ok(AccountPreferenceSchema {
            fields: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceField {
                        value_type: "string".into(),
                    },
                ),
                (
                    "platform.ios.nine_key".into(),
                    AccountPreferenceField {
                        value_type: "boolean".into(),
                    },
                ),
            ]),
            maximum_bytes: 65_536,
            update_mode: "replace".into(),
            revision_required: true,
        })
    }

    fn preferences(&self, access_token: &str) -> Result<AccountPreferences, AccountError> {
        if access_token == token(b'a') {
            return Err(AccountError::Unauthorized);
        }
        Ok(AccountPreferences {
            revision: 42,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String("quanpin".into()),
                ),
                (
                    "platform.ios.nine_key".into(),
                    AccountPreferenceValue::Boolean(true),
                ),
            ]),
        })
    }

    fn put_preferences(
        &self,
        preferences: &AccountPreferences,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountError> {
        if access_token == token(b'a') {
            return Err(AccountError::Unauthorized);
        }
        Ok(AccountPreferences {
            revision: preferences.revision + 1,
            settings: preferences.settings.clone(),
        })
    }
}

fn installed(storage: &MemoryStorage, expires_at_unix_ms: u64) {
    *storage.0.lock().unwrap() = Some(SavedAccountSession {
        tokens: tokens(b'a', b'b', 900),
        expires_at_unix_ms,
    });
}

fn valid_future_expiry() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 60_000
}

#[test]
fn validates_public_inputs_and_tokens() {
    assert!(validate_identity(&AccountIdentity {
        user_id: "user-1".into()
    })
    .is_ok());
    assert!(validate_identity(&AccountIdentity {
        user_id: "bad\n".into()
    })
    .is_err());
    assert_eq!(
        validate_provider_target("email", " fixture@example.test"),
        Err(AccountError::Invalid)
    );
    assert_eq!(validate_provider_target("apple", ""), Ok(()));
    assert_eq!(
        validate_provider_target("apple", "unexpected-target"),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        validate_login("challenge", "１２３４５６"),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        validate_apple_login("challenge", "identity-token\n"),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        validate_google_login("challenge", "4/0Afixture-code"),
        Ok(())
    );
    for code in ["", "4/0A fixture", "4/0A\u{1}", "码"] {
        assert_eq!(
            validate_google_login("challenge", code),
            Err(AccountError::Invalid)
        );
    }
    assert_eq!(
        validate_google_login("challenge", &"a".repeat(2049)),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        validate_login_request("challenge", "4/0Afixture-code"),
        Ok(())
    );
    assert_eq!(validate_login_request("challenge", "123456"), Ok(()));
    let mut invalid = tokens(b'a', b'b', 900);
    invalid.access_token = token(b'A');
    assert_eq!(validate_tokens(&invalid), Err(AccountError::Unavailable));

    let mut unbounded = tokens(b'a', b'b', 86_400 * 30 + 1);
    assert_eq!(validate_tokens(&unbounded), Err(AccountError::Unavailable));
    unbounded.expires_in = 86_400 * 30;
    assert!(validate_tokens(&unbounded).is_ok());
}

#[test]
fn rejects_persisted_session_expiry_beyond_thirty_days() {
    let storage = MemoryStorage::default();
    installed(&storage, u64::MAX);
    let session = BackendAccountSession::new(FakeApi::new(), storage);
    assert_eq!(session.status(), Err(AccountError::Storage));
}

#[test]
fn account_preferences_validate_and_merge_preserves_other_platforms() {
    let base = AccountPreferences {
        revision: 42,
        settings: BTreeMap::from([
            (
                "input.schema".into(),
                AccountPreferenceValue::String("quanpin".into()),
            ),
            (
                "platform.ios.nine_key".into(),
                AccountPreferenceValue::Boolean(true),
            ),
        ]),
    };
    let schema = AccountPreferenceSchema {
        fields: BTreeMap::from([
            (
                "input.schema".into(),
                AccountPreferenceField {
                    value_type: "string".into(),
                },
            ),
            (
                "platform.android.nine_key".into(),
                AccountPreferenceField {
                    value_type: "boolean".into(),
                },
            ),
        ]),
        maximum_bytes: 65_536,
        update_mode: "replace".into(),
        revision_required: true,
    };
    let replacing = BTreeMap::from([
        (
            "input.schema".into(),
            AccountPreferenceValue::String("shuangpin".into()),
        ),
        (
            "platform.android.nine_key".into(),
            AccountPreferenceValue::Boolean(false),
        ),
    ]);
    let merged = merge_account_preferences(&base, &replacing, &schema).unwrap();
    assert_eq!(merged.revision, 42);
    assert_eq!(
        merged.settings["input.schema"],
        AccountPreferenceValue::String("shuangpin".into())
    );
    assert_eq!(
        merged.settings["platform.ios.nine_key"],
        AccountPreferenceValue::Boolean(true)
    );
    assert_eq!(
        merged.settings["platform.android.nine_key"],
        AccountPreferenceValue::Boolean(false)
    );
    assert_eq!(
        merge_account_preferences(
            &base,
            &BTreeMap::from([("input.schema".into(), AccountPreferenceValue::Boolean(true),)]),
            &schema
        ),
        Err(AccountError::Invalid)
    );
}

#[test]
fn account_preferences_keep_photo_sized_strings_within_the_negotiated_limit() {
    let photo = "A".repeat(4 * 512_000_usize.div_ceil(3));
    let design = format!(r#"{{"photo":"{photo}"}}"#);
    let key = "platform.harmony.custom_keyboard_skin";
    let base = AccountPreferences {
        revision: 1,
        settings: BTreeMap::new(),
    };
    let mut schema = AccountPreferenceSchema {
        fields: BTreeMap::from([(
            key.into(),
            AccountPreferenceField {
                value_type: "string".into(),
            },
        )]),
        maximum_bytes: MAX_JSON_BYTES,
        update_mode: "replace".into(),
        revision_required: true,
    };
    let replacing = BTreeMap::from([(key.into(), AccountPreferenceValue::String(design.clone()))]);

    let merged = merge_account_preferences(&base, &replacing, &schema).unwrap();
    assert_eq!(merged.settings[key], AccountPreferenceValue::String(design));

    schema.maximum_bytes = 65_536;
    assert_eq!(
        merge_account_preferences(&base, &replacing, &schema),
        Err(AccountError::Invalid)
    );
}

#[test]
fn account_preferences_refresh_after_unauthorized_and_preserve_revision_conflicts() {
    let storage = MemoryStorage::default();
    installed(&storage, valid_future_expiry());
    let api = FakeApi::new();
    let refreshes = Arc::clone(&api.refreshes);
    let session = BackendAccountSession::new(api, storage);
    let schema = session.preference_schema().unwrap();
    assert!(schema.fields.contains_key("input.schema"));
    let cloud = session.preferences().unwrap();
    assert_eq!(cloud.revision, 42);
    let updated = session
        .put_preferences(&cloud)
        .expect("refresh should make the write succeed");
    assert_eq!(updated.revision, 43);
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(
        AccountError::from_status(StatusCode::CONFLICT),
        AccountError::Conflict
    );
    assert_eq!(AccountError::Conflict.code(), "account_conflict");
}

#[test]
fn refreshes_once_for_concurrent_callers() {
    let storage = MemoryStorage::default();
    installed(&storage, 0);
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let mut api = FakeApi::new();
    api.refresh_gate = Some(Arc::clone(&gate));
    let count = Arc::clone(&api.refreshes);
    let session = Arc::new(BackendAccountSession::new(api, storage));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let session = Arc::clone(&session);
            thread::spawn(move || session.access_token(None))
        })
        .collect();
    while count.load(Ordering::SeqCst) == 0 {
        thread::yield_now();
    }
    let (lock, ready) = &*gate;
    *lock.lock().unwrap() = true;
    ready.notify_all();
    let values: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap().unwrap())
        .collect();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(values.iter().all(|value| value == &token(b'c')));
}

#[test]
fn unauthorized_refresh_clears_storage() {
    let storage = MemoryStorage::default();
    installed(&storage, 0);
    let api = FakeApi::new();
    api.reject_refresh.store(true, Ordering::SeqCst);
    let session = BackendAccountSession::new(api, storage.clone());
    assert_eq!(session.access_token(None), Err(AccountError::Unauthorized));
    assert!(storage.load().unwrap().is_none());
    assert_eq!(session.status().unwrap(), None);
}

#[test]
fn late_refresh_cannot_restore_forgotten_session() {
    let storage = MemoryStorage::default();
    installed(&storage, 0);
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let mut api = FakeApi::new();
    api.refresh_gate = Some(Arc::clone(&gate));
    let count = Arc::clone(&api.refreshes);
    let session = Arc::new(BackendAccountSession::new(api, storage.clone()));
    let worker = {
        let session = Arc::clone(&session);
        thread::spawn(move || session.access_token(None))
    };
    while count.load(Ordering::SeqCst) == 0 {
        thread::yield_now();
    }
    session.forget().unwrap();
    let (lock, ready) = &*gate;
    *lock.lock().unwrap() = true;
    ready.notify_all();
    assert_eq!(worker.join().unwrap(), Err(AccountError::Cancelled));
    assert!(storage.load().unwrap().is_none());
}

#[test]
fn generation_exhaustion_refuses_async_account_operations() {
    let storage = MemoryStorage::default();
    installed(&storage, 0);
    let session = BackendAccountSession::new(FakeApi::new(), storage);
    session.set_generation_for_test(u64::MAX - 1);

    assert_eq!(
        session.sign_in("synthetic-challenge", "123456"),
        Err(AccountError::Unavailable)
    );
    session.set_generation_for_test(u64::MAX);
    assert_eq!(
        session.access_token(Some(&token(b'a'))),
        Err(AccountError::Unavailable)
    );
    session.forget().unwrap();
    assert_eq!(session.status().unwrap(), None);
}

#[test]
fn logout_clears_local_session_before_remote_result() {
    let storage = MemoryStorage::default();
    installed(&storage, valid_future_expiry());
    let session = BackendAccountSession::new(FakeApi::new(), storage.clone());
    session.logout(true).unwrap();
    assert!(storage.load().unwrap().is_none());
    assert_eq!(session.status().unwrap(), None);
}

fn serve_once(response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        std::io::Write::write_all(&mut stream, &response).unwrap();
    });
    format!("http://{address}")
}

fn serve_once_and_capture(response: Vec<u8>) -> (String, Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let expected_bytes = loop {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its headers");
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length: ")
                        .or_else(|| line.strip_prefix("Content-Length: "))
                })
                .unwrap()
                .parse::<usize>()
                .unwrap();
            break header_end + 4 + content_length;
        };
        while request.len() < expected_bytes {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its body");
            request.extend_from_slice(&buffer[..read]);
        }
        sender.send(request).unwrap();
        std::io::Write::write_all(&mut stream, &response).unwrap();
    });
    (format!("http://{address}"), receiver)
}

#[test]
fn transport_rejects_redirects() {
    let origin = serve_once(
        b"HTTP/1.1 302 Found\r\nLocation: https://example.test/\r\nContent-Length: 0\r\n\r\n"
            .to_vec(),
    );
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(client.providers(), Err(AccountError::Unavailable));
}

#[test]
fn transport_rejects_oversized_responses() {
    let body = vec![b'x'; MAX_JSON_BYTES + 1];
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend(body);
    let origin = serve_once(response);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(client.providers(), Err(AccountError::Unavailable));
}

#[test]
fn restores_dictionary_snapshot_only_after_a_new_cloud_revision() {
    let body = serde_json::json!({ "revision": 8, "reset": true }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let result = client
        .restore_dictionary_snapshot(b"{\"type\":\"header\"}\n", 7, &token(b'a'))
        .unwrap();
    assert_eq!(
        result,
        AccountDictionarySnapshotRestore {
            revision: 8,
            reset: true
        }
    );

    let body = serde_json::json!({ "revision": 8, "reset": false }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    assert_eq!(
        client.restore_dictionary_snapshot(b"snapshot", 7, &token(b'a')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn streams_dictionary_snapshot_file_with_exact_body_and_media_type() {
    let body = serde_json::json!({ "revision": 8, "reset": true }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let (origin, request) = serve_once_and_capture(response.into_bytes());
    let directory = tempfile::tempdir().unwrap();
    let snapshot = directory.path().join("fixture.ndjson");
    let contents = b"{\"type\":\"header\"}\n{\"type\":\"footer\"}\n";
    std::fs::write(&snapshot, contents).unwrap();

    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client
            .restore_dictionary_snapshot_file(&snapshot, 7, &token(b'a'))
            .unwrap(),
        AccountDictionarySnapshotRestore {
            revision: 8,
            reset: true,
        }
    );

    let request = request.recv().unwrap();
    let header_end = request
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap();
    let headers = std::str::from_utf8(&request[..header_end]).unwrap();
    assert!(headers.starts_with("PUT /v1/users/me/dictionary/snapshot?revision=7 HTTP/1.1\r\n"));
    assert!(headers.contains("content-type: application/x-ndjson\r\n"));
    assert!(headers.contains(&format!("content-length: {}\r\n", contents.len())));
    assert_eq!(&request[header_end + 4..], contents);
}

#[cfg(unix)]
#[test]
fn dictionary_snapshot_to_file_rejects_symlinked_parent() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let linked = directory.path().join("linked");
    symlink(target.path(), &linked).unwrap();
    let destination = linked.join("snapshot.ndjson");
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();

    assert_eq!(
        client.dictionary_snapshot_to_file(&destination, &token(b'a')),
        Err(AccountError::Invalid)
    );
    assert!(!target.path().join("snapshot.ndjson").exists());
}

#[test]
fn dictionary_snapshot_file_restore_validates_file_and_revision_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let empty = directory.path().join("empty.ndjson");
    std::fs::write(&empty, []).unwrap();
    let oversized = directory.path().join("oversized.ndjson");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_DICTIONARY_SNAPSHOT_BYTES as u64 + 1)
        .unwrap();
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();

    for path in [
        Path::new("relative.ndjson"),
        empty.as_path(),
        oversized.as_path(),
    ] {
        assert_eq!(
            client.restore_dictionary_snapshot_file(path, 7, &token(b'a')),
            Err(AccountError::Invalid)
        );
    }
    assert_eq!(
        client.restore_dictionary_snapshot_file(&empty, -1, &token(b'a')),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        client.restore_dictionary_snapshot_file(&empty, 7, "short"),
        Err(AccountError::Invalid)
    );
}

#[cfg(unix)]
#[test]
fn dictionary_snapshot_file_restore_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.ndjson");
    std::fs::write(&target, b"synthetic snapshot\n").unwrap();
    let link = directory.path().join("link.ndjson");
    symlink(&target, &link).unwrap();
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();

    assert_eq!(
        client.restore_dictionary_snapshot_file(&link, 7, &token(b'a')),
        Err(AccountError::Invalid)
    );
}

#[cfg(unix)]
#[test]
fn dictionary_snapshot_file_restore_rejects_symlinked_parent() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let linked = directory.path().join("linked");
    symlink(target.path(), &linked).unwrap();
    let snapshot = linked.join("snapshot.ndjson");
    std::fs::write(
        target.path().join("snapshot.ndjson"),
        b"synthetic snapshot\n",
    )
    .unwrap();
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();

    assert_eq!(
        client.restore_dictionary_snapshot_file(&snapshot, 7, &token(b'a')),
        Err(AccountError::Invalid)
    );
}

#[test]
fn dictionary_snapshot_file_restore_rejects_nonadvancing_response() {
    let body = serde_json::json!({ "revision": 7, "reset": true }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let directory = tempfile::tempdir().unwrap();
    let snapshot = directory.path().join("fixture.ndjson");
    std::fs::write(&snapshot, b"fixture\n").unwrap();
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();

    assert_eq!(
        client.restore_dictionary_snapshot_file(&snapshot, 7, &token(b'a')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn dictionary_snapshot_restore_requires_json_response_media_type() {
    let body = serde_json::json!({ "revision": 8, "reset": true }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
        body.len(),
        body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();

    assert_eq!(
        client.restore_dictionary_snapshot(b"fixture\n", 7, &token(b'a')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn account_dictionary_transport_maps_flattened_responses() {
    let id = "0123456789abcdef".repeat(4);
    let page_body = serde_json::json!({
        "entries": [{
            "id": id,
            "kind": "pinyin",
            "code": "ni",
            "word": "fixture",
            "weight": 1,
            "revision": 2
        }],
        "has_more": false,
        "offset": 0
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        page_body.len(),
        page_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let page = client
        .dictionary(DictionaryKind::Pinyin, "fixture", 0, &token(b'a'))
        .unwrap();
    assert_eq!(page.entries[0].word, "fixture");
    assert_eq!(page.entries[0].revision, 2);

    let change_body = serde_json::json!({
        "revision": 3,
        "previous": null,
        "replacement": {
            "id": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
            "kind": "pinyin",
            "code": "ni",
            "word": "fixture",
            "weight": 1,
            "revision": 3
        }
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        change_body.len(),
        change_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let change = client
        .add_dictionary(DictionaryKind::Pinyin, "ni", "fixture", 1, &token(b'a'))
        .unwrap();
    assert_eq!(change.revision, 3);
    assert_eq!(change.replacement.unwrap().id.len(), 64);

    let export = b"ni\tfixture\t1\n".to_vec();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n",
        export.len()
    )
    .into_bytes()
    .into_iter()
    .chain(export)
    .collect();
    let client = BackendAccountClient::loopback(&serve_once(response)).unwrap();
    let exported = client
        .export_dictionary(DictionaryKind::Pinyin, "standard", &token(b'a'))
        .unwrap();
    assert_eq!(exported.text, "ni\tfixture\t1\n");
    assert_eq!(exported.filename, "dictionary-pinyin.tsv");

    let catalog_body = serde_json::json!({
        "entries": [{
            "kind": "pinyin",
            "code": "ni'hao",
            "word": "你好",
            "weight": 100000
        }],
        "offset": 0,
        "has_more": false,
        "revision": 42,
        "normalized": "ni'hao"
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        catalog_body.len(),
        catalog_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let catalog = client
        .dictionary_catalog(
            DictionaryKind::Pinyin,
            "nihc",
            0,
            "shuangpin",
            "xiaohe",
            &token(b'a'),
        )
        .unwrap();
    assert_eq!(catalog.revision, 42);
    assert_eq!(catalog.normalized, "ni'hao");

    let change_body = serde_json::json!({
        "revision": 43,
        "previous": null,
        "replacement": null
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        change_body.len(),
        change_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let change = client
        .edit_dictionary_catalog(DictionaryKind::Pinyin, "ni", "你", 42, None, &token(b'a'))
        .unwrap();
    assert_eq!(change.revision, 43);

    let changes_body = serde_json::json!({
        "changes": [{
            "revision": 44,
            "previous": null,
            "replacement": null
        }],
        "next": 44,
        "has_more": false
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        changes_body.len(),
        changes_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let changes = client.dictionary_changes(43, 1, &token(b'a')).unwrap();
    assert_eq!(changes.next, 44);
    assert!(!changes.has_more);

    let snapshot = b"{\"type\":\"header\"}\n".to_vec();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/x-ndjson\r\n\r\n",
        snapshot.len()
    )
    .into_bytes()
    .into_iter()
    .chain(snapshot)
    .collect();
    let client = BackendAccountClient::loopback(&serve_once(response)).unwrap();
    assert_eq!(
        client.dictionary_snapshot(&token(b'a')).unwrap(),
        b"{\"type\":\"header\"}\n"
    );

    let snapshot = b"streamed snapshot\n".to_vec();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/x-ndjson\r\n\r\n",
        snapshot.len()
    )
    .into_bytes()
    .into_iter()
    .chain(snapshot.clone())
    .collect();
    let client = BackendAccountClient::loopback(&serve_once(response)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("snapshot.ndjson");
    let size = client
        .dictionary_snapshot_to_file(&destination, &token(b'a'))
        .unwrap();
    assert_eq!(size, snapshot.len() as u64);
    assert_eq!(std::fs::read(destination).unwrap(), snapshot);

    let invalid_changes = serde_json::json!({
        "changes": [{"revision": 44, "previous": null, "replacement": null}],
        "next": 43,
        "has_more": false
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        invalid_changes.len(),
        invalid_changes
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    assert_eq!(
        client.dictionary_changes(43, 1, &token(b'a')),
        Err(AccountError::Unavailable)
    );
    assert_eq!(
        client.dictionary_changes(-1, 1, &token(b'a')),
        Err(AccountError::Invalid)
    );
}

#[test]
fn validates_candidate_transport_boundaries() {
    let query = AccountCandidateQuery {
        text: "nihc".into(),
        kind: "pinyin".into(),
        scheme: "shuangpin".into(),
        profile: "xiaohe".into(),
        limit: 100,
    };
    assert!(validate_candidate_query(&query).is_ok());
    assert!(validate_candidate_query(&AccountCandidateQuery {
        text: "".into(),
        ..query.clone()
    })
    .is_err());
    assert!(validate_ranking_arguments(&query, 42, "pin", 1, 1).is_ok());
    assert!(validate_ranking_arguments(
        &AccountCandidateQuery {
            kind: "quick".into(),
            ..query.clone()
        },
        42,
        "pin",
        1,
        1
    )
    .is_err());
    assert!(validate_candidate_value(&query, "nihc", "你好").is_ok());
    assert!(validate_candidate_value(&query, "", "你好").is_err());
}

#[test]
fn account_candidate_transport_maps_canonical_and_fixed_state() {
    let candidate_body = serde_json::json!({
        "candidates": [{
            "code": "nihc",
            "word": "你好",
            "weight": 10,
            "canonical_pinyin": "ni'hao"
        }],
        "context": "server:context",
        "revision": 42
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        candidate_body.len(),
        candidate_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let query = AccountCandidateQuery {
        text: "nihc".into(),
        kind: "pinyin".into(),
        scheme: "shuangpin".into(),
        profile: "xiaohe".into(),
        limit: 100,
    };
    let candidates = client.personal_candidates(&query, &token(b'a')).unwrap();
    assert_eq!(candidates.candidates[0].mutation_code(), "ni'hao");

    let ranking_body = serde_json::json!({
        "revision": 43,
        "changed": true,
        "selection": { "count": 0 }
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        ranking_body.len(),
        ranking_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let ranking = client
        .rank_candidate(
            &query,
            "ni'hao",
            "你好",
            42,
            "pin",
            1,
            1,
            false,
            &token(b'a'),
        )
        .unwrap();
    assert!(ranking.changed);
    assert_eq!(ranking.revision, 43);

    let positions_body = serde_json::json!({
        "positions": [{
            "context": "server:context",
            "code": "ni'hao",
            "word": "你好",
            "position": 1
        }],
        "offset": 0,
        "has_more": false
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        positions_body.len(),
        positions_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let positions = client
        .fixed_positions("server:context", 0, &token(b'a'))
        .unwrap();
    assert_eq!(positions.positions[0].position, 1);

    let revision_body = r#"{"revision":44}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        revision_body.len(),
        revision_body
    );
    let client = BackendAccountClient::loopback(&serve_once(response.into_bytes())).unwrap();
    let revision = client
        .set_fixed_position("server:context", "ni'hao", "你好", None, 43, &token(b'a'))
        .unwrap();
    assert_eq!(revision.revision, 44);
}

#[test]
fn google_target_accepts_only_the_loopback_callback() {
    for target in [
        "http://127.0.0.1:1024/callback",
        "http://127.0.0.1:53682/callback",
        "http://127.0.0.1:65535/callback",
        "http://[::1]:49152/callback",
    ] {
        assert_eq!(
            validate_provider_target("google", target),
            Ok(()),
            "{target}"
        );
    }
    for target in [
        "",
        "http://127.0.0.1/callback",
        "http://127.0.0.1:1023/callback",
        "http://127.0.0.1:65536/callback",
        "http://127.0.0.1:080/callback",
        "http://127.0.0.1:+8080/callback",
        "http://127.0.0.1:8080/callback/",
        "http://127.0.0.1:8080/callback?next=1",
        "http://127.0.0.1:8080/callback#fragment",
        "http://127.0.0.1:8080/other",
        "http://localhost:8080/callback",
        "http://127.0.0.2:8080/callback",
        "http://user@127.0.0.1:8080/callback",
        "https://127.0.0.1:8080/callback",
        " http://127.0.0.1:8080/callback",
    ] {
        assert_eq!(
            validate_provider_target("google", target),
            Err(AccountError::Invalid),
            "{target}"
        );
    }
    assert_eq!(
        validate_provider_target("email", "http://127.0.0.1:8080/callback"),
        Ok(())
    );
    assert_eq!(
        validate_provider_target("apple", "http://127.0.0.1:8080/callback"),
        Err(AccountError::Invalid)
    );
}

#[test]
fn google_authorization_url_must_redirect_to_this_listener() {
    let target = "http://127.0.0.1:53682/callback";
    let url = google_authorization_url(target, GOOGLE_FIXTURE_STATE);
    assert_eq!(
        google::google_authorization_state(&url, target).as_deref(),
        Ok(GOOGLE_FIXTURE_STATE)
    );
    assert_eq!(
        google::google_authorization_state(&url, "http://127.0.0.1:53683/callback"),
        Err(AccountError::Unavailable)
    );
    for url in [
        google_authorization_url(target, GOOGLE_FIXTURE_STATE)
            .replace("accounts.google.com", "accounts.google.com.evil.test"),
        google_authorization_url(target, GOOGLE_FIXTURE_STATE).replace("https://", "http://"),
        google_authorization_url(target, GOOGLE_FIXTURE_STATE)
            .replace("accounts.google.com/", "accounts.google.com:8443/"),
        google_authorization_url(target, GOOGLE_FIXTURE_STATE) + "#fragment",
        google_authorization_url(target, GOOGLE_FIXTURE_STATE) + "&state=second",
        google_authorization_url(target, ""),
        google_authorization_url(target, GOOGLE_FIXTURE_STATE).replace("state=", "other="),
        google_authorization_url(target, GOOGLE_FIXTURE_STATE) + "&x=\"quoted\"",
        "https://evil.test/o/oauth2/v2/auth?state=fixture".into(),
    ] {
        assert_eq!(
            google::google_authorization_state(&url, target),
            Err(AccountError::Unavailable),
            "{url}"
        );
    }
}

#[test]
fn google_callback_requires_the_path_and_matching_state() {
    use google::{parse_google_callback, GoogleCallback};
    let state = GOOGLE_FIXTURE_STATE;
    assert_eq!(
        parse_google_callback(
            &format!("GET /callback?state={state}&code=4%2F0Afixture&scope=openid HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
            state
        ),
        GoogleCallback::Code("4/0Afixture".into())
    );
    assert_eq!(
        parse_google_callback(
            &format!("GET /callback?error=access_denied&state={state} HTTP/1.1\r\n\r\n"),
            state
        ),
        GoogleCallback::Failed(AccountError::Cancelled)
    );
    assert_eq!(
        parse_google_callback(
            &format!("GET /callback?state={state} HTTP/1.1\r\n\r\n"),
            state
        ),
        GoogleCallback::Failed(AccountError::Unavailable)
    );
    assert_eq!(
        parse_google_callback(
            &format!("GET /callback?state={state}&code=bad%20code HTTP/1.1\r\n\r\n"),
            state
        ),
        GoogleCallback::Failed(AccountError::Unavailable)
    );
    for head in [
        "GET /favicon.ico HTTP/1.1\r\n\r\n".to_string(),
        format!("GET /callback/extra?state={state}&code=fixture HTTP/1.1\r\n\r\n"),
        format!("POST /callback?state={state}&code=fixture HTTP/1.1\r\n\r\n"),
        "GET /callback?state=other-state&code=fixture HTTP/1.1\r\n\r\n".to_string(),
        "GET /callback?state=other-state&error=access_denied HTTP/1.1\r\n\r\n".to_string(),
        "GET /callback?code=fixture HTTP/1.1\r\n\r\n".to_string(),
        format!("GET /callback?state={state}&state={state}&code=fixture HTTP/1.1\r\n\r\n"),
        format!("GET http://127.0.0.1/callback?state={state}&code=fixture HTTP/1.1\r\n\r\n"),
        String::new(),
        "garbage".to_string(),
    ] {
        assert_eq!(
            parse_google_callback(&head, state),
            GoogleCallback::Ignored,
            "{head}"
        );
    }
}

fn browser_request(target: &str, path_and_query: &str) -> String {
    let address = target
        .strip_prefix("http://")
        .and_then(|rest| rest.strip_suffix("/callback"))
        .unwrap()
        .to_string();
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    std::io::Write::write_all(
        &mut stream,
        format!("GET {path_and_query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes(),
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn google_sign_in_waits_for_the_matching_loopback_redirect() {
    let storage = MemoryStorage::default();
    let api = FakeApi::new();
    let session = BackendAccountSession::new(api.clone(), storage.clone());
    let signed_in = session
        .sign_in_google_with_browser(|url| {
            let parsed = Url::parse(url).unwrap();
            let target = parsed
                .query_pairs()
                .find(|(key, _)| key == "redirect_uri")
                .unwrap()
                .1
                .into_owned();
            thread::spawn(move || {
                let favicon = browser_request(&target, "/favicon.ico");
                assert!(favicon.starts_with("HTTP/1.1 404 "));
                let forged = browser_request(&target, "/callback?state=forged&code=attacker");
                assert!(forged.starts_with("HTTP/1.1 404 "));
                let page = browser_request(
                    &target,
                    &format!("/callback?state={GOOGLE_FIXTURE_STATE}&code=4%2F0Afixture-code"),
                );
                assert!(page.starts_with("HTTP/1.1 200 OK\r\n"));
                assert!(page.contains("Content-Type: text/html; charset=utf-8"));
                assert!(page.contains("已收到 Google 授权"));
                assert!(page.contains("请回到水杉输入法"));
                assert!(!page.contains("{{"), "every template placeholder is filled");
                // The page is self-contained: no script runs and nothing is fetched.
                assert!(page.contains("Content-Security-Policy: default-src 'none';"));
                assert!(!page.contains("<script"));
                // The code has not been exchanged yet, so the page must not claim success.
                assert!(!page.contains("登录已完成"));
                assert!(!page.contains("4/0Afixture-code"));
            });
            Ok(())
        })
        .unwrap();
    assert_eq!(signed_in, user());
    assert_eq!(
        api.logins.lock().unwrap().as_slice(),
        [(
            "fixture-challenge".to_string(),
            "4/0Afixture-code".to_string()
        )]
    );
    assert!(storage.load().unwrap().is_some());
}

#[test]
fn google_sign_in_treats_denial_and_timeout_as_cancellation() {
    let api = FakeApi::new();
    let session = BackendAccountSession::new(api.clone(), MemoryStorage::default());
    let denied = session.sign_in_google_with_browser(|url| {
        let parsed = Url::parse(url).unwrap();
        let target = parsed
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1
            .into_owned();
        thread::spawn(move || {
            let page = browser_request(
                &target,
                &format!("/callback?error=access_denied&state={GOOGLE_FIXTURE_STATE}"),
            );
            assert!(page.contains("已取消"));
        });
        Ok(())
    });
    assert_eq!(denied, Err(AccountError::Cancelled));

    let timed_out = session.sign_in_google_with_timeout(|_| Ok(()), Duration::from_millis(200));
    assert_eq!(timed_out, Err(AccountError::Cancelled));

    let unopened = session.sign_in_google_with_browser(|_| Err(AccountError::Unavailable));
    assert_eq!(unopened, Err(AccountError::Unavailable));
    assert!(api.logins.lock().unwrap().is_empty());
}

#[test]
fn google_sign_in_can_be_cancelled_while_waiting_for_the_browser() {
    let api = FakeApi::new();
    let session = BackendAccountSession::new(api.clone(), MemoryStorage::default());
    let (opened, browser_opened) = mpsc::channel();
    let started = std::time::Instant::now();
    let result = thread::scope(|scope| {
        let session = &session;
        scope.spawn(move || {
            browser_opened.recv().unwrap();
            thread::sleep(Duration::from_millis(200));
            session.cancel_google_sign_in();
        });
        session.sign_in_google_with_browser(|_| {
            opened.send(()).unwrap();
            Ok(())
        })
    });
    assert_eq!(result, Err(AccountError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(api.logins.lock().unwrap().is_empty());
    // Cancelling with nothing in progress is harmless and does not poison the next sign-in.
    session.cancel_google_sign_in();
    let timed_out = session.sign_in_google_with_timeout(|_| Ok(()), Duration::from_millis(200));
    assert_eq!(timed_out, Err(AccountError::Cancelled));
}

#[test]
fn google_wait_leaves_room_for_the_backend_before_the_challenge_expires() {
    use google::google_callback_window;
    assert_eq!(
        google_callback_window(300, GOOGLE_SIGN_IN_TIMEOUT),
        Some(Duration::from_secs(270))
    );
    assert_eq!(
        google_callback_window(3600, GOOGLE_SIGN_IN_TIMEOUT),
        Some(GOOGLE_SIGN_IN_TIMEOUT)
    );
    assert_eq!(
        google_callback_window(300, Duration::from_millis(200)),
        Some(Duration::from_millis(200))
    );
    assert_eq!(google_callback_window(30, GOOGLE_SIGN_IN_TIMEOUT), None);
    assert_eq!(google_callback_window(1, GOOGLE_SIGN_IN_TIMEOUT), None);
}

#[test]
fn a_trickling_loopback_client_cannot_hold_the_listener_past_its_budget() {
    use google::receive_google_callback;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let trickle = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            while !stop.load(Ordering::SeqCst) {
                if std::io::Write::write_all(&mut stream, b"G").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        })
    };
    let started = std::time::Instant::now();
    let result = receive_google_callback(
        &listener,
        GOOGLE_FIXTURE_STATE,
        started + Duration::from_millis(500),
        &AtomicBool::new(false),
    );
    let elapsed = started.elapsed();
    stop.store(true, Ordering::SeqCst);
    trickle.join().unwrap();
    assert_eq!(result, Err(AccountError::Cancelled));
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn backend_login_forwards_a_google_authorization_code() {
    let body = serde_json::to_string(&tokens(b'a', b'b', 900)).unwrap();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let (origin, request) = serve_once_and_capture(response.into_bytes());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    client
        .login("fixture-challenge", "4/0Afixture-code")
        .unwrap();
    let request = String::from_utf8(request.recv().unwrap()).unwrap();
    assert!(request.starts_with("POST /v1/auth/login "));
    assert!(request
        .ends_with(r#"{"challenge_id":"fixture-challenge","credential":"4/0Afixture-code"}"#));
}

#[test]
fn avatar_urls_only_reach_the_bucket_and_google() {
    for url in [
        "https://media.msime.app/avatars/abc.jpg",
        "https://lh3.googleusercontent.com/a/person=s96-c",
        "https://googleusercontent.com/a",
    ] {
        assert!(account_avatar_url_allowed(url), "{url}");
    }
    for url in [
        "http://media.msime.app/avatars/abc.jpg",
        "https://media.msime.app:8443/avatars/abc.jpg",
        "https://user@media.msime.app/avatars/abc.jpg",
        "https://evilgoogleusercontent.com/a",
        "https://msime.app/avatars/abc.jpg",
        "https://example.test/a.png",
        "not a url",
    ] {
        assert!(!account_avatar_url_allowed(url), "{url}");
    }
    assert!(account_avatar_is_uploaded(
        "https://media.msime.app/avatars/abc.jpg"
    ));
    assert!(!account_avatar_is_uploaded(
        "https://lh3.googleusercontent.com/a/person=s96-c"
    ));
}

#[test]
fn avatar_uploads_are_read_by_their_contents() {
    let directory = tempfile::tempdir().unwrap();
    let png = directory.path().join("picked.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\nrest-of-png").unwrap();
    // A JPEG named .png is still a JPEG: the bytes decide, not the name.
    let jpeg = directory.path().join("photo.png");
    std::fs::write(&jpeg, [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]).unwrap();
    assert_eq!(
        read_account_avatar_upload(&png).unwrap().content_type,
        "image/png"
    );
    assert_eq!(
        read_account_avatar_upload(&jpeg).unwrap().content_type,
        "image/jpeg"
    );
    let gif = directory.path().join("anim.gif");
    std::fs::write(&gif, b"GIF89a....").unwrap();
    let webp = directory.path().join("still.webp");
    std::fs::write(&webp, b"RIFF\0\0\0\0WEBPVP8 ").unwrap();
    let large = directory.path().join("large.png");
    let mut oversized = b"\x89PNG\r\n\x1a\n".to_vec();
    oversized.resize(MAX_ACCOUNT_AVATAR_UPLOAD_BYTES as usize + 1, 0);
    std::fs::write(&large, oversized).unwrap();
    let empty = directory.path().join("empty.png");
    std::fs::write(&empty, b"").unwrap();
    let link = directory.path().join("link.png");
    std::os::unix::fs::symlink(&png, &link).unwrap();
    for path in [&gif, &webp, &large, &empty, &link, directory.path()] {
        assert_eq!(
            read_account_avatar_upload(path),
            Err(AccountError::Invalid),
            "{path:?}"
        );
    }
    assert_eq!(
        read_account_avatar_upload(Path::new("relative.png")),
        Err(AccountError::Invalid)
    );
}

#[test]
fn avatar_upload_sends_the_image_itself_and_removal_deletes() {
    let image = AccountAvatarImage {
        content_type: "image/png",
        bytes: b"\x89PNG\r\n\x1a\nsynthetic".to_vec(),
    };
    let body = r#"{"user":{"id":"u","display_name":"n","created_at":"c"},"identities":[]}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        body.len(),
        body
    );
    let (origin, request) = serve_once_and_capture(response.into_bytes());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    client.upload_avatar(&image, &token(b'a')).unwrap();
    let request = request.recv().unwrap();
    let text = String::from_utf8_lossy(&request).to_lowercase();
    assert!(
        text.starts_with("put /v1/users/me/avatar http/1.1\r\n"),
        "{text}"
    );
    assert!(text.contains("\r\ncontent-type: image/png\r\n"));
    assert!(text.contains(&format!("\r\nauthorization: bearer {}\r\n", token(b'a'))));
    assert!(request.ends_with(&image.bytes));

    // Refused locally before any request: a GIF, an empty body and an oversized one.
    for bad in [
        AccountAvatarImage {
            content_type: "image/gif",
            bytes: b"GIF89a".to_vec(),
        },
        AccountAvatarImage {
            content_type: "image/png",
            bytes: Vec::new(),
        },
        AccountAvatarImage {
            content_type: "image/jpeg",
            bytes: vec![0; MAX_ACCOUNT_AVATAR_UPLOAD_BYTES as usize + 1],
        },
    ] {
        assert_eq!(
            client.upload_avatar(&bad, &token(b'a')),
            Err(AccountError::Invalid)
        );
    }

    // A DELETE has no body and so no Content-Length, which `serve_once_and_capture` expects; reading up to the end of the headers is enough here.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sender, received) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its headers");
            request.extend_from_slice(&buffer[..read]);
        }
        sender.send(request).unwrap();
        std::io::Write::write_all(
            &mut stream,
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
        )
        .unwrap();
    });
    let client = BackendAccountClient::loopback(&origin).unwrap();
    client.delete_avatar(&token(b'b')).unwrap();
    let request = String::from_utf8(received.recv().unwrap()).unwrap();
    assert!(
        request.starts_with("DELETE /v1/users/me/avatar HTTP/1.1\r\n"),
        "{request}"
    );
}

#[test]
fn user_profile_fields_are_optional_and_bounded() {
    // A session saved before the fields existed still loads.
    let old: AccountUser =
        serde_json::from_str(r#"{"id":"u","display_name":"n","created_at":"c"}"#).unwrap();
    assert_eq!((old.email, old.avatar_url), (None, None));
    let current: AccountUser = serde_json::from_str(
        r#"{"id":"u","display_name":"n","created_at":"c","email":"a@example.test","avatar_url":"https://media.msime.app/avatars/x.jpg"}"#,
    )
    .unwrap();
    assert_eq!(current.email.as_deref(), Some("a@example.test"));
    // Absent fields stay absent when the session is written back.
    let written = serde_json::to_value(user()).unwrap();
    assert!(written.get("email").is_none() && written.get("avatar_url").is_none());
    let mut long = user();
    long.avatar_url = Some(format!("https://media.msime.app/{}", "a".repeat(2048)));
    assert_eq!(validate_user(&long), Err(AccountError::Invalid));
    let image = AccountAvatarImage {
        content_type: "image/jpeg",
        bytes: vec![1, 2, 3],
    };
    assert_eq!(image.data_url(), "data:image/jpeg;base64,AQID");
}

/// The backend's refresh contract: each refresh token works once, and presenting a spent one revokes the session (`msime-cloud` `Store.Refresh`).
#[derive(Clone)]
struct RotatingBackend {
    state: Arc<Mutex<RotatingState>>,
    /// Runs inside `refresh` before it answers, standing in for another process that writes the store without taking the lock.
    during_refresh: Option<Arc<dyn Fn() + Send + Sync>>,
}

struct RotatingState {
    current: Option<String>,
    next: u8,
    refreshes: usize,
    revoked: bool,
}

impl RotatingBackend {
    fn new(refresh_token: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(RotatingState {
                current: Some(refresh_token.into()),
                next: 0,
                refreshes: 0,
                revoked: false,
            })),
            during_refresh: None,
        }
    }
}

impl AccountApi for RotatingBackend {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        Ok(HashMap::new())
    }

    fn challenge(&self, _provider: &str, _target: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn login(&self, _challenge: &str, _credential: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        if let Some(hook) = &self.during_refresh {
            hook();
        }
        let mut state = self.state.lock().unwrap();
        state.refreshes += 1;
        if state.current.as_deref() != Some(refresh_token) {
            state.current = None;
            state.revoked = true;
            return Err(AccountError::Unauthorized);
        }
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let access = HEX[usize::from(state.next % 16)];
        let refresh = HEX[usize::from((state.next + 8) % 16)];
        state.next += 1;
        state.current = Some(token(refresh));
        Ok(tokens(access, refresh, 900))
    }

    fn profile(&self, _access_token: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn rename(&self, _display_name: &str, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn logout(&self, _access_token: &str, _all: bool) -> Result<(), AccountError> {
        Ok(())
    }

    fn delete_account(&self, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

fn expired_session() -> SavedAccountSession {
    SavedAccountSession {
        tokens: tokens(b'a', b'b', 900),
        expires_at_unix_ms: 1,
    }
}

fn session_file(directory: &Path) -> FileAccountSessionStorage {
    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Apple)
}

#[test]
fn a_session_shared_through_the_file_is_not_refreshed_from_a_spent_token() {
    let directory = tempfile::tempdir().unwrap();
    session_file(directory.path())
        .save(&expired_session())
        .unwrap();
    let backend = RotatingBackend::new(&token(b'b'));
    // The settings app and the input method, each with its own copy of the session in memory.
    let settings = BackendAccountSession::new(backend.clone(), session_file(directory.path()));
    let input_method = BackendAccountSession::new(backend.clone(), session_file(directory.path()));
    assert!(settings.status().unwrap().is_some());

    let rotated = input_method.access_token(None).unwrap();
    // Before the fix the settings app refreshed from the token the input method had already spent, the backend revoked the session, and the settings app deleted it.
    assert_eq!(settings.access_token(None).unwrap(), rotated);

    let state = backend.state.lock().unwrap();
    assert_eq!(state.refreshes, 1);
    assert!(!state.revoked);
    drop(state);
    assert!(session_file(directory.path()).load().unwrap().is_some());
}

#[test]
fn a_rejected_refresh_keeps_a_session_another_process_saved_meanwhile() {
    let directory = tempfile::tempdir().unwrap();
    session_file(directory.path())
        .save(&expired_session())
        .unwrap();
    let mut backend = RotatingBackend::new(&token(b'f'));
    let other = directory.path().to_path_buf();
    backend.during_refresh = Some(Arc::new(move || {
        session_file(&other)
            .save(&SavedAccountSession {
                tokens: tokens(b'c', b'f', 900),
                expires_at_unix_ms: valid_future_expiry(),
            })
            .unwrap();
    }));
    let session = BackendAccountSession::new(backend, session_file(directory.path()));

    assert_eq!(session.access_token(None).unwrap(), token(b'c'));
    let kept = session_file(directory.path()).load().unwrap().unwrap();
    assert_eq!(kept.tokens.refresh_token, token(b'f'));
}

#[test]
fn signing_out_in_one_process_signs_out_the_other() {
    let directory = tempfile::tempdir().unwrap();
    session_file(directory.path())
        .save(&expired_session())
        .unwrap();
    let backend = RotatingBackend::new(&token(b'b'));
    let settings = BackendAccountSession::new(backend.clone(), session_file(directory.path()));
    let input_method = BackendAccountSession::new(backend, session_file(directory.path()));
    assert!(settings.status().unwrap().is_some());

    input_method.forget().unwrap();
    assert!(settings.status().unwrap().is_none());
    assert!(matches!(
        settings.access_token(None),
        Err(AccountError::Unauthorized)
    ));
}

#[test]
fn session_file_layouts_round_trip_and_read_each_other() {
    let directory = tempfile::tempdir().unwrap();
    let session = SavedAccountSession {
        tokens: tokens(b'a', b'b', 900),
        expires_at_unix_ms: 1_790_000_000_123,
    };
    let apple = session_file(directory.path());
    apple.save(&session).unwrap();
    let text = std::fs::read_to_string(directory.path().join(ACCOUNT_SESSION_FILE)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("expiresAt").is_some());
    assert!(value.get("expires_at_unix_ms").is_none());

    let native = FileAccountSessionStorage::new(directory.path(), AccountSessionFileLayout::Native);
    assert_eq!(
        native.load().unwrap().unwrap().expires_at_unix_ms,
        session.expires_at_unix_ms
    );
    native.save(&session).unwrap();
    assert_eq!(
        apple.load().unwrap().unwrap().expires_at_unix_ms,
        session.expires_at_unix_ms
    );
}

#[cfg(unix)]
#[test]
fn session_file_is_owner_only_and_a_widened_one_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("state");
    let storage = session_file(&directory);
    storage.save(&expired_session()).unwrap();
    let file = directory.join(ACCOUNT_SESSION_FILE);
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&file), 0o600);

    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(storage.load(), Err(AccountError::Storage)));
}

#[test]
fn oversized_session_file_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let storage = session_file(directory.path());
    storage.save(&expired_session()).unwrap();
    std::fs::write(
        directory.path().join(ACCOUNT_SESSION_FILE),
        vec![b' '; 64 * 1024 + 1],
    )
    .unwrap();
    assert!(matches!(storage.load(), Err(AccountError::Storage)));
}
