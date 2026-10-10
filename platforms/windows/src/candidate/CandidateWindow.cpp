#include "CandidateWindow.h"
#include "AccessibleWindow.h"
#include "CandidateAccessibility.h"
#include "CandidateFlyoutWindow.h"
#include "CandidateFontFormat.h"
#include "CandidateWheel.h"
#include "CursorResource.h"
#include "GameCandidateAnchor.h"
#include "WaveOverlayUtils.h"
#include "NativeFontAlias.h"
#include "ServerResources.h"
#include "TypingEffectSignal.h"
#include "WindowShadow.h"
#include <algorithm>
#include <commctrl.h>
#include <iterator>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
// A named CALLBACK rather than a lambda, as ShellLauncher's EnumWindows proc
// already is. FONTENUMPROCW is __stdcall; a captureless lambda converts to a
// __cdecl function pointer, and on x86 those are different types - this was a
// compile error for the 32-bit build and only compiled at all on x86_64, where
// there is one calling convention.
int CALLBACK note_font_found(const LOGFONTW *, const TEXTMETRICW *, DWORD,
                             LPARAM data) {
  *reinterpret_cast<bool *>(data) = true;
  return 0;
}
bool installed_font(const std::wstring &family) {
  HDC dc = GetDC(nullptr);
  if (!dc) return false;
  LOGFONTW logfont{};
  wcsncpy_s(logfont.lfFaceName, family.c_str(), LF_FACESIZE - 1);
  bool found = false;
  EnumFontFamiliesExW(dc, &logfont, note_font_found,
                      reinterpret_cast<LPARAM>(&found), 0);
  ReleaseDC(nullptr, dc);
  return found;
}
constexpr wchar_t class_name[] = L"MSIME.Client.Preview.Candidates" MSIME_EDITION_NAME_SUFFIX;
// 打字闪光和 Power Mode 抖动持续期间约每秒重画 30 帧，结束后停掉定时器；连击定时器只在显示的计数过期时触发一次。
constexpr UINT_PTR typing_flash_timer = 0x4501;
constexpr UINT_PTR typing_combo_timer = 0x4502;
constexpr UINT typing_flash_frame_millis = 30;
// Affect only this UI operation; restore the caller's thread context even on
// failure. The created HWND retains PMv2 awareness for its entire lifetime.
struct DpiScope {
  DPI_AWARENESS_CONTEXT previous;
  DpiScope()
      : previous(SetThreadDpiAwarenessContext(
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)) {
    if (!previous)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~DpiScope() { SetThreadDpiAwarenessContext(previous); }
};
struct Font {
  HDC dc;
  HFONT font;
  HGDIOBJ previous;
  Font(HDC target, int height, const wchar_t *family = L"Segoe UI")
      : dc(target), font(CreateFontW(-height, 0, 0, 0, FW_NORMAL, FALSE, FALSE,
                                     FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                                     CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                                     DEFAULT_PITCH, family)),
        previous(nullptr) {
    if (!font)
      throw std::runtime_error("Candidate font unavailable");
    previous = SelectObject(dc, font);
    if (!previous || previous == HGDI_ERROR) {
      DeleteObject(font);
      throw std::runtime_error("Candidate font selection failed");
    }
  }
  ~Font() {
    SelectObject(dc, previous);
    DeleteObject(font);
  }
};
std::wstring wide(const std::string &text) {
  if (text.size() > 4096)
    throw std::invalid_argument("Oversized window text");
  if (text.empty())
    return {};
  const int count =
      MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), nullptr, 0);
  if (!count)
    throw std::invalid_argument("Invalid window text");
  std::wstring result(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), result.data(),
                          count) != count)
    throw std::invalid_argument("Invalid window text");
  return result;
}
// 悬停提示的文字：候选和释义拼起来可能超过 wide() 的 4096 字节上限，提示又不值得为此让候选窗失败，所以单独转换，非法字节换成替换字符。
std::wstring tooltip_wide(const std::string &text) {
  if (text.empty() || text.size() > 65536)
    return {};
  const int count = MultiByteToWideChar(CP_UTF8, 0, text.data(),
                                        static_cast<int>(text.size()), nullptr, 0);
  if (count <= 0)
    return {};
  std::wstring result(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                          result.data(), count) != count)
    return {};
  return result;
}
// Text width in device independent pixels. DirectWrite is the same engine the renderer draws with, so the card cannot be sized for a different shaping. `weight` is the weight the text is drawn at: the preedit is semibold.
double measured_width(msimeui::DeviceResources &device, const std::wstring &text,
                      const std::wstring &family, float size,
                      IDWriteFontFallback *fallback,
                      DWRITE_FONT_WEIGHT weight = DWRITE_FONT_WEIGHT_NORMAL) {
  if (text.empty() || size <= 0.0f)
    return 0.0;
  auto *factory = device.GetDWriteFactory();
  auto *format = device.GetTextFormat(
      family, size, weight, DWRITE_TEXT_ALIGNMENT_LEADING,
      DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP);
  set_candidate_font_fallback(format, fallback);
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  DWRITE_TEXT_METRICS metrics{};
  if (factory && format &&
      SUCCEEDED(factory->CreateTextLayout(text.c_str(),
                                          static_cast<UINT32>(text.size()),
                                          format, 8192.0f, size * 4.0f,
                                          layout.GetAddressOf())) &&
      layout && SUCCEEDED(layout->GetMetrics(&metrics)))
    return metrics.widthIncludingTrailingWhitespace;
  // Without a usable factory the card is still sized, just less precisely.
  return static_cast<double>(text.size()) * static_cast<double>(size) * 0.92;
}
// The page indicator in the preedit row, "2 / 5", or empty when the card draws no pager: without candidates or a page count from the Engine there is nothing to page.
std::wstring pager_label(const CandidatePresentation &value) {
  if (value.candidates.empty() || value.page_count == 0 ||
      value.page >= value.page_count)
    return {};
  return std::to_wstring(value.page + 1) + L" / " +
         std::to_wstring(value.page_count);
}
// Height of text wrapped to `width` DIPs, with the same top aligned, wrapping format paint() draws a run below the first line with.
double wrapped_height(msimeui::DeviceResources &device, const std::wstring &text,
                      const std::wstring &family, float size, double width,
                      IDWriteFontFallback *fallback) {
  if (text.empty() || size <= 0.0f || !(width > 0.0))
    return 0.0;
  auto *factory = device.GetDWriteFactory();
  auto *format = device.GetTextFormat(
      family, size, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING,
      DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_WORD_WRAPPING_WRAP);
  set_candidate_font_fallback(format, fallback);
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  DWRITE_TEXT_METRICS metrics{};
  if (factory && format &&
      SUCCEEDED(factory->CreateTextLayout(text.c_str(),
                                          static_cast<UINT32>(text.size()),
                                          format, static_cast<float>(width),
                                          65536.0f, layout.GetAddressOf())) &&
      layout && SUCCEEDED(layout->GetMetrics(&metrics)))
    return std::ceil(metrics.height);
  // Same fallback as the shipped presenter: whole lines of the estimated width.
  return std::ceil(measured_width(device, text, family, size, fallback) / width) *
         static_cast<double>(size) * 1.25;
}
struct Painting {
  HWND window;
  PAINTSTRUCT state{};
  HDC dc;
  explicit Painting(HWND value)
      : window(value), dc(BeginPaint(window, &state)) {}
  ~Painting() { EndPaint(window, &state); }
};
// Build a real per-glyph fallback chain from the configured faces.
//
// PreviewConfig documents these as "supplementary faces tried in order when the
// main font lacks a glyph", but the window only ever used them to replace the
// primary family when that family was not installed at all. Once the primary
// existed, a missing glyph fell through to DirectWrite's system fallback and
// the user's list was ignored entirely - which is the case the setting is for,
// since the primary is usually a Latin/CJK face and the missing glyph is an
// emoji or a rare character.
Microsoft::WRL::ComPtr<IDWriteFontFallback>
build_font_fallback(IDWriteFactory *factory,
                    const std::vector<std::wstring> &families) {
  Microsoft::WRL::ComPtr<IDWriteFontFallback> result;
  if (!factory || families.empty())
    return result;
  Microsoft::WRL::ComPtr<IDWriteFactory2> factory2;
  if (FAILED(factory->QueryInterface(IID_PPV_ARGS(&factory2))) || !factory2)
    return result; // Windows 7 and older: keep the system chain.
  Microsoft::WRL::ComPtr<IDWriteFontFallbackBuilder> builder;
  if (FAILED(factory2->CreateFontFallbackBuilder(&builder)) || !builder)
    return result;
  // The whole Unicode range, in the user's order.
  DWRITE_UNICODE_RANGE range{0, 0x10FFFF};
  for (const auto &family : families) {
    const wchar_t *name = family.c_str();
    if (FAILED(builder->AddMapping(&range, 1, &name, 1)))
      return result;
  }
  // Append the system chain last so anything the list does not cover still
  // resolves the way it did before.
  Microsoft::WRL::ComPtr<IDWriteFontFallback> system;
  if (SUCCEEDED(factory2->GetSystemFontFallback(&system)) && system)
    builder->AddMappings(system.Get());
  if (FAILED(builder->CreateFontFallback(&result)))
    result.Reset();
  return result;
}
// Owner-drawn menu rows.
//
// The rows and their rules were already right; only the presentation was the
// OS default, so a light system menu appeared over a dark card and no skin's
// colours reached it. Owner drawing keeps the platform's own keyboard handling
// and dismissal - which a hand-rolled flyout would have to reimplement - while
// painting the rows from the skin.
} // namespace
CandidateWindow::CandidateWindow(Reader reader, Click click, unsigned font_size,
                                 unsigned preedit_font_size,
                                 std::string font_family,
                                 std::vector<std::string> fallback_fonts,
                                 std::optional<bool> dark_theme,
                                 bool horizontal, bool show_preedit, Page page,
                                 Rendered rendered, bool mouse_wheel)
    : reader_(std::move(reader)), click_(std::move(click)), page_(std::move(page)),
      rendered_(std::move(rendered)),
      font_size_(font_size),
      preedit_font_size_(preedit_font_size),
      palette_(candidate_native_palette(dark_theme.value_or(false))),
      font_family_(wide(font_family)), dark_theme_(dark_theme), horizontal_(horizontal),
      show_preedit_(show_preedit), mouse_wheel_(mouse_wheel) {
  if (font_family_.empty() || font_family_.size() > 128)
    throw std::invalid_argument("Invalid candidate font family");
  if (font_size_ < 12 || font_size_ > 32 || preedit_font_size_ < 12 ||
      preedit_font_size_ > 32)
    throw std::invalid_argument("Invalid candidate font size");
  // Keep the configured faces for the per-glyph chain, and separately allow one
  // of them to stand in when the primary family is not installed at all. The
  // two are different problems and both need handling.
  for (const auto &fallback : fallback_fonts) {
    auto candidate = wide(fallback);
    if (!candidate.empty() && candidate.size() <= 128)
      fallback_families_.push_back(std::move(candidate));
  }
  if (!installed_font(font_family_)) {
    for (const auto &candidate : fallback_families_) {
      if (installed_font(candidate)) {
        font_family_ = candidate;
        break;
      }
    }
  }
  // GDI selects installed faces above; DirectWrite needs canonical families
  // both for measurement and the per-glyph fallback mapping.
  font_family_ = native_font_alias(font_family_);
  for (auto &family : fallback_families_)
    family = native_font_alias(family);
  if (!reader_)
    throw std::invalid_argument("Missing candidate reader");
  DpiScope dpi_scope;
  WNDCLASSEXW descriptor{};
  descriptor.cbSize = sizeof(descriptor);
  descriptor.lpfnWndProc = procedure;
  descriptor.hInstance = GetModuleHandleW(nullptr);
  descriptor.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
  descriptor.lpszClassName = class_name;
  // This process owns the class; no per-window unregister/re-register race.
  if (!RegisterClassExW(&descriptor) &&
      GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Candidate class unavailable");
  // No redirection bitmap: the card is composed with per-pixel alpha, which is
  // what gives it rounded corners instead of a rectangular window cut-out.
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST |
                                WS_EX_NOREDIRECTIONBITMAP,
                            class_name, L"", WS_POPUP, 0, 0, 1, 1, nullptr,
                            nullptr, descriptor.hInstance, this);
  if (!window_)
    throw std::runtime_error("Candidate window unavailable");
  TypingEffectSignal::instance().attach(window_);
  // 与 macOS 候选窗每个子视图的 accessibilityLabel 对应：读屏通过 UI Automation 读到每行候选、页码、翻页箭头、预编辑和 logo。
  accessible_ = std::make_unique<AccessibleWindow>(window_);
  // 悬停提示，与 macOS 每个候选按钮的 toolTip 对应。建不出来不算候选窗失败，只是没有提示。
  INITCOMMONCONTROLSEX controls{sizeof(controls), ICC_BAR_CLASSES};
  if (InitCommonControlsEx(&controls))
    tooltip_ = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                               TOOLTIPS_CLASSW, nullptr,
                               WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX, CW_USEDEFAULT,
                               CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, window_,
                               nullptr, descriptor.hInstance, nullptr);
  // 设了最大宽度，提示里的 "\n" 才会换行，过长的一行也会折开。
  if (tooltip_)
    SendMessageW(tooltip_, TTM_SETMAXTIPWIDTH, 0, 480);
}
CandidateWindow::Apartment::Apartment() {
  const HRESULT entered =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  // S_FALSE only means this thread was already inside the same apartment; the
  // reference still has to be released. A different mode is left untouched.
  if (FAILED(entered) && entered != RPC_E_CHANGED_MODE)
    throw std::runtime_error("Candidate apartment unavailable");
  owned = entered != RPC_E_CHANGED_MODE;
}
CandidateWindow::Apartment::~Apartment() {
  if (owned)
    CoUninitialize();
}
CandidateWindow::~CandidateWindow() {
  // 先断开读屏拿着的提供者，再销毁窗口。
  accessible_.reset();
  if (tooltip_)
    DestroyWindow(tooltip_);
  if (window_) {
    TypingEffectSignal::instance().detach(window_);
    DestroyWindow(window_);
  }
  if (logo_)
    DestroyIcon(logo_);
  if (latched_process_)
    CloseHandle(latched_process_);
}
ID2D1Bitmap *CandidateWindow::logo_bitmap(int pixels) {
  if (pixels <= 0)
    return nullptr;
  if (!logo_ || logo_pixels_ != pixels) {
    // Loaded at the drawn size rather than LR_SHARED's cached standard size, as the floating toolbar does, so the mark is not resampled.
    const HANDLE loaded =
        LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_MSIME_LOGO),
                   IMAGE_ICON, pixels, pixels, LR_DEFAULTCOLOR);
    if (!loaded)
      return nullptr;
    if (logo_)
      DestroyIcon(logo_);
    logo_ = static_cast<HICON>(loaded);
    logo_pixels_ = pixels;
  }
  return device_.GetBitmapFromIcon(logo_, L"icon:candidate-logo:" +
                                              std::to_wstring(pixels));
}
void CandidateWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  painted_.reset();
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::set_theme_palette(CandidatePalette palette) {
  set_palette(std::move(palette));
  invalidate_geometry();
}
void CandidateWindow::invalidate_skin_images() {
  device_.ClearBitmapCache();
  invalidate_geometry();
}
bool CandidateWindow::set_fonts(const CandidateFontSettings &settings) {
  if (!settings.valid())
    return false;
  if (font_settings_ && *font_settings_ == settings)
    return true;
  try {
    // Resolve all names before replacing any live display state.
    auto primary = wide(settings.family);
    std::vector<std::wstring> fallback;
    fallback.reserve(settings.fallback.size());
    for (const auto &name : settings.fallback)
      fallback.push_back(wide(name));
    if (!installed_font(primary)) {
      for (const auto &name : fallback) {
        if (installed_font(name)) {
          primary = name;
          break;
        }
      }
    }
    primary = native_font_alias(primary);
    for (auto &name : fallback)
      name = native_font_alias(name);
    auto remembered = settings;
    font_family_.swap(primary);
    fallback_families_.swap(fallback);
    font_settings_ = std::move(remembered);
    font_size_ = settings.size;
    preedit_font_size_ = settings.preedit_size;
    font_fallback_.Reset();
    invalidate_geometry();
    return true;
  } catch (...) {
    return false;
  }
}
void CandidateWindow::set_layout(CandidateLayoutSettings settings) {
  if (horizontal_ == settings.horizontal && show_preedit_ == settings.show_preedit &&
      wubi_code_hint_ == settings.wubi_code_hint &&
      show_app_logo_ == settings.show_app_logo &&
      reserved_gloss_lines_ == settings.reserved_gloss_lines)
    return;
  horizontal_ = settings.horizontal;
  show_preedit_ = settings.show_preedit;
  show_app_logo_ = settings.show_app_logo;
  reserved_gloss_lines_ = settings.reserved_gloss_lines;
  // The hint is applied by the reader; remembering it here is what repaints an unchanged generation when it is toggled.
  wubi_code_hint_ = settings.wubi_code_hint;
  invalidate_geometry();
}
bool CandidateWindow::set_style(const CandidateWindowStyle &style) {
  if (!style.valid())
    return false;
  if (style == style_)
    return true;
  style_ = style;
  // A new scale changes every size measured in device pixels, and opacity and radius only show in a new frame; both start from a fresh layout.
  invalidate_geometry();
  return true;
}
void CandidateWindow::invalidate_geometry() {
  // Geometry can change without an Engine generation change. Never reuse old
  // hit rectangles or a pressed row; keep the input lease and pinned anchor.
  shown_.reset();
  painted_.reset();
  pressed_.reset();
  pressed_page_.reset();
  hovered_.reset();
  wheel_accumulator_ = 0;
  tallest_ = 0;
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::hide() {
  // The composition is over, so the next one starts its flip decision fresh.
  tallest_ = 0;
  // And its position fresh: "keep the first position" lasts until the card
  // disappears, so the next appearance anchors at the caret again.
  anchor_.reset();
  shown_.reset();
  painted_.reset();
  pressed_.reset();
  pressed_page_.reset();
  hovered_.reset();
  wheel_accumulator_ = 0;
  // 提示是候选窗拥有的弹出窗口，候选窗隐藏时它不会跟着消失，正开着的提示要收起来。
  if (tooltip_)
    SendMessageW(tooltip_, TTM_POP, 0, 0);
  // 隐藏的候选窗没有可读的东西。
  if (accessible_)
    accessible_->publish({AccessibleContainer::List, candidate_accessible_name, {}});
  ShowWindow(window_, SW_HIDE);
}
// Measuring the page reads Engine text, so unusable presentation data reaches
// this path as well as the paint one. The owner's thread learns nothing about
// it: the card gives up and stays hidden, exactly as the window procedure does.
void CandidateWindow::refresh() {
  DpiScope dpi_scope;
  if (failed_) {
    hide();
    return;
  }
  try {
    reposition();
  } catch (...) {
    fail(failure_at_stage("refresh", static_cast<uint32_t>(GetLastError())));
  }
}
void CandidateWindow::fail(ComponentFailureSite site) {
  if (!failed_)
    failure_site_ = site;
  failed_ = true;
  hide();
}
void CandidateWindow::set_foreground(HWND foreground,
                                     ForegroundPresentation presentation) {
  // 锁存记下的窗口矩形是在 refresh 的 PMv2 上下文里取的，这里要在同一个上下文里比较，否则缩放屏上会误判成矩形变了。
  DpiScope dpi_scope;
  DWORD pid = 0;
  if (foreground)
    GetWindowThreadProcessId(foreground, &pid);
  // 全屏与否决定按整块显示器还是按工作区钳制；变化时让下一次 refresh 重新排版。
  if ((presentation == ForegroundPresentation::Windowed) !=
      (presentation_ == ForegroundPresentation::Windowed))
    shown_.reset();
  // 这一轮是不是刚进入非窗口化：上一轮前台不是这个进程，或上一轮是窗口化。持续的非窗口化不算，因为 foreground_presentation 按 HWND 缓存 300ms，锁存刚触发时拿到的还是触发前的全屏结果。
  const bool entered_fullscreen =
      presentation != ForegroundPresentation::Windowed &&
      (foreground_pid_ != pid ||
       presentation_ == ForegroundPresentation::Windowed);
  foreground_ = foreground;
  foreground_pid_ = pid;
  presentation_ = presentation;
  if (latch_watch_) {
    const auto &watch = *latch_watch_;
    const DWORD watched_pid = client_pid(watch.ticket);
    const bool expired = GetTickCount64() - watch.since >= 2000;
    RECT rect{};
    const char *trigger = nullptr;
    if (!expired) {
      if (IsIconic(watch.window))
        trigger = "iconic";
      else if (pid != watched_pid)
        trigger = "foreground";
      else if (!GetWindowRect(watch.window, &rect) ||
               !EqualRect(&rect, &watch.rect))
        trigger = "rect";
      else if (display_changed_)
        trigger = "displaychange";
    }
    if (trigger) {
      if (latched_)
        release_latch(nullptr);
      latched_ = watch.ticket;
      latched_seen_fullscreen_ = false;
      // 留一个只能等待的句柄，进程退出时解除锁存：pid 会被系统复用，不解除的话下一个拿到这个 pid 的游戏会一直被抑制。打不开时锁存照样生效，只是少了这条解除途径。
      latched_process_ = OpenProcess(SYNCHRONIZE, FALSE, watched_pid);
      suppression_changes_.push_back(
          {CandidateSuppression::Latched, true, watched_pid, trigger});
    }
    if (trigger || expired)
      latch_watch_.reset();
  }
  display_changed_ = false;
  if (latched_ && latched_process_ &&
      WaitForSingleObject(latched_process_, 0) == WAIT_OBJECT_0)
    release_latch("exited");
  // 用户把游戏改成窗口化后，候选窗不会再把它挤出全屏。锁存之后要先见过这个进程重新回到非窗口化的前台，才认它变成窗口化：触发锁存的那次弹出本身就可能把游戏挤成窗口化，那时解除等于白锁。解除后重新允许盯梢，游戏回到全屏后的下一次弹出再盯 2 秒。
  if (latched_ && pid == client_pid(*latched_)) {
    if (entered_fullscreen) {
      latched_seen_fullscreen_ = true;
    } else if (latched_seen_fullscreen_ &&
               presentation == ForegroundPresentation::Windowed) {
      const DWORD released = client_pid(*latched_);
      release_latch("windowed");
      latch_armed_.erase(
          std::remove_if(latch_armed_.begin(), latch_armed_.end(),
                         [released](const PipeTicket &ticket) {
                           return client_pid(ticket) == released;
                         }),
          latch_armed_.end());
    }
  }
}
void CandidateWindow::release_latch(const char *cause) {
  // cause 为空表示被新的锁存顶替，不单独记一行解除。
  if (cause)
    suppression_changes_.push_back(
        {CandidateSuppression::Latched, false, client_pid(*latched_), cause});
  latched_.reset();
  if (latched_process_) {
    CloseHandle(latched_process_);
    latched_process_ = nullptr;
  }
}
void CandidateWindow::policy_hide(const CandidatePresentation &value) {
  hide();
  if (rendered_ && value.render_serial != receipted_serial_) {
    receipted_serial_ = value.render_serial;
    rendered_(value);
  }
}
void CandidateWindow::keep_on_top() {
  if (!shown_ || presentation_ != ForegroundPresentation::Fullscreen ||
      !foreground_pid_ || !IsWindowVisible(window_))
    return;
  const auto now = GetTickCount64();
  if (topmost_serial_ == shown_->render_serial || now - topmost_at_ < 1000)
    return;
  // 只认前台进程自己的窗口：厂商 overlay、录屏工具这类常驻置顶窗口压在上面时不去抢，否则每轮循环都会互相顶一次。
  HWND above = GetWindow(window_, GW_HWNDPREV);
  while (above && (!IsWindowVisible(above) ||
                   (GetWindowLongPtrW(above, GWL_EXSTYLE) & WS_EX_TRANSPARENT) != 0))
    above = GetWindow(above, GW_HWNDPREV);
  DWORD owner = 0;
  if (!above || !GetWindowThreadProcessId(above, &owner) ||
      owner != foreground_pid_)
    return;
  SetWindowPos(window_, HWND_TOPMOST, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
  topmost_serial_ = shown_->render_serial;
  topmost_at_ = now;
}
void CandidateWindow::reposition() {
  auto value = reader_();
  if (!value || !value->visible) {
    hide();
    return;
  }
  const DWORD pid = client_pid(value->lease.transport);
  // 同一个客户端换了登记代次，说明它断开后又连上了，锁存随之解除。
  if (latched_ && latched_->client == value->lease.transport.client &&
      !same_ticket(*latched_, value->lease.transport))
    release_latch("reconnected");
  // 游戏会话的兜底和独占抑制只看属于这个客户端进程的前台：用户 Alt-Tab 切走或前台是启动器时，不锚到、也不抑制在别的应用上。QUNS 不针对某个窗口，可能是另一个进程在独占，所以独占抑制同样要求 pid 相等。
  const bool foreground_client =
      value->game_host && foreground_ && foreground_pid_ == pid;
  const bool exclusive =
      foreground_client &&
      presentation_ == ForegroundPresentation::ExclusiveFullscreen;
  if (exclusive && exclusive_pid_ != pid) {
    if (exclusive_pid_)
      suppression_changes_.push_back({CandidateSuppression::ExclusiveFullscreen,
                                      false, exclusive_pid_, nullptr});
    suppression_changes_.push_back(
        {CandidateSuppression::ExclusiveFullscreen, true, pid, nullptr});
    exclusive_pid_ = pid;
  } else if (!exclusive && exclusive_pid_) {
    suppression_changes_.push_back({CandidateSuppression::ExclusiveFullscreen,
                                    false, exclusive_pid_, nullptr});
    exclusive_pid_ = 0;
  }
  // 独占全屏下外部窗口显示不出来，弹出来还可能把游戏挤出全屏；游戏会话本来就没人画候选，隐藏不会比现在差。
  if (exclusive ||
      (value->game_host && latched_ && client_pid(*latched_) == pid)) {
    policy_hide(*value);
    return;
  }
  RECT client_area{};
  bool foreground_owned = false;
  if (foreground_client && GetClientRect(foreground_, &client_area) &&
      !IsRectEmpty(&client_area)) {
    POINT corners[2] = {{client_area.left, client_area.top},
                        {client_area.right, client_area.bottom}};
    if (ClientToScreen(foreground_, &corners[0]) &&
        ClientToScreen(foreground_, &corners[1])) {
      client_area = {corners[0].x, corners[0].y, corners[1].x, corners[1].y};
      foreground_owned = true;
    }
  }
  // 游戏给不出可信锚点时放到游戏客户区左下部。替换放在跟随光标的锁定和下面的快照比较之前，同一个兜底点不会反复触发重绘。
  if (foreground_owned) {
    GameAnchorInput anchor_input;
    if (value->y != invalid_candidate_anchor_y)
      anchor_input.anchor = POINT{value->x, value->y};
    anchor_input.game_host = value->game_host;
    anchor_input.foreground_owned = true;
    anchor_input.client = client_area;
    // client_area 是 PMv2 下的物理像素，缩放取游戏所在显示器的有效 DPI，与它同一单位。
    anchor_input.scale =
        static_cast<double>(monitor_effective_dpi(
            MonitorFromWindow(foreground_, MONITOR_DEFAULTTONEAREST))) /
        96.0;
    if (const auto fallback = game_candidate_anchor(anchor_input)) {
      value->x = fallback->x;
      value->y = fallback->y;
    }
  }
  // The first show can arrive before TSF has produced a usable text extent.
  // Do not let card_bounds clamp the sentinel into the monitor work area; a
  // subsequent MoveCandidateWnd with a real anchor will retry this refresh.
  if (value->y == invalid_candidate_anchor_y) {
    policy_hide(*value);
    return;
  }
  if (value->candidates.size() > 9)
    throw std::invalid_argument("Oversized window page");
  // 候选窗口跟随光标. With following off the card stays where it first
  // appeared: the caret still moves as the user types, but the anchor this
  // layout uses does not. Everything downstream - the monitor it lands on, the
  // flip decision, the cached-frame comparison below - reads the anchored
  // copy, so a caret move alone no longer even wakes the window.
  // 打字特效浮层在候选窗收起之后（上屏、英文直输）还要知道光标在哪。
  typing_anchor_ = POINT{value->x, value->y};
  typing_anchor_foreground_ = GetForegroundWindow();
  auto anchored = *value;
  if (!follow_cursor_) {
    if (anchor_) {
      anchored.x = anchor_->x;
      anchored.y = anchor_->y;
    } else {
      anchor_ = POINT{value->x, value->y};
    }
  }
  value = anchored;
  // render_serial 也要比：读音和逐词拆解在同一个 Engine generation 上单独到达（CandidateMailbox::readings），只换了快照编号。
  if (shown_ && shown_dpi_ == GetDpiForWindow(window_) &&
      shown_->session == value->session &&
      shown_->generation == value->generation &&
      shown_->render_serial == value->render_serial && shown_->x == value->x &&
      shown_->y == value->y && shown_->lease.epoch == value->lease.epoch &&
      shown_->lease.token == value->lease.token &&
      same_ticket(shown_->lease.transport, value->lease.transport))
    return; // Do not create a self-sustaining WM_PAINT polling loop.
  const HMONITOR target =
      MonitorFromPoint({value->x, value->y}, MONITOR_DEFAULTTONEAREST);
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  if (!GetMonitorInfoW(target, &monitor))
    throw std::runtime_error("Candidate monitor unavailable");
  // 前台在这块显示器上全屏时任务栏不在，按整块显示器钳制，不然候选窗会停在任务栏原来的位置上面。
  const bool fullscreen =
      presentation_ != ForegroundPresentation::Windowed && foreground_ &&
      MonitorFromWindow(foreground_, MONITOR_DEFAULTTONEAREST) == target;
  const auto &work = fullscreen ? monitor.rcMonitor : monitor.rcWork;
  // Move a hidden one-pixel window onto the target monitor first. Its own DPI,
  // rather than process-global or previous-monitor DPI, determines the layout.
  if (MonitorFromWindow(window_, MONITOR_DEFAULTTONEAREST) != target) {
    hide();
    if (!SetWindowPos(window_, nullptr, work.left, work.top, 1, 1,
                      SWP_NOACTIVATE | SWP_NOZORDER))
      throw std::runtime_error("Candidate monitor move failed");
  }
  const auto dpi = GetDpiForWindow(window_);
  const auto bounds = card_bounds(*value, work, dpi);
  if (!SetWindowPos(window_, HWND_TOPMOST, bounds.x, bounds.y, bounds.width,
                    bounds.height, SWP_NOACTIVATE | SWP_SHOWWINDOW))
    throw std::runtime_error("Candidate positioning failed");
  // 游戏会话第一次在自己的几何全屏前台上弹出：盯住那个窗口 2 秒，见 set_foreground。
  if (foreground_owned &&
      presentation_ == ForegroundPresentation::Fullscreen &&
      std::none_of(latch_armed_.begin(), latch_armed_.end(),
                   [&](const PipeTicket &ticket) {
                     return same_ticket(ticket, value->lease.transport);
                   })) {
    RECT rect{};
    if (GetWindowRect(foreground_, &rect)) {
      // 只留最近的连接：断开的连接不会再弹出，留着只占地方。
      if (latch_armed_.size() >= 16)
        latch_armed_.erase(latch_armed_.begin());
      latch_armed_.push_back(value->lease.transport);
      latch_watch_ =
          LatchWatch{value->lease.transport, foreground_, rect, GetTickCount64()};
    }
  }
  shown_ = value;
  shown_dpi_ = dpi;
  InvalidateRect(window_, nullptr, FALSE);
}
// Measure the page, size the card from the shared geometry and keep it inside
// the work area. Half the work area caps each axis, as the shipped card does.
CandidateBounds CandidateWindow::card_bounds(const CandidatePresentation &value,
                                             const RECT &work, unsigned dpi) {
  const int64_t available_width = int64_t(work.right) - work.left;
  const int64_t available_height = int64_t(work.bottom) - work.top;
  if (available_width <= 0 || available_height <= 0)
    throw std::invalid_argument("Invalid candidate work area");
  device_.EnsureFactories();
  // The first layout precedes painting: do not let the format cache make font
  // selection depend on whether a previous frame has already been drawn.
  if (!fallback_families_.empty() && !font_fallback_)
    font_fallback_ = build_font_fallback(device_.GetDWriteFactory(),
                                        fallback_families_);
  const double scale = layout_scale(dpi);
  CandidateCardInput input;
  input.horizontal = horizontal_;
  input.preedit_visible = show_preedit_;
  input.font_size = font_size_;
  input.preedit_font_size = preedit_font_size_;
  input.max_width = static_cast<double>(available_width) / scale / 2.0;
  // A horizontal page of long candidates may widen to the work area less 16 DIPs on each side rather than start a second line.
  input.max_single_line_width =
      static_cast<double>(available_width) / scale - 2.0 * 16.0;
  input.max_height = static_cast<double>(available_height) / scale / 2.0;
  input.skin_min_width = skin_min_width_;
  input.logo_visible = show_app_logo_;
  input.reserved_secondary_lines = reserved_secondary_lines(value);
  if (horizontal_)
    input.reserved_secondary_height =
        reserved_secondary_height(input.reserved_secondary_lines);
  if (show_preedit_)
    input.preedit_width = measured_width(
        device_, wide(value.preedit), font_family_,
        static_cast<float>(preedit_font_size_), font_fallback_.Get(),
        DWRITE_FONT_WEIGHT_SEMI_BOLD);
  // 翻页和拼音共用首行；只要有 logo 或翻页，拼音隐藏时首行也照样画。
  input.page_width = measured_width(
      device_, pager_label(value), font_family_,
      static_cast<float>(CandidateCardMetrics{}.pager_font),
      font_fallback_.Get());
  input.items = measure_items(value);
  input.wrapped = wrap_measure(value);
  const auto card = candidate_card_size(input);
  const auto shadow_left =
      static_cast<int64_t>(std::lround(shadow_insets_.left * scale));
  const auto shadow_top =
      static_cast<int64_t>(std::lround(shadow_insets_.top * scale));
  const auto shadow_right =
      static_cast<int64_t>(std::lround(shadow_insets_.right * scale));
  const auto shadow_bottom =
      static_cast<int64_t>(std::lround(shadow_insets_.bottom * scale));
  const auto content_width =
      (std::min)(static_cast<int64_t>(card.width * scale + 0.5),
                 (std::max)(int64_t{1},
                            available_width - shadow_left - shadow_right));
  // The window has to be tall enough to hold the mascot as well, or the
  // artwork would be clipped by the window it overhangs.
  const int64_t decoration =
      decoration_image_.empty()
          ? 0
          : static_cast<int64_t>(decoration_top_ * scale + 0.5);
  decoration_offset_ = decoration_image_.empty()
                           ? 0.0f
                           : static_cast<float>(decoration_top_);
  const auto content_height =
      (std::min)(static_cast<int64_t>(card.height * scale + 0.5) + decoration,
                 (std::max)(int64_t{1},
                            available_height - shadow_top - shadow_bottom));
  // A vertical list grows as the user keeps typing. Deciding the flip from the
  // tallest it has been this composition keeps it on one side of the caret
  // instead of jumping below-to-above mid-word; tallest_ is cleared in hide().
  if (!horizontal_)
    tallest_ = (std::max)(tallest_, content_height);
  CandidatePlacementInput placement;
  placement.anchor_x = value.x;
  placement.anchor_y = value.y;
  placement.width = static_cast<int>(content_width);
  placement.height = static_cast<int>(content_height);
  placement.decision_height = static_cast<int>(
      horizontal_ ? content_height
                  : candidate_vertical_decision_height(tallest_, scale,
                                                       available_height));
  placement.work_left = work.left;
  placement.work_top = work.top;
  placement.work_right = work.right;
  placement.work_bottom = work.bottom;
  placement.scale = scale;
  // Place the visible card against the caret, while keeping its transparent
  // blur margins inside the work area. The helper returns the outer HWND.
  return candidate_shadow_bounds(placement, {static_cast<int>(shadow_left),
                                             static_cast<int>(shadow_top),
                                             static_cast<int>(shadow_right),
                                             static_cast<int>(shadow_bottom)});
}
// The shipped presenter draws three runs per candidate: the text with its badge, the annotation (辅助码) and the translation, the last at a smaller size. Measuring them apart is what lets a long annotation or translation wrap under the text instead of being clipped off the end of one long label. The translation run is candidate_secondary_text, which puts a Korean Hanja's 훈음 above its translation.
std::vector<CandidateItemWidths>
CandidateWindow::measure_items(const CandidatePresentation &value) {
  const auto metrics =
      candidate_card_metrics(font_size_, preedit_font_size_, show_preedit_);
  std::vector<CandidateItemWidths> items;
  items.reserve(value.candidates.size());
  for (const auto &candidate : value.candidates) {
    auto width = [&](const std::string &text, double size) {
      return measured_width(device_, wide(text), font_family_,
                            static_cast<float>(size), font_fallback_.Get());
    };
    items.push_back({width(candidate_primary_text(candidate), font_size_),
                     width(candidate.annotation, font_size_),
                     width(candidate_secondary_text(candidate),
                           metrics.translation_font),
                     candidate_secondary_lines(candidate)});
  }
  return items;
}
size_t CandidateWindow::reserved_secondary_lines(
    const CandidatePresentation &value) const {
  // 韩文汉字列表的每一行都在释义行画 훈음，不管翻译开关怎样，所以再多留一行，译文在它下面。
  const bool hanja = std::any_of(
      value.candidates.begin(), value.candidates.end(),
      [](const PresentationCandidate &candidate) { return !candidate.gloss.empty(); });
  // 这一页的方案或模式根本不请求释义（日文、网址模式这些）时，偏好算出的释义行一行也不留。
  return (value.shows_glosses ? reserved_gloss_lines_ : 0u) + (hanja ? 1u : 0u);
}
double CandidateWindow::reserved_secondary_height(size_t lines) {
  if (lines < 2)
    return 0.0;
  const auto metrics =
      candidate_card_metrics(font_size_, preedit_font_size_, show_preedit_);
  // 和 macOS reservedGlossHeightForFont 量「X\nX」一样：每行一个 X，宽度给足，不会折行。
  std::wstring placeholder = L"X";
  for (size_t line = 1; line < lines; ++line)
    placeholder += L"\nX";
  return wrapped_height(device_, placeholder, font_family_,
                        static_cast<float>(metrics.translation_font), 8192.0,
                        font_fallback_.Get());
}
CandidateWrapMeasure
CandidateWindow::wrap_measure(const CandidatePresentation &value) {
  const auto metrics =
      candidate_card_metrics(font_size_, preedit_font_size_, show_preedit_);
  // Copies, not references: the measure outlives neither call, but it must not depend on that.
  struct Runs {
    std::wstring text, annotation, translation;
  };
  std::vector<Runs> runs;
  runs.reserve(value.candidates.size());
  for (const auto &candidate : value.candidates)
    runs.push_back({wide(candidate_primary_text(candidate)),
                    wide(candidate.annotation),
                    wide(candidate_secondary_text(candidate))});
  return [this, runs = std::move(runs), font = static_cast<float>(font_size_),
          translation_font = static_cast<float>(metrics.translation_font)](
             size_t index, CandidateRun run, double width) {
    if (index >= runs.size())
      return 0.0;
    const auto &item = runs[index];
    const bool translation = run == CandidateRun::translation;
    return wrapped_height(device_,
                          run == CandidateRun::text ? item.text
                          : translation            ? item.translation
                                                   : item.annotation,
                          font_family_, translation ? translation_font : font,
                          width, font_fallback_.Get());
  };
}
void CandidateWindow::take_typing_effect() {
  const std::optional<uint32_t> packed = TypingEffectSignal::instance().take();
  if (!packed)
    return;
  // A 0 decodes to no combo and no flash, which kills both timers below and clears a count still on the card.
  effect_ = decode_typing_effect(*packed);
  effect_started_ = GetTickCount64();
  // The focused session's effect pack, published with the key; before any session has published one the preference-derived intensity and the host's own flash stand.
  if (const auto settings = unpack_typing_effect_settings(TypingEffectSignal::instance().settings()))
    effect_settings_ = *settings;
  else
    effect_settings_ = resolve_typing_effect_settings(effect_intensity_, std::nullopt, std::nullopt);
  // Windows' "Show animations" switch is its reduced motion setting: with it off the card does not flash, and the combo count still shows.
  BOOL animations = TRUE;
  if (!SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animations, 0))
    animations = TRUE;
  const bool power_saver = TypingEffectOverlay::power_saver();
  const auto drawn = typing_effect_drawn_style(effect_.style, animations != FALSE, power_saver);
  // 火花、光标行闪光和连击徽标交给光标处的浮层画；浮层不可用时卡片照旧自己画计数，各样式都闪卡片。
  effect_badge_elsewhere_ = false;
  if (typing_effect_presenter_) {
    TypingEffectPresentation presentation;
    presentation.effect = effect_;
    presentation.settings = effect_settings_;
    presentation.palette = TypingEffectSignal::instance().palette();
    presentation.card = typing_effect_card();
    presentation.candidate_window = window_;
    if (typing_anchor_ && GetForegroundWindow() == typing_anchor_foreground_)
      presentation.anchor = typing_anchor_;
    const auto channel = [](float value) {
      return static_cast<uint32_t>(std::lround((std::min)(1.0f, (std::max)(0.0f, value)) * 255.0f));
    };
    presentation.accent = (channel(palette_.accent.r) << 16) | (channel(palette_.accent.g) << 8) | channel(palette_.accent.b);
    presentation.animations = animations != FALSE;
    presentation.power_saver = power_saver;
    effect_badge_elsewhere_ = typing_effect_presenter_(presentation);
  }
  effect_flashing_ = animations && typing_effect_card_flashes(drawn, effect_badge_elsewhere_) &&
                     typing_effect_flash_alpha(effect_, effect_settings_.intensity, 0, effect_settings_.flash_millis) > 0.0f;
  // Power Mode 抖一下候选卡片，和 macOS 的 shakeLayer 一样；关掉动画或开着节电模式时不抖（drawn 已经退回了闪光）。
  effect_shaking_ = drawn == TypingEffectStyle::power_mode && IsWindowVisible(window_);
  if (effect_flashing_ || effect_shaking_)
    SetTimer(window_, typing_flash_timer, typing_flash_frame_millis, nullptr);
  else
    KillTimer(window_, typing_flash_timer);
  if (effect_.combo >= 2)
    SetTimer(window_, typing_combo_timer, typing_effect_combo_millis, nullptr);
  else
    KillTimer(window_, typing_combo_timer);
  if (IsWindowVisible(window_))
    InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::typing_effect_tick(UINT_PTR timer) {
  if (timer == typing_combo_timer) {
    KillTimer(window_, typing_combo_timer);
  } else {
    const uint64_t elapsed = GetTickCount64() - effect_started_;
    if (elapsed >= effect_settings_.flash_millis)
      effect_flashing_ = false;
    if (elapsed >= typing_shake_millis)
      effect_shaking_ = false;
    if (!effect_flashing_ && !effect_shaking_)
      KillTimer(window_, typing_flash_timer);
  }
  if (IsWindowVisible(window_))
    InvalidateRect(window_, nullptr, FALSE);
}
std::optional<TypingRect> CandidateWindow::typing_effect_card() const {
  if (!window_ || !shown_ || !IsWindowVisible(window_))
    return std::nullopt;
  RECT outer{};
  if (!GetWindowRect(window_, &outer))
    return std::nullopt;
  // 和 card_bounds 同样的换算：窗口四周是透明的阴影边距，顶上可能还有吉祥物那一条。
  const double scale = layout_scale(shown_dpi_);
  const double decoration = decoration_image_.empty() ? 0.0 : decoration_top_ * scale;
  return TypingRect{static_cast<float>(outer.left + std::lround(shadow_insets_.left * scale)),
                    static_cast<float>(outer.top + std::lround(shadow_insets_.top * scale + decoration)),
                    static_cast<float>(outer.right - std::lround(shadow_insets_.right * scale)),
                    static_cast<float>(outer.bottom - std::lround(shadow_insets_.bottom * scale))};
}
void CandidateWindow::paint() {
  DpiScope dpi_scope;
  Painting painting(window_);
  if (!painting.dc)
    throw std::runtime_error("Candidate painting unavailable");
  const auto value = reader_(); // Never paint the last cached owner's text.
  if (!value || !value->visible) {
    hide();
    return;
  }
  if (value->candidates.size() > 9)
    throw std::invalid_argument("Oversized window page");
  // The user's scale rides on the target's DPI, as the system scale does, so every DIP below - fonts, paddings, radius, shadow - grows with it and matches the size card_bounds gave the window. At 100% the window's own DPI stays the only source.
  device_.SetDpiOverride(
      style_.scale_percent == 100
          ? 0.0f
          : static_cast<float>(layout_scale(GetDpiForWindow(window_)) * 96.0));
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Candidate device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Candidate render target unavailable");
  // DrawText goes through the windows.h macro so the call matches whichever
  // name the Direct2D declaration picked up for this target.
  // 首行高度取决于有没有 logo 和翻页，必须和 card_bounds 量尺寸时一样算，行的位置和点击区域才对得上。
  const auto label = pager_label(*value);
  const auto metrics = candidate_card_metrics(font_size_, preedit_font_size_,
                                              show_preedit_, show_app_logo_,
                                              !label.empty());
  const auto size = target->GetSize();
  const auto frame = candidate_shadow_frame(
      size.width -
          static_cast<float>(shadow_insets_.left + shadow_insets_.right),
      size.height - decoration_offset_ -
          static_cast<float>(shadow_insets_.top + shadow_insets_.bottom),
      decoration_offset_, shadow_insets_);
  auto brush = [&](const CandidateColor &color) {
    auto *value = device_.GetSolidColorBrush(D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!value)
      throw std::runtime_error("Candidate brush unavailable");
    return value;
  };
  // Points are device independent here; the composition target carries the
  // scale, so the constructor's validated sizes go straight to DirectWrite.
  // Built once per paint and shared by every run below; the formats themselves
  // are cached by DeviceResources, so attaching here is what actually puts the
  // user's faces in front of the system chain.
  if (!fallback_families_.empty() && !font_fallback_)
    font_fallback_ = build_font_fallback(device_.GetDWriteFactory(),
                                         fallback_families_);
  // A run placed below the first line is top aligned and wraps, matching wrapped_height(); everything on a first line is centred in it and does not wrap. Candidate text wider than its column wraps too but stays centred in its measured box, as the shipped presenter draws it.
  auto format = [&](double points, DWRITE_TEXT_ALIGNMENT alignment,
                    bool wrap = false, bool centred = false,
                    DWRITE_FONT_WEIGHT weight = DWRITE_FONT_WEIGHT_NORMAL) {
    auto *value = device_.GetTextFormat(
        font_family_, static_cast<float>(points), weight, alignment,
        wrap && !centred ? DWRITE_PARAGRAPH_ALIGNMENT_NEAR
                         : DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        wrap ? DWRITE_WORD_WRAPPING_WRAP : DWRITE_WORD_WRAPPING_NO_WRAP);
    if (!value)
      throw std::runtime_error("Candidate text format unavailable");
    set_candidate_font_fallback(value, font_fallback_.Get());
    return value;
  };
  const float inset = palette_.border_width / 2.0f;
  const CandidateColor text_color = palette_.text;
  target->BeginDraw();
  // Clear to nothing: only the rounded card itself is opaque, so the corners
  // stay transparent rather than showing a square window edge.
  target->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
  // Power Mode 的抖动：整张卡片连同阴影横向位移一两个设备无关像素，阴影的透明边距装得下；抖完回到原位，点击区域不受影响。
  // 每帧都设一次：上一帧若在 EndDraw 之前抛出，留在目标上的位移不会带进这一帧。
  target->SetTransform(
      effect_shaking_
          ? D2D1::Matrix3x2F::Translation(
                typing_shake_offset(static_cast<uint32_t>(GetTickCount64() - effect_started_), effect_settings_.intensity),
                0.0f)
          : D2D1::Matrix3x2F::Identity());
  const D2D1_RECT_F card_rect{
      static_cast<float>(frame.card_left) + inset,
      static_cast<float>(frame.card_top) + inset,
      static_cast<float>(frame.card_left + frame.card_width) - inset,
      static_cast<float>(frame.card_top + frame.card_height) - inset};
  // The Fluent flyout shadow, 0 8px 16px: a 16px CSS blur is a Gaussian of sigma 8.
  const WindowShadowPass shadow_passes[] = {
      {8.0f, palette_.shadow_alpha, 0.0f, 8.0f},
  };
  // The user's radius, else the package's, else the theme's.
  const float radius = candidate_card_radius(style_, skin_radius_, palette_.radius);
  const float row_radius = candidate_row_radius(style_, palette_.item_radius, radius);
  draw_window_shadow_passes(target, card_rect, radius, shadow_passes,
                            std::size(shadow_passes));
  const D2D1_ROUNDED_RECT card{card_rect, radius, radius};
  // The user's opacity fades the card itself - surface, package background and border - and nothing drawn on it, so the text stays as legible as the theme made it.
  target->FillRoundedRectangle(card, brush(candidate_faded(palette_.surface, style_)));
  const float background_opacity = background_.opacity * style_.opacity();
  const uint64_t effect_elapsed = GetTickCount64() - effect_started_;
  const float flash = effect_flashing_ ? typing_effect_flash_alpha(effect_, effect_settings_.intensity, effect_elapsed, effect_settings_.flash_millis) : 0.0f;
  // The package background sits on the surface and under the border and the text, masked by the card's rounded outline.
  if (!background_.image.empty() && background_opacity > 0.0f) {
    D2D1_SIZE_F natural{};
    auto *bitmap = device_.GetBitmapFromFile(background_.image, &natural);
    const auto rects = candidate_background_rects(
        background_.fit,
        {card_rect.left, card_rect.top, card_rect.right, card_rect.bottom},
        natural.width, natural.height);
    Microsoft::WRL::ComPtr<ID2D1Factory> factory;
    target->GetFactory(&factory);
    Microsoft::WRL::ComPtr<ID2D1RoundedRectangleGeometry> outline;
    if (bitmap && rects && factory &&
        SUCCEEDED(factory->CreateRoundedRectangleGeometry(card, &outline))) {
      target->PushLayer(D2D1::LayerParameters(D2D1::InfiniteRect(), outline.Get()),
                        nullptr);
      const auto &to = rects->destination;
      const auto &from = rects->source;
      const D2D1_RECT_F source{from.left, from.top, from.right, from.bottom};
      target->DrawBitmap(bitmap, D2D1_RECT_F{to.left, to.top, to.right, to.bottom},
                         background_opacity, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                         &source);
      target->PopLayer();
    }
  }
  target->DrawRoundedRectangle(card, brush(candidate_faded(palette_.border, style_)),
                               palette_.border_width);
  // The typing flash: a faint accent wash over the surface, under the text, and an accent outline that grows with the style. Both fade with the flash. An effect pack's first colour takes the accent's place.
  if (flash > 0.0f) {
    const auto flash_color = effect_settings_.color ? candidate_rgb(*effect_settings_.color) : palette_.accent;
    // 不透明度量化成有限的几档：brush() 按颜色缓存画刷，每帧一个新的透明度会让缓存一直变大。
    auto wash = flash_color;
    wash.a = typing_effect_quantized_alpha(flash * 0.12f);
    target->FillRoundedRectangle(card, brush(wash));
    auto outline = flash_color;
    outline.a = typing_effect_quantized_alpha(flash);
    target->DrawRoundedRectangle(card, brush(outline),
                                 palette_.border_width + static_cast<float>(static_cast<uint32_t>(effect_.style)));
  }
  // The mascot, drawn last so it sits over the card's top edge - that overlap
  // is the whole point of the decoration.
  if (!decoration_image_.empty() && decoration_offset_ > 0.0f) {
    D2D1_SIZE_F natural{};
    if (auto *bitmap = device_.GetBitmapFromFile(decoration_image_, &natural)) {
      // A package gives a width, not a box: the height follows the image's own aspect ratio, and artwork too tall for the band plus the pad_y overlap shrinks uniformly instead of being squashed. Placed along the card's top edge as the manifest aligns it, as the settings preview places it.
      if (const auto rect = candidate_decoration_rect(
              decoration_align_, static_cast<float>(frame.card_left),
              static_cast<float>(frame.card_left + frame.card_width),
              static_cast<float>(frame.card_top),
              static_cast<float>(metrics.pad_x),
              static_cast<float>(metrics.pad_y), decoration_offset_,
              static_cast<float>(decoration_width_), natural.width,
              natural.height))
        target->DrawBitmap(
            bitmap, D2D1_RECT_F{rect->left, rect->top, rect->right, rect->bottom},
            1.0f, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    }
  }
  // Fluent 风格的首行：左边是水杉 logo（`show_app_logo` 关掉时不画）和强调色半粗体的拼音，右边是次要色的页码和上一页、下一页箭头。只要有 logo 或翻页，拼音隐藏时这一行也在。
  auto box = [&frame](const CandidateRowBounds &bounds) {
    return D2D1_RECT_F{static_cast<float>(frame.card_left + bounds.left),
                       static_cast<float>(frame.card_top + bounds.top),
                       static_cast<float>(frame.card_left + bounds.right),
                       static_cast<float>(frame.card_top + bounds.bottom)};
  };
  // Loaded at the size it is drawn at, in real pixels, as the floating toolbar loads it, and skipped rather than substituted if the icon will not load.
  if (metrics.logo_visible)
    if (auto *logo = logo_bitmap(static_cast<int>(std::lround(
            metrics.logo_side * layout_scale(GetDpiForWindow(window_))))))
      target->DrawBitmap(logo, box(candidate_logo_bounds(metrics)), 1.0f,
                         D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
  const std::optional<CandidatePagerLayout> pager = candidate_pager_layout(
      frame.card_width,
      measured_width(device_, label, font_family_,
                     static_cast<float>(metrics.pager_font),
                     font_fallback_.Get()),
      metrics);
  if (pager) {
    target->DrawText(label.c_str(), static_cast<UINT32>(label.size()),
                      format(metrics.pager_font, DWRITE_TEXT_ALIGNMENT_TRAILING),
                      box(pager->indicator), brush(palette_.number));
    // The previous arrow is dimmed on the first page, where it does nothing. The next one never is: the Engine fetches candidates lazily, so the page count grows as the user pages and the last page counted is not known to be the last.
    auto previous_color = palette_.number;
    if (value->page == 0)
      previous_color.a *= 0.4f;
    const wchar_t previous_glyph[] = L"\u2039", next_glyph[] = L"\u203A";
    target->DrawText(previous_glyph, 1,
                      format(metrics.pager_font, DWRITE_TEXT_ALIGNMENT_CENTER),
                      box(pager->previous), brush(previous_color));
    target->DrawText(next_glyph, 1,
                      format(metrics.pager_font, DWRITE_TEXT_ALIGNMENT_CENTER),
                      box(pager->next), brush(palette_.number));
  }
  const D2D1_RECT_F preedit_rect{
      static_cast<float>(frame.card_left + candidate_preedit_left(metrics)),
      static_cast<float>(frame.card_top + metrics.pad_y),
      static_cast<float>(frame.card_left +
                         (pager ? pager->left - metrics.pager_gap
                                : frame.card_width - metrics.pad_x / 2.0)),
      static_cast<float>(frame.card_top) +
          static_cast<float>(metrics.pad_y + metrics.preedit_row)};
  // The combo count, right-aligned at the end of the preedit row before the pager. Drawn only where it fits beside the reading, so it never covers what the user is typing.
  if (!effect_badge_elsewhere_ && typing_effect_shows_combo(effect_.combo, effect_elapsed)) {
    const auto combo = L"\u00D7" + std::to_wstring(effect_.combo);
    const double combo_width = measured_width(
        device_, combo, font_family_, static_cast<float>(metrics.pager_font),
        font_fallback_.Get(), DWRITE_FONT_WEIGHT_SEMI_BOLD);
    const double reading_width =
        show_preedit_ ? measured_width(device_, wide(value->preedit), font_family_,
                                       static_cast<float>(preedit_font_size_),
                                       font_fallback_.Get(), DWRITE_FONT_WEIGHT_SEMI_BOLD)
                      : 0.0;
    if (static_cast<double>(preedit_rect.left) + reading_width + metrics.pager_gap + combo_width <=
        static_cast<double>(preedit_rect.right))
      target->DrawText(combo.c_str(), static_cast<UINT32>(combo.size()),
                        format(metrics.pager_font, DWRITE_TEXT_ALIGNMENT_TRAILING, false,
                               false, DWRITE_FONT_WEIGHT_SEMI_BOLD),
                        preedit_rect, brush(flash > 0.0f && effect_.tier_up ? palette_.accent : palette_.number),
                        D2D1_DRAW_TEXT_OPTIONS_CLIP);
  }
  if (show_preedit_) {
    const D2D1_RECT_F &rect = preedit_rect;
    const auto text = wide(value->preedit);
    // Same clamp, same reason as the candidate rows below: a long enough reading would otherwise be drawn past the card, or under the pager.
    if (rect.right > rect.left)
      target->DrawText(text.c_str(), static_cast<UINT32>(text.size()),
                        format(preedit_font_size_, DWRITE_TEXT_ALIGNMENT_LEADING,
                               false, false, DWRITE_FONT_WEIGHT_SEMI_BOLD),
                        rect, brush(palette_.accent),
                        D2D1_DRAW_TEXT_OPTIONS_CLIP);
    // The insertion point. Without it, moving left or right inside a long pinyin string gave no indication of where the next key would land - and the settings preview drew a caret the real window never did.
    if (value->preedit_caret != std::string::npos &&
        value->preedit_caret <= value->preedit.size()) {
      const auto before =
          wide(value->preedit.substr(0, value->preedit_caret));
      const auto offset = measured_width(
          device_, before, font_family_,
          static_cast<float>(preedit_font_size_), font_fallback_.Get(),
          DWRITE_FONT_WEIGHT_SEMI_BOLD);
      const float x = rect.left + static_cast<float>(offset);
      // A hairline rather than a filled block, so it does not obscure the character it sits before.
      const float inset_y = static_cast<float>(metrics.preedit_row) * 0.15f;
      // A caret past the clipped preedit is not drawn, so it cannot land on the pager.
      if (x <= rect.right)
        target->FillRectangle(
            D2D1_RECT_F{x, rect.top + inset_y, x + 1.5f, rect.bottom - inset_y},
            brush(palette_.accent));
    }
  }
  // The selection number keeps its own column so candidates start on one
  // vertical line, as the shipped card does.
  const float number = static_cast<float>(font_size_) * 0.8f;
  const float gutter = static_cast<float>(metrics.number_and_bar);
  const size_t count = value->candidates.size();
  // Laid out at the width actually drawn, which the work area may have narrowed below what card_bounds asked for.
  const size_t reserved_lines = horizontal_ ? reserved_secondary_lines(*value) : 0;
  auto rows = candidate_page_layout(
      measure_items(*value), frame.card_width, metrics, horizontal_,
      wrap_measure(*value),
      candidate_reserved_row_height(metrics, reserved_lines,
                                    reserved_secondary_height(reserved_lines)));
  const float first_line = static_cast<float>(metrics.candidate_row);
  for (size_t i = 0; i < count; ++i) {
    const auto &row = rows[i].bounds;
    const auto &item = rows[i].item;
    // Rows are laid out in card coordinates; the decoration strip sits above
    // the card, so every row moves down with it. Without this the rows would
    // be drawn over the artwork and the hit test below would disagree.
    const D2D1_RECT_F rect{static_cast<float>(frame.card_left + row.left),
                           static_cast<float>(frame.card_top + row.top),
                           static_cast<float>(frame.card_left + row.right),
                           static_cast<float>(frame.card_top + row.bottom)};
    if (value->candidates[i].highlighted || hovered_ == i) {
      const D2D1_ROUNDED_RECT selection{rect, row_radius, row_radius};
      target->FillRoundedRectangle(selection, brush(value->candidates[i].highlighted
                                                        ? palette_.selected
                                                        : palette_.hover));
      if (value->candidates[i].highlighted && palette_.show_selected_bar) {
        const auto extent = candidate_selection_bar(
            rect.left, rect.top, rect.bottom, metrics.candidate_row);
        const float bar_radius =
            static_cast<float>(candidate_selection_bar_width * 0.5);
        const D2D1_ROUNDED_RECT bar{{static_cast<float>(extent.left),
                                     static_cast<float>(extent.top),
                                     static_cast<float>(extent.right),
                                     static_cast<float>(extent.bottom)},
                                    bar_radius, bar_radius};
        target->FillRoundedRectangle(bar, brush(palette_.accent));
      }
    }
    // Alpha 0 means the skin named no selected colour, so the row keeps its
    // normal one. Skins that fill the selection with an opaque accent set it,
    // because their unselected text would otherwise be unreadable on the fill.
    const bool selected = value->candidates[i].highlighted;
    const auto number_color = candidate_row_number_color(palette_, selected);
    const auto row_text_color =
        candidate_row_text_color(palette_, text_color, selected,
                                 value->candidates[i].fixed_position != 0);
    const auto number_label = std::to_wstring(i + 1);
    target->DrawText(number_label.c_str(), static_cast<UINT32>(number_label.size()),
                      format(font_size_, DWRITE_TEXT_ALIGNMENT_TRAILING),
                      D2D1_RECT_F{rect.left, rect.top, rect.left + number,
                                  rect.top + first_line},
                      brush(number_color));
    const auto text = wide(candidate_primary_text(value->candidates[i]));
    // Text wider than its column was laid out wrapped (wrapped_height() at this same width), and the row already grew by that height, so it wraps here inside the row that hit testing uses. Text that fits keeps the single-line format, so rounding cannot wrap what was laid out as one line. Still clipped to the row as a guard: without it anything the layout did not account for would paint past the card edge onto the transparent shadow margin.
    target->DrawText(
        text.c_str(), static_cast<UINT32>(text.size()),
        format(font_size_, DWRITE_TEXT_ALIGNMENT_LEADING, item.text_wrapped, true),
        D2D1_RECT_F{rect.left + gutter, rect.top, rect.right,
                    rect.top + static_cast<float>(item.text_height)},
        brush(row_text_color), D2D1_DRAW_TEXT_OPTIONS_CLIP);
    // The annotation keeps the plain text colour even on a fixed-position row, and follows the selected text colour like the text does. The translation is the theme's secondary colour: a package's translation colour, or else the number colour (selected_number on the selected row).
    const auto annotation_color =
        candidate_row_text_color(palette_, text_color, selected, false);
    const auto translation_color =
        candidate_row_translation_color(palette_, selected);
    // Runs extend to the row's right edge rather than their measured width, so rounding cannot wrap or clip a run that was laid out as fitting.
    auto draw_run = [&](const std::string &run, const CandidateRunBox &box,
                        double size, const CandidateColor &color) {
      if (run.empty() || box.width <= 0.0)
        return;
      const auto run_text = wide(run);
      const float left = rect.left + gutter + static_cast<float>(box.x);
      const float top = rect.top + static_cast<float>(box.y);
      target->DrawText(run_text.c_str(), static_cast<UINT32>(run_text.size()),
                        format(size, DWRITE_TEXT_ALIGNMENT_LEADING, box.below),
                        D2D1_RECT_F{left, top, rect.right,
                                    top + static_cast<float>(box.height)},
                        brush(color), D2D1_DRAW_TEXT_OPTIONS_CLIP);
    };
    draw_run(value->candidates[i].annotation, item.annotation, font_size_,
             annotation_color);
    const auto secondary = candidate_secondary_text(value->candidates[i]);
    // Tab 预选的释义列在高亮候选上画下划线，和 macOS 候选按钮的 armedGlossColumn 一样；数字、空格会上屏这一列。
    const auto armed = selected ? candidate_gloss_column_range(value->candidates[i],
                                                               value->armed_gloss_column)
                                : std::nullopt;
    Microsoft::WRL::ComPtr<IDWriteTextLayout> underlined;
    auto *factory = device_.GetDWriteFactory();
    if (armed && factory && item.translation.width > 0.0) {
      const auto run_text = wide(secondary);
      const auto start = wide(secondary.substr(0, armed->first)).size();
      const auto length = wide(secondary.substr(armed->first, armed->second)).size();
      const float left = rect.left + gutter + static_cast<float>(item.translation.x);
      if (SUCCEEDED(factory->CreateTextLayout(
              run_text.c_str(), static_cast<UINT32>(run_text.size()),
              format(metrics.translation_font, DWRITE_TEXT_ALIGNMENT_LEADING,
                     item.translation.below),
              (std::max)(rect.right - left, 0.0f),
              static_cast<float>(item.translation.height), underlined.GetAddressOf())) &&
          underlined &&
          SUCCEEDED(underlined->SetUnderline(
              TRUE, DWRITE_TEXT_RANGE{static_cast<UINT32>(start), static_cast<UINT32>(length)})))
        target->DrawTextLayout(
            D2D1_POINT_2F{left, rect.top + static_cast<float>(item.translation.y)},
            underlined.Get(), brush(translation_color), D2D1_DRAW_TEXT_OPTIONS_CLIP);
      else
        underlined.Reset();
    }
    if (!underlined)
      draw_run(secondary, item.translation, metrics.translation_font, translation_color);
  }
  target->SetTransform(D2D1::Matrix3x2F::Identity());
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  if (SUCCEEDED(drawn) && FAILED(device_.Present()))
    throw std::runtime_error("Candidate presentation failed");
  // Cast rather than compare directly: HRESULT is signed and the macro is
  // unsigned in some SDK and MinGW versions, which makes the comparison a
  // warning on one toolchain and silent on another.
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    // Losing the device is not a presentation failure; rebuild on the next
    // refresh rather than hiding a live composition. Clearing shown_ is what
    // makes that rebuild reachable: reposition() returns early while the cached
    // frame still matches, so leaving it set would suppress the very repaint
    // this path is counting on, and painted_ would stay behind for good. That
    // also strands clicks, because hit() refuses without a painted page.
    device_.DiscardTarget();
    shown_.reset();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Candidate drawing failed");
  painted_ = value;
  painted_rows_ = std::move(rows);
  painted_pager_ = pager;
  painted_dpi_ = GetDpiForWindow(window_);
  sync_tooltips();
  if (accessible_)
    accessible_->publish(candidate_accessible_tree(
        *painted_, painted_rows_, painted_pager_, metrics, show_preedit_,
        CandidateAccessibleFrame{frame.card_left, frame.card_top, frame.card_width,
                                 layout_scale(painted_dpi_)},
        static_cast<bool>(click_), static_cast<bool>(page_), MSIME_EDITION_DISPLAY_NAME_UTF8));
  if (rendered_)
    rendered_(*painted_);
}
void CandidateWindow::sync_tooltips() {
  if (!tooltip_ || !painted_)
    return;
  // 行在卡片坐标里，换回客户区像素：和 hit() 的换算相反，加上阴影边距和装饰图让出的高度。
  const double scale = layout_scale(painted_dpi_);
  std::vector<RECT> rects;
  if (painted_->pointer_input) {
    rects.reserve(painted_rows_.size());
    for (const auto &row : painted_rows_)
      rects.push_back(
          {static_cast<LONG>(std::floor((row.bounds.left + shadow_insets_.left) * scale)),
           static_cast<LONG>(std::floor((row.bounds.top + shadow_insets_.top + decoration_offset_) * scale)),
           static_cast<LONG>(std::ceil((row.bounds.right + shadow_insets_.left) * scale)),
           static_cast<LONG>(std::ceil((row.bounds.bottom + shadow_insets_.top + decoration_offset_) * scale))});
  }
  const auto same = [](const RECT &a, const RECT &b) {
    return a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom;
  };
  if (tooltip_serial_ == painted_->render_serial && rects.size() == tooltip_rects_.size() &&
      std::equal(rects.begin(), rects.end(), tooltip_rects_.begin(), same))
    return;
  TTTOOLINFOW tool{};
  // V2 的大小在没有 comctl32 v6 清单的进程里也被接受；带 lpReserved 的完整大小会让旧版控件拒绝登记。
  tool.cbSize = TTTOOLINFOW_V2_SIZE;
  tool.hwnd = window_;
  for (size_t id = 0; id < tooltip_rects_.size(); ++id) {
    tool.uId = id;
    SendMessageW(tooltip_, TTM_DELTOOLW, 0, reinterpret_cast<LPARAM>(&tool));
  }
  tooltip_rects_.clear();
  tooltip_serial_ = painted_->render_serial;
  for (size_t index = 0; index < rects.size(); ++index) {
    tool = {};
    tool.cbSize = TTTOOLINFOW_V2_SIZE;
    tool.uFlags = TTF_SUBCLASS;
    tool.hwnd = window_;
    tool.uId = index;
    tool.rect = rects[index];
    tool.lpszText = LPSTR_TEXTCALLBACKW;
    if (!SendMessageW(tooltip_, TTM_ADDTOOLW, 0, reinterpret_cast<LPARAM>(&tool)))
      break;
    tooltip_rects_.push_back(rects[index]);
  }
}
void CandidateWindow::invoke_accessible(int id, LPARAM token) {
  if (!accessible_ || !accessible_->current(token) || !painted_ ||
      !painted_->pointer_input || !IsWindowVisible(window_))
    return;
  const auto &value = *painted_;
  // 翻页与点箭头一样一次一页，带上画出来的那一页的身份，页已经换了时 Server 会拒绝。
  if (id == candidate_accessible_previous || id == candidate_accessible_next) {
    const bool previous = id == candidate_accessible_previous;
    if (page_ && painted_pager_ && (!previous || value.page > 0))
      page_(CandidatePage{value.lease, value.session, value.generation, previous, 1u});
    return;
  }
  if (!click_ || id < 1 || static_cast<size_t>(id) > value.candidates.size())
    return;
  const auto &candidate = value.candidates[static_cast<size_t>(id - 1)];
  click_(CandidateClick{value.lease, candidate.session, candidate.generation, candidate.index});
}
std::optional<CandidateClick> CandidateWindow::hit(int x, int y) {
  if (!click_ || !painted_ || !painted_->pointer_input || !IsWindowVisible(window_))
    return std::nullopt;
  RECT bounds{};
  if (!GetClientRect(window_, &bounds))
    return std::nullopt;
  const double scale = layout_scale(painted_dpi_);
  // Convert from physical client pixels into the visible card. The transparent
  // blur margins and decoration are deliberately not interactive.
  const double card_x = x / scale - shadow_insets_.left;
  const double card_y =
      y / scale - shadow_insets_.top - static_cast<double>(decoration_offset_);
  const double card_width =
      bounds.right / scale - shadow_insets_.left - shadow_insets_.right;
  const double card_height = bounds.bottom / scale - shadow_insets_.top -
                             shadow_insets_.bottom - decoration_offset_;
  if (card_x < 0.0 || card_y < 0.0)
    return std::nullopt;
  const auto row =
      candidate_card_hit(card_x, card_y, card_width, card_height, painted_rows_);
  if (!row || *row >= painted_->candidates.size())
    return std::nullopt;
  const auto &candidate = painted_->candidates[*row];
  return CandidateClick{painted_->lease, candidate.session,
                        candidate.generation, candidate.index};
}
std::optional<bool> CandidateWindow::pager_hit(int x, int y) {
  if (!page_ || !painted_ || !painted_->pointer_input || !painted_pager_ ||
      !IsWindowVisible(window_))
    return std::nullopt;
  const double scale = layout_scale(painted_dpi_);
  // The same conversion into the visible card as hit().
  const double card_x = x / scale - shadow_insets_.left;
  const double card_y =
      y / scale - shadow_insets_.top - static_cast<double>(decoration_offset_);
  return candidate_pager_hit(card_x, card_y, painted_pager_,
                             painted_->page == 0);
}
void CandidateWindow::show_context_menu(const CandidateClick &click,
                                        POINT client_point) {
  if (!painted_ || !click_)
    return;
  const auto candidate = std::find_if(
      painted_->candidates.begin(), painted_->candidates.end(),
      [&](const PresentationCandidate &item) {
        return item.session == click.session &&
               item.generation == click.generation &&
               item.index == click.index;
      });
  if (candidate == painted_->candidates.end())
    return;
  const auto text = wide(candidate->text);
  size_t code_points = 0;
  for (size_t i = 0; i < text.size(); ++i) {
    ++code_points;
    if (i + 1 < text.size() &&
        IS_HIGH_SURROGATE(text[i]) && IS_LOW_SURROGATE(text[i + 1]))
      ++i;
  }

  // The flyout is not modal. TrackPopupMenuEx ran a nested message loop, and the Server's pump is a bounded PeekMessage batch that also applies preference changes, syncs Caps Lock and drives the toolbar - so for as long as the menu was open, none of that ran.
  if (!flyout_)
    flyout_ = std::make_unique<CandidateFlyoutWindow>(
        [this](const CandidateMenuChoice &choice) {
          if (!click_)
            return;
          if (const auto action =
                  menu_target_.choose(choice.command, choice.position))
            click_(*action);
        });
  // Every opening acts on its own candidate and wears the current theme: the flyout is reused, so neither may be fixed at the first right click. The reference rebuilds its menu on every open for the same effect (candidate_presenter.cpp:560-563).
  menu_target_.open(click);
  flyout_->set_palette(palette_);
  POINT screen = client_point;
  if (!ClientToScreen(window_, &screen))
    throw std::runtime_error("Candidate context menu position unavailable");
  flyout_->open(screen.x, screen.y, code_points, candidate->actions_available,
                candidate->fixed_position);
}
LRESULT CALLBACK CandidateWindow::procedure(HWND window, UINT message,
                                            WPARAM wparam,
                                            LPARAM lparam) noexcept {
  auto *self = reinterpret_cast<CandidateWindow *>(
      GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<CandidateWindow *>(
        reinterpret_cast<CREATESTRUCTW *>(lparam)->lpCreateParams);
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
    self->window_ = window;
  }
  if (self) {
    try {
      switch (message) {
      case WM_MOUSEACTIVATE:
        return self->click_ ? MA_NOACTIVATE : MA_NOACTIVATEANDEAT;
      case WM_MOUSEWHEEL: {
        if (!self->mouse_wheel_) {
          // Keep the opt-in contract shared with the Server. Returning the
          // message to the default procedure preserves the pre-feature
          // behavior when wheel paging is disabled instead of swallowing a
          // wheel event over this NOACTIVATE window.
          self->wheel_accumulator_ = 0;
          return DefWindowProcW(window, message, wparam, lparam);
        }
        if (!self->page_ || !self->painted_ || !self->painted_->pointer_input ||
            !IsWindowVisible(window)) {
          self->wheel_accumulator_ = 0;
          return 0;
        }
        const auto steps = consume_candidate_wheel_delta(
            self->wheel_accumulator_, GET_WHEEL_DELTA_WPARAM(wparam),
            WHEEL_DELTA);
        const auto &value = *self->painted_;
        if (steps.page_up > 0)
          self->page_(CandidatePage{value.lease, value.session, value.generation,
                                    true, static_cast<unsigned>(steps.page_up), true});
        if (steps.page_down > 0)
          self->page_(CandidatePage{value.lease, value.session, value.generation,
                                    false, static_cast<unsigned>(steps.page_down), true});
        return 0;
      }
      case WM_LBUTTONDOWN:
        self->pressed_page_ = self->pager_hit(static_cast<short>(LOWORD(lparam)),
                                              static_cast<short>(HIWORD(lparam)));
        self->pressed_ = self->pressed_page_
                             ? std::nullopt
                             : self->hit(static_cast<short>(LOWORD(lparam)),
                                         static_cast<short>(HIWORD(lparam)));
        if (self->pressed_ || self->pressed_page_) {
          SetCapture(window);
          TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
          if (!TrackMouseEvent(&track)) {
            self->pressed_.reset();
            self->pressed_page_.reset();
          }
        }
        return 0;
      case WM_MOUSEMOVE: {
        TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
        TrackMouseEvent(&track);
        const auto click = self->hit(static_cast<short>(LOWORD(lparam)),
                                     static_cast<short>(HIWORD(lparam)));
        const auto arrow = self->pager_hit(static_cast<short>(LOWORD(lparam)),
                                           static_cast<short>(HIWORD(lparam)));
        SetCursor(LoadCursorW(nullptr, click || arrow ? wide_cursor(IDC_HAND)
                                                      : wide_cursor(IDC_ARROW)));
        std::optional<size_t> hovered;
        if (click && self->painted_) {
          for (size_t i = 0; i < self->painted_->candidates.size(); ++i)
            if (self->painted_->candidates[i].index == click->index) hovered = i;
        }
        if (hovered != self->hovered_) { self->hovered_ = hovered; InvalidateRect(window, nullptr, FALSE); }
        return 0;
      }
      case WM_LBUTTONUP: {
        // Take the press before releasing capture. ReleaseCapture delivers
        // WM_CAPTURECHANGED to this window, and that is the cancellation path -
        // it clears pressed_. Releasing first therefore destroys the very press
        // this handler is about to read, and the click never reaches click_.
        const auto pressed = self->pressed_;
        const auto pressed_page = self->pressed_page_;
        self->pressed_.reset();
        self->pressed_page_.reset();
        if (GetCapture() == window) ReleaseCapture();
        if (pressed_page) {
          // One page per click, in the direction of the arrow the press started on. The painted page's identity goes with it, so the Server refuses a click on a page that has since changed.
          if (self->pager_hit(static_cast<short>(LOWORD(lparam)),
                              static_cast<short>(HIWORD(lparam))) ==
              pressed_page) {
            const auto &value = *self->painted_;
            self->page_(CandidatePage{value.lease, value.session,
                                      value.generation, *pressed_page, 1u});
          }
          return 0;
        }
        const auto hit = self->hit(static_cast<short>(LOWORD(lparam)),
                                   static_cast<short>(HIWORD(lparam)));
        if (pressed && hit && pressed->session == hit->session &&
            pressed->generation == hit->generation &&
            pressed->index == hit->index &&
            pressed->lease.epoch == hit->lease.epoch &&
            pressed->lease.token == hit->lease.token &&
            same_ticket(pressed->lease.transport, hit->lease.transport))
          self->click_(*hit);
        return 0;
      }
      case WM_RBUTTONUP: {
        const auto hit = self->hit(static_cast<short>(LOWORD(lparam)),
                                   static_cast<short>(HIWORD(lparam)));
        if (hit)
          self->show_context_menu(*hit,
                                  POINT{static_cast<short>(LOWORD(lparam)),
                                        static_cast<short>(HIWORD(lparam))});
        return 0;
      }
      case WM_CANCELMODE:
      case WM_CAPTURECHANGED:
      case WM_MOUSELEAVE:
        if (GetCapture() == window) ReleaseCapture();
        self->pressed_.reset();
        self->pressed_page_.reset();
        self->hovered_.reset();
        SetCursor(LoadCursorW(nullptr, wide_cursor(IDC_ARROW)));
        return 0;
      case WM_NOTIFY: {
        auto *header = reinterpret_cast<NMHDR *>(lparam);
        if (header && self->tooltip_ && header->hwndFrom == self->tooltip_ &&
            header->code == TTN_GETDISPINFOW) {
          auto *info = reinterpret_cast<NMTTDISPINFOW *>(lparam);
          const auto index = static_cast<size_t>(header->idFrom);
          self->tooltip_text_.clear();
          if (self->painted_ && index < self->painted_->candidates.size())
            self->tooltip_text_ =
                tooltip_wide(candidate_tooltip_text(self->painted_->candidates[index]));
          info->lpszText = self->tooltip_text_.data();
          return 0;
        }
        break;
      }
      case WM_ERASEBKGND:
        return 1;
      case WM_GETOBJECT:
        if (self->accessible_)
          if (const auto answer = self->accessible_->answer(wparam, lparam))
            return *answer;
        break;
      case accessible_invoke_message:
        self->invoke_accessible(static_cast<int>(wparam), lparam);
        return 0;
      case WM_DESTROY:
        if (self->accessible_)
          self->accessible_->disconnect();
        break;
      case WM_POWERBROADCAST:
        if (wparam != PBT_APMRESUMEAUTOMATIC &&
            wparam != PBT_APMRESUMECRITICAL && wparam != PBT_APMRESUMESUSPEND)
          break;
        [[fallthrough]];
      case WM_DISPLAYCHANGE:
      case WM_DWMCOMPOSITIONCHANGED:
      case WM_DPICHANGED:
        // Composition surfaces keep both the display device and the DPI they
        // were created with. A topology change, DWM restart or system resume
        // can invalidate that device, while a high-to-low DPI move can leave
        // the old surface large enough to bypass EnsureForComposition's size
        // rebuild. Recreate it before the next frame in every case.
        self->device_.DiscardTarget();
        // 反应式锁存的触发之一：游戏刚弹出候选窗就改了显示模式。
        if (message == WM_DISPLAYCHANGE)
          self->display_changed_ = true;
        [[fallthrough]];
      case WM_SETTINGCHANGE:
        // The next refresh recomputes from the authenticated caret anchor;
        // do not recursively reposition from inside SetWindowPos's callback.
        self->shown_.reset();
        self->painted_.reset();
        self->pressed_.reset();
        self->pressed_page_.reset();
        self->hovered_.reset();
        return message == WM_POWERBROADCAST ? TRUE : 0;
      case WM_PAINT:
        self->paint();
        return 0;
      case typing_effect_message:
        self->take_typing_effect();
        return 0;
      case WM_TIMER:
        if (wparam != typing_flash_timer && wparam != typing_combo_timer)
          break;
        self->typing_effect_tick(static_cast<UINT_PTR>(wparam));
        return 0;
      case WM_CLOSE:
        self->hide();
        return 0;
      case WM_NCDESTROY:
        self->window_ = nullptr;
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        break;
      }
    } catch (...) {
      // No exception/input text may cross the Win32 callback.
      self->fail(failure_in_message(message, static_cast<uint32_t>(GetLastError())));
      return 0;
    }
  }
  return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace msime::windows
