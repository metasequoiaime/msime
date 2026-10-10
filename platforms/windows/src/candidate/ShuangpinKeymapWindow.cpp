#include "ShuangpinKeymapWindow.h"
#include "CandidateWindow.h"
#include "ToolbarCoordinates.h"
#include "ToolbarTooltips.h"
#include "WindowShadow.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include <cmath>
#include <iterator>
#include <memory>
#include <stdexcept>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.ShuangpinKeymap" MSIME_EDITION_NAME_SUFFIX;
// 和候选窗一样只在这次界面操作里切到每显示器 DPI v2，结束时恢复调用方的线程设置；窗口坐标都是物理像素，和候选窗、TSF 报来的锚点一致。
struct DpiScope {
  DPI_AWARENESS_CONTEXT previous;
  DpiScope()
      : previous(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)) {
    if (!previous)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~DpiScope() { SetThreadDpiAwarenessContext(previous); }
};
long scaled(double value, double unit) { return static_cast<long>(std::lround(value * unit)); }
std::wstring wide(const std::string &text) {
  if (text.empty())
    return {};
  const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                        static_cast<int>(text.size()), nullptr, 0);
  if (count <= 0)
    return {};
  std::wstring result(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()),
                          result.data(), count) != count)
    return {};
  return result;
}
// 一张方案表：host-api 回答 {ok, value}，value 是字符串到字符串的对象；读不出来时当作空表，键帽上只剩键名。
nlohmann::json profile_table(char *(*query)(const uint8_t *, size_t), const std::string &profile) {
  auto raw = msime::host_api::own_string(
      query(reinterpret_cast<const uint8_t *>(profile.data()), profile.size()));
  if (!raw)
    return nlohmann::json::object();
  const auto envelope = nlohmann::json::parse(raw.get(), nullptr, false);
  if (!envelope.is_object() || !envelope.value("ok", false))
    return nlohmann::json::object();
  const auto value = envelope.find("value");
  return value != envelope.end() && value->is_object() ? *value : nlohmann::json::object();
}
} // namespace

ShuangpinKeymapWindow::ShuangpinKeymapWindow() {
  DpiScope dpi_scope;
  WNDCLASSEXW type{};
  type.cbSize = sizeof(type);
  type.lpfnWndProc = procedure;
  type.hInstance = GetModuleHandleW(nullptr);
  type.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
  type.lpszClassName = kClassName;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Shuangpin keymap class unavailable");
  // 不激活、不进任务栏和 Alt+Tab、置顶；WS_EX_TRANSPARENT 让鼠标点击穿过去，和 macOS 面板的 ignoresMouseEvents 一样只给人看。没有重定向位图：卡片按像素带透明度合成，圆角和阴影外面是透明的。
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_TRANSPARENT |
                                WS_EX_NOREDIRECTIONBITMAP,
                            kClassName, L"MSIME shuangpin keymap", WS_POPUP, 0, 0, 1, 1, nullptr, nullptr,
                            type.hInstance, this);
  if (!window_)
    throw std::runtime_error("Shuangpin keymap window unavailable");
}
ShuangpinKeymapWindow::~ShuangpinKeymapWindow() {
  hide();
  if (window_)
    DestroyWindow(window_);
}
void ShuangpinKeymapWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  if (window_ && IsWindowVisible(window_))
    InvalidateRect(window_, nullptr, FALSE);
}
void ShuangpinKeymapWindow::hide() {
  if (window_ && IsWindowVisible(window_))
    ShowWindow(window_, SW_HIDE);
  placed_ = RECT{};
}
void ShuangpinKeymapWindow::fail(ComponentFailureSite site) {
  if (!failed_)
    failure_site_ = site;
  failed_ = true;
  hide();
}
void ShuangpinKeymapWindow::load_profile(const std::string &profile) {
  if (profile == profile_ && !title_.empty())
    return;
  profile_ = profile;
  rows_ = shuangpin_keymap_rows(profile_table(msime_client_shuangpin_key_hints, profile_));
  zero_initials_ = wide(shuangpin_keymap_zero_initial_text(profile_table(msime_client_shuangpin_zero_initials, profile_)));
  title_ = toolbar_scheme_title("shuangpin", profile_, "") + L"键位";
}
void ShuangpinKeymapWindow::update(const std::optional<ShuangpinKeymapFrame> &frame,
                                   const CandidateWindow &candidates) {
  if (failed_ || !window_)
    return;
  const HWND candidate = candidates.handle();
  if (!frame || !candidate || !IsWindowVisible(candidate)) {
    hide();
    return;
  }
  try {
    DpiScope dpi_scope;
    // 候选窗的窗口外框四周是透明的阴影边距（上 20、下 40、左 32 DIP），顶上还可能有吉祥物那一条；按外框摆，键位图会以为候选窗翻到了光标上方、盖住正在输入的那几行，还会向左错开、和卡片隔出一大段空。所以取卡片本身的矩形。
    const auto visible_card = candidates.card_on_screen();
    if (!visible_card) {
      hide();
      return;
    }
    const RECT card{std::lround(visible_card->left), std::lround(visible_card->top),
                    std::lround(visible_card->right), std::lround(visible_card->bottom)};
    const HMONITOR monitor = MonitorFromRect(&card, MONITOR_DEFAULTTONEAREST);
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(monitor, &info))
      throw std::runtime_error("Shuangpin keymap monitor unavailable");
    const std::string profile = frame->hint.profile;
    const bool content_changed = profile != profile_ || frame->hint.key != highlighted_;
    load_profile(profile);
    highlighted_ = frame->hint.key;
    // 尺寸按候选窗所在显示器的 DPI 算，和候选窗一起跨屏时也一致。
    const double unit = toolbar_pixel_unit(GetDpiForWindow(candidate), 1.0);
    ShuangpinKeymapPlacementInput placement;
    placement.candidate = {card.left, card.top, card.right, card.bottom};
    placement.anchor_x = frame->anchor_x;
    placement.anchor_y = frame->anchor_y;
    placement.work = {info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom};
    placement.card_width = scaled(metrics_.width, unit);
    placement.card_height = scaled(metrics_.height, unit);
    placement.shadow_left = scaled(metrics_.shadow.left, unit);
    placement.shadow_top = scaled(metrics_.shadow.top, unit);
    placement.gap = scaled(8.0, unit);
    placement.margin = scaled(16.0, unit);
    placement.line_height = scaled(24.0, unit);
    const auto placed = shuangpin_keymap_placement(placement);
    const RECT next{placed.x, placed.y, placed.x + static_cast<long>(std::ceil(metrics_.window_width() * unit)),
                    placed.y + static_cast<long>(std::ceil(metrics_.window_height() * unit))};
    if (!IsWindowVisible(window_) || !EqualRect(&next, &placed_)) {
      if (!SetWindowPos(window_, HWND_TOPMOST, next.left, next.top, next.right - next.left, next.bottom - next.top,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW))
        throw std::runtime_error("Shuangpin keymap positioning failed");
      placed_ = next;
      InvalidateRect(window_, nullptr, FALSE);
    } else if (content_changed) {
      InvalidateRect(window_, nullptr, FALSE);
    }
  } catch (...) {
    fail(failure_at_stage("update", static_cast<uint32_t>(GetLastError())));
  }
}
void ShuangpinKeymapWindow::paint() {
  PAINTSTRUCT state{};
  const HDC dc = BeginPaint(window_, &state);
  if (!dc)
    return;
  struct End {
    HWND window;
    PAINTSTRUCT &state;
    ~End() { EndPaint(window, &state); }
  } end{window_, state};
  if (title_.empty())
    return;
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Shuangpin keymap device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Shuangpin keymap render target unavailable");
  auto brush = [&](const CandidateColor &color) {
    auto *created = device_.GetSolidColorBrush(D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!created)
      throw std::runtime_error("Shuangpin keymap brush unavailable");
    return created;
  };
  auto format = [&](const wchar_t *family, float size, DWRITE_FONT_WEIGHT weight, DWRITE_TEXT_ALIGNMENT alignment) {
    auto *created = device_.GetTextFormat(family, size, weight, alignment, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                                          DWRITE_WORD_WRAPPING_NO_WRAP);
    if (!created)
      throw std::runtime_error("Shuangpin keymap text format unavailable");
    return created;
  };
  const float left = static_cast<float>(metrics_.shadow.left);
  const float top = static_cast<float>(metrics_.shadow.top);
  const D2D1_RECT_F card{left, top, left + static_cast<float>(metrics_.width),
                         top + static_cast<float>(metrics_.height)};
  target->BeginDraw();
  target->Clear(D2D1::ColorF(0, 0.0f));
  draw_window_shadow(target, card, palette_.radius, static_cast<float>(metrics_.shadow.scale));
  const float inset = palette_.border_width / 2.0f;
  const D2D1_ROUNDED_RECT body{{card.left + inset, card.top + inset, card.right - inset, card.bottom - inset},
                               palette_.radius, palette_.radius};
  target->FillRoundedRectangle(body, brush(palette_.surface));
  target->DrawRoundedRectangle(body, brush(palette_.border), palette_.border_width);

  // 标题行：左边方案名加「键位」，右边一句说明，和 macOS 面板的页眉相同。
  const float side = static_cast<float>(metrics_.inset);
  const D2D1_RECT_F header{card.left + side, card.top + static_cast<float>(metrics_.header_top), card.right - side,
                           card.top + static_cast<float>(metrics_.header_top + metrics_.header_height)};
  target->DrawText(title_.c_str(), static_cast<UINT32>(title_.size()),
                   format(L"Segoe UI", 12.0f, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING), header,
                   brush(palette_.text));
  static constexpr wchar_t hint[] = L"当前按键会高亮 · 上屏后自动隐藏";
  target->DrawText(hint, static_cast<UINT32>(std::size(hint) - 1),
                   format(L"Segoe UI", 10.0f, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_TRAILING), header,
                   brush(palette_.number));

  auto *key_format = format(L"Consolas", 11.0f, DWRITE_FONT_WEIGHT_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING);
  auto *codes_format = format(L"Segoe UI", 9.0f, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_CENTER);
  // 未高亮的键帽是卡面上一层浅色，描一圈正文色的细边；高亮键填强调色，字按强调色的明暗取黑或白。
  auto key_fill = palette_.hover;
  auto key_border = palette_.text;
  key_border.a = 0.16f;
  auto on_accent = shuangpin_keymap_on_accent(palette_.accent);
  auto on_accent_secondary = on_accent;
  on_accent_secondary.a = 0.86f;
  const float key_radius = static_cast<float>(metrics_.key_radius);
  for (size_t row = 0; row < rows_.size(); ++row) {
    for (size_t index = 0; index < rows_[row].size(); ++index) {
      const auto &key = rows_[row][index];
      const auto rect = shuangpin_keymap_key_rect(metrics_, row, index, rows_[row].size());
      const D2D1_RECT_F bounds{card.left + static_cast<float>(rect.left) + 0.5f,
                               card.top + static_cast<float>(rect.top) + 0.5f,
                               card.left + static_cast<float>(rect.right) - 0.5f,
                               card.top + static_cast<float>(rect.bottom) - 0.5f};
      const bool highlighted = highlighted_ != 0 && key.key.size() == 1 && key.key[0] == highlighted_;
      const D2D1_ROUNDED_RECT shape{bounds, key_radius, key_radius};
      target->FillRoundedRectangle(shape, brush(highlighted ? palette_.accent : key_fill));
      target->DrawRoundedRectangle(shape, brush(highlighted ? CandidateColor{1.0f, 1.0f, 1.0f, 0.28f} : key_border),
                                   1.0f);
      const auto name = wide(key.key);
      target->DrawText(name.c_str(), static_cast<UINT32>(name.size()), key_format,
                       D2D1_RECT_F{bounds.left + 7.0f, bounds.top + 3.0f, bounds.right, bounds.top + 17.0f},
                       brush(highlighted ? on_accent : palette_.text));
      const auto codes = wide(key.codes);
      if (!codes.empty())
        target->DrawText(codes.c_str(), static_cast<UINT32>(codes.size()), codes_format,
                         D2D1_RECT_F{bounds.left, bounds.bottom - 17.0f, bounds.right, bounds.bottom - 3.0f},
                         brush(highlighted ? on_accent_secondary : palette_.number));
    }
  }

  // 底部的零声母说明，居中一行。
  const D2D1_RECT_F footer{card.left + side,
                           card.bottom - static_cast<float>(metrics_.footer_bottom + metrics_.footer_height),
                           card.right - side, card.bottom - static_cast<float>(metrics_.footer_bottom)};
  target->DrawText(zero_initials_.c_str(), static_cast<UINT32>(zero_initials_.size()),
                   format(L"Segoe UI", 10.0f, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER), footer,
                   brush(palette_.number));
  const HRESULT drawn = target->EndDraw();
  if (SUCCEEDED(drawn) && FAILED(device_.Present()))
    throw std::runtime_error("Shuangpin keymap presentation failed");
  // 有符号的 HRESULT 和某些 SDK、MinGW 里无符号的宏比较，见 CandidateWindow.cpp 的同一处。
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    device_.DiscardTarget();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Shuangpin keymap drawing failed");
}
LRESULT CALLBACK ShuangpinKeymapWindow::procedure(HWND window, UINT message, WPARAM w, LPARAM l) noexcept {
  auto *self = reinterpret_cast<ShuangpinKeymapWindow *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<ShuangpinKeymapWindow *>(reinterpret_cast<CREATESTRUCTW *>(l)->lpCreateParams);
    self->window_ = window;
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
  }
  if (!self || self->failed_)
    return DefWindowProcW(window, message, w, l);
  try {
    switch (message) {
    case WM_MOUSEACTIVATE:
      return MA_NOACTIVATE;
    case WM_NCHITTEST:
      return HTTRANSPARENT;
    case WM_ERASEBKGND:
      return 1;
    case WM_DPICHANGED:
      // 跟着候选窗挪到另一块 DPI 不同的显示器：尺寸已经按候选窗所在显示器的 DPI 算好，不采用系统建议的矩形；丢掉旧的合成表面，下次绘制按新的 DPI 重建。
      self->device_.DiscardTarget();
      InvalidateRect(window, nullptr, FALSE);
      return 0;
    case WM_DISPLAYCHANGE:
    case WM_SETTINGCHANGE:
    case WM_DWMCOMPOSITIONCHANGED:
      // 换了显示器设置：丢掉旧的合成表面并收起，下一轮主循环按新的环境重新摆放。
      self->device_.DiscardTarget();
      self->hide();
      return 0;
    case WM_PAINT:
      self->paint();
      return 0;
    }
  } catch (...) {
    self->fail(failure_in_message(message, static_cast<uint32_t>(GetLastError())));
    return 0;
  }
  return DefWindowProcW(window, message, w, l);
}
} // namespace msime::windows
