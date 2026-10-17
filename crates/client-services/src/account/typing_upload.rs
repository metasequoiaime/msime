//! 打字统计上云：`PUT /v1/users/me/typing-statistics` 上传最近几天的计数，`DELETE` 清空本人全部设备的云端副本（msime-cloud `internal/account/typing_statistics.go`）。请求体由 [`crate::typing_statistics::upload::build_upload`] 构建，这里只负责发送。
//!
//! 两个接口都回 204。请求体最多 64 KiB，七天的计数远小于它；超过时本地就拒绝（`account_too_large`）。每个用户每小时 60 次，超了是 `account_rate_limited`。

use super::*;
use crate::typing_statistics::upload::TypingStatisticsUpload;

pub const TYPING_STATISTICS_PATH: &str = "/v1/users/me/typing-statistics";
/// 服务端的请求体上限（`typingStatisticsBodyBytes`）。
const MAX_TYPING_STATISTICS_BYTES: usize = 64 * 1024;

impl BackendAccountClient {
    pub(super) fn put_typing_statistics(
        &self,
        upload: &TypingStatisticsUpload,
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) || upload.days.is_empty() {
            return Err(AccountError::Invalid.into());
        }
        let body = serde_json::to_vec(upload).map_err(|_| AccountError::Invalid)?;
        self.request_detailed(
            Method::PUT,
            TYPING_STATISTICS_PATH,
            Some(access_token),
            Some(body),
            MAX_TYPING_STATISTICS_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )
        .map(|_| ())
    }

    pub(super) fn delete_typing_statistics(
        &self,
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        self.request_detailed(
            Method::DELETE,
            TYPING_STATISTICS_PATH,
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
