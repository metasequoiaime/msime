//! 账号设置文档与 iOS 键盘设置之间的映射：上传时把 App Group 里的键盘设置（[`IosKeyboardPreferences`]）和共享偏好导出成账号键值，下载时先校验整份云端文档、算出要写回 App Group 的键盘设置，再把它应用到共享偏好。Tauri iOS 和 `msime_client_account_ios_settings_*` 共用这一份。
//!
//! 写回分两步：宿主先按 [`IosPreferencePlan::requested_native`] 写 App Group，再拿实际保存下来的值调 [`IosPreferencePlan::apply_shared`] 写共享偏好，后一步失败时宿主把 App Group 恢复成旧值。

use super::settings_sync::{
    frequency_account_preferences, insert_bool, insert_string, valid_haptic_strength,
};
use crate::account::{
    validate_account_preferences, AccountError, AccountPreferenceValue, AccountPreferences,
};
use crate::preferences::{
    ChineseScheme, FrequencyMode, InputScheme, Preferences, ShuangpinProfile, TouchKeyboardLayout,
    TouchKeyboardScheme, TouchKeyboardSkinDesign, WubiProfile,
};
use crate::skin::theme::GlobalTheme;
use msime_client_core::ios_keyboard::IosKeyboardPreferences;
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

/// 本机设置导出成账号键值。调用方再按版本（`edition::filter_uploaded_account_settings`）和服务端字段表过滤，然后合并上传。`fallback_custom_skin` 是共享偏好里自定义主题的键盘设计，App Group 里没有时用它。
pub fn local_account_preferences(
    native: &IosKeyboardPreferences,
    shared: &Preferences,
    fallback_custom_skin: Option<&TouchKeyboardSkinDesign>,
) -> Result<BTreeMap<String, AccountPreferenceValue>, AccountError> {
    if !native.is_valid() {
        return Err(AccountError::Storage);
    }
    let mut settings = BTreeMap::new();
    // 云端的 `input.schema` 装不下粤拼、注音、越南文、藏文和笔画（旧设备会拒收整份文档），所以这几个方案不写方案、九键开关和触屏布局，云端保留原有的值。
    // 全拼的 26 键、9 键、14 键、18 键、九键双拼和手写都上传成全拼，用哪种键盘写进 `platform.ios.keyboard_layout`（与 Android 的 `platform.android.keyboard_layout` 同一套词）；还不认识它的旧版本只读 `platform.ios.nine_key`，所以两个键一起上传，九键双拼在那里是 false、落成全拼 26 键，与原生 iOS 原来的 Swift 映射相同。
    let scheme = match native.input_scheme.as_str() {
        "quanpin" => Some(("quanpin", None, "twenty_six_key")),
        "nineKey" => Some(("quanpin", None, "nine_key")),
        "fourteenKey" => Some(("quanpin", None, "fourteen_key")),
        "eighteenKey" => Some(("quanpin", None, "eighteen_key")),
        "nineKeyShuangpin" => Some(("quanpin", None, "nine_key_shuangpin")),
        "handwriting" => Some(("quanpin", None, "handwriting")),
        "shuangpin" => Some(("shuangpin", Some("xiaohe"), "twenty_six_key")),
        "ziranma" => Some(("shuangpin", Some("ziranma"), "twenty_six_key")),
        "microsoft" => Some(("shuangpin", Some("microsoft"), "twenty_six_key")),
        "shoudao" => Some(("shuangpin", Some("shoudao"), "twenty_six_key")),
        "wubi" => Some(("wubi", None, "twenty_six_key")),
        "japanese" => Some(("japanese", None, "twenty_six_key")),
        "japaneseNineKey" => Some(("japanese", None, "nine_key")),
        "korean" => Some(("korean", None, "twenty_six_key")),
        scheme if UNSYNCED_NATIVE_SCHEMES.contains(&scheme) => None,
        _ => return Err(AccountError::Storage),
    };
    if let Some((schema, profile, _)) = scheme {
        insert_string(&mut settings, "input.schema", schema);
        if let Some(profile) = profile {
            insert_string(&mut settings, "input.shuangpin_schema", profile);
        }
        // 键盘偏好只记五笔这个方案，86 还是 98 记在共享偏好里。
        if schema == "wubi" {
            insert_string(
                &mut settings,
                "input.wubi_schema",
                match shared.wubi_profile {
                    WubiProfile::Wubi86 => "wubi86",
                    WubiProfile::Wubi98 => "wubi98",
                },
            );
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
    if let Some((_, _, layout)) = scheme {
        insert_string(&mut settings, IOS_KEYBOARD_LAYOUT, layout);
        insert_bool(&mut settings, "platform.ios.nine_key", layout == "nine_key");
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
    // 「原生」先不上传，云端保留原来的主题：加入它之前的 iOS 版本只认七个主题 id，同一账号下还没升级的设备收到它会拒绝整份设置文档。
    if native.global_theme != GlobalTheme::Native.id() {
        insert_string(
            &mut settings,
            "platform.ios.global_theme",
            &native.global_theme,
        );
    }
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

/// 账号的 `input.schema` 装不下的 iOS 键盘方案：导出时不写方案、九键开关和触屏布局，应用时本机是其中之一就保留本机的方案（见 `requested_native`）。
const UNSYNCED_NATIVE_SCHEMES: [&str; 5] =
    ["cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"];

/// iOS 的触屏布局键，取值与 Android 的 `platform.android.keyboard_layout` 相同。服务端只限它是不超过 32 字节的字符串，以后可能多出别的取值，所以下载时不认识的取值当作没有这一项。
const IOS_KEYBOARD_LAYOUT: &str = "platform.ios.keyboard_layout";
const IOS_KEYBOARD_LAYOUTS: [&str; 6] = [
    "twenty_six_key",
    "nine_key",
    "fourteen_key",
    "handwriting",
    "nine_key_shuangpin",
    "eighteen_key",
];

/// 一份已经校验过的云端设置文档要改的 iOS 值；没有出现在文档里的键是 `None`，应用时保留本机的值。
#[derive(Debug, PartialEq)]
pub struct IosPreferencePlan {
    input_scheme: Option<String>,
    /// 云端写着认得的 `platform.ios.keyboard_layout`：方案的键盘布局由它说了算，见 `requested_native`。
    has_keyboard_layout: bool,
    wubi_profile: Option<WubiProfile>,
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
    /// 校验整份云端文档并取出 iOS 认得的值；任何一个值不合规都拒绝整份文档（`Invalid`）。
    pub fn from_cloud(cloud: &AccountPreferences) -> Result<Self, AccountError> {
        validate_account_preferences(cloud)?;
        let values = &cloud.settings;
        // 有 `keyboard_layout` 时以它为准，没有（旧版本上传的文档、服务端还不认识这个键）时退回 `nine_key`。
        let layout = string_setting(values, IOS_KEYBOARD_LAYOUT)?
            .filter(|value| IOS_KEYBOARD_LAYOUTS.contains(&value.as_str()));
        let nine_key_setting = bool_setting(values, "platform.ios.nine_key")?;
        let nine_key = match layout.as_deref() {
            Some(layout) => layout == "nine_key",
            None => nine_key_setting == Some(true),
        };
        // 账号里的五笔版本有就写进共享偏好，不管云端的方案是不是五笔，与 Android、桌面和鸿蒙共用的 `settings_sync::apply_input_settings` 相同：方案是全拼、账号里留着别的设备选的 98 时，本机以后切到五笔就是 98。没有这个键的文档来自还不认识 98 五笔、或者当时不在五笔上的设备，保留本机的五笔版本。不认识的取值与其余的键一样拒绝整份文档。
        let wubi_profile = string_setting(values, "input.wubi_schema")?
            .map(|value| match value.as_str() {
                "wubi86" => Ok(WubiProfile::Wubi86),
                "wubi98" => Ok(WubiProfile::Wubi98),
                _ => Err(AccountError::Invalid),
            })
            .transpose()?;
        let input_scheme = match string_setting(values, "input.schema")?.as_deref() {
            None => None,
            Some("quanpin") => Some(
                match layout.as_deref() {
                    Some("fourteen_key") => "fourteenKey",
                    Some("eighteen_key") => "eighteenKey",
                    Some("nine_key_shuangpin") => "nineKeyShuangpin",
                    Some("handwriting") => "handwriting",
                    _ if nine_key => "nineKey",
                    _ => "quanpin",
                }
                .into(),
            ),
            Some("shuangpin") => {
                let profile = string_setting(values, "input.shuangpin_schema")?
                    .unwrap_or_else(|| "xiaohe".into());
                match profile.as_str() {
                    "xiaohe" => Some("shuangpin".into()),
                    "ziranma" | "microsoft" | "shoudao" => Some(profile),
                    _ => return Err(AccountError::Invalid),
                }
            }
            Some("wubi") => Some("wubi".into()),
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
            // A scheme this host does not offer (a newer device's Cantonese, Zhuyin, Vietnamese or Stroke) keeps the local one rather than refusing the whole sync, so the rest of the document still applies.
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
            .is_some_and(|value| !valid_haptic_strength(value))
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
            has_keyboard_layout: layout.is_some(),
            wubi_profile,
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

    /// 宿主应写进 App Group 的键盘设置：本机当前值 `current` 加上文档里的值。
    pub fn requested_native(
        &self,
        current: &IosKeyboardPreferences,
    ) -> Result<IosKeyboardPreferences, AccountError> {
        if !current.is_valid() {
            return Err(AccountError::Storage);
        }
        let mut requested = current.clone();
        // 云端写着 `platform.ios.keyboard_layout` 时它就是那台设备选的布局，照常切换，26 键也会把本机的 14 键换掉。没有这一项的文档来自还不上传它的版本，那时 14 键上传成全拼、不是九键：云端是这一组值时本机的 14 键保留，免得在本机「应用云端设置」把 14 键改回 26 键；云端是九键或别的方案时照常切换。18 键同理：还不认识 `eighteen_key` 的版本读到它时当作没有这一项，回写的也是全拼。
        let keeps_fourteen_key = !self.has_keyboard_layout
            && self.input_scheme.as_deref() == Some("quanpin")
            && matches!(current.input_scheme.as_str(), "fourteenKey" | "eighteenKey");
        // 本机是账号装不下的方案时没有导出方案，云端留着的是上次的方案：合并上传后写回本机、或者下载别的设备的设置时都不拿它盖掉本机的方案，否则用户刚切过去的粤拼会被这一轮自己的上传改回全拼。
        let keeps_unsynced_scheme =
            UNSYNCED_NATIVE_SCHEMES.contains(&current.input_scheme.as_str());
        if let Some(value) = self
            .input_scheme
            .as_ref()
            .filter(|_| !keeps_fourteen_key && !keeps_unsynced_scheme)
        {
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

    /// 把文档应用到共享偏好。`native` 是宿主实际保存下来的键盘设置，方案、繁简、学习和全局主题以它为准。
    pub fn apply_shared(
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
        if let Some(value) = self.wubi_profile {
            preferences.wubi_profile = value;
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
        "korean" => Ok(TouchKeyboardScheme::Korean),
        "cantonese" => Ok(TouchKeyboardScheme::Cantonese),
        "cantoneseShuangpin" => Ok(TouchKeyboardScheme::CantoneseShuangpin),
        "zhuyin" => Ok(TouchKeyboardScheme::Zhuyin),
        "vietnamese" => Ok(TouchKeyboardScheme::Vietnamese),
        "tibetan" => Ok(TouchKeyboardScheme::Tibetan),
        "stroke" => Ok(TouchKeyboardScheme::Stroke),
        "fourteenKey" => Ok(TouchKeyboardScheme::FourteenKey),
        "nineKeyShuangpin" => Ok(TouchKeyboardScheme::NineKeyShuangpin),
        "eighteenKey" => Ok(TouchKeyboardScheme::EighteenKey),
        _ => Err(AccountError::Invalid),
    }
}

/// 记住选日文或韩文后要回到的中文方案；从日文、韩文、越南文或藏文切走时保留已经记住的那个。
fn remember_chinese_scheme(preferences: &mut Preferences) {
    let chinese = match preferences.scheme {
        InputScheme::Quanpin => ChineseScheme::Quanpin,
        InputScheme::Shuangpin => ChineseScheme::Shuangpin,
        InputScheme::Wubi => ChineseScheme::Wubi,
        InputScheme::Cantonese => ChineseScheme::Cantonese,
        InputScheme::Zhuyin => ChineseScheme::Zhuyin,
        InputScheme::Stroke => ChineseScheme::Stroke,
        InputScheme::Japanese
        | InputScheme::Korean
        | InputScheme::Vietnamese
        | InputScheme::Tibetan => return,
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
        preferences.touch_keyboard_schemes.first_enabled()
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
        TouchKeyboardScheme::Tibetan => {
            remember_chinese_scheme(preferences);
            preferences.scheme = InputScheme::Tibetan;
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Cantonese | TouchKeyboardScheme::CantoneseShuangpin => {
            preferences.scheme = InputScheme::Cantonese;
            preferences.last_chinese_scheme = Some(ChineseScheme::Cantonese);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Zhuyin => {
            preferences.scheme = InputScheme::Zhuyin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Zhuyin);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        // 注音 9 键目前只有 Android 宿主会写出；这里只让 `select_touch_scheme` 对它有确定的取值。
        TouchKeyboardScheme::ZhuyinNineKey => {
            preferences.scheme = InputScheme::Zhuyin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Zhuyin);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::NineKey;
        }
        // 笔画键盘由宿主自己画，26 键与九键下都显示同一块笔画键盘，这里与注音一样记为 26 键。
        TouchKeyboardScheme::Stroke => {
            preferences.scheme = InputScheme::Stroke;
            preferences.last_chinese_scheme = Some(ChineseScheme::Stroke);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        TouchKeyboardScheme::Wubi => {
            preferences.scheme = InputScheme::Wubi;
            preferences.last_chinese_scheme = Some(ChineseScheme::Wubi);
            preferences.touch_keyboard_layout = TouchKeyboardLayout::TwentySixKey;
        }
        // 九键双拼的引擎方案是全拼，布局是九键双拼（原生 iOS 的 `nineKeyShuangpin`）。
        TouchKeyboardScheme::Quanpin
        | TouchKeyboardScheme::NineKey
        | TouchKeyboardScheme::FourteenKey
        | TouchKeyboardScheme::EighteenKey
        | TouchKeyboardScheme::NineKeyShuangpin
        | TouchKeyboardScheme::Handwriting => {
            preferences.scheme = InputScheme::Quanpin;
            preferences.last_chinese_scheme = Some(ChineseScheme::Quanpin);
            preferences.touch_keyboard_layout = match selected {
                TouchKeyboardScheme::NineKey => TouchKeyboardLayout::NineKey,
                TouchKeyboardScheme::FourteenKey => TouchKeyboardLayout::FourteenKey,
                TouchKeyboardScheme::EighteenKey => TouchKeyboardLayout::EighteenKey,
                TouchKeyboardScheme::NineKeyShuangpin => TouchKeyboardLayout::NineKeyShuangpin,
                TouchKeyboardScheme::Handwriting => TouchKeyboardLayout::Handwriting,
                _ => TouchKeyboardLayout::TwentySixKey,
            };
        }
    }
}

#[cfg(test)]
mod wubi_tests;

#[cfg(test)]
mod tests {
    use super::{
        local_account_preferences, IosPreferencePlan, IOS_KEYBOARD_LAYOUT, UNSYNCED_NATIVE_SCHEMES,
    };
    use crate::account::{AccountError, AccountPreferenceValue, AccountPreferences};
    use crate::skin::theme::GlobalTheme;
    use msime_client_core::ios_keyboard::IosKeyboardPreferences;
    use msime_client_core::preferences::{
        InputScheme, Preferences, ShuangpinProfile, TouchKeyboardLayout, TouchKeyboardScheme,
        WubiProfile,
    };
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
            tablet_split_keyboard: None,
            dictionary_learning: false,
            global_theme: "custom".into(),
            custom_keyboard_skin: None,
        }
    }

    #[test]
    fn cantonese_zhuyin_and_stroke_are_remembered_and_vietnamese_keeps_the_last_chinese_scheme() {
        use msime_client_core::preferences::ChineseScheme;
        for (scheme, remembered) in [
            (InputScheme::Cantonese, Some(ChineseScheme::Cantonese)),
            (InputScheme::Zhuyin, Some(ChineseScheme::Zhuyin)),
            (InputScheme::Stroke, Some(ChineseScheme::Stroke)),
            (InputScheme::Vietnamese, Some(ChineseScheme::Wubi)),
            (InputScheme::Tibetan, Some(ChineseScheme::Wubi)),
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
        for scheme in ["cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"] {
            let mut native = native();
            native.input_scheme = scheme.into();
            let settings =
                local_account_preferences(&native, &Preferences::default(), None).unwrap();
            assert!(!settings.contains_key("input.schema"), "{scheme}");
            assert!(!settings.contains_key("input.shuangpin_schema"), "{scheme}");
            assert!(!settings.contains_key("platform.ios.nine_key"), "{scheme}");
            assert!(
                !settings.contains_key("platform.ios.keyboard_layout"),
                "{scheme}"
            );
            assert_eq!(
                settings["input.character_set"],
                AccountPreferenceValue::String("traditional".into()),
                "{scheme}"
            );
        }
    }

    #[test]
    fn the_cantonese_zhuyin_vietnamese_and_stroke_touch_schemes_select_their_input_schemes() {
        use msime_client_core::preferences::ChineseScheme;
        for (native, touch, scheme, remembered) in [
            (
                "cantonese",
                TouchKeyboardScheme::Cantonese,
                InputScheme::Cantonese,
                ChineseScheme::Cantonese,
            ),
            (
                "cantoneseShuangpin",
                TouchKeyboardScheme::CantoneseShuangpin,
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
            (
                "tibetan",
                TouchKeyboardScheme::Tibetan,
                InputScheme::Tibetan,
                ChineseScheme::Wubi,
            ),
            (
                "stroke",
                TouchKeyboardScheme::Stroke,
                InputScheme::Stroke,
                ChineseScheme::Stroke,
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

    /// 全拼 14 键：原生 id `fourteenKey` 选中全拼的 14 键布局；上传时按全拼，布局写进 `platform.ios.keyboard_layout`，给旧版本的 `platform.ios.nine_key` 照旧是 false。
    #[test]
    fn the_fourteen_key_scheme_selects_quanpin_on_the_fourteen_key_layout_and_uploads_as_quanpin() {
        use msime_client_core::preferences::ChineseScheme;
        assert_eq!(
            super::touch_scheme("fourteenKey"),
            Ok(TouchKeyboardScheme::FourteenKey)
        );
        let mut preferences = Preferences {
            scheme: InputScheme::Wubi,
            last_chinese_scheme: Some(ChineseScheme::Wubi),
            ..Preferences::default()
        };
        preferences
            .touch_keyboard_schemes
            .enabled
            .insert(TouchKeyboardScheme::FourteenKey);
        super::select_touch_scheme(&mut preferences, TouchKeyboardScheme::FourteenKey);
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::FourteenKey)
        );
        assert_eq!(preferences.scheme, InputScheme::Quanpin);
        assert_eq!(
            preferences.last_chinese_scheme,
            Some(ChineseScheme::Quanpin)
        );
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::FourteenKey
        );
        // 没有打开 14 键时退回第一个启用的方案，与其他需要用户打开的方案一样。
        let mut fallback = Preferences::default();
        super::select_touch_scheme(&mut fallback, TouchKeyboardScheme::FourteenKey);
        assert_eq!(
            fallback.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::Quanpin)
        );
        assert_eq!(
            fallback.touch_keyboard_layout,
            TouchKeyboardLayout::TwentySixKey
        );

        let mut native = native();
        native.input_scheme = "fourteenKey".into();
        assert!(native.is_valid());
        let settings = local_account_preferences(&native, &Preferences::default(), None).unwrap();
        assert_eq!(
            settings["input.schema"],
            AccountPreferenceValue::String("quanpin".into())
        );
        assert_eq!(
            settings["platform.ios.nine_key"],
            AccountPreferenceValue::Boolean(false)
        );
        assert_eq!(
            settings["platform.ios.keyboard_layout"],
            AccountPreferenceValue::String("fourteen_key".into())
        );
    }

    /// 九键双拼：原生 iOS 的 `nineKeyShuangpin` 是合法的键盘设置，选中全拼的九键双拼布局；上传时按全拼，布局是 `nine_key_shuangpin`，给旧版本的九键开关是 false。
    #[test]
    fn the_nine_key_shuangpin_scheme_round_trips() {
        assert_eq!(
            super::touch_scheme("nineKeyShuangpin"),
            Ok(TouchKeyboardScheme::NineKeyShuangpin)
        );
        let mut native = native();
        native.input_scheme = "nineKeyShuangpin".into();
        assert!(native.is_valid());
        let settings = local_account_preferences(&native, &Preferences::default(), None).unwrap();
        assert_eq!(
            settings["input.schema"],
            AccountPreferenceValue::String("quanpin".into())
        );
        assert!(!settings.contains_key("input.shuangpin_schema"));
        let mut preferences = Preferences::default();
        preferences
            .touch_keyboard_schemes
            .enabled
            .insert(TouchKeyboardScheme::NineKeyShuangpin);
        super::select_touch_scheme(&mut preferences, TouchKeyboardScheme::NineKeyShuangpin);
        assert_eq!(preferences.scheme, InputScheme::Quanpin);
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::NineKeyShuangpin
        );
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::NineKeyShuangpin)
        );
    }

    /// 全拼 18 键：原生 id `eighteenKey` 选中全拼的 18 键布局，上传时按全拼，布局写进 `platform.ios.keyboard_layout`；下载时按这个布局回到 `eighteenKey`，应用到共享偏好也是 18 键。
    #[test]
    fn the_eighteen_key_scheme_round_trips_as_quanpin_on_the_eighteen_key_layout() {
        assert_eq!(
            super::touch_scheme("eighteenKey"),
            Ok(TouchKeyboardScheme::EighteenKey)
        );
        let mut local = native();
        local.input_scheme = "eighteenKey".into();
        assert!(local.is_valid());
        let settings = local_account_preferences(&local, &Preferences::default(), None).unwrap();
        assert_eq!(
            settings["input.schema"],
            AccountPreferenceValue::String("quanpin".into())
        );
        assert_eq!(
            settings["platform.ios.keyboard_layout"],
            AccountPreferenceValue::String("eighteen_key".into())
        );
        assert_eq!(
            settings["platform.ios.nine_key"],
            AccountPreferenceValue::Boolean(false)
        );

        let cloud = AccountPreferences {
            revision: 3,
            settings: settings.clone(),
        };
        let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
        let mut current = native();
        current.input_scheme = "quanpin".into();
        let requested = plan.requested_native(&current).unwrap();
        assert_eq!(requested.input_scheme, "eighteenKey");
        let mut preferences = Preferences::default();
        preferences
            .touch_keyboard_schemes
            .enabled
            .insert(TouchKeyboardScheme::EighteenKey);
        plan.apply_shared(&requested, &mut preferences).unwrap();
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::EighteenKey)
        );
        assert_eq!(preferences.scheme, InputScheme::Quanpin);
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::EighteenKey
        );
    }

    /// 每个全拼入口上传的触屏布局：26 键、九键、14 键、18 键和手写各有自己的值，九键开关只在九键时为真。
    #[test]
    fn upload_names_the_touch_layout_of_each_quanpin_entry() {
        for (scheme, layout, nine_key) in [
            ("quanpin", "twenty_six_key", false),
            ("nineKey", "nine_key", true),
            ("fourteenKey", "fourteen_key", false),
            ("eighteenKey", "eighteen_key", false),
            ("handwriting", "handwriting", false),
            ("nineKeyShuangpin", "nine_key_shuangpin", false),
            ("japaneseNineKey", "nine_key", true),
            ("wubi", "twenty_six_key", false),
        ] {
            let mut native = native();
            native.input_scheme = scheme.into();
            let settings =
                local_account_preferences(&native, &Preferences::default(), None).unwrap();
            assert_eq!(
                settings["platform.ios.keyboard_layout"],
                AccountPreferenceValue::String(layout.into()),
                "{scheme}"
            );
            assert_eq!(
                settings["platform.ios.nine_key"],
                AccountPreferenceValue::Boolean(nine_key),
                "{scheme}"
            );
        }
    }

    /// 云端有 `platform.ios.keyboard_layout` 时按它落成方案，写明的 26 键也会换掉本机的 14 键；它和九键开关说法不一时听它的；不认识的取值当作没有，退回九键开关和保留 14 键的规则。
    #[test]
    fn applying_cloud_prefers_the_ios_keyboard_layout() {
        let cloud = |schema: &str, layout: Option<&str>, nine_key: bool| AccountPreferences {
            revision: 7,
            settings: BTreeMap::from_iter(
                [
                    (
                        "input.schema".to_owned(),
                        AccountPreferenceValue::String(schema.into()),
                    ),
                    (
                        "platform.ios.nine_key".to_owned(),
                        AccountPreferenceValue::Boolean(nine_key),
                    ),
                ]
                .into_iter()
                .chain(layout.map(|layout| {
                    (
                        "platform.ios.keyboard_layout".to_owned(),
                        AccountPreferenceValue::String(layout.into()),
                    )
                })),
            ),
        };
        let requested = |cloud: &AccountPreferences, local: &str| {
            let mut current = native();
            current.input_scheme = local.into();
            IosPreferencePlan::from_cloud(cloud)
                .unwrap()
                .requested_native(&current)
                .unwrap()
                .input_scheme
        };
        assert_eq!(
            requested(&cloud("quanpin", Some("fourteen_key"), false), "quanpin"),
            "fourteenKey"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("eighteen_key"), false), "quanpin"),
            "eighteenKey"
        );
        assert_eq!(
            requested(
                &cloud("quanpin", Some("twenty_six_key"), false),
                "eighteenKey"
            ),
            "quanpin"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("handwriting"), false), "quanpin"),
            "handwriting"
        );
        assert_eq!(
            requested(
                &cloud("quanpin", Some("nine_key_shuangpin"), false),
                "quanpin"
            ),
            "nineKeyShuangpin"
        );
        assert_eq!(
            requested(
                &cloud("quanpin", Some("twenty_six_key"), false),
                "fourteenKey"
            ),
            "quanpin"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("twenty_six_key"), true), "quanpin"),
            "quanpin"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("nine_key"), false), "quanpin"),
            "nineKey"
        );
        assert_eq!(
            requested(&cloud("japanese", Some("nine_key"), false), "quanpin"),
            "japaneseNineKey"
        );
        assert_eq!(
            requested(&cloud("wubi", Some("fourteen_key"), false), "quanpin"),
            "wubi"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("twelve_key"), true), "quanpin"),
            "nineKey"
        );
        assert_eq!(
            requested(&cloud("quanpin", Some("twelve_key"), false), "fourteenKey"),
            "fourteenKey"
        );
        let mut wrong_type = cloud("quanpin", None, false);
        wrong_type.settings.insert(
            "platform.ios.keyboard_layout".into(),
            AccountPreferenceValue::Boolean(true),
        );
        assert!(IosPreferencePlan::from_cloud(&wrong_type).is_err());
    }

    /// 云端的全拼、不是九键可能就是本机 14 键上传的：本机是 14 键时保留，云端是九键或别的方案时照常切换，本机不是 14 键时照常落成全拼 26 键。
    #[test]
    fn applying_cloud_quanpin_keeps_a_local_fourteen_key_choice() {
        let cloud = |nine_key: bool, schema: &str| AccountPreferences {
            revision: 7,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String(schema.into()),
                ),
                (
                    "platform.ios.nine_key".into(),
                    AccountPreferenceValue::Boolean(nine_key),
                ),
            ]),
        };
        let requested = |cloud: &AccountPreferences, local: &str| {
            let mut current = native();
            current.input_scheme = local.into();
            IosPreferencePlan::from_cloud(cloud)
                .unwrap()
                .requested_native(&current)
                .unwrap()
                .input_scheme
        };
        assert_eq!(
            requested(&cloud(false, "quanpin"), "fourteenKey"),
            "fourteenKey"
        );
        assert_eq!(requested(&cloud(true, "quanpin"), "fourteenKey"), "nineKey");
        // 18 键同理：还不认识 `eighteen_key` 的版本回写的全拼不把本机的 18 键换掉。
        assert_eq!(
            requested(&cloud(false, "quanpin"), "eighteenKey"),
            "eighteenKey"
        );
        assert_eq!(requested(&cloud(true, "quanpin"), "eighteenKey"), "nineKey");
        assert_eq!(requested(&cloud(false, "wubi"), "fourteenKey"), "wubi");
        assert_eq!(requested(&cloud(false, "quanpin"), "nineKey"), "quanpin");

        let mut native = native();
        native.input_scheme = "fourteenKey".into();
        let mut preferences = Preferences::default();
        preferences
            .touch_keyboard_schemes
            .enabled
            .insert(TouchKeyboardScheme::FourteenKey);
        let plan = IosPreferencePlan::from_cloud(&cloud(false, "quanpin")).unwrap();
        let native = plan.requested_native(&native).unwrap();
        plan.apply_shared(&native, &mut preferences).unwrap();
        assert_eq!(
            preferences.touch_keyboard_schemes.selected,
            Some(TouchKeyboardScheme::FourteenKey)
        );
        assert_eq!(
            preferences.touch_keyboard_layout,
            TouchKeyboardLayout::FourteenKey
        );
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
        // 笔画同样是需要用户打开的方案。
        let mut preferences = Preferences::default();
        super::select_touch_scheme(&mut preferences, TouchKeyboardScheme::Stroke);
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
            settings["platform.ios.keyboard_layout"],
            AccountPreferenceValue::String("nine_key".into())
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
    fn upload_leaves_the_native_theme_out_for_older_devices() {
        let mut native = native();
        native.global_theme = GlobalTheme::Native.id().into();
        let settings = local_account_preferences(&native, &Preferences::default(), None).unwrap();
        assert!(!settings.contains_key("platform.ios.global_theme"));
        assert!(settings.contains_key("platform.ios.custom_theme_base"));
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
        let mut preferences = Preferences {
            clipboard_history: true,
            ..Preferences::default()
        };
        // 新装的默认启用列表只有中文方案，日文 9 键要用户自己打开；云端选中一个本机没启用的方案会回到第一个启用的方案（见 `a_touch_scheme_that_is_not_enabled_falls_back_to_the_first_enabled_one`）。这里测的是下载把共享输入状态写进来，所以这台设备上日文 9 键是开着的。
        preferences
            .touch_keyboard_schemes
            .enabled
            .insert(TouchKeyboardScheme::JapaneseNineKey);
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

    /// 本机是账号装不下的方案时没有导出方案，云端留着的是上次的方案和布局。上传之后把合并结果写回本机、或者应用别的设备的文档时，本机的方案不变，其余的键照常应用。
    #[test]
    fn a_local_scheme_the_account_cannot_hold_is_kept_when_the_cloud_applies() {
        for scheme in UNSYNCED_NATIVE_SCHEMES {
            let mut native = native();
            native.input_scheme = scheme.into();
            native.traditional_chinese_output = false;
            let exported =
                local_account_preferences(&native, &Preferences::default(), None).unwrap();
            assert!(!exported.contains_key("input.schema"), "{scheme}");
            let cloud = AccountPreferences {
                revision: 4,
                settings: BTreeMap::from([
                    (
                        "input.schema".into(),
                        AccountPreferenceValue::String("quanpin".into()),
                    ),
                    (
                        IOS_KEYBOARD_LAYOUT.into(),
                        AccountPreferenceValue::String("nine_key".into()),
                    ),
                    (
                        "input.character_set".into(),
                        AccountPreferenceValue::String("traditional".into()),
                    ),
                ]),
            };
            let plan = IosPreferencePlan::from_cloud(&cloud).unwrap();
            let requested = plan.requested_native(&native).unwrap();
            assert_eq!(requested.input_scheme, scheme, "{scheme}");
            assert!(requested.traditional_chinese_output, "{scheme}");
        }
    }

    #[test]
    fn an_unknown_cloud_scheme_keeps_the_local_one_and_the_rest_applies() {
        for unknown in [
            "cantonese",
            "zhuyin",
            "vietnamese",
            "tibetan",
            "stroke",
            "esperanto",
        ] {
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

    #[test]
    fn the_wubi_profile_travels_as_the_account_wubi_schema() {
        let mut native = native();
        native.input_scheme = "wubi".into();
        let shared = Preferences {
            wubi_profile: WubiProfile::Wubi98,
            ..Preferences::default()
        };
        let settings = local_account_preferences(&native, &shared, None).unwrap();
        assert_eq!(
            settings["input.wubi_schema"],
            AccountPreferenceValue::String("wubi98".into())
        );
        // 不是五笔时不写这个键。
        native.input_scheme = "quanpin".into();
        let settings = local_account_preferences(&native, &shared, None).unwrap();
        assert!(!settings.contains_key("input.wubi_schema"));

        native.input_scheme = "wubi".into();
        for (value, expected) in [
            (Some("wubi98"), WubiProfile::Wubi98),
            (Some("wubi86"), WubiProfile::Wubi86),
            (None, WubiProfile::Wubi98),
        ] {
            let mut settings = BTreeMap::from([(
                "input.schema".into(),
                AccountPreferenceValue::String("wubi".into()),
            )]);
            if let Some(value) = value {
                settings.insert(
                    "input.wubi_schema".into(),
                    AccountPreferenceValue::String(value.into()),
                );
            }
            let plan = IosPreferencePlan::from_cloud(&AccountPreferences {
                revision: 12,
                settings,
            })
            .unwrap();
            let requested = plan.requested_native(&native).unwrap();
            assert_eq!(requested.input_scheme, "wubi", "{value:?}");
            let mut preferences = Preferences {
                wubi_profile: WubiProfile::Wubi98,
                ..Preferences::default()
            };
            preferences
                .touch_keyboard_schemes
                .enabled
                .insert(TouchKeyboardScheme::Wubi);
            plan.apply_shared(&requested, &mut preferences).unwrap();
            assert_eq!(preferences.scheme, InputScheme::Wubi, "{value:?}");
            assert_eq!(preferences.wubi_profile, expected, "{value:?}");
        }

        let invalid = AccountPreferences {
            revision: 13,
            settings: BTreeMap::from([
                (
                    "input.schema".into(),
                    AccountPreferenceValue::String("wubi".into()),
                ),
                (
                    "input.wubi_schema".into(),
                    AccountPreferenceValue::String("wubi06".into()),
                ),
            ]),
        };
        assert_eq!(
            IosPreferencePlan::from_cloud(&invalid),
            Err(AccountError::Invalid)
        );
    }

    /// 本地识别的停顿时间和判停灵敏度（#10310）只留在本机：共享文档里改了它们，iOS 上传的账号键值一个也不变。
    #[test]
    fn voice_pause_settings_never_reach_the_account() {
        let mut shared = Preferences::default();
        shared.voice_input.pause_millis = 4000;
        shared.voice_input.stop_sensitivity = 20;
        assert_eq!(
            local_account_preferences(&native(), &shared, None).unwrap(),
            local_account_preferences(&native(), &Preferences::default(), None).unwrap()
        );
    }
}
