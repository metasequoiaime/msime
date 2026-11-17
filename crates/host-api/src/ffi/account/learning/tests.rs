//! 经真实 C 入口验证登录隔离、独立密钥存储和线上管理请求，测试数据均为合成值。

use super::*;
use crate::ffi::session_store::tests::{leaked_store, registered, session_bytes, unregister};
use msime_client_services::account::{AccountSessionStorage, SavedAccountSession};
use std::ffi::CStr;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

mod rounds;

fn call(source: &Value, action: Value) -> Value {
    let mut request = source.clone();
    request["action"] = action;
    let bytes = request.to_string();
    // SAFETY: 请求缓冲区在调用期间有效，返回的字符串读完释放。
    unsafe {
        let raw = msime_client_account(bytes.as_ptr(), bytes.len());
        let value = serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap();
        msime_client_string_free(raw);
        value
    }
}

fn binding(source: &Value) -> Value {
    let response = call(source, json!({"operation": "learning_binding"}));
    assert_eq!(response["ok"], true, "{response}");
    response["value"]["login"].clone()
}

fn action(login: Value, request: Value) -> Value {
    json!({"operation": "learning_operation", "login": login,
        "device_name": "合成测试设备", "request": request})
}

fn fixture() -> (tempfile::TempDir, Value, FileAccountSessionStorage) {
    let root = tempfile::tempdir().unwrap();
    let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
    let saved: SavedAccountSession =
        serde_json::from_slice(&session_bytes("synthetic-user", 'a', true)).unwrap();
    storage.save(&saved).unwrap();
    let source = json!({"directory": root.path()});
    (root, source, storage)
}

#[test]
fn binding_is_local_and_does_not_create_learning_keys_or_expose_credentials() {
    let (_root, source, storage) = fixture();
    let login = binding(&source);
    assert_eq!(login["user_id"], "synthetic-user");
    assert_eq!(login.as_object().unwrap().len(), 2);
    assert!(login["session_id"].is_string());
    let mut expired = storage.load().unwrap().unwrap();
    expired.expires_at_unix_ms = 0;
    storage.save(&expired).unwrap();
    // 绑定查询在令牌过期时仍只读本机，不走刷新或生成新身份。
    assert_eq!(binding(&source), login);
    assert!(storage.load_learning_keystore().unwrap().is_none());
    assert!(storage.load_clipboard_keystore().unwrap().is_none());
}

#[test]
fn no_login_returns_null_without_falling_back_to_anonymous_identity() {
    let root = tempfile::tempdir().unwrap();
    let source = json!({"directory": root.path()});
    assert_eq!(binding(&source), Value::Null);
    assert!(!root.path().join("learning-keystore").exists());
}

#[test]
fn malformed_management_requests_are_rejected_before_creating_keys() {
    let (_root, source, storage) = fixture();
    let login = binding(&source);
    let valid = action(login, json!({"op": "status"}));
    let mut cases = Vec::new();
    for field in ["login", "device_name", "request"] {
        let mut invalid = valid.clone();
        invalid.as_object_mut().unwrap().remove(field);
        cases.push(invalid);
    }
    let mut invalid = valid.clone();
    invalid["extra"] = json!(true);
    cases.push(invalid);
    let mut invalid = valid.clone();
    invalid["login"]["extra"] = json!(true);
    cases.push(invalid);
    for request in [
        json!({"op": "status", "extra": true}),
        json!({"op": "confirm", "key_id": "synthetic"}),
        json!({"op": "approve", "device_id": 1}),
        json!({"op": "unknown"}),
    ] {
        let mut invalid = valid.clone();
        invalid["request"] = request;
        cases.push(invalid);
    }
    cases.push(json!({"operation": "learning_binding", "extra": true}));
    for invalid in cases {
        assert_eq!(
            call(&source, invalid),
            json!({"ok": false, "error": "account_invalid"})
        );
    }
    assert!(storage.load_learning_keystore().unwrap().is_none());
}

#[test]
fn all_management_operations_decode_without_copying_driver_rules() {
    let login =
        json!({"user_id": "synthetic-user", "session_id": "00000000-0000-4000-8000-000000000001"});
    for request in [
        json!({"op": "status"}),
        json!({"op": "reset"}),
        json!({"op": "disable"}),
        json!({"op": "approve", "device_id": "synthetic"}),
        json!({"op": "reject", "device_id": "synthetic"}),
        json!({"op": "remove", "device_id": "synthetic"}),
        json!({"op": "confirm", "key_id": "synthetic", "approver_device_id": "synthetic"}),
        json!({"op": "deny", "key_id": "synthetic", "approver_device_id": "synthetic"}),
    ] {
        assert!(serde_json::from_value::<Request>(action(login.clone(), request)).is_ok());
    }
}

#[test]
fn same_user_relogin_rejects_old_ui_operation_before_writing_a_key() {
    let (_root, source, storage) = fixture();
    let old = binding(&source);
    let mut saved = storage.load().unwrap().unwrap();
    saved.session_id = Some(msime_client_core::uuid::Uuid::new_v4());
    storage.save(&saved).unwrap();
    assert_ne!(binding(&source), old);
    assert_eq!(
        call(&source, action(old, json!({"op": "status"}))),
        json!({"ok": false, "error": "account_cancelled"})
    );
    assert!(storage.load_learning_keystore().unwrap().is_none());
}

#[test]
fn directory_management_persists_only_the_independent_learning_slot() {
    let (_root, source, storage) = fixture();
    storage
        .save_clipboard_keystore("synthetic-clipboard-sentinel")
        .unwrap();
    // 非法目标由共享驱动拒绝，不联网；新身份仍按共享驱动规则先持久化。
    let result = call(
        &source,
        action(
            binding(&source),
            json!({"op": "approve", "device_id": "invalid"}),
        ),
    );
    assert_eq!(result, json!({"ok": false, "error": "invalid_device_id"}));
    let key = storage.load_learning_keystore().unwrap().unwrap();
    let saved: Value = serde_json::from_str(&key).unwrap();
    assert_eq!(saved["login"], binding(&source));
    assert!(saved["keystore"].as_str().unwrap().starts_with("v1."));
    assert_eq!(
        storage.load_clipboard_keystore().unwrap().as_deref(),
        Some("synthetic-clipboard-sentinel")
    );
    assert!(!result.to_string().contains("keystore"));
}

#[test]
fn injected_management_respects_learning_capability_and_preserves_other_slots() {
    for flags in [0, 2] {
        let store = leaked_store();
        store
            .items
            .lock()
            .unwrap()
            .insert(1, session_bytes("synthetic-user", 'a', true));
        store
            .items
            .lock()
            .unwrap()
            .insert(4, b"synthetic-clipboard-sentinel".to_vec());
        let id = registered(store, flags, 0);
        let source = json!({"session_store": id});
        let result = call(
            &source,
            action(
                binding(&source),
                json!({"op": "approve", "device_id": "invalid"}),
            ),
        );
        assert_eq!(
            result["error"],
            if flags == 2 {
                "invalid_device_id"
            } else {
                "account_storage"
            }
        );
        let items = store.items.lock().unwrap();
        assert_eq!(items.contains_key(&5), flags == 2);
        assert_eq!(items.get(&4).unwrap(), b"synthetic-clipboard-sentinel");
        drop(items);
        assert_eq!(unregister(id)["ok"], true);
    }
}

#[test]
fn explicit_disable_uses_authenticated_learning_endpoint_and_returns_no_key() {
    let (_root, source, storage) = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("synthetic server accept failed: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let mut buffer = [0; 1024];
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    struct Origin;
    impl Drop for Origin {
        fn drop(&mut self) {
            TEST_ORIGIN.with_borrow_mut(|slot| *slot = None);
        }
    }
    TEST_ORIGIN.with_borrow_mut(|slot| *slot = Some(origin));
    let _origin = Origin;
    let result = call(&source, action(binding(&source), json!({"op": "disable"})));
    let request = worker.join().unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["value"]["status"], "disabled");
    assert!(request.starts_with("DELETE /v1/users/me/learning/devices/"));
    assert!(request
        .to_ascii_lowercase()
        .contains(&format!("authorization: bearer {}", "a".repeat(64))));
    assert!(storage.load_learning_keystore().unwrap().is_some());
    assert!(!result.to_string().contains("keystore"));
    assert!(result["value"]["device_id"].is_string());
}
