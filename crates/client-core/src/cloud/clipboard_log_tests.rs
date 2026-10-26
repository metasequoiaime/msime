use super::*;

const NOW: i64 = 1_790_000_000;

/// 一个只在内存里的日志：每个请求按 [`Ledger::plan`] 处理，要写的事件立刻接上，就像这台设备每次都抢到了下一个序号。
struct Log {
    ledger: Ledger,
    counter: u64,
    now: i64,
    written: Vec<Vec<u8>>,
}

impl Log {
    fn new() -> Self {
        Self {
            ledger: Ledger::genesis(),
            counter: 0,
            now: NOW,
            written: Vec::new(),
        }
    }

    fn draft(&mut self) -> Draft {
        self.counter += 1;
        Draft {
            now: self.now,
            nonce: format!("{:032x}", self.counter),
            item_id: format!("{:064x}", self.counter),
        }
    }

    fn seal(&mut self) -> Vec<u8> {
        let draft = self.draft();
        self.ledger.seal(&draft)
    }

    fn call(&mut self, method: &str, path: &str, body: Value) -> Response {
        let body = if body.is_null() {
            Vec::new()
        } else {
            body.to_string().into_bytes()
        };
        self.raw(method, path, &body)
    }

    fn raw(&mut self, method: &str, path: &str, body: &[u8]) -> Response {
        let draft = self.draft();
        match self.ledger.plan(method, path, body, &draft) {
            Plan::Reply(response) => response,
            Plan::Append { bytes, response } => {
                self.ledger.apply(&bytes).unwrap();
                self.written.push(bytes);
                response
            }
        }
    }
}

fn code(response: &Response) -> String {
    serde_json::from_str::<Value>(response.body.as_deref().unwrap()).unwrap()["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn body(response: &Response) -> Value {
    serde_json::from_str(response.body.as_deref().unwrap()).unwrap()
}

fn device(seed: u8) -> (String, String) {
    let public_key = [seed; 32];
    (
        hex::encode(&Sha256::digest(public_key)[..16]),
        B64.encode(public_key),
    )
}

fn wrap() -> String {
    format!("v1.{}", B64.encode([7u8; WRAP_BYTES]))
}

fn ciphertext() -> String {
    format!("v1.{}", B64.encode([1u8; 60]))
}

/// 登记一台设备、建好密钥，返回密钥编号。
fn ready(log: &mut Log) -> String {
    let (id, key) = device(1);
    log.call(
        "PUT",
        &format!("{API}/devices/{id}"),
        json!({ "public_key": key, "name": "" }),
    );
    let key_id = "b".repeat(32);
    let rotated = log.call(
        "POST",
        &format!("{API}/keys"),
        json!({ "previous_key_id": null, "key_id": key_id, "wraps": [{ "device_id": id, "wrap": wrap() }] }),
    );
    assert_eq!(rotated.status, 204);
    key_id
}

// ---- 与参考中继 `store` 的单元测试一一对应 ----

#[test]
fn a_new_space_starts_enabled_without_a_key() {
    let mut log = Log::new();
    let (id, _) = device(1);
    let keys = log.call("GET", &format!("{API}/keys?device_id={id}"), Value::Null);
    assert_eq!(keys.status, 200);
    assert_eq!(
        body(&keys),
        json!({ "enabled": true, "key_id": null, "wrap": null, "devices": [] })
    );
    assert_eq!(
        code(&log.call("GET", &format!("{API}/keys?device_id=nope"), Value::Null)),
        "invalid_device_id"
    );
    // 读取不写日志。
    assert!(log.written.is_empty());
}

#[test]
fn devices_are_checked_against_their_public_key_and_capped() {
    let mut log = Log::new();
    let (id, key) = device(1);
    let (_, other_key) = device(2);
    let put = |log: &mut Log, id: &str, key: &str, name: &str| {
        log.call(
            "PUT",
            &format!("{API}/devices/{id}"),
            json!({ "public_key": key, "name": name }),
        )
    };
    assert_eq!(
        code(&put(&mut log, &id, &other_key, "合成")),
        "invalid_clipboard_device"
    );
    assert_eq!(
        code(&put(&mut log, &id, &key, "bad\u{7}")),
        "invalid_clipboard_device"
    );
    assert_eq!(
        code(&put(&mut log, &id, &format!("{key}="), "合成")),
        "invalid_clipboard_device"
    );
    assert_eq!(put(&mut log, &id, &key, "合成").status, 204);
    // 20 台都没批准时，第 21 台挤掉最早的一台。
    for seed in 2..=21 {
        let (id, key) = device(seed);
        assert_eq!(put(&mut log, &id, &key, "合成").status, 204);
    }
    let keys = body(&log.call("GET", &format!("{API}/keys?device_id={id}"), Value::Null));
    let devices = keys["devices"].as_array().unwrap();
    assert_eq!(devices.len(), MAX_DEVICES);
    assert!(devices
        .iter()
        .all(|device| device["device_id"] != id.as_str()));
    // 全部批准之后再来一台被拒绝。
    let wraps: Vec<Value> = devices
        .iter()
        .map(|device| json!({ "device_id": device["device_id"], "wrap": wrap() }))
        .collect();
    let rotated = log.call(
        "POST",
        &format!("{API}/keys"),
        json!({ "previous_key_id": null, "key_id": "a".repeat(32), "wraps": wraps, "revoke": [] }),
    );
    assert_eq!(rotated.status, 204);
    let (late, late_key) = device(30);
    assert_eq!(
        code(&put(&mut log, &late, &late_key, "合成")),
        "clipboard_device_limit"
    );
}

#[test]
fn uploads_need_the_current_key_and_are_deduplicated() {
    let mut log = Log::new();
    let (id, key) = device(1);
    log.call(
        "PUT",
        &format!("{API}/devices/{id}"),
        json!({ "public_key": key, "name": "" }),
    );
    let key_id = "b".repeat(32);
    let upload = |log: &mut Log, key_id: &str, dedup: &str| {
        log.call(
            "POST",
            API,
            json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": dedup }),
        )
    };
    assert_eq!(
        code(&upload(&mut log, &key_id, &"d".repeat(64))),
        "clipboard_key_conflict"
    );
    log.call(
        "POST",
        &format!("{API}/keys"),
        json!({ "previous_key_id": null, "key_id": key_id, "wraps": [{ "device_id": id, "wrap": wrap() }] }),
    );
    let first = upload(&mut log, &key_id, &"d".repeat(64));
    assert_eq!(first.status, 200);
    let second = upload(&mut log, &key_id, &"d".repeat(64));
    let first = body(&first);
    let second = body(&second);
    assert_eq!(first["id"], second["id"]);
    assert!(is_lower_hex(first["id"].as_str().unwrap(), 64));
    assert_eq!(
        code(&log.call("POST", API, json!({ "text": "明文" }))),
        "clipboard_e2e_required"
    );
    assert_eq!(
        code(&log.call(
            "POST",
            API,
            json!({ "ciphertext": "v1.short", "key_id": key_id, "dedup": "d".repeat(64) })
        )),
        "invalid_clipboard_ciphertext"
    );
    // 第五十一条挤掉最旧的一条。
    for index in 0..MAX_ITEMS {
        assert_eq!(
            upload(&mut log, &key_id, &format!("{index:064x}")).status,
            200
        );
    }
    let page = body(&log.call("GET", API, Value::Null));
    assert_eq!(page["items"].as_array().unwrap().len(), MAX_ITEMS);
    assert!(page["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["id"] != first["id"]));
}

#[test]
fn a_space_whose_items_are_all_pinned_refuses_new_uploads() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    let upload = |log: &mut Log, dedup: &str| {
        log.call(
            "POST",
            API,
            json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": dedup }),
        )
    };
    let mut ids = Vec::new();
    for index in 0..MAX_ITEMS {
        let item = upload(&mut log, &format!("{index:064x}"));
        assert_eq!(item.status, 200);
        ids.push(body(&item)["id"].as_str().unwrap().to_owned());
    }
    for id in &ids {
        let pinned = log.call("PUT", &format!("{API}/{id}/pin"), json!({ "pinned": true }));
        assert_eq!(pinned.status, 204);
    }
    // 新的一条没有位置：答 409，置顶的条目一条不少。
    let refused = upload(&mut log, &"e".repeat(64));
    assert_eq!(refused.status, 409);
    assert_eq!(code(&refused), "clipboard_full");
    let page = body(&log.call("GET", API, Value::Null));
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), MAX_ITEMS);
    assert!(items.iter().all(|item| item["pinned"] == true));
    // 重复上传一条已置顶的内容只是更新它。
    assert_eq!(upload(&mut log, &format!("{:064x}", 0)).status, 200);
    // 取消一条置顶后又有了位置，新的一条挤掉的是这条未置顶的。
    log.call(
        "PUT",
        &format!("{API}/{}/pin", ids[1]),
        json!({ "pinned": false }),
    );
    assert_eq!(upload(&mut log, &"e".repeat(64)).status, 200);
    let page = body(&log.call("GET", API, Value::Null));
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), MAX_ITEMS);
    assert!(items.iter().all(|item| item["id"] != ids[1].as_str()));
}

#[test]
fn unknown_routes_and_bodies_are_refused() {
    let mut log = Log::new();
    assert_eq!(
        code(&log.call("GET", "/v1/users/me/profile", Value::Null)),
        "not_found"
    );
    assert_eq!(
        code(&log.call("GET", &format!("{API}x"), Value::Null)),
        "not_found"
    );
    assert_eq!(code(&log.call("PATCH", API, Value::Null)), "not_found");
    assert_eq!(
        code(&log.call("PUT", &format!("{API}/retention"), json!({ "days": 3 }))),
        "invalid_retention"
    );
    assert_eq!(
        code(&log.call("PUT", &format!("{API}/settings"), json!({}))),
        "enabled_required"
    );
    assert_eq!(
        code(&log.raw("PUT", &format!("{API}/settings"), b"not json")),
        "invalid_json"
    );
    assert_eq!(
        code(&log.call("DELETE", &format!("{API}/{}", "c".repeat(64)), Value::Null)),
        "not_found"
    );
    assert!(log.written.is_empty());
}

// ---- 日志本身 ----

#[test]
fn replaying_the_written_events_rebuilds_the_same_ledger() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    for index in 0..5 {
        log.call(
            "POST",
            API,
            json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": format!("{index:064x}") }),
        );
    }
    let id = body(&log.call("GET", API, Value::Null))["items"][2]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    log.call("PUT", &format!("{API}/{id}/pin"), json!({ "pinned": true }));
    log.call("PUT", &format!("{API}/retention"), json!({ "days": 7 }));
    let (other, other_key) = device(9);
    log.call(
        "PUT",
        &format!("{API}/devices/{other}"),
        json!({ "public_key": other_key, "name": "合成" }),
    );
    log.call(
        "PUT",
        &format!("{API}/keys/{key_id}/wraps/{other}"),
        json!({ "wrap": wrap() }),
    );

    let mut replayed = Ledger::genesis();
    for bytes in &log.written {
        replayed.apply(bytes).unwrap();
    }
    assert_eq!(replayed, log.ledger);
    assert_eq!(replayed.seq, log.written.len() as u64);
    assert_eq!(
        body(&log.call("GET", &format!("{API}/keys?device_id={other}"), Value::Null))["wrap"],
        wrap()
    );
}

#[test]
fn of_two_key_creations_from_the_same_position_only_the_first_is_kept() {
    let mut log = Log::new();
    let (first, first_key) = device(1);
    let (second, second_key) = device(2);
    for (id, key) in [(&first, &first_key), (&second, &second_key)] {
        log.call(
            "PUT",
            &format!("{API}/devices/{id}"),
            json!({ "public_key": key, "name": "" }),
        );
    }
    let create = |id: &str, key_id: &str| {
        json!({ "previous_key_id": null, "key_id": key_id, "wraps": [{ "device_id": id, "wrap": wrap() }] })
            .to_string()
    };
    let draft = |seed: u64| Draft {
        now: NOW,
        nonce: format!("{seed:032x}"),
        item_id: format!("{seed:064x}"),
    };
    let path = format!("{API}/keys");
    let Plan::Append { bytes: winner, .. } = log.ledger.plan(
        "POST",
        &path,
        create(&first, &"a".repeat(32)).as_bytes(),
        &draft(100),
    ) else {
        panic!("key creation should append");
    };
    let Plan::Append { bytes: loser, .. } = log.ledger.plan(
        "POST",
        &path,
        create(&second, &"c".repeat(32)).as_bytes(),
        &draft(101),
    ) else {
        panic!("key creation should append");
    };
    // 两台设备抢同一个序号：网盘只让一个写成；另一个即使被人塞进日志，也接不上链。
    log.ledger.apply(&winner).unwrap();
    let before = log.ledger.clone();
    assert_eq!(log.ledger.apply(&loser), Err(LogError::Chain));
    assert_eq!(log.ledger, before);
    // 输的一方重读以后再处理同一个请求：比较 `previous_key_id` 失败，驱动重取状态。
    let Plan::Reply(conflict) = log.ledger.plan(
        "POST",
        &path,
        create(&second, &"c".repeat(32)).as_bytes(),
        &draft(102),
    ) else {
        panic!("a stale key creation must not append");
    };
    assert_eq!(code(&conflict), "clipboard_key_conflict");
    assert_eq!(
        log.ledger.state.key_id.as_deref(),
        Some("a".repeat(32).as_str())
    );
}

#[test]
fn malformed_or_refused_events_keep_their_number_without_changing_state() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    let state = log.ledger.state.clone();
    let event = |ledger: &Ledger, change: Value| {
        let mut value =
            json!({ "seq": ledger.seq + 1, "prev": ledger.head, "nonce": "00", "at": NOW });
        value
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        value.to_string().into_bytes()
    };
    let refused = [
        // 密文不合规。
        event(
            &log.ledger,
            json!({ "op": "add", "id": "f".repeat(64), "ciphertext": "v1.x", "key_id": key_id, "dedup": "d".repeat(64) }),
        ),
        // 不是当前密钥。
        event(
            &log.ledger,
            json!({ "op": "add", "id": "f".repeat(64), "ciphertext": ciphertext(), "key_id": "9".repeat(32), "dedup": "d".repeat(64) }),
        ),
        // 条目编号不合规。
        event(
            &log.ledger,
            json!({ "op": "add", "id": "../x", "ciphertext": ciphertext(), "key_id": key_id, "dedup": "d".repeat(64) }),
        ),
        // 不认识的操作。
        event(&log.ledger, json!({ "op": "teleport" })),
        // 序号与文件名不符。
        json!({ "seq": 999, "prev": log.ledger.head, "nonce": "00", "at": NOW, "op": "clear" })
            .to_string()
            .into_bytes(),
    ];
    for bytes in refused {
        let mut bytes = bytes;
        // 每个都要接在上一个后面。
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["prev"] = log.ledger.head.clone().into();
        if value["seq"] != 999 {
            value["seq"] = (log.ledger.seq + 1).into();
        }
        bytes = value.to_string().into_bytes();
        let seq = log.ledger.seq;
        log.ledger.apply(&bytes).unwrap();
        assert_eq!(log.ledger.seq, seq + 1);
        assert_eq!(log.ledger.head, hex::encode(Sha256::digest(&bytes)));
        assert_eq!(log.ledger.state, state);
    }
    // 不是 JSON 的文件也照样占一个序号；下一个事件接在它的哈希后面。
    let garbage = b"\x00not json".to_vec();
    log.ledger.apply(&garbage).unwrap();
    assert_eq!(log.ledger.head, hex::encode(Sha256::digest(&garbage)));
    assert_eq!(log.ledger.state, state);
    let upload = log.call(
        "POST",
        API,
        json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": "d".repeat(64) }),
    );
    assert_eq!(upload.status, 200);
}

#[test]
fn an_event_that_does_not_follow_the_remembered_head_is_refused() {
    let mut log = Log::new();
    ready(&mut log);
    let before = log.ledger.clone();
    // 有人删掉了本机已经读过的一段，换上另一段历史：下一个事件接不上本机的链头。
    let forged = json!({ "seq": before.seq + 1, "prev": GENESIS_HEAD, "nonce": "00", "at": NOW, "op": "clear" })
        .to_string()
        .into_bytes();
    assert_eq!(log.ledger.apply(&forged), Err(LogError::Chain));
    assert_eq!(log.ledger, before);
}

#[test]
fn sealing_starts_an_epoch_whose_snapshot_any_device_writes_identically() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    log.call(
        "POST",
        API,
        json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": "d".repeat(64) }),
    );
    let draft = log.draft();
    let seal = log.ledger.seal(&draft);
    log.ledger.apply(&seal).unwrap();
    log.written.push(seal);
    assert!(log.ledger.sealed);
    // 封口之后这个 epoch 不再接事件。
    let late = log.seal();
    assert_eq!(log.ledger.apply(&late), Err(LogError::Sealed));
    let next = log.ledger.next_epoch().unwrap();
    assert_eq!((next.epoch, next.seq, next.events), (1, log.ledger.seq, 0));

    // 另一台设备从头重放，写出的快照逐字节相同。
    let mut other = Ledger::genesis();
    for bytes in &log.written {
        other.apply(bytes).unwrap();
    }
    let snapshot = next.snapshot();
    assert_eq!(other.next_epoch().unwrap().snapshot(), snapshot);
    // 新设备只读快照，得到同样的位置和状态。
    let fresh = Ledger::from_snapshot(1, &snapshot).unwrap();
    assert_eq!(fresh, next);
    assert!(Ledger::from_snapshot(2, &snapshot).is_err());
}

#[test]
fn snapshots_from_the_drive_are_checked_field_by_field() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    log.call(
        "POST",
        API,
        json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": "d".repeat(64) }),
    );
    let snapshot: Value = serde_json::from_slice(&log.ledger.snapshot()).unwrap();
    assert!(Ledger::from_snapshot(0, snapshot.to_string().as_bytes()).is_ok());
    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = snapshot.clone();
        edit(&mut value);
        Ledger::from_snapshot(0, value.to_string().as_bytes())
    };
    for edit in [
        &(|value: &mut Value| value["version"] = 2.into()) as &dyn Fn(&mut Value),
        &|value: &mut Value| value["format"] = "other".into(),
        &|value: &mut Value| value["head"] = "xyz".into(),
        &|value: &mut Value| value["state"]["items"][0]["id"] = "../x".into(),
        &|value: &mut Value| value["state"]["items"][0]["ciphertext"] = "v1.x".into(),
        &|value: &mut Value| value["state"]["devices"][0]["public_key"] = "AAAA".into(),
        &|value: &mut Value| value["state"]["retention_days"] = 3.into(),
        &|value: &mut Value| value["state"]["extra"] = true.into(),
    ] {
        assert_eq!(tamper(edit), Err(LogError::Malformed));
    }
    assert_eq!(
        Ledger::from_snapshot(0, b"not json"),
        Err(LogError::Malformed)
    );
}

#[test]
fn expired_items_are_hidden_when_read_and_pruned_by_the_next_seal() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    log.call("PUT", &format!("{API}/retention"), json!({ "days": 1 }));
    for index in 0..2 {
        log.call(
            "POST",
            API,
            json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": format!("{index:064x}") }),
        );
    }
    let pinned = body(&log.call("GET", API, Value::Null))["items"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    log.call(
        "PUT",
        &format!("{API}/{pinned}/pin"),
        json!({ "pinned": true }),
    );
    assert!(!log.ledger.state.has_expired_items(NOW));

    // 两天后：未置顶的那条不再出现在列表里，日志要求压缩；封口时按封口的时间删掉它。
    log.now = NOW + 2 * 86_400;
    let page = body(&log.call("GET", API, Value::Null));
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["id"], pinned.as_str());
    assert_eq!(log.ledger.state.items.len(), 2);
    assert!(log.ledger.compaction_due(log.now));
    let seal = log.seal();
    log.ledger.apply(&seal).unwrap();
    let next = log.ledger.next_epoch().unwrap();
    assert_eq!(next.state.items.len(), 1);
    assert!(!next.compaction_due(log.now));
}

#[test]
fn writes_that_change_nothing_append_nothing() {
    let mut log = Log::new();
    ready(&mut log);
    let written = log.written.len();
    assert_eq!(log.call("DELETE", API, Value::Null).status, 204);
    assert_eq!(
        log.call("PUT", &format!("{API}/retention"), json!({ "days": 0 }))
            .status,
        204
    );
    assert_eq!(
        body(&log.call(
            "PUT",
            &format!("{API}/settings"),
            json!({ "enabled": true })
        )),
        json!({ "enabled": true })
    );
    let (id, key) = device(1);
    assert_eq!(
        log.call(
            "PUT",
            &format!("{API}/devices/{id}"),
            json!({ "public_key": key, "name": "" })
        )
        .status,
        204
    );
    assert_eq!(log.written.len(), written);
}

#[test]
fn compaction_follows_event_count_and_destructive_writes() {
    let mut log = Log::new();
    let key_id = ready(&mut log);
    // 换钥之后马上压缩，旧 epoch 里的封装就不必留着。
    assert!(log.ledger.compaction_due(NOW));
    let seal = log.seal();
    log.ledger.apply(&seal).unwrap();
    log.ledger = log.ledger.next_epoch().unwrap();
    assert!(!log.ledger.compaction_due(NOW));
    for index in 0..=COMPACT_AFTER_EVENTS {
        log.call(
            "POST",
            API,
            json!({ "ciphertext": ciphertext(), "key_id": key_id, "dedup": format!("{:064x}", index % 3) }),
        );
    }
    assert!(log.ledger.compaction_due(NOW));
    let seal = log.seal();
    log.ledger.apply(&seal).unwrap();
    log.ledger = log.ledger.next_epoch().unwrap();
    assert_eq!(log.ledger.state.items.len(), 3);
    log.call("DELETE", API, Value::Null);
    assert!(log.ledger.compaction_due(NOW));
}

#[test]
fn turning_the_clipboard_off_or_rotating_advances_the_key_generation() {
    let mut log = Log::new();
    ready(&mut log);
    assert_eq!(log.ledger.state.generation, 1);
    let disabled = log.call(
        "PUT",
        &format!("{API}/settings"),
        json!({ "enabled": false }),
    );
    assert_eq!(body(&disabled), json!({ "enabled": false }));
    assert_eq!(log.ledger.state.generation, 2);
    assert!(log.ledger.state.devices.is_empty());
    assert_eq!(log.ledger.state.key_id, None);
    let (id, _) = device(1);
    let keys = body(&log.call("GET", &format!("{API}/keys?device_id={id}"), Value::Null));
    assert_eq!(keys["enabled"], false);
}

#[test]
fn the_reference_validation_matches_the_relay_rules() {
    let (id, key) = device(3);
    assert!(valid_device(&id, &key));
    assert!(!valid_device(&id.to_uppercase(), &key));
    assert!(!valid_device(&id, &format!("{key}=")));
    assert!(valid_device_name(&"名".repeat(64)));
    assert!(!valid_device_name(&"名".repeat(65)));
    assert!(valid_wrap(&wrap()));
    assert!(!valid_wrap(&format!(
        "v1.{}",
        B64.encode([7u8; WRAP_BYTES - 1])
    )));
    assert!(valid_ciphertext(&ciphertext()));
    assert!(!valid_ciphertext(&format!("v1.{}", B64.encode([1u8; 40]))));
    assert!(!valid_ciphertext(&format!(
        "v1.{}",
        B64.encode(vec![1u8; MAX_CIPHERTEXT_CHARS])
    )));
    assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    let upload = parse_request("POST", API, &vec![b' '; MAX_UPLOAD_BYTES + 1]).unwrap_err();
    assert_eq!(upload.status, 413);
}
