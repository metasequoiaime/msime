//! 中继的全部接口逻辑，不碰网络：[`RelayStore::handle`] 收一个已经认出空间的请求，返回状态码和正文。接口的形状与 `docs/cloud-clipboard-e2e.md` 完全相同，中继自己的差异（静态令牌、空间默认开启、条目不记写入设备）见 `docs/cloud-clipboard-relay.md`。
//!
//! 每个请求在一个 SQLite 事务里完成，所以并发换钥只有一个成功、去重和五十条上限不会被并发上传打破，进程中途被杀也不会留下一半的状态。

//! 请求的解析和不依赖状态的校验（路由、正文形状、公钥与编号一致、封装和密文的格式）与客户端的 WebDAV 后端共用 `msime_client_core::cloud::clipboard_log` 里的那一份，两边答出的错误码因此一致；这里只做依赖状态的部分。

use msime_client_core::cloud::clipboard_log::{parse_request, RelayRequest, WrapEntry};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;

pub use msime_client_core::cloud::clipboard_log::{
    Response, MAX_CIPHERTEXT_CHARS, MAX_DEVICES, MAX_ITEMS,
};

/// 交给 [`RelayStore::handle`] 的请求。`path` 是去掉基地址之后的路径，可以带查询串。
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a [u8],
}

/// 一次请求里出错的两种情况：按协议回答的错误，和数据库本身的失败。
enum Failure {
    Reply(Response),
    Database,
}

impl From<rusqlite::Error> for Failure {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

type Outcome = Result<Response, Failure>;

fn reply(status: u16, code: &str) -> Failure {
    Failure::Reply(Response::error(status, code))
}

/// 全部空间的数据，存在一个 SQLite 文件里。
pub struct RelayStore {
    connection: Mutex<Connection>,
}

impl RelayStore {
    /// 打开（没有时创建）数据文件。
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::with_connection(Connection::open(path)?)
    }

    /// 只在内存里的一份，测试用。
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS spaces(
                 space TEXT PRIMARY KEY,
                 enabled INTEGER NOT NULL DEFAULT 1,
                 retention_days INTEGER NOT NULL DEFAULT 0,
                 key_id TEXT,
                 sequence INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS devices(
                 space TEXT NOT NULL,
                 device_id TEXT NOT NULL,
                 public_key TEXT NOT NULL,
                 name TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 ordinal INTEGER NOT NULL,
                 PRIMARY KEY(space, device_id)
             );
             CREATE TABLE IF NOT EXISTS wraps(
                 space TEXT NOT NULL,
                 device_id TEXT NOT NULL,
                 key_id TEXT NOT NULL,
                 wrap TEXT NOT NULL,
                 PRIMARY KEY(space, device_id)
             );
             CREATE TABLE IF NOT EXISTS items(
                 space TEXT NOT NULL,
                 id TEXT NOT NULL,
                 ciphertext TEXT NOT NULL,
                 key_id TEXT NOT NULL,
                 dedup TEXT NOT NULL,
                 sequence INTEGER NOT NULL,
                 updated_at TEXT NOT NULL,
                 updated_unix INTEGER NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY(space, id),
                 UNIQUE(space, dedup)
             );",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// 回答 `space` 里的一个请求。`space` 由调用方从令牌算出，这里不再核对凭据。
    pub fn handle(&self, space: &str, request: &Request<'_>) -> Response {
        let parsed = match parse_request(request.method, request.path, request.body) {
            Ok(parsed) => parsed,
            Err(response) => return response,
        };
        let mut connection = match self.connection.lock() {
            Ok(connection) => connection,
            Err(poisoned) => poisoned.into_inner(),
        };
        let outcome = (|| -> Outcome {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT OR IGNORE INTO spaces(space) VALUES(?1)",
                params![space],
            )?;
            let space = Space {
                transaction: &transaction,
                id: space,
            };
            let response = route(&space, parsed)?;
            if (200..300).contains(&response.status) {
                transaction.commit()?;
            }
            Ok(response)
        })();
        match outcome {
            Ok(response) | Err(Failure::Reply(response)) => response,
            Err(Failure::Database) => Response::error(500, "internal"),
        }
    }
}

fn route(space: &Space<'_>, request: RelayRequest) -> Outcome {
    match request {
        RelayRequest::List => space.list(),
        RelayRequest::Add {
            ciphertext,
            key_id,
            dedup,
        } => space.add(&ciphertext, &key_id, &dedup),
        RelayRequest::Clear => space.clear(),
        RelayRequest::Keys { device_id } => space.keys(&device_id),
        RelayRequest::Rotate {
            previous_key_id,
            key_id,
            wraps,
            revoke,
        } => space.rotate(previous_key_id, &key_id, &wraps, &revoke),
        RelayRequest::Approve {
            key_id,
            device_id,
            wrap,
        } => space.approve(&key_id, &device_id, &wrap),
        RelayRequest::Register {
            device_id,
            public_key,
            name,
        } => space.register(&device_id, &public_key, &name),
        RelayRequest::Reject { device_id } => space.reject(&device_id),
        RelayRequest::SetEnabled { enabled } => space.set_enabled(enabled),
        RelayRequest::SetRetention { days } => space.set_retention(days),
        RelayRequest::Delete { id } => space.delete(&id),
        RelayRequest::Pin { id, pinned } => space.pin(&id, pinned),
    }
}

fn now() -> (String, i64) {
    let now = time::OffsetDateTime::now_utc();
    let now = now.replace_nanosecond(0).unwrap_or(now);
    let text = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    (text, now.unix_timestamp())
}

fn random_id() -> Result<String, Failure> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Database)?;
    Ok(hex::encode(bytes))
}

struct Space<'a> {
    transaction: &'a Transaction<'a>,
    id: &'a str,
}

struct State {
    enabled: bool,
    retention_days: i64,
    key_id: Option<String>,
}

impl Space<'_> {
    fn state(&self) -> Result<State, Failure> {
        Ok(self.transaction.query_row(
            "SELECT enabled, retention_days, key_id FROM spaces WHERE space=?1",
            params![self.id],
            |row| {
                Ok(State {
                    enabled: row.get::<_, i64>(0)? != 0,
                    retention_days: row.get(1)?,
                    key_id: row.get(2)?,
                })
            },
        )?)
    }

    fn device_exists(&self, device_id: &str) -> Result<bool, Failure> {
        Ok(self
            .transaction
            .query_row(
                "SELECT 1 FROM devices WHERE space=?1 AND device_id=?2",
                params![self.id, device_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// 超过保留期的未置顶条目。官方服务端定时清理，中继在每次读写条目时顺手清掉。
    fn prune(&self, retention_days: i64) -> Result<(), Failure> {
        if retention_days > 0 {
            let cutoff = now().1 - retention_days * 86_400;
            self.transaction.execute(
                "DELETE FROM items WHERE space=?1 AND pinned=0 AND updated_unix<?2",
                params![self.id, cutoff],
            )?;
        }
        Ok(())
    }

    fn item_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "ciphertext": row.get::<_, String>(1)?,
            "key_id": row.get::<_, String>(2)?,
            "updated_at": row.get::<_, String>(3)?,
            "pinned": row.get::<_, i64>(4)? != 0,
            // 中继没有登录会话，认不出写入的设备，与官方服务端认不出时一样留空。
            "device": "",
        }))
    }

    fn keys(&self, device_id: &str) -> Outcome {
        let state = self.state()?;
        let wrap: Option<String> = match &state.key_id {
            Some(key_id) => self
                .transaction
                .query_row(
                    "SELECT wrap FROM wraps WHERE space=?1 AND device_id=?2 AND key_id=?3",
                    params![self.id, device_id, key_id],
                    |row| row.get(0),
                )
                .optional()?,
            None => None,
        };
        let mut statement = self.transaction.prepare(
            "SELECT d.device_id, d.public_key, d.name, d.created_at, w.device_id IS NOT NULL
             FROM devices d
             LEFT JOIN spaces s ON s.space=d.space
             LEFT JOIN wraps w ON w.space=d.space AND w.device_id=d.device_id AND w.key_id=s.key_id
             WHERE d.space=?1 ORDER BY d.ordinal",
        )?;
        let devices = statement
            .query_map(params![self.id], |row| {
                Ok(json!({
                    "device_id": row.get::<_, String>(0)?,
                    "public_key": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "approved": row.get::<_, bool>(4)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Response::json(
            200,
            json!({
                "enabled": state.enabled,
                "key_id": state.key_id,
                "wrap": wrap,
                "devices": devices,
            }),
        ))
    }

    fn register(&self, device_id: &str, public_key: &str, name: &str) -> Outcome {
        let renamed = self.transaction.execute(
            "UPDATE devices SET name=?3 WHERE space=?1 AND device_id=?2",
            params![self.id, device_id, name],
        )?;
        if renamed == 1 {
            return Ok(Response::no_content());
        }
        let count: i64 = self.transaction.query_row(
            "SELECT count(*) FROM devices WHERE space=?1",
            params![self.id],
            |row| row.get(0),
        )?;
        if count >= MAX_DEVICES as i64 {
            // 满了时先淘汰最早登记、还没被批准的一台；都已批准时拒绝。
            let evicted = self.transaction.execute(
                "DELETE FROM devices WHERE space=?1 AND device_id=(
                     SELECT d.device_id FROM devices d LEFT JOIN spaces s ON s.space=d.space
                     WHERE d.space=?1 AND NOT EXISTS(
                         SELECT 1 FROM wraps w WHERE w.space=d.space AND w.device_id=d.device_id AND w.key_id=s.key_id)
                     ORDER BY d.ordinal LIMIT 1)",
                params![self.id],
            )?;
            if evicted == 0 {
                return Err(reply(409, "clipboard_device_limit"));
            }
        }
        let ordinal: i64 = self.transaction.query_row(
            "SELECT COALESCE(max(ordinal), 0) + 1 FROM devices WHERE space=?1",
            params![self.id],
            |row| row.get(0),
        )?;
        self.transaction.execute(
            "DELETE FROM wraps WHERE space=?1 AND device_id=?2",
            params![self.id, device_id],
        )?;
        self.transaction.execute(
            "INSERT INTO devices(space, device_id, public_key, name, created_at, ordinal) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.id, device_id, public_key, name, now().0, ordinal],
        )?;
        Ok(Response::no_content())
    }

    fn reject(&self, device_id: &str) -> Outcome {
        let removed = self.transaction.execute(
            "DELETE FROM devices WHERE space=?1 AND device_id=?2",
            params![self.id, device_id],
        )?;
        if removed == 0 {
            return Err(reply(404, "not_found"));
        }
        self.transaction.execute(
            "DELETE FROM wraps WHERE space=?1 AND device_id=?2",
            params![self.id, device_id],
        )?;
        Ok(Response::no_content())
    }

    fn rotate(
        &self,
        previous_key_id: Option<String>,
        key_id: &str,
        wraps: &[WrapEntry],
        revoke: &[String],
    ) -> Outcome {
        let state = self.state()?;
        if !state.enabled {
            return Err(reply(403, "clipboard_sync_disabled"));
        }
        if state.key_id != previous_key_id {
            return Err(reply(409, "clipboard_key_conflict"));
        }
        for device_id in revoke {
            self.transaction.execute(
                "DELETE FROM devices WHERE space=?1 AND device_id=?2",
                params![self.id, device_id],
            )?;
        }
        for wrap in wraps {
            if !self.device_exists(&wrap.device_id)? {
                return Err(reply(400, "invalid_clipboard_key"));
            }
        }
        self.transaction
            .execute("DELETE FROM items WHERE space=?1", params![self.id])?;
        self.transaction
            .execute("DELETE FROM wraps WHERE space=?1", params![self.id])?;
        self.transaction.execute(
            "UPDATE spaces SET key_id=?2 WHERE space=?1",
            params![self.id, key_id],
        )?;
        for wrap in wraps {
            self.transaction.execute(
                "INSERT INTO wraps(space, device_id, key_id, wrap) VALUES(?1, ?2, ?3, ?4)",
                params![self.id, wrap.device_id, key_id, wrap.wrap],
            )?;
        }
        Ok(Response::no_content())
    }

    fn approve(&self, key_id: &str, device_id: &str, wrap: &str) -> Outcome {
        if self.state()?.key_id.as_deref() != Some(key_id) {
            return Err(reply(409, "clipboard_key_conflict"));
        }
        if !self.device_exists(device_id)? {
            return Err(reply(404, "not_found"));
        }
        self.transaction.execute(
            "INSERT INTO wraps(space, device_id, key_id, wrap) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(space, device_id) DO UPDATE SET key_id=excluded.key_id, wrap=excluded.wrap",
            params![self.id, device_id, key_id, wrap],
        )?;
        Ok(Response::no_content())
    }

    fn list(&self) -> Outcome {
        let state = self.state()?;
        self.prune(state.retention_days)?;
        let items = match &state.key_id {
            Some(key_id) => {
                let mut statement = self.transaction.prepare(
                    "SELECT id, ciphertext, key_id, updated_at, pinned FROM items
                     WHERE space=?1 AND key_id=?2 ORDER BY pinned DESC, sequence DESC LIMIT ?3",
                )?;
                let items = statement
                    .query_map(params![self.id, key_id, MAX_ITEMS as i64], Self::item_json)?
                    .collect::<Result<Vec<_>, _>>()?;
                items
            }
            None => Vec::new(),
        };
        Ok(Response::json(
            200,
            json!({
                "enabled": state.enabled,
                "retention_days": state.retention_days,
                "key_id": state.key_id,
                "items": items,
            }),
        ))
    }

    fn add(&self, ciphertext: &str, key_id: &str, dedup: &str) -> Outcome {
        let state = self.state()?;
        if !state.enabled {
            return Err(reply(403, "clipboard_sync_disabled"));
        }
        if state.key_id.as_deref() != Some(key_id) {
            return Err(reply(409, "clipboard_key_conflict"));
        }
        self.prune(state.retention_days)?;
        // 不属于当前密钥的条目不会再出现在列表里，趁这次写入删掉，免得占用五十条的名额。
        self.transaction.execute(
            "DELETE FROM items WHERE space=?1 AND key_id<>?2",
            params![self.id, key_id],
        )?;
        // 五十条都已置顶时，新的一条会在下面裁剪时被立刻删掉：明确答 409，而不是挤掉置顶的条目或答 500。去重标签落在一条置顶条目上时只是更新它，照常接受。
        let (pinned, pinned_duplicate): (i64, bool) = self.transaction.query_row(
            "SELECT (SELECT COUNT(*) FROM items WHERE space=?1 AND pinned<>0),
                    EXISTS(SELECT 1 FROM items WHERE space=?1 AND dedup=?2 AND pinned<>0)",
            params![self.id, dedup],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if pinned >= MAX_ITEMS as i64 && !pinned_duplicate {
            return Err(reply(409, "clipboard_full"));
        }
        let sequence: i64 = self.transaction.query_row(
            "UPDATE spaces SET sequence=sequence+1 WHERE space=?1 RETURNING sequence",
            params![self.id],
            |row| row.get(0),
        )?;
        let (updated_at, updated_unix) = now();
        self.transaction.execute(
            "INSERT INTO items(space, id, ciphertext, key_id, dedup, sequence, updated_at, updated_unix)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(space, dedup) DO UPDATE SET ciphertext=excluded.ciphertext, key_id=excluded.key_id,
                 sequence=excluded.sequence, updated_at=excluded.updated_at, updated_unix=excluded.updated_unix",
            params![self.id, random_id()?, ciphertext, key_id, dedup, sequence, updated_at, updated_unix],
        )?;
        self.transaction.execute(
            "DELETE FROM items WHERE space=?1 AND id NOT IN (
                 SELECT id FROM items WHERE space=?1 ORDER BY pinned DESC, sequence DESC LIMIT ?2)",
            params![self.id, MAX_ITEMS as i64],
        )?;
        let item = self.transaction.query_row(
            "SELECT id, ciphertext, key_id, updated_at, pinned FROM items WHERE space=?1 AND dedup=?2",
            params![self.id, dedup],
            Self::item_json,
        )?;
        Ok(Response::json(200, item))
    }

    fn clear(&self) -> Outcome {
        self.transaction
            .execute("DELETE FROM items WHERE space=?1", params![self.id])?;
        Ok(Response::no_content())
    }

    fn delete(&self, id: &str) -> Outcome {
        let removed = self.transaction.execute(
            "DELETE FROM items WHERE space=?1 AND id=?2",
            params![self.id, id],
        )?;
        if removed == 0 {
            return Err(reply(404, "not_found"));
        }
        Ok(Response::no_content())
    }

    fn pin(&self, id: &str, pinned: bool) -> Outcome {
        let changed = self.transaction.execute(
            "UPDATE items SET pinned=?3 WHERE space=?1 AND id=?2",
            params![self.id, id, pinned],
        )?;
        if changed == 0 {
            return Err(reply(404, "not_found"));
        }
        Ok(Response::no_content())
    }

    fn set_retention(&self, days: i64) -> Outcome {
        self.transaction.execute(
            "UPDATE spaces SET retention_days=?2 WHERE space=?1",
            params![self.id, days],
        )?;
        Ok(Response::no_content())
    }

    /// 关闭时连同条目、密钥、封装和全部设备登记一起删除，再次打开时由第一台设备重新建钥。
    fn set_enabled(&self, enabled: bool) -> Outcome {
        self.transaction.execute(
            "UPDATE spaces SET enabled=?2 WHERE space=?1",
            params![self.id, enabled],
        )?;
        if !enabled {
            self.transaction.execute(
                "UPDATE spaces SET key_id=NULL WHERE space=?1",
                params![self.id],
            )?;
            for table in ["items", "wraps", "devices"] {
                self.transaction.execute(
                    &format!("DELETE FROM {table} WHERE space=?1"),
                    params![self.id],
                )?;
            }
        }
        Ok(Response::json(200, json!({ "enabled": enabled })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
    use msime_client_core::is_lower_hex;
    use sha2::{Digest, Sha256};

    const API: &str = "/v1/users/me/clipboard";
    const WRAP_BYTES: usize = 32 + 24 + 32 + 16;

    const SPACE: &str = "synthetic-space";

    fn call(store: &RelayStore, method: &str, path: &str, body: Value) -> Response {
        let body = if body.is_null() {
            Vec::new()
        } else {
            body.to_string().into_bytes()
        };
        store.handle(
            SPACE,
            &Request {
                method,
                path,
                body: &body,
            },
        )
    }

    fn code(response: &Response) -> String {
        serde_json::from_str::<Value>(response.body.as_deref().unwrap()).unwrap()["error"]["code"]
            .as_str()
            .unwrap()
            .to_owned()
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

    #[test]
    fn a_new_space_starts_enabled_without_a_key() {
        let store = RelayStore::in_memory().unwrap();
        let (id, _) = device(1);
        let keys = call(
            &store,
            "GET",
            &format!("{API}/keys?device_id={id}"),
            Value::Null,
        );
        assert_eq!(keys.status, 200);
        let keys: Value = serde_json::from_str(keys.body.as_deref().unwrap()).unwrap();
        assert_eq!(
            keys,
            json!({ "enabled": true, "key_id": null, "wrap": null, "devices": [] })
        );
        assert_eq!(
            code(&call(
                &store,
                "GET",
                &format!("{API}/keys?device_id=nope"),
                Value::Null
            )),
            "invalid_device_id"
        );
    }

    #[test]
    fn devices_are_checked_against_their_public_key_and_capped() {
        let store = RelayStore::in_memory().unwrap();
        let (id, key) = device(1);
        let (_, other_key) = device(2);
        let put = |id: &str, key: &str, name: &str| {
            call(
                &store,
                "PUT",
                &format!("{API}/devices/{id}"),
                json!({ "public_key": key, "name": name }),
            )
        };
        assert_eq!(
            code(&put(&id, &other_key, "合成")),
            "invalid_clipboard_device"
        );
        assert_eq!(
            code(&put(&id, &key, "bad\u{7}")),
            "invalid_clipboard_device"
        );
        assert_eq!(
            code(&put(&id, &format!("{key}="), "合成")),
            "invalid_clipboard_device"
        );
        assert_eq!(put(&id, &key, "合成").status, 204);
        // 20 台都没批准时，第 21 台挤掉最早的一台。
        for seed in 2..=21 {
            let (id, key) = device(seed);
            assert_eq!(put(&id, &key, "合成").status, 204);
        }
        let keys = call(
            &store,
            "GET",
            &format!("{API}/keys?device_id={id}"),
            Value::Null,
        );
        let keys: Value = serde_json::from_str(keys.body.as_deref().unwrap()).unwrap();
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
        let rotated = call(
            &store,
            "POST",
            &format!("{API}/keys"),
            json!({ "previous_key_id": null, "key_id": "a".repeat(32), "wraps": wraps, "revoke": [] }),
        );
        assert_eq!(rotated.status, 204);
        let (late, late_key) = device(30);
        assert_eq!(
            code(&put(&late, &late_key, "合成")),
            "clipboard_device_limit"
        );
    }

    #[test]
    fn uploads_need_the_current_key_and_are_deduplicated() {
        let store = RelayStore::in_memory().unwrap();
        let (id, key) = device(1);
        call(
            &store,
            "PUT",
            &format!("{API}/devices/{id}"),
            json!({ "public_key": key, "name": "" }),
        );
        let key_id = "b".repeat(32);
        let ciphertext = format!("v1.{}", B64.encode([1u8; 60]));
        let upload = |key_id: &str, dedup: &str| {
            call(
                &store,
                "POST",
                API,
                json!({ "ciphertext": ciphertext, "key_id": key_id, "dedup": dedup }),
            )
        };
        assert_eq!(
            code(&upload(&key_id, &"d".repeat(64))),
            "clipboard_key_conflict"
        );
        call(
            &store,
            "POST",
            &format!("{API}/keys"),
            json!({ "previous_key_id": null, "key_id": key_id, "wraps": [{ "device_id": id, "wrap": wrap() }] }),
        );
        let first = upload(&key_id, &"d".repeat(64));
        assert_eq!(first.status, 200);
        let second = upload(&key_id, &"d".repeat(64));
        let first: Value = serde_json::from_str(first.body.as_deref().unwrap()).unwrap();
        let second: Value = serde_json::from_str(second.body.as_deref().unwrap()).unwrap();
        assert_eq!(first["id"], second["id"]);
        assert!(is_lower_hex(first["id"].as_str().unwrap(), 64));
        assert_eq!(
            code(&call(&store, "POST", API, json!({ "text": "明文" }))),
            "clipboard_e2e_required"
        );
        assert_eq!(
            code(&call(
                &store,
                "POST",
                API,
                json!({ "ciphertext": "v1.short", "key_id": key_id, "dedup": "d".repeat(64) })
            )),
            "invalid_clipboard_ciphertext"
        );
        // 第五十一条挤掉最旧的一条。
        for index in 0..MAX_ITEMS {
            assert_eq!(upload(&key_id, &format!("{index:064x}")).status, 200);
        }
        let page = call(&store, "GET", API, Value::Null);
        let page: Value = serde_json::from_str(page.body.as_deref().unwrap()).unwrap();
        assert_eq!(page["items"].as_array().unwrap().len(), MAX_ITEMS);
        assert!(page["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["id"] != first["id"]));
    }

    #[test]
    fn a_space_whose_items_are_all_pinned_refuses_new_uploads() {
        let store = RelayStore::in_memory().unwrap();
        let (id, key) = device(1);
        call(
            &store,
            "PUT",
            &format!("{API}/devices/{id}"),
            json!({ "public_key": key, "name": "" }),
        );
        let key_id = "b".repeat(32);
        call(
            &store,
            "POST",
            &format!("{API}/keys"),
            json!({ "previous_key_id": null, "key_id": key_id, "wraps": [{ "device_id": id, "wrap": wrap() }] }),
        );
        let ciphertext = format!("v1.{}", B64.encode([1u8; 60]));
        let upload = |dedup: &str| {
            call(
                &store,
                "POST",
                API,
                json!({ "ciphertext": ciphertext, "key_id": key_id, "dedup": dedup }),
            )
        };
        let mut ids = Vec::new();
        for index in 0..MAX_ITEMS {
            let item = upload(&format!("{index:064x}"));
            assert_eq!(item.status, 200);
            let item: Value = serde_json::from_str(item.body.as_deref().unwrap()).unwrap();
            ids.push(item["id"].as_str().unwrap().to_owned());
        }
        for id in &ids {
            let pinned = call(
                &store,
                "PUT",
                &format!("{API}/{id}/pin"),
                json!({ "pinned": true }),
            );
            assert_eq!(pinned.status, 204);
        }
        // 新的一条没有位置：答 409，置顶的条目一条不少。
        let refused = upload(&"e".repeat(64));
        assert_eq!(refused.status, 409);
        assert_eq!(code(&refused), "clipboard_full");
        let page = call(&store, "GET", API, Value::Null);
        let page: Value = serde_json::from_str(page.body.as_deref().unwrap()).unwrap();
        let items = page["items"].as_array().unwrap();
        assert_eq!(items.len(), MAX_ITEMS);
        assert!(items.iter().all(|item| item["pinned"] == true));
        // 重复上传一条已置顶的内容只是更新它。
        assert_eq!(upload(&format!("{:064x}", 0)).status, 200);
        // 取消一条置顶后又有了位置，新的一条挤掉的是这条未置顶的。
        call(
            &store,
            "PUT",
            &format!("{API}/{}/pin", ids[1]),
            json!({ "pinned": false }),
        );
        assert_eq!(upload(&"e".repeat(64)).status, 200);
        let page = call(&store, "GET", API, Value::Null);
        let page: Value = serde_json::from_str(page.body.as_deref().unwrap()).unwrap();
        let items = page["items"].as_array().unwrap();
        assert_eq!(items.len(), MAX_ITEMS);
        assert!(items.iter().all(|item| item["id"] != ids[1].as_str()));
    }

    #[test]
    fn unknown_routes_and_bodies_are_refused() {
        let store = RelayStore::in_memory().unwrap();
        assert_eq!(
            code(&call(&store, "GET", "/v1/users/me/profile", Value::Null)),
            "not_found"
        );
        assert_eq!(
            code(&call(&store, "GET", &format!("{API}x"), Value::Null)),
            "not_found"
        );
        assert_eq!(code(&call(&store, "PATCH", API, Value::Null)), "not_found");
        assert_eq!(
            code(&call(
                &store,
                "PUT",
                &format!("{API}/retention"),
                json!({ "days": 3 })
            )),
            "invalid_retention"
        );
        assert_eq!(
            code(&call(&store, "PUT", &format!("{API}/settings"), json!({}))),
            "enabled_required"
        );
        assert_eq!(
            code(&store.handle(
                SPACE,
                &Request {
                    method: "PUT",
                    path: &format!("{API}/settings"),
                    body: b"not json",
                }
            )),
            "invalid_json"
        );
        assert_eq!(
            code(&call(
                &store,
                "DELETE",
                &format!("{API}/{}", "c".repeat(64)),
                Value::Null
            )),
            "not_found"
        );
    }
}
