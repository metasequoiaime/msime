#pragma once

#include "ClipboardWindow.h"
#include "../../settings/win7/SettingsWindow.h"

namespace msime::windows::win7 {
enum CloudClipboardPanelControl { cloud_clipboard_panel_body = 400, cloud_clipboard_panel_insert, cloud_clipboard_panel_notice };
class CloudClipboardWindow final {
public:
  using InsertText = ClipboardWindow::InsertText;
  using CopyText = ClipboardWindow::CopyText;
  using Transport = ClipboardWindow::Transport;
  using Confirmation = msime::settings::win7::SettingsWindow::Confirmation;
  using Services = msime::settings::win7::SettingsWindow::CloudClipboardServices;
  explicit CloudClipboardWindow(InsertText insert = {}, CopyText copy = {}, Services services = {}, Transport migration = {}, Confirmation confirmation = {});
  ~CloudClipboardWindow();
  CloudClipboardWindow(const CloudClipboardWindow&) = delete;
  CloudClipboardWindow& operator=(const CloudClipboardWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows::win7
