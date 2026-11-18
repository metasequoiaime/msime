//! 触屏双拼选择的目录和偏好事务；宿主只显示目录并保存共享层给出的偏好。

use crate::edition::Edition;
use crate::preferences::{
    ChineseScheme, InputScheme, Preferences, ShuangpinProfile, TouchKeyboardLayout,
    TouchKeyboardScheme,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TouchShuangpinOption {
    pub profile: ShuangpinProfile,
    pub title: &'static str,
    pub touch_scheme: TouchKeyboardScheme,
}

/// 顺序沿用移动端已有四套方案，追加新方案，不改变默认启用集合。
pub fn catalog(edition: &Edition) -> Vec<TouchShuangpinOption> {
    use ShuangpinProfile as P;
    use TouchKeyboardScheme as T;
    [
        (P::Xiaohe, T::Xiaohe, "小鹤双拼"),
        (P::Ziranma, T::Ziranma, "自然码双拼"),
        (P::Microsoft, T::Microsoft, "微软双拼"),
        (P::Shoudao, T::Shoudao, "首道双拼"),
        (P::Sogou, T::Sogou, "搜狗双拼"),
        (P::Jiajia, T::Jiajia, "加加双拼"),
        (P::Ziguang, T::Ziguang, "紫光双拼"),
        (P::SmartAbc, T::SmartAbc, "智能 ABC 双拼"),
        (P::Guobiao, T::Guobiao, "国标双拼"),
    ]
    .into_iter()
    .filter(|(_, scheme, _)| edition.offers_touch_scheme(*scheme))
    .map(|(profile, touch_scheme, title)| TouchShuangpinOption {
        profile,
        title,
        touch_scheme,
    })
    .collect()
}

/// 显式选择才启用目标入口；保留其他语言、布局偏好和自定义表。
pub fn select(
    preferences: &mut Preferences,
    profile: ShuangpinProfile,
    edition: &Edition,
) -> Result<(), &'static str> {
    let option = catalog(edition)
        .into_iter()
        .find(|option| option.profile == profile)
        .ok_or("touch_shuangpin_unavailable")?;
    preferences.scheme = InputScheme::Shuangpin;
    preferences.last_chinese_scheme = Some(ChineseScheme::Shuangpin);
    preferences.shuangpin_profile = profile;
    preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
    preferences
        .touch_keyboard_schemes
        .enabled
        .insert(option.touch_scheme);
    preferences.touch_keyboard_schemes.selected = Some(option.touch_scheme);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_each_new_profile_preserves_other_preferences() {
        for profile in [
            ShuangpinProfile::Sogou,
            ShuangpinProfile::Jiajia,
            ShuangpinProfile::Ziguang,
            ShuangpinProfile::SmartAbc,
            ShuangpinProfile::Guobiao,
        ] {
            let mut current = Preferences {
                touch_keyboard_layout: TouchKeyboardLayout::NineKey,
                candidate_page_size: 7,
                ..Preferences::default()
            };
            let before = current.clone();
            let expected = catalog(Edition::full())
                .into_iter()
                .find(|item| item.profile == profile)
                .unwrap();
            select(&mut current, profile, Edition::full()).unwrap();
            assert_eq!(current.scheme, InputScheme::Shuangpin);
            assert_eq!(current.last_chinese_scheme, Some(ChineseScheme::Shuangpin));
            assert_eq!(current.shuangpin_profile, profile);
            assert_eq!(
                current.touch_keyboard_layout,
                TouchKeyboardLayout::TwentySixKey
            );
            assert_eq!(
                current.touch_keyboard_schemes.selected,
                Some(expected.touch_scheme)
            );
            let mut enabled = before.touch_keyboard_schemes.enabled.clone();
            enabled.insert(expected.touch_scheme);
            assert_eq!(current.touch_keyboard_schemes.enabled, enabled);
            current.scheme = before.scheme;
            current.last_chinese_scheme = before.last_chinese_scheme;
            current.shuangpin_profile = before.shuangpin_profile;
            current.touch_keyboard_layout = before.touch_keyboard_layout;
            current.touch_keyboard_schemes = before.touch_keyboard_schemes.clone();
            assert_eq!(current, before);
        }
    }

    #[test]
    fn reading_catalog_never_enables_new_profiles() {
        let before = Preferences::default();
        assert_eq!(catalog(Edition::full()).len(), 9);
        assert_eq!(Preferences::default(), before);
        for scheme in [
            TouchKeyboardScheme::Sogou,
            TouchKeyboardScheme::Jiajia,
            TouchKeyboardScheme::Ziguang,
            TouchKeyboardScheme::SmartAbc,
            TouchKeyboardScheme::Guobiao,
        ] {
            assert!(!before.touch_keyboard_schemes.enabled.contains(&scheme));
        }
    }

    #[test]
    fn unavailable_selection_leaves_preferences_unchanged() {
        let mut current = Preferences::default();
        let before = current.clone();
        let edition = Edition::by_id("wubi").unwrap();
        assert!(catalog(edition).is_empty());
        assert!(select(&mut current, ShuangpinProfile::Sogou, edition).is_err());
        assert!(select(&mut current, ShuangpinProfile::Custom, Edition::full()).is_err());
        assert_eq!(current, before);
    }
}
