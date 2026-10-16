//! 「其他平台下载」的「发送链接」：`POST /v1/users/me/download-link {platform}`，服务端把固定模板的下载页链接发到账号已经验证过的邮箱，返回 202 `{sent_to}`（打码后的收件地址，例如 `u***@example.com`）（msime-cloud `internal/account/download_link.go`）。
//!
//! 收件地址由服务端决定，客户端给不了。没有已验证邮箱的账号收到 409 `no_verified_email`，`code()` 是 `account_no_verified_email`，宿主改为提示「复制链接」。每个用户每小时 3 次、每天 10 次；邮件没有配置或发送失败是 503（`provider_disabled`、`delivery_failed`），映射成 `account_unavailable`。

use super::*;

pub const DOWNLOAD_LINK_PATH: &str = "/v1/users/me/download-link";
/// 服务端认的平台（`downloadLinkPlatforms`），也是官网下载页 `release` 参数的取值。
pub const DOWNLOAD_LINK_PLATFORMS: [&str; 7] = [
    "windows",
    "macos",
    "linux",
    "harmony-pc",
    "ios",
    "android",
    "harmony",
];
/// 打码后的收件地址的字节上限，与 Android 的 `DownloadLinkApi` 相同（邮箱地址最长 320 字节）。
const MAX_SENT_TO_BYTES: usize = 320;

/// 发送的结果。`sent_to` 是服务端打码后的收件地址；服务端已经回了 2xx（邮件已发出），但这个字段缺失或不合规（空、超过 320 字节、含控制字符）时为 `None`，不把已经发出的邮件报成失败。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDownloadLink {
    pub sent_to: Option<String>,
}

/// 从 2xx 的正文里读 `sent_to`，读不出或不合规时为 `None`。
fn sent_to(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Response {
        sent_to: String,
    }
    serde_json::from_slice::<Response>(body)
        .ok()
        .map(|response| response.sent_to)
        .filter(|value| !value.is_empty() && crate::text::is_bounded_text(value, MAX_SENT_TO_BYTES))
}

impl BackendAccountClient {
    pub(super) fn send_download_link(
        &self,
        platform: &str,
        access_token: &str,
    ) -> Result<AccountDownloadLink, AccountFailure> {
        if !DOWNLOAD_LINK_PLATFORMS.contains(&platform)
            || !crate::text::is_lower_hex(access_token, 64)
        {
            return Err(AccountError::Invalid.into());
        }
        #[derive(Serialize)]
        struct Body<'a> {
            platform: &'a str,
        }
        let body = serde_json::to_vec(&Body { platform }).map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_detailed(
            Method::POST,
            DOWNLOAD_LINK_PATH,
            Some(access_token),
            Some(body),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )?;
        Ok(AccountDownloadLink {
            sent_to: sent_to(&bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_masked_address_is_kept_only_when_it_is_plain_text() {
        assert_eq!(
            sent_to(br#"{"sent_to":"u***@example.test"}"#).as_deref(),
            Some("u***@example.test")
        );
        for body in [
            &b""[..],
            br#"{}"#,
            br#"{"sent_to":""}"#,
            br#"{"sent_to":7}"#,
            br#"{"sent_to":"u***@example.test\u0007"}"#,
        ] {
            assert_eq!(sent_to(body), None, "{}", String::from_utf8_lossy(body));
        }
        let long = format!(r#"{{"sent_to":"{}"}}"#, "a".repeat(MAX_SENT_TO_BYTES + 1));
        assert_eq!(sent_to(long.as_bytes()), None);
    }
}
