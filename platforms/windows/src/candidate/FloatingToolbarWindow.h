#pragma once
#include "CandidatePalette.h"
#include "ModeMailbox.h"
#include "FloatingToolbarSettings.h"
#include "ToolbarIcons.h"
#include "ToolbarLayout.h"
#include "ComponentFailure.h"
#include <functional>
#include <memory>
#include <array>
#include <optional>
#include <string>
#include <vector>
// windows.h first: its DrawText macro has to reach the Direct2D declarations.
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
class AccessibleWindow;
// Null toggles the stored value for the toolbar. A value is the Server
// session's explicit target, so a delayed preference write cannot invert a
// newer state that was already persisted by another surface.
struct CharacterSetClick {
  std::optional<bool> desired;
};
using CharacterSetClickWorker = SingleClickWorker<CharacterSetClick>;
// 从工具栏打开的菜单贴在哪里：按钮中心的横坐标和工具栏卡片的上下沿，屏幕物理像素。
struct ToolbarMenuAnchor {
  int center_x = 0;
  int top = 0;
  int bottom = 0;
};
class FloatingToolbarWindow final {
public:
  using Reader = std::function<std::optional<ModePresentation>()>;
  using Click = std::function<void(const ModeClick &)>;
  using Action = std::function<void()>;
  using PositionChanged = std::function<void(POINT)>;
  using MenuAction = std::function<void(const ToolbarMenuAnchor &)>;
  FloatingToolbarWindow(Reader reader, Click click);
  ~FloatingToolbarWindow();
  // Share the candidate card's resolved tokens so one theme covers the surface.
  void set_palette(CandidatePalette palette);
  // Caps Lock and the input mode change what the language button shows.
  void set_language_state(ToolbarLanguageState state) {
    if (state.caps_lock == language_.caps_lock &&
        state.mode == language_.mode && state.scheme == language_.scheme &&
        state.dedicated_english == language_.dedicated_english)
      return;
    language_ = state;
    if (window_)
      InvalidateRect(window_, nullptr, FALSE);
  }
  void set_scale(double scale) { scale_ = scale; }
  void set_font_size(int size) { font_size_ = size; }
  void set_items(std::array<bool, 6> items) { layout_.items = items; }
  // UI-thread only; refresh must remeasure even when input mode is unchanged.
  void set_settings(const FloatingToolbarSettings &settings) {
    if (!settings.valid()) return;
    const double scale = static_cast<double>(settings.scale_percent) / 100.0;
    if (scale_ == scale && font_size_ == static_cast<int>(settings.font_size) &&
        layout_ == settings) return;
    scale_ = scale;
    font_size_ = static_cast<int>(settings.font_size);
    layout_ = settings;
    hovered_.reset();
    pressed_.reset();
    shown_.reset();
    shown_character_set_.reset();
  }
  // 语言按钮提示里的方案名（toolbar_scheme_title），随存储的方案和键位变化。
  void set_scheme_title(std::wstring title) {
    if (title == scheme_title_)
      return;
    scheme_title_ = std::move(title);
    // 方案名也是语言按钮交给读屏的名字，读屏的树在重画时更新；只换双拼、五笔键位时方案和模式都没变，不重画的话读屏读到的还是旧方案名。
    if (window_)
      InvalidateRect(window_, nullptr, FALSE);
  }
  void set_position(std::optional<POINT> position) { dragged_position_ = position; }
  void set_position_changed(PositionChanged callback) { position_changed_ = std::move(callback); }
  // Tells a busy read apart from a client that is gone; see refresh().
  void set_active_reader(std::function<bool()> reader) {
    active_reader_ = std::move(reader);
  }
  void set_character_set_reader(std::function<std::optional<bool>()> reader) {
    character_set_reader_ = std::move(reader);
  }
  // 设置窗口和共享应用（MSIME.exe）各自在不在 Server 旁边。设置按钮要前者，手写按钮要后者；缺了的按钮画成不可用，与托盘对应的行一样，不接受一次什么都不做的点击。表情和屏幕键盘没有共享应用时打开系统自带的面板，所以不受这两个开关影响。
  void set_shell_available(bool settings, bool panels) {
    if (settings_available_ == settings && panels_available_ == panels)
      return;
    settings_available_ = settings;
    panels_available_ = panels;
    if (window_)
      InvalidateRect(window_, nullptr, FALSE);
  }
  void set_settings_action(Action action) { settings_action_ = std::move(action); }
  void set_character_set_action(Action action) { character_set_action_ = std::move(action); }
  void set_emoji_action(Action action) { emoji_action_ = std::move(action); }
  void set_keyboard_action(Action action) { keyboard_action_ = std::move(action); }
  void set_hide_action(Action action) { hide_action_ = std::move(action); }
  void set_handwriting_action(Action action) { handwriting_action_ = std::move(action); }
  void set_voice_action(Action action) { voice_action_ = std::move(action); }
  // 切换输入方案按钮：在按钮旁边弹出方案菜单。
  void set_input_scheme_action(MenuAction action) { input_scheme_action_ = std::move(action); }
  // 在工具栏卡片上点右键：弹出实用菜单（设置、检查更新、帮助、隐藏工具栏等）。
  void set_context_menu_action(MenuAction action) { context_menu_action_ = std::move(action); }
  // 在工具栏上按下左键或右键（点按钮、拖动、开菜单）时调用，Server 用它给空闲隐藏重新计时，与 macOS 点击或拖动工具栏会重新计时一致。
  void set_activity_action(Action action) { activity_action_ = std::move(action); }
  FloatingToolbarWindow(const FloatingToolbarWindow &) = delete;
  FloatingToolbarWindow &operator=(const FloatingToolbarWindow &) = delete;
  void refresh(bool enabled);
  void hide();
  // 失败后工具栏保持隐藏，之后的调用都不再做事；Server 去掉工具栏继续运行。
  bool failed() const { return failed_; }
  // 第一次失败的位置，只有固定标签和数字，可以写进诊断日志。
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }
  HWND handle() const { return window_; }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void paint();
  // 当前设置下要画的按钮和尺寸。
  std::vector<int> slots() const;
  ToolbarMetrics metrics() const;
  // 按钮能不能用：缺设置窗口或共享应用时，依赖它的按钮不能用。
  bool usable(int button) const;
  // 按钮 `index` 弹出菜单用的锚点，已换成屏幕物理像素（toolbar_physical_coordinate）。
  ToolbarMenuAnchor menu_anchor(std::optional<size_t> index, int x) const;
  // 每个按钮一个悬停提示区域，跟着按钮的位置和个数重建。
  void sync_tooltips();
  // 按钮 `index` 的提示文字，提示控件来要的时候现算，所以总是当前状态。
  std::wstring tooltip_text(size_t index);
  // 按钮的提示文字，也是读屏读到的名字。`value` 是 reader_ 读到的模式，没有时只给按钮的名字。
  std::wstring button_name(int button, const std::optional<ModePresentation> &value) const;
  // 执行第 `position` 个按钮，鼠标点击和读屏执行共用。`x` 是点击的横坐标，只在没有按钮位置时用来摆菜单。
  void run(size_t position, const ModePresentation &value, int x);
  // 按刚画好的按钮发布读屏的元素树（ToolbarAccessibility.h）。
  void sync_accessibility(const std::optional<ModePresentation> &value);
  // 读屏要求执行一个按钮（accessible_invoke_message）。`token` 对不上当前的树时丢掉。
  void invoke_accessible(int id, LPARAM token);
  // 记下第一次失败并隐藏；error 由 catch 现场先取，免得隐藏窗口时被改写。
  void fail(ComponentFailureSite site);
  // The product mark, at `pixels` square, or nothing when the executable has
  // no icon resource - which is every unit test that links this library.
  ID2D1Bitmap *logo_bitmap(int pixels);
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
  Reader reader_;
  Click click_;
  Action settings_action_;
  Action character_set_action_;
  Action emoji_action_;
  Action keyboard_action_;
  Action hide_action_;
  Action handwriting_action_;
  Action voice_action_;
  MenuAction input_scheme_action_;
  MenuAction context_menu_action_;
  Action activity_action_;
  PositionChanged position_changed_;
  HWND window_ = nullptr;
  // 悬停提示控件；建不出来时为空，工具栏照常工作只是没有提示。
  HWND tooltip_ = nullptr;
  // 已经登记在提示控件上的区域个数，id 从 0 起连续。
  size_t tooltip_tools_ = 0;
  // 交给提示控件的文字要活到它画完，所以放在这里。
  std::wstring tooltip_text_;
  // 交给读屏的 UI Automation 提供者；窗口建好后才创建，CreateWindowExW 期间为空。
  std::unique_ptr<AccessibleWindow> accessible_;
  std::wstring scheme_title_;
  // The icon the mark is drawn from, and the size it was loaded at. Reloaded
  // when the DPI or the user's scale changes, so the mark is never resampled
  // from a frame of the wrong size.
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  std::optional<ModePresentation> shown_;
  std::optional<bool> shown_character_set_;
  std::optional<POINT> dragged_position_;
  // True between WM_ENTERSIZEMOVE and WM_EXITSIZEMOVE, so a programmatic
  // placement is not mistaken for one the user made.
  bool moving_ = false;
  // 这次移动循环开始时窗口的左上角；松手时和它比，动了才算用户拖到了新位置（floating_toolbar_drag_end）。
  std::optional<POINT> move_start_;
  bool settings_available_ = true;
  bool panels_available_ = true;
  std::function<bool()> active_reader_;
  std::function<std::optional<bool>()> character_set_reader_;
  bool failed_ = false;
  std::optional<ComponentFailureSite> failure_site_;
  // Pointer feedback. Without these the buttons gave no sign of being buttons.
  std::optional<size_t> hovered_;
  std::optional<size_t> pressed_;
  std::optional<FocusLease> pressed_lease_;
  bool tracking_mouse_ = false;
  // Caps Lock and the input mode, which the language button reflects.
  ToolbarLanguageState language_;
  // True once the toolbar has been positioned. The default corner is only for
  // the first placement; afterwards the user's own position is preserved.
  bool placed_ = false;
  double scale_ = 1.0;
  int font_size_ = 24;
  // 画哪些按钮、画不画 logo；scale_percent 和 font_size 在上面两个成员里，这里只用其余字段。
  FloatingToolbarSettings layout_;
};
} // namespace msime::windows
