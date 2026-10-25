#pragma once

#include "EmojiCatalog.h"
#include <memory>
#include <string_view>

namespace msime::windows::win7 {
enum EmojiPanelControl {
  emoji_panel_tab = 600, emoji_panel_group, emoji_panel_search, emoji_panel_entries, emoji_panel_detail,
  emoji_panel_previous, emoji_panel_next, emoji_panel_page, emoji_panel_insert, emoji_panel_copy,
  emoji_panel_refresh, emoji_panel_clipboard, emoji_panel_status,
  emoji_panel_preview,
};
class EmojiWindow final {
public:
  using InsertText = std::function<bool(std::wstring_view)>;
  using CopyText = std::function<bool(HWND,std::wstring_view)>;
  using OpenClipboard = std::function<bool()>;
  explicit EmojiWindow(InsertText insert = {}, CopyText copy = {}, emoji::Transport transport = {}, OpenClipboard clipboard = {});
  ~EmojiWindow();
  EmojiWindow(const EmojiWindow&) = delete;
  EmojiWindow& operator=(const EmojiWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
}
