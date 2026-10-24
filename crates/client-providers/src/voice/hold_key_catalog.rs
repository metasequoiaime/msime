//! 按住录音选择器的公开目录；默认值来自共享偏好，不读取用户设置。

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VoiceHoldKeyPreset {
    pub key: &'static str,
    pub macos_label: &'static str,
    pub desktop_label: &'static str,
    pub other_label: &'static str,
    pub macos_action: &'static str,
    pub desktop_action: &'static str,
    pub other_action: &'static str,
    pub fallback: bool,
}

pub fn hold_keys() -> [VoiceHoldKeyPreset; 3] {
    let defaults = msime_client_core::preferences::VoiceInputPreferences::default();
    [
        VoiceHoldKeyPreset {
            key: "hotkey_ralt",
            macos_label: "右 Option",
            desktop_label: "右 Alt",
            other_label: "右 Alt",
            macos_action: "按住右 Option 录音",
            desktop_action: "长按右 Alt 录音",
            other_action: "右 Alt 切换语音",
            fallback: defaults.hotkey_ralt,
        },
        VoiceHoldKeyPreset {
            key: "hotkey_rctrl_ralt",
            macos_label: "右 Control+右 Option",
            desktop_label: "右 Ctrl+右 Alt",
            other_label: "Ctrl+右 Alt",
            macos_action: "按住右 Control+右 Option 录音",
            desktop_action: "长按右 Ctrl+右 Alt 录音",
            other_action: "Ctrl+右 Alt 切换语音",
            fallback: defaults.hotkey_rctrl_ralt,
        },
        VoiceHoldKeyPreset {
            key: "hotkey_ctrl_win",
            macos_label: "Control+Command",
            desktop_label: "Ctrl+Win",
            other_label: "Ctrl+Win",
            macos_action: "按住 Control+Command 录音",
            desktop_action: "长按 Ctrl+Win 录音",
            other_action: "Ctrl+Win 切换语音",
            fallback: defaults.hotkey_ctrl_win,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::preferences::VoiceInputPreferences;

    #[test]
    fn hold_key_catalog_preserves_order_and_platform_labels() {
        let catalog = hold_keys();
        assert_eq!(
            catalog.each_ref().map(|entry| entry.key),
            ["hotkey_ralt", "hotkey_rctrl_ralt", "hotkey_ctrl_win"]
        );
        assert_eq!(
            catalog.each_ref().map(|entry| entry.macos_label),
            ["右 Option", "右 Control+右 Option", "Control+Command"]
        );
        assert_eq!(
            catalog.each_ref().map(|entry| entry.desktop_label),
            ["右 Alt", "右 Ctrl+右 Alt", "Ctrl+Win"]
        );
        assert_eq!(
            catalog.each_ref().map(|entry| entry.other_label),
            ["右 Alt", "Ctrl+右 Alt", "Ctrl+Win"]
        );
    }

    #[test]
    fn hold_key_catalog_reads_shared_preference_defaults() {
        let defaults = VoiceInputPreferences::default();
        assert_eq!(
            hold_keys().each_ref().map(|entry| entry.fallback),
            [
                defaults.hotkey_ralt,
                defaults.hotkey_rctrl_ralt,
                defaults.hotkey_ctrl_win
            ]
        );
    }
}
