//! 候选选中文字色的纯更新规则，宿主只应用补丁并沿用偏好保存流程。

use super::{CustomTheme, PreferencesError};
use crate::theme::GlobalTheme;
use serde::Serialize;

/// 只包含此次操作需要改写的字段，不替换宿主刚读到的完整文档。
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SelectedTextUpdate {
    pub selected_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_theme: Option<GlobalTheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<GlobalTheme>,
    pub reset_skins: bool,
}

/// 计算选色或交还主题的补丁，保存和并发控制仍由调用方沿用原流程。
pub fn selected_text_update(
    current: GlobalTheme,
    value: Option<&str>,
) -> Result<SelectedTextUpdate, PreferencesError> {
    let selected_text = match value {
        Some(value) => {
            let mut custom = CustomTheme::default();
            custom.candidate_colors.selected_text = Some(value.to_owned());
            custom.validate()?;
            Some(value.to_ascii_uppercase())
        }
        None => None,
    };
    let selecting = selected_text.is_some();
    let reset_skins = selecting && current != GlobalTheme::Custom;
    Ok(SelectedTextUpdate {
        selected_text,
        global_theme: selecting.then_some(GlobalTheme::Custom),
        base: reset_skins.then_some(if current == GlobalTheme::Native {
            GlobalTheme::System
        } else {
            current
        }),
        reset_skins,
    })
}

#[cfg(test)]
mod tests {
    use super::selected_text_update;
    use crate::theme::GlobalTheme;

    #[test]
    fn selecting_a_colour_preserves_the_previous_theme_as_base_and_resets_both_skin_slots() {
        for current in GlobalTheme::ALL {
            let update = selected_text_update(current, Some("#7b3fbf")).unwrap();
            assert_eq!(update.selected_text.as_deref(), Some("#7B3FBF"));
            assert_eq!(update.global_theme, Some(GlobalTheme::Custom));
            assert_eq!(update.reset_skins, current != GlobalTheme::Custom);
            assert_eq!(
                update.base,
                match current {
                    GlobalTheme::Custom => None,
                    GlobalTheme::Native => Some(GlobalTheme::System),
                    other => Some(other),
                }
            );
        }
    }

    #[test]
    fn clearing_only_removes_the_picked_colour() {
        for current in GlobalTheme::ALL {
            let update = selected_text_update(current, None).unwrap();
            assert_eq!(update.selected_text, None);
            assert_eq!(update.global_theme, None);
            assert_eq!(update.base, None);
            assert!(!update.reset_skins);
        }
    }

    #[test]
    fn invalid_colours_are_rejected_before_a_patch_is_returned() {
        for invalid in ["", "red", "#12345", "#12345678", "#GG0000", " #123456"] {
            assert!(selected_text_update(GlobalTheme::Night, Some(invalid)).is_err());
        }
    }
}
