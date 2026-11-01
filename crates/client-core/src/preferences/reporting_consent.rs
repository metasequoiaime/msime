//! 上报持续同意的本机身份。由偏好存储与开关一起保存，不接受调用方提供的身份。

use super::PreferencesError;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageReportingConsent {
    pub epoch: Uuid,
    pub since_unix_ms: u64,
}

impl UsageReportingConsent {
    pub(super) fn new() -> Result<Self, PreferencesError> {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?;
        let since_unix_ms = u64::try_from(since.as_millis()).map_err(std::io::Error::other)?;
        let consent = Self {
            epoch: Uuid::new_v4(),
            since_unix_ms,
        };
        if !consent.valid() {
            return Err(std::io::Error::other("invalid reporting consent clock").into());
        }
        Ok(consent)
    }

    pub(super) fn valid(&self) -> bool {
        self.epoch.get_version() == Some(uuid::Version::Random)
            && self.epoch.get_variant() == uuid::Variant::RFC4122
            && (1..=253_402_300_799_999).contains(&self.since_unix_ms)
    }
}

/// 元数据损坏只使授权历史失效，不能拖垮其余设置或覆盖用户实际的开关。
pub(super) fn deserialize<'de, D>(
    deserializer: D,
) -> Result<Option<UsageReportingConsent>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value::<UsageReportingConsent>(value)
        .ok()
        .filter(UsageReportingConsent::valid))
}
