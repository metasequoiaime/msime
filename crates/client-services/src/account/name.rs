//! 昵称草稿的规范化、后端校验与显示回退；不访问账号、文件或网络。

use serde::Serialize;

pub const MAX_DISPLAY_NAME_CHARS: usize = 64;

#[derive(Serialize)]
pub struct NamePresentation {
    pub normalized: String,
    pub preferred: String,
    pub valid: bool,
    pub maximum_chars: usize,
    /// 给按 UTF-16 单元限制长度的原生控件预留空间，实际合法性仍按码点校验。
    pub maximum_utf16_units: usize,
}

/// 保留原设置页的去空白方式，保存前的合法性直接复用账号请求的校验。
pub fn present_name(value: &str, id: &str) -> NamePresentation {
    let normalized = msime_client_base::text::trim_ui_whitespace(value);
    let preferred = if normalized.is_empty() {
        let short_id: String = id.chars().take(6).collect();
        format!("水杉小鹿·{}", short_id.to_ascii_uppercase())
    } else {
        normalized.to_owned()
    };
    NamePresentation {
        normalized: normalized.to_owned(),
        preferred,
        valid: super::validate::validate_display_name(normalized).is_ok(),
        maximum_chars: MAX_DISPLAY_NAME_CHARS,
        maximum_utf16_units: MAX_DISPLAY_NAME_CHARS * 2,
    }
}
