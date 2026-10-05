//! The Android global theme keys of the account preferences document, kept free of the plugin and mobile types so host tests compile and cover them.

use msime_client_core::account::{AccountError, AccountPreferenceValue};
use msime_client_core::preferences::Preferences;
use msime_client_core::skin::theme::GlobalTheme;
use std::collections::BTreeMap;

const GLOBAL_THEME: &str = "platform.android.global_theme";
const CUSTOM_THEME_BASE: &str = "platform.android.custom_theme_base";
const CUSTOM_KEYBOARD_SKIN: &str = "platform.android.custom_keyboard_skin";
const CUSTOM_CANDIDATE_SKIN: &str = "platform.android.custom_candidate_skin";

fn insert_string(settings: &mut BTreeMap<String, AccountPreferenceValue>, key: &str, value: &str) {
    settings.insert(
        key.to_owned(),
        AccountPreferenceValue::String(value.to_owned()),
    );
}

/// A string value, or `None` when the key is absent or holds another type (the same reading `android_account` gives every string key).
fn string_setting<'a>(
    settings: &'a BTreeMap<String, AccountPreferenceValue>,
    key: &str,
) -> Option<&'a str> {
    match settings.get(key) {
        Some(AccountPreferenceValue::String(value)) => Some(value),
        _ => None,
    }
}

/// Writes the global theme, the custom theme's base, its keyboard design and its external candidate package. The design is JSON and the package an id; for both an empty string is "none", so clearing either syncs as well.
pub(crate) fn insert_theme_settings(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    preferences: &Preferences,
) -> Result<(), AccountError> {
    insert_string(settings, GLOBAL_THEME, preferences.global_theme.id());
    insert_string(
        settings,
        CUSTOM_THEME_BASE,
        preferences.custom_theme.base.id(),
    );
    let custom_skin = match &preferences.custom_theme.keyboard {
        Some(design) => serde_json::to_string(design).map_err(|_| AccountError::Invalid)?,
        None => String::new(),
    };
    insert_string(settings, CUSTOM_KEYBOARD_SKIN, &custom_skin);
    insert_string(
        settings,
        CUSTOM_CANDIDATE_SKIN,
        preferences
            .custom_theme
            .candidate_skin
            .as_deref()
            .unwrap_or_default(),
    );
    Ok(())
}

/// Reads back what `insert_theme_settings` writes, for every key the schema supports (`supports(key, "string")`). An unknown theme id, a base that is not a base theme, an unreadable design or a package id that is not an external id is refused.
pub(crate) fn apply_theme_settings(
    preferences: &mut Preferences,
    values: &BTreeMap<String, AccountPreferenceValue>,
    supports: impl Fn(&str, &str) -> Result<bool, AccountError>,
) -> Result<(), AccountError> {
    if let Some(value) = string_setting(values, GLOBAL_THEME) {
        if supports(GLOBAL_THEME, "string")? {
            preferences.global_theme = GlobalTheme::from_id(value).ok_or(AccountError::Invalid)?;
        }
    }
    if let Some(value) = string_setting(values, CUSTOM_THEME_BASE) {
        if supports(CUSTOM_THEME_BASE, "string")? {
            preferences.custom_theme.base = GlobalTheme::from_id(value)
                .filter(|base| base.is_base())
                .ok_or(AccountError::Invalid)?;
        }
    }
    if let Some(value) = string_setting(values, CUSTOM_KEYBOARD_SKIN) {
        if supports(CUSTOM_KEYBOARD_SKIN, "string")? {
            preferences.custom_theme.keyboard = if value.is_empty() {
                None
            } else {
                Some(serde_json::from_str(value).map_err(|_| AccountError::Invalid)?)
            };
        }
    }
    if let Some(value) = string_setting(values, CUSTOM_CANDIDATE_SKIN) {
        if supports(CUSTOM_CANDIDATE_SKIN, "string")? {
            if !value.is_empty() && !msime_client_core::skin::catalog::is_selectable_id(value) {
                return Err(AccountError::Invalid);
            }
            preferences.custom_theme.candidate_skin = (!value.is_empty()).then(|| value.to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::preferences::TouchKeyboardSkinDesign;

    fn supported(_: &str, expected: &str) -> Result<bool, AccountError> {
        Ok(expected == "string")
    }

    fn round_trip(preferences: &Preferences) -> Preferences {
        let mut settings = BTreeMap::new();
        insert_theme_settings(&mut settings, preferences).unwrap();
        let mut applied = Preferences::default();
        apply_theme_settings(&mut applied, &settings, supported).unwrap();
        applied
    }

    fn applying(key: &str, value: &str) -> Result<Preferences, AccountError> {
        let mut settings = BTreeMap::new();
        insert_theme_settings(&mut settings, &Preferences::default()).unwrap();
        insert_string(&mut settings, key, value);
        let mut applied = Preferences::default();
        apply_theme_settings(&mut applied, &settings, supported).map(|()| applied)
    }

    #[test]
    fn an_absent_keyboard_design_and_package_sync_as_empty_strings_and_read_back_as_none() {
        let mut preferences = Preferences::default();
        preferences.global_theme = GlobalTheme::Custom;
        preferences.custom_theme.base = GlobalTheme::Night;
        let mut settings = BTreeMap::new();
        insert_theme_settings(&mut settings, &preferences).unwrap();
        assert_eq!(
            settings[GLOBAL_THEME],
            AccountPreferenceValue::String("custom".into())
        );
        assert_eq!(
            settings[CUSTOM_THEME_BASE],
            AccountPreferenceValue::String("night".into())
        );
        assert_eq!(
            settings[CUSTOM_KEYBOARD_SKIN],
            AccountPreferenceValue::String(String::new())
        );
        assert_eq!(
            settings[CUSTOM_CANDIDATE_SKIN],
            AccountPreferenceValue::String(String::new())
        );

        // A stale design and package on the device are cleared by the synced "none".
        let mut applied = Preferences::default();
        applied.custom_theme.keyboard = Some(TouchKeyboardSkinDesign::default());
        applied.custom_theme.candidate_skin = Some("sakura".into());
        apply_theme_settings(&mut applied, &settings, supported).unwrap();
        assert_eq!(applied.global_theme, GlobalTheme::Custom);
        assert_eq!(applied.custom_theme.base, GlobalTheme::Night);
        assert_eq!(applied.custom_theme.keyboard, None);
        assert_eq!(applied.custom_theme.candidate_skin, None);
    }

    #[test]
    fn a_keyboard_design_and_package_survive_the_round_trip() {
        let mut preferences = Preferences::default();
        preferences.global_theme = GlobalTheme::Custom;
        preferences.custom_theme.base = GlobalTheme::Paper;
        preferences.custom_theme.keyboard = Some(TouchKeyboardSkinDesign {
            background: 0x123456,
            ..TouchKeyboardSkinDesign::default()
        });
        preferences.custom_theme.candidate_skin = Some("sakura".into());
        let applied = round_trip(&preferences);
        assert_eq!(applied.global_theme, preferences.global_theme);
        assert_eq!(applied.custom_theme, preferences.custom_theme);
    }

    /// 水杉四季与四个季节主题照样同步：作为全局主题，也作为自定义主题的底。
    #[test]
    fn the_seasonal_themes_survive_the_round_trip() {
        for theme in [
            GlobalTheme::Siji,
            GlobalTheme::Chunya,
            GlobalTheme::Xiayin,
            GlobalTheme::Qiushan,
            GlobalTheme::Dongxue,
        ] {
            let mut preferences = Preferences::default();
            preferences.global_theme = theme;
            assert_eq!(round_trip(&preferences).global_theme, theme);
            preferences.global_theme = GlobalTheme::Custom;
            preferences.custom_theme.base = theme;
            assert_eq!(round_trip(&preferences).custom_theme.base, theme);
        }
        // 四季主题加入之前同步上去的同名皮肤选择仍然是合法值。
        assert_eq!(
            applying(CUSTOM_CANDIDATE_SKIN, "qiushan")
                .unwrap()
                .custom_theme
                .candidate_skin
                .as_deref(),
            Some("qiushan")
        );
    }

    #[test]
    fn unknown_themes_non_base_bases_and_non_external_packages_are_refused() {
        for (key, value) in [
            (GLOBAL_THEME, "fluent"),
            (GLOBAL_THEME, "Night"),
            (CUSTOM_THEME_BASE, "custom"),
            (CUSTOM_THEME_BASE, "willow_green"),
            (CUSTOM_KEYBOARD_SKIN, "not json"),
            (CUSTOM_CANDIDATE_SKIN, "night"),
            (CUSTOM_CANDIDATE_SKIN, "../sakura"),
        ] {
            assert!(
                matches!(applying(key, value), Err(AccountError::Invalid)),
                "{key} = {value}"
            );
        }
    }

    #[test]
    fn keys_the_schema_does_not_support_are_left_alone() {
        let mut preferences = Preferences::default();
        preferences.global_theme = GlobalTheme::Ink;
        preferences.custom_theme.candidate_skin = Some("sakura".into());
        let mut settings = BTreeMap::new();
        insert_theme_settings(&mut settings, &preferences).unwrap();
        let mut applied = Preferences::default();
        apply_theme_settings(&mut applied, &settings, |_, _| Ok(false)).unwrap();
        assert_eq!(applied, Preferences::default());
    }
}
