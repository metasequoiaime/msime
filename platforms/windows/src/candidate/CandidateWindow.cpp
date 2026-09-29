#include "CandidateWindow.h"
#include "CandidateFlyoutWindow.h"
#include "CandidateFontFormat.h"
#include "CandidateWheel.h"
#include "CursorResource.h"
#include "NativeFontAlias.h"
#include "WindowShadow.h"
#include <algorithm>
#include <iterator>

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
constexpr wchar_t class_name[] = L"MSIME.Client.Preview.Candidates";
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
  if (window_)
    DestroyWindow(window_);
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
      wubi_code_hint_ == settings.wubi_code_hint)
    return;
  horizontal_ = settings.horizontal;
  show_preedit_ = settings.show_preedit;
  // The hint is applied by the reader; remembering it here is what repaints an unchanged generation when it is toggled.
  wubi_code_hint_ = settings.wubi_code_hint;
  invalidate_geometry();
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
    failed_ = true;
    hide();
  }
}
void CandidateWindow::reposition() {
  auto value = reader_();
  if (!value || !value->visible) {
    hide();
    return;
  }
  // The first show can arrive before TSF has produced a usable text extent.
  // Do not let card_bounds clamp the sentinel into the monitor work area; a
  // subsequent MoveCandidateWnd with a real anchor will retry this refresh.
  if (value->y == invalid_candidate_anchor_y) {
    hide();
    return;
  }
  if (value->candidates.size() > 9)
    throw std::invalid_argument("Oversized window page");
  // 候选窗口跟随光标. With following off the card stays where it first
  // appeared: the caret still moves as the user types, but the anchor this
  // layout uses does not. Everything downstream - the monitor it lands on, the
  // flip decision, the cached-frame comparison below - reads the anchored
  // copy, so a caret move alone no longer even wakes the window.
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
  if (shown_ && shown_dpi_ == GetDpiForWindow(window_) &&
      shown_->session == value->session &&
      shown_->generation == value->generation && shown_->x == value->x &&
      shown_->y == value->y && shown_->lease.epoch == value->lease.epoch &&
      shown_->lease.token == value->lease.token &&
      same_ticket(shown_->lease.transport, value->lease.transport))
    return; // Do not create a self-sustaining WM_PAINT polling loop.
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  if (!GetMonitorInfoW(
          MonitorFromPoint({value->x, value->y}, MONITOR_DEFAULTTONEAREST),
          &monitor))
    throw std::runtime_error("Candidate monitor unavailable");
  const auto &work = monitor.rcWork;
  // Move a hidden one-pixel window onto the target monitor first. Its own DPI,
  // rather than process-global or previous-monitor DPI, determines the layout.
  if (MonitorFromWindow(window_, MONITOR_DEFAULTTONEAREST) !=
      MonitorFromPoint({value->x, value->y}, MONITOR_DEFAULTTONEAREST)) {
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
  const double scale = static_cast<double>(dpi) / 96.0;
  CandidateCardInput input;
  input.horizontal = horizontal_;
  input.preedit_visible = show_preedit_;
  input.font_size = font_size_;
  input.preedit_font_size = preedit_font_size_;
  input.max_width = static_cast<double>(available_width) / scale / 2.0;
  input.max_height = static_cast<double>(available_height) / scale / 2.0;
  input.skin_min_width = skin_min_width_;
  if (show_preedit_) {
    input.preedit_width = measured_width(
        device_, wide(value.preedit), font_family_,
        static_cast<float>(preedit_font_size_), font_fallback_.Get(),
        DWRITE_FONT_WEIGHT_SEMI_BOLD);
    input.page_width = measured_width(
        device_, pager_label(value), font_family_,
        static_cast<float>(CandidateCardMetrics{}.pager_font),
        font_fallback_.Get());
  }
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
// The shipped presenter draws three runs per candidate: the text with its badge, the annotation (辅助码) and the translation, the last at a smaller size. Measuring them apart is what lets a long annotation or translation wrap under the text instead of being clipped off the end of one long label.
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
    items.push_back({width(candidate.text + candidate.badge, font_size_),
                     width(candidate.annotation, font_size_),
                     width(candidate.translation, metrics.translation_font)});
  }
  return items;
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
    runs.push_back({wide(candidate.text + candidate.badge),
                    wide(candidate.annotation), wide(candidate.translation)});
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
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Candidate device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Candidate render target unavailable");
  // DrawText goes through the windows.h macro so the call matches whichever
  // name the Direct2D declaration picked up for this target.
  const auto metrics = candidate_card_metrics(font_size_, preedit_font_size_,
                                              show_preedit_);
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
  const D2D1_RECT_F card_rect{
      static_cast<float>(frame.card_left) + inset,
      static_cast<float>(frame.card_top) + inset,
      static_cast<float>(frame.card_left + frame.card_width) - inset,
      static_cast<float>(frame.card_top + frame.card_height) - inset};
  // The Fluent flyout shadow, 0 8px 16px: a 16px CSS blur is a Gaussian of sigma 8.
  const WindowShadowPass shadow_passes[] = {
      {8.0f, palette_.shadow_alpha, 0.0f, 8.0f},
  };
  const float radius = skin_radius_.value_or(palette_.radius);
  draw_window_shadow_passes(target, card_rect, radius, shadow_passes,
                            std::size(shadow_passes));
  const D2D1_ROUNDED_RECT card{card_rect, radius, radius};
  target->FillRoundedRectangle(card, brush(palette_.surface));
  // The package background sits on the surface and under the border and the text, masked by the card's rounded outline.
  if (!background_.image.empty() && background_.opacity > 0.0f) {
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
                         background_.opacity, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                         &source);
      target->PopLayer();
    }
  }
  target->DrawRoundedRectangle(card, brush(palette_.border),
                               palette_.border_width);
  // The mascot, drawn last so it sits over the card's top edge - that overlap
  // is the whole point of the decoration.
  if (!decoration_image_.empty() && decoration_offset_ > 0.0f) {
    D2D1_SIZE_F natural{};
    if (auto *bitmap = device_.GetBitmapFromFile(decoration_image_, &natural)) {
      const float drawn_width = static_cast<float>(decoration_width_);
      // Keep the image's own aspect ratio: a package gives a width, not a box,
      // so deriving the height is what stops the artwork being squashed.
      const float drawn_height =
          natural.width > 0.0f ? drawn_width * (natural.height / natural.width)
                               : decoration_offset_;
      // Placed along the card's top edge as the manifest aligns it, as the settings preview places it.
      const float left = candidate_decoration_left(
          decoration_align_, static_cast<float>(frame.card_left),
          static_cast<float>(frame.card_left + frame.card_width),
          static_cast<float>(metrics.pad_x), drawn_width);
      const float right = left + drawn_width;
      const float bottom = static_cast<float>(shadow_insets_.top) +
                           decoration_offset_ +
                           static_cast<float>(metrics.pad_y);
      const float top = (std::max)(static_cast<float>(shadow_insets_.top),
                                   bottom - drawn_height);
      target->DrawBitmap(bitmap, D2D1_RECT_F{left, top, right, bottom}, 1.0f,
                         D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    }
  }
  // The Fluent top row: the preedit in the accent colour at semibold on the left, the page indicator and the previous and next arrows in the secondary colour on the right.
  std::optional<CandidatePagerLayout> pager;
  if (show_preedit_) {
    const auto label = pager_label(*value);
    pager = candidate_pager_layout(
        frame.card_width,
        measured_width(device_, label, font_family_,
                       static_cast<float>(metrics.pager_font),
                       font_fallback_.Get()),
        metrics);
    const D2D1_RECT_F rect{
        static_cast<float>(frame.card_left + metrics.pad_x),
        static_cast<float>(frame.card_top + metrics.pad_y),
        static_cast<float>(frame.card_left +
                           (pager ? pager->left - metrics.pager_gap
                                  : frame.card_width - metrics.pad_x / 2.0)),
        static_cast<float>(frame.card_top) +
            static_cast<float>(metrics.pad_y + metrics.preedit_row)};
    const auto text = wide(value->preedit);
    // Same clamp, same reason as the candidate rows below: a long enough reading would otherwise be drawn past the card, or under the pager.
    if (rect.right > rect.left)
      target->DrawText(text.c_str(), static_cast<UINT32>(text.size()),
                        format(preedit_font_size_, DWRITE_TEXT_ALIGNMENT_LEADING,
                               false, false, DWRITE_FONT_WEIGHT_SEMI_BOLD),
                        rect, brush(palette_.accent),
                        D2D1_DRAW_TEXT_OPTIONS_CLIP);
    if (pager) {
      auto box = [&frame](const CandidateRowBounds &bounds) {
        return D2D1_RECT_F{static_cast<float>(frame.card_left + bounds.left),
                           static_cast<float>(frame.card_top + bounds.top),
                           static_cast<float>(frame.card_left + bounds.right),
                           static_cast<float>(frame.card_top + bounds.bottom)};
      };
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
  auto rows = candidate_page_layout(measure_items(*value), frame.card_width,
                                    metrics, horizontal_, wrap_measure(*value));
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
      const D2D1_ROUNDED_RECT selection{rect, palette_.item_radius,
                                        palette_.item_radius};
      target->FillRoundedRectangle(selection, brush(value->candidates[i].highlighted
                                                        ? palette_.selected
                                                        : palette_.hover));
      if (value->candidates[i].highlighted && palette_.show_selected_bar) {
        const auto extent = candidate_selection_bar(
            rect.left, rect.top, rect.bottom, metrics.candidate_row);
        const float radius =
            static_cast<float>(candidate_selection_bar_width * 0.5);
        const D2D1_ROUNDED_RECT bar{{static_cast<float>(extent.left),
                                     static_cast<float>(extent.top),
                                     static_cast<float>(extent.right),
                                     static_cast<float>(extent.bottom)},
                                    radius, radius};
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
    const auto label = std::to_wstring(i + 1);
    target->DrawText(label.c_str(), static_cast<UINT32>(label.size()),
                      format(font_size_, DWRITE_TEXT_ALIGNMENT_TRAILING),
                      D2D1_RECT_F{rect.left, rect.top, rect.left + number,
                                  rect.top + first_line},
                      brush(number_color));
    const auto text = wide(value->candidates[i].text + value->candidates[i].badge);
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
    draw_run(value->candidates[i].translation, item.translation,
             metrics.translation_font, translation_color);
  }
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
  if (rendered_)
    rendered_(*painted_);
}
std::optional<CandidateClick> CandidateWindow::hit(int x, int y) {
  if (!click_ || !painted_ || !IsWindowVisible(window_))
    return std::nullopt;
  RECT bounds{};
  if (!GetClientRect(window_, &bounds))
    return std::nullopt;
  const double scale = painted_dpi_ ? painted_dpi_ / 96.0 : 1.0;
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
  if (!page_ || !painted_ || !painted_pager_ || !IsWindowVisible(window_))
    return std::nullopt;
  const double scale = painted_dpi_ ? painted_dpi_ / 96.0 : 1.0;
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
        if (!self->page_ || !self->painted_ || !IsWindowVisible(window)) {
          self->wheel_accumulator_ = 0;
          return 0;
        }
        const auto steps = consume_candidate_wheel_delta(
            self->wheel_accumulator_, GET_WHEEL_DELTA_WPARAM(wparam),
            WHEEL_DELTA);
        const auto &value = *self->painted_;
        if (steps.page_up > 0)
          self->page_(CandidatePage{value.lease, value.session, value.generation,
                                    true, static_cast<unsigned>(steps.page_up)});
        if (steps.page_down > 0)
          self->page_(CandidatePage{value.lease, value.session, value.generation,
                                    false, static_cast<unsigned>(steps.page_down)});
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
      case WM_ERASEBKGND:
        return 1;
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
      case WM_CLOSE:
        self->hide();
        return 0;
      case WM_NCDESTROY:
        self->window_ = nullptr;
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        break;
      }
    } catch (...) {
      self->failed_ = true;
      self->hide(); // No exception/input text may cross the Win32 callback.
      return 0;
    }
  }
  return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace msime::windows
