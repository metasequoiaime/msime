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
  // Engine 自己的英文模式（Ctrl+Shift+E），由 Server 在焦点会话上设置。
  ToggleDedicatedEnglish,
  // 存储的 traditional_chinese_output，走工具栏简繁按钮用的同一个工作线程。
  ToggleTraditionalOutput,
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
  SelectStroke,
  // Settings pages.
  OpenTheme,
  OpenDictionary,
  // 悬浮工具栏右键菜单的几行，与 macOS 工具栏设置按钮的实用菜单一一对应：系统表情面板（Win+.）、检查更新、访问官网、使用帮助、问题反馈、隐藏悬浮状态栏（写回偏好）。
  OpenSystemEmoji,
  CheckForUpdates,
  OpenWebsite,
  OpenHelp,
  OpenFeedback,
  HideFloatingToolbar,
  // 托盘卡片的「云剪贴板…」：共享应用的云剪贴板面板，焦点在密码框里时拒绝打开。
  OpenCloudClipboard,
  // 主题页的一行，写存储的 global_theme；行的 value 是主题 id。
  SelectTheme,
  // 托盘卡片自己的翻页：输入方案页、主题页和返回主页。只在卡片里处理，从不交给 Server。
  ShowSchemes,
  ShowThemes,
  ShowMain,
};
} // namespace msime::windows
