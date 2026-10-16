//! 插件与名单相关的失败，按「插件」设置页的叫法命名，由所有提供该页面的宿主共用：桌面外壳用它们应答命令，host-api 的 `msime_client_plugins` 在应答里放入同样的 `code` 和 `detail`，因此无论页面运行在哪个宿主里，解析的都是同一套词汇。

use serde::Serialize;
use std::path::Path;

use super::mentions::MentionError;
use super::{PluginError, PluginKind};

/// 页面能够说明的一种失败：`code` 决定显示哪句话，`detail` 是 client-services 对被拒绝的包或名字报告的规则，让用户知道该修哪个文件或哪一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginFailure {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl PluginFailure {
    /// 除 `code` 外无需补充的失败：`invalid` 表示页面不可能有意发出的请求，`storage` 表示底层磁盘出错。
    pub fn code(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    fn with_detail(code: &'static str, detail: String) -> Self {
        Self {
            code,
            detail: Some(detail),
        }
    }
}

impl From<PluginError> for PluginFailure {
    fn from(value: PluginError) -> Self {
        match value {
            PluginError::Invalid(reason) => Self::with_detail("plugin_invalid", reason),
            PluginError::UnsupportedSource => Self::code("plugin_unsupported_source"),
            PluginError::Archive(reason) => Self::with_detail("plugin_archive", reason),
            PluginError::Reserved => Self::code("plugin_reserved"),
            PluginError::Storage => Self::code("plugin_storage"),
            PluginError::Io(_) => Self::code("storage"),
        }
    }
}

impl From<MentionError> for PluginFailure {
    fn from(value: MentionError) -> Self {
        match value {
            MentionError::Invalid(reason) => Self::with_detail("mention_invalid", reason),
            MentionError::Format => Self::code("mention_format"),
            MentionError::Storage => Self::code("mention_storage"),
            MentionError::Io(_) => Self::code("storage"),
        }
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl From<super::script_grants::ScriptGrantError> for PluginFailure {
    fn from(value: super::script_grants::ScriptGrantError) -> Self {
        use super::script_grants::ScriptGrantError;
        match value {
            ScriptGrantError::Missing(reason) => Self::with_detail("plugin_script_missing", reason),
            ScriptGrantError::Changed => Self::code("plugin_script_changed"),
            ScriptGrantError::Invalid(reason) => {
                Self::with_detail("plugin_script_grant_invalid", reason)
            }
            ScriptGrantError::Limit => Self::code("plugin_script_limit"),
            ScriptGrantError::Memory => Self::code("plugin_script_memory"),
            ScriptGrantError::Format => Self::code("plugin_script_grants_format"),
            ScriptGrantError::Storage => Self::code("plugin_storage"),
            ScriptGrantError::Io(_) => Self::code("storage"),
        }
    }
}

/// 按页面的称呼方式删除一个已安装的包：种类按 `PluginKind::as_str` 的拼法，再加上 id。未知种类返回 `invalid`，其余错误由 `remove` 决定。
pub fn remove_named(root: &Path, kind: &str, id: &str) -> Result<(), PluginFailure> {
    let kind = PluginKind::parse(kind).ok_or(PluginFailure::code("invalid"))?;
    Ok(super::remove(root, kind, id)?)
}
