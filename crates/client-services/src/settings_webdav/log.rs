//! 有界的加密事件日志，复用现有 PUT 临时文件 + MOVE Overwrite:F 的独占创建。
use super::*;
use msime_client_core::settings_sync::{SealedSettingsEvent, MAX_SEALED_EVENT_BYTES};

const V1: &str = "msime-settings/v1/";
const TMP: &str = "msime-settings/v1/tmp/";
const FORMAT_FILE: &str = "msime-settings/v1/format.json";
/// 本阶段保留全部链，达到上限后显式停下；不能在没有检查点协议时静默丢弃旧事件。
const MAX_EVENTS: u64 = 1024;
const MAX_PULL_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct EventLog<'a> {
    dav: Dav<'a>,
}
impl<'a> EventLog<'a> {
    pub fn new(http: &'a dyn DavHttp, clock: &'a dyn Environment) -> Self {
        Self {
            dav: Dav::new(http, clock),
        }
    }

    fn path(sequence: u64) -> String {
        format!("{V1}e-{sequence:012}.json")
    }
    fn format() -> Vec<u8> {
        br#"{"format":"app.msime.settings-sync-log","version":1}"#.to_vec()
    }

    fn initialize(&self) -> Result<(), SettingsWebDavError> {
        for directory in [ROOT, V1, TMP] {
            self.dav.mkcol(directory)?;
        }
        if let Some(bytes) = self.dav.get(FORMAT_FILE, 4096)? {
            if serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                != serde_json::from_slice(&Self::format()).ok()
            {
                return Err(SettingsWebDavError::Protocol);
            }
        } else if !self.dav.create_in(TMP, FORMAT_FILE, &Self::format())? {
            let bytes = self
                .dav
                .get(FORMAT_FILE, 4096)?
                .ok_or(SettingsWebDavError::Protocol)?;
            if serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                != serde_json::from_slice(&Self::format()).ok()
            {
                return Err(SettingsWebDavError::Protocol);
            }
        }
        Ok(())
    }

    pub fn pull(
        &self,
        cipher: &SettingsSyncCipher,
        known: Option<&SettingsSyncEvent>,
    ) -> Result<Option<SettingsSyncEvent>, SettingsWebDavError> {
        self.initialize()?;
        let names = self.dav.list(V1)?.ok_or(SettingsWebDavError::Protocol)?;
        let mut numbers = std::collections::BTreeSet::new();
        for name in names {
            if let Some(digits) = name
                .strip_prefix("e-")
                .and_then(|name| name.strip_suffix(".json"))
            {
                if digits.len() != 12 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(SettingsWebDavError::Protocol);
                }
                let number = digits
                    .parse::<u64>()
                    .map_err(|_| SettingsWebDavError::Protocol)?;
                if number >= MAX_EVENTS {
                    return Err(SettingsWebDavError::LogFull);
                }
                numbers.insert(number);
            }
        }
        let mut latest = known.cloned();
        let mut pulled = 0;
        if let Some(known) = known {
            // 直接读取已知链头，不信任可能过期或被篡改的列目录结果。
            let bytes = self
                .dav
                .get(&Self::path(known.sequence), MAX_SEALED_EVENT_BYTES as u64)?
                .ok_or(RelayError::Rollback)?;
            let current = cipher.open_event(&SealedSettingsEvent::from_json(&bytes)?)?;
            if current != *known {
                return Err(RelayError::Rollback.into());
            }
            if numbers.last().is_none_or(|last| *last < known.sequence) {
                return Err(RelayError::Rollback.into());
            }
        }
        let start = known.map_or(0, |known| known.sequence + 1);
        let end = numbers.last().copied();
        if let Some(end) = end {
            for sequence in start..=end {
                if !numbers.contains(&sequence) {
                    return Err(RelayError::Rollback.into());
                }
                let bytes = self
                    .dav
                    .get(&Self::path(sequence), MAX_SEALED_EVENT_BYTES as u64)?
                    .ok_or(RelayError::Rollback)?;
                pulled += bytes.len();
                if pulled > MAX_PULL_BYTES {
                    return Err(SettingsWebDavError::LogFull);
                }
                let event = cipher.open_event(&SealedSettingsEvent::from_json(&bytes)?)?;
                if event.sequence != sequence {
                    return Err(RelayError::Rollback.into());
                }
                if let Some(previous) = latest.as_ref() {
                    event
                        .validate_successor(previous)
                        .map_err(|_| RelayError::Rollback)?;
                } else if event.sequence != 0 {
                    return Err(RelayError::Rollback.into());
                }
                latest = Some(event);
            }
        }
        Ok(latest)
    }

    pub fn append(
        &self,
        cipher: &SettingsSyncCipher,
        event: &SettingsSyncEvent,
    ) -> Result<(), SettingsWebDavError> {
        if event.sequence >= MAX_EVENTS {
            return Err(SettingsWebDavError::LogFull);
        }
        let bytes = cipher.seal_event(event)?.to_json()?;
        if !self
            .dav
            .create_in(TMP, &Self::path(event.sequence), &bytes)?
        {
            // 另一台设备抢到了序号，本轮不覆盖；下一轮重拉并重新合并。
            return Err(SettingsWebDavError::Conflict);
        }
        Ok(())
    }
}
