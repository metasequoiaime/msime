//! 账号设置文档（`/v1/users/me/preferences` 的 `settings`）与鸿蒙宿主之间的映射。原来写在鸿蒙的 `AccountPreferencePlan.ts` 里，搬到这里以后 `msime_client_account_harmony_settings_*` 和 `msime_client_cloud_sync` 共用这一份。
//!
//! 鸿蒙的本机设置就是共享偏好（宿主经共享 NAPI 读写的同一份 `PreferencesStore`），按键音、振动开关和振动强度在宿主自己的反馈文件里，由调用方作为 [`HostKeyboardFeedback`] 传入、应用后写回。所以映射与 Android 是同一套（[`super::settings_sync`]），只是平台键放在 `platform.harmony.*` 下：鸿蒙手机和 Android 手机是两台各有键盘的设备，共用一个命名空间会让一台改掉另一台的皮肤和键距。设计改版新增、只有 Android 才有的键（工具栏、按键音包、语音语言、本地设置文件、自定义键盘皮肤库）鸿蒙不导出也不应用。
//!
//! 服务端字段表还没有收录 `platform.harmony.*` 时，上传前按字段表过滤会去掉它们，应用时也不读它们，只有各平台共有的 `input.*` 键同步。
//!
//! 与 TS 版不同的地方都以 Rust 为准：云端某个键的取值本机不认识或超出范围时只跳过这一个键并在 `skipped` 里列出，不拒绝整份文档（与 Android 相同）；值的类型与字段表不符、字段表与本机期望的类型冲突、字段表本身不合规时仍然拒绝整份文档。

use super::settings_sync::{
    apply_touch_settings, insert_input_settings, insert_string, insert_touch_settings,
    needs_feedback, valid_haptic_strength, HostKeyboardFeedback, TouchHostKeys,
};
use super::{
    validate_preference_schema, AccountError, AccountPreferenceSchema, AccountPreferenceValue,
    AccountPreferences,
};
use crate::host_surface::HostPlatform;
use crate::preferences::Preferences;
use crate::skin::theme::GlobalTheme;
use std::collections::BTreeMap;

/// 按键反馈三个键，值来自鸿蒙宿主的反馈文件而不是共享偏好。
pub const HARMONY_FEEDBACK_KEYS: [&str; 3] = [
    "platform.harmony.sound_enabled",
    "platform.harmony.haptics_enabled",
    "platform.harmony.haptic_strength",
];

/// 鸿蒙的触屏布局键。
pub const HARMONY_KEYBOARD_LAYOUT: &str = "platform.harmony.keyboard_layout";

/// 鸿蒙的全局主题键。
const HARMONY_GLOBAL_THEME: &str = "platform.harmony.global_theme";

const HARMONY_KEYS: TouchHostKeys = TouchHostKeys {
    platform: HostPlatform::Harmony,
    keyboard_layout: HARMONY_KEYBOARD_LAYOUT,
    // 14 键先不上传：已经发出去的鸿蒙版本读到 `fourteen_key` 会拒收整份文档。上传是把本机的键盖在云端文档上，省略这个键等于保留云端原值；写成 26 键则会把同账号的其他鸿蒙设备切回 26 键。本机是 14 键时应用也跳过这个键。18 键由新版本识别，按共享布局映射正常上传。
    uploads_fourteen_key: false,
    global_theme: HARMONY_GLOBAL_THEME,
    custom_theme_base: "platform.harmony.custom_theme_base",
    custom_keyboard_skin: "platform.harmony.custom_keyboard_skin",
    custom_candidate_skin: "platform.harmony.custom_candidate_skin",
    custom_candidate_skin_dark: "platform.harmony.custom_candidate_skin_dark",
    theme: "platform.harmony.theme",
    touch_key_spacing_tenths: "platform.harmony.touch_key_spacing_tenths",
    touch_row_spacing_tenths: "platform.harmony.touch_row_spacing_tenths",
    keyboard_height_adjustment: "platform.harmony.keyboard_height_adjustment",
    voice_shortcut: "platform.harmony.voice_shortcut",
    number_keypad_order: "platform.harmony.number_keypad_order",
    twenty_six_key_number_layout: "platform.harmony.twenty_six_key_number_layout",
    shuangpin_key_hints: "platform.harmony.shuangpin_key_hints",
    feedback: HARMONY_FEEDBACK_KEYS,
};

/// 应用云端设置到鸿蒙宿主的结果。
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonySettingsApplied {
    /// 应用之后的共享偏好，已经通过 `Preferences::validate`。
    pub preferences: Preferences,
    /// 字段表和云端文档里都有按键反馈的键时，应用之后的按键反馈（调用方写回反馈文件）；没有时为 `None`，调用方什么也不写。
    pub feedback: Option<HostKeyboardFeedback>,
    /// 取值超出本机范围或是本机不认识的枚举值而没有应用的键，按键名排序。
    pub skipped: Vec<String>,
}

/// 本机设置导出成账号文档的键值。调用方再按版本（`edition::filter_uploaded_account_settings`）和服务端字段表过滤，然后合并上传。`feedback` 为空时不导出按键反馈三个键。
pub fn export_harmony_settings(
    preferences: &Preferences,
    feedback: Option<&HostKeyboardFeedback>,
) -> Result<BTreeMap<String, AccountPreferenceValue>, AccountError> {
    let mut settings = BTreeMap::new();
    insert_input_settings(&mut settings, preferences);
    // 本机是 14 键时不导出触屏布局，见 `HARMONY_KEYS.uploads_fourteen_key`；18 键按共享布局映射导出。
    insert_touch_settings(&mut settings, preferences, feedback, &HARMONY_KEYS)?;
    // 鸿蒙不提供的全局主题（只在 iOS 的 `native`）在鸿蒙上按 `system` 绘制，也按 `system` 上传：已经发出去的鸿蒙版本只认七个主题 id，收到别的会拒收整份文档。
    if !preferences
        .global_theme
        .is_offered_on(HostPlatform::Harmony)
    {
        insert_string(
            &mut settings,
            HARMONY_GLOBAL_THEME,
            GlobalTheme::System.id(),
        );
    }
    // 反馈文件里的振动强度不认得时按缺省的 `medium` 上传，理由同上。
    if feedback.is_some_and(|feedback| !valid_haptic_strength(&feedback.haptic_strength)) {
        insert_string(&mut settings, HARMONY_FEEDBACK_KEYS[2], "medium");
    }
    Ok(settings)
}

/// 应用云端文档之前是否需要先读出宿主的按键反馈：字段表和文档里都有其中某个键时才需要。
pub fn needs_harmony_feedback(
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
) -> bool {
    needs_feedback(cloud, schema, &HARMONY_FEEDBACK_KEYS)
}

/// 把云端文档应用到本机偏好，只读各平台共有的 `input.*` 键和 `platform.harmony.*`，别的平台的键（包括同名的 `platform.android.*`）不动。`feedback` 是宿主当前的按键反馈，[`needs_harmony_feedback`] 为真时必须传入，否则返回 `Storage`。字段表不合规时是 `Unavailable`。调用方之前应当已经按版本过滤过文档（`edition::filter_downloaded_account_settings`）。
pub fn apply_harmony_settings(
    local: &Preferences,
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
    feedback: Option<HostKeyboardFeedback>,
) -> Result<HarmonySettingsApplied, AccountError> {
    validate_preference_schema(schema)?;
    let (preferences, feedback, skipped) =
        apply_touch_settings(local, cloud, schema, feedback, &HARMONY_KEYS, |_| Ok(()))?;
    Ok(HarmonySettingsApplied {
        preferences,
        feedback,
        skipped,
    })
}

#[cfg(test)]
mod tests;
