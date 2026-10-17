//! 「绑定 QQ 群助手」：群友在 QQ 群里 @机器人 发 `/bind` 拿到 8 位数字的绑定码，在这里提交（`POST /v1/users/me/qq-links {code}`），之后机器人的 `/stats` 读这个账号上传的打字统计。`GET /v1/users/me/qq-links` 列出本人的绑定，`DELETE /v1/users/me/qq-links/{id}` 解绑（msime-cloud `internal/account/typing_statistics.go`）。
//!
//! 绑定码由机器人发、在 App 里输入，不反过来：群聊是公开的，App 出的码发进群里会被别人抢先拿去读到你的数据。服务端只存群和成员标识的 SHA-256，所以一条绑定只有编号和时间。码不存在、过期或用过是 404 `qq_link_code_not_found`（`code()` 为 `account_qq_link_code_not_found`），账号已经绑了 10 个群友是 409 `qq_link_limit`（`account_qq_link_limit`），码的格式不对是 400；每个用户每小时只能提交 10 次，猜错的也算。

use super::*;
use serde_json::Value;

pub const QQ_LINKS_PATH: &str = "/v1/users/me/qq-links";
/// 服务端一个账号最多绑定的群友数（`qqLinksPerUser`），列表最多读这么多行。
pub const MAX_QQ_LINKS: usize = 10;

/// 一条绑定。`created_at` 是服务端给的 RFC 3339 原文。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountQqLink {
    pub id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountQqLinks {
    pub links: Vec<AccountQqLink>,
}

/// 绑定码：正好 8 位 ASCII 数字。
pub fn valid_qq_link_code(code: &str) -> bool {
    code.len() == 8 && code.bytes().all(|byte| byte.is_ascii_digit())
}

/// 绑定编号：服务端的 bigserial，不以 0 开头的 1 到 19 位数字。
pub fn valid_qq_link_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 19
        && !id.starts_with('0')
        && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn link(row: &Value) -> Option<AccountQqLink> {
    let id = row
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| valid_qq_link_id(id))?;
    let created_at = row
        .get("created_at")
        .and_then(Value::as_str)
        .filter(|value| parse_rfc3339(value).is_some())?;
    Some(AccountQqLink {
        id: id.to_owned(),
        created_at: created_at.to_owned(),
    })
}

/// 解读 `{"links":[...]}`：外层不是这个形状时不可用，里面形状不对的行跳过。
pub(super) fn parse_qq_links(body: &[u8]) -> Result<AccountQqLinks, AccountFailure> {
    let value: Value = serde_json::from_slice(body).map_err(|_| AccountError::Unavailable)?;
    let rows = value
        .get("links")
        .and_then(Value::as_array)
        .ok_or(AccountError::Unavailable)?;
    Ok(AccountQqLinks {
        links: rows.iter().take(MAX_QQ_LINKS).filter_map(link).collect(),
    })
}

impl BackendAccountClient {
    pub(super) fn list_qq_links(
        &self,
        access_token: &str,
    ) -> Result<AccountQqLinks, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let bytes = self.request_detailed(
            Method::GET,
            QQ_LINKS_PATH,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )?;
        parse_qq_links(&bytes)
    }

    pub(super) fn redeem_qq_link(
        &self,
        code: &str,
        access_token: &str,
    ) -> Result<AccountQqLink, AccountFailure> {
        if !valid_qq_link_code(code) || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
        }
        let body = serde_json::to_vec(&Body { code }).map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_detailed(
            Method::POST,
            QQ_LINKS_PATH,
            Some(access_token),
            Some(body),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )?;
        serde_json::from_slice::<Value>(&bytes)
            .ok()
            .as_ref()
            .and_then(link)
            .ok_or_else(|| AccountError::Unavailable.into())
    }

    pub(super) fn delete_qq_link(
        &self,
        id: &str,
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        if !valid_qq_link_id(id) || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        self.request_detailed(
            Method::DELETE,
            &format!("{QQ_LINKS_PATH}/{id}"),
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

    #[test]
    fn codes_and_ids_have_the_server_shape() {
        assert!(valid_qq_link_code("01234567"));
        for code in [
            "1234567",
            "123456789",
            "1234 567",
            "１２３４５６７８",
            "abcdefgh",
            "",
        ] {
            assert!(!valid_qq_link_code(code), "{code}");
        }
        assert!(valid_qq_link_id("1") && valid_qq_link_id("9223372036854775807"));
        for id in ["", "0", "01", "1a", "12345678901234567890", "../1"] {
            assert!(!valid_qq_link_id(id), "{id}");
        }
    }

    #[test]
    fn server_refusals_have_their_own_codes() {
        let failure = |status, code: &str| {
            AccountFailure::from_http_response(
                status,
                format!(r#"{{"error":{{"code":"{code}"}}}}"#).as_bytes(),
                None,
            )
            .code()
        };
        assert_eq!(
            failure(404, "qq_link_code_not_found"),
            "account_qq_link_code_not_found"
        );
        assert_eq!(failure(409, "qq_link_limit"), "account_qq_link_limit");
        assert_eq!(failure(404, "qq_link_not_found"), "account_not_found");
        assert_eq!(failure(429, "rate_limit_exceeded"), "account_rate_limited");
    }

    #[test]
    fn the_list_keeps_well_formed_rows() {
        let parsed = parse_qq_links(
            br#"{"links":[{"id":"12","created_at":"2026-10-17T08:00:00Z"},{"id":"x","created_at":"2026-10-17T08:00:00Z"},{"id":"13","created_at":"yesterday"},{"id":14,"created_at":"2026-10-17T08:00:00Z"}]}"#,
        )
        .unwrap();
        assert_eq!(
            parsed.links,
            [AccountQqLink {
                id: "12".to_owned(),
                created_at: "2026-10-17T08:00:00Z".to_owned()
            }]
        );
        assert_eq!(parse_qq_links(br#"{"links":[]}"#).unwrap().links, []);
        for body in [&b""[..], br#"{}"#, br#"{"links":{}}"#] {
            assert!(parse_qq_links(body).is_err());
        }
    }
}
