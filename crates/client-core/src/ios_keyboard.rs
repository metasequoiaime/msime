//! iOS 键盘扩展在 App Group 里的设置：键盘每次重画都要读，所以不放在共享偏好文档里。Tauri 移动壳的插件、账号设置的 iOS 映射（client-services 的 `account::ios_settings`）和 C ABI 用的都是这一个形状，字段名是插件 JSON 的 camelCase。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `custom_keyboard_skin` 的字节上限。
pub const MAX_IOS_CUSTOM_KEYBOARD_SKIN_BYTES: usize = 800_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IosKeyboardPreferences {
    pub input_scheme: String,
    pub traditional_chinese_output: bool,
    pub sound_enabled: bool,
    pub haptics_enabled: bool,
    pub haptic_strength: String,
    pub english_suggestions: bool,
    /// The candidate strip draws the shared desktop candidate skin and colours instead of the keyboard skin's; the switch lives in the App Group because the keyboard reads it on every redraw.
    pub candidate_palette_follows_desktop: bool,
    /// 行内预编辑: the keyboard also writes the composition into the text field as marked text. Off by default and kept in the App Group, because the shared `tsf_preedit_style` defaults to raw in every document and would switch every existing iOS user over.
    pub inline_preedit: bool,
    /// Whether this device can vibrate for key presses: false on iPad, which has no Taptic Engine. Read-only; the plugin reports it and never stores it.
    pub haptics_available: bool,
    /// 数字行与 Tab 键 on the iPad full-width keyboard, kept in the App Group. The plugin reports it only on an iPad and writes it only when present, so a phone neither shows nor stores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet_full_keys: Option<bool>,
    /// 横屏分离式键盘：iPad 全宽键盘横屏时把键区分成左右两半，存在 App Group 的 `keyboard.tablet.split`，默认关。和 `tablet_full_keys` 一样只在 iPad 上报告、只在请求里带着时才写入。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet_split_keyboard: Option<bool>,
    pub dictionary_learning: bool,
    /// 全局主题 id（`Preferences::global_theme`），存在 App Group 的 `globalTheme` 下，键盘扩展不读偏好文档也能拿到。只有八个主题 id 有效。
    pub global_theme: String,
    /// The custom theme's keyboard design (`Preferences::custom_theme.keyboard`) as JSON; `None` when the custom theme has no design and draws its base theme's keyboard. The App Group keeps it under `customKeyboardSkin.v1`.
    pub custom_keyboard_skin: Option<String>,
}

impl IosKeyboardPreferences {
    pub fn is_valid(&self) -> bool {
        matches!(
            self.input_scheme.as_str(),
            "quanpin"
                | "nineKey"
                | "shuangpin"
                | "ziranma"
                | "microsoft"
                | "shoudao"
                | "wubi"
                | "japaneseNineKey"
                | "japanese"
                | "korean"
                | "handwriting"
                | "cantonese"
                | "zhuyin"
                | "vietnamese"
                | "tibetan"
                | "stroke"
                | "fourteenKey"
                | "nineKeyShuangpin"
                | "eighteenKey"
        ) && matches!(
            self.haptic_strength.as_str(),
            "light" | "medium" | "strong" | "system"
        )
            // 全局主题 id 直接按 client-core 的 `theme::GlobalTheme` 校验，不另抄一份列表。
            && crate::theme::GlobalTheme::from_id(&self.global_theme).is_some()
            && self.custom_keyboard_skin.as_ref().is_none_or(|value| {
                value.len() <= MAX_IOS_CUSTOM_KEYBOARD_SKIN_BYTES
                    && serde_json::from_str::<Value>(value)
                        .is_ok_and(|document| document.is_object())
            })
    }
}
