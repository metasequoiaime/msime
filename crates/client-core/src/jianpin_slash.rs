//! 双拼组字末尾按 `/` 转超级简拼（`local_modes.super_jianpin_slash`，#10860）在偏好层的规则：`/` 已经被谁占用。引擎按同一套条件让位（`msime_engine` 的 `super_jianpin_slash_ready`），原生设置页要给开关置灰、写明占用者时调用这里，不在页面里另写一遍（还没有页面接入，见 `.agents/notes/implemented/feature/2026-11-05-shuangpin-postfix-super-jianpin.md`）。

use crate::preferences::Preferences;

/// `/` 已经归别的功能用，后置简拼让位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlashConflict {
    /// 双拼辅助码开着万象式，且用 `/` 结束四码（`shuangpin_helpcode.modes.direct_helpcode` 与 `direct_helpcode_slash`）。
    DirectHelpcodeSlash,
    /// 双拼辅助码的前导键是 `/`（`shuangpin_helpcode.trigger_key`）。
    HelpcodeTriggerKey,
    /// 中文标点下 `/` 出顿号（`enumeration_comma_slash`）。
    EnumerationComma,
}

/// `/` 此刻被谁占用；`None` 表示后置简拼能用 `/`。只看 `/` 的占用，不看 `super_jianpin` 和本开关自己是否打开。
pub fn super_jianpin_slash_conflict(preferences: &Preferences) -> Option<SlashConflict> {
    let helpcode = &preferences.shuangpin_helpcode;
    if helpcode.modes.direct_helpcode && helpcode.modes.direct_helpcode_slash {
        return Some(SlashConflict::DirectHelpcodeSlash);
    }
    if helpcode.trigger_key == "/" {
        return Some(SlashConflict::HelpcodeTriggerKey);
    }
    if crate::punctuation::punctuation_marks(preferences).enumeration_comma_slash {
        return Some(SlashConflict::EnumerationComma);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::preferences::LocalModePreferences;

    #[test]
    fn the_switch_defaults_on_and_is_written_only_when_off() {
        assert!(LocalModePreferences::default().super_jianpin_slash);
        let written = serde_json::to_value(Preferences::default()).unwrap();
        assert!(written["local_modes"].get("super_jianpin_slash").is_none());

        // 早于这个开关的文档没有这个键，读出来是开，其余模式开关照原样。
        let mut older = serde_json::to_value(LocalModePreferences::default()).unwrap();
        older["emoji"] = false.into();
        let loaded: LocalModePreferences = serde_json::from_value(older).unwrap();
        assert!(loaded.super_jianpin_slash && !loaded.emoji && loaded.super_jianpin);

        // 关掉后文档里写 false，读回来还是关。
        let off = LocalModePreferences {
            super_jianpin_slash: false,
            ..LocalModePreferences::default()
        };
        let written = serde_json::to_value(off).unwrap();
        assert_eq!(written["super_jianpin_slash"], false);
        let loaded: LocalModePreferences = serde_json::from_value(written).unwrap();
        assert_eq!(loaded, off);
    }

    #[test]
    fn factory_preferences_leave_the_slash_free() {
        assert_eq!(super_jianpin_slash_conflict(&Preferences::default()), None);
    }

    #[test]
    fn each_owner_of_the_slash_is_reported() {
        let mut direct = Preferences::default();
        direct.shuangpin_helpcode.modes.direct_helpcode = true;
        assert_eq!(
            super_jianpin_slash_conflict(&direct),
            Some(SlashConflict::DirectHelpcodeSlash)
        );
        // 万象式开着但四码改用大写结束时，`/` 还是空着的。
        direct.shuangpin_helpcode.modes.direct_helpcode_slash = false;
        assert_eq!(super_jianpin_slash_conflict(&direct), None);

        let mut trigger = Preferences::default();
        trigger.shuangpin_helpcode.trigger_key = "/".to_owned();
        assert_eq!(
            super_jianpin_slash_conflict(&trigger),
            Some(SlashConflict::HelpcodeTriggerKey)
        );
        // 全拼辅助码的前导键不管双拼。
        let mut quanpin = Preferences::default();
        quanpin.quanpin_helpcode.trigger_key = "/".to_owned();
        assert_eq!(super_jianpin_slash_conflict(&quanpin), None);

        let comma = Preferences {
            enumeration_comma_slash: true,
            ..Preferences::default()
        };
        assert_eq!(
            super_jianpin_slash_conflict(&comma),
            Some(SlashConflict::EnumerationComma)
        );
    }
}
