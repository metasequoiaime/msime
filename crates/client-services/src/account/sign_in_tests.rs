//! 宿主自己弹登录界面的那几种登录（#8201）：Apple 原生登录、Google ID token、Google 回环、安卓的 Apple 网页登录。假的 [`AccountApi`] 记下每次 challenge 和换令牌的参数，测试核对会话发出去的是什么、拒绝了什么、存下了什么。

use super::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn tokens() -> AccountTokens {
    AccountTokens {
        access_token: token(b'a'),
        refresh_token: token(b'b'),
        token_type: "Bearer".into(),
        expires_in: 900,
        user: user(),
    }
}

const NONCE: &str = "fixture-nonce-0123456789";
const STATE: &str = "fixture-state_0123456789";
const TARGET: &str = "http://127.0.0.1:53682/callback";
/// 形状合格的合成 JWT：三段 base64url，内容无意义，服务端才验签。
const ID_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.eyJub25jZSI6ImZpeHR1cmUifQ.c2lnbmF0dXJl";

fn apple_authorization_url(state: &str) -> String {
    let mut url = Url::parse("https://appleid.apple.com/auth/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", "app.msime.signin")
        .append_pair(
            "redirect_uri",
            "https://api.msime.app/v1/auth/apple/callback",
        )
        .append_pair("response_type", "code id_token")
        .append_pair("response_mode", "form_post")
        .append_pair("scope", "name email")
        .append_pair("state", state)
        .append_pair("nonce", NONCE);
    url.to_string()
}

fn google_authorization_url(redirect: &str) -> String {
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", "fixture-client.apps.googleusercontent.com")
        .append_pair("redirect_uri", redirect)
        .append_pair("response_type", "code")
        .append_pair("state", STATE);
    url.to_string()
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl AccountSessionStorage for MemoryStorage {
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
}

/// 每个字段是下一次调用要回的值；`calls` 按顺序记下发出去的参数。
#[derive(Clone)]
struct SignInApi {
    nonce: Arc<Mutex<Option<String>>>,
    authorization_url: Arc<Mutex<Option<String>>>,
    apple_web_url: Arc<Mutex<String>>,
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
    network: Arc<AtomicUsize>,
}

impl SignInApi {
    fn new() -> Self {
        Self {
            nonce: Arc::new(Mutex::new(Some(NONCE.into()))),
            authorization_url: Arc::new(Mutex::new(None)),
            apple_web_url: Arc::new(Mutex::new(apple_authorization_url(STATE))),
            calls: Arc::new(Mutex::new(Vec::new())),
            network: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn record(&self, kind: &str, first: &str, second: &str) {
        self.network.fetch_add(1, Ordering::SeqCst);
        self.calls
            .lock()
            .unwrap()
            .push((kind.into(), first.into(), second.into()));
    }

    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }

    fn call(kind: &str, first: &str, second: &str) -> (String, String, String) {
        (kind.into(), first.into(), second.into())
    }
}

impl AccountApi for SignInApi {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        Ok(Default::default())
    }

    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        self.record("challenge", provider, target);
        Ok(AccountChallenge {
            challenge_id: "fixture-challenge".into(),
            expires_in: 300,
            nonce: self.nonce.lock().unwrap().clone(),
            authorization_url: self.authorization_url.lock().unwrap().clone(),
        })
    }

    fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
        self.record("login", challenge, credential);
        Ok(tokens())
    }

    fn apple_web_challenge(
        &self,
        code_challenge: &str,
        app: &str,
    ) -> Result<AccountAppleWebChallenge, AccountError> {
        self.record("apple_web", code_challenge, app);
        Ok(AccountAppleWebChallenge {
            authorization_url: self.apple_web_url.lock().unwrap().clone(),
            expires_in: 300,
        })
    }

    fn apple_web_login(
        &self,
        grant: &str,
        code_verifier: &str,
    ) -> Result<AccountTokens, AccountError> {
        self.record("apple_web_login", grant, code_verifier);
        Ok(tokens())
    }

    fn refresh(&self, _refresh_token: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn profile(&self, _access_token: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn rename(&self, _display_name: &str, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn logout(&self, _access_token: &str, _all: bool) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn delete_account(&self, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

fn session() -> (
    BackendAccountSession<SignInApi, MemoryStorage>,
    SignInApi,
    MemoryStorage,
) {
    let api = SignInApi::new();
    let storage = MemoryStorage::default();
    (
        BackendAccountSession::new(api.clone(), storage.clone()),
        api,
        storage,
    )
}

#[test]
fn apple_and_google_id_token_challenges_ask_without_a_target_and_need_a_nonce() {
    let (session, api, _) = session();
    assert_eq!(
        session.apple_challenge().unwrap().nonce.as_deref(),
        Some(NONCE)
    );
    assert_eq!(
        session
            .google_id_token_challenge()
            .unwrap()
            .nonce
            .as_deref(),
        Some(NONCE)
    );
    assert_eq!(
        api.calls(),
        [
            SignInApi::call("challenge", "apple", ""),
            SignInApi::call("challenge", "google", "")
        ]
    );
    // 没有 nonce 的 challenge 换不到会话（服务端按 nonce 验 identity token），不交给宿主。
    for missing in [None, Some(String::new())] {
        *api.nonce.lock().unwrap() = missing;
        assert_eq!(session.apple_challenge(), Err(AccountError::Unavailable));
        assert_eq!(
            session.google_id_token_challenge(),
            Err(AccountError::Unavailable)
        );
    }
}

#[test]
fn the_account_page_request_code_still_refuses_a_google_challenge_without_a_target() {
    let (session, api, _) = session();
    assert_eq!(
        session.request_code("google", "").unwrap_err(),
        AccountError::Invalid
    );
    assert_eq!(api.network.load(Ordering::SeqCst), 0);
}

#[test]
fn a_google_loopback_challenge_needs_an_authorization_url_for_that_target() {
    let (session, api, _) = session();
    *api.authorization_url.lock().unwrap() = Some(google_authorization_url(TARGET));
    let challenge = session.google_loopback_challenge(TARGET).unwrap();
    assert_eq!(challenge.challenge_id, "fixture-challenge");
    assert_eq!(
        challenge.authorization_url,
        Some(google_authorization_url(TARGET))
    );
    assert_eq!(
        api.calls(),
        [SignInApi::call("challenge", "google", TARGET)]
    );

    for url in [
        None,
        Some(google_authorization_url("http://127.0.0.1:53683/callback")),
        Some(google_authorization_url(TARGET).replace("accounts.google.com", "evil.test")),
    ] {
        *api.authorization_url.lock().unwrap() = url;
        assert_eq!(
            session.google_loopback_challenge(TARGET),
            Err(AccountError::Unavailable)
        );
    }
    let before = api.network.load(Ordering::SeqCst);
    for target in [
        "",
        "https://127.0.0.1:53682/callback",
        "http://127.0.0.1:80/callback",
    ] {
        assert_eq!(
            session.google_loopback_challenge(target),
            Err(AccountError::Invalid)
        );
    }
    assert_eq!(api.network.load(Ordering::SeqCst), before);
}

#[test]
fn identity_tokens_and_codes_sign_in_and_save_the_session() {
    let (session, api, storage) = session();
    assert_eq!(
        session.sign_in_apple("apple-challenge", ID_TOKEN).unwrap(),
        user()
    );
    assert!(storage.load().unwrap().is_some());
    storage.clear().unwrap();
    assert_eq!(
        session
            .sign_in_google_id_token("google-challenge", ID_TOKEN)
            .unwrap(),
        user()
    );
    assert!(storage.load().unwrap().is_some());
    storage.clear().unwrap();
    assert_eq!(
        session
            .sign_in_google("loopback-challenge", "4/0Afixture-code")
            .unwrap(),
        user()
    );
    assert!(storage.load().unwrap().is_some());
    assert_eq!(
        api.calls(),
        [
            SignInApi::call("login", "apple-challenge", ID_TOKEN),
            SignInApi::call("login", "google-challenge", ID_TOKEN),
            SignInApi::call("login", "loopback-challenge", "4/0Afixture-code"),
        ]
    );
}

#[test]
fn a_google_id_token_that_is_not_a_jwt_is_refused_before_the_network() {
    let (session, api, storage) = session();
    let oversized = format!("{}.{}.c2ln", "a".repeat(6000), "b".repeat(6000));
    for credential in [
        "",
        "4/0Afixture-code",
        "header.payload",
        "header..signature",
        "header.payload.signature.extra",
        "header.pay load.signature",
        "header.pay+load.signature",
        oversized.as_str(),
    ] {
        assert_eq!(
            session.sign_in_google_id_token("google-challenge", credential),
            Err(AccountError::Invalid),
            "{credential:?}"
        );
    }
    assert_eq!(
        session.sign_in_google_id_token("", ID_TOKEN),
        Err(AccountError::Invalid)
    );
    assert_eq!(api.network.load(Ordering::SeqCst), 0);
    assert!(storage.load().unwrap().is_none());
    // 恰好 12000 字节的仍然合法，与服务端的上限相同。
    let largest = format!("{}.{}.c2ln", "a".repeat(5994), "b".repeat(6000));
    assert_eq!(largest.len(), 12_000);
    assert!(session
        .sign_in_google_id_token("google-challenge", &largest)
        .is_ok());
}

#[test]
fn an_apple_web_sign_in_sends_only_the_s256_digest_of_a_fresh_verifier() {
    let (session, api, _) = session();
    let first = session.apple_web_start("app.msime.android.pinyin").unwrap();
    let second = session.apple_web_start("app.msime.android").unwrap();
    assert_eq!(first.authorization_url, apple_authorization_url(STATE));
    assert_eq!(first.expires_in, 300);
    assert_ne!(first.code_verifier, second.code_verifier);
    for started in [&first, &second] {
        assert_eq!(started.code_verifier.len(), 43);
        assert!(apple_web::valid_code_verifier(&started.code_verifier));
    }
    let digest = |verifier: &str| URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    assert_eq!(
        api.calls(),
        [
            SignInApi::call(
                "apple_web",
                &digest(&first.code_verifier),
                "app.msime.android.pinyin"
            ),
            SignInApi::call(
                "apple_web",
                &digest(&second.code_verifier),
                "app.msime.android"
            ),
        ]
    );
}

/// 期望值由 Python 的 `hashlib.sha256` 与 `base64.urlsafe_b64encode` 独立算出：verifier 是字节 0..32 的无填充 base64url。
#[test]
fn pkce_matches_an_independently_computed_s256_digest() {
    let verifier = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
    assert_eq!(
        apple_web::code_challenge(verifier),
        "6oZqdX5MOLq_qBJ8vppAnT4fk6AP8UiP9zX8-Rev_9A"
    );
    assert!(apple_web::valid_code_verifier(verifier));
    assert!(apple_web::valid_code_verifier(&"a~.-_".repeat(25)[..125]));
    for verifier in [
        "a".repeat(42),
        "a".repeat(129),
        format!("{}=", "a".repeat(43)),
        format!("{}+", "a".repeat(43)),
    ] {
        assert!(!apple_web::valid_code_verifier(&verifier), "{verifier}");
    }
}

#[test]
fn an_apple_authorization_url_must_be_apples_own_page_with_one_state() {
    let (session, api, _) = session();
    let good = apple_authorization_url(STATE);
    for url in [
        good.replace("https://", "http://"),
        good.replace("appleid.apple.com", "appleid.apple.com.evil.test"),
        good.replace("appleid.apple.com", "appleid.apple.com:8443"),
        good.replace("https://", "https://user@"),
        good.replace("/auth/authorize", "/auth/other"),
        format!("{good}#fragment"),
        format!("{good}&state=second"),
        apple_authorization_url(""),
        good.replace("state=", "other="),
        format!("{good}&x=\"quoted\""),
        format!("{good}&{}", "x".repeat(4096)),
    ] {
        *api.apple_web_url.lock().unwrap() = url.clone();
        assert_eq!(
            session.apple_web_start("app.msime.android").unwrap_err(),
            AccountError::Unavailable,
            "{url}"
        );
    }
}

#[test]
fn an_apple_web_start_refuses_an_app_id_that_cannot_be_a_package_name() {
    let (session, api, _) = session();
    for app in [
        "",
        "App.msime.android",
        "app..android",
        "app.msime.android.",
        "app.1msime",
        "app.msime/android",
        "app.msime android",
        &"a".repeat(129),
    ] {
        assert_eq!(
            session.apple_web_start(app).unwrap_err(),
            AccountError::Invalid,
            "{app:?}"
        );
    }
    assert_eq!(api.network.load(Ordering::SeqCst), 0);
}

#[test]
fn an_apple_web_grant_is_exchanged_with_its_verifier_and_saved() {
    let (session, api, storage) = session();
    let grant = URL_SAFE_NO_PAD.encode([7_u8; 32]);
    let verifier = URL_SAFE_NO_PAD.encode([9_u8; 32]);
    assert_eq!(
        session.sign_in_apple_web(&grant, &verifier).unwrap(),
        user()
    );
    assert!(storage.load().unwrap().is_some());
    assert_eq!(
        api.calls(),
        [SignInApi::call("apple_web_login", &grant, &verifier)]
    );
}

#[test]
fn a_malformed_apple_web_grant_or_verifier_is_refused_before_the_network() {
    let (session, api, storage) = session();
    let grant = URL_SAFE_NO_PAD.encode([7_u8; 32]);
    let verifier = URL_SAFE_NO_PAD.encode([9_u8; 32]);
    // 43 个字符的最后一个只用到 4 位，`B` 留下了非零的多余位，不是规范编码。
    let non_canonical = format!("{}B", &grant[..42]);
    assert!(apple_web::valid_apple_web_grant(&format!(
        "{}A",
        &grant[..42]
    )));
    for bad_grant in [
        String::new(),
        grant[..42].to_owned(),
        format!("{grant}A"),
        format!("{grant}="),
        non_canonical,
        URL_SAFE_NO_PAD.encode([7_u8; 31]),
        grant.replace(&grant[..1], "+"),
    ] {
        assert_eq!(
            session.sign_in_apple_web(&bad_grant, &verifier),
            Err(AccountError::Invalid),
            "{bad_grant:?}"
        );
    }
    for bad_verifier in [
        "short".to_owned(),
        "a".repeat(129),
        format!("{}!", &verifier[..42]),
    ] {
        assert_eq!(
            session.sign_in_apple_web(&grant, &bad_verifier),
            Err(AccountError::Invalid),
            "{bad_verifier:?}"
        );
    }
    assert_eq!(api.network.load(Ordering::SeqCst), 0);
    assert!(storage.load().unwrap().is_none());
}
