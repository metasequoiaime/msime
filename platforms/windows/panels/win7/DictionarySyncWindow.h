#pragma once

#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/DictionarySyncWorker.h"
#include <memory>

namespace msime::windows::win7 {
enum DictionarySyncPanelControl { dictionary_sync_panel_body = 400 };
// Win7 的词库同步面板（`--route=dictionary-sync`）：Win7 设置窗口的导航不收云端同步的页面（SettingsChrome.h 的 page_available），与云剪贴板一样放在独立的面板里，由设置窗口词库页的「词库同步…」打开。控件和动作在 settings/win7/DictionarySyncForm.h。
class DictionarySyncWindow final {
public:
  using Confirmation = msime::settings::win7::SettingsWindow::Confirmation;
  using Services = msime::settings::win7::DictionarySyncServices;
  explicit DictionarySyncWindow(Services services = {}, Confirmation confirmation = {});
  ~DictionarySyncWindow();
  DictionarySyncWindow(const DictionarySyncWindow&) = delete;
  DictionarySyncWindow& operator=(const DictionarySyncWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows::win7
