#pragma once
#include "ReplyCodec.h"
#include "ToolbarIcons.h"
#include "TrayMenuCommand.h"

namespace msime::windows {
// The worker protocol sets modes explicitly; it has no toggle opcode.
// Unknown is not false: wait for the focused TIP's reported state.
inline std::optional<WorkerMode> toolbar_mode_command(
    int button, std::optional<bool> chinese, std::optional<bool> fullwidth,
    std::optional<bool> chinese_punctuation) {
  switch (button) {
  case kToolbarLanguage:
    if (chinese) return *chinese ? WorkerMode::English : WorkerMode::Chinese;
    break;
  case kToolbarFullwidth:
    if (fullwidth) return *fullwidth ? WorkerMode::Halfwidth : WorkerMode::Fullwidth;
    break;
  case kToolbarPunctuation:
    if (chinese_punctuation)
      return *chinese_punctuation ? WorkerMode::AsciiPunctuation
                                 : WorkerMode::ChinesePunctuation;
    break;
  default:
    break;
  }
  return std::nullopt;
}
// A tray mode row resolved against the focused TIP's reported state.
struct TrayMenuModeRequest {
  // False when the state the row depends on is unknown: nothing is sent and the row reports failure, as a toolbar button does.
  bool known = false;
  // What to send. Empty while known means the TIP is already in the state the row names, so the row succeeds without a command.
  std::optional<WorkerMode> mode;
};
// The language rows name a state rather than toggling, so a row already in its state sends nothing. Everything that is sent is exactly what the toolbar's button would send from the same state, so the rows ride the same Server-to-TSF worker commands and the mode worker's handling of the Engine's English mode applies unchanged.
inline TrayMenuModeRequest
tray_menu_mode_command(TrayMenuCommand command, std::optional<bool> chinese,
                       std::optional<bool> fullwidth,
                       std::optional<bool> chinese_punctuation,
                       bool dedicated_english) {
  switch (command) {
  case TrayMenuCommand::SelectChinese:
    if (!chinese)
      return {};
    // In the Engine's English mode the TIP may still report Chinese; the toolbar's language button then sends English, which the mode worker turns into leaving that mode.
    if (*chinese && !dedicated_english)
      return {true, std::nullopt};
    return {true, toolbar_mode_command(kToolbarLanguage, chinese, fullwidth,
                                       chinese_punctuation)};
  case TrayMenuCommand::SelectEnglish:
    if (!chinese)
      return {};
    if (!*chinese || dedicated_english)
      return {true, std::nullopt};
    return {true, toolbar_mode_command(kToolbarLanguage, chinese, fullwidth,
                                       chinese_punctuation)};
  case TrayMenuCommand::ToggleFullwidth:
    if (!fullwidth)
      return {};
    return {true, toolbar_mode_command(kToolbarFullwidth, chinese, fullwidth,
                                       chinese_punctuation)};
  case TrayMenuCommand::ToggleChinesePunctuation:
    if (!chinese_punctuation)
      return {};
    return {true, toolbar_mode_command(kToolbarPunctuation, chinese, fullwidth,
                                       chinese_punctuation)};
  case TrayMenuCommand::ToggleFloatingToolbar:
  case TrayMenuCommand::OpenEmojiPanel:
  case TrayMenuCommand::OpenHandwritingPanel:
  case TrayMenuCommand::OpenKeyboardPanel:
  case TrayMenuCommand::ToggleVoiceInput:
  case TrayMenuCommand::OpenSettings:
  case TrayMenuCommand::OpenAbout:
  case TrayMenuCommand::ToggleTranslations:
  case TrayMenuCommand::SelectQuanpin:
  case TrayMenuCommand::SelectShuangpin:
  case TrayMenuCommand::SelectWubi:
  case TrayMenuCommand::SelectJapanese:
  case TrayMenuCommand::SelectKorean:
  case TrayMenuCommand::SelectCantonese:
  case TrayMenuCommand::SelectZhuyin:
  case TrayMenuCommand::SelectVietnamese:
  case TrayMenuCommand::SelectTibetan:
  case TrayMenuCommand::SelectStroke:
  case TrayMenuCommand::OpenTheme:
  case TrayMenuCommand::OpenDictionary:
  case TrayMenuCommand::OpenSystemEmoji:
  case TrayMenuCommand::CheckForUpdates:
  case TrayMenuCommand::OpenWebsite:
  case TrayMenuCommand::OpenHelp:
  case TrayMenuCommand::OpenFeedback:
  case TrayMenuCommand::HideFloatingToolbar:
  case TrayMenuCommand::ToggleDedicatedEnglish:
  case TrayMenuCommand::ToggleTraditionalOutput:
  case TrayMenuCommand::OpenCloudClipboard:
  case TrayMenuCommand::SelectTheme:
  case TrayMenuCommand::ShowSchemes:
  case TrayMenuCommand::ShowThemes:
  case TrayMenuCommand::ShowMain:
    break;
  }
  return {};
}
// Whether a tray row is one of the mode rows above.
inline bool tray_menu_mode_row(TrayMenuCommand command) {
  return command == TrayMenuCommand::SelectChinese ||
         command == TrayMenuCommand::SelectEnglish ||
         command == TrayMenuCommand::ToggleFullwidth ||
         command == TrayMenuCommand::ToggleChinesePunctuation;
}
} // namespace msime::windows
