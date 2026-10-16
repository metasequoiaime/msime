//! 账号会话经驱动访问云剪贴板：用一个按 `docs/cloud-clipboard-e2e.md` 实现的内存服务端，跑两台设备从开启、待批准、批准到移除的完整流程。

use super::*;
use crate::cloud::clipboard_e2e::{HttpRequest, HttpResponse};
use serde_json::json;
use std::collections::BTreeMap;

fn token(byte: u8) -> String {
    std::iter::repeat_n(char::from(byte), 64).collect()
}

fn tokens(access: u8, refresh: u8) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token(refresh),
        token_type: "Bearer".into(),
        expires_in: 900,
        user: AccountUser {
            id: "fixture-user".into(),
            display_name: "Fixture".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        },
    }
}

#[derive(Clone)]
struct Device {
    device_id: String,
    public_key: String,
    name: String,
}

#[derive(Default)]
struct Server {
    enabled: bool,
    key_id: Option<String>,
    devices: Vec<Device>,
    wraps: BTreeMap<String, String>,
    items: Vec<Value>,
    /// 带这个令牌的请求回 401，模拟访问令牌在操作中途过期。
    rejected_token: Option<String>,
    /// 每个请求的方法、路径（不含查询）和令牌。
    log: Vec<(String, String, String)>,
}

fn response(status: u16, body: Value) -> HttpResponse {
    HttpResponse {
        status,
        body: if body.is_null() {
            String::new()
        } else {
            body.to_string()
        },
    }
}

fn error(status: u16, code: &str) -> HttpResponse {
    response(
        status,
        json!({ "error": { "code": code, "message": code } }),
    )
}

impl Server {
    fn handle(&mut self, request: &HttpRequest, token: &str) -> HttpResponse {
        let path = request.path.as_str();
        self.log.push((
            request.method.clone(),
            path.split('?').next().unwrap().to_owned(),
            token.to_owned(),
        ));
        if self.rejected_token.as_deref() == Some(token) {
            return error(401, "unauthorized");
        }
        let body: Value = request
            .body
            .as_deref()
            .map(|body| serde_json::from_str(body).unwrap())
            .unwrap_or(Value::Null);
        let api = "/v1/users/me/clipboard";
        match (request.method.as_str(), path) {
            ("GET", p) if p.starts_with(&format!("{api}/keys?device_id=")) => {
                let device_id = p.rsplit('=').next().unwrap();
                let devices = self
                    .devices
                    .iter()
                    .map(|device| {
                        json!({
                            "device_id": device.device_id,
                            "public_key": device.public_key,
                            "name": device.name,
                            "created_at": "2026-10-11T08:00:00Z",
                            "approved": self.wraps.contains_key(&device.device_id),
                        })
                    })
                    .collect::<Vec<_>>();
                response(
                    200,
                    json!({
                        "enabled": self.enabled,
                        "key_id": self.key_id,
                        "wrap": self.wraps.get(device_id),
                        "devices": devices,
                    }),
                )
            }
            ("PUT", p) if p.starts_with(&format!("{api}/devices/")) => {
                let device_id = p.rsplit('/').next().unwrap().to_owned();
                if !self.devices.iter().any(|d| d.device_id == device_id) {
                    self.devices.push(Device {
                        device_id,
                        public_key: body["public_key"].as_str().unwrap().into(),
                        name: body["name"].as_str().unwrap().into(),
                    });
                }
                response(204, Value::Null)
            }
            ("DELETE", p) if p.starts_with(&format!("{api}/devices/")) => {
                let device_id = p.rsplit('/').next().unwrap();
                let before = self.devices.len();
                self.devices.retain(|d| d.device_id != device_id);
                self.wraps.remove(device_id);
                if before == self.devices.len() {
                    error(404, "not_found")
                } else {
                    response(204, Value::Null)
                }
            }
            ("POST", p) if p == format!("{api}/keys") => {
                if body["previous_key_id"].as_str() != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                let revoke = body["revoke"].as_array().unwrap().clone();
                self.devices
                    .retain(|d| !revoke.iter().any(|r| r.as_str() == Some(&d.device_id)));
                self.items.clear();
                self.wraps.clear();
                self.key_id = body["key_id"].as_str().map(str::to_owned);
                for wrap in body["wraps"].as_array().unwrap() {
                    self.wraps.insert(
                        wrap["device_id"].as_str().unwrap().into(),
                        wrap["wrap"].as_str().unwrap().into(),
                    );
                }
                response(204, Value::Null)
            }
            ("PUT", p) if p.starts_with(&format!("{api}/keys/")) => {
                let parts = p.split('/').collect::<Vec<_>>();
                let (key_id, device_id) = (parts[6], parts[8]);
                if Some(key_id) != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                if !self.devices.iter().any(|d| d.device_id == device_id) {
                    return error(404, "not_found");
                }
                self.wraps
                    .insert(device_id.into(), body["wrap"].as_str().unwrap().into());
                response(204, Value::Null)
            }
            ("GET", p) if p == api => response(
                200,
                json!({
                    "enabled": self.enabled,
                    "retention_days": 7,
                    "key_id": self.key_id,
                    "items": self.items,
                }),
            ),
            ("POST", p) if p == api => {
                if body["key_id"].as_str() != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                let item = json!({
                    "id": format!("{:064x}", self.items.len() + 1),
                    "ciphertext": body["ciphertext"],
                    "key_id": body["key_id"],
                    "updated_at": "2026-10-11T08:00:00Z",
                    "pinned": false,
                    "device": "Fixture",
                });
                self.items.insert(0, item.clone());
                response(200, item)
            }
            _ => error(404, "not_found"),
        }
    }
}

struct Api {
    server: Arc<Mutex<Server>>,
}

impl AccountApi for Api {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        Ok(std::collections::HashMap::new())
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        Ok(tokens(b'a', b'b'))
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        Ok(tokens(b'c', b'd'))
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Ok(())
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Ok(())
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Ok(())
    }
    fn clipboard_e2e(
        &self,
        request: &HttpRequest,
        access_token: &str,
    ) -> Result<HttpResponse, AccountError> {
        Ok(self.server.lock().unwrap().handle(request, access_token))
    }
}

/// 一台设备的会话存储：会话和密钥串都在内存里，记下每次保存的密钥串。
#[derive(Clone, Default)]
struct Storage {
    session: Arc<Mutex<Option<SavedAccountSession>>>,
    keystore: Arc<Mutex<Option<String>>>,
    saves: Arc<Mutex<Vec<String>>>,
}

impl AccountSessionStorage for Storage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(self.session.lock().unwrap().clone())
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.session.lock().unwrap() = Some(session.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        *self.session.lock().unwrap() = None;
        *self.keystore.lock().unwrap() = None;
        Ok(())
    }
    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        Ok(self.keystore.lock().unwrap().clone())
    }
    fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
        *self.keystore.lock().unwrap() = Some(value.to_owned());
        self.saves.lock().unwrap().push(value.to_owned());
        Ok(())
    }
}

type Session = BackendAccountSession<Api, Storage>;

fn server() -> Arc<Mutex<Server>> {
    Arc::new(Mutex::new(Server {
        enabled: true,
        ..Server::default()
    }))
}

fn device(server: &Arc<Mutex<Server>>) -> (Session, Storage) {
    let storage = Storage::default();
    let session = BackendAccountSession::new(
        Api {
            server: Arc::clone(server),
        },
        storage.clone(),
    );
    session.sign_in("synthetic-challenge", "123456").unwrap();
    (session, storage)
}

fn clipboard_code(result: Result<impl std::fmt::Debug, AccountClipboardError>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn two_devices_go_from_first_use_through_approval_to_removal() {
    let server = server();
    let (first, first_storage) = device(&server);
    let (second, second_storage) = device(&server);

    // 第一台设备第一次打开：登记、建钥，随后能上传和读回自己写的条目。
    let page = first.clipboard("").unwrap();
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert!(page.items.is_empty());
    let added = first.add_clipboard("合成文本 one").unwrap();
    assert_eq!(added.text, "合成文本 one");
    first.add_clipboard("合成文本 two").unwrap();
    let page = first.clipboard("one").unwrap();
    assert_eq!(page.retention_days, 7);
    assert_eq!(
        page.items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>(),
        ["合成文本 one"]
    );
    assert!(first_storage.keystore.lock().unwrap().is_some());

    // 第二台设备：登记后等待批准，读不到条目，上传被拒绝。
    let pending = second.clipboard("").unwrap();
    assert_eq!(pending.e2e.status, AccountClipboardE2eStatus::Pending);
    assert!(pending.items.is_empty());
    assert_eq!(
        clipboard_code(second.add_clipboard("合成文本 three")),
        "clipboard_device_pending"
    );
    let second_code = pending.e2e.code.clone();

    // 第一台设备看到待批准的设备和同一个核对码，批准它。
    assert_eq!(first.clipboard("").unwrap().e2e.pending_devices, 1);
    let devices = first.clipboard_devices().unwrap();
    let waiting = devices.devices.iter().find(|d| !d.approved).unwrap();
    assert_eq!(waiting.code, second_code);
    assert!(!waiting.current);
    let second_id = waiting.device_id.clone();
    let approved = first.approve_clipboard_device(&second_id).unwrap();
    assert!(approved.devices.iter().all(|d| d.approved));

    let page = second.clipboard("").unwrap();
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert_eq!(page.items.len(), 2);

    // 移除第二台设备：换钥、清空条目，第二台设备回到待批准。
    let removed = first.remove_clipboard_device(&second_id).unwrap();
    assert_eq!(removed.devices.len(), 1);
    assert!(first.clipboard("").unwrap().items.is_empty());
    assert_eq!(
        second.clipboard("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    // 拒绝它这次重新登记。
    let rejected = first.reject_clipboard_device(&second_id).unwrap();
    assert_eq!(rejected.devices.len(), 1);

    // 第二台设备重置云剪贴板后自己持有新密钥，第一台反过来需要批准。
    let reset = second.reset_clipboard().unwrap();
    assert_eq!(reset.status, AccountClipboardE2eStatus::Ready);
    assert_eq!(
        first.clipboard("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    assert!(second_storage.saves.lock().unwrap().len() >= 2);
}

#[test]
fn every_request_carries_the_access_token_and_an_expired_one_is_refreshed_once() {
    let server = server();
    let (session, _) = device(&server);
    session.clipboard("").unwrap();
    assert!(server
        .lock()
        .unwrap()
        .log
        .iter()
        .all(|(_, _, token_used)| *token_used == token(b'a')));

    server.lock().unwrap().log.clear();
    server.lock().unwrap().rejected_token = Some(token(b'a'));
    let page = session.clipboard("").unwrap();
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    let log = server.lock().unwrap().log.clone();
    assert_eq!(log[0].2, token(b'a'));
    assert!(log[1..].iter().all(|(_, _, used)| *used == token(b'c')));
    // 被拒的那个请求用新令牌重发，之后不再用旧令牌。
    assert_eq!(log[0].1, log[1].1);
}

#[test]
fn the_keystore_is_saved_before_the_next_request_is_sent() {
    let server = server();
    let (session, storage) = device(&server);
    session.clipboard("").unwrap();
    let saves = storage.saves.lock().unwrap().clone();
    // 第一次保存的是新生成的设备身份，最后一次带上了刚建好的剪贴板密钥。
    assert_eq!(saves.len(), 2);
    assert_eq!(saves[0].split('.').count(), 2);
    assert_eq!(saves[1].split('.').count(), 4);
    assert!(saves[1].starts_with(&saves[0]));

    // 已保存的密钥串原样沿用：同一台设备不会再登记一次。
    session.clipboard("").unwrap();
    assert_eq!(server.lock().unwrap().devices.len(), 1);
    assert_eq!(storage.saves.lock().unwrap().len(), 2);
}

#[test]
fn a_disabled_clipboard_lists_nothing_and_refuses_changes() {
    let server = server();
    server.lock().unwrap().enabled = false;
    let (session, _) = device(&server);
    let page = session.clipboard("").unwrap();
    assert!(!page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Disabled);
    assert_eq!(
        clipboard_code(session.add_clipboard("合成文本")),
        "clipboard_sync_disabled"
    );
    assert_eq!(
        clipboard_code(session.reset_clipboard()),
        "clipboard_sync_disabled"
    );
}

#[test]
fn invalid_arguments_and_a_signed_out_session_fail_without_requests() {
    let server = server();
    let storage = Storage::default();
    let signed_out = BackendAccountSession::new(
        Api {
            server: Arc::clone(&server),
        },
        storage.clone(),
    );
    assert_eq!(
        clipboard_code(signed_out.clipboard("")),
        "account_unauthorized"
    );
    let (session, _) = device(&server);
    for result in [
        session.approve_clipboard_device("ABC").map(|_| ()),
        session.remove_clipboard_device(&"g".repeat(32)).map(|_| ()),
        session.add_clipboard(" ").map(|_| ()),
        session.clipboard("bad\u{7}").map(|_| ()),
    ] {
        assert_eq!(clipboard_code(result), "account_invalid");
    }
    assert!(server.lock().unwrap().log.is_empty());
    assert!(storage.keystore.lock().unwrap().is_none());
}

#[test]
fn driver_codes_are_kept_and_unknown_ones_fall_back_to_the_status() {
    let error = |status, code: &str| {
        AccountClipboardError::Clipboard(DriverError {
            status,
            code: code.into(),
        })
        .code()
    };
    assert_eq!(
        error(0, "clipboard_device_pending"),
        "clipboard_device_pending"
    );
    assert_eq!(
        error(409, "clipboard_key_conflict"),
        "clipboard_key_conflict"
    );
    assert_eq!(
        error(409, "clipboard_device_limit"),
        "clipboard_device_limit"
    );
    assert_eq!(error(404, "not_found"), "not_found");
    assert_eq!(error(401, "unauthorized"), "account_unauthorized");
    assert_eq!(error(429, "http_429"), "account_rate_limited");
    assert_eq!(error(403, "account_banned"), "account_banned");
    assert_eq!(error(0, "something_new"), "account_unavailable");
    assert_eq!(
        cloud_clipboard_error_code("clipboard_sync_disabled"),
        Some("clipboard_sync_disabled")
    );
    assert_eq!(cloud_clipboard_error_code("conflict"), None);
}

#[test]
fn the_device_name_is_never_empty_or_overlong() {
    let name = cloud_clipboard_device_name();
    assert!(!name.trim().is_empty());
    assert!(name.chars().count() <= 64);
}
