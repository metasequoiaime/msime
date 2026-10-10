#pragma once
#include "CandidatePalette.h"
#include "TrayMenuLayout.h"
#include <cstdint>
#include <functional>
#include <memory>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// and NOMINMAX keeps its min/max macros away from the standard library.
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
class AccessibleWindow;
// The tray menu the shipped language bar opens: a composed card of commands, shown on request and dismissed as soon as it loses the pointer or focus. It owns no input state and never takes focus from the application being typed into, so the mode rows still address the focused TIP.
class TrayMenuWindow final {
public:
  // Runs a chosen command. Returning false leaves the menu open so a failed
  // command does not look like it was accepted.
  using Command = std::function<bool(TrayMenuCommand)>;
  // The live state the rows show: the toolbar switch, the focused TIP's modes and the stored preferences, so each row shows what the Server reports rather than a guess.
  using State = std::function<TrayMenuState()>;
  TrayMenuWindow(TrayMenuCapabilities capabilities, Command command,
                 State state);
  // 同一张卡片画别的一组行：悬浮工具栏的切换输入方案菜单和右键菜单。每次打开和每次点完一行都重新取这些行，所以勾选总是 Server 现在报告的状态。
  using Items = std::function<std::vector<TrayMenuItem>()>;
  TrayMenuWindow(Items items, Command command);
  ~TrayMenuWindow();
  TrayMenuWindow(const TrayMenuWindow &) = delete;
  TrayMenuWindow &operator=(const TrayMenuWindow &) = delete;
  // Open under the tray icon, in work area pixels.
  void show(int icon_center_x, int icon_top);
  // show() throws on Direct2D failure. Opening the menu must never take the
  // Server down, so callers use this and treat false as "no menu this time".
  bool open(int icon_center_x, int icon_top) noexcept;
  // 从悬浮工具栏的按钮打开：上方放得下就开在工具栏上方，否则开在下方（toolbar_menu_bounds）。失败与 open 一样只报告 false。
  bool open_beside(int anchor_center_x, int anchor_top, int anchor_bottom) noexcept;
  // The card is WS_EX_NOACTIVATE and never receives WM_KILLFOCUS, so the UI
  // pump polls the pointer to decide when to dismiss it.
  bool pointer_inside() const noexcept;
  void hide();
  void set_palette(CandidatePalette palette);
  bool visible() const;
  bool failed() const { return failed_; }
  HWND handle() const { return window_; }
  // 主题页的一行选中后调用，参数是主题 id。和 Command 一样，返回 false 时卡片留着；没设置时主题行什么也不做。
  using ThemeAction = std::function<bool(const std::string &)>;
  void set_theme_action(ThemeAction action) { theme_action_ = std::move(action); }
  // 最近一次键盘导航的时刻（GetTickCount64）。卡片开在托盘图标上方，用键盘的人指针多半在卡片外，UI 循环拿它和指针一起判断卡片是不是闲着。
  uint64_t keyboard_activity() const { return keyboard_at_; }

private:
  // Resolved once per window; null when neither icon font is installed.
  const wchar_t *icon_family_ = nullptr;
  IDWriteTextFormat *icon_text_format_ = nullptr;
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void create_window();
  void paint();
  std::optional<size_t> hit(int x, int y) const;
  void choose(size_t index, bool from_keyboard = false);
  // 卡片开着时用低级键盘钩子收键：卡片不抢焦点，键盘消息到不了它。钩子只在卡片可见时装着，同一时刻只属于一张卡片。
  static LRESULT CALLBACK keyboard_procedure(int code, WPARAM wparam, LPARAM lparam) noexcept;
  void start_keyboard();
  void stop_keyboard();
  void key(TrayMenuKey key);
  // 换到卡片的另一页，按原来的锚点重新摆放（页的高度不同）。
  void show_page(TrayMenuPage page, bool from_keyboard);
  // Rebuild the rows and their geometry from the live state.
  void refresh_items();
  // 按现在的行和高亮发布读屏的元素树（TrayMenuAccessibility.h）；卡片不可见时发布一棵空树。
  void sync_accessibility();
  // 读屏要求执行一行（accessible_invoke_message），等同于用键盘执行它。`token` 对不上当前的树时丢掉。
  void invoke_accessible(int id, LPARAM token);
  // The product mark for the header, loaded from the Server's resources at the size it is drawn. Null when the module carries no mark, as in the test executables; the header then shows its name alone.
  ID2D1Bitmap *logo_bitmap(int pixels);
  // 按锚点摆放卡片；`anchor_bottom` 为空时按托盘的规则只往上开。
  void place(int center_x, int anchor_top, std::optional<int> anchor_bottom);
  TrayMenuCapabilities capabilities_;
  Command command_;
  State state_;
  Items items_builder_;
  ThemeAction theme_action_;
  TrayMenuPage page_ = TrayMenuPage::Main;
  // 最近一次摆放用的锚点，翻页时按它重新摆放。
  int anchor_x_ = 0;
  int anchor_top_ = 0;
  std::optional<int> anchor_bottom_;
  uint64_t keyboard_at_ = 0;
  // Direct2D's imaging factory is a COM server; this thread owns an apartment.
  struct Apartment {
    Apartment();
    ~Apartment();
    Apartment(const Apartment &) = delete;
    Apartment &operator=(const Apartment &) = delete;
    bool owned = false;
  } apartment_;
  msimeui::DeviceResources device_;
  CandidatePalette palette_;
  TrayMenuMetrics metrics_;
  std::vector<TrayMenuItem> items_;
  TrayMenuGeometry geometry_{};
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  HWND window_ = nullptr;
  size_t hovered_ = static_cast<size_t>(-1);
  // 高亮是键盘导航停上去的（而不是鼠标悬停）：只有这时才把那一行报告为读屏的键盘焦点。
  bool keyboard_highlight_ = false;
  // 交给读屏的 UI Automation 提供者；窗口建好后才创建，CreateWindowExW 期间为空。
  std::unique_ptr<AccessibleWindow> accessible_;
  unsigned dpi_ = 96;
  // place() 正在挪动卡片：挪到 DPI 不同的显示器时系统同步发来 WM_DPICHANGED，这时尺寸已经按目标显示器的 DPI 算好，不能当作显示环境变了把刚打开的卡片收起。
  bool placing_ = false;
  bool failed_ = false;
};
} // namespace msime::windows
