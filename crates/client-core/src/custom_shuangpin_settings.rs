//! 自定义双拼管理的偏好事务；码表内容由共享引擎在调用前校验。

use crate::edition::Edition;
use crate::preferences::{
    ChineseScheme, InputScheme, Preferences, ShuangpinCustomProfile, ShuangpinProfile,
    TouchKeyboardLayout, TouchKeyboardScheme,
};

/// 用户明确使用自定义表时，同时选中共享的小鹤触屏入口，实际码表名最后写入。
pub fn select(
    preferences: &mut Preferences,
    table: ShuangpinCustomProfile,
    edition: &Edition,
) -> Result<(), &'static str> {
    if !edition.offers_touch_scheme(TouchKeyboardScheme::Xiaohe) {
        return Err("custom_shuangpin_unavailable");
    }
    preferences.scheme = InputScheme::Shuangpin;
    preferences.last_chinese_scheme = Some(ChineseScheme::Shuangpin);
    preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
    preferences
        .touch_keyboard_schemes
        .enabled
        .insert(TouchKeyboardScheme::Xiaohe);
    preferences.touch_keyboard_schemes.selected = Some(TouchKeyboardScheme::Xiaohe);
    preferences.shuangpin_profile = ShuangpinProfile::Custom;
    preferences.shuangpin_custom_profile = table;
    Ok(())
}

/// 删除只清空普通双拼表；仅正在使用自定义时回退小鹤，不动输入入口、九键表或词库。
pub fn remove(preferences: &mut Preferences) {
    preferences.shuangpin_custom_profile = ShuangpinCustomProfile::default();
    if preferences.shuangpin_profile == ShuangpinProfile::Custom {
        preferences.shuangpin_profile = ShuangpinProfile::Xiaohe;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_custom_updates_the_whole_touch_selection_and_preserves_other_values() {
        let mut preferences = Preferences {
            candidate_page_size: 7,
            touch_keyboard_layout: TouchKeyboardLayout::NineKey,
            ..Preferences::default()
        };
        let original = preferences.clone();
        let table = ShuangpinCustomProfile {
            initials: [("zh".into(), "v".into())].into(),
            ..ShuangpinCustomProfile::default()
        };
        select(&mut preferences, table.clone(), Edition::full()).unwrap();
        assert_eq!(preferences.scheme, InputScheme::Shuangpin);
        assert_eq!(
            preferences.last_chinese_scheme,
            Some(ChineseScheme::Shuangpin)
        );
        assert_eq!(preferences.shuangpin_profile, ShuangpinProfile::Custom);
        assert_eq!(preferences.shuangpin_custom_profile, table);
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::TwentySixKey
        );
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::Xiaohe)
        );
        let mut enabled = original.touch_keyboard_schemes.enabled.clone();
        enabled.insert(TouchKeyboardScheme::Xiaohe);
        assert_eq!(preferences.touch_keyboard_schemes.enabled, enabled);
        preferences.scheme = original.scheme;
        preferences.last_chinese_scheme = original.last_chinese_scheme;
        preferences.shuangpin_profile = original.shuangpin_profile;
        preferences.shuangpin_custom_profile = original.shuangpin_custom_profile.clone();
        preferences.touch_keyboard_layout = original.touch_keyboard_layout;
        preferences.touch_keyboard_schemes = original.touch_keyboard_schemes.clone();
        assert_eq!(preferences, original);
    }

    #[test]
    fn removing_is_isolated_and_unavailable_editions_cannot_select() {
        for profile in [ShuangpinProfile::Custom, ShuangpinProfile::Ziranma] {
            let mut preferences = Preferences {
                shuangpin_profile: profile,
                shuangpin_custom_profile: ShuangpinCustomProfile {
                    initials: [("zh".into(), "v".into())].into(),
                    ..ShuangpinCustomProfile::default()
                },
                ..Preferences::default()
            };
            let mut expected = preferences.clone();
            expected.shuangpin_custom_profile = ShuangpinCustomProfile::default();
            if profile == ShuangpinProfile::Custom {
                expected.shuangpin_profile = ShuangpinProfile::Xiaohe;
            }
            remove(&mut preferences);
            assert_eq!(preferences, expected);
        }
        let mut preferences = Preferences::default();
        let original = preferences.clone();
        assert!(select(
            &mut preferences,
            ShuangpinCustomProfile::default(),
            Edition::by_id("wubi").unwrap()
        )
        .is_err());
        assert_eq!(preferences, original);
    }
}
