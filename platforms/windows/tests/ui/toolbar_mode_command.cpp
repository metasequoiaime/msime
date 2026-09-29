#include "../../src/candidate/ToolbarModeCommand.h"
#include <cassert>
#include <tuple>
using namespace msime::windows;
int main() {
  for (const auto &[button, on, off] : {
      std::tuple{kToolbarLanguage, FanyImeWorkerReplyType::SwitchToEnglish,
                  FanyImeWorkerReplyType::SwitchToChinese},
      std::tuple{kToolbarFullwidth, FanyImeWorkerReplyType::SwitchToHalfwidth,
                  FanyImeWorkerReplyType::SwitchToFullwidth},
      std::tuple{kToolbarPunctuation, FanyImeWorkerReplyType::SwitchToPuncEn,
                  FanyImeWorkerReplyType::SwitchToPuncCn}}) {
    for (bool state : {false, true}) {
      // Only the selected button's state is required, not unrelated states.
      const auto mode = toolbar_mode_command(button,
          button == kToolbarLanguage ? std::optional{state} : std::nullopt,
          button == kToolbarFullwidth ? std::optional{state} : std::nullopt,
          button == kToolbarPunctuation ? std::optional{state} : std::nullopt);
      assert(mode);
      const auto bytes = worker_mode_bytes(*mode);
      assert(bytes && bytes->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
      const uint32_t opcode = state ? on : off;
      for (size_t i = 0; i < 4; ++i)
        assert(bytes->at(i) == ((opcode >> (8 * i)) & 0xff));
      for (size_t i = 4; i < bytes->size(); ++i) assert(bytes->at(i) == 0);
    }
    assert(!toolbar_mode_command(button,
        button == kToolbarLanguage ? std::nullopt : std::optional{true},
        button == kToolbarFullwidth ? std::nullopt : std::optional{true},
        button == kToolbarPunctuation ? std::nullopt : std::optional{true}));
  }
  for (int button : {-1, 3, 4, 5, 6, 7, 8, 9, 10, 99})
    assert(!toolbar_mode_command(button, true, true, true));

  // Tray language rows name a state: a row already in it sends nothing and still succeeds, and any command sent is the toolbar's own.
  {
    const std::optional<bool> unknown;
    auto tray = [](TrayMenuCommand command, std::optional<bool> chinese,
                   std::optional<bool> fullwidth, std::optional<bool> punctuation,
                   bool dedicated) {
      return tray_menu_mode_command(command, chinese, fullwidth, punctuation,
                                    dedicated);
    };
    auto sends = [](const TrayMenuModeRequest &request, WorkerMode mode) {
      return request.known && request.mode && *request.mode == mode;
    };
    auto idle = [](const TrayMenuModeRequest &request) {
      return request.known && !request.mode;
    };
    assert(sends(tray(TrayMenuCommand::SelectChinese, false, unknown, unknown, false),
                 WorkerMode::Chinese));
    assert(idle(tray(TrayMenuCommand::SelectChinese, true, unknown, unknown, false)));
    assert(sends(tray(TrayMenuCommand::SelectEnglish, true, unknown, unknown, false),
                 WorkerMode::English));
    assert(idle(tray(TrayMenuCommand::SelectEnglish, false, unknown, unknown, false)));
    // The Engine's English mode: the TIP reports Chinese, the row shows English, and the Chinese row sends what the toolbar sends, English, which the mode worker turns into leaving that mode.
    assert(sends(tray(TrayMenuCommand::SelectChinese, true, unknown, unknown, true),
                 WorkerMode::English));
    assert(idle(tray(TrayMenuCommand::SelectEnglish, true, unknown, unknown, true)));
    assert(sends(tray(TrayMenuCommand::SelectChinese, false, unknown, unknown, true),
                 WorkerMode::Chinese));
    // Switches flip the reported state.
    assert(sends(tray(TrayMenuCommand::ToggleFullwidth, unknown, false, unknown, false),
                 WorkerMode::Fullwidth));
    assert(sends(tray(TrayMenuCommand::ToggleFullwidth, unknown, true, unknown, false),
                 WorkerMode::Halfwidth));
    assert(sends(tray(TrayMenuCommand::ToggleChinesePunctuation, unknown, unknown,
                      false, false),
                 WorkerMode::ChinesePunctuation));
    assert(sends(tray(TrayMenuCommand::ToggleChinesePunctuation, unknown, unknown,
                      true, false),
                 WorkerMode::AsciiPunctuation));
    // Unknown is not false: nothing is sent and the row reports failure.
    for (auto command : {TrayMenuCommand::SelectChinese, TrayMenuCommand::SelectEnglish})
      assert(!tray(command, unknown, true, true, false).known);
    assert(!tray(TrayMenuCommand::ToggleFullwidth, true, unknown, true, false).known);
    assert(!tray(TrayMenuCommand::ToggleChinesePunctuation, true, true, unknown, false)
                .known);
    // Rows that are not modes never reach the worker.
    for (auto command :
         {TrayMenuCommand::ToggleFloatingToolbar, TrayMenuCommand::OpenEmojiPanel,
          TrayMenuCommand::OpenHandwritingPanel, TrayMenuCommand::OpenKeyboardPanel,
          TrayMenuCommand::ToggleVoiceInput, TrayMenuCommand::OpenSettings,
          TrayMenuCommand::OpenAbout, TrayMenuCommand::ToggleTranslations,
          TrayMenuCommand::SelectQuanpin, TrayMenuCommand::SelectShuangpin,
          TrayMenuCommand::SelectWubi, TrayMenuCommand::SelectJapanese,
          TrayMenuCommand::OpenTheme, TrayMenuCommand::OpenDictionary}) {
      assert(!tray(command, true, true, true, false).known);
      assert(!tray_menu_mode_row(command));
    }
    for (auto command : {TrayMenuCommand::SelectChinese, TrayMenuCommand::SelectEnglish,
                         TrayMenuCommand::ToggleFullwidth,
                         TrayMenuCommand::ToggleChinesePunctuation})
      assert(tray_menu_mode_row(command));
  }
}
