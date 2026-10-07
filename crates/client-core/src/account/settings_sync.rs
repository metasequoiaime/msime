//! Android 本地偏好与账号设置同步文档（`/v1/users/me/preferences` 的 `settings`）之间的映射。
//!
//! 这份映射原来写在 Tauri 应用的 `android_account.rs` 里，安卓原生宿主拿不到；搬到这里以后 Tauri 和原生宿主（经 C ABI）共用一份。键名和取值与原来完全一致，唯一的行为变化是：云端某个键的取值超出本机范围或是本机不认识的枚举值时，只跳过这一个键并在结果里列出，其余的键照常应用，而不是让整份文档失败。值的类型与字段表不符、字段表本身与本机期望的类型冲突，仍然拒绝整份文档。
//!
//! 只映射设备之间应当一致的设置。凭据（各类 token、密钥）和诊断日志是设备本地的，永远不出现在导出结果里，测试锁住了这一点。
//!
//! 设计改版新增的 Android 设置（应用主题、单手、按键细节、工具栏的常用语/输入方式/隐藏、手写、离线语音）不在共享偏好里，而在 Android 宿主自己的本地设置文件里。宿主把其中参与同步的值按同步键传进来（[`insert_android_local_settings`]），应用时拿回云端校验过的值写回本地文件（[`android_local_settings`]）。隐私模式、开发者选项和语音数据贡献也在那个文件里，但不在 [`ANDROID_LOCAL_SETTINGS`] 里，所以永远不同步。
//!
//! 按键音、振动开关和振动强度不在共享偏好里，而在宿主自己的本地存储（Android 的 `KeyboardFeedbackStore`），由调用方读出来作为 [`HostKeyboardFeedback`] 传入，应用后再由调用方写回。

use super::{AccountError, AccountPreferenceSchema, AccountPreferenceValue, AccountPreferences};
use crate::preferences::{
    FrequencyMode, InputScheme, Preferences, ShuangpinProfile, ThemeMode, TouchKeyboardLayout,
    WubiProfile,
};
use crate::skin::theme::GlobalTheme;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 按键反馈三个键，值来自宿主本地存储而不是共享偏好。
pub const ANDROID_FEEDBACK_KEYS: [&str; 3] = [
    "platform.android.sound_enabled",
    "platform.android.haptics_enabled",
    "platform.android.haptic_strength",
];

const GLOBAL_THEME: &str = "platform.android.global_theme";
const CUSTOM_THEME_BASE: &str = "platform.android.custom_theme_base";
const CUSTOM_KEYBOARD_SKIN: &str = "platform.android.custom_keyboard_skin";
const CUSTOM_CANDIDATE_SKIN: &str = "platform.android.custom_candidate_skin";
/// 整个自定义键盘皮肤库（JSON 数组，只含设计参数），值来自宿主的皮肤库而不是共享偏好。
pub const CUSTOM_KEYBOARD_SKINS: &str = "platform.android.custom_keyboard_skins";
/// [`CUSTOM_KEYBOARD_SKINS`] 的字节上限，与服务端字段表相同。
pub const MAX_CUSTOM_KEYBOARD_SKINS_BYTES: usize = 786_432;
/// 按键音包 id 的长度上限，与服务端字段表相同。
const MAX_KEY_SOUND_PACK_CHARS: usize = 64;
/// 语音识别语言的长度上限，与服务端字段表相同。
const MAX_VOICE_LANGUAGE_CHARS: usize = 16;

/// 工具栏按钮在同步文档里的键，与 `touch_toolbar` 的成员一一对应。常用语、输入方式和「隐藏」三个开关在 Android 本地设置里，见 [`ANDROID_LOCAL_SETTINGS`]。
const TOOLBAR_KEYS: [&str; 8] = [
    "platform.android.toolbar_layout",
    "platform.android.toolbar_emoji",
    "platform.android.toolbar_clipboard",
    "platform.android.toolbar_skin",
    "platform.android.toolbar_ai",
    "platform.android.toolbar_character_set",
    "platform.android.toolbar_fullwidth",
    "platform.android.toolbar_punctuation",
];

/// `touch_toolbar` 里与 [`TOOLBAR_KEYS`] 同序的成员。
fn toolbar_member(
    toolbar: &mut crate::preferences::TouchToolbarPreferences,
    index: usize,
) -> &mut bool {
    match index {
        0 => &mut toolbar.layout,
        1 => &mut toolbar.emoji,
        2 => &mut toolbar.clipboard,
        3 => &mut toolbar.skin,
        4 => &mut toolbar.ai,
        5 => &mut toolbar.character_set,
        6 => &mut toolbar.fullwidth,
        _ => &mut toolbar.punctuation,
    }
}

/// Android 本地设置的一项在同步文档里允许的取值。
#[derive(Clone, Copy, Debug)]
enum LocalSetting {
    Boolean,
    Choice(&'static [&'static str]),
    /// `min..=max` 里从 `min` 起每 `step` 一档的整数。
    Integer {
        min: i64,
        max: i64,
        step: i64,
    },
}

impl LocalSetting {
    fn schema_type(self) -> &'static str {
        match self {
            LocalSetting::Boolean => "boolean",
            LocalSetting::Choice(_) => "string",
            LocalSetting::Integer { .. } => "integer",
        }
    }

    /// 合规时返回规范化的值（`number` 类型里不带小数的值换成整数），否则 `None`。
    fn accept(self, value: &AccountPreferenceValue) -> Option<AccountPreferenceValue> {
        match (self, value) {
            (LocalSetting::Boolean, AccountPreferenceValue::Boolean(_)) => Some(value.clone()),
            (LocalSetting::Choice(choices), AccountPreferenceValue::String(text)) => {
                choices.contains(&text.as_str()).then(|| value.clone())
            }
            (LocalSetting::Integer { min, max, step }, AccountPreferenceValue::Integer(number)) => {
                ((min..=max).contains(number) && (number - min) % step == 0)
                    .then_some(AccountPreferenceValue::Integer(*number))
            }
            (LocalSetting::Integer { .. }, AccountPreferenceValue::Number(number))
                if number.is_finite() && number.fract() == 0.0 =>
            {
                self.accept(&AccountPreferenceValue::Integer(*number as i64))
            }
            _ => None,
        }
    }
}

/// 应用主题的 ID，与 [`crate::skin::app_theme::AppTheme`] 的选择器顺序相同，测试锁住两边一致。
const APP_THEME_IDS: [&str; 5] = ["siji", "chunya", "xiayin", "qiushan", "dongxue"];

/// Android 本地设置文件里参与同步的键（与服务端字段表的键相同）和允许的取值。这些设置只有 Android 宿主用，不在共享偏好里；隐私模式、开发者选项和语音数据贡献不在这张表里，永远不同步。
const ANDROID_LOCAL_SETTINGS: [(&str, LocalSetting); 17] = [
    ("general.app_theme", LocalSetting::Choice(&APP_THEME_IDS)),
    (
        "platform.android.one_handed",
        LocalSetting::Choice(&["off", "left", "right"]),
    ),
    ("platform.android.key_popup", LocalSetting::Boolean),
    ("platform.android.swipe_down_symbols", LocalSetting::Boolean),
    (
        "platform.android.swipe_symbols_direction",
        LocalSetting::Choice(&["down", "up"]),
    ),
    ("platform.android.space_cursor", LocalSetting::Boolean),
    ("platform.android.space_voice", LocalSetting::Boolean),
    (
        "platform.android.key_animation",
        LocalSetting::Choice(&["none", "bounce", "ripple", "glow", "lift"]),
    ),
    ("platform.android.toolbar_phrase", LocalSetting::Boolean),
    ("platform.android.toolbar_scheme", LocalSetting::Boolean),
    ("platform.android.toolbar_hidden", LocalSetting::Boolean),
    (
        "platform.android.handwriting_mode",
        LocalSetting::Choice(&["single", "overlap", "line"]),
    ),
    (
        "platform.android.handwriting_delay_ms",
        LocalSetting::Integer {
            min: 200,
            max: 1500,
            step: 100,
        },
    ),
    (
        "platform.android.handwriting_show_pinyin",
        LocalSetting::Boolean,
    ),
    (
        "platform.android.handwriting_stroke_color",
        LocalSetting::Choice(&["follow_skin", "black", "white", "blue"]),
    ),
    (
        "platform.android.handwriting_stroke_width",
        LocalSetting::Integer {
            min: 1,
            max: 8,
            step: 1,
        },
    ),
    (
        "platform.android.voice_offline_fallback",
        LocalSetting::Boolean,
    ),
];

fn local_setting(key: &str) -> Option<LocalSetting> {
    ANDROID_LOCAL_SETTINGS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, setting)| *setting)
}

/// 把宿主本地设置里参与同步的值加进导出结果。不在 [`ANDROID_LOCAL_SETTINGS`] 里的键和取值不合规的键不导出，按键名排序返回。
pub fn insert_android_local_settings(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    local: &BTreeMap<String, AccountPreferenceValue>,
) -> Vec<String> {
    let mut rejected = Vec::new();
    for (key, value) in local {
        match local_setting(key).and_then(|setting| setting.accept(value)) {
            Some(value) => {
                settings.insert(key.clone(), value);
            }
            None => rejected.push(key.clone()),
        }
    }
    rejected
}

/// 云端文档里属于 Android 本地设置的值：字段表收录且取值合规的交给宿主写回本地文件；取值不合规的键名按序返回，调用方把它们并进 `skipped`。字段表与本机期望的类型冲突时拒绝整份文档。
pub fn android_local_settings(
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
) -> Result<(BTreeMap<String, AccountPreferenceValue>, Vec<String>), AccountError> {
    let mut accepted = BTreeMap::new();
    let mut skipped = Vec::new();
    for (key, setting) in ANDROID_LOCAL_SETTINGS {
        let Some(value) = cloud.settings.get(key) else {
            continue;
        };
        if !supports_schema_field(schema, key, setting.schema_type())? {
            continue;
        }
        match setting.accept(value) {
            Some(value) => {
                accepted.insert(key.to_owned(), value);
            }
            None => skipped.push(key.to_owned()),
        }
    }
    Ok((accepted, skipped))
}

/// 自定义键盘皮肤库的 JSON 是否可以同步：不超过字节上限、是一个 JSON 数组。
pub fn valid_custom_keyboard_skins(value: &str) -> bool {
    value.len() <= MAX_CUSTOM_KEYBOARD_SKINS_BYTES
        && matches!(
            serde_json::from_str::<serde_json::Value>(value),
            Ok(serde_json::Value::Array(_))
        )
}

/// 把宿主的自定义键盘皮肤库加进导出结果；不合规（超长或不是数组）时不导出，返回假。
pub fn insert_custom_keyboard_skins(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    library: &str,
) -> bool {
    if !valid_custom_keyboard_skins(library) {
        return false;
    }
    insert_string(settings, CUSTOM_KEYBOARD_SKINS, library);
    true
}

/// 云端文档里的自定义键盘皮肤库：字段表收录、值是字符串时返回 `Ok(Some(Ok(值)))`；值不合规时返回 `Ok(Some(Err(())))`，调用方跳过它；没有这个键时为 `Ok(None)`。
pub fn custom_keyboard_skins(
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
) -> Result<Option<Result<String, ()>>, AccountError> {
    match cloud.settings.get(CUSTOM_KEYBOARD_SKINS) {
        Some(AccountPreferenceValue::String(value))
            if supports_schema_field(schema, CUSTOM_KEYBOARD_SKINS, "string")? =>
        {
            Ok(Some(if valid_custom_keyboard_skins(value) {
                Ok(value.clone())
            } else {
                Err(())
            }))
        }
        _ => Ok(None),
    }
}

/// 宿主本地的按键反馈设置，字段名与 Android 插件的 JSON 相同。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyboardFeedback {
    pub sound_enabled: bool,
    pub haptics_enabled: bool,
    pub haptic_strength: String,
}

/// 振动强度是否是移动端认识的三档之一。
pub fn valid_haptic_strength(value: &str) -> bool {
    matches!(value, "light" | "medium" | "strong")
}

/// 应用云端设置的结果。
#[derive(Clone, Debug, PartialEq)]
pub struct AndroidSettingsApplied {
    /// 应用之后的共享偏好，已经通过 `Preferences::validate`。
    pub preferences: Preferences,
    /// 云端文档里有按键反馈的键时，应用之后的按键反馈（调用方写回宿主存储）；没有时为 `None`，调用方什么也不写。
    pub feedback: Option<HostKeyboardFeedback>,
    /// 取值超出本机范围或是本机不认识的枚举值而没有应用的键，按键名排序。
    pub skipped: Vec<String>,
}

/// 方案在账号里的取值；账号 schema 还没有收录的方案为空。粤拼、注音、越南文、藏文和笔画不写，而不是映射到相近的方案，这样账号保留上次记录的方案，不会被改成用户没选过的方案。
pub fn account_input_schema(scheme: InputScheme) -> Option<&'static str> {
    match scheme {
        InputScheme::Quanpin => Some("quanpin"),
        InputScheme::Shuangpin => Some("shuangpin"),
        InputScheme::Wubi => Some("wubi"),
        InputScheme::Japanese => Some("japanese"),
        InputScheme::Korean => Some("korean"),
        InputScheme::Cantonese
        | InputScheme::Zhuyin
        | InputScheme::Vietnamese
        | InputScheme::Tibetan
        | InputScheme::Stroke => None,
    }
}

/// 五笔版本在账号里的取值，即 `input.wubi_schema`。
pub fn account_wubi_schema(profile: WubiProfile) -> &'static str {
    match profile {
        WubiProfile::Wubi86 => "wubi86",
        WubiProfile::Wubi98 => "wubi98",
    }
}

fn shuangpin_schema(profile: ShuangpinProfile) -> &'static str {
    match profile {
        ShuangpinProfile::Xiaohe => "xiaohe",
        ShuangpinProfile::Ziranma => "ziranma",
        ShuangpinProfile::Shoudao => "shoudao",
        ShuangpinProfile::Microsoft => "microsoft",
    }
}

fn keyboard_layout(layout: TouchKeyboardLayout) -> &'static str {
    match layout {
        TouchKeyboardLayout::TwentySixKey => "twenty_six_key",
        TouchKeyboardLayout::NineKey => "nine_key",
        TouchKeyboardLayout::Handwriting => "handwriting",
    }
}

fn theme_mode(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
        ThemeMode::System => "system",
    }
}

fn insert_string(settings: &mut BTreeMap<String, AccountPreferenceValue>, key: &str, value: &str) {
    settings.insert(
        key.to_owned(),
        AccountPreferenceValue::String(value.to_owned()),
    );
}

fn insert_bool(settings: &mut BTreeMap<String, AccountPreferenceValue>, key: &str, value: bool) {
    settings.insert(key.to_owned(), AccountPreferenceValue::Boolean(value));
}

fn insert_integer(settings: &mut BTreeMap<String, AccountPreferenceValue>, key: &str, value: i64) {
    settings.insert(key.to_owned(), AccountPreferenceValue::Integer(value));
}

/// 本机设置导出成账号文档的键值。调用方再按版本（`edition::filter_uploaded_account_settings`）和服务端字段表过滤，然后合并上传。`feedback` 为空时不导出按键反馈三个键。
pub fn export_android_settings(
    preferences: &Preferences,
    feedback: Option<&HostKeyboardFeedback>,
) -> Result<BTreeMap<String, AccountPreferenceValue>, AccountError> {
    let mut settings = BTreeMap::new();
    if let Some(schema) = account_input_schema(preferences.scheme) {
        insert_string(&mut settings, "input.schema", schema);
    }
    insert_string(
        &mut settings,
        "input.character_set",
        if preferences.traditional_chinese_output {
            "traditional"
        } else {
            "simplified"
        },
    );
    insert_string(
        &mut settings,
        "input.shuangpin_schema",
        shuangpin_schema(preferences.shuangpin_profile),
    );
    // 五笔版本只随五笔方案上传（与 iOS、鸿蒙一致）：上传是合并进账号文档的，不在五笔上时本机的缺省 86 不该盖掉账号里别的设备选的 98。
    if preferences.scheme == InputScheme::Wubi {
        insert_string(
            &mut settings,
            "input.wubi_schema",
            account_wubi_schema(preferences.wubi_profile),
        );
    }
    insert_bool(&mut settings, "input.learning", preferences.learning);
    insert_string(
        &mut settings,
        "input.frequency_mode",
        preferences.frequency.mode.as_str(),
    );
    insert_integer(
        &mut settings,
        "input.frequency_trigger_count",
        i64::from(preferences.frequency.trigger_count),
    );
    insert_integer(
        &mut settings,
        "input.frequency_linear_step",
        i64::from(preferences.frequency.linear_step),
    );
    insert_bool(
        &mut settings,
        "input.chinese_punctuation",
        preferences.chinese_punctuation,
    );
    insert_bool(
        &mut settings,
        "input.smart_punctuation",
        preferences.smart_punctuation,
    );
    insert_bool(
        &mut settings,
        "input.paired_punctuation",
        preferences.paired_punctuation,
    );
    insert_bool(
        &mut settings,
        "input.wubi_code_hint",
        preferences.wubi_code_hint,
    );
    insert_string(
        &mut settings,
        "platform.android.keyboard_layout",
        keyboard_layout(preferences.touch_keyboard_layout),
    );
    insert_theme_settings(&mut settings, preferences)?;
    insert_string(
        &mut settings,
        "platform.android.theme",
        theme_mode(preferences.theme),
    );
    insert_integer(
        &mut settings,
        "platform.android.touch_key_spacing_tenths",
        i64::from(preferences.touch_key_spacing_tenths),
    );
    insert_integer(
        &mut settings,
        "platform.android.touch_row_spacing_tenths",
        i64::from(preferences.touch_row_spacing_tenths),
    );
    insert_integer(
        &mut settings,
        "platform.android.keyboard_height_adjustment",
        i64::from(preferences.touch_keyboard_height_adjustment),
    );
    insert_bool(
        &mut settings,
        "platform.android.voice_shortcut",
        preferences.touch_voice_shortcut,
    );
    insert_new_android_settings(&mut settings, preferences);
    if let Some(feedback) = feedback {
        insert_bool(
            &mut settings,
            ANDROID_FEEDBACK_KEYS[0],
            feedback.sound_enabled,
        );
        insert_bool(
            &mut settings,
            ANDROID_FEEDBACK_KEYS[1],
            feedback.haptics_enabled,
        );
        insert_string(
            &mut settings,
            ANDROID_FEEDBACK_KEYS[2],
            &feedback.haptic_strength,
        );
    }
    Ok(settings)
}

/// 设计改版新增、仍在共享偏好里的键：按键音包、工具栏按钮和语音识别语言。其余新增的 Android 设置在宿主的本地设置文件里，见 [`insert_android_local_settings`]。
fn insert_new_android_settings(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    preferences: &Preferences,
) {
    let pack = &preferences.plugins.key_sound.pack;
    if pack.chars().count() <= MAX_KEY_SOUND_PACK_CHARS {
        insert_string(settings, "platform.android.key_sound_pack", pack);
    }
    let mut toolbar = preferences.touch_toolbar;
    for (index, key) in TOOLBAR_KEYS.iter().enumerate() {
        insert_bool(settings, key, *toolbar_member(&mut toolbar, index));
    }
    let language = &preferences.voice_input.language;
    if language.chars().count() <= MAX_VOICE_LANGUAGE_CHARS {
        insert_string(settings, "platform.android.voice_language", language);
    }
}

/// 全局主题、自定义主题的底色、它的键盘设计和外部候选窗口皮肤包。设计是 JSON，包是 id；两者都用空串表示「没有」，这样清除也能同步。
fn insert_theme_settings(
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

/// 应用云端文档之前是否需要先读出宿主的按键反馈：字段表和文档里都有其中某个键时才需要。
pub fn needs_host_feedback(cloud: &AccountPreferences, schema: &AccountPreferenceSchema) -> bool {
    ANDROID_FEEDBACK_KEYS
        .iter()
        .any(|key| schema.fields.contains_key(*key) && cloud.settings.contains_key(*key))
}

/// 字段表收录了 `key` 且类型是 `expected` 时为真，没有收录时为假，类型冲突时拒绝整份文档。
fn supports_schema_field(
    schema: &AccountPreferenceSchema,
    key: &str,
    expected: &str,
) -> Result<bool, AccountError> {
    match schema.fields.get(key) {
        None => Ok(false),
        Some(field)
            if field.value_type == expected
                || ((expected == "number" || expected == "integer")
                    && matches!(field.value_type.as_str(), "integer" | "number")) =>
        {
            Ok(true)
        }
        Some(_) => Err(AccountError::Invalid),
    }
}

/// 一次应用的进行状态：逐键改写偏好，取值不能用的键记进 `skipped`。
struct Applier<'a> {
    values: &'a BTreeMap<String, AccountPreferenceValue>,
    schema: &'a AccountPreferenceSchema,
    preferences: Preferences,
    feedback: Option<HostKeyboardFeedback>,
    skipped: Vec<String>,
}

impl Applier<'_> {
    fn string(&self, key: &str) -> Result<Option<String>, AccountError> {
        match self.values.get(key) {
            Some(AccountPreferenceValue::String(value))
                if supports_schema_field(self.schema, key, "string")? =>
            {
                Ok(Some(value.clone()))
            }
            _ => Ok(None),
        }
    }

    fn boolean(&self, key: &str) -> Result<Option<bool>, AccountError> {
        match self.values.get(key) {
            Some(AccountPreferenceValue::Boolean(value))
                if supports_schema_field(self.schema, key, "boolean")? =>
            {
                Ok(Some(*value))
            }
            _ => Ok(None),
        }
    }

    /// 整数值；`number` 类型的字段里带小数的值本机表示不了，返回 `Some(None)` 让调用方跳过。
    fn integer(&self, key: &str) -> Result<Option<Option<i64>>, AccountError> {
        let value = match self.values.get(key) {
            Some(AccountPreferenceValue::Integer(value)) => Some(*value),
            Some(AccountPreferenceValue::Number(value)) if value.is_finite() => {
                (value.fract() == 0.0).then_some(*value as i64)
            }
            _ => return Ok(None),
        };
        if !supports_schema_field(self.schema, key, "integer")? {
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// 用 `change` 改写偏好；它返回 `None`（不认识的取值）或改完之后 `Preferences::validate` 不通过（超出本机范围）时，撤销这一个键并记下它。
    fn set(&mut self, key: &str, change: impl FnOnce(&mut Preferences) -> Option<()>) {
        let mut next = self.preferences.clone();
        if change(&mut next).is_some() && next.validate().is_ok() {
            self.preferences = next;
        } else {
            self.skipped.push(key.to_owned());
        }
    }

    fn set_string(
        &mut self,
        key: &str,
        change: impl FnOnce(&mut Preferences, &str) -> Option<()>,
    ) -> Result<(), AccountError> {
        if let Some(value) = self.string(key)? {
            self.set(key, |preferences| change(preferences, &value));
        }
        Ok(())
    }

    fn set_bool(
        &mut self,
        key: &str,
        change: impl FnOnce(&mut Preferences, bool),
    ) -> Result<(), AccountError> {
        if let Some(value) = self.boolean(key)? {
            self.set(key, |preferences| {
                change(preferences, value);
                Some(())
            });
        }
        Ok(())
    }

    fn set_integer(
        &mut self,
        key: &str,
        change: impl FnOnce(&mut Preferences, i64) -> Option<()>,
    ) -> Result<(), AccountError> {
        match self.integer(key)? {
            None => {}
            Some(None) => self.skipped.push(key.to_owned()),
            Some(Some(value)) => self.set(key, |preferences| change(preferences, value)),
        }
        Ok(())
    }

    fn feedback_mut(&mut self) -> Result<&mut HostKeyboardFeedback, AccountError> {
        self.feedback.as_mut().ok_or(AccountError::Storage)
    }
}

/// 把云端文档应用到本机偏好。`feedback` 是宿主当前的按键反馈，[`needs_host_feedback`] 为真时必须传入，否则返回 `Storage`。调用方之前应当已经按版本过滤过文档（`edition::filter_downloaded_account_settings`）。
pub fn apply_android_settings(
    local: &Preferences,
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
    feedback: Option<HostKeyboardFeedback>,
) -> Result<AndroidSettingsApplied, AccountError> {
    super::validate_account_preferences(cloud)?;
    for (key, value) in &cloud.settings {
        if let Some(field) = schema.fields.get(key) {
            if field.value_type != value.kind()
                && !(field.value_type == "number" && value.kind() == "integer")
            {
                return Err(AccountError::Invalid);
            }
        }
    }
    let needs_feedback = needs_host_feedback(cloud, schema);
    let mut applier = Applier {
        values: &cloud.settings,
        schema,
        preferences: local.clone(),
        feedback: if needs_feedback { feedback } else { None },
        skipped: Vec::new(),
    };

    // 本机不提供的方案（较新设备上的粤拼、注音、越南文或笔画）保留本机的方案，文档里其他的键照常应用。
    applier.set_string("input.schema", |preferences, value| {
        preferences.scheme = match value {
            "quanpin" => InputScheme::Quanpin,
            "shuangpin" => InputScheme::Shuangpin,
            "wubi" => InputScheme::Wubi,
            "japanese" => InputScheme::Japanese,
            "korean" => InputScheme::Korean,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_string("input.character_set", |preferences, value| {
        preferences.traditional_chinese_output = match value {
            "traditional" => true,
            "simplified" => false,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_string("input.shuangpin_schema", |preferences, value| {
        preferences.shuangpin_profile = match value {
            "xiaohe" => ShuangpinProfile::Xiaohe,
            "ziranma" => ShuangpinProfile::Ziranma,
            "shoudao" => ShuangpinProfile::Shoudao,
            "microsoft" => ShuangpinProfile::Microsoft,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_string("input.wubi_schema", |preferences, value| {
        preferences.wubi_profile = match value {
            "wubi86" => WubiProfile::Wubi86,
            "wubi98" => WubiProfile::Wubi98,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_bool("input.learning", |preferences, value| {
        preferences.learning = value
    })?;
    applier.set_string("input.frequency_mode", |preferences, value| {
        preferences.frequency.mode = match value {
            "disabled" => FrequencyMode::Disabled,
            "pin" => FrequencyMode::Pin,
            "halve" => FrequencyMode::Halve,
            "linear" => FrequencyMode::Linear,
            "promote" => FrequencyMode::Promote,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_integer("input.frequency_trigger_count", |preferences, value| {
        preferences.frequency.trigger_count = u8::try_from(value).ok()?;
        Some(())
    })?;
    applier.set_integer("input.frequency_linear_step", |preferences, value| {
        preferences.frequency.linear_step = u8::try_from(value).ok()?;
        Some(())
    })?;
    applier.set_bool("input.chinese_punctuation", |preferences, value| {
        preferences.chinese_punctuation = value
    })?;
    applier.set_bool("input.smart_punctuation", |preferences, value| {
        preferences.smart_punctuation = value
    })?;
    applier.set_bool("input.paired_punctuation", |preferences, value| {
        preferences.paired_punctuation = value
    })?;
    applier.set_bool("input.wubi_code_hint", |preferences, value| {
        preferences.wubi_code_hint = value
    })?;
    applier.set_string("platform.android.keyboard_layout", |preferences, value| {
        preferences.touch_keyboard_layout = match value {
            "twenty_six_key" => TouchKeyboardLayout::TwentySixKey,
            "nine_key" => TouchKeyboardLayout::NineKey,
            "handwriting" => TouchKeyboardLayout::Handwriting,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_string(GLOBAL_THEME, |preferences, value| {
        preferences.global_theme = GlobalTheme::from_id(value)?;
        Some(())
    })?;
    applier.set_string(CUSTOM_THEME_BASE, |preferences, value| {
        preferences.custom_theme.base =
            GlobalTheme::from_id(value).filter(|base| base.is_base())?;
        Some(())
    })?;
    applier.set_string(CUSTOM_KEYBOARD_SKIN, |preferences, value| {
        preferences.custom_theme.keyboard = if value.is_empty() {
            None
        } else {
            Some(serde_json::from_str(value).ok()?)
        };
        Some(())
    })?;
    applier.set_string(CUSTOM_CANDIDATE_SKIN, |preferences, value| {
        if !value.is_empty() && !crate::skin::catalog::is_selectable_id(value) {
            return None;
        }
        preferences.custom_theme.candidate_skin = (!value.is_empty()).then(|| value.to_owned());
        Some(())
    })?;
    applier.set_string("platform.android.theme", |preferences, value| {
        preferences.theme = match value {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            "system" => ThemeMode::System,
            _ => return None,
        };
        Some(())
    })?;
    applier.set_integer(
        "platform.android.touch_key_spacing_tenths",
        |preferences, value| {
            preferences.touch_key_spacing_tenths = u8::try_from(value).ok()?;
            Some(())
        },
    )?;
    applier.set_integer(
        "platform.android.touch_row_spacing_tenths",
        |preferences, value| {
            preferences.touch_row_spacing_tenths = u8::try_from(value).ok()?;
            Some(())
        },
    )?;
    applier.set_integer(
        "platform.android.keyboard_height_adjustment",
        |preferences, value| {
            preferences.touch_keyboard_height_adjustment = i8::try_from(value).ok()?;
            Some(())
        },
    )?;
    applier.set_bool("platform.android.voice_shortcut", |preferences, value| {
        preferences.touch_voice_shortcut = value
    })?;

    apply_new_android_settings(&mut applier)?;

    if let Some(value) = applier.boolean(ANDROID_FEEDBACK_KEYS[0])? {
        applier.feedback_mut()?.sound_enabled = value;
    }
    if let Some(value) = applier.boolean(ANDROID_FEEDBACK_KEYS[1])? {
        applier.feedback_mut()?.haptics_enabled = value;
    }
    if let Some(value) = applier.string(ANDROID_FEEDBACK_KEYS[2])? {
        if valid_haptic_strength(&value) {
            applier.feedback_mut()?.haptic_strength = value;
        } else {
            applier.skipped.push(ANDROID_FEEDBACK_KEYS[2].to_owned());
        }
    }

    let Applier {
        preferences,
        feedback,
        mut skipped,
        ..
    } = applier;
    preferences.validate().map_err(|_| AccountError::Invalid)?;
    skipped.sort();
    Ok(AndroidSettingsApplied {
        preferences,
        feedback,
        skipped,
    })
}

/// [`insert_new_android_settings`] 的反方向。取值不认识或超出本机范围时跳过这一个键。
fn apply_new_android_settings(applier: &mut Applier<'_>) -> Result<(), AccountError> {
    applier.set_string("platform.android.key_sound_pack", |preferences, value| {
        if value.chars().count() > MAX_KEY_SOUND_PACK_CHARS {
            return None;
        }
        preferences.plugins.key_sound.pack = value.to_owned();
        Some(())
    })?;
    for (index, key) in TOOLBAR_KEYS.iter().enumerate() {
        applier.set_bool(key, |preferences, value| {
            *toolbar_member(&mut preferences.touch_toolbar, index) = value
        })?;
    }
    applier.set_string("platform.android.voice_language", |preferences, value| {
        if value.chars().count() > MAX_VOICE_LANGUAGE_CHARS {
            return None;
        }
        preferences.voice_input.language = value.to_owned();
        Some(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests;
