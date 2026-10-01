//! Plugin and name-list failures as the 插件 settings page names them, shared by every host that serves the page: the desktop shell answers its commands with these, and host-api's `msime_client_plugins` puts the same code and detail in its reply, so the page decodes one vocabulary whichever host it runs in.

use serde::Serialize;
use std::path::Path;

use super::mentions::MentionError;
use super::{PluginError, PluginKind};

/// A failure the page can name: `code` picks the sentence, `detail` is the rule client-core reports for a refused pack or name, so the user learns which file or entry to fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginFailure {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl PluginFailure {
    /// A failure with nothing to add to its code: `invalid` for a request the page could not have meant, `storage` for a disk that failed underneath.
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

/// Remove an installed pack named the way the page names it: the kind as `PluginKind::as_str` spells it, and the id. An unknown kind is `invalid`; the rest is `remove`'s.
pub fn remove_named(root: &Path, kind: &str, id: &str) -> Result<(), PluginFailure> {
    let kind = PluginKind::parse(kind).ok_or(PluginFailure::code("invalid"))?;
    Ok(super::remove(root, kind, id)?)
}
