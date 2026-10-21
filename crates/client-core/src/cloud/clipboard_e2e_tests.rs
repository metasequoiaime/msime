use super::*;
use std::collections::BTreeMap;

// ---- 按 docs/cloud-clipboard-e2e.md 实现的内存服务端 ----

#[derive(Clone)]
struct FakeDevice {
    device_id: String,
    public_key: String,
    name: String,
    created_at: String,
}

#[derive(Clone)]
struct FakeItem {
    id: String,
    ciphertext: String,
    key_id: String,
    dedup: String,
    sequence: u64,
}

struct FakeServer {
    enabled: bool,
    key_id: Option<String>,
    devices: Vec<FakeDevice>,
    wraps: BTreeMap<String, String>,
    items: Vec<FakeItem>,
    sequence: u64,
    /// 测试用：把发给某台设备的公钥换成别的，模拟撒谎的服务端。
    forged_public_keys: BTreeMap<String, String>,
    /// 测试用：把发给请求设备的封装换成这个。
    forged_wrap: Option<String>,
    log: Vec<String>,
}

fn error(status: u16, code: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: json!({ "error": { "code": code, "message": code } }).to_string(),
    }
}

fn ok(body: Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: body.to_string(),
    }
}

fn no_content() -> HttpResponse {
    HttpResponse {
        status: 204,
        body: String::new(),
    }
}

fn wrap_is_well_formed(wrap: &str) -> bool {
    wrap.len() == 142
        && wrap
            .strip_prefix("v1.")
            .and_then(|rest| B64.decode(rest).ok())
            .is_some_and(|bytes| bytes.len() == WRAP_BYTES)
}

impl FakeServer {
    fn new() -> Self {
        Self {
            enabled: true,
            key_id: None,
            devices: Vec::new(),
            wraps: BTreeMap::new(),
            items: Vec::new(),
            sequence: 0,
            forged_public_keys: BTreeMap::new(),
            forged_wrap: None,
            log: Vec::new(),
        }
    }

    fn item_json(item: &FakeItem) -> Value {
        json!({
            "id": item.id,
            "ciphertext": item.ciphertext,
            "key_id": item.key_id,
            "updated_at": format!("2026-10-11T00:00:{:02}Z", item.sequence % 60),
            "pinned": false,
            "device": "Test",
        })
    }

    fn handle(&mut self, request: &HttpRequest) -> HttpResponse {
        self.log.push(format!(
            "{} {}",
            request.method,
            request.path.split('?').next().unwrap()
        ));
        let body: Value = request
            .body
            .as_deref()
            .map(|body| serde_json::from_str(body).unwrap())
            .unwrap_or(Value::Null);
        let path = request.path.as_str();
        match (request.method.as_str(), path) {
            ("GET", p) if p.starts_with("/v1/users/me/clipboard/keys?device_id=") => {
                let device_id = &p["/v1/users/me/clipboard/keys?device_id=".len()..];
                let wrap = self
                    .forged_wrap
                    .clone()
                    .or_else(|| self.wraps.get(device_id).cloned());
                let devices = self
                    .devices
                    .iter()
                    .map(|device| {
                        json!({
                            "device_id": device.device_id,
                            "public_key": self
                                .forged_public_keys
                                .get(&device.device_id)
                                .unwrap_or(&device.public_key),
                            "name": device.name,
                            "created_at": device.created_at,
                            "approved": self.wraps.contains_key(&device.device_id),
                        })
                    })
                    .collect::<Vec<_>>();
                ok(json!({
                    "enabled": self.enabled,
                    "key_id": self.key_id,
                    "wrap": wrap,
                    "devices": devices,
                }))
            }
            ("PUT", p) if p.starts_with("/v1/users/me/clipboard/devices/") => {
                let device_id = &p["/v1/users/me/clipboard/devices/".len()..];
                let public_key = body["public_key"].as_str().unwrap().to_owned();
                let raw = decode_exact::<32>(&public_key).unwrap();
                if super::device_id(&raw) != device_id {
                    return error(400, "invalid_clipboard_device");
                }
                if let Some(device) = self.devices.iter_mut().find(|d| d.device_id == device_id) {
                    device.name = body["name"].as_str().unwrap().to_owned();
                } else {
                    self.devices.push(FakeDevice {
                        device_id: device_id.to_owned(),
                        public_key,
                        name: body["name"].as_str().unwrap().to_owned(),
                        created_at: "2026-10-11T00:00:00Z".to_owned(),
                    });
                }
                no_content()
            }
            ("DELETE", p) if p.starts_with("/v1/users/me/clipboard/devices/") => {
                let device_id = &p["/v1/users/me/clipboard/devices/".len()..];
                let before = self.devices.len();
                self.devices.retain(|d| d.device_id != device_id);
                self.wraps.remove(device_id);
                if before == self.devices.len() {
                    error(404, "not_found")
                } else {
                    no_content()
                }
            }
            ("POST", "/v1/users/me/clipboard/keys") => {
                if !self.enabled {
                    return error(403, "clipboard_sync_disabled");
                }
                if body["previous_key_id"].as_str() != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                let revoke = body["revoke"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let wraps = body["wraps"].as_array().unwrap();
                if wraps.is_empty() {
                    return error(400, "invalid_clipboard_key");
                }
                for wrap in wraps {
                    let device_id = wrap["device_id"].as_str().unwrap();
                    if revoke.iter().any(|r| r == device_id)
                        || !self.devices.iter().any(|d| d.device_id == device_id)
                        || !wrap_is_well_formed(wrap["wrap"].as_str().unwrap())
                    {
                        return error(400, "invalid_clipboard_key");
                    }
                }
                self.devices.retain(|d| !revoke.contains(&d.device_id));
                self.items.clear();
                self.wraps.clear();
                self.key_id = Some(body["key_id"].as_str().unwrap().to_owned());
                for wrap in wraps {
                    self.wraps.insert(
                        wrap["device_id"].as_str().unwrap().to_owned(),
                        wrap["wrap"].as_str().unwrap().to_owned(),
                    );
                }
                no_content()
            }
            ("PUT", p) if p.starts_with("/v1/users/me/clipboard/keys/") => {
                let rest = &p["/v1/users/me/clipboard/keys/".len()..];
                let (key_id, device_id) = rest.split_once("/wraps/").unwrap();
                if Some(key_id) != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                if !self.devices.iter().any(|d| d.device_id == device_id) {
                    return error(404, "not_found");
                }
                let wrap = body["wrap"].as_str().unwrap();
                if !wrap_is_well_formed(wrap) {
                    return error(400, "invalid_clipboard_key");
                }
                self.wraps.insert(device_id.to_owned(), wrap.to_owned());
                no_content()
            }
            ("GET", "/v1/users/me/clipboard") => {
                let mut items = self
                    .items
                    .iter()
                    .filter(|item| Some(&item.key_id) == self.key_id.as_ref())
                    .cloned()
                    .collect::<Vec<_>>();
                items.sort_by_key(|item| std::cmp::Reverse(item.sequence));
                ok(json!({
                    "enabled": self.enabled,
                    "retention_days": 7,
                    "key_id": self.key_id,
                    "items": items.iter().map(Self::item_json).collect::<Vec<_>>(),
                }))
            }
            ("POST", "/v1/users/me/clipboard") => {
                if body.get("text").is_some() {
                    return error(400, "clipboard_e2e_required");
                }
                if !self.enabled {
                    return error(403, "clipboard_sync_disabled");
                }
                if body["key_id"].as_str() != self.key_id.as_deref() {
                    return error(409, "clipboard_key_conflict");
                }
                let ciphertext = body["ciphertext"].as_str().unwrap();
                let dedup = body["dedup"].as_str().unwrap();
                assert!(ciphertext.starts_with("v1.") && ciphertext.len() <= MAX_CIPHERTEXT_CHARS);
                assert!(is_lower_hex(dedup, 64));
                self.sequence += 1;
                let sequence = self.sequence;
                if let Some(item) = self.items.iter_mut().find(|item| item.dedup == dedup) {
                    item.sequence = sequence;
                    item.ciphertext = ciphertext.to_owned();
                    return ok(Self::item_json(item));
                }
                let item = FakeItem {
                    id: format!("{:064x}", sequence),
                    ciphertext: ciphertext.to_owned(),
                    key_id: body["key_id"].as_str().unwrap().to_owned(),
                    dedup: dedup.to_owned(),
                    sequence,
                };
                self.items.push(item.clone());
                ok(Self::item_json(&item))
            }
            _ => error(404, "not_found"),
        }
    }
}

/// 一台测试设备：持有自己的密钥串，经 [`run`] 对假服务端执行操作。
struct Device {
    name: &'static str,
    keystore: Option<String>,
}

impl Device {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            keystore: None,
        }
    }

    fn run(&mut self, server: &mut FakeServer, operation: Operation) -> Result<Value, DriverError> {
        let mut saved = self.keystore.clone();
        let outcome = run(
            operation,
            self.keystore.as_deref(),
            self.name,
            |request| Ok::<_, ()>(server.handle(request)),
            |keystore| {
                saved = Some(keystore.to_owned());
                Ok(())
            },
            || (),
        )
        .unwrap();
        self.keystore = saved;
        outcome
    }

    fn list(&mut self, server: &mut FakeServer, search: &str) -> Value {
        self.run(
            server,
            Operation::List {
                search: search.to_owned(),
            },
        )
        .unwrap()
    }

    fn add(&mut self, server: &mut FakeServer, text: &str) -> Result<Value, DriverError> {
        self.run(
            server,
            Operation::Add {
                text: text.to_owned(),
            },
        )
    }

    fn status(&mut self, server: &mut FakeServer) -> Value {
        self.run(server, Operation::Status).unwrap()
    }

    fn id(&self) -> String {
        Keystore::parse(self.keystore.as_deref().unwrap())
            .unwrap()
            .device_id()
    }

    fn code(&self) -> String {
        Keystore::parse(self.keystore.as_deref().unwrap())
            .unwrap()
            .verification_code()
    }
}

fn texts(list: &Value) -> Vec<&str> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["text"].as_str().unwrap())
        .collect()
}

/// 第一台设备开启后，第二台设备登记并由第一台批准。
fn paired(server: &mut FakeServer) -> (Device, Device) {
    let mut mac = Device::new("MacBook");
    let mut phone = Device::new("Pixel");
    mac.list(server, "");
    phone.list(server, "");
    let phone_id = phone.id();
    mac.run(
        server,
        Operation::Approve {
            device_id: phone_id,
        },
    )
    .unwrap();
    (mac, phone)
}

// ---- 密码学原语 ----

fn fixed_key() -> ClipboardKey {
    ClipboardKey::new([2u8; 32])
}

#[test]
fn known_answer_vectors_pin_the_wire_format() {
    // 这些值一旦发布就不能变：服务端和其他设备上已有的密文、封装都按它们解。
    let keystore = Keystore {
        secret: StaticSecret::from([1u8; 32]),
        key: None,
    };
    let public_key = keystore.public_key();
    assert_eq!(B64.encode(public_key), PUBLIC_KEY_1);
    assert_eq!(keystore.device_id(), DEVICE_ID_1);
    assert_eq!(keystore.verification_code(), CODE_1);
    let key = fixed_key();
    assert_eq!(key.id, KEY_ID_2);
    let sealed = seal_with_nonce(&key, "你好，水杉", [3u8; 24]);
    assert_eq!(sealed.ciphertext, CIPHERTEXT);
    assert_eq!(sealed.dedup, DEDUP);
    assert_eq!(sealed.key_id, KEY_ID_2);
    let wrap = wrap_with(&key, &public_key, [4u8; 32], [5u8; 24]).unwrap();
    assert_eq!(wrap, WRAP);
    assert_eq!(wrap.len(), 142);
    let opened = unwrap_key(&keystore.secret, KEY_ID_2, WRAP).unwrap();
    assert_eq!(opened.bytes, [2u8; 32]);
    assert_eq!(open_item(&key, CIPHERTEXT).as_deref(), Some("你好，水杉"));
}

// 由独立实现（libsodium 的 XChaCha20-Poly1305 与 X25519、pyca/cryptography 的 HKDF）按 docs/cloud-clipboard-e2e.md 算出，不是从这个模块的输出抄来的。
const PUBLIC_KEY_1: &str = "pOCSkrZRwni5dyxWn1-puxPZBrRqtoyd-dwrRAn4ogk";
const DEVICE_ID_1: &str = "1a92f23852dc908d97316a3b13578281";
const CODE_1: &str = "2394 0449 2387 6915";
const KEY_ID_2: &str = "adbf2e90f1a25f07c0ed00dab2fe0111";
const CIPHERTEXT: &str =
    "v1.AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDSs_d9FTm8DYPmHrNMo8EHveNrtc9i5-5P-oZPjKUgQ";
const DEDUP: &str = "09d559780465d91370e3ff6500307bf8e7d0b1a710ab458d56c0473c4a914967";
const WRAP: &str = "v1.rAGyIJ6GNU-4UyN7XeD0-rE8f8v0M6YcAZNpYX_s8QsFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQVhdd1FDAEiV7k07T6jPAEft_OuS5qsh3eCYrP63-5naP8MhdVxvn2vq71x-EnsUrU";

#[test]
fn keystore_round_trips_and_rejects_tampering() {
    let mut keystore = Keystore::generate().unwrap();
    let encoded = keystore.encode();
    assert_eq!(encoded.split('.').count(), 2);
    let parsed = Keystore::parse(&encoded).unwrap();
    assert_eq!(parsed.device_id(), keystore.device_id());
    assert!(parsed.key_id().is_none());

    keystore.key = Some(fixed_key());
    let encoded = keystore.encode();
    let parsed = Keystore::parse(&encoded).unwrap();
    assert_eq!(parsed.key_id(), Some(fixed_key().id.as_str()));

    let parts = encoded.split('.').collect::<Vec<_>>();
    let wrong_id = format!("v1.{}.{}.{}", parts[1], "0".repeat(32), parts[3]);
    assert!(Keystore::parse(&wrong_id).is_none());
    assert!(Keystore::parse(&format!("{encoded}.extra")).is_none());
    assert!(Keystore::parse(&format!("v1.{}.{}", parts[1], parts[2])).is_none());
    assert!(Keystore::parse("v2.AAAA").is_none());
    assert!(Keystore::parse("v1.AAAA").is_none());
    assert!(Keystore::parse("").is_none());
}

#[test]
fn items_open_only_with_their_key_and_untampered() {
    let key = fixed_key();
    let other = ClipboardKey::new([9u8; 32]);
    let sealed = seal_with_nonce(&key, "line one\nline two", [7u8; 24]);
    assert_eq!(
        open_item(&key, &sealed.ciphertext).as_deref(),
        Some("line one\nline two")
    );
    assert!(open_item(&other, &sealed.ciphertext).is_none());

    let mut bytes = B64.decode(&sealed.ciphertext[3..]).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(open_item(&key, &format!("v1.{}", B64.encode(&bytes))).is_none());
    assert!(open_item(&key, &sealed.ciphertext[3..]).is_none());
    assert!(open_item(&key, "v1.").is_none());
    assert!(open_item(&key, "v1.!!!!").is_none());

    // 同一把钥匙在不同编号下（AAD 不同）解不开。
    let relabelled = ClipboardKey {
        id: "f".repeat(32),
        bytes: key.bytes,
    };
    assert!(open_item(&relabelled, &sealed.ciphertext).is_none());
}

#[test]
fn dedup_tags_are_stable_per_key_and_text() {
    let key = fixed_key();
    let a = seal_with_nonce(&key, "same", [1u8; 24]);
    let b = seal_with_nonce(&key, "same", [2u8; 24]);
    assert_ne!(a.ciphertext, b.ciphertext);
    assert_eq!(a.dedup, b.dedup);
    assert_ne!(a.dedup, seal_with_nonce(&key, "other", [1u8; 24]).dedup);
    let other = ClipboardKey::new([9u8; 32]);
    assert_ne!(a.dedup, seal_with_nonce(&other, "same", [1u8; 24]).dedup);
}

#[test]
fn the_longest_text_fits_the_ciphertext_limit() {
    let key = fixed_key();
    // 4000 个 UTF-16 单元里字节最多的是 4000 个三字节的 BMP 字符。
    let text = "水".repeat(MAX_TEXT_UTF16_UNITS);
    assert!(is_valid_text(&text));
    let sealed = seal_with_nonce(&key, &text, [0u8; 24]);
    assert!(sealed.ciphertext.len() <= MAX_CIPHERTEXT_CHARS);
    assert_eq!(
        open_item(&key, &sealed.ciphertext).as_deref(),
        Some(text.as_str())
    );
    assert!(!is_valid_text(&"水".repeat(MAX_TEXT_UTF16_UNITS + 1)));
    assert!(!is_valid_text("  \n"));
    assert!(!is_valid_text("a\u{7}b"));
}

#[test]
fn wraps_open_only_for_their_device_and_key_id() {
    let key = fixed_key();
    let alice = Keystore::generate().unwrap();
    let bob = Keystore::generate().unwrap();
    let wrap = wrap_key(&key, &alice.public_key()).unwrap().unwrap();
    assert_eq!(wrap.len(), 142);
    assert_eq!(
        unwrap_key(&alice.secret, &key.id, &wrap).unwrap().bytes,
        key.bytes
    );
    assert!(unwrap_key(&bob.secret, &key.id, &wrap).is_none());
    assert!(unwrap_key(&alice.secret, &"0".repeat(32), &wrap).is_none());
    assert!(unwrap_key(&alice.secret, &key.id, &wrap[..141]).is_none());
    // 全零公钥（低阶点）得不出共享密钥，拒绝封装。
    assert!(wrap_with(&key, &[0u8; 32], [4u8; 32], [5u8; 24]).is_none());
}

#[test]
fn verification_codes_are_sixteen_grouped_digits() {
    let keystore = Keystore::generate().unwrap();
    let code = keystore.verification_code();
    assert_eq!(code.len(), 19);
    let groups = code.split(' ').collect::<Vec<_>>();
    assert_eq!(groups.len(), 4);
    assert!(groups
        .iter()
        .all(|group| group.len() == 4 && group.bytes().all(|b| b.is_ascii_digit())));
}

// ---- 驱动 ----

#[test]
fn first_device_creates_the_key_and_round_trips_items() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    let list = mac.list(&mut server, "");
    assert_eq!(list["enabled"], true);
    assert_eq!(list["e2e"]["status"], "ready");
    assert_eq!(list["items"], json!([]));
    assert_eq!(list["retention_days"], 7);
    assert_eq!(
        server.log,
        [
            "GET /v1/users/me/clipboard/keys".to_owned(),
            format!("PUT /v1/users/me/clipboard/devices/{}", mac.id()),
            "GET /v1/users/me/clipboard/keys".to_owned(),
            "POST /v1/users/me/clipboard/keys".to_owned(),
            "GET /v1/users/me/clipboard/keys".to_owned(),
            "GET /v1/users/me/clipboard".to_owned(),
        ]
    );
    assert_eq!(server.devices[0].name, "MacBook");

    let added = mac.add(&mut server, "你好").unwrap();
    assert_eq!(added["text"], "你好");
    assert_eq!(added["pinned"], false);
    assert!(!server.items[0].ciphertext.contains("你好"));
    mac.add(&mut server, "second").unwrap();
    mac.add(&mut server, "你好").unwrap();
    assert_eq!(server.items.len(), 2, "same text is deduplicated by tag");
    assert_eq!(texts(&mac.list(&mut server, "")), ["你好", "second"]);
    assert_eq!(texts(&mac.list(&mut server, "SEC")), ["second"]);

    server.log.clear();
    mac.list(&mut server, "");
    assert_eq!(
        server.log,
        [
            "GET /v1/users/me/clipboard/keys",
            "GET /v1/users/me/clipboard"
        ],
        "a ready device lists with two requests"
    );
}

#[test]
fn a_new_device_waits_for_approval_and_then_reads_everything() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    mac.add(&mut server, "from mac").unwrap();

    let mut phone = Device::new("Pixel");
    let list = phone.list(&mut server, "");
    assert_eq!(list["e2e"]["status"], "pending");
    assert_eq!(list["items"], json!([]));
    assert_eq!(list["e2e"]["code"], phone.code());
    assert_eq!(
        phone.add(&mut server, "nope").unwrap_err(),
        DriverError::local("clipboard_device_pending")
    );

    let mac_list = mac.list(&mut server, "");
    assert_eq!(mac_list["e2e"]["pending_devices"], 1);
    let status = mac.status(&mut server);
    assert_eq!(status["status"], "ready");
    let pending = status["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|device| device["device_id"] == phone.id())
        .unwrap();
    assert_eq!(pending["approved"], false);
    assert_eq!(pending["current"], false);
    assert_eq!(
        pending["code"],
        phone.code(),
        "both screens show the same code"
    );
    assert_eq!(pending["name"], "Pixel");

    let approved = mac
        .run(
            &mut server,
            Operation::Approve {
                device_id: phone.id(),
            },
        )
        .unwrap();
    assert!(approved["devices"]
        .as_array()
        .unwrap()
        .iter()
        .all(|device| device["approved"] == true));

    assert_eq!(texts(&phone.list(&mut server, "")), ["from mac"]);
    phone.add(&mut server, "from phone").unwrap();
    assert_eq!(
        texts(&mac.list(&mut server, "")),
        ["from phone", "from mac"]
    );
    assert_eq!(mac.list(&mut server, "")["e2e"]["pending_devices"], 0);
}

#[test]
fn removing_a_device_rotates_the_key_and_locks_it_out() {
    let mut server = FakeServer::new();
    let (mut mac, mut phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.list(&mut server, "");
    mac.run(
        &mut server,
        Operation::Approve {
            device_id: tablet.id(),
        },
    )
    .unwrap();
    mac.add(&mut server, "secret").unwrap();
    let old_key = server.key_id.clone();

    let status = mac
        .run(
            &mut server,
            Operation::Remove {
                device_id: phone.id(),
            },
        )
        .unwrap();
    assert_eq!(status["status"], "ready");
    assert_ne!(server.key_id, old_key);
    assert!(server.items.is_empty());
    assert!(!server.devices.iter().any(|d| d.device_id == phone.id()));
    // 新密钥只封装给本机，其他设备要重新批准。
    assert_eq!(server.wraps.len(), 1);
    assert_eq!(tablet.list(&mut server, "")["e2e"]["status"], "pending");

    // 被移除的设备再来会重新登记，成为待批准。
    let list = phone.list(&mut server, "");
    assert_eq!(list["e2e"]["status"], "pending");
    assert!(Keystore::parse(phone.keystore.as_deref().unwrap())
        .unwrap()
        .key_id()
        .is_none());
}

#[test]
fn reset_from_a_pending_device_takes_over() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    mac.add(&mut server, "old").unwrap();
    let mut phone = Device::new("Pixel");
    phone.list(&mut server, "");

    let status = phone.run(&mut server, Operation::Reset).unwrap();
    assert_eq!(status["status"], "ready");
    assert!(server.items.is_empty());
    assert_eq!(mac.list(&mut server, "")["e2e"]["status"], "pending");
    phone.add(&mut server, "new").unwrap();
    assert_eq!(texts(&phone.list(&mut server, "")), ["new"]);
}

#[test]
fn reject_deletes_a_pending_registration() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    let mut phone = Device::new("Pixel");
    phone.list(&mut server, "");
    let status = mac
        .run(
            &mut server,
            Operation::Reject {
                device_id: phone.id(),
            },
        )
        .unwrap();
    assert_eq!(status["devices"].as_array().unwrap().len(), 1);
    assert_eq!(
        mac.run(
            &mut server,
            Operation::Reject {
                device_id: phone.id()
            }
        )
        .unwrap_err(),
        DriverError {
            status: 404,
            code: "not_found".to_owned()
        }
    );
    let own = mac.id();
    assert_eq!(
        mac.run(&mut server, Operation::Reject { device_id: own })
            .unwrap_err(),
        DriverError::local("invalid_device_id")
    );
}

#[test]
fn concurrent_first_devices_settle_on_one_key() {
    let mut server = FakeServer::new();
    let a = start(
        Operation::List {
            search: String::new(),
        },
        None,
        "A",
    )
    .unwrap();
    let b = start(
        Operation::List {
            search: String::new(),
        },
        None,
        "B",
    )
    .unwrap();

    // 两台设备交替推进，直到都到达「建钥」那一步，再先后提交。
    fn advance_until_create(server: &mut FakeServer, mut step: Step) -> Step {
        loop {
            let Step::Request { request, state, .. } = step else {
                panic!("unexpected end")
            };
            if request.method == "POST" && request.path.ends_with("/keys") {
                return Step::Request {
                    keystore: state.keystore.clone(),
                    request,
                    state,
                };
            }
            step = resume(state, server.handle(&request)).unwrap();
        }
    }
    fn finish(server: &mut FakeServer, mut step: Step) -> Value {
        loop {
            match step {
                Step::Request { request, state, .. } => {
                    step = resume(state, server.handle(&request)).unwrap()
                }
                Step::Done { result, .. } => return result,
                Step::Failed { error, .. } => panic!("{error:?}"),
            }
        }
    }
    let a = advance_until_create(&mut server, a);
    let b = advance_until_create(&mut server, b);
    assert_eq!(finish(&mut server, a)["e2e"]["status"], "ready");
    assert_eq!(finish(&mut server, b)["e2e"]["status"], "pending");
    assert_eq!(server.wraps.len(), 1);
}

#[test]
fn a_key_change_during_upload_is_retried_with_the_new_key() {
    let mut server = FakeServer::new();
    let (mut mac, mut phone) = paired(&mut server);
    let step = start(
        Operation::Add {
            text: "racing".to_owned(),
        },
        mac.keystore.as_deref(),
        "MacBook",
    )
    .unwrap();
    let Step::Request { request, state, .. } = step else {
        panic!()
    };
    let mut step = resume(state, server.handle(&request)).unwrap();
    // 上传前另一台设备重置了密钥，又把本机重新批准。
    phone.run(&mut server, Operation::Reset).unwrap();
    let mac_id = mac.id();
    phone
        .run(&mut server, Operation::Approve { device_id: mac_id })
        .unwrap();
    let result = loop {
        match step {
            Step::Request { request, state, .. } => {
                step = resume(state, server.handle(&request)).unwrap()
            }
            Step::Done { result, keystore } => {
                mac.keystore = Some(keystore);
                break result;
            }
            Step::Failed { error, .. } => panic!("{error:?}"),
        }
    };
    assert_eq!(result["text"], "racing");
    assert_eq!(texts(&phone.list(&mut server, "")), ["racing"]);
}

#[test]
fn upload_conflicts_other_than_a_key_change_fail_without_retrying() {
    let mut server = FakeServer::new();
    let mut device = Device::new("合成设备");
    device.list(&mut server, "");
    let step = start(
        Operation::Add {
            text: "合成上传".to_owned(),
        },
        device.keystore.as_deref(),
        "合成设备",
    )
    .unwrap();
    let Step::Request { request, state, .. } = step else {
        panic!()
    };
    let Step::Request { request, state, .. } = resume(state, server.handle(&request)).unwrap()
    else {
        panic!()
    };
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, API);

    for (response, code) in [
        (error(409, "clipboard_full"), "clipboard_full"),
        (error(409, "synthetic_conflict"), "synthetic_conflict"),
        (
            HttpResponse {
                status: 409,
                body: "<html>synthetic conflict</html>".to_owned(),
            },
            "http_409",
        ),
    ] {
        let Step::Failed { error, keystore } = resume(state.clone(), response).unwrap() else {
            panic!("非换钥冲突必须立即失败，不能继续请求密钥或上传");
        };
        assert_eq!(error.status, 409);
        assert_eq!(error.code, code);
        assert!(Some(&keystore) == device.keystore.as_ref());
    }
}

#[test]
fn disabled_sync_is_reported_without_creating_anything() {
    let mut server = FakeServer::new();
    server.enabled = false;
    let mut mac = Device::new("MacBook");
    let list = mac.list(&mut server, "");
    assert_eq!(list["enabled"], false);
    assert_eq!(list["e2e"]["status"], "disabled");
    assert!(server.devices.is_empty());
    assert_eq!(
        mac.add(&mut server, "x").unwrap_err(),
        DriverError {
            status: 403,
            code: "clipboard_sync_disabled".to_owned()
        }
    );
    assert_eq!(mac.status(&mut server)["status"], "disabled");
}

#[test]
fn a_lying_server_cannot_get_a_wrap_for_a_forged_key() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    let mut phone = Device::new("Pixel");
    phone.list(&mut server, "");
    // 服务端把 phone 的公钥换成攻击者的：编号对不上，不显示也不封装。
    let attacker = Keystore::generate().unwrap();
    server
        .forged_public_keys
        .insert(phone.id(), B64.encode(attacker.public_key()));
    let status = mac.status(&mut server);
    assert!(!status["devices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|device| device["device_id"] == phone.id()));
    assert_eq!(
        mac.run(
            &mut server,
            Operation::Approve {
                device_id: phone.id()
            }
        )
        .unwrap_err(),
        DriverError::invalid_response()
    );
    assert!(!server.wraps.contains_key(&phone.id()));
}

#[test]
fn a_wrap_for_someone_else_leaves_the_device_pending() {
    let mut server = FakeServer::new();
    let (_mac, mut phone) = paired(&mut server);
    let other = Keystore::generate().unwrap();
    let key = ClipboardKey::new([8u8; 32]);
    server.key_id = Some(key.id.clone());
    server.forged_wrap = wrap_key(&key, &other.public_key()).unwrap();
    assert_eq!(phone.list(&mut server, "")["e2e"]["status"], "pending");
}

#[test]
fn items_from_another_key_are_counted_not_shown() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    mac.add(&mut server, "good").unwrap();
    let foreign = seal_with_nonce(&ClipboardKey::new([6u8; 32]), "bad", [1u8; 24]);
    server.items.push(FakeItem {
        id: "a".repeat(64),
        ciphertext: foreign.ciphertext,
        key_id: server.key_id.clone().unwrap(),
        dedup: foreign.dedup,
        sequence: 99,
    });
    let list = mac.list(&mut server, "");
    assert_eq!(texts(&list), ["good"]);
    assert_eq!(list["e2e"]["undecryptable"], 1);
}

#[test]
fn server_errors_surface_their_code() {
    let mut server = FakeServer::new();
    let mut mac = Device::new("MacBook");
    mac.list(&mut server, "");
    let step = start(Operation::Status, mac.keystore.as_deref(), "MacBook").unwrap();
    let Step::Request { state, .. } = step else {
        panic!()
    };
    let failed = resume(
        state.clone(),
        HttpResponse {
            status: 401,
            body: json!({"error": {"code": "unauthorized", "message": "x"}}).to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        failed,
        Step::Failed {
            error: DriverError {
                status: 401,
                code: "unauthorized".to_owned()
            },
            keystore: mac.keystore.clone().unwrap(),
        }
    );
    let failed = resume(
        state.clone(),
        HttpResponse {
            status: 502,
            body: "<html>".to_owned(),
        },
    )
    .unwrap();
    assert!(matches!(failed, Step::Failed { error, .. } if error.code == "http_502"));
    let failed = resume(
        state,
        HttpResponse {
            status: 200,
            body: "{}".to_owned(),
        },
    )
    .unwrap();
    assert!(
        matches!(failed, Step::Failed { error, .. } if error == DriverError::invalid_response())
    );
}

#[test]
fn invalid_operations_fail_before_any_request() {
    for (operation, code) in [
        (
            Operation::Add {
                text: " ".to_owned(),
            },
            "invalid_clipboard_text",
        ),
        (
            Operation::Add {
                text: "x\u{0}".to_owned(),
            },
            "invalid_clipboard_text",
        ),
        (
            Operation::List {
                search: "x".repeat(1025),
            },
            "invalid_search",
        ),
        (
            Operation::Approve {
                device_id: "XYZ".to_owned(),
            },
            "invalid_device_id",
        ),
    ] {
        let step = start(operation, None, "MacBook").unwrap();
        assert!(
            matches!(step, Step::Failed { error, keystore } if error.code == code && keystore.starts_with("v1."))
        );
    }
}

#[test]
fn device_names_are_sanitised() {
    assert_eq!(sanitize_device_name("  Mac\u{7}Book  "), "MacBook");
    assert_eq!(sanitize_device_name(&"长".repeat(100)).chars().count(), 64);
}

#[test]
fn the_json_entry_point_carries_state_between_steps() {
    let mut server = FakeServer::new();
    let mut output = drive_json(
        json!({"start": {"op": "add", "text": "via json"}, "keystore": null, "device_name": "Host"})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let (result, keystore) = loop {
        if let Some(done) = output.get("done") {
            break (
                done.clone(),
                output["keystore"].as_str().unwrap().to_owned(),
            );
        }
        assert!(output.get("error").is_none(), "{output}");
        let request: HttpRequest = serde_json::from_value(output["request"].clone()).unwrap();
        let response = server.handle(&request);
        output = drive_json(
            json!({"resume": output["state"], "response": response})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    };
    assert_eq!(result["text"], "via json");
    assert!(Keystore::parse(&keystore).unwrap().key_id().is_some());
    assert!(drive_json(b"{}").is_err());
    assert!(drive_json(b"not json").is_err());
    assert!(drive_json(br#"{"start":{"op":"bogus"}}"#).is_err());
}
