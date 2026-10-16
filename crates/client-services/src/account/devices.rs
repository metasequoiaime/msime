//! 「我的设备」：`GET /v1/users/me/sessions` 列出本人未撤销、未过期的登录会话，`DELETE /v1/users/me/sessions/{id}` 撤销其中一个（msime-cloud `internal/account/sessions.go`）。
//!
//! 每一行只按 JSON 类型校验（字符串、布尔、RFC 3339 时间），不对 `platform` 做枚举校验：浏览器里登录的会话 `platform` 是 `web`，User-Agent 解析不出来时 `platform`、`name`、`app_version` 都是空串，这些都要保留，否则用户没法撤销网页登录。类型不对的行跳过，最多读 50 行。

use super::*;
use serde_json::Value;

pub const SESSIONS_PATH: &str = "/v1/users/me/sessions";
/// 服务端一次最多列出的会话数（`maxListedSessions`）。
pub const MAX_LISTED_SESSIONS: usize = 50;
const MAX_PLATFORM_BYTES: usize = 32;
const MAX_NAME_BYTES: usize = 512;
const MAX_APP_VERSION_BYTES: usize = 64;

/// 一个登录会话。时间是服务端给的 RFC 3339 原文。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDevice {
    pub id: String,
    pub platform: String,
    pub name: String,
    pub app_version: String,
    pub created_at: String,
    pub last_active: String,
    /// 是不是发出这次请求的会话。
    pub current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDevices {
    pub sessions: Vec<AccountDevice>,
}

/// 服务端会话 id 的形状：64 位小写十六进制（`sessionID`）。本地保存的 `SavedAccountSession::session_id` 是客户端生成的 UUID，不是它。
pub fn valid_device_session_id(id: &str) -> bool {
    crate::text::is_lower_hex(id, 64)
}

/// 解读 `{"sessions":[...]}`：外层不是这个形状时不可用，里面类型不对的行跳过。
pub(super) fn parse_devices(body: &[u8]) -> Result<AccountDevices, AccountFailure> {
    let value: Value = serde_json::from_slice(body).map_err(|_| AccountError::Unavailable)?;
    let rows = value
        .get("sessions")
        .and_then(Value::as_array)
        .ok_or(AccountError::Unavailable)?;
    let sessions = rows
        .iter()
        .take(MAX_LISTED_SESSIONS)
        .filter_map(device)
        .collect();
    Ok(AccountDevices { sessions })
}

fn device(row: &Value) -> Option<AccountDevice> {
    let text = |key: &str, maximum: usize| {
        row.get(key)
            .and_then(Value::as_str)
            .filter(|value| crate::text::is_bounded_text(value, maximum))
            .map(str::to_owned)
    };
    let time = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .filter(|value| parse_rfc3339(value).is_some())
            .map(str::to_owned)
    };
    let id = text("id", 64).filter(|id| valid_device_session_id(id))?;
    Some(AccountDevice {
        id,
        platform: text("platform", MAX_PLATFORM_BYTES)?,
        name: text("name", MAX_NAME_BYTES)?,
        app_version: text("app_version", MAX_APP_VERSION_BYTES)?,
        created_at: time("created_at")?,
        last_active: time("last_active")?,
        current: row.get("current")?.as_bool()?,
    })
}

impl BackendAccountClient {
    pub(super) fn list_devices(
        &self,
        access_token: &str,
    ) -> Result<AccountDevices, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let bytes = self.request_detailed(
            Method::GET,
            SESSIONS_PATH,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )?;
        parse_devices(&bytes)
    }

    pub(super) fn revoke_device(&self, id: &str, access_token: &str) -> Result<(), AccountFailure> {
        if !valid_device_session_id(id) || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        self.request_detailed(
            Method::DELETE,
            &format!("{SESSIONS_PATH}/{id}"),
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> Value {
        serde_json::json!({
            "id": id,
            "platform": "macos",
            "name": "MacBook",
            "app_version": "0.9.0",
            "created_at": "2026-10-01T08:00:00Z",
            "last_active": "2026-10-16T08:00:00.123456Z",
            "current": false,
        })
    }

    #[test]
    fn web_and_unknown_sessions_are_kept_and_bad_rows_skipped() {
        let id = |byte: char| byte.to_string().repeat(64);
        let mut web = row(&id('a'));
        web["platform"] = "web".into();
        web["name"] = "Chrome · iPhone".into();
        web["app_version"] = "".into();
        let mut unknown = row(&id('b'));
        unknown["platform"] = "".into();
        unknown["name"] = "".into();
        unknown["app_version"] = "".into();
        unknown["current"] = true.into();
        let mut wrong_type = row(&id('c'));
        wrong_type["current"] = "yes".into();
        let mut bad_time = row(&id('d'));
        bad_time["created_at"] = "yesterday".into();
        let bad_id = row("not-a-session");
        let body = serde_json::json!({ "sessions": [web, unknown, wrong_type, bad_time, bad_id] });
        let devices = parse_devices(body.to_string().as_bytes()).unwrap();
        assert_eq!(devices.sessions.len(), 2);
        assert_eq!(devices.sessions[0].platform, "web");
        assert_eq!(devices.sessions[1].platform, "");
        assert!(devices.sessions[1].current);

        let many: Vec<_> = (0..60).map(|_| row(&id('e'))).collect();
        let devices = parse_devices(
            serde_json::json!({ "sessions": many })
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(devices.sessions.len(), MAX_LISTED_SESSIONS);
        assert_eq!(
            parse_devices(br#"{"sessions":{}}"#).unwrap_err().code(),
            "account_unavailable"
        );
    }

    #[test]
    fn session_ids_are_64_lowercase_hex() {
        assert!(valid_device_session_id(&"0f".repeat(32)));
        assert!(!valid_device_session_id(&"0F".repeat(32)));
        assert!(!valid_device_session_id(&"0f".repeat(31)));
        assert!(!valid_device_session_id(
            "6f9619ff-8b86-d011-b42d-00c04fc964ff"
        ));
    }
}
