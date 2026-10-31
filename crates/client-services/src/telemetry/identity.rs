use super::{canonical_platform, TelemetryApp, TelemetryError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(super) const RECORD_PREFIX: &str = "MSIME-CRASH/1 ";

/// 构建与本地采集归属，不含进程路径、设备信息或用户内容；采集前固定，升级补报不重建。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrashContext {
    pub platform: String,
    pub version: String,
    pub revision: String,
    pub run: String,
    pub attempt: String,
    pub arch: String,
    pub component: String,
    /// 仅在本机记录头中保存；入队消息剥离它，不能上传持续同意的身份。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_epoch: Option<Uuid>,
}

impl CrashContext {
    pub(super) fn new(app: &TelemetryApp, component: &str) -> Result<Self, TelemetryError> {
        let context = Self {
            platform: app.platform().to_owned(),
            version: app.version().to_owned(),
            revision: env!("MSIME_CRASH_REVISION").to_owned(),
            run: env!("MSIME_CRASH_RUN").to_owned(),
            attempt: env!("MSIME_CRASH_ATTEMPT").to_owned(),
            arch: env!("MSIME_CRASH_ARCH").to_owned(),
            component: component.to_owned(),
            consent_epoch: None,
        };
        context
            .is_valid()
            .then_some(context)
            .ok_or(TelemetryError::Invalid)
    }

    pub(super) fn is_valid(&self) -> bool {
        let token = |value: &str| {
            value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        };
        canonical_platform(&self.platform) == Some(self.platform.as_str())
            && TelemetryApp::new(&self.platform, &self.version).is_ok()
            && token(&self.version)
            && [
                &self.revision,
                &self.run,
                &self.attempt,
                &self.arch,
                &self.component,
            ]
            .into_iter()
            .all(|value| token(value))
            && !self.arch.is_empty()
            && !self.component.is_empty()
            && self.consent_epoch.is_none_or(|epoch| {
                epoch.get_version() == Some(uuid::Version::Random)
                    && epoch.get_variant() == uuid::Variant::RFC4122
            })
    }

    pub fn record_header(&self) -> String {
        format!(
            "{RECORD_PREFIX}{}\n",
            serde_json::to_string(self).expect("crash identity serializes")
        )
    }

    pub fn from_header(header: &str) -> Option<Self> {
        if header.len() > 1024 {
            return None;
        }
        let context: Self =
            serde_json::from_str(header.trim_end_matches('\n').strip_prefix(RECORD_PREFIX)?)
                .ok()?;
        context.is_valid().then_some(context)
    }

    pub(super) fn message(&self, summary: &str) -> String {
        let mut public_context = self.clone();
        public_context.consent_epoch = None;
        // 后台用消息第一行和调用栈首个业务帧聚合；把身份放在消息第二行，不能让元数据变成首帧。
        let metadata = format!(
            "\n[msime-crash-v1] {}",
            serde_json::to_string(&public_context).expect("crash identity serializes")
        );
        let available = super::MAX_MESSAGE_CHARS.saturating_sub(metadata.chars().count());
        summary
            .chars()
            .take(available)
            .chain(metadata.chars())
            .collect()
    }
}
