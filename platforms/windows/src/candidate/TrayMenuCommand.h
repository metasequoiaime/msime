#pragma once

namespace msime::windows {
// What a tray menu row asks the Server to do. Kept apart from the menu model so the mode command mapping can name these without pulling in the menu's labels, which are UTF-8 text that only the menu targets compile with /utf-8.
enum class TrayMenuCommand {
  ToggleFloatingToolbar,
  OpenEmojiPanel,
  OpenHandwritingPanel,
  OpenKeyboardPanel,
  ToggleVoiceInput,
  OpenSettings,
  OpenAbout,
  // Input mode rows, sent to the focused TIP over the same worker protocol the floating toolbar uses.
  SelectChinese,
  SelectEnglish,
  ToggleFullwidth,
  ToggleChinesePunctuation,
  // Stored preferences the Server writes through the revisioned store.
  ToggleTranslations,
  SelectQuanpin,
  SelectShuangpin,
  SelectWubi,
  SelectJapanese,
  SelectKorean,
  SelectCantonese,
  SelectZhuyin,
  SelectVietnamese,
  SelectTibetan,
  // Settings pages.
  OpenTheme,
  OpenDictionary,
};
} // namespace msime::windows
