#pragma once

#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/CloudDictionaryWorker.h"

namespace msime::windows::win7 {
enum CloudDictionaryPanelControl { cloud_dictionary_panel_body = 600, cloud_dictionary_panel_notice };
class CloudDictionaryWindow final {
public:
  using FilePicker = msime::settings::win7::SettingsWindow::FilePicker;
  using Confirmation = msime::settings::win7::SettingsWindow::Confirmation;
  explicit CloudDictionaryWindow(FilePicker file_picker = {}, Confirmation confirmation = {}, msime::settings::win7::CloudDictionaryServices services = {});
  ~CloudDictionaryWindow();
  CloudDictionaryWindow(const CloudDictionaryWindow&) = delete;
  CloudDictionaryWindow& operator=(const CloudDictionaryWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows::win7
