#include "CandidateWindow.h"
#include "CandidateCanvasDrawing.h"
#include "CandidateDecorationDrawing.h"
#include "AccessibleWindow.h"
#include "CandidateAccessibility.h"
#include "CandidateFlyoutWindow.h"
#include "CandidateFontFormat.h"
#include "CandidateMotion.h"
#include "CandidateWheel.h"
#include "CursorResource.h"
#include "GameCandidateAnchor.h"
#include "WaveOverlayUtils.h"
#include "NativeFontAlias.h"
#include "ServerResources.h"
#include "TypingEffectSignal.h"
#include "WindowShadow.h"
#include <algorithm>
#include <cmath>
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
constexpr UINT_PTR skin_animation_timer = 0x4503;
constexpr UINT_PTR image_frame_timer = 0x4504;
constexpr UINT typing_flash_frame_millis = 30;
// 还没有会话发布特效设置时闪光的强度。Windows 固定打字效果强度为 50（client-core 的 `HostPlatform::fixed_effect_intensity`），不读 plugins.effect_intensity；特效包自带的强度随每个按键从输入线程发布（TypingEffectSignal），不受影响。
constexpr uint32_t typing_effect_default_intensity = 50;
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
// 一行文字画出来有多高（DIP，向上取整），和 macOS 的 MSIMECandidateTextHeight 一样：回退字体的字形可能比主字体高，所以按要画的文字量；文字为空时量的是主字体自己的一行。量不了时按 1.35 倍字号估算。
double measured_line_height(msimeui::DeviceResources &device, const std::wstring &text,
                            const std::wstring &family, float size,
                            IDWriteFontFallback *fallback) {
  auto *factory = device.GetDWriteFactory();
  auto *format = device.GetTextFormat(
      family, size, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING,
      DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP);
  set_candidate_font_fallback(format, fallback);
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  DWRITE_TEXT_METRICS metrics{};
  if (size > 0.0f && factory && format &&
      SUCCEEDED(factory->CreateTextLayout(text.c_str(),
                                          static_cast<UINT32>(text.size()),
                                          format, 8192.0f, size * 4.0f,
                                          layout.GetAddressOf())) &&
      layout && SUCCEEDED(layout->GetMetrics(&metrics)) && metrics.height > 0.0f)
    return std::ceil(static_cast<double>(metrics.height));
  return static_cast<double>(size) * 1.35;
}
// The page indicator in the preedit row, "2 / 5", or empty when the card draws no pager: without candidates or a page count from the Engine there is nothing to page. `shown` 是共享偏好 `show_candidate_page_number`：关掉时页码和两个箭头都不画，首行也不为它们留高度，键盘和滚轮照常翻页。
std::wstring pager_label(const CandidatePresentation &value, bool shown) {
  if (!shown || value.candidates.empty() || value.page_count == 0 ||
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
// 四个角各用自己圆角的矩形，依次是左上、右上、右下、左下，皮肤 `item_corners_follow_card` 的高亮用它。每个半径不超过短边的一半；建不出几何时为空，调用方退回普通圆角矩形。
Microsoft::WRL::ComPtr<ID2D1PathGeometry>
corner_rect_geometry(ID2D1RenderTarget *target, const D2D1_RECT_F &rect,
                     const std::array<double, 4> &radii) {
  const float width = rect.right - rect.left, height = rect.bottom - rect.top;
  Microsoft::WRL::ComPtr<ID2D1Factory> factory;
  target->GetFactory(&factory);
  Microsoft::WRL::ComPtr<ID2D1PathGeometry> geometry;
  Microsoft::WRL::ComPtr<ID2D1GeometrySink> sink;
  if (!(width > 0.0f) || !(height > 0.0f) || !factory ||
      FAILED(factory->CreatePathGeometry(geometry.GetAddressOf())) ||
      FAILED(geometry->Open(sink.GetAddressOf())))
    return nullptr;
  const float limit = (std::min)(width, height) * 0.5f;
  auto radius = [limit](double value) {
    return (std::max)(0.0f, (std::min)(static_cast<float>(value), limit));
  };
  const float top_left = radius(radii[0]), top_right = radius(radii[1]),
              bottom_right = radius(radii[2]), bottom_left = radius(radii[3]);
  auto arc = [&sink](float x, float y, float r) {
    if (r > 0.0f)
      sink->AddArc(D2D1_ARC_SEGMENT{D2D1_POINT_2F{x, y}, D2D1_SIZE_F{r, r}, 0.0f,
                                    D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_ARC_SIZE_SMALL});
  };
  sink->BeginFigure(D2D1_POINT_2F{rect.left + top_left, rect.top}, D2D1_FIGURE_BEGIN_FILLED);
  sink->AddLine(D2D1_POINT_2F{rect.right - top_right, rect.top});
  arc(rect.right, rect.top + top_right, top_right);
  sink->AddLine(D2D1_POINT_2F{rect.right, rect.bottom - bottom_right});
  arc(rect.right - bottom_right, rect.bottom, bottom_right);
  sink->AddLine(D2D1_POINT_2F{rect.left + bottom_left, rect.bottom});
  arc(rect.left, rect.bottom - bottom_left, bottom_left);
  sink->AddLine(D2D1_POINT_2F{rect.left, rect.top + top_left});
  arc(rect.left + top_left, rect.top, top_left);
  sink->EndFigure(D2D1_FIGURE_END_CLOSED);
  if (FAILED(sink->Close()))
    return nullptr;
  return geometry;
}
// 列表翻页箭头的实心三角，尖朝前一页（左）或后一页（右），在 box 里居中，高 glyph_size、宽为高的 0.84 倍；三个角沿两条边各切掉 roundness 再用曲线连起来，整个是一块填充，半透明（不可用）时也看不到叠出来的深色。照 msime-windows PagerArrows.cpp 的 RenderTriangle。
Microsoft::WRL::ComPtr<ID2D1PathGeometry>
rounded_triangle_geometry(ID2D1RenderTarget *target, const D2D1_RECT_F &box,
                          bool previous, float glyph_size, float roundness) {
  Microsoft::WRL::ComPtr<ID2D1Factory> factory;
  target->GetFactory(&factory);
  Microsoft::WRL::ComPtr<ID2D1PathGeometry> geometry;
  Microsoft::WRL::ComPtr<ID2D1GeometrySink> sink;
  if (!factory || FAILED(factory->CreatePathGeometry(geometry.GetAddressOf())) ||
      FAILED(geometry->Open(sink.GetAddressOf())))
    return nullptr;
  const float height = (std::min)(glyph_size, box.bottom - box.top);
  const float width = height * 0.84f;
  const float cx = (box.left + box.right) * 0.5f, cy = (box.top + box.bottom) * 0.5f;
  const float tip_x = previous ? cx - width * 0.5f : cx + width * 0.5f;
  const float base_x = previous ? cx + width * 0.5f : cx - width * 0.5f;
  const D2D1_POINT_2F corners[3] = {D2D1_POINT_2F{tip_x, cy},
                                    D2D1_POINT_2F{base_x, cy - height * 0.5f},
                                    D2D1_POINT_2F{base_x, cy + height * 0.5f}};
  // 不超过最短边的一半，相邻两处切口不会交叉。
  const float radius = (std::max)(0.0f, (std::min)(roundness, height * 0.5f));
  auto toward = [radius](D2D1_POINT_2F from, D2D1_POINT_2F to) {
    const float dx = to.x - from.x, dy = to.y - from.y;
    const float length = std::sqrt(dx * dx + dy * dy);
    const float t = length > 0.0f ? radius / length : 0.0f;
    return D2D1_POINT_2F{from.x + dx * t, from.y + dy * t};
  };
  sink->BeginFigure(toward(corners[0], corners[1]), D2D1_FIGURE_BEGIN_FILLED);
  for (int i = 1; i <= 3; ++i) {
    const D2D1_POINT_2F corner = corners[i % 3];
    sink->AddLine(toward(corner, corners[i - 1]));
    sink->AddQuadraticBezier(
        D2D1_QUADRATIC_BEZIER_SEGMENT{corner, toward(corner, corners[(i + 1) % 3])});
  }
  sink->EndFigure(D2D1_FIGURE_END_CLOSED);
  if (FAILED(sink->Close()))
    return nullptr;
  return geometry;
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
// 系统「透明效果」（设置 → 个性化 → 颜色）。关着时系统材质一律退回纯色，要材质的皮肤也退回不透明底色；读不到时按开着处理，同 msime-windows。
bool system_transparency_enabled() {
  DWORD value = 1;
  DWORD size = sizeof(value);
  if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                   L"EnableTransparency", RRF_RT_REG_DWORD, nullptr, &value, &size) != ERROR_SUCCESS)
    return true;
  return value != 0;
}
bool system_high_contrast() {
  HIGHCONTRASTW contrast{};
  contrast.cbSize = sizeof(contrast);
  return SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast), &contrast, 0) &&
         (contrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
}
msimeui::BackdropMaterial backdrop_material(const CandidateBackdropMaterial &material) {
  msimeui::BackdropMaterial converted;
  converted.source = material.source == CandidateBackdropSource::wallpaper
                         ? msimeui::BackdropMaterial::Source::Wallpaper
                         : msimeui::BackdropMaterial::Source::WindowsBehind;
  converted.tint = D2D1::ColorF(material.tint_r, material.tint_g, material.tint_b, 1.0f);
  converted.tintOpacity = material.tint_opacity;
  converted.luminosityOpacity = material.luminosity_opacity;
  return converted;
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
      font_family_(wide(font_family)), configured_font_family_(wide(font_family)),
      dark_theme_(dark_theme), horizontal_(horizontal),
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
  configured_font_family_ = font_family_;
  for (auto &family : fallback_families_)
    family = native_font_alias(family);
  configured_fallback_families_ = fallback_families_;
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
  KillTimer(window_, image_frame_timer);
  image_frames_.stop();
  finish_skin_animation();
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
  // 选中竖条开关会挪动缺省版式的序号（见 skin_metrics），这时要重排，不只是重画。
  const bool relayout = palette.show_selected_bar != palette_.show_selected_bar;
  palette_ = std::move(palette);
  if (relayout) {
    invalidate_geometry();
    return;
  }
  painted_.reset();
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}

bool CandidateWindow::skin_motion_enabled() const {
  if (!animation_active_ || active_animation_ == CandidateSkinAnimationPreset::none ||
      animation_.duration_ms == 0)
    return false;
  BOOL enabled = TRUE;
  if (!SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &enabled, 0))
    enabled = TRUE;
  return enabled != FALSE;
}

float CandidateWindow::skin_motion_progress() const {
  if (!skin_motion_enabled())
    return 1.0f;
  const auto elapsed = GetTickCount64() - animation_started_;
  return static_cast<float>(std::min<uint64_t>(elapsed, animation_.duration_ms)) /
         static_cast<float>(animation_.duration_ms);
}

void CandidateWindow::begin_skin_animation(CandidateSkinAnimationPreset preset, size_t rows) {
  // 新的显示或选中变化会打断旧的退出帧，退出快照不能覆盖新会话。
  finishing_animation_ = false;
  selection_animation_ = false;
  exit_value_.reset();
  if (preset == CandidateSkinAnimationPreset::none || animation_.duration_ms == 0) {
    finish_skin_animation();
    return;
  }
  active_animation_ = preset;
  // 逐行浮现的进场要等最后一行也播完才算结束。
  animation_total_ms_ =
      candidate_enter_total_ms(preset, animation_.duration_ms, animation_.stagger_ms, rows);
  animation_started_ = GetTickCount64();
  animation_active_ = true;
  SetTimer(window_, skin_animation_timer, 16, nullptr);
}

void CandidateWindow::finish_skin_animation() {
  animation_active_ = false;
  selection_animation_ = false;
  active_animation_ = CandidateSkinAnimationPreset::none;
  if (window_)
    KillTimer(window_, skin_animation_timer);
}

void CandidateWindow::skin_animation_tick() {
  if (!animation_active_) {
    KillTimer(window_, skin_animation_timer);
    return;
  }
  if (!skin_motion_enabled() ||
      GetTickCount64() - animation_started_ >= animation_total_ms_) {
    const bool finishing = finishing_animation_;
    finishing_animation_ = false;
    finish_skin_animation();
    if (finishing)
      hide_now();
  }
  if (IsWindowVisible(window_))
    InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::set_theme_palette(CandidatePalette palette) {
  set_palette(std::move(palette));
  invalidate_geometry();
}
void CandidateWindow::invalidate_skin_images() {
  KillTimer(window_, image_frame_timer);
  image_frames_.clear();
  image_epoch_ = 0;
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
    configured_font_family_ = std::move(primary);
    configured_fallback_families_ = std::move(fallback);
    // An active skin font stays first; only the configured faces behind it change.
    apply_font_chain();
    font_settings_ = std::move(remembered);
    font_size_ = settings.size;
    preedit_font_size_ = settings.preedit_size;
    invalidate_geometry();
    return true;
  } catch (...) {
    return false;
  }
}
void CandidateWindow::apply_font_chain() {
  auto chain = candidate_skin_font_chain(configured_font_family_,
                                         configured_fallback_families_,
                                         skin_font_family_);
  font_family_ = std::move(chain.primary);
  fallback_families_ = std::move(chain.fallback);
  font_fallback_.Reset();
}
void CandidateWindow::set_skin_font_family(const std::string &family) {
  // A face that is not installed is no skin font at all, so the user's own chain draws rather than DirectWrite's substitute.
  std::wstring next = family.empty() ? std::wstring{} : wide(family);
  if (next.size() > 128 || (!next.empty() && !installed_font(next)))
    next.clear();
  if (!next.empty())
    next = native_font_alias(next);
  if (skin_font_family_ == next)
    return;
  skin_font_family_ = std::move(next);
  apply_font_chain();
  invalidate_geometry();
}
void CandidateWindow::set_skin_layout(const CandidateSkinLayout &layout) {
  // 换了一个不再（或刚开始）卷轴展开的皮肤：展开区随之拆掉，界面和 Server 都回到收起（msime-windows 的场景重建同样收起）。
  if (layout.expand_panel.value_or(false) != skin_layout_.expand_panel.value_or(false))
    collapse_expansion();
  skin_layout_ = layout;
  invalidate_geometry();
}
void CandidateWindow::set_skin_canvas(CandidateSkinCanvas canvas) {
  canvas_layers_ = candidate_canvas_layers(canvas.decorations);
  canvas_ = std::move(canvas);
  invalidate_geometry();
}
void CandidateWindow::set_layout(CandidateLayoutSettings settings) {
  if (horizontal_ == settings.horizontal && show_preedit_ == settings.show_preedit &&
      wubi_code_hint_ == settings.wubi_code_hint &&
      show_app_logo_ == settings.show_app_logo &&
      reserved_gloss_lines_ == settings.reserved_gloss_lines &&
      show_page_number_ == settings.show_page_number)
    return;
  horizontal_ = settings.horizontal;
  show_preedit_ = settings.show_preedit;
  show_app_logo_ = settings.show_app_logo;
  show_page_number_ = settings.show_page_number;
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
  hovered_page_.reset();
  wheel_accumulator_ = 0;
  tallest_ = 0;
  // 展开区按新的字体和版式重排；开着的展开保持开着。
  painted_expand_.reset();
  painted_grid_ = {};
  hovered_cell_.reset();
  pressed_cell_.reset();
  hovered_toggle_ = pressed_toggle_ = hovered_scrollbar_ = false;
  thumb_grab_.reset();
  grid_width_cache_.clear();
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::hide_now() {
  KillTimer(window_, image_frame_timer);
  image_frames_.stop();
  image_epoch_ = 0;
  finish_skin_animation();
  exit_value_.reset();
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
  hovered_page_.reset();
  wheel_accumulator_ = 0;
  // 候选窗关闭（上屏或取消输入）时收起卷轴展开。
  collapse_expansion();
  painted_expand_.reset();
  painted_grid_ = {};
  hovered_cell_.reset();
  pressed_cell_.reset();
  hovered_toggle_ = pressed_toggle_ = hovered_scrollbar_ = false;
  thumb_grab_.reset();
  // 提示是候选窗拥有的弹出窗口，候选窗隐藏时它不会跟着消失，正开着的提示要收起来。
  if (tooltip_)
    SendMessageW(tooltip_, TTM_POP, 0, 0);
  // 隐藏的候选窗没有可读的东西。
  if (accessible_)
    accessible_->publish({AccessibleContainer::List, candidate_accessible_name, {}});
  ShowWindow(window_, SW_HIDE);
}

void CandidateWindow::hide_immediately() {
  // 正在退场时，收起的时刻已经在 hide() 开始退场时记下了。
  if (!finishing_animation_ && window_ && IsWindowVisible(window_))
    last_hidden_at_ = GetTickCount64();
  finishing_animation_ = false;
  hide_now();
}

bool CandidateWindow::exit_lease_lost() const {
  return finishing_animation_ && exit_value_ && lease_current_ &&
         !lease_current_(exit_value_->lease);
}

std::optional<CandidatePresentation> CandidateWindow::read_presentation() const {
  auto value = reader_();
  // 非阻塞读取争用不代表组字结束。只沿用仍属当前焦点的可见帧，真实快照（包括明确隐藏）始终优先。
  if (!value && !finishing_animation_ && shown_ && shown_->visible &&
      lease_current_ && lease_current_(shown_->lease))
    return shown_;
  return value;
}

void CandidateWindow::hide_for(const std::optional<CandidatePresentation> &value) {
  // 当前焦点里的可见旧帧已经由 read_presentation 保留。剩下的读空只能继续租约已确认的正常退场，不能据此新开退场或保留未知归属的候选。
  if (!value) {
    if (finishing_animation_ && exit_value_ && lease_current_ &&
        lease_current_(exit_value_->lease))
      return;
    hide_immediately();
    return;
  }
  if (value->composition_ended)
    hide();
  else
    hide_immediately();
}

void CandidateWindow::hide() {
  if (finishing_animation_)
    return;
  // 连续输入的判断（candidate_enter_allowed）从卡片开始收起时算起，和 macOS 的 _lastHide 一样，不等退场播完。
  if (window_ && IsWindowVisible(window_))
    last_hidden_at_ = GetTickCount64();
  BOOL animations = TRUE;
  if (!SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animations, 0))
    animations = TRUE;
  if (IsWindowVisible(window_) && animations && animation_.duration_ms > 0 &&
      animation_.exit != CandidateSkinAnimationPreset::none && shown_ &&
      shown_->visible) {
    exit_value_ = shown_;
    finishing_animation_ = true;
    selection_animation_ = false;
    active_animation_ = animation_.exit;
    animation_total_ms_ = animation_.duration_ms;
    animation_started_ = GetTickCount64();
    animation_active_ = true;
    SetTimer(window_, skin_animation_timer, 16, nullptr);
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  hide_now();
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
  // 会话、焦点和全屏策略变化涉及隐私与输入目标，必须立即清掉窗口，不能把旧文字留在退出帧里。
  hide_immediately();
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
  auto value = read_presentation();
  if (!value || !value->visible) {
    hide_for(value);
    return;
  }
  // Server 那边已经收起（候选窗关闭过）、焦点换到了别的客户端、改成了竖排，或这一页不能展开（释义页、只能用键盘选的列表）时收起；继续输入时保持展开。
  if (candidate_expand_stale(
          expand_.expanded, value->expansion_open,
          expand_lease_ && expand_lease_->epoch == value->lease.epoch &&
              expand_lease_->token == value->lease.token &&
              same_ticket(expand_lease_->transport, value->lease.transport),
          skin_metrics(*value).expand_panel)) {
    collapse_expansion();
    shown_.reset();
  }
  if (finishing_animation_) {
    finishing_animation_ = false;
    exit_value_.reset();
    finish_skin_animation();
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
  // render_serial 也要比：读音在同一个 Engine generation 上单独到达（CandidateMailbox::readings），只换了快照编号。
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
    // 换显示器是马上要在别处重新出现，不播退场：退场的计时会在新位置把刚出现的卡片收掉。
    hide_immediately();
    if (!SetWindowPos(window_, nullptr, work.left, work.top, 1, 1,
                      SWP_NOACTIVATE | SWP_NOZORDER))
      throw std::runtime_error("Candidate monitor move failed");
  }
  const auto dpi = GetDpiForWindow(window_);
  const auto bounds = card_bounds(*value, work, dpi);
  const bool was_visible = IsWindowVisible(window_) != FALSE;
  const auto previous_highlight = [&]() -> std::optional<size_t> {
    if (!shown_) return std::nullopt;
    for (size_t index = 0; index < shown_->candidates.size(); ++index)
      if (shown_->candidates[index].highlighted) return index;
    return std::nullopt;
  }();
  const auto next_highlight = [&]() -> std::optional<size_t> {
    for (size_t index = 0; index < value->candidates.size(); ++index)
      if (value->candidates[index].highlighted) return index;
    return std::nullopt;
  }();
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
  if (!was_visible) {
    // 刚收起不到 400ms 又出现，算同一段连续输入（上屏后接着打下一个词），不播进场，和 macOS 的 kTypingBurstInterval 一样。
    begin_skin_animation(candidate_enter_allowed(GetTickCount64(), last_hidden_at_)
                             ? animation_.enter
                             : CandidateSkinAnimationPreset::none,
                         value->candidates.size());
  } else if (animation_active_ && !selection_animation_ && !finishing_animation_) {
    // 进场还在播时高亮移动不重播（见下一支），但新的一页比第一帧多了行：最后一行晚到的那段也要播完，结束时刻只往后推、不提前，和 macOS showPanel 给新行补 addRowReveals 再抬高 _enterEnd 一样。
    animation_total_ms_ = (std::max)(
        animation_total_ms_,
        candidate_enter_total_ms(active_animation_, animation_.duration_ms,
                                 animation_.stagger_ms, value->candidates.size()));
  } else if (previous_highlight != next_highlight &&
             (!animation_active_ || selection_animation_)) {
    // A highlight move while the card is still entering lets the entrance finish rather than restarting the card.
    begin_skin_animation(animation_.selection);
    selection_animation_ = animation_active_;
  }
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
  input.skin = skin_metrics(value);
  input.number_width = number_label_width(value.candidates.size(), input.skin);
  input.line_height = candidate_line_height(value);
  if (horizontal_)
    input.reserved_secondary_height =
        reserved_secondary_height(input.reserved_secondary_lines);
  // 拼音里画插入点时，光标两边的字各让开一点（candidate_preedit_caret），宽度先留出来。
  if (show_preedit_)
    input.preedit_width =
        measured_width(device_, wide(value.preedit), font_family_,
                       static_cast<float>(preedit_font_size_), font_fallback_.Get(),
                       DWRITE_FONT_WEIGHT_SEMI_BOLD) +
        (value.preedit_caret != std::string::npos ? candidate_preedit_caret_slot : 0.0);
  // 翻页和拼音共用首行；只要有 logo 或翻页，拼音隐藏时首行也照样画。卷轴展开的皮肤不画翻页，`show_candidate_page_number` 关掉时也不画。
  input.page_width = input.skin.expand_panel
                         ? 0.0
                         : measured_width(device_, pager_label(value, show_page_number_),
                                          font_family_,
                                          static_cast<float>(CandidateCardMetrics{}.pager_font),
                                          font_fallback_.Get());
  input.items = measure_items(value);
  input.wrapped = wrap_measure(value);
  // canvas：内容内边距让面板比这张卡片四边各大出 canvas_offset_，最小宽度按整块面板比。没有 canvas 时为 0。
  canvas_offset_ =
      canvas_.active
          ? candidate_canvas_content_offset(
                canvas_, candidate_canvas_base_padding(candidate_card_metrics(
                             font_size_, preedit_font_size_, show_preedit_, show_app_logo_,
                             input.page_width > 0.0, input.skin, input.number_width,
                             input.line_height)))
          : CandidateCanvasEdges{};
  input.canvas_extra_width = canvas_offset_.left + canvas_offset_.right;
  // 卷轴展开：展开着时卡片按网格变宽变高；后续候选还没到时沿用上一帧的高度。
  const auto grid = input.skin.expand_panel ? expand_grid(value) : ExpandGrid{};
  const auto card =
      input.skin.expand_panel
          ? candidate_expand_card_size(input, grid_widths(grid), expand_page_size(value),
                                       candidate_expand_held_viewport(
                                           expand_.expanded, value.expanded != nullptr,
                                           expand_viewport_))
          : candidate_card_size(input);
  // 窗口按面板（没有 canvas 时就是卡片）定尺寸、贴着光标摆放；面板四周是阴影的透明边距，canvas 时再加外留白或挂件伸出去的距离（candidate_canvas_margins）。
  const double panel_width = card.width + canvas_offset_.left + canvas_offset_.right;
  const double panel_height = card.height + canvas_offset_.top + canvas_offset_.bottom;
  const CandidateShadowInsets shadow_base{};
  const auto margins = candidate_canvas_margins(
      canvas_, {shadow_base.top, shadow_base.right, shadow_base.bottom, shadow_base.left},
      panel_width, panel_height, static_cast<double>(available_width) / scale,
      static_cast<double>(available_height) / scale -
          (decoration_image_.empty() ? 0.0 : decoration_top_));
  shadow_insets_ = {margins.left + canvas_offset_.left, margins.top + canvas_offset_.top,
                    margins.right + canvas_offset_.right,
                    margins.bottom + canvas_offset_.bottom};
  const auto shadow_left = static_cast<int64_t>(std::lround(margins.left * scale));
  const auto shadow_top = static_cast<int64_t>(std::lround(margins.top * scale));
  const auto shadow_right = static_cast<int64_t>(std::lround(margins.right * scale));
  const auto shadow_bottom = static_cast<int64_t>(std::lround(margins.bottom * scale));
  // 卡片的像素尺寸向上取整，**不要**四舍五入。窗口定下来之后，绘制是拿客户区像素除回 DIP 当作卡片宽度再排一次版（`frame.card_width`），四舍五入向下的那半个像素会让绘制时的可用宽度比量出来的窄一点点；而「这一页能不能排成一行」是个硬判断（`candidate_single_line_columns` 比较 `natural_total <= line_width`），差半个像素就会翻面：量尺寸时认定一行放得下、于是只给了一行的高度，绘制时却判定放不下而折行，第二行的候选就画在卡片外面了（打半个拼音时候选最宽，正好卡在这个边界上）。向上取整让绘制拿到的宽度永远不小于量出来的，两次排版的结论因此一致。
  const auto content_width =
      (std::min)(static_cast<int64_t>(std::ceil(panel_width * scale)),
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
  // 同上，高度也向上取整：量高度用的是量尺寸那次的折行结论，绘制少半个像素就可能多折一行。
  const auto content_height =
      (std::min)(static_cast<int64_t>(std::ceil(panel_height * scale)) + decoration,
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
// 和原来的候选窗一样，每个候选画三段：带徽标的候选文字、注释（辅助码）和释义，释义的字号小一些。三段分开量，长注释、长释义才能折到候选文字下面，而不是接在一整条标签的末尾被截掉。释义那一段是 candidate_secondary_text，韩文汉字的 훈음 在释义上面，脚本插件的注释单独占最后一行；注释那一段是 candidate_annotation_run，包在括号里的引擎注释，量和画用的是同一串文字。
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
                     width(candidate_annotation_run(candidate), font_size_),
                     width(candidate_secondary_text(candidate),
                           metrics.translation_font),
                     candidate_secondary_lines(candidate)});
  }
  return items;
}
double CandidateWindow::number_label_width(size_t count,
                                           const CandidateSkinMetrics &skin) {
  const auto size = static_cast<float>(
      candidate_card_metrics(font_size_, preedit_font_size_, show_preedit_, show_app_logo_,
                             false, skin)
          .number_font);
  double widest = 0.0;
  for (size_t number = 1; number <= (std::min)(count, size_t{9}); ++number)
    widest = (std::max)(widest, measured_width(device_, std::to_wstring(number), font_family_,
                                               size, font_fallback_.Get()));
  return widest;
}
double CandidateWindow::candidate_line_height(const CandidatePresentation &value) {
  const auto size = static_cast<float>(font_size_);
  double tallest = measured_line_height(device_, std::wstring{}, font_family_, size,
                                        font_fallback_.Get());
  // 候选文字和注释分开量：两段各自不超过 wide() 的上限，连起来可能超过。
  for (const auto &candidate : value.candidates)
    for (const auto &run : {candidate_primary_text(candidate), candidate_annotation_run(candidate)})
      if (!run.empty())
        tallest = (std::max)(tallest, measured_line_height(device_, wide(run), font_family_, size,
                                                           font_fallback_.Get()));
  return tallest;
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
                    wide(candidate_annotation_run(candidate)),
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
  // 焦点会话随按键发布特效包；还没有会话发布设置时，使用固定默认强度和宿主自带闪光。
  if (const auto settings = unpack_typing_effect_settings(TypingEffectSignal::instance().settings()))
    effect_settings_ = *settings;
  else
    effect_settings_ = resolve_typing_effect_settings(typing_effect_default_intensity, std::nullopt, std::nullopt);
  // 特效的强调色是系统强调色（设置 › 个性化 › 颜色），与 macOS 用 controlAccentColor 一样，不随主题；取不到时退回主题的强调色。
  if (const auto accent = system_accent_.rgb())
    effect_accent_ = candidate_rgb(*accent);
  else
    effect_accent_ = palette_.accent;
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
    presentation.accent = (channel(effect_accent_.r) << 16) | (channel(effect_accent_.g) << 8) | channel(effect_accent_.b);
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
  // 和 card_bounds 同样的换算：窗口四周是透明的阴影边距（canvas 时还有外留白，卡片是整块面板），顶上可能还有吉祥物那一条。
  const double scale = layout_scale(shown_dpi_);
  const double decoration = decoration_image_.empty() ? 0.0 : decoration_top_ * scale;
  const double left = shadow_insets_.left - canvas_offset_.left,
               top = shadow_insets_.top - canvas_offset_.top,
               right = shadow_insets_.right - canvas_offset_.right,
               bottom = shadow_insets_.bottom - canvas_offset_.bottom;
  return TypingRect{static_cast<float>(outer.left + std::lround(left * scale)),
                    static_cast<float>(outer.top + std::lround(top * scale + decoration)),
                    static_cast<float>(outer.right - std::lround(right * scale)),
                    static_cast<float>(outer.bottom - std::lround(bottom * scale))};
}
ID2D1Bitmap *CandidateWindow::skin_image(const std::wstring &path, D2D1_SIZE_F *size) {
  if (auto *bitmap = image_frames_.frame(path, size))
    return bitmap;
  return device_.GetBitmapFromFile(path, size);
}

void CandidateWindow::schedule_image_frame() {
  KillTimer(window_, image_frame_timer);
  if (!IsWindowVisible(window_) || image_suspended_ || failed_)
    return;
  if (const auto delay = image_frames_.next_delay()) {
    // Win32 定时器最低 10ms；只预约真正的帧边界，有限循环末帧没有下一次唤醒。
    const auto interval = (std::clamp)(*delay, uint64_t{USER_TIMER_MINIMUM}, uint64_t{USER_TIMER_MAXIMUM});
    SetTimer(window_, image_frame_timer, static_cast<UINT>(interval), nullptr);
  }
}

void CandidateWindow::paint() {
  DpiScope dpi_scope;
  Painting painting(window_);
  if (!painting.dc)
    throw std::runtime_error("Candidate painting unavailable");
  // 与定位共用读取规则：争用时只画当前租约的已显示帧，失焦不能沿用旧内容。
  auto value = read_presentation();
  if ((!value || (!value->visible && value->composition_ended)) &&
      finishing_animation_ && exit_value_ && lease_current_ && !exit_lease_lost())
    value = exit_value_;
  if (!value || !value->visible) {
    hide_for(value);
    return;
  }
  if (value->candidates.size() > 9)
    throw std::invalid_argument("Oversized window page");
  // The user's scale rides on the target's DPI, as the system scale does, so every DIP below - fonts, paddings, radius, shadow - grows with it and matches the size card_bounds gave the window. At 100% the window's own DPI stays the only source.
  device_.SetDpiOverride(
      style_.scale_percent == 100
          ? 0.0f
          : static_cast<float>(layout_scale(GetDpiForWindow(window_)) * 96.0));
  // 皮肤的系统材质（CandidateBackdrop.h）：要垫材质时 DeviceResources 换成 Windows.UI.Composition 合成，材质参数先交给它，建表面时就用上。建不起来就记下，这个窗口以后按材质不可用的回退画。
  auto backdrop = backdrop_plan();
  if (backdrop.active())
    device_.SetBackdropMaterial(backdrop_material(backdrop.material));
  device_.SetBackdropComposition(backdrop.active());
  if (!device_.EnsureForComposition(window_)) {
    if (!backdrop.active())
      throw std::runtime_error("Candidate device unavailable");
    backdrop_unavailable_ = true;
    backdrop = backdrop_plan();
    device_.SetBackdropComposition(false);
    if (!device_.EnsureForComposition(window_))
      throw std::runtime_error("Candidate device unavailable");
  }
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Candidate render target unavailable");
  BOOL images_enabled = TRUE;
  if (!SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &images_enabled, 0))
    images_enabled = TRUE;
  const auto image_now = GetTickCount64();
  if (image_epoch_ == 0)
    image_epoch_ = image_now;
  image_frames_.begin_frame(target, image_now - image_epoch_, images_enabled && !image_suspended_);
  // DrawText goes through the windows.h macro so the call matches whichever
  // name the Direct2D declaration picked up for this target.
  // 首行高度取决于有没有 logo 和翻页，必须和 card_bounds 量尺寸时一样算，行的位置和点击区域才对得上。
  // 皮肤的版式也和 card_bounds 一样解析，同一个序号宽度。卷轴展开的皮肤不画翻页。
  const auto skin = skin_metrics(*value);
  const auto label =
      skin.expand_panel ? std::wstring{} : pager_label(*value, show_page_number_);
  const auto metrics = candidate_card_metrics(
      font_size_, preedit_font_size_, show_preedit_, show_app_logo_, !label.empty(), skin,
      number_label_width(value->candidates.size(), skin), candidate_line_height(*value));
  // 卡片按窗口客户区排，和 card_bounds 定的窗口尺寸一致。渲染目标比窗口大（合成交换链按 64 像素分档、只增不减），按它的 GetSize() 排会把卡片右边和下边画到窗口外面裁掉。
  const auto size = device_.GetClientSize();
  const auto frame = candidate_shadow_frame(
      size.width -
          static_cast<float>(shadow_insets_.left + shadow_insets_.right),
      size.height - decoration_offset_ -
          static_cast<float>(shadow_insets_.top + shadow_insets_.bottom),
      decoration_offset_, shadow_insets_);
  const float motion_progress = skin_motion_progress();
  // 进场和选中用 ease-out，退场用 ease-in，和 macOS 的 SettleCurve 与退场曲线一样。
  const float eased = finishing_animation_ ? candidate_ease_in(motion_progress)
                                           : candidate_ease_out(motion_progress);
  // The selection preset animates the highlighted row below; only the enter and exit presets move the whole card.
  const bool selection_motion = animation_active_ && selection_animation_;
  const bool motion_entering = animation_active_ && !selection_animation_ &&
                               (active_animation_ == CandidateSkinAnimationPreset::fade ||
                                active_animation_ == CandidateSkinAnimationPreset::fade_scale ||
                                active_animation_ == CandidateSkinAnimationPreset::slide_up ||
                                active_animation_ == CandidateSkinAnimationPreset::spring ||
                                active_animation_ == CandidateSkinAnimationPreset::glow);
  float motion_alpha = 1.0f;
  float motion_scale = 1.0f;
  float motion_offset_y = 0.0f;
  if (motion_entering) {
    motion_alpha = finishing_animation_
                       ? (active_animation_ == CandidateSkinAnimationPreset::glow
                              ? 1.0f - 0.18f * eased
                              : 1.0f - eased)
                       : (active_animation_ == CandidateSkinAnimationPreset::glow
                              ? 0.82f + 0.18f * eased
                              : eased);
    if (active_animation_ == CandidateSkinAnimationPreset::fade_scale ||
        active_animation_ == CandidateSkinAnimationPreset::spring)
      motion_scale = finishing_animation_
                         ? (active_animation_ == CandidateSkinAnimationPreset::spring
                                ? 1.0f - 0.04f * (eased + 0.08f * std::sin(eased * 6.2831853f) * (1.0f - eased))
                                : 1.0f - 0.04f * eased)
                         : (active_animation_ == CandidateSkinAnimationPreset::spring
                                ? 0.96f + 0.04f * (eased + 0.08f * std::sin(eased * 6.2831853f) * (1.0f - eased))
                                : 0.96f + 0.04f * eased);
    // slide_up 进场从下方 8 DIP 升到原位；退场是进场的倒放，向下沉出 8 DIP，和 macOS 的 CenteredTransform(layer, 1.0, -kSlideDistance) 一样（D2D 的 y 朝下，所以是正的）。
    if (active_animation_ == CandidateSkinAnimationPreset::slide_up)
      motion_offset_y = finishing_animation_ ? 8.0f * eased : 8.0f * (1.0f - eased);
  }
  // 逐行浮现（fade_scale、slide_up 的进场）：每一行在卡片之外再按自己的进度淡入、从下方 3 DIP 升到原位，第 i 行晚 min(i × stagger_ms, 80ms) 开始，见 CandidateMotion.h。
  const bool row_reveal = motion_entering && !finishing_animation_ &&
                          candidate_motion_staggers_rows(active_animation_) &&
                          skin_motion_enabled();
  const uint64_t motion_elapsed = GetTickCount64() - animation_started_;
  const float card_alpha = motion_alpha;
  // brush() caches one brush per colour and the cache never shrinks, so the per-frame opacity takes the typing flash's finite steps.
  motion_alpha = typing_effect_quantized_alpha(motion_alpha);
  // 画刷当下乘的透明度：卡片的进出场，候选行逐行浮现时再乘上那一行的进度。
  float paint_alpha = motion_alpha;
  auto brush = [&](const CandidateColor &color) {
    auto faded = color;
    faded.a *= paint_alpha;
    auto *value = device_.GetSolidColorBrush(D2D1::ColorF(faded.r, faded.g, faded.b, faded.a));
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
  // 皮肤与用户偏好的先后，和颜色、字体的叠法一样：用户在设置里真正能改的项由用户说了算，其余的由皮肤包决定，皮肤没写的回到调色板。卡片圆角是用户的 candidate_corner_radius，其次皮肤的 corner_radius_dip，再次调色板；高亮圆角是皮肤的 item_corner_radius_dip 或调色板的，用户设了卡片圆角时仍被它封顶（candidate_row_radius）；外框线宽、阴影档位、材质和版式 Windows 上没有用户偏好，皮肤直接决定；用户的不透明度照常淡化卡片底色、背景图和外框。
  const float border_width = skin_layout_.border_width.value_or(palette_.border_width);
  const float inset = border_width / 2.0f;
  // Candidate labels and annotations take a package's `candidate_text`; the pager, toolbar, keymap and menus keep `text`.
  const CandidateColor text_color = palette_.candidate_text.value_or(palette_.text);
  target->BeginDraw();
  // Clear to nothing: only the rounded card itself is opaque, so the corners
  // stay transparent rather than showing a square window edge.
  target->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
  // Power Mode 的抖动：整张卡片连同阴影横向位移一两个设备无关像素，阴影的透明边距装得下；抖完回到原位，点击区域不受影响。
  // 每帧都设一次：上一帧若在 EndDraw 之前抛出，留在目标上的位移不会带进这一帧。
  const float shake = effect_shaking_
                          ? typing_shake_offset(static_cast<uint32_t>(GetTickCount64() - effect_started_), effect_settings_.intensity)
                          : 0.0f;
  auto transform = D2D1::Matrix3x2F::Translation(shake, motion_offset_y);
  if (motion_scale != 1.0f)
    transform = transform * D2D1::Matrix3x2F::Scale(
                              motion_scale, motion_scale,
                              D2D1::Point2F(size.width / 2.0f, size.height / 2.0f));
  target->SetTransform(transform);
  // canvas 皮肤的面板比内容卡片四边各大出 canvas_offset_（内容内边距）；没有 canvas 时面板就是卡片。阴影、底色、背景图、外框、打字闪光、吉祥物和自由挂件都对着面板画，候选内容照旧按卡片（frame）排。
  const CandidateCanvasRect panel{frame.card_left - canvas_offset_.left,
                                  frame.card_top - canvas_offset_.top,
                                  frame.card_left + frame.card_width + canvas_offset_.right,
                                  frame.card_top + frame.card_height + canvas_offset_.bottom};
  // 自由挂件按 msime-windows CandidateSkinOverlay 的规则贴着面板、等比缩放在自己的框里；跟着进出场动效一起淡入淡出，用户的不透明度只淡化面板，不淡化挂件。
  auto draw_decorations = [&](const std::vector<CandidateCanvasDecoration> &layer) {
    for (const auto &decoration : layer) {
      D2D1_SIZE_F natural{};
      auto *bitmap = skin_image(decoration.image, &natural);
      if (!bitmap)
        continue;
      if (const auto rect = candidate_canvas_contain(
              candidate_canvas_decoration_box(decoration, panel), natural.width, natural.height))
        target->DrawBitmap(bitmap,
                           D2D1_RECT_F{static_cast<float>(rect->left), static_cast<float>(rect->top),
                                       static_cast<float>(rect->right),
                                       static_cast<float>(rect->bottom)},
                           motion_alpha, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    }
  };
  // z_index 低于 1 的挂件在面板和它的阴影之下。
  draw_decorations(canvas_layers_.below);
  // 九宫格面板图：画得出来时它就是卡片的样子，底色、背景图、外框和阴影都让给它（同 CSS border-image 与 msime-windows 的 panelImage）。图在套用皮肤时确认过解得出来，这里仍拿不到位图（设备刚丢失之类）时退回底色，下一帧再试。
  ID2D1Bitmap *const panel_bitmap = candidate_canvas_panel_drawn(canvas_)
                                        ? skin_image(canvas_.border->source)
                                        : nullptr;
  const D2D1_RECT_F card_rect{
      static_cast<float>(panel.left) + inset,
      static_cast<float>(panel.top) + inset,
      static_cast<float>(panel.right) - inset,
      static_cast<float>(panel.bottom) - inset};
  // The Fluent flyout shadow, 0 8px 16px: a 16px CSS blur is a Gaussian of sigma 8. 皮肤的 shadow 档位只缩放它的不透明度，`none` 时不画。
  const float shadow_alpha = candidate_skin_shadow_alpha(skin_layout_.shadow, palette_.shadow_alpha);
  const WindowShadowPass shadow_passes[] = {
      {8.0f, shadow_alpha, 0.0f, 8.0f},
  };
  // The user's radius, else the package's, else the theme's.
  const float radius = candidate_card_radius(style_, skin_radius_, palette_.radius);
  const float row_radius = candidate_row_radius(
      style_, skin_layout_.item_corner_radius.value_or(palette_.item_radius), radius);
  // 卡片外缘：外框线的外沿，圆角跟着外扩半个线宽（直角卡片的外框是直角）。系统材质按它裁剪，阴影在垫了材质时只画在它外面。
  const D2D1_RECT_F card_outer{static_cast<float>(panel.left), static_cast<float>(panel.top),
                               static_cast<float>(panel.right), static_cast<float>(panel.bottom)};
  const float card_outer_radius = radius > 0.0f ? radius + inset : 0.0f;
  // 透明度为 0 的一层画不出来时，draw_window_shadow_passes 会退回一层兜底阴影，所以 `none` 直接跳过。
  if (shadow_alpha > 0.0f && !panel_bitmap) {
    // 垫了材质时卡片底色是透明的，阴影透上来会把毛玻璃压暗，只画在卡片外面（msime-windows 的 SetShadowOutsideOnly）；裁剪用的几何建不起来就不画阴影。
    Microsoft::WRL::ComPtr<ID2D1Geometry> outside;
    if (backdrop.active()) {
      Microsoft::WRL::ComPtr<ID2D1Factory> factory;
      target->GetFactory(&factory);
      Microsoft::WRL::ComPtr<ID2D1RectangleGeometry> everything;
      Microsoft::WRL::ComPtr<ID2D1RoundedRectangleGeometry> inside;
      Microsoft::WRL::ComPtr<ID2D1GeometryGroup> group;
      if (factory &&
          SUCCEEDED(factory->CreateRectangleGeometry(
              D2D1::RectF(-size.width, -size.height, size.width * 2.0f, size.height * 2.0f), &everything)) &&
          SUCCEEDED(factory->CreateRoundedRectangleGeometry(D2D1::RoundedRect(card_outer, card_outer_radius, card_outer_radius),
                                                            &inside))) {
        ID2D1Geometry *parts[] = {everything.Get(), inside.Get()};
        if (SUCCEEDED(factory->CreateGeometryGroup(D2D1_FILL_MODE_ALTERNATE, parts, 2, &group)))
          outside = group;
      }
    }
    if (!backdrop.active() || outside) {
      if (outside)
        target->PushLayer(D2D1::LayerParameters(D2D1::InfiniteRect(), outside.Get()), nullptr);
      draw_window_shadow_passes(target, card_rect, radius, shadow_passes,
                                std::size(shadow_passes));
      if (outside)
        target->PopLayer();
    }
  }
  const D2D1_ROUNDED_RECT card{card_rect, radius, radius};
  // The user's opacity fades the card itself - surface, package background and border - and nothing drawn on it, so the text stays as legible as the theme made it. 垫了系统材质时色调已经在合成层里，卡片不再叠底色；要材质却画不出来时底色补成不透明（candidate_backdrop_surface_alpha）。材质本身随用户的不透明度一起淡化，见 EndDraw 之前的 SetBackdrop。
  if (panel_bitmap) {
    draw_canvas_panel(target, panel_bitmap, *canvas_.border, panel, style_.opacity() * motion_alpha);
  } else {
    auto surface = palette_.surface;
    surface.a = candidate_backdrop_surface_alpha(backdrop, surface.a);
    target->FillRoundedRectangle(card, brush(candidate_faded(surface, style_)));
  }
  const float background_opacity = panel_bitmap ? 0.0f : background_.opacity * style_.opacity();
  const uint64_t effect_elapsed = GetTickCount64() - effect_started_;
  const float flash = effect_flashing_ ? typing_effect_flash_alpha(effect_, effect_settings_.intensity, effect_elapsed, effect_settings_.flash_millis) : 0.0f;
  // The package background sits on the surface and under the border and the text, masked by the card's rounded outline.
  if (!background_.image.empty() && background_opacity > 0.0f) {
    D2D1_SIZE_F natural{};
    auto *bitmap = skin_image(background_.image, &natural);
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
  // 皮肤写 `border_width_dip = 0` 就是不要外框，不交给 Direct2D 去画一条 0 宽的线。面板图自带边框。
  if (border_width > 0.0f && !panel_bitmap)
    target->DrawRoundedRectangle(card, brush(candidate_faded(palette_.border, style_)),
                                 border_width);
  // The typing flash: a faint accent wash over the surface, under the text, and an accent outline that grows with the style. Both fade with the flash. The accent is the system's, as on macOS; an effect pack's first colour takes its place.
  if (flash > 0.0f) {
    const auto flash_color = effect_settings_.color ? candidate_rgb(*effect_settings_.color) : effect_accent_;
    // 不透明度量化成有限的几档：brush() 按颜色缓存画刷，每帧一个新的透明度会让缓存一直变大。
    auto wash = flash_color;
    wash.a = typing_effect_quantized_alpha(flash * 0.12f);
    target->FillRoundedRectangle(card, brush(wash));
    auto outline = flash_color;
    outline.a = typing_effect_quantized_alpha(flash);
    target->DrawRoundedRectangle(card, brush(outline),
                                 border_width + static_cast<float>(static_cast<uint32_t>(effect_.style)));
  }
  // The mascot, drawn last so it sits over the card's top edge - that overlap
  // is the whole point of the decoration.
  if (!decoration_image_.empty() && decoration_offset_ > 0.0f) {
    D2D1_SIZE_F natural{};
    if (auto *bitmap = skin_image(decoration_image_, &natural)) {
      // A package gives a width, not a box: the height follows the image's own aspect ratio, and artwork too tall for the band plus the pad_y overlap shrinks uniformly instead of being squashed. Placed along the card's top edge as the manifest aligns it, as the settings preview places it.
      if (const auto rect = candidate_decoration_rect(
              decoration_align_, static_cast<float>(panel.left),
              static_cast<float>(panel.right),
              static_cast<float>(panel.top),
              static_cast<float>(metrics.pad_x),
              static_cast<float>(metrics.pad_y), decoration_offset_,
              static_cast<float>(decoration_width_), natural.width,
              natural.height)) {
        target->DrawBitmap(
            bitmap, D2D1_RECT_F{rect->left, rect->top, rect->right, rect->bottom},
            motion_alpha, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
        if (decoration_candidate_) {
          const auto &overlay = *decoration_candidate_;
          const auto highlighted = std::find_if(value->candidates.begin(), value->candidates.end(),
              [](const PresentationCandidate &candidate) { return candidate.highlighted; });
          const int highlight = highlighted == value->candidates.end() ? -1
              : static_cast<int>(highlighted - value->candidates.begin());
          if (const auto seat = overlay.seat(value->candidates.size(), highlight)) {
            const auto &candidate = value->candidates[*seat];
            // 原文已按输出模式转换；只控制独立标注，不猜测括号、纠错标记或后缀。
            const auto word = candidate.text + (overlay.stripBadge ? std::string{} : candidate.badge);
            if (!word.empty() && word.size() <= 4096) {
              const float box_width = static_cast<float>(decoration_width_);
              const float box_height = static_cast<float>(decoration_top_);
              const float align = decoration_align_ == CandidateSkinAlign::left ? 0.0f
                  : decoration_align_ == CandidateSkinAlign::center ? 0.5f : 1.0f;
              // 图片等比缩小时，声明框仍沿同一边与图片底边对齐。
              const D2D1_RECT_F box{rect->left + (rect->right - rect->left - box_width) * align,
                  rect->bottom - box_height,
                  rect->left + (rect->right - rect->left - box_width) * align + box_width, rect->bottom};
              const auto text = candidate_decoration_text(device_.GetDWriteFactory(),
                  format(overlay.fontSizeDip, DWRITE_TEXT_ALIGNMENT_LEADING), wide(word), overlay, box);
              if (text)
                target->DrawTextLayout(text->origin, text->layout.Get(),
                    brush(parse_css_color(overlay.color, candidate_rgb(0xffffff))),
                    D2D1_DRAW_TEXT_OPTIONS_NONE);
            }
          }
        }
      }
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
  // 皮肤打开 `page_arrows` 时首行不画翻页，箭头在候选画完之后画进列表（candidate_list_arrows_layout）。
  const std::optional<CandidatePagerLayout> pager =
      metrics.list_arrows ? std::nullopt
                          : candidate_pager_layout(
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
  // 拼音占首行里 logo 之后、翻页之前的那一段，再按皮肤的 preedit_margin_dip 往里收。
  const auto band = candidate_header_band(metrics);
  const D2D1_RECT_F preedit_rect{
      static_cast<float>(frame.card_left + candidate_preedit_left(metrics)),
      static_cast<float>(frame.card_top + band.top),
      static_cast<float>(frame.card_left +
                         (pager ? pager->left - metrics.pager_gap
                                : frame.card_width - metrics.pad_x / 2.0 -
                                      metrics.preedit_margin.right)),
      static_cast<float>(frame.card_top) + static_cast<float>(band.bottom)};
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
                        preedit_rect, brush(flash > 0.0f && effect_.tier_up ? effect_accent_ : palette_.number),
                        D2D1_DRAW_TEXT_OPTIONS_CLIP);
  }
  if (show_preedit_) {
    const D2D1_RECT_F &rect = preedit_rect;
    // A skin's preedit band is rounded like the rows, as msime-windows draws it, and its divider is a 1 DIP rule under the whole preedit row, across the card's content width.
    if (palette_.preedit_background && rect.right > rect.left)
      target->FillRoundedRectangle(D2D1_ROUNDED_RECT{rect, row_radius, row_radius},
                                   brush(*palette_.preedit_background));
    if (palette_.preedit_divider)
      target->FillRectangle(
          D2D1_RECT_F{static_cast<float>(frame.card_left + metrics.pad_x / 2.0),
                      rect.bottom - 1.0f,
                      static_cast<float>(frame.card_left + frame.card_width -
                                         metrics.pad_x / 2.0),
                      rect.bottom},
          brush(*palette_.preedit_divider));
    const auto text = wide(value->preedit);
    auto *preedit_format = format(preedit_font_size_, DWRITE_TEXT_ALIGNMENT_LEADING, false,
                                  false, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    auto *preedit_brush = brush(palette_.preedit_text.value_or(palette_.accent));
    const bool caret = value->preedit_caret != std::string::npos &&
                       value->preedit_caret <= value->preedit.size();
    // Same clamp, same reason as the candidate rows below: a long enough reading would otherwise be drawn past the card, or under the pager.
    if (rect.right > rect.left && !caret) {
      target->DrawText(text.c_str(), static_cast<UINT32>(text.size()), preedit_format, rect,
                       preedit_brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
    } else if (rect.right > rect.left) {
      // The insertion point. Without it, moving left or right inside a long pinyin string gave no indication of where the next key would land. 和 macOS 一样，光标在拼音中间时两边的字让开，不压字；拼音长过这一段时整行左移，插入点总在看得见的地方（candidate_preedit_caret）。
      const auto before = wide(value->preedit.substr(0, value->preedit_caret));
      const auto after = text.substr((std::min)(before.size(), text.size()));
      const auto place = candidate_preedit_caret(
          measured_width(device_, before, font_family_, static_cast<float>(preedit_font_size_),
                         font_fallback_.Get(), DWRITE_FONT_WEIGHT_SEMI_BOLD),
          !after.empty(), static_cast<double>(rect.right - rect.left));
      const float scroll = static_cast<float>(place.scroll);
      target->PushAxisAlignedClip(rect, D2D1_ANTIALIAS_MODE_ALIASED);
      if (!before.empty())
        target->DrawText(before.c_str(), static_cast<UINT32>(before.size()), preedit_format,
                         D2D1_RECT_F{rect.left - scroll, rect.top, rect.right, rect.bottom},
                         preedit_brush);
      if (!after.empty())
        target->DrawText(after.c_str(), static_cast<UINT32>(after.size()), preedit_format,
                         D2D1_RECT_F{rect.left + static_cast<float>(place.after), rect.top,
                                     (std::max)(rect.right, rect.left + static_cast<float>(place.after)),
                                     rect.bottom},
                         preedit_brush);
      // A hairline rather than a filled block, so it does not obscure the character it sits before.
      const float inset_y = static_cast<float>(band.height) * 0.15f;
      const float x = rect.left + static_cast<float>(place.caret);
      target->FillRectangle(
          D2D1_RECT_F{x, rect.top + inset_y,
                      x + static_cast<float>(candidate_preedit_caret_width),
                      rect.bottom - inset_y},
          brush(palette_.preedit_caret.value_or(palette_.accent)));
      target->PopAxisAlignedClip();
    }
  }
  // The selection number keeps its own column so candidates start on one
  // vertical line, as the shipped card does.
  const float gutter = static_cast<float>(metrics.number_and_bar);
  const size_t count = value->candidates.size();
  // Laid out at the width actually drawn, which the work area may have narrowed below what card_bounds asked for.
  const size_t reserved_lines = horizontal_ ? reserved_secondary_lines(*value) : 0;
  const auto items = measure_items(*value);
  const auto wrapped = wrap_measure(*value);
  const double minimum_row = candidate_reserved_row_height(
      metrics, reserved_lines, reserved_secondary_height(reserved_lines));
  // 卷轴展开：首行和网格排在滚动区里，按当前偏移往上移（偏移按设备像素取整，文字不会在两帧之间重采样）；按钮固定在右上角。滚动区的高度不超过这张卡片实际拿到的高度。
  std::optional<CandidateExpandLayout> expand_layout;
  ExpandGrid grid;
  double expand_offset = 0.0;
  std::vector<CandidateRowLayout> rows;
  if (metrics.expand_panel) {
    grid = expand_grid(*value);
    expand_layout = candidate_expand_layout(
        items, frame.card_width, metrics, grid_widths(grid), expand_page_size(*value),
        frame.card_height - metrics.pad_y - metrics.slack_y - metrics.preedit_row, wrapped,
        minimum_row,
        candidate_expand_held_viewport(expand_.expanded, value->expanded != nullptr,
                                       expand_viewport_));
    candidate_expand_clamp(expand_, expand_layout->max_offset());
    const double pixels = layout_scale(GetDpiForWindow(window_));
    expand_offset = std::round(expand_.offset * pixels) / pixels;
    rows = expand_layout->rows;
    for (auto &row : rows) {
      row.bounds.top -= expand_offset;
      row.bounds.bottom -= expand_offset;
    }
    target->PushAxisAlignedClip(box({expand_layout->left, expand_layout->top,
                                     expand_layout->right, expand_layout->bottom()}),
                                D2D1_ANTIALIAS_MODE_ALIASED);
  } else {
    rows = candidate_page_layout(items, frame.card_width, metrics, horizontal_, wrapped,
                                 minimum_row);
  }
  const float first_line = static_cast<float>(metrics.candidate_row);
  // 皮肤 `item_corners_follow_card`：高亮落在列表四角上的角跟卡片同圆角。msime-windows 以外圆角大于 0 作为开关，卡片是直角时也取一个极小值，免得贴角的高亮退回普通圆角、从直角外框里探出来。
  const auto rows_area = metrics.corners_follow_card
                             ? candidate_rows_area(frame.card_width, rows, metrics)
                             : std::nullopt;
  const double outer_radius = (std::max)(static_cast<double>(radius), 0.01);
  for (size_t i = 0; i < count; ++i) {
    const auto &row = rows[i].bounds;
    const auto &item = rows[i].item;
    // 逐行浮现：这一行的透明度和位移只作用在画上，行的位置和点击区域不动。
    if (row_reveal) {
      const float reveal = candidate_ease_out(candidate_row_reveal_progress(
          motion_elapsed, i, animation_.stagger_ms, animation_.duration_ms));
      paint_alpha = typing_effect_quantized_alpha(card_alpha * reveal);
      target->SetTransform(
          D2D1::Matrix3x2F::Translation(
              0.0f, static_cast<float>(candidate_row_reveal_distance) * (1.0f - reveal)) *
          transform);
    }
    // Rows are laid out in card coordinates; the decoration strip sits above
    // the card, so every row moves down with it. Without this the rows would
    // be drawn over the artwork and the hit test below would disagree.
    const D2D1_RECT_F rect{static_cast<float>(frame.card_left + row.left),
                           static_cast<float>(frame.card_top + row.top),
                           static_cast<float>(frame.card_left + row.right),
                           static_cast<float>(frame.card_top + row.bottom)};
    if (value->candidates[i].highlighted || hovered_ == i) {
      const bool moving = selection_motion && value->candidates[i].highlighted;
      // 行挨着行时底色比行四边各缩进 selection_inset，相邻两行的底色之间留出一条缝（macOS 的 NSInsetRect(bounds, 1, 1)）。
      const float fill_inset = static_cast<float>(metrics.selection_inset);
      const D2D1_RECT_F inset_rect{rect.left + fill_inset, rect.top + fill_inset,
                                   rect.right - fill_inset, rect.bottom - fill_inset};
      D2D1_RECT_F fill = inset_rect;
      auto fill_color = value->candidates[i].highlighted ? palette_.selected : palette_.hover;
      if (moving && active_animation_ == CandidateSkinAnimationPreset::spring) {
        // The highlight grows from 94% past its size and settles; the text stays where hit testing expects it.
        const float grow = 0.94f + 0.06f * eased + 0.12f * std::sin(eased * 3.1415927f) * (1.0f - eased);
        const float half_width = (inset_rect.right - inset_rect.left) * grow * 0.5f;
        const float half_height = (inset_rect.bottom - inset_rect.top) * grow * 0.5f;
        const float center_x = (inset_rect.left + inset_rect.right) * 0.5f;
        const float center_y = (inset_rect.top + inset_rect.bottom) * 0.5f;
        fill = {center_x - half_width, center_y - half_height, center_x + half_width,
                center_y + half_height};
      } else if (moving && active_animation_ != CandidateSkinAnimationPreset::glow) {
        fill_color.a *= typing_effect_quantized_alpha(0.45f + 0.55f * eased);
      }
      const D2D1_ROUNDED_RECT selection{fill, row_radius, row_radius};
      Microsoft::WRL::ComPtr<ID2D1PathGeometry> cornered;
      if (rows_area)
        cornered = corner_rect_geometry(
            target, fill,
            candidate_row_corner_radii(row, *rows_area, metrics.preedit_row <= 0.0,
                                       row_radius, outer_radius));
      if (cornered)
        target->FillGeometry(cornered.Get(), brush(fill_color));
      else
        target->FillRoundedRectangle(selection, brush(fill_color));
      if (moving && active_animation_ == CandidateSkinAnimationPreset::glow) {
        auto glow = palette_.accent;
        glow.a = typing_effect_quantized_alpha(0.65f * (1.0f - eased));
        if (cornered)
          target->DrawGeometry(cornered.Get(), brush(glow), 2.0f);
        else
          target->DrawRoundedRectangle(selection, brush(glow), 2.0f);
      }
      if (value->candidates[i].highlighted && palette_.show_selected_bar) {
        // 竖条的宽、高和位置可以由皮肤改（selected_bar_width、selected_bar_height_em、selected_bar_inside）；缺省版式和 macOS 一样画在行内 1 DIP 处。
        const auto extent = candidate_selection_bar(
            rect.left, rect.top, rect.bottom, metrics.candidate_row,
            metrics.selection_bar_width, metrics.selection_bar_height,
            metrics.selection_bar_offset);
        const float bar_radius =
            static_cast<float>(metrics.selection_bar_width * 0.5);
        const D2D1_ROUNDED_RECT bar{{static_cast<float>(extent.left),
                                     static_cast<float>(extent.top),
                                     static_cast<float>(extent.right),
                                     static_cast<float>(extent.bottom)},
                                    bar_radius, bar_radius};
        target->FillRoundedRectangle(
            bar, brush(palette_.selected_bar.value_or(palette_.accent)));
      }
    }
    // Alpha 0 means the skin named no selected colour, so the row keeps its
    // normal one. Skins that fill the selection with an opaque accent set it,
    // because their unselected text would otherwise be unreadable on the fill.
    const bool selected = value->candidates[i].highlighted;
    const auto number_color = candidate_row_number_color(palette_, selected);
    // AI 占位行用序号的颜色（次要色）画，和真候选区分开。
    const auto row_text_color =
        value->candidates[i].pending
            ? number_color
            : candidate_row_text_color(palette_, text_color, selected,
                                       value->candidates[i].fixed_position != 0);
    // 序号从行左缘 number_left 处左对齐（缺省 8 DIP，皮肤写了 number_inset 时是它），到候选文字之前为止，和第一行文字竖直居中，和 macOS 一样。number_scale 让序号字号小到画不出（0）时就不画。
    const auto number_label = std::to_wstring(i + 1);
    if (metrics.number_font >= 1.0)
      target->DrawText(
          number_label.c_str(), static_cast<UINT32>(number_label.size()),
          format(metrics.number_font, DWRITE_TEXT_ALIGNMENT_LEADING),
          D2D1_RECT_F{rect.left + static_cast<float>(metrics.number_left), rect.top,
                      rect.left + gutter, rect.top + first_line},
          brush(number_color));
    const auto text = wide(candidate_primary_text(value->candidates[i]));
    // Text wider than its column was laid out wrapped (wrapped_height() at this same width), and the row already grew by that height, so it wraps here inside the row that hit testing uses. Text that fits keeps the single-line format, so rounding cannot wrap what was laid out as one line. Still clipped to the row as a guard: without it anything the layout did not account for would paint past the card edge onto the transparent shadow margin.
    target->DrawText(
        text.c_str(), static_cast<UINT32>(text.size()),
        format(font_size_, DWRITE_TEXT_ALIGNMENT_LEADING, item.text_wrapped, true),
        D2D1_RECT_F{rect.left + gutter, rect.top, rect.right,
                    rect.top + static_cast<float>(item.text_height)},
        brush(row_text_color), D2D1_DRAW_TEXT_OPTIONS_CLIP);
    // The annotation takes the candidate text's colour, as macOS draws it with titleAttributes: the fixed-position colour on an unselected fixed row, the selected text colour on the selected row. The translation is the theme's secondary colour: a package's translation colour, or else the number colour (selected_number on the selected row); 未高亮的固定排位候选没有皮肤释义色时用它正文色的 0.62 倍透明度。
    const auto annotation_color =
        candidate_row_text_color(palette_, text_color, selected,
                                 value->candidates[i].fixed_position != 0);
    const auto translation_color = candidate_row_translation_color(
        palette_, selected, value->candidates[i].fixed_position != 0);
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
    draw_run(candidate_annotation_run(value->candidates[i]), item.annotation, font_size_,
             annotation_color);
    const auto secondary_run = candidate_secondary_run(value->candidates[i]);
    const auto &secondary = secondary_run.text;
    // Tab 预选的释义列在高亮候选上画下划线，和 macOS 候选按钮的 armedGlossColumn 一样；数字、空格会上屏这一列。
    const auto armed = selected ? candidate_gloss_column_range(value->candidates[i],
                                                               value->armed_gloss_column)
                                : std::nullopt;
    // 读音按释义色的 0.72 倍透明度画，比它跟着的释义轻一些；选中底色不透明时不减淡，那时的释义色已经是按底色挑出的次要色，再减淡在中等亮度的底色上就看不清（macOS 的 MSIMECandidatePronunciationOpacity 同样例外）。
    const bool fade_readings =
        !secondary_run.readings.empty() && !(selected && palette_.selected.a >= 0.85f);
    // 脚本插件的注释单独占释义那一段的最后一行，画成斜体，和输入法自己的译文、读音分开，颜色仍是释义色。
    const bool italic_script = secondary_run.script_start != std::string::npos;
    // 下划线、读音减淡和插件注释的斜体都要给 IDWriteTextLayout 的一段设样式；建不出来时整段按释义色画，文字照样看得到。
    Microsoft::WRL::ComPtr<IDWriteTextLayout> styled;
    auto *factory = device_.GetDWriteFactory();
    if ((armed || fade_readings || italic_script) && factory && item.translation.width > 0.0) {
      const auto run_text = wide(secondary);
      // UTF-8 字节位置换成 UTF-16 下标；各段的起止都在完整字符上。
      const auto utf16 = [&](size_t bytes) {
        return static_cast<UINT32>(wide(secondary.substr(0, bytes)).size());
      };
      const auto range = [&](size_t start, size_t length) {
        const auto first = utf16(start);
        return DWRITE_TEXT_RANGE{first, utf16(start + length) - first};
      };
      const float left = rect.left + gutter + static_cast<float>(item.translation.x);
      bool ready =
          SUCCEEDED(factory->CreateTextLayout(
              run_text.c_str(), static_cast<UINT32>(run_text.size()),
              format(metrics.translation_font, DWRITE_TEXT_ALIGNMENT_LEADING,
                     item.translation.below),
              (std::max)(rect.right - left, 0.0f),
              static_cast<float>(item.translation.height), styled.GetAddressOf())) &&
          styled;
      if (ready && armed)
        ready = SUCCEEDED(styled->SetUnderline(TRUE, range(armed->first, armed->second)));
      if (ready && fade_readings) {
        auto reading_color = translation_color;
        reading_color.a *= 0.72f;
        auto *reading_brush = brush(reading_color);
        for (const auto &[start, length] : secondary_run.readings)
          if (ready)
            ready = SUCCEEDED(styled->SetDrawingEffect(reading_brush, range(start, length)));
      }
      if (ready && italic_script)
        ready = SUCCEEDED(styled->SetFontStyle(
            DWRITE_FONT_STYLE_ITALIC,
            range(secondary_run.script_start, secondary.size() - secondary_run.script_start)));
      if (ready)
        target->DrawTextLayout(
            D2D1_POINT_2F{left, rect.top + static_cast<float>(item.translation.y)},
            styled.Get(), brush(translation_color), D2D1_DRAW_TEXT_OPTIONS_CLIP);
      else
        styled.Reset();
    }
    if (!styled)
      draw_run(secondary, item.translation, metrics.translation_font, translation_color);
  }
  // 逐行浮现改过的透明度和位移只属于候选行，后面的网格、箭头照卡片本身画。
  if (row_reveal) {
    paint_alpha = motion_alpha;
    target->SetTransform(transform);
  }
  if (expand_layout) {
    // 网格：没有序号、没有选中态，文字和首行的候选文字对齐；悬停和按下都铺 hover 底色（msime-windows 的网格沿用首行的外观，selectable 关掉）。只画看得见的那几行。
    const auto cell_text = candidate_row_text_color(palette_, text_color, false, false);
    for (size_t slot = 0; slot < expand_layout->cells.size() && slot < grid.count; ++slot) {
      auto cell = expand_layout->cells[slot];
      cell.top -= expand_offset;
      cell.bottom -= expand_offset;
      if (cell.bottom <= expand_layout->top || cell.top >= expand_layout->bottom())
        continue;
      const auto rect = box(cell);
      if (hovered_cell_ == slot || pressed_cell_ == slot)
        target->FillRoundedRectangle(D2D1_ROUNDED_RECT{rect, row_radius, row_radius},
                                     brush(palette_.hover));
      const auto text = wide((*grid.items)[grid.start + slot].text);
      target->DrawText(text.c_str(), static_cast<UINT32>(text.size()),
                       format(font_size_, DWRITE_TEXT_ALIGNMENT_LEADING),
                       D2D1_RECT_F{rect.left + gutter, rect.top, rect.right, rect.bottom},
                       brush(cell_text), D2D1_DRAW_TEXT_OPTIONS_CLIP);
    }
    // 细滚动条：只画滑块，不画轨道，序号色；指针在滚动条上或正在拖时更深（msime-windows 的 0.45 与 0.85）。
    if (const auto thumb = candidate_expand_thumb(*expand_layout, expand_.offset)) {
      auto color = palette_.number;
      color.a *= hovered_scrollbar_ || thumb_grab_ ? 0.85f : 0.45f;
      const float thumb_radius = static_cast<float>(metrics.expand_panel->thumb_radius);
      target->FillRoundedRectangle(D2D1_ROUNDED_RECT{box(*thumb), thumb_radius, thumb_radius},
                                   brush(color));
    }
    target->PopAxisAlignedClip();
    // 展开按钮：序号色的折线，收起时尖朝下「∨」，展开时尖朝上「∧」；悬停铺 hover，按下铺 selected（透明时仍是 hover）。
    const auto &expand_style = *metrics.expand_panel;
    const auto button = box(expand_layout->toggle);
    if (pressed_toggle_ || hovered_toggle_) {
      const float button_radius = (std::min)(4.0f, row_radius);
      const auto fill = pressed_toggle_ && palette_.selected.a > 0.001f ? palette_.selected
                                                                        : palette_.hover;
      target->FillRoundedRectangle(D2D1_ROUNDED_RECT{button, button_radius, button_radius},
                                   brush(fill));
    }
    const float half_width =
        (std::min)(static_cast<float>(expand_style.glyph), button.right - button.left) * 0.5f;
    const float half_height = half_width * 0.5f;
    const float cx = (button.left + button.right) * 0.5f, cy = (button.top + button.bottom) * 0.5f;
    const float tip = expand_.expanded ? cy - half_height : cy + half_height;
    const float tail = expand_.expanded ? cy + half_height : cy - half_height;
    const float stroke = static_cast<float>(expand_style.stroke);
    target->DrawLine(D2D1_POINT_2F{cx - half_width, tail}, D2D1_POINT_2F{cx, tip},
                     brush(palette_.number), stroke);
    target->DrawLine(D2D1_POINT_2F{cx, tip}, D2D1_POINT_2F{cx + half_width, tail},
                     brush(palette_.number), stroke);
  }
  // 皮肤 `page_arrows`：两枚翻页箭头画在列表里，按 msime-windows 的 PagerArrows。chevron 是序号色的折线，triangle 是候选文字色的实心三角、横排时前面一条 page_arrow_divider 色的竖线；悬停时铺 hover 底色，按下时铺 selected（透明时仍是 hover）。上一页在第一页上变暗、不响应，下一页一直可用，和首行的翻页同一条规则。
  const auto list_pager = candidate_list_arrows_layout(frame.card_width, rows, metrics);
  if (list_pager && metrics.list_arrows) {
    const auto &arrows = *metrics.list_arrows;
    if (arrows.divider_width > 0.0 && palette_.page_arrow_divider) {
      const auto &previous = list_pager->previous;
      target->FillRectangle(box({previous.left - arrows.divider_gap - arrows.divider_width,
                                 previous.top, previous.left - arrows.divider_gap,
                                 previous.bottom}),
                            brush(*palette_.page_arrow_divider));
    }
    const CandidateColor glyph = arrows.triangle ? text_color : palette_.number;
    const float button_radius = arrows.triangle ? row_radius : (std::min)(3.0f, row_radius);
    for (const bool previous : {true, false}) {
      const D2D1_RECT_F part = box(previous ? list_pager->previous : list_pager->next);
      const bool enabled = !previous || value->page > 0;
      if (enabled && (pressed_page_ == previous || hovered_page_ == previous)) {
        const auto fill = pressed_page_ == previous && palette_.selected.a > 0.001f
                              ? palette_.selected
                              : palette_.hover;
        target->FillRoundedRectangle(D2D1_ROUNDED_RECT{part, button_radius, button_radius},
                                     brush(fill));
      }
      auto color = glyph;
      if (!enabled)
        color.a *= arrows.triangle ? 0.25f : 0.3f;
      if (arrows.triangle) {
        if (const auto shape = rounded_triangle_geometry(
                target, part, previous, static_cast<float>(arrows.glyph_size),
                static_cast<float>(arrows.stroke)))
          target->FillGeometry(shape.Get(), brush(color));
        continue;
      }
      // 高是宽两倍的折线：前一页「<」，后一页「>」。
      const float half = (std::min)(static_cast<float>(arrows.glyph_size), part.bottom - part.top) * 0.5f;
      const float depth = (std::min)(static_cast<float>(arrows.glyph_width()), half * 0.55f);
      const float cx = (part.left + part.right) * 0.5f, cy = (part.top + part.bottom) * 0.5f;
      const float tip = previous ? cx - depth * 0.5f : cx + depth * 0.5f;
      const float tail = previous ? cx + depth * 0.5f : cx - depth * 0.5f;
      const float stroke = static_cast<float>(arrows.stroke);
      target->DrawLine(D2D1_POINT_2F{tail, cy - half}, D2D1_POINT_2F{tip, cy}, brush(color), stroke);
      target->DrawLine(D2D1_POINT_2F{tip, cy}, D2D1_POINT_2F{tail, cy + half}, brush(color), stroke);
    }
  }
  // z_index 不低于 1 的挂件在面板与候选文字之上；挂件只是装饰，点击照样落在下面的候选上。
  draw_decorations(canvas_layers_.above);
  target->SetTransform(D2D1::Matrix3x2F::Identity());
  // 材质的位置和这一帧的内容一起提交：跟着卡片的平移、缩放和抖动走，和卡片底色一样随用户的不透明度与进出场动效淡化。
  if (backdrop.active()) {
    const auto area = candidate_backdrop_rect(card_outer.left, card_outer.top, card_outer.right, card_outer.bottom,
                                              card_outer_radius, shake, motion_offset_y, motion_scale,
                                              size.width / 2.0f, size.height / 2.0f);
    device_.SetBackdrop(D2D1_RECT_F{area.left, area.top, area.right, area.bottom}, area.radius,
                        style_.opacity() * motion_alpha);
  }
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  const HRESULT presented = SUCCEEDED(drawn) ? device_.Present() : drawn;
  // 设备丢了（EndDraw 的 D2DERR_RECREATE_TARGET，或 Present 的 DXGI_ERROR_DEVICE_REMOVED/RESET，见 DeviceResources::IsDeviceLost）不算画不出来：下一次刷新时重建，而不是让整个 Server 退出。
  if (msimeui::DeviceResources::IsDeviceLost(presented)) {
    // Losing the device is not a presentation failure; rebuild on the next
    // refresh rather than hiding a live composition. Clearing shown_ is what
    // makes that rebuild reachable: reposition() returns early while the cached
    // frame still matches, so leaving it set would suppress the very repaint
    // this path is counting on, and painted_ would stay behind for good. That
    // also strands clicks, because hit() refuses without a painted page.
    image_frames_.reset_target();
    device_.DiscardTarget();
    shown_.reset();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Candidate drawing failed");
  if (FAILED(presented))
    throw std::runtime_error("Candidate presentation failed");
  schedule_image_frame();
  painted_ = value;
  painted_rows_ = std::move(rows);
  painted_pager_ = pager ? pager : list_pager;
  painted_dpi_ = GetDpiForWindow(window_);
  painted_backdrop_ = backdrop;
  painted_card_ = D2D1_RECT_F{static_cast<float>(panel.left), static_cast<float>(panel.top),
                              static_cast<float>(panel.right), static_cast<float>(panel.bottom)};
  painted_expand_ = expand_layout;
  painted_expand_offset_ = expand_offset;
  painted_expand_step_ = metrics.candidate_row + metrics.item_gap;
  painted_grid_ = grid;
  if (expand_layout && expand_.expanded && grid.count > 0)
    expand_viewport_ = expand_layout->viewport;
  sync_tooltips();
  if (accessible_) {
    // 展开区：按钮，以及网格里看得见的候选；首行滚出去的候选不报。
    std::optional<CandidateAccessibleExpand> accessible_expand;
    if (expand_layout) {
      accessible_expand.emplace();
      accessible_expand->toggle = expand_layout->toggle;
      accessible_expand->expanded = expand_.expanded;
      accessible_expand->top = expand_layout->top;
      accessible_expand->bottom = expand_layout->bottom();
      for (size_t slot = 0; slot < expand_layout->cells.size() && slot < grid.count; ++slot) {
        auto cell = expand_layout->cells[slot];
        cell.top -= expand_offset;
        cell.bottom -= expand_offset;
        if (cell.bottom > expand_layout->top && cell.top < expand_layout->bottom())
          accessible_expand->cells.push_back({slot, (*grid.items)[grid.start + slot].text, cell});
      }
    }
    accessible_->publish(candidate_accessible_tree(
        *painted_, painted_rows_, painted_pager_, metrics, show_preedit_,
        CandidateAccessibleFrame{frame.card_left, frame.card_top, frame.card_width,
                                 layout_scale(painted_dpi_)},
        static_cast<bool>(click_), static_cast<bool>(page_), MSIME_EDITION_DISPLAY_NAME_UTF8,
        accessible_expand ? &*accessible_expand : nullptr));
  }
  if (rendered_)
    rendered_(*painted_);
  // 后续候选不够填满一屏时，不用等滚动就再排一批。
  reveal_if_near_end();
}
void CandidateWindow::sync_tooltips() {
  if (!tooltip_ || !painted_)
    return;
  // 行在卡片坐标里，换回客户区像素：和 hit() 的换算相反，加上阴影边距和装饰图让出的高度。
  const double scale = layout_scale(painted_dpi_);
  std::vector<RECT> rects;
  if (painted_->pointer_input) {
    rects.reserve(painted_rows_.size());
    for (const auto &row : painted_rows_) {
      // 卷轴展开时首行随内容滚动，提示的区域只留看得见的那一段；整行滚出去时区域为空，不弹提示。下标仍和候选一一对应。
      double top = row.bounds.top, bottom = row.bounds.bottom;
      if (painted_expand_) {
        top = (std::max)(top, painted_expand_->top);
        bottom = (std::min)(bottom, painted_expand_->bottom());
        if (bottom < top)
          bottom = top;
      }
      rects.push_back(
          {static_cast<LONG>(std::floor((row.bounds.left + shadow_insets_.left) * scale)),
           static_cast<LONG>(std::floor((top + shadow_insets_.top + decoration_offset_) * scale)),
           static_cast<LONG>(std::ceil((row.bounds.right + shadow_insets_.left) * scale)),
           static_cast<LONG>(std::ceil((bottom + shadow_insets_.top + decoration_offset_) * scale))});
    }
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
  // 展开按钮与点它一样；网格里的候选与点它一样上屏。
  if (id == candidate_accessible_expand) {
    if (painted_expand_)
      toggle_expansion();
    return;
  }
  if (id >= candidate_accessible_expanded_first) {
    commit_cell(static_cast<size_t>(id - candidate_accessible_expanded_first));
    return;
  }
  // 翻页与点箭头一样一次一页，带上画出来的那一页的身份，页已经换了时 Server 会拒绝。
  if (id == candidate_accessible_previous || id == candidate_accessible_next) {
    const bool previous = id == candidate_accessible_previous;
    if (page_ && painted_pager_ && (!previous || value.page > 0)) {
      CandidatePage page{value.lease, value.session, value.generation, previous, 1u};
      page.translation_page = value.translation_page;
      page_(page);
    }
    return;
  }
  if (!click_ || id < 1 || static_cast<size_t>(id) > value.candidates.size())
    return;
  const auto &candidate = value.candidates[static_cast<size_t>(id - 1)];
  CandidateClick click{value.lease, candidate.session, candidate.generation, candidate.index};
  click.translation_page = value.translation_page;
  click_(click);
}
CandidateSkinMetrics CandidateWindow::skin_metrics(const CandidatePresentation &value) const {
  // canvas 的 `line_gap_dip` 落在拼音行的下外边距上。
  auto skin = candidate_canvas_skin_metrics(
      candidate_skin_metrics(skin_layout_, horizontal_, font_size_), canvas_);
  skin.show_selected_bar = palette_.show_selected_bar;
  if (skin.expand_panel && (!value.pointer_input || value.translation_page != 0)) {
    skin.expand_panel = false;
    skin.page_arrows = skin_layout_.page_arrows.value_or(false);
  }
  return skin;
}
size_t CandidateWindow::expand_page_size(const CandidatePresentation &value) {
  return value.page_size ? value.page_size : value.candidates.size();
}
CandidateWindow::ExpandGrid CandidateWindow::expand_grid(const CandidatePresentation &value) {
  ExpandGrid grid;
  if (!expand_.expanded || !value.expanded || value.candidates.empty() ||
      !skin_metrics(value).expand_panel)
    return grid;
  size_t last = 0;
  for (const auto &candidate : value.candidates)
    last = (std::max)(last, candidate.index);
  grid.items = value.expanded;
  grid.start = candidate_expand_start(*grid.items, last);
  grid.available = grid.items->size() - grid.start;
  if (grid.available == 0)
    return grid;
  const auto &first = (*grid.items)[grid.start];
  candidate_expand_sync(expand_, first.index, first.text, grid.available);
  grid.count = (std::min)(expand_.revealed, grid.available);
  return grid;
}
std::vector<double> CandidateWindow::grid_widths(const ExpandGrid &grid) {
  std::vector<double> widths;
  if (!grid.items || grid.count == 0)
    return widths;
  // 缓存只为眼下的几批候选服务，长过上限时只留这一批的。
  std::vector<const std::string *> batch;
  batch.reserve(grid.count);
  for (size_t slot = 0; slot < grid.count; ++slot)
    batch.push_back(&(*grid.items)[grid.start + slot].text);
  candidate_expand_trim_widths(grid_width_cache_, batch);
  device_.EnsureFactories();
  if (!fallback_families_.empty() && !font_fallback_)
    font_fallback_ = build_font_fallback(device_.GetDWriteFactory(), fallback_families_);
  widths.reserve(grid.count);
  for (size_t slot = 0; slot < grid.count; ++slot) {
    const auto &text = (*grid.items)[grid.start + slot].text;
    auto found = grid_width_cache_.find(text);
    if (found == grid_width_cache_.end())
      found = grid_width_cache_
                  .emplace(text, measured_width(device_, wide(text), font_family_,
                                                static_cast<float>(font_size_),
                                                font_fallback_.Get()))
                  .first;
    widths.push_back(found->second);
  }
  return widths;
}
void CandidateWindow::toggle_expansion() {
  if (!painted_ || !painted_expand_)
    return;
  if (expand_.expanded) {
    collapse_expansion();
  } else {
    candidate_expand_open(expand_);
    expand_lease_ = painted_->lease;
    expand_viewport_ = 0.0;
    if (expansion_)
      expansion_(painted_->lease, true);
  }
  // 展开与收起是两种尺寸：立刻按新的状态重新排版，不等主循环下一轮。
  shown_.reset();
  hovered_cell_.reset();
  pressed_cell_.reset();
  refresh();
}
std::optional<RECT> CandidateWindow::painted_card() const {
  if (!painted_card_ || !painted_dpi_)
    return std::nullopt;
  // DIP 到像素和画图同一个比例：渲染目标的 DPI 是窗口 DPI 乘用户缩放（layout_scale）。
  const double scale = layout_scale(painted_dpi_);
  return RECT{static_cast<LONG>(std::lround(painted_card_->left * scale)),
              static_cast<LONG>(std::lround(painted_card_->top * scale)),
              static_cast<LONG>(std::lround(painted_card_->right * scale)),
              static_cast<LONG>(std::lround(painted_card_->bottom * scale))};
}
std::string CandidateWindow::painted_backdrop() const {
  auto label = candidate_backdrop_label(painted_backdrop_, device_.BackdropWallpaperAvailable());
  // 建不起材质时带上失败在哪一步（DeviceResources::BackdropFailure），截图日志里看得出原因。
  if (painted_backdrop_.state == CandidateBackdropState::unavailable)
    if (const auto failure = device_.BackdropFailure(); !failure.empty())
      label += ": " + failure;
  return label;
}
CandidateBackdropPlan CandidateWindow::backdrop_plan() const {
  CandidateBackdropSystem system;
  system.composition_unavailable = backdrop_unavailable_;
  // 只有要材质的皮肤才去读系统开关，其余皮肤每帧不多读一次注册表。
  if (skin_layout_.backdrop != CandidateSkinBackdrop::none && !skin_layout_.panel_image) {
    system.transparency_enabled = system_transparency_enabled();
    system.high_contrast = system_high_contrast();
  }
  return candidate_backdrop_plan(skin_layout_.backdrop, skin_layout_.panel_image, palette_, system);
}
bool CandidateWindow::open_expansion() {
  if (!expand_.expanded)
    toggle_expansion();
  return expand_.expanded;
}
void CandidateWindow::collapse_expansion() {
  if (!expand_.expanded)
    return;
  const auto lease = expand_lease_;
  candidate_expand_collapse(expand_);
  expand_lease_.reset();
  expand_viewport_ = 0.0;
  if (expansion_ && lease)
    expansion_(*lease, false);
}
void CandidateWindow::reveal_if_near_end() {
  if (!painted_expand_ ||
      !candidate_expand_wants_more(expand_, painted_expand_->max_offset(), painted_expand_step_,
                                   painted_grid_.available) ||
      !candidate_expand_reveal(expand_, painted_grid_.available))
    return;
  shown_.reset();
  InvalidateRect(window_, nullptr, FALSE);
}
void CandidateWindow::commit_cell(size_t slot) {
  if (!click_ || !painted_ || !painted_->pointer_input || !painted_grid_.items ||
      slot >= painted_grid_.count || !IsWindowVisible(window_))
    return;
  const auto &item = (*painted_grid_.items)[painted_grid_.start + slot];
  CandidateClick click{painted_->lease, painted_->session, painted_->generation, item.index};
  click.expanded_text = item.text;
  click_(click);
}
std::pair<double, double> CandidateWindow::card_point(int x, int y) const {
  const double scale = layout_scale(painted_dpi_);
  return {x / scale - shadow_insets_.left,
          y / scale - shadow_insets_.top - static_cast<double>(decoration_offset_)};
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
  // 卷轴展开时首行在滚动区里：只有看得见的那一段点得到。
  std::optional<size_t> row;
  if (painted_expand_) {
    const auto part = candidate_expand_hit(*painted_expand_, painted_expand_offset_, card_x, card_y);
    if (part.part == CandidateExpandPart::row)
      row = part.index;
  } else {
    row = candidate_card_hit(card_x, card_y, card_width, card_height, painted_rows_);
  }
  if (!row || *row >= painted_->candidates.size())
    return std::nullopt;
  const auto &candidate = painted_->candidates[*row];
  CandidateClick click{painted_->lease, candidate.session,
                       candidate.generation, candidate.index};
  click.translation_page = painted_->translation_page;
  return click;
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
  // 本页第几个候选：「删除」行据此提示 Ctrl+Shift+Alt+N。
  const auto slot = static_cast<size_t>(candidate - painted_->candidates.begin());
  flyout_->open(screen.x, screen.y, code_points, candidate->actions_available,
                candidate->fixed_position, slot);
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
        const auto route = candidate_wheel_route(
            self->expand_.expanded && self->painted_expand_.has_value(), self->mouse_wheel_);
        if (route == CandidateWheelRoute::scroll) {
          // 展开着：首行与网格一起滚，不翻页，也不看「鼠标滚轮翻页」开关；滚到顶或底就停住。
          self->wheel_accumulator_ = 0;
          if (!IsWindowVisible(window))
            return 0;
          if (candidate_expand_scroll(self->expand_, GET_WHEEL_DELTA_WPARAM(wparam), WHEEL_DELTA,
                                      self->painted_expand_step_,
                                      self->painted_expand_->max_offset()))
            InvalidateRect(window, nullptr, FALSE);
          self->reveal_if_near_end();
          return 0;
        }
        if (route == CandidateWheelRoute::pass) {
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
        const auto wheel = [&](bool previous, int count) {
          CandidatePage page{value.lease, value.session, value.generation,
                             previous, static_cast<unsigned>(count), true};
          page.translation_page = value.translation_page;
          self->page_(page);
        };
        if (steps.page_up > 0)
          wheel(true, steps.page_up);
        if (steps.page_down > 0)
          wheel(false, steps.page_down);
        return 0;
      }
      case WM_LBUTTONDOWN:
        // 展开区：按钮、滚动条和网格先认；落在首行上的照常走下面的候选点击。
        if (self->painted_expand_ && self->painted_ && self->painted_->pointer_input &&
            IsWindowVisible(window)) {
          const auto [card_x, card_y] = self->card_point(static_cast<short>(LOWORD(lparam)),
                                                         static_cast<short>(HIWORD(lparam)));
          const auto &layout = *self->painted_expand_;
          const auto part =
              candidate_expand_hit(layout, self->painted_expand_offset_, card_x, card_y);
          bool captured = false;
          if (part.part == CandidateExpandPart::toggle) {
            self->pressed_toggle_ = captured = true;
          } else if (part.part == CandidateExpandPart::scrollbar) {
            if (const auto thumb = candidate_expand_thumb(layout, self->expand_.offset)) {
              // 在轨道上滑块之外按下：滑块中心先跳到那里，再接着拖。
              double grab = card_y - thumb->top;
              if (card_y < thumb->top || card_y >= thumb->bottom) {
                grab = (thumb->bottom - thumb->top) * 0.5;
                self->expand_.offset = candidate_expand_drag(layout, card_y, grab);
              }
              self->thumb_grab_ = grab;
              captured = true;
            }
          } else if (part.part == CandidateExpandPart::cell && part.index < self->painted_grid_.count) {
            self->pressed_cell_ = part.index;
            captured = true;
          }
          if (captured) {
            InvalidateRect(window, nullptr, FALSE);
            SetCapture(window);
            TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
            TrackMouseEvent(&track);
            return 0;
          }
        }
        self->pressed_page_ = self->pager_hit(static_cast<short>(LOWORD(lparam)),
                                              static_cast<short>(HIWORD(lparam)));
        self->pressed_ = self->pressed_page_
                             ? std::nullopt
                             : self->hit(static_cast<short>(LOWORD(lparam)),
                                         static_cast<short>(HIWORD(lparam)));
        // 列表里的翻页箭头按下时铺按下底色，要重画。
        if (self->pressed_page_ && self->painted_pager_ && self->painted_pager_->in_list)
          InvalidateRect(window, nullptr, FALSE);
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
        // 展开区的悬停：按钮、网格里的候选和滚动条；拖滚动条时滚动跟着指针。
        bool expand_hand = false;
        if (self->painted_expand_ && self->painted_ && self->painted_->pointer_input) {
          const auto [card_x, card_y] = self->card_point(static_cast<short>(LOWORD(lparam)),
                                                         static_cast<short>(HIWORD(lparam)));
          const auto &layout = *self->painted_expand_;
          if (self->thumb_grab_ && (wparam & MK_LBUTTON)) {
            const double offset = candidate_expand_drag(layout, card_y, *self->thumb_grab_);
            if (offset != self->expand_.offset) {
              self->expand_.offset = offset;
              InvalidateRect(window, nullptr, FALSE);
              self->reveal_if_near_end();
            }
          }
          const auto part =
              candidate_expand_hit(layout, self->painted_expand_offset_, card_x, card_y);
          const bool toggle = part.part == CandidateExpandPart::toggle;
          const bool scrollbar = part.part == CandidateExpandPart::scrollbar;
          const auto cell = part.part == CandidateExpandPart::cell && part.index < self->painted_grid_.count
                                ? std::optional<size_t>(part.index)
                                : std::nullopt;
          if (toggle != self->hovered_toggle_ || scrollbar != self->hovered_scrollbar_ ||
              cell != self->hovered_cell_) {
            self->hovered_toggle_ = toggle;
            self->hovered_scrollbar_ = scrollbar;
            self->hovered_cell_ = cell;
            InvalidateRect(window, nullptr, FALSE);
          }
          expand_hand = toggle || cell.has_value();
        }
        SetCursor(LoadCursorW(nullptr, click || arrow || expand_hand ? wide_cursor(IDC_HAND)
                                                                     : wide_cursor(IDC_ARROW)));
        std::optional<size_t> hovered;
        if (click && self->painted_) {
          for (size_t i = 0; i < self->painted_->candidates.size(); ++i)
            if (self->painted_->candidates[i].index == click->index) hovered = i;
        }
        if (hovered != self->hovered_) { self->hovered_ = hovered; InvalidateRect(window, nullptr, FALSE); }
        // 只有列表里的翻页箭头画悬停底色；首行的翻页照旧不画。
        const auto hovered_page =
            self->painted_pager_ && self->painted_pager_->in_list ? arrow : std::nullopt;
        if (hovered_page != self->hovered_page_) {
          self->hovered_page_ = hovered_page;
          InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
      }
      case WM_LBUTTONUP: {
        // Take the press before releasing capture. ReleaseCapture delivers
        // WM_CAPTURECHANGED to this window, and that is the cancellation path -
        // it clears pressed_. Releasing first therefore destroys the very press
        // this handler is about to read, and the click never reaches click_.
        const auto pressed = self->pressed_;
        const auto pressed_page = self->pressed_page_;
        const bool pressed_toggle = self->pressed_toggle_;
        const auto pressed_cell = self->pressed_cell_;
        const bool dragging = self->thumb_grab_.has_value();
        self->pressed_.reset();
        self->pressed_page_.reset();
        self->pressed_toggle_ = false;
        self->pressed_cell_.reset();
        self->thumb_grab_.reset();
        if (GetCapture() == window) ReleaseCapture();
        // 展开区：在按钮上按下又松开才展开或收起；在网格的同一个候选上按下又松开才上屏。
        if (pressed_toggle || pressed_cell || dragging) {
          InvalidateRect(window, nullptr, FALSE);
          if (self->painted_expand_) {
            const auto [card_x, card_y] = self->card_point(static_cast<short>(LOWORD(lparam)),
                                                           static_cast<short>(HIWORD(lparam)));
            const auto part = candidate_expand_hit(*self->painted_expand_,
                                                   self->painted_expand_offset_, card_x, card_y);
            if (pressed_toggle && part.part == CandidateExpandPart::toggle)
              self->toggle_expansion();
            else if (pressed_cell && part.part == CandidateExpandPart::cell &&
                     part.index == *pressed_cell)
              self->commit_cell(*pressed_cell);
          }
          return 0;
        }
        if (pressed_page) {
          // 收起按下底色；翻了页也会重画，没翻页（松开时已离开箭头）也要重画。
          if (self->painted_pager_ && self->painted_pager_->in_list)
            InvalidateRect(window, nullptr, FALSE);
          // One page per click, in the direction of the arrow the press started on. The painted page's identity goes with it, so the Server refuses a click on a page that has since changed.
          if (self->pager_hit(static_cast<short>(LOWORD(lparam)),
                              static_cast<short>(HIWORD(lparam))) ==
              pressed_page) {
            const auto &value = *self->painted_;
            CandidatePage page{value.lease, value.session, value.generation,
                               *pressed_page, 1u};
            page.translation_page = value.translation_page;
            self->page_(page);
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
        // 拖滚动条时指针可以离开卡片，捕获还在就接着拖，只收起悬停。
        if (message == WM_MOUSELEAVE && self->thumb_grab_ && GetCapture() == window) {
          self->hovered_cell_.reset();
          self->hovered_toggle_ = false;
          InvalidateRect(window, nullptr, FALSE);
          return 0;
        }
        if (GetCapture() == window) ReleaseCapture();
        if (self->hovered_page_ || self->pressed_page_ || self->hovered_cell_ ||
            self->pressed_cell_ || self->hovered_toggle_ || self->pressed_toggle_ ||
            self->hovered_scrollbar_ || self->thumb_grab_)
          InvalidateRect(window, nullptr, FALSE);
        self->pressed_.reset();
        self->pressed_page_.reset();
        self->hovered_.reset();
        self->hovered_page_.reset();
        self->hovered_cell_.reset();
        self->pressed_cell_.reset();
        self->hovered_toggle_ = self->pressed_toggle_ = self->hovered_scrollbar_ = false;
        self->thumb_grab_.reset();
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
        if (wparam == PBT_APMSUSPEND) {
          self->image_suspended_ = true;
          self->image_frames_.stop();
          KillTimer(window, image_frame_timer);
          return TRUE;
        }
        if (wparam != PBT_APMRESUMEAUTOMATIC &&
            wparam != PBT_APMRESUMECRITICAL && wparam != PBT_APMRESUMESUSPEND)
          break;
        self->image_suspended_ = false;
        self->image_epoch_ = 0;
        [[fallthrough]];
      case WM_DISPLAYCHANGE:
      case WM_DWMCOMPOSITIONCHANGED:
      case WM_DPICHANGED:
        // Composition surfaces keep both the display device and the DPI they
        // were created with. A topology change, DWM restart or system resume
        // can invalidate that device, while a high-to-low DPI move can leave
        // the old surface large enough to bypass EnsureForComposition's size
        // rebuild. Recreate it before the next frame in every case.
        self->image_frames_.reset_target();
        self->device_.DiscardTarget();
        // 反应式锁存的触发之一：游戏刚弹出候选窗就改了显示模式。
        if (message == WM_DISPLAYCHANGE)
          self->display_changed_ = true;
        [[fallthrough]];
      case WM_SETTINGCHANGE:
        KillTimer(window, image_frame_timer);
        self->image_frames_.stop();
        self->image_epoch_ = 0;
        if (IsWindowVisible(window))
          InvalidateRect(window, nullptr, FALSE);
        // The next refresh recomputes from the authenticated caret anchor;
        // do not recursively reposition from inside SetWindowPos's callback.
        self->shown_.reset();
        self->painted_.reset();
        self->pressed_.reset();
        self->pressed_page_.reset();
        self->hovered_.reset();
        self->hovered_page_.reset();
        return message == WM_POWERBROADCAST ? TRUE : 0;
      case WM_PAINT:
        self->paint();
        return 0;
      case typing_effect_message:
        self->take_typing_effect();
        return 0;
      case WM_TIMER:
        if (wparam == image_frame_timer) {
          KillTimer(window, image_frame_timer);
          if (IsWindowVisible(window) && !self->image_suspended_)
            InvalidateRect(window, nullptr, FALSE);
          return 0;
        }
        if (wparam == skin_animation_timer) {
          self->skin_animation_tick();
          return 0;
        }
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
