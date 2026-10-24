#pragma once

#include "../../settings/win7/SettingsWindow.h"

namespace msime::windows::win7 {
enum VocabularyPanelControl { vocabulary_panel_body = 500, vocabulary_panel_notice };
class VocabularyWindow final {
public:
  using FilePicker = msime::settings::win7::SettingsWindow::FilePicker;
  using Confirmation = msime::settings::win7::SettingsWindow::Confirmation;
  explicit VocabularyWindow(FilePicker file_picker = {}, Confirmation confirmation = {});
  ~VocabularyWindow();
  VocabularyWindow(const VocabularyWindow&) = delete;
  VocabularyWindow& operator=(const VocabularyWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows::win7
