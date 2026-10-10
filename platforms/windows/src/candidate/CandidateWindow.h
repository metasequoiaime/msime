#pragma once
#include "CandidateCardSize.h"
#include "CandidateClickWorker.h"
#include "CandidateFontSettings.h"
#include "CandidateLayoutSettings.h"
#include "CandidateMenuLayout.h"
#include "CandidatePalette.h"
#include "CandidatePresentation.h"
#include "CandidateShadow.h"
#include "CandidateSkin.h"
#include "TypingEffectPolicy.h"
#include "TypingEffectOverlay.h"
#include "CandidateWindowStyle.h"
#include "ComponentFailure.h"
#include "FullscreenForeground.h"
#include <functional>
#include <memory>
#include <utility>
#include <vector>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// which is how the rest of this UI stack spells DrawTextW.
#include <windows.h>
#include <msimeui/DeviceResources.h>
// IDWriteFontFallback and IDWriteTextFormat1 live here, not in dwrite.h.
#include <dwrite_2.h>

namespace msime::windows {
// Forward declared: the flyout pulls in its own window headers, and only the
// implementation needs them.
class CandidateFlyoutWindow;
class AccessibleWindow;
// 候选窗因前台呈现方式被策略隐藏的原因：游戏会话的前台处在 D3D 独占全屏，或这个游戏进程已被反应式锁存。
enum class CandidateSuppression { ExclusiveFullscreen, Latched };
// 一次抑制状态的变化，由主循环取出写进诊断日志。cause 是锁存触发或解除的固定标签，独占抑制没有 cause。
struct CandidateSuppressionChange {
  CandidateSuppression reason;
  bool active;
  DWORD pid;
  const char *cause;
};
// Main/UI thread owns construction, polling, painting and destruction. Reader
// outlives the window and returns a freshly validated value, never Engine
// state.
// One owner-drawn menu row's label. Owner drawing keeps the platform's own
// keyboard handling and dismissal while letting the skin paint the row.
class CandidateWindow final {
public:
  using Reader = std::function<std::optional<CandidatePresentation>()>;
  using Click = std::function<void(const CandidateClick &)>;
  using Page = std::function<void(const CandidatePage &)>;
  using Rendered = std::function<void(const CandidatePresentation &)>;
  explicit CandidateWindow(Reader reader, Click click = {}, unsigned font_size = 16,
                           unsigned preedit_font_size = 16,
                           std::string font_family = "Segoe UI",
                           std::vector<std::string> fallback_fonts = {},
                           std::optional<bool> dark_theme = std::nullopt,
                           bool horizontal = false, bool show_preedit = true,
                           Page page = {}, Rendered rendered = {},
                           bool mouse_wheel = false);
  ~CandidateWindow();
  CandidateWindow(const CandidateWindow &) = delete;
  CandidateWindow &operator=(const CandidateWindow &) = delete;
  void refresh();
  // 主循环每轮在 refresh() 之前调用。游戏会话的兜底锚点、独占抑制、反应式锁存，以及全屏下按整块显示器钳制，都读这里记下的前台。
  void set_foreground(HWND foreground, ForegroundPresentation presentation);
  // refresh() 之后调用：前台几何全屏、并且前台进程自己的窗口压在候选窗上面时重申置顶。每个快照最多一次，两次至少间隔 1 秒，免得和常驻置顶的 overlay 来回抢。
  void keep_on_top();
  // 取出上次以来的抑制变化。这个类拿不到 Server 的诊断日志，由主循环写。
  std::vector<CandidateSuppressionChange> take_suppression_changes() {
    return std::exchange(suppression_changes_, {});
  }
  // UI thread only. Invalid updates leave the previous display intact.
  bool set_fonts(const CandidateFontSettings &settings);
  void set_layout(CandidateLayoutSettings settings);
  // Adopt resolved skin tokens. The next refresh repaints with them; the
  // built-in theme stays in place until a package is actually resolved.
  void set_palette(CandidatePalette palette);
  void set_theme_palette(CandidatePalette palette);
  void invalidate_skin_images();
  // Minimum card width asked for by the active skin package, in DIPs.
  void set_skin_min_width(double value) { skin_min_width_ = value; }
  // 候选窗口跟随光标. With this off the card keeps the position it first
  // appeared at until it disappears, rather than tracking the caret through a
  // word. A change takes effect at the next appearance, since the pinned
  // anchor is only forgotten when the card hides.
  void set_follow_cursor(bool enabled) { follow_cursor_ = enabled; }
  // 「鼠标滚轮翻页」（`navigation.mouse_wheel`），主循环每轮按已发布的偏好设置，和 macOS 每次渲染都重读一样即时生效。关掉时丢掉攒了一半的滚动量，免得重新打开后第一格翻得太早。
  void set_mouse_wheel(bool enabled) {
    if (mouse_wheel_ != enabled)
      wheel_accumulator_ = 0;
    mouse_wheel_ = enabled;
  }
  // The mascot a package draws above the card. Empty image means none.
  void set_skin_decoration(const CandidateSkinDecoration &decoration) {
    decoration_image_ = decoration.image;
    decoration_top_ = decoration.top_dip;
    decoration_width_ = decoration.width_dip;
    decoration_align_ = decoration.align;
  }
  // The image a package draws over the card surface. Empty image means none.
  void set_skin_background(CandidateSkinBackground background) {
    background_ = std::move(background);
  }
  // The card radius a package asks for; none keeps the theme's.
  void set_skin_corner_radius(std::optional<float> radius) { skin_radius_ = radius; }
  // preferences.plugins.effect_intensity, 0-100: how bright the typing flash is until a session publishes its resolved effect settings. The style, the combo and those settings come with each key from the input thread (TypingEffectSignal).
  void set_effect_intensity(uint32_t intensity) { effect_intensity_ = (std::min)(intensity, 100u); }
  // 光标处的打字特效浮层：每取到一个特效就交给它画火花、光标行闪光和连击徽标。返回 true 表示浮层在负责连击数，卡片就不在拼音行里再画一份；没有设置或返回 false 时，卡片照旧自己画计数、各样式都闪。
  using TypingEffectPresenter = std::function<bool(const TypingEffectPresentation &)>;
  void set_typing_effect_presenter(TypingEffectPresenter presenter) { typing_effect_presenter_ = std::move(presenter); }
  // The user's scale, opacity and corner radius. Scale changes the card's size, so the next refresh lays it out again. Invalid values leave the previous style intact.
  bool set_style(const CandidateWindowStyle &style);
  void hide();
  bool failed() const { return failed_; }
  // 第一次失败的位置，只有固定标签和数字，可以写进诊断日志。
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }
  HWND handle() const { return window_; }
  // 候选卡片（不含四周的透明阴影边距和顶上吉祥物那一条）此刻的屏幕矩形，物理像素，候选窗不可见时为空。调用方要在每显示器 DPI 感知的线程上下文里，双拼键位图贴着它摆放。
  std::optional<TypingRect> card_on_screen() const { return typing_effect_card(); }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void reposition();
  // 策略隐藏一个可见快照（INVALID_Y 又没有兜底、独占全屏、锁存）时照样发渲染回执，否则每个选词键都要白等渲染回执超时。同一个 render_serial 只发一次。
  void policy_hide(const CandidatePresentation &value);
  // 解除反应式锁存并关掉进程句柄；cause 非空时记一条解除。
  void release_latch(const char *cause);
  // 记下第一次失败并隐藏；error 由 catch 现场先取，免得隐藏窗口时被改写。
  void fail(ComponentFailureSite site);
  void invalidate_geometry();
  CandidateBounds card_bounds(const CandidatePresentation &value,
                              const RECT &work, unsigned dpi);
  // Each candidate's runs measured the way paint() draws them, and their wrapped heights. card_bounds and paint share both, so the card is sized for exactly the rows that get drawn.
  std::vector<CandidateItemWidths> measure_items(const CandidatePresentation &value);
  CandidateWrapMeasure wrap_measure(const CandidatePresentation &value);
  // 横排候选为还没到的释义预留几行：偏好算出的有释义来源的目标语言行数（这一页不请求释义时为 0），韩文汉字列表再加 훈음 那一行。card_bounds 和 paint 都读它，量出来的尺寸和画出来的行才一致。
  size_t reserved_secondary_lines(const CandidatePresentation &value) const;
  // 预留的 lines 行释义用释义字体量出的高度（lines 行占位文字，和 wrap_measure 量多行释义同一种格式），交给 candidate_reserved_row_height。不到两行时为 0：一行释义按固定行高算，不用量。
  double reserved_secondary_height(size_t lines);
  void paint();
  // UI thread: adopt the waiting typing effect and start its flash and combo timers.
  void take_typing_effect();
  // Repaint for the next flash frame, and stop the timer once the flash has faded.
  void typing_effect_tick(UINT_PTR timer);
  // Device pixels per DIP for a window DPI, including the user's scale. Layout, rendering, the logo and hit testing all read this one factor.
  double layout_scale(unsigned dpi) const {
    return static_cast<double>(dpi ? dpi : 96) / 96.0 * style_.scale();
  }
  // The brand mark leading the preedit row, loaded at `pixels` square. Null when the icon will not load, and the row then draws no mark.
  ID2D1Bitmap *logo_bitmap(int pixels);
  std::optional<CandidateClick> hit(int x, int y);
  // The pager arrow under a client point: true for the previous page, false for the next. None over anything else, over the previous arrow on the first page, or without a page callback.
  std::optional<bool> pager_hit(int x, int y);
  void show_context_menu(const CandidateClick &click, POINT client_point);
  // 按刚画好的行重新登记悬停提示的区域；行的位置和快照都没变时不动，打字闪光每秒重画几十次也不会反复登记。
  void sync_tooltips();
  // 读屏要求执行一个元素（accessible_invoke_message）：候选行等同于点它，翻页箭头等同于点箭头。`token` 对不上当前的树时丢掉。
  void invoke_accessible(int id, LPARAM token);
  Reader reader_;
  Click click_;
  Page page_;
  Rendered rendered_;
  HWND window_ = nullptr;
  std::optional<CandidatePresentation> shown_;
  unsigned shown_dpi_ = 0;
  std::optional<CandidatePresentation> painted_;
  // Rows as last drawn, set together with painted_. Hit testing reads these rather than recomputing, because row heights depend on text the click path should not measure again.
  std::vector<CandidateRowLayout> painted_rows_;
  // The pager as last drawn, in card coordinates, set together with painted_. None when the preedit row drew no pager.
  std::optional<CandidatePagerLayout> painted_pager_;
  std::optional<CandidateClick> pressed_;
  // The pager arrow a press started on; the page turns when the release lands on the same arrow.
  std::optional<bool> pressed_page_;
  std::optional<size_t> hovered_;
  unsigned painted_dpi_ = 0;
  bool failed_ = false;
  std::optional<ComponentFailureSite> failure_site_;
  unsigned font_size_ = 16;
  unsigned preedit_font_size_ = 16;
  // Direct2D's imaging factory is a COM server, and this thread is the Server's
  // own UI thread, which otherwise never enters an apartment.
  struct Apartment {
    Apartment();
    ~Apartment();
    Apartment(const Apartment &) = delete;
    Apartment &operator=(const Apartment &) = delete;
    bool owned = false;
  } apartment_;
  // Direct2D through the shared UI stack; no second renderer in this tree.
  msimeui::DeviceResources device_;
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  CandidatePalette palette_;
  std::wstring font_family_;
  std::optional<CandidateFontSettings> font_settings_;
  std::optional<bool> dark_theme_;
  bool horizontal_ = false;
  bool show_preedit_ = true;
  bool wubi_code_hint_ = true;
  // 共享偏好 `show_app_logo`，经 set_layout 即时生效；关掉时首行不画 logo。
  bool show_app_logo_ = false;
  // 偏好算出的释义预留行数（0 到 2），经 set_layout 即时生效。
  unsigned reserved_gloss_lines_ = 0;
  // Configured supplementary faces, in order, for the per-glyph fallback chain.
  // Minimum card width asked for by the active skin package, in DIPs.
  double skin_min_width_ = 0.0;
  bool follow_cursor_ = true;
  // Where this appearance first anchored; cleared in hide().
  std::optional<POINT> anchor_;
  // Decoration artwork: absolute path, how far it rises above the card, and
  // its drawn width. The height follows the image's own aspect ratio.
  std::wstring decoration_image_;
  double decoration_top_ = 0.0;
  double decoration_width_ = 0.0;
  CandidateSkinAlign decoration_align_ = CandidateSkinAlign::right;
  CandidateSkinBackground background_;
  std::optional<float> skin_radius_;
  CandidateWindowStyle style_;
  // Pixels reserved above the card for the artwork, computed when the card is
  // sized and reused when it is painted so the two cannot disagree.
  float decoration_offset_ = 0.0f;
  CandidateShadowInsets shadow_insets_{};
  // Owner-drawn menu labels, kept alive for the duration of the popup: the draw messages carry pointers into this list.
  // Built on first use: most sessions never open the right-click menu, and the flyout owns two windows and two Direct2D devices.
  std::unique_ptr<CandidateFlyoutWindow> flyout_;
  // 悬停提示（comctl32 的 tooltip 控件），每行候选一个区域，文字在 TTN_GETDISPINFOW 时按 painted_ 现取。建不出来时为空，只是没有提示。
  HWND tooltip_ = nullptr;
  std::vector<RECT> tooltip_rects_;
  uint64_t tooltip_serial_ = 0;
  std::wstring tooltip_text_;
  // 交给读屏的 UI Automation 提供者，每次画完换上新的元素树（CandidateAccessibility.h）。窗口建好后才创建，CreateWindowExW 期间为空。
  std::unique_ptr<AccessibleWindow> accessible_;
  // The candidate the open flyout acts on, recorded afresh on every right click because the flyout itself outlives any one opening.
  CandidateMenuTarget<CandidateClick> menu_target_;
  std::vector<std::wstring> fallback_families_;
  Microsoft::WRL::ComPtr<IDWriteFontFallback> font_fallback_;
  // Tallest this vertical list has been since the last hide(), in physical
  // pixels. Only the flip decision reads it; placement uses the real height.
  int64_t tallest_ = 0;
  // Wheel paging is opt-in. Off, the wheel goes back to DefWindowProc rather
  // than being swallowed by this NOACTIVATE window.
  bool mouse_wheel_ = false;
  int wheel_accumulator_ = 0;
  // The typing effect last taken from the input thread, when it arrived (GetTickCount64), and whether its flash is still fading. The combo count outlives the flash: it stays on the card until the library's idle window ends it.
  uint32_t effect_intensity_ = 50;
  TypingEffect effect_{};
  // The settings the current flash is drawn with, adopted with it so a flash keeps its length and colour while it fades.
  TypingEffectSettings effect_settings_{};
  uint64_t effect_started_ = 0;
  bool effect_flashing_ = false;
  // Power Mode 的抖动还在进行（typing_shake_millis 之内）：卡片按 typing_shake_offset 横向位移着画。
  bool effect_shaking_ = false;
  // 浮层在画连击徽标，卡片不画计数。
  bool effect_badge_elsewhere_ = false;
  TypingEffectPresenter typing_effect_presenter_;
  // 最近一次显示候选时 TSF 给的光标锚点，以及当时的前台窗口：上屏之后候选窗收起了，浮层还要知道光标在哪；前台换了就不再用它。
  std::optional<POINT> typing_anchor_;
  HWND typing_anchor_foreground_ = nullptr;
  // 候选卡片（不含阴影和吉祥物那一条）此刻的屏幕矩形，候选窗不可见时为空。
  std::optional<TypingRect> typing_effect_card() const;
  // set_foreground 记下的前台窗口、它的进程和呈现方式。
  HWND foreground_ = nullptr;
  DWORD foreground_pid_ = 0;
  ForegroundPresentation presentation_ = ForegroundPresentation::Windowed;
  // policy_hide 已经回执过的 render_serial。
  uint64_t receipted_serial_ = 0;
  // keep_on_top 上次重申置顶时的快照和时间（GetTickCount64）。
  uint64_t topmost_serial_ = 0;
  uint64_t topmost_at_ = 0;
  // 反应式锁存：游戏会话第一次在自己的几何全屏前台上弹出后，盯住那个窗口 2 秒。期间窗口最小化、前台换到别的进程、窗口矩形变了，或收到 WM_DISPLAYCHANGE，就当作这次弹出把游戏挤出了 QUNS 看不到的独占全屏（Vulkan 独占、OpenGL 改分辨率），对这个进程停止弹出。
  struct LatchWatch {
    PipeTicket ticket;
    HWND window;
    RECT rect;
    uint64_t since;
  };
  std::optional<LatchWatch> latch_watch_;
  // 已经盯过第一次弹出的客户端连接，每个连接只盯一次；只记一个的话，在两个游戏之间来回切换会反复重新盯梢。锁存因窗口化解除后，该进程的连接重新允许盯一次。
  std::vector<PipeTicket> latch_armed_;
  // 被锁存的客户端；抑制作用在它的整个进程上。该进程的前台在锁存后重新进入过非窗口化、之后又变成窗口化，同一个客户端重新连上（换了登记代次），或进程退出，就解除。
  std::optional<PipeTicket> latched_;
  // 被锁存进程的 SYNCHRONIZE 句柄，用来发现它已经退出；打不开时为空。
  HANDLE latched_process_ = nullptr;
  // 锁存之后是否见过该进程的前台重新进入非窗口化，见 set_foreground。
  bool latched_seen_fullscreen_ = false;
  bool display_changed_ = false;
  // 正处在独占抑制下的游戏进程，0 表示没有。
  DWORD exclusive_pid_ = 0;
  std::vector<CandidateSuppressionChange> suppression_changes_;
};
} // namespace msime::windows
