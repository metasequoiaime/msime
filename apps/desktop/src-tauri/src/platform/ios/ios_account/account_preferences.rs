use crate::platform::mobile::mobile_account_helpers::valid_mobile_haptic_strength;
use crate::platform::mobile::mobile_account_preferences::{
    frequency_account_preferences, insert_bool, insert_string,
};
use msime_client_core::account::{
    validate_account_preferences, AccountError, AccountPreferenceValue, AccountPreferences,
};
use msime_client_core::preferences::{
    ChineseScheme, FrequencyMode, InputScheme, Preferences, ShuangpinProfile, TouchKeyboardLayout,
    TouchKeyboardScheme, TouchKeyboardSkinDesign,
};
use msime_client_core::skin::theme::GlobalTheme;
use msime_tauri_mobile_platform::IosKeyboardPreferences;
use std::collections::BTreeMap;

fn decoded_custom_skin(
    value: Option<&str>,
    fallback: Option<&TouchKeyboardSkinDesign>,
) -> Option<TouchKeyboardSkinDesign> {
    value
        .and_then(|value| serde_json::from_str::<TouchKeyboardSkinDesign>(value).ok())
        .map(TouchKeyboardSkinDesign::normalized)
        .or_else(|| fallback.cloned())
}

pub(crate) fn local_account_preferences(
    native: &IosKeyboardPreferences,
    shared: &Preferences,
    fallback_custom_skin: Option<&TouchKeyboardSkinDesign>,
) -> Result<BTreeMap<String, AccountPreferenceValue>, AccountError> {
    if !native.is_valid() {
        return Err(AccountError::Storage);
    }
    let mut settings = BTreeMap::new();
    // The cloud `input.schema` cannot carry Cantonese, Zhuyin or Vietnamese (an older device would refuse the whole document), so those leave the scheme and the nine-key switch out and the cloud keeps what it has.
    let scheme = match native.input_scheme.as_str() {
        "quanpin" | "handwriting" | "thoughtfulReply" => Some(("quanpin", None, false)),
        "nineKey" => Some(("quanpin", None, true)),
        "shuangpin" => Some(("shuangpin", Some("xiaohe"), false)),
        "ziranma" => Some(("shuangpin", Some("ziranma"), false)),
        "microsoft" => Some(("shuangpin", Some("microsoft"), false)),
        "shoudao" => Some(("shuangpin", Some("shoudao"), false)),
        "wubi" => Some(("wubi", None, false)),
        "japanese" => Some(("japanese", None, false)),
        "japaneseNineKey" => Some(("japanese", None, true)),
        "korean" => Some(("korean", None, false)),
        "cantonese" | "zhuyin" | "vietnamese" => None,
        _ => return Err(AccountError::Storage),
    };
    if let Some((schema, profile, _)) = scheme {
        insert_string(&mut settings, "input.schema", schema);
        if let Some(profile) = profile {
            insert_string(&mut settings, "input.shuangpin_schema", profile);
        }
    }
    insert_string(
        &mut settings,
        "input.character_set",
        if native.traditional_chinese_output {
            "traditional"
        } else {
            "simplified"
        },
    );
    if let Some((_, _, nine_key)) = scheme {
        insert_bool(&mut settings, "platform.ios.nine_key", nine_key);
    }
    insert_bool(
        &mut settings,
        "platform.ios.sound_enabled",
        native.sound_enabled,
    );
    insert_bool(
        &mut settings,
        "platform.ios.haptics_enabled",
        native.haptics_enabled,
    );
    insert_string(
        &mut settings,
        "platform.ios.haptic_strength",
        &native.haptic_strength,
    );
    insert_bool(
        &mut settings,
        "platform.ios.dictionary_learning",
        shared.learning,
    );
    settings.extend(frequency_account_preferences(&shared.frequency));
    insert_string(
        &mut settings,
        "platform.ios.global_theme",
        &native.global_theme,
    );
    insert_string(
        &mut settings,
        "platform.ios.custom_theme_base",
        shared.custom_theme.base.id(),
    );
    // A custom theme without a keyboard design draws its base's keyboard; there is no design to upload then, and the cloud keeps whatever design it has.
    if let Some(custom) =
        decoded_custom_skin(native.custom_keyboard_skin.as_deref(), fallback_custom_skin)
    {
        let custom = serde_json::to_string(&custom).map_err(|_| AccountError::Invalid)?;
        insert_string(&mut settings, "platform.ios.custom_keyboard_skin", &custom);
    }
    Ok(settings)
}

fn string_setting(
    settings: &BTreeMap<String, AccountPreferenceValue>,
    key: &str,
) -> Result<Option<String>, AccountError> {
    match settings.get(key) {
        None => Ok(None),
        Some(AccountPreferenceValue::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(AccountError::Invalid),
    }
}

fn bool_setting(
    settings: &BTreeMap<String, AccountPreferenceValue>,
    key: &str,
) -> Result<Option<bool>, AccountError> {
    match settings.get(key) {
        None => Ok(None),
        Some(AccountPreferenceValue::Boolean(value)) => Ok(Some(*value)),
        Some(_) => Err(AccountError::Invalid),
    }
}

fn integer_setting(
    settings: &BTreeMap<String, AccountPreferenceValue>,
    key: &str,
) -> Result<Option<i64>, AccountError> {
    match settings.get(key) {
        None => Ok(None),
        Some(AccountPreferenceValue::Integer(value)) => Ok(Some(*value)),
        Some(AccountPreferenceValue::Number(value))
            if value.is_finite() && value.fract() == 0.0 =>
        {
            Ok(Some(*value as i64))
        }
        Some(_) => Err(AccountError::Invalid),
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct IosPreferencePlan {
    input_scheme: Option<String>,
    traditional_chinese_output: Option<bool>,
    sound_enabled: Option<bool>,
    haptics_enabled: Option<bool>,
    haptic_strength: Option<String>,
    dictionary_learning: Option<bool>,
    frequency_mode: Option<FrequencyMode>,
    frequency_trigger_count: Option<u8>,
    frequency_linear_step: Option<u8>,
    global_theme: Option<String>,
    custom_theme_base: Option<GlobalTheme>,
    custom_keyboard_skin: Option<TouchKeyboardSkinDesign>,
}

impl IosPreferencePlan {
    pub(crate) fn from_cloud(cloud: &AccountPreferences) -> Result<Self, AccountError> {
        validate_account_preferences(cloud)?;
        let values = &cloud.settings;
        let nine_key = bool_setting(values, "platform.ios.nine_key")? == Some(true);
        let input_scheme = match string_setting(values, "input.schema")?.as_deref() {
            None => None,
            Some("quanpin") => Some(if nine_key { "nineKey" } else { "quanpin" }.into()),
            Some("shuangpin") => {
                let profile = string_setting(values, "input.shuangpin_schema")?
                    .unwrap_or_else(|| "xiaohe".into());
                match profile.as_str() {
                    "xiaohe" => Some("shuangpin".into()),
                    "ziranma" | "microsoft" | "shoudao" => Some(profile),
                    _ => return Err(AccountError::Invalid),
                }
            }
            Some("wubi") => {
                if string_setting(values, "input.wubi_schema")?
                    .as_deref()
                    .unwrap_or("wubi86")
                    != "wubi86"
                {
                    return Err(AccountError::Invalid);
                }
                Some("wubi".into())
            }
            Some("japanese") => {
                if string_setting(values, "input.japanese_schema")?
                    .as_deref()
                    .unwrap_or("romaji")
                    != "romaji"
                {
                    return Err(AccountError::Invalid);
                }
                Some(
                    if nine_key {
                        "japaneseNineKey"
                    } else {
                        "japanese"
                    }
                    .into(),
                )
            }
            Some("korean") => Some("korean".into()),
            // A scheme this host does not offer (a newer device's Cantonese, Zhuyin or Vietnamese) keeps the local one rather than refusing the whole sync, so the rest of the document still applies.
            Some(_) => None,
        };
        let traditional_chinese_output =
            match string_setting(values, "input.character_set")?.as_deref() {
                None => None,
                Some("simplified") => Some(false),
                Some("traditional") => Some(true),
                Some(_) => return Err(AccountError::Invalid),
            };
        let haptic_strength = string_setting(values, "platform.ios.haptic_strength")?;
        if haptic_strength
            .as_deref()
            .is_some_and(|value| !valid_mobile_haptic_strength(value))
        {
            return Err(AccountError::Invalid);
        }
        let global_theme = string_setting(values, "platform.ios.global_theme")?;
        if global_theme
            .as_deref()
            .is_some_and(|value| GlobalTheme::from_id(value).is_none())
        {
            return Err(AccountError::Invalid);
        }
        let custom_theme_base = string_setting(values, "platform.ios.custom_theme_base")?
            .map(|value| {
                GlobalTheme::from_id(&value)
                    .filter(|base| base.is_base())
                    .ok_or(AccountError::Invalid)
            })
            .transpose()?;
        let custom_keyboard_skin = string_setting(values, "platform.ios.custom_keyboard_skin")?
            .map(|value| {
                serde_json::from_str::<TouchKeyboardSkinDesign>(&value)
                    .map(TouchKeyboardSkinDesign::normalized)
                    .map_err(|_| AccountError::Invalid)
            })
            .transpose()?;
        let frequency_mode = string_setting(values, "input.frequency_mode")?
            .map(|value| match value.as_str() {
                "disabled" => Ok(FrequencyMode::Disabled),
                "pin" => Ok(FrequencyMode::Pin),
                "halve" => Ok(FrequencyMode::Halve),
                "linear" => Ok(FrequencyMode::Linear),
                "promote" => Ok(FrequencyMode::Promote),
                _ => Err(AccountError::Invalid),
            })
            .transpose()?;
        let frequency_trigger_count = integer_setting(values, "input.frequency_trigger_count")?
            .map(|value| u8::try_from(value).map_err(|_| AccountError::Invalid))
            .transpose()?;
        let frequency_linear_step = integer_setting(values, "input.frequency_linear_step")?
            .map(|value| u8::try_from(value).map_err(|_| AccountError::Invalid))
            .transpose()?;
        Ok(Self {
            input_scheme,
            traditional_chinese_output,
            sound_enabled: bool_setting(values, "platform.ios.sound_enabled")?,
            haptics_enabled: bool_setting(values, "platform.ios.haptics_enabled")?,
            haptic_strength,
            dictionary_learning: bool_setting(values, "platform.ios.dictionary_learning")?,
            frequency_mode,
            frequency_trigger_count,
            frequency_linear_step,
            global_theme,
            custom_theme_base,
            custom_keyboard_skin,
        })
    }

    pub(crate) fn requested_native(
        &self,
        current: &IosKeyboardPreferences,
    ) -> Result<IosKeyboardPreferences, AccountError> {
        if !current.is_valid() {
            return Err(AccountError::Storage);
        }
        let mut requested = current.clone();
        if let Some(value) = &self.input_scheme {
            requested.input_scheme.clone_from(value);
        }
        if let Some(value) = self.traditional_chinese_output {
            requested.traditional_chinese_output = value;
        }
        if let Some(value) = self.sound_enabled {
            requested.sound_enabled = value;
        }
        if let Some(value) = self.haptics_enabled {
            requested.haptics_enabled = value;
        }
        if let Some(value) = &self.haptic_strength {
            requested.haptic_strength.clone_from(value);
        }
        if let Some(value) = self.dictionary_learning {
            requested.dictionary_learning = value;
        }
        if let Some(value) = &self.global_theme {
            requested.global_theme.clone_from(value);
        }
        if let Some(value) = &self.custom_keyboard_skin {
            requested.custom_keyboard_skin =
                Some(serde_json::to_string(value).map_err(|_| AccountError::Invalid)?);
        }
        requested
            .is_valid()
            .then_some(requested)
            .ok_or(AccountError::Invalid)
    }

    pub(crate) fn apply_shared(
        &self,
        native: &IosKeyboardPreferences,
        preferences: &mut Preferences,
    ) -> Result<(), AccountError> {
        if !native.is_valid() {
            return Err(AccountError::Storage);
        }
        if self.input_scheme.is_some() {
            select_touch_scheme(preferences, touch_scheme(&native.input_scheme)?);
        }
        if self.traditional_chinese_output.is_some() {
            preferences.traditional_chinese_output = native.traditional_chinese_output;
        }
        if self.dictionary_learning.is_some() {
            preferences.learning = native.dictionary_learning;
        }
        if self.global_theme.is_some() {
            preferences.global_theme =
                GlobalTheme::from_id(&native.global_theme).ok_or(AccountError::Invalid)?;
        }
        if let Some(value) = self.custom_theme_base {
            preferences.custom_theme.base = value;
        }
        if let Some(value) = &self.custom_keyboard_skin {
            preferences.custom_theme.keyboard = Some(value.clone());
        }
        if let Some(value) = self.frequency_mode {
            preferences.frequency.mode = value;
        }
        if let Some(value) = self.frequency_trigger_count {
            preferences.frequency.trigger_count = value;
        }
        if let Some(value) = self.frequency_linear_step {
            preferences.frequency.linear_step = value;
        }
        preferences.validate().map_err(|_| AccountError::Invalid)
    }
}

fn touch_scheme(value: &str) -> Result<TouchKeyboardScheme, AccountError> {
    match value {
        "quanpin" => Ok(TouchKeyboardScheme::Quanpin),
        "nineKey" => Ok(TouchKeyboardScheme::NineKey),
        "shuangpin" => Ok(TouchKeyboardScheme::Xiaohe),
        "ziranma" => Ok(TouchKeyboardScheme::Ziranma),
        "microsoft" => Ok(TouchKeyboardScheme::Microsoft),
        "shoudao" => Ok(TouchKeyboardScheme::Shoudao),
        "wubi" => Ok(TouchKeyboardScheme::Wubi),
        "japaneseNineKey" => Ok(TouchKeyboardScheme::JapaneseNineKey),
        "japanese" => Ok(TouchKeyboardScheme::Japanese),
        "handwriting" => Ok(TouchKeyboardScheme::Handwriting),
        "thoughtfulReply" => Ok(TouchKeyboardScheme::ThoughtfulReply),
        "korean" => Ok(TouchKeyboardScheme::Korean),
        "cantonese" => Ok(TouchKeyboardScheme::Cantonese),
        "zhuyin" => Ok(TouchKeyboardScheme::Zhuyin),
        "vietnamese" => Ok(TouchKeyboardScheme::Vietnamese),
        _ => Err(AccountError::Invalid),
    }
}

/// Keep the Chinese scheme a Japanese or Korean selection returns to; switching away from Japanese, Korean or Vietnamese keeps the one already remembered.
fn remember_chinese_scheme(preferences: &mut Preferences) {
    let chinese = match preferences.scheme {
        InputScheme::Quanpin => ChineseScheme::Quanpin,
        InputScheme::Shuangpin => ChineseScheme::Shuangpin,
        InputScheme::Wubi => ChineseScheme::Wubi,
        InputScheme::Cantonese => ChineseScheme::Cantonese,
        InputScheme::Zhuyin => ChineseScheme::Zhuyin,
        InputScheme::Japanese | InputScheme::Korean | InputScheme::Vietnamese => return,
    };
    preferences.last_chinese_scheme = Some(chinese);
}

fn select_touch_scheme(preferences: &mut Preferences, requested: TouchKeyboardScheme) {
    let selected = if preferences
        .touch_keyboard_schemes
        .enabled
        .contains(&requested)
    {
        requested
    } else {
        TouchKeyboardScheme::ALL
            .into_iter()
            .find(|scheme| preferences.touch_keyboard_schemes.enabled.contains(scheme))
            .unwrap_or(TouchKeyboardScheme::Quanpin)
    };
    preferences.touch_keyboard_schemes.selected = Some(selected);
    match selected {
        TouchKeyboardScheme::Xiaohe
        | TouchKeyboardScheme::Ziranma
        | TouchKeyboardScheme::Microsoft
        | TouchKeyboardScheme::Shoudao => {
            preferences.scheme = InputScheme::Shuangpin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Shuangpin);
            preferences.shuangpin_profile = match selected {
                TouchKeyboardScheme::Xiaohe => ShuangpinProfile::Xiaohe,
                TouchKeyboardScheme::Ziranma => ShuangpinProfile::Ziranma,
                TouchKeyboardScheme::Microsoft => ShuangpinProfile::Microsoft,
                TouchKeyboardScheme::Shoudao => ShuangpinProfile::Shoudao,
                _ => unreachable!(),
            };
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Japanese | TouchKeyboardScheme::JapaneseNineKey => {
            remember_chinese_scheme(preferences);
            preferences.scheme = InputScheme::Japanese;
            preferences.touch_keyboard_layout = if selected == TouchKeyboardScheme::JapaneseNineKey
            {
                TouchKeyboardLayout::NineKey
            } else {
                TouchKeyboardLayout::TwentySixKey
            };
        }
        TouchKeyboardScheme::Korean => {
            remember_chinese_scheme(preferences);
            preferences.scheme = InputScheme::Korean;
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Vietnamese => {
            remember_chinese_scheme(preferences);
            preferences.scheme = InputScheme::Vietnamese;
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Cantonese => {
            preferences.scheme = InputScheme::Cantonese;
            preferences.last_chinese_scheme = Some(ChineseScheme::Cantonese);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Zhuyin => {
            preferences.scheme = InputScheme::Zhuyin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Zhuyin);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Wubi => {
            preferences.scheme = InputScheme::Wubi;
            preferences.last_chinese_scheme = Some(ChineseScheme::Wubi);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Quanpin
        | TouchKeyboardScheme::NineKey
        | TouchKeyboardScheme::Handwriting
        | TouchKeyboardScheme::ThoughtfulReply => {
            preferences.scheme = InputScheme::Quanpin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Quanpin);
            preferences.touch_keyboard_layout = match selected {
                TouchKeyboardScheme::NineKey => TouchKeyboardLayout::NineKey,
                TouchKeyboardScheme::Handwriting => TouchKeyboardLayout::Handwriting,
                _ => TouchKeyboardLayout::TwentySixKey,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{local_account_preferences, IosPreferencePlan};
    use msime_client_core::account::{AccountError, AccountPreferenceValue, AccountPreferences};
    use msime_client_core::preferences::{
        InputScheme, Preferences, ShuangpinProfile, TouchKeyboardLayout, TouchKeyboardScheme,
    };
    use msime_client_core::skin::theme::GlobalTheme;
    use msime_tauri_mobile_platform::IosKeyboardPreferences;
    use std::collections::BTreeMap;

    fn native() -> IosKeyboardPreferences {
        IosKeyboardPreferences {
            input_scheme: "japaneseNineKey".into(),
            traditional_chinese_output: true,
            sound_enabled: true,
            haptics_enabled: true,
            haptic_strength: "strong".into(),
            english_suggestions: true,
            candidate_palette_follows_desktop: false,
            inline_preedit: false,
            haptics_available: true,
            tablet_full_keys: None,
            dictionary_learning: false,
            global_theme: "custom".into(),
            custom_keyboard_skin: None,
        }
    }

    #[test]
    fn cantonese_and_zhuyin_are_remembered_and_vietnamese_keeps_the_last_chinese_scheme() {
        use msime_client_core::preferences::ChineseScheme;
        for (scheme, remembered) in [
            (InputScheme::Cantonese, Some(ChineseScheme::Cantonese)),
            (InputScheme::Zhuyin, Some(ChineseScheme::Zhuyin)),
            (InputScheme::Vietnamese, Some(ChineseScheme::Wubi)),
        ] {
            let mut preferences = Preferences {
                scheme,
                last_chinese_scheme: Some(ChineseScheme::Wubi),
                ..Preferences::default()
            };
            super::remember_chinese_scheme(&mut preferences);
            assert_eq!(preferences.last_chinese_scheme, remembered, "{scheme:?}");
        }
    }

    #[test]
    fn upload_leaves_out_the_schemes_the_cloud_cannot_carry() {
        for scheme in ["cantonese", "zhuyin", "vietnamese"] {
            let mut native = native();
            native.input_scheme = scheme.into();
            let settings =
                local_account_preferences(&native, &Preferences::default(), None).unwrap();
            assert!(!settings.contains_key("input.schema"), "{scheme}");
            assert!(!settings.contains_key("input.shuangpin_schema"), "{scheme}");
            assert!(!settings.contains_key("platform.ios.nine_key"), "{scheme}");
            assert_eq!(
                settings["input.character_set"],
                AccountPreferenceValue::String("traditional".into()),
                "{scheme}"
            );
        }
    }

    #[test]
    fn the_cantonese_zhuyin_and_vietnamese_touch_schemes_select_their_input_schemes() {
        use msime_client_core::preferences::ChineseScheme;
        for (native, touch, scheme, remembered) in [
            (
                "cantonese",
                TouchKeyboardScheme::Cantonese,
                InputScheme::Cantonese,
                ChineseScheme::Cantonese,
            ),
            (
                "zhuyin",
                TouchKeyboardScheme::Zhuyin,
                InputScheme::Zhuyin,
                ChineseScheme::Zhuyin,
            ),
            (
                "vietnamese",
                TouchKeyboardScheme::Vietnamese,
                InputScheme::Vietnamese,
                ChineseScheme::Wubi,
            ),
        ] {
            assert_eq!(super::touch_scheme(native), Ok(touch));
            let mut preferences = Preferences {
                scheme: InputScheme::Wubi,
                last_chinese_scheme: Some(ChineseScheme::Wubi),
                touch_keyboard_layout: TouchKeyboardLayout::NineKey,
                ..Preferences::default()
            };
            preferences.touch_keyboard_schemes.enabled.insert(touch);
            super::select_touch_scheme(&mut preferences, touch);
            assert_eq!(preferences.touch_keyboard_schemes.selected, Some(touch));
            assert_eq!(preferences.scheme, scheme, "{native}");
            assert_eq!(
                preferences.last_chinese_scheme,
                Some(remembered),
                "{native}"
            );
            assert_eq!(
                preferences.touch_keyboard_layout,
                TouchKeyboardLayout::TwentySixKey,
                "{native}"
            );
        }
    }

    #[test]
    fn a_touch_scheme_that_is_not_enabled_falls_back_to_the_first_enabled_one() {
        let mut preferences = Preferences::default();
        assert!(!preferences
            .touch_keyboard_schemes
            .enabled
            .contains(&TouchKeyboardScheme::Zhuyin));
        super::select_touch_scheme(&mut preferences, TouchKeyboardScheme::Zhuyin);
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::Quanpin)
        );
        assert_eq!(preferences.scheme, InputScheme::Quanpin);
    }

    #[test]
    fn the_native_theme_allowlist_is_the_global_theme_ids() {
        for theme in GlobalTheme::ALL {
            let mut native = native();
            native.global_theme = theme.id().into();
            assert!(native.is_valid(), "{}", theme.id());
        }
        let mut native = native();
        native.global_theme = "midnight".into();
        assert!(!native.is_valid());
    }

    #[test]
    fn upload_maps_the_complete_apple_ios_preference_surface() {
        let settings = local_account_preferences(&native(), &Preferences::default(), None).unwrap();
        assert_eq!(
            settings["input.schema"],
            AccountPreferenceValue::String("japanese".into())
        );
        assert_eq!(
            settings["platform.ios.nine_key"],
            AccountPreferenceValue::Boolean(true)
        );
        assert_eq!(
            settings["input.character_set"],
            AccountPreferenceValue::String("traditional".into())
        );
        for key in [
            "platform.ios.sound_enabled",
            "platform.ios.haptics_enabled",
            "platform.ios.haptic_strength",
            "platform.ios.dictionary_learning",
            "platform.ios.global_theme",
            "platform.ios.custom_theme_base",
        ] {
            assert!(settings.contains_key(key), "missing {key}");
        }
        // No design anywhere: the key is left out rather than uploading a design nobody made.
        assert!(!settings.contains_key("platform.ios.custom_keyboard_skin"));
        let design = msime_client_core::preferences::TouchKeyboardSkinDesign::default();
        let settings =
            local_account_preferences(&native(), &Preferences::default(), Some(&design)).unwrap();
        assert_eq!(
            settings["platform.ios.custom_keyboard_skin"],
            AccountPreferenceValue::String(serde_json::to_string(&design).unwrap())
        );
        for key in [
            "input.frequency_mode",
            "input.frequency_trigger_count",
            "input.frequency_linear_step",
        ] {
            assert!(settings.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn download_validates_every_value_before_building_an_apply_plan() {
        let cloud = AccountPreferences {
            revision: 7,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String("shuangpin".into()),
                ),
                (
                    "input.shuangpin_schema".into(),
                    AccountPreferenceValue::String("ziranma".into()),
                ),
                (
                    "platform.ios.haptic_strength".into(),
                    AccountPreferenceValue::String("light".into()),
                ),
            ]),
        };
        let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
        let mut requested = native();
        requested.input_scheme = "quanpin".into();
        let requested = plan.requested_native(&requested).unwrap();
        assert_eq!(requested.input_scheme, "ziranma");
        assert_eq!(requested.haptic_strength, "light");

        for value in [
            AccountPreferenceValue::String("unsupported".into()),
            AccountPreferenceValue::Boolean(true),
        ] {
            let invalid = AccountPreferences {
                revision: 7,
                settings: BTreeMap::from([("platform.ios.haptic_strength".into(), value)]),
            };
            assert_eq!(
                IosPreferencePlan::from_cloud(&invalid),
                Err(AccountError::Invalid)
            );
        }
        for base in ["custom", "fluent"] {
            let invalid = AccountPreferences {
                revision: 7,
                settings: BTreeMap::from([(
                    "platform.ios.custom_theme_base".into(),
                    AccountPreferenceValue::String(base.into()),
                )]),
            };
            assert_eq!(
                IosPreferencePlan::from_cloud(&invalid),
                Err(AccountError::Invalid),
                "{base}"
            );
        }
    }

    #[test]
    fn download_updates_shared_input_state_without_touching_private_settings() {
        let cloud = AccountPreferences {
            revision: 8,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String("japanese".into()),
                ),
                (
                    "platform.ios.nine_key".into(),
                    AccountPreferenceValue::Boolean(true),
                ),
                (
                    "input.character_set".into(),
                    AccountPreferenceValue::String("traditional".into()),
                ),
                (
                    "platform.ios.dictionary_learning".into(),
                    AccountPreferenceValue::Boolean(false),
                ),
                (
                    "platform.ios.global_theme".into(),
                    AccountPreferenceValue::String("night".into()),
                ),
                (
                    "platform.ios.custom_theme_base".into(),
                    AccountPreferenceValue::String("paper".into()),
                ),
                (
                    "input.frequency_mode".into(),
                    AccountPreferenceValue::String("linear".into()),
                ),
                (
                    "input.frequency_trigger_count".into(),
                    AccountPreferenceValue::Integer(7),
                ),
                (
                    "input.frequency_linear_step".into(),
                    AccountPreferenceValue::Integer(4),
                ),
            ]),
        };
        let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
        let mut native = native();
        native.input_scheme = "japaneseNineKey".into();
        native.global_theme = "night".into();
        let mut preferences = Preferences::default();
        preferences.clipboard_history = true;
        plan.apply_shared(&native, &mut preferences).unwrap();
        assert_eq!(preferences.scheme, InputScheme::Japanese);
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::NineKey
        );
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::JapaneseNineKey)
        );
        assert!(preferences.traditional_chinese_output);
        assert!(!preferences.learning);
        assert_eq!(preferences.global_theme, GlobalTheme::Night);
        assert_eq!(preferences.custom_theme.base, GlobalTheme::Paper);
        assert_eq!(preferences.custom_theme.keyboard, None);
        assert_eq!(preferences.frequency.mode.as_str(), "linear");
        assert_eq!(preferences.frequency.trigger_count, 7);
        assert_eq!(preferences.frequency.linear_step, 4);
        assert!(preferences.clipboard_history);

        let cloud = AccountPreferences {
            revision: 9,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String("shuangpin".into()),
                ),
                (
                    "input.shuangpin_schema".into(),
                    AccountPreferenceValue::String("microsoft".into()),
                ),
            ]),
        };
        let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
        native.input_scheme = "microsoft".into();
        plan.apply_shared(&native, &mut preferences).unwrap();
        assert_eq!(preferences.scheme, InputScheme::Shuangpin);
        assert_eq!(preferences.shuangpin_profile, ShuangpinProfile::Microsoft);
    }

    #[test]
    fn an_unknown_cloud_scheme_keeps_the_local_one_and_the_rest_applies() {
        for unknown in ["cantonese", "zhuyin", "vietnamese", "esperanto"] {
            let cloud = AccountPreferences {
                revision: 11,
                settings: BTreeMap::from([
                    (
                        "input.schema".into(),
                        AccountPreferenceValue::String(unknown.into()),
                    ),
                    (
                        "input.character_set".into(),
                        AccountPreferenceValue::String("traditional".into()),
                    ),
                ]),
            };
            let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
            let mut native = native();
            native.input_scheme = "wubi".into();
            native.traditional_chinese_output = false;
            let requested = plan.requested_native(&native).unwrap();
            assert_eq!(requested.input_scheme, "wubi", "{unknown}");
            assert!(requested.traditional_chinese_output, "{unknown}");

            let mut preferences = Preferences {
                scheme: InputScheme::Wubi,
                ..Preferences::default()
            };
            plan.apply_shared(&requested, &mut preferences).unwrap();
            assert_eq!(preferences.scheme, InputScheme::Wubi, "{unknown}");
            assert!(preferences.traditional_chinese_output, "{unknown}");
        }
    }

    #[test]
    fn download_rejects_frequency_values_outside_shared_bounds() {
        for key in [
            "input.frequency_trigger_count",
            "input.frequency_linear_step",
        ] {
            let cloud = AccountPreferences {
                revision: 10,
                settings: BTreeMap::from([(key.into(), AccountPreferenceValue::Integer(0))]),
            };
            let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
            let mut preferences = Preferences::default();
            assert_eq!(
                plan.apply_shared(&native(), &mut preferences),
                Err(AccountError::Invalid)
            );
        }
    }
}
