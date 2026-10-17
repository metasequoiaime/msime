//! 给已登录账号添加登录方式（`purpose:"link"`，#8201 的决定 9）：`BackendAccountClient::loopback` 连本机假服务器，回放 msime-cloud `origin/main` 的 `begin`、`login`、`appleWebBegin`、`appleWebLogin` 的响应形状，核对请求带着登录账号的令牌和 `purpose:"link"`、需要最近登录和已被别的账号绑定时的码，以及绑定成功后本地会话换成服务端新签发的那一个。

use super::transport_tests::{error_response, http, json_response, read_request, Received};
use super::*;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};

const USER: &str = "fixture-user";
const EMAIL: &str = "person@example.test";
/// 形状合格的合成 JWT：三段 base64url，内容无意义。
const ID_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.eyJub25jZSI6ImZpeHR1cmUifQ.c2lnbmF0dXJl";
const APPLE_URL: &str = "https://appleid.apple.com/auth/authorize?client_id=app.msime.signin&state=fixture-state&nonce=fixture-nonce";

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn saved(access: char, refresh: char, user: &str, expires_in_ms: u64) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: token(access),
            refresh_token: token(refresh),
            token_type: "Bearer".into(),
            expires_in: 900,
            user: AccountUser {
                id: user.into(),
                display_name: "Fixture".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: now_ms() + expires_in_ms,
        session_id: Some(uuid::Uuid::new_v4()),
    }
}

/// 会话里测试关心的部分：令牌、账号和登录 ID（`SavedAccountSession` 不实现 `PartialEq` 和 `Debug`，令牌不进日志）。
type Key = (String, String, String, Option<uuid::Uuid>);

fn key(session: &SavedAccountSession) -> Option<Key> {
    Some((
        session.tokens.access_token.clone(),
        session.tokens.refresh_token.clone(),
        session.tokens.user.id.clone(),
        session.session_id,
    ))
}

/// 服务端签发的会话，响应与 `/v1/auth/login` 相同。
fn issued(access: char, refresh: char, user: &str) -> Vec<u8> {
    json_response(
        "200 OK",
        json!({
            "access_token": token(access),
            "refresh_token": token(refresh),
            "token_type": "Bearer",
            "expires_in": 900,
            "user": { "id": user, "display_name": "Fixture", "created_at": "2026-01-01T00:00:00Z" }
        }),
    )
}

fn email_challenge() -> Vec<u8> {
    json_response(
        "201 Created",
        json!({ "challenge_id": "c".repeat(64), "expires_in": 300 }),
    )
}

/// 内存里的会话存储，按「别的进程也在读写」处理：每次访问都重新读，测试可以从服务端线程里替换它，模拟另一个进程刷新、退出或换号。
#[derive(Clone)]
struct Shared(Arc<Mutex<Option<SavedAccountSession>>>);

impl Shared {
    fn with(session: Option<SavedAccountSession>) -> Self {
        Self(Arc::new(Mutex::new(session)))
    }

    fn current(&self) -> Option<SavedAccountSession> {
        self.0.lock().unwrap().clone()
    }

    fn key(&self) -> Option<Key> {
        self.current().as_ref().and_then(key)
    }

    fn replace(&self, session: Option<SavedAccountSession>) {
        *self.0.lock().unwrap() = session;
    }
}

impl AccountSessionStorage for Shared {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(self.current())
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        self.replace(Some(session.clone()));
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        self.replace(None);
        Ok(())
    }
    fn shared_across_processes(&self) -> bool {
        true
    }
}

/// 一个回合：收到请求以后先调用 `before`（例如替换会话存储），再回 `response`。
type Step = (Vec<u8>, Option<Box<dyn FnOnce() + Send>>);

/// 依次回答每一个回合，每个连接一个；收到的请求按顺序交回。
fn serve(steps: Vec<Step>) -> (String, Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for (response, before) in steps {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            if let Some(before) = before {
                before();
            }
            sent.send(request).unwrap();
            std::io::Write::write_all(&mut stream, &response).unwrap();
        }
    });
    (origin, received)
}

fn plain(responses: Vec<Vec<u8>>) -> Vec<Step> {
    responses
        .into_iter()
        .map(|response| (response, None))
        .collect()
}

fn session(origin: &str, storage: &Shared) -> BackendAccountSession<BackendAccountClient, Shared> {
    BackendAccountSession::new(
        BackendAccountClient::loopback(origin).unwrap(),
        storage.clone(),
    )
}

fn bearer(request: &Received) -> Option<String> {
    request.header("authorization").map(str::to_owned)
}

fn bearer_of(character: char) -> Option<String> {
    Some(format!("Bearer {}", token(character)))
}

#[test]
fn every_link_request_carries_the_account_token_and_purpose_link() {
    let (origin, received) = serve(plain(vec![
        email_challenge(),
        json_response(
            "201 Created",
            json!({ "challenge_id": "d".repeat(64), "expires_in": 300, "nonce": "fixture-nonce" }),
        ),
        issued('e', 'f', USER),
        json_response(
            "201 Created",
            json!({ "authorization_url": APPLE_URL, "expires_in": 300 }),
        ),
        issued('e', 'f', USER),
    ]));
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let access = token('a');
    client.link_challenge("email", EMAIL, &access).unwrap();
    let google = client.link_challenge("google", "", &access).unwrap();
    assert_eq!(google.nonce.as_deref(), Some("fixture-nonce"));
    client
        .link_login(&"c".repeat(64), "123456", &access)
        .unwrap();
    let code_challenge = apple_web::code_challenge(&"v".repeat(43));
    let apple = client
        .apple_web_link_challenge(&code_challenge, "app.msime.android", &access)
        .unwrap();
    assert_eq!(apple.authorization_url, APPLE_URL);
    let grant = "g".repeat(42) + "A";
    let verifier = "v".repeat(43);
    client
        .apple_web_link_login(&grant, &verifier, &access)
        .unwrap();

    let sent: Vec<_> = received.iter().collect();
    let expected: [(&str, Value); 5] = [
        (
            "POST /v1/auth/challenges HTTP/1.1",
            json!({ "provider": "email", "target": EMAIL, "purpose": "link" }),
        ),
        (
            "POST /v1/auth/challenges HTTP/1.1",
            json!({ "provider": "google", "target": "", "purpose": "link" }),
        ),
        (
            "POST /v1/auth/login HTTP/1.1",
            json!({ "challenge_id": "c".repeat(64), "credential": "123456" }),
        ),
        (
            "POST /v1/auth/apple/web HTTP/1.1",
            json!({ "code_challenge": code_challenge, "code_challenge_method": "S256", "app": "app.msime.android", "purpose": "link" }),
        ),
        (
            "POST /v1/auth/apple/web/login HTTP/1.1",
            json!({ "grant": grant, "code_verifier": verifier }),
        ),
    ];
    assert_eq!(sent.len(), expected.len());
    for (request, (line, body)) in sent.iter().zip(expected) {
        assert_eq!(request.line(), line);
        assert_eq!(bearer(request), bearer_of('a'), "{line}");
        assert_eq!(request.json(), body, "{line}");
    }
}

#[test]
fn a_finished_link_replaces_the_local_session_with_the_one_the_server_issued() {
    let (origin, received) = serve(plain(vec![
        email_challenge(),
        issued('e', 'f', USER),
        json_response(
            "201 Created",
            json!({ "challenge_id": "d".repeat(64), "expires_in": 300, "nonce": "fixture-nonce" }),
        ),
        issued('1', '2', USER),
        json_response(
            "201 Created",
            json!({ "authorization_url": APPLE_URL, "expires_in": 300 }),
        ),
        issued('3', '4', USER),
    ]));
    let before = saved('a', 'b', USER, 3_600_000);
    let storage = Shared::with(Some(before.clone()));
    let session = session(&origin, &storage);

    let challenge = session.link_request_code("email", EMAIL).unwrap();
    assert_eq!(challenge.challenge_id, "c".repeat(64));
    // 发验证码不改本地会话。
    assert_eq!(storage.key(), key(&before));
    let user = session
        .link_email(&challenge.challenge_id, "123456")
        .unwrap();
    assert_eq!(user.id, USER);
    let linked = storage.current().unwrap();
    assert_eq!(linked.tokens.access_token, token('e'));
    assert_eq!(linked.tokens.refresh_token, token('f'));
    // 新会话是一次新的登录，有自己的登录 ID。
    assert!(linked.session_id.is_some());
    assert_ne!(linked.session_id, before.session_id);

    // 接着绑定的请求用的是新会话的令牌。
    let google = session.link_google_id_token_challenge().unwrap();
    session
        .link_google_id_token(&google.challenge_id, ID_TOKEN)
        .unwrap();
    assert_eq!(storage.current().unwrap().tokens.access_token, token('1'));
    let started = session.link_apple_web_start("app.msime.android").unwrap();
    assert_eq!(started.authorization_url, APPLE_URL);
    session
        .link_apple_web(&("g".repeat(42) + "A"), &started.code_verifier)
        .unwrap();
    assert_eq!(storage.current().unwrap().tokens.access_token, token('3'));

    let tokens: Vec<_> = received.iter().map(|request| bearer(&request)).collect();
    assert_eq!(
        tokens,
        [
            bearer_of('a'),
            bearer_of('a'),
            bearer_of('e'),
            bearer_of('e'),
            bearer_of('1'),
            bearer_of('1'),
        ]
    );
}

#[test]
fn refusals_keep_the_local_session_and_report_what_the_host_can_act_on() {
    let (origin, received) = serve(plain(vec![
        error_response("403 Forbidden", "recent_login_required", &[]),
        error_response("409 Conflict", "identity_already_linked", &[]),
        error_response("401 Unauthorized", "invalid_credentials", &[]),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "60")],
        ),
        error_response("400 Bad Request", "invalid_grant", &[]),
    ]));
    let before = saved('a', 'b', USER, 3_600_000);
    let storage = Shared::with(Some(before.clone()));
    let session = session(&origin, &storage);

    // 会话不是 10 分钟内登录的：宿主让用户重新登录同一个账号后再来。
    assert_eq!(
        session
            .link_request_code("email", EMAIL)
            .unwrap_err()
            .code(),
        "account_recent_login_required"
    );
    // 这个登录方式已经绑在别的账号上。
    assert_eq!(
        session
            .link_google_id_token(&"d".repeat(64), ID_TOKEN)
            .unwrap_err()
            .code(),
        "account_conflict"
    );
    // 验证码不对：401 不刷新、不重试，本地会话留着。
    assert_eq!(
        session
            .link_email(&"c".repeat(64), "000000")
            .unwrap_err()
            .code(),
        "account_unauthorized"
    );
    let limited = session.link_request_code("email", EMAIL).unwrap_err();
    assert_eq!(
        (limited.code(), limited.retry_after_seconds),
        ("account_rate_limited", Some(60))
    );
    // Apple 的 grant 不认识、用过或过期：宿主保留等待中的流程。
    let verifier = "v".repeat(43);
    assert_eq!(
        session
            .link_apple_web(&("g".repeat(42) + "A"), &verifier)
            .unwrap_err()
            .code(),
        "account_invalid"
    );
    assert_eq!(storage.key(), key(&before));
    // 五次请求，没有一次刷新。
    let lines: Vec<_> = received
        .iter()
        .map(|request| request.line().to_owned())
        .collect();
    assert_eq!(
        lines,
        [
            "POST /v1/auth/challenges HTTP/1.1",
            "POST /v1/auth/login HTTP/1.1",
            "POST /v1/auth/login HTTP/1.1",
            "POST /v1/auth/challenges HTTP/1.1",
            "POST /v1/auth/apple/web/login HTTP/1.1",
        ]
    );
}

#[test]
fn a_link_without_a_signed_in_account_or_with_a_bad_request_never_reaches_the_server() {
    // 没有假服务器：这些请求都不该发出去。
    let storage = Shared::with(None);
    let session = session("http://127.0.0.1:9", &storage);
    let verifier = "v".repeat(43);
    let unauthorized = [
        session.link_request_code("email", EMAIL).unwrap_err(),
        session.link_google_id_token_challenge().unwrap_err(),
        session
            .link_apple_web_start("app.msime.android")
            .unwrap_err(),
        session.link_email(&"c".repeat(64), "123456").unwrap_err(),
        session
            .link_google_id_token(&"d".repeat(64), ID_TOKEN)
            .unwrap_err(),
        session
            .link_apple_web(&("g".repeat(42) + "A"), &verifier)
            .unwrap_err(),
    ];
    for failure in unauthorized {
        assert_eq!(failure.code(), "account_unauthorized");
        assert!(!failure.network_failure);
    }
    assert!(storage.current().is_none());

    // 参数不合法时连会话都不看：只能绑邮箱，验证码 6 位，Google 要 JWT，Apple 的 app、grant 和 verifier 按登录的规则。
    storage.replace(Some(saved('a', 'b', USER, 3_600_000)));
    let invalid = [
        session
            .link_request_code("phone", "+8613800000000")
            .unwrap_err(),
        session.link_request_code("google", "").unwrap_err(),
        session.link_request_code("email", "").unwrap_err(),
        session.link_email(&"c".repeat(64), "12345").unwrap_err(),
        session
            .link_google_id_token(&"d".repeat(64), "not-a-jwt")
            .unwrap_err(),
        session.link_apple_web_start("App.Msime").unwrap_err(),
        session
            .link_apple_web(&("g".repeat(42) + "h"), &verifier)
            .unwrap_err(),
        session
            .link_apple_web(&("g".repeat(42) + "A"), "short")
            .unwrap_err(),
    ];
    for failure in invalid {
        assert_eq!(failure.code(), "account_invalid");
    }
}

#[test]
fn an_expiring_or_rejected_token_is_refreshed_before_starting_a_link() {
    let (origin, received) = serve(plain(vec![
        // 令牌快过期：先刷新，再带新令牌申请。
        issued('c', 'd', USER),
        email_challenge(),
        // 令牌被拒：带着它刷新一次，在同一个账号里重试。
        error_response("401 Unauthorized", "invalid_credentials", &[]),
        issued('e', 'f', USER),
        email_challenge(),
    ]));
    let storage = Shared::with(Some(saved('a', 'b', USER, 10_000)));
    let session = session(&origin, &storage);
    session.link_request_code("email", EMAIL).unwrap();
    assert_eq!(storage.current().unwrap().tokens.refresh_token, token('d'));
    session.link_request_code("email", EMAIL).unwrap();
    assert_eq!(storage.current().unwrap().tokens.refresh_token, token('f'));

    let sent: Vec<_> = received.iter().collect();
    let shape: Vec<_> = sent
        .iter()
        .map(|request| (request.line().to_owned(), bearer(request)))
        .collect();
    assert_eq!(
        shape,
        [
            ("POST /v1/auth/refresh HTTP/1.1".to_owned(), None),
            (
                "POST /v1/auth/challenges HTTP/1.1".to_owned(),
                bearer_of('c')
            ),
            (
                "POST /v1/auth/challenges HTTP/1.1".to_owned(),
                bearer_of('c')
            ),
            ("POST /v1/auth/refresh HTTP/1.1".to_owned(), None),
            (
                "POST /v1/auth/challenges HTTP/1.1".to_owned(),
                bearer_of('e')
            ),
        ]
    );
    assert_eq!(sent[0].json(), json!({ "refresh_token": token('b') }));
    assert_eq!(sent[3].json(), json!({ "refresh_token": token('d') }));
}

#[test]
fn a_link_that_finishes_after_another_process_changed_the_session() {
    let first = saved('a', 'b', USER, 3_600_000);
    // 另一个进程在绑定进行中刷新了同一次登录（同一个登录 ID、新令牌）：绑定照常写入。
    let mut refreshed = saved('5', '6', USER, 3_600_000);
    refreshed.session_id = first.session_id;
    // 另一个进程在绑定进行中换成了别的账号：新会话丢掉，别的账号的会话不动。
    let other = saved('7', '8', "fixture-other", 3_600_000);
    // 另一个进程在绑定进行中退出、又登录了同一个账号（新的登录 ID）：同样不写入。
    let again = saved('9', '0', USER, 3_600_000);
    let storage = Shared::with(Some(first));
    let (refresh_to, switch_to, relogin_to) = (storage.clone(), storage.clone(), storage.clone());
    let (refreshed_copy, other_copy, again_copy) =
        (refreshed.clone(), other.clone(), again.clone());
    let (origin, _received) = serve(vec![
        (
            issued('e', 'f', USER),
            Some(Box::new(move || refresh_to.replace(Some(refreshed_copy)))),
        ),
        (
            issued('1', '2', USER),
            Some(Box::new(move || switch_to.replace(Some(other_copy)))),
        ),
        (
            issued('3', '4', USER),
            Some(Box::new(move || relogin_to.replace(Some(again_copy)))),
        ),
    ]);
    let session = session(&origin, &storage);
    session.link_email(&"c".repeat(64), "123456").unwrap();
    assert_eq!(storage.current().unwrap().tokens.access_token, token('e'));

    for expected in [&other, &again] {
        storage.replace(Some(saved('a', 'b', USER, 3_600_000)));
        assert_eq!(
            session
                .link_email(&"c".repeat(64), "123456")
                .unwrap_err()
                .code(),
            "account_cancelled"
        );
        assert_eq!(storage.key(), key(expected));
    }
}

#[test]
fn a_link_answered_for_another_account_is_not_kept() {
    let (origin, _received) = serve(plain(vec![issued('e', 'f', "fixture-other")]));
    let before = saved('a', 'b', USER, 3_600_000);
    let storage = Shared::with(Some(before.clone()));
    let session = session(&origin, &storage);
    assert_eq!(
        session
            .link_email(&"c".repeat(64), "123456")
            .unwrap_err()
            .code(),
        "account_unavailable"
    );
    assert_eq!(storage.key(), key(&before));
}

#[test]
fn a_link_without_a_response_is_a_network_failure() {
    let (origin, _received) = serve(plain(vec![Vec::new(), http("204 No Content", &[], b"")]));
    let storage = Shared::with(Some(saved('a', 'b', USER, 3_600_000)));
    let session = session(&origin, &storage);
    assert_eq!(
        session
            .link_request_code("email", EMAIL)
            .unwrap_err()
            .code(),
        "account_network"
    );
    // 204 没有正文，读不出 challenge。
    assert_eq!(
        session
            .link_request_code("email", EMAIL)
            .unwrap_err()
            .code(),
        "account_unavailable"
    );
}

/// 宿主注入的存储：字节在内存里，记下每次加锁和解锁的项。
#[derive(Default)]
struct HostBackend {
    items: Mutex<std::collections::HashMap<u32, Vec<u8>>>,
    locks: Mutex<Vec<(u32, bool)>>,
}

impl SessionStoreBackend for HostBackend {
    fn read(
        &self,
        item: SessionStoreItem,
        _capacity: usize,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        Ok(self.items.lock().unwrap().get(&item.raw()).cloned())
    }
    fn write(&self, item: SessionStoreItem, bytes: &[u8]) -> Result<WriteOutcome, AccountError> {
        self.items
            .lock()
            .unwrap()
            .insert(item.raw(), bytes.to_vec());
        Ok(WriteOutcome::Written)
    }
    fn remove(&self, item: SessionStoreItem) -> Result<(), AccountError> {
        self.items.lock().unwrap().remove(&item.raw());
        Ok(())
    }
    fn lock(&self, item: SessionStoreItem, _timeout: Duration) -> Result<u64, AccountError> {
        self.locks.lock().unwrap().push((item.raw(), true));
        Ok(1)
    }
    fn unlock(&self, item: SessionStoreItem, _token: u64) {
        self.locks.lock().unwrap().push((item.raw(), false));
    }
    fn shared_across_processes(&self) -> bool {
        true
    }
    fn layout(&self) -> AccountSessionFileLayout {
        AccountSessionFileLayout::Native
    }
}

/// 按目录的文件存储和宿主注入的存储都一样：绑定得到的新会话在刷新锁里写进存储，下一次读到的就是它。
#[test]
fn a_link_is_saved_in_file_and_host_injected_storage_under_the_refresh_lock() {
    let directory = tempfile::tempdir().unwrap();
    let file = FileAccountSessionStorage::new(directory.path(), AccountSessionFileLayout::Native);
    file.save(&saved('a', 'b', USER, 3_600_000)).unwrap();
    let backend = Arc::new(HostBackend::default());
    let host = HostSessionStorage::new(
        backend.clone(),
        HostSessionSlot::Account,
        Arc::new(HostStoreLocks::new()),
    );
    host.save(&saved('a', 'b', USER, 3_600_000)).unwrap();

    let (origin, received) = serve(plain(vec![issued('e', 'f', USER), issued('e', 'f', USER)]));
    let client = BackendAccountClient::loopback(&origin).unwrap();
    BackendAccountSession::new(client.clone(), file)
        .link_email(&"c".repeat(64), "123456")
        .unwrap();
    BackendAccountSession::new(client, host)
        .link_email(&"c".repeat(64), "123456")
        .unwrap();

    let reread = FileAccountSessionStorage::new(directory.path(), AccountSessionFileLayout::Native)
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(reread.tokens.access_token, token('e'));
    let reread = HostSessionStorage::new(
        backend.clone(),
        HostSessionSlot::Account,
        Arc::new(HostStoreLocks::new()),
    )
    .load()
    .unwrap()
    .unwrap();
    assert_eq!(reread.tokens.access_token, token('e'));
    // 只为写入新会话拿了一次登录会话的锁（令牌没过期，不用刷新），并成对释放。
    let account = SessionStoreItem::Account.raw();
    assert_eq!(
        *backend.locks.lock().unwrap(),
        [(account, true), (account, false)]
    );
    for request in received.iter() {
        assert_eq!(bearer(&request), bearer_of('a'));
    }
}
