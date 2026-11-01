#pragma once
#include "CandidateBackdrop.h"
#include "CandidateCanvas.h"
#include "CandidateCardSize.h"
#include "CandidateClickWorker.h"
#include "CandidateExpandPanel.h"
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
#include "SystemAccent.h"
#include <functional>
#include <memory>
#include <unordered_map>
#include <utility>
#include <vector>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// which is how the rest of this UI stack spells DrawTextW.
#include <windows.h>
#include <msimeui/DeviceResources.h>
#include "CandidateImageFrames.h"
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
// 所属桌面的 UI 线程独占构造、轮询、绘制与销毁；Reader 比窗口长寿，返回经租约验证的值，
// 不暴露 Engine 状态。上下文菜单和关联浮层也由同一线程更新与销毁。
class CandidateWindow final {
public:
  using Reader = std::function<std::optional<CandidatePresentation>()>;
  using Click = std::function<void(const CandidateClick &)>;
  // 右键菜单「加入生词本」交出去的东西：候选原文、这一帧显示着的释义（各语言行用 U+2028 分隔，见 translation_lines）和读音（按 "\n" 分行）。只在 UI 线程上调用，交给 Server 的工作线程去写生词本。
  struct PersonalWordbookRequest {
    std::string candidate, gloss, pronunciation;
  };
  using PersonalWordbook = std::function<void(const PersonalWordbookRequest &)>;
  // 右键菜单「朗读」：`text` 在 UI 线程上打开菜单时按候选原文和显示着的释义答要读的英文（没有时不出这一项），`speak` 把它交给 Server 的朗读工作线程。两个都设了才有这一项。
  struct ReadAloud {
    std::function<std::optional<std::string>(const std::string &candidate, const std::string &gloss)> text;
    std::function<void(const std::string &)> speak;
  };
  using Page = std::function<void(const CandidatePage &)>;
  using Rendered = std::function<void(const CandidatePresentation &)>;
  explicit CandidateWindow(Reader reader, Click click = {}, unsigned font_size = 16,
                           unsigned preedit_font_size = 16,
                           std::string font_family = "Segoe UI",
                           std::vector<std::string> fallback_fonts = {},
                           std::optional<bool> dark_theme = std::nullopt,
                           bool horizontal = false, bool show_preedit = true,
                           Page page = {}, Rendered rendered = {},
                           bool mouse_wheel = false, bool attach_typing_effect = true);
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
  // 皮肤的 font_family 排在最前，用户配置的字体整体退为回退链；传空、未安装或过长时恢复用户设置。之后的字体偏好变化只更新用户配置，皮肤字体保持在前。
  void set_skin_font_family(const std::string &family);
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
    decoration_candidate_ = decoration.candidate;
  }
  // The image a package draws over the card surface. Empty image means none.
  void set_skin_background(CandidateSkinBackground background) {
    background_ = std::move(background);
  }
  // The card radius a package asks for; none keeps the theme's.
  void set_skin_corner_radius(std::optional<float> radius) { skin_radius_ = radius; }
  // 包的版式、外框线宽、高亮圆角、阴影档位、材质、翻页箭头、收紧宽度和卷轴展开（CandidateSkinLayout.h）。每次排版按当前排列和字号解析；和用户偏好的先后见 paint() 里的说明。卷轴展开的开关变了就收起。
  void set_skin_layout(const CandidateSkinLayout &layout);
  // canvas 皮肤在当前明暗下要画的画布（CandidateCanvas.h）：candidate_canvas_drawable 的结果，面板图已经按明暗选好、确认解得出来，挂件只留解得出来的。不是 canvas 包时传默认值（active 为 false），卡片和以前逐位相同。下一次排版按它重新定窗口和卡片的尺寸。
  void set_skin_canvas(CandidateSkinCanvas canvas);
  // 卷轴展开（CandidateExpandPanel.h）打开或收起时通知 Server（SessionController::set_candidate_expansion）：打开后 Server 才去取这一代的完整候选列表，随快照的 CandidatePresentation::expanded 交回来。
  using Expansion = std::function<void(const FocusLease &, bool)>;
  void set_expansion_handler(Expansion handler) { expansion_ = std::move(handler); }
  // 不阻塞地确认焦点租约（SessionController::candidate_lease_current）。读快照争用时，仅保留已确认租约的可见帧或正常退场；焦点已换走或未设置检查时立即收起。
  using LeaseCheck = std::function<bool(const FocusLease &)>;
  // 不设时右键菜单没有「加入生词本」。
  void set_personal_wordbook_handler(PersonalWordbook handler) { personal_wordbook_ = std::move(handler); }
  // 不设时右键菜单没有「朗读」。
  void set_read_aloud_handler(ReadAloud handler) { read_aloud_ = std::move(handler); }
  void set_lease_check(LeaseCheck check) { lease_current_ = std::move(check); }
  // 和点「∨」一样打开卷轴展开，展开通知照常发出；这一帧没有画出按钮或已经展开时什么也不做。返回此刻是否展开着。Server 不调它，候选皮肤截图工具（tests/tools/candidate_skin_gallery）用它画展开状态。
  bool open_expansion();
  // 兼容旧皮肤的动效数据，候选显隐与高亮始终立即呈现；图片帧与装饰动效独立播放。
  void set_skin_animation(CandidateSkinAnimation animation) {
    (void)animation;
    animation_ = {};
  }
  // 光标处的打字特效浮层：每取到一个特效就交给它画火花、光标行闪光和连击徽标。返回 true 表示浮层在负责连击数，卡片就不在拼音行里再画一份；没有设置或返回 false 时，卡片照旧自己画计数、各样式都闪。
  using TypingEffectPresenter = std::function<bool(const TypingEffectPresentation &)>;
  void set_typing_effect_presenter(TypingEffectPresenter presenter) { typing_effect_presenter_ = std::move(presenter); }
  // The user's scale, opacity and corner radius. Scale changes the card's size, so the next refresh lays it out again. Invalid values leave the previous style intact.
  bool set_style(const CandidateWindowStyle &style);
  void hide();
  // 多桌面路由只让当前焦点所属的窗口领取全局打字特效，失焦窗口不会消费其他桌面的信号。
  void set_typing_effect_active(bool active);
  bool failed() const { return failed_; }
  // 第一次失败的位置，只有固定标签和数字，可以写进诊断日志。
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }
  HWND handle() const { return window_; }
  // 候选卡片（不含四周的透明阴影边距、canvas 的外留白和顶上吉祥物那一条；canvas 时是整块面板）此刻的屏幕矩形，物理像素，候选窗不可见时为空。调用方要在每显示器 DPI 感知的线程上下文里，双拼键位图贴着它摆放。
  std::optional<TypingRect> card_on_screen() const { return typing_effect_card(); }
  // 上一帧实际画出的卡片在客户区里的矩形（物理像素，不含阴影边距、canvas 的外留白和吉祥物那一条；canvas 时是整块面板），还没画过时为空。只读，Server 不用；候选皮肤截图工具拿它和窗口客户区、card_on_screen() 比，确认卡片没有画到窗口外面。
  std::optional<RECT> painted_card() const;
  // 上一帧卡片背后的系统材质（皮肤 `backdrop`），一行说明：垫了哪种材质和参数，或为什么退回不透明底色。只读，Server 不用；候选皮肤截图工具写进日志。
  std::string painted_backdrop() const;

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
  // Rebuild font_family_ and fallback_families_ from the configured fonts and the skin font, and drop the cached fallback chain.
  void apply_font_chain();
  CandidateBounds card_bounds(const CandidatePresentation &value,
                              const RECT &work, unsigned dpi);
  // Each candidate's runs measured the way paint() draws them, and their wrapped heights. card_bounds and paint share both, so the card is sized for exactly the rows that get drawn.
  std::vector<CandidateItemWidths> measure_items(const CandidatePresentation &value);
  CandidateWrapMeasure wrap_measure(const CandidatePresentation &value);
  // 这一页最宽的序号（1 到 count）按序号字号量出的宽度：候选文字从它之后开始（CandidateCardInput::number_width）。
  double number_label_width(size_t count, const CandidateSkinMetrics &skin);
  // 这一页候选字一行的高度：主字体的一行，和每个候选的文字、注释用回退字体画出来的那一行，取最高的，和 macOS 按每个候选量 MSIMECandidateTextHeight 一样。card_bounds 和 paint 都读它，量出来的行高和画出来的一致。
  double candidate_line_height(const CandidatePresentation &value);
  // 横排候选为还没到的释义预留几行：偏好算出的有释义来源的目标语言行数（这一页不请求释义时为 0），韩文汉字列表再加 훈음 那一行。card_bounds 和 paint 都读它，量出来的尺寸和画出来的行才一致。
  size_t reserved_secondary_lines(const CandidatePresentation &value) const;
  // 预留的 lines 行释义用释义字体量出的高度（lines 行占位文字，和 wrap_measure 量多行释义同一种格式），交给 candidate_reserved_row_height。不到两行时为 0：一行释义按固定行高算，不用量。
  double reserved_secondary_height(size_t lines);
  void paint();
  ID2D1Bitmap *skin_image(const std::wstring &path, D2D1_SIZE_F *size = nullptr);
  void schedule_image_frame();
  // UI thread: adopt the waiting typing effect and start its flash and combo timers.
  void take_typing_effect();
  // Repaint for the next flash frame, and stop the timer once the flash has faded.
  void typing_effect_tick(UINT_PTR timer);
  void skin_animation_tick();
  // rows 是这一页的候选行数，逐行浮现的进场按它算总时长；选中动效不错峰，传 0。
  void begin_skin_animation(CandidateSkinAnimationPreset preset, size_t rows = 0);
  void finish_skin_animation();
  void hide_now();
  void hide_immediately();
  // 读空时仅沿用仍属当前焦点的可见帧；明确隐藏的快照不被旧帧覆盖，正常退场交给 hide_for 和 paint 延续。
  std::optional<CandidatePresentation> read_presentation() const;
  // 组字正常结束时可以退场，其余明确隐藏立即收起；读空只延续租约已确认的现有退场。
  void hide_for(const std::optional<CandidatePresentation> &value);
  // 正在播的退场所画的那一帧，它的焦点租约已经不是当前焦点（焦点换到了别的客户端或应用）。
  bool exit_lease_lost() const;
  bool skin_motion_enabled() const;
  float skin_motion_progress() const;
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
  // 读屏要求执行一个元素（accessible_invoke_message）：候选行等同于点它，翻页箭头等同于点箭头，展开按钮和网格里的候选也一样。`token` 对不上当前的树时丢掉。
  void invoke_accessible(int id, LPARAM token);
  // 这一帧用的皮肤版式。卷轴展开只对能用鼠标选的 Engine 列表有意义：释义页和只能用键盘选的列表照常画列表和翻页（page_arrows 照常生效）。
  CandidateSkinMetrics skin_metrics(const CandidatePresentation &value) const;
  // 这一帧网格要排的后续候选：挂着的完整列表里排在当前页之后的那些（start 起 available 个），最多排出 count 个（一批一批地加）。顺带对照网格第一项，换了候选就回到顶部。没展开、这一页不能展开或列表还没到时为空。
  struct ExpandGrid {
    std::shared_ptr<const std::vector<ExpandedCandidate>> items;
    size_t start = 0, count = 0, available = 0;
  };
  ExpandGrid expand_grid(const CandidatePresentation &value);
  // 网格里每个候选的文字宽度（候选字号），按文字缓存：同一批候选每次重画都要排一遍网格，DirectWrite 只量一次。
  std::vector<double> grid_widths(const ExpandGrid &grid);
  // 每页候选数，网格最多分这么多列；view 没带时按这一页的候选数。
  static size_t expand_page_size(const CandidatePresentation &value);
  // 点「∨」「∧」：展开或收起，并通知 Server。
  void toggle_expansion();
  // 收起（候选窗关闭、换了客户端、改成竖排、换了皮肤）；没展开时什么也不做。
  void collapse_expansion();
  // 滚到离底部一行半以内、还有没排出来的候选时再排一批。
  void reveal_if_near_end();
  // 点网格里第 slot 个候选：带着它在完整列表里的下标和画出来的文字交给 click_。
  void commit_cell(size_t slot);
  // 客户区像素换成卡片坐标（DIP），和 hit() 同一种换算。
  std::pair<double, double> card_point(int x, int y) const;
  Reader reader_;
  Click click_;
  PersonalWordbook personal_wordbook_;
  // 打开着的右键菜单的「加入生词本」要交的内容，和 menu_target_ 一样每次打开重记。
  std::optional<PersonalWordbookRequest> menu_personal_;
  ReadAloud read_aloud_;
  // 打开着的右键菜单的「朗读」要读的英文，同样每次打开重记。
  std::optional<std::string> menu_read_aloud_;
  Page page_;
  Rendered rendered_;
  Expansion expansion_;
  LeaseCheck lease_current_;
  // 卷轴展开的状态和打开它时的租约；expand_viewport_ 是上一帧展开区的高度，后续候选还在路上的那一帧沿用它，卡片不先缩回一行再长回来。
  CandidateExpandState expand_;
  std::optional<FocusLease> expand_lease_;
  double expand_viewport_ = 0.0;
  std::unordered_map<std::string, double> grid_width_cache_;
  // 刚画好的展开区（滚动偏移为 0 的布局）、画时用的偏移（按设备像素取整）、一格滚动的距离和网格里的候选，点击、悬停和读屏都读这些。
  std::optional<CandidateExpandLayout> painted_expand_;
  double painted_expand_offset_ = 0.0, painted_expand_step_ = 0.0;
  ExpandGrid painted_grid_;
  std::optional<size_t> hovered_cell_, pressed_cell_;
  bool hovered_toggle_ = false, pressed_toggle_ = false, hovered_scrollbar_ = false;
  // 正在拖滚动条时，按下处在滑块里的高度。
  std::optional<double> thumb_grab_;
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
  // 鼠标停在列表里的哪一枚翻页箭头上（皮肤 `page_arrows`），按 msime-windows 的 PagerArrows 画悬停底色；首行的翻页没有悬停底色。
  std::optional<bool> hovered_page_;
  unsigned painted_dpi_ = 0;
  // 上一帧画出的卡片矩形（DIP），painted_card() 读它。
  std::optional<D2D1_RECT_F> painted_card_;
  // 这一帧卡片背后垫什么（CandidateBackdrop.h），读系统的透明效果和高对比度开关。
  CandidateBackdropPlan backdrop_plan() const;
  // 建 Windows.UI.Composition 的材质合成失败过：这个窗口不再尝试，要材质的皮肤画不透明底色（同 msime-windows 的 backdropUnavailable_）。
  bool backdrop_unavailable_ = false;
  CandidateBackdropPlan painted_backdrop_;
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
  CandidateImageFrames image_frames_;
  uint64_t image_epoch_ = 0;
  bool image_suspended_ = false;
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  CandidatePalette palette_;
  std::wstring font_family_;
  // The user's resolved, alias-mapped face; font_family_ is this unless a skin font is active.
  std::wstring configured_font_family_;
  // The installed, alias-mapped skin font_family; empty when the active skin names none.
  std::wstring skin_font_family_;
  // The user's configured fallback faces; fallback_families_ is derived from these and the skin font.
  std::vector<std::wstring> configured_fallback_families_;
  std::optional<CandidateFontSettings> font_settings_;
  std::optional<bool> dark_theme_;
  bool horizontal_ = false;
  bool show_preedit_ = true;
  bool wubi_code_hint_ = true;
  // 共享偏好 `show_app_logo`，经 set_layout 即时生效；关掉时首行不画 logo。
  bool show_app_logo_ = false;
  // 偏好算出的释义预留行数（0 到 2），经 set_layout 即时生效。
  unsigned reserved_gloss_lines_ = 0;
  // 共享偏好 `show_candidate_page_number`，经 set_layout 即时生效；关掉时首行不画页码和翻页箭头。
  bool show_page_number_ = true;
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
  std::optional<msime::candidate::DecorationCandidate> decoration_candidate_;
  CandidateSkinBackground background_;
  std::optional<float> skin_radius_;
  CandidateSkinLayout skin_layout_;
  CandidateSkinAnimation animation_;
  CandidateSkinAnimationPreset active_animation_ = CandidateSkinAnimationPreset::none;
  uint64_t animation_started_ = 0;
  // 这次动效一共播多久（毫秒）：卡片的时长，逐行浮现的进场再加上最后一行的延迟。
  uint64_t animation_total_ms_ = 0;
  // 卡片上一次开始收起的时刻（GetTickCount64），从没收起过时为空。离它不到 candidate_typing_burst_ms 又出现时不播进场。
  std::optional<uint64_t> last_hidden_at_;
  bool animation_active_ = false;
  bool finishing_animation_ = false;
  // The running animation is the selection preset: it moves only the highlighted row, never the whole card.
  bool selection_animation_ = false;
  std::optional<CandidatePresentation> exit_value_;
  CandidateWindowStyle style_;
  // Pixels reserved above the card for the artwork, computed when the card is
  // sized and reused when it is painted so the two cannot disagree.
  float decoration_offset_ = 0.0f;
  // 内容卡片（行、首行和点击都按它的坐标排）四周的透明空白，DIP。没有 canvas 时就是阴影边距；canvas 时由 card_bounds 和 decoration_offset_ 一起算：面板四周的空白（阴影边距加外留白或挂件伸出去的距离）再加上 canvas_offset_。
  CandidateShadowInsets shadow_insets_{};
  CandidateSkinCanvas canvas_;
  // canvas_ 的挂件按 z_index 分成面板之下和面板之上两层。
  CandidateCanvasLayers canvas_layers_;
  // 面板比内容卡片四边各多出的距离（candidate_canvas_content_offset，DIP，可以是负数），card_bounds 算好、paint 和命中测试沿用。不是 canvas 时为 0，面板就是卡片。
  CandidateCanvasEdges canvas_offset_{};
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
  TypingEffect effect_{};
  // The settings the current flash is drawn with, adopted with it so a flash keeps its length and colour while it fades.
  TypingEffectSettings effect_settings_{};
  uint64_t effect_started_ = 0;
  bool effect_flashing_ = false;
  // Power Mode 的抖动还在进行（typing_shake_millis 之内）：卡片按 typing_shake_offset 横向位移着画。
  bool effect_shaking_ = false;
  // 浮层在画连击徽标，卡片不画计数。
  bool effect_badge_elsewhere_ = false;
  // 这次特效的强调色：特效包没给颜色时火花、闪光和连击数用它。取系统强调色，与 macOS 的 controlAccentColor 一样；取不到时是主题的强调色。随特效一起取，闪光淡出期间不变。
  SystemAccent system_accent_;
  CandidateColor effect_accent_{};
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
