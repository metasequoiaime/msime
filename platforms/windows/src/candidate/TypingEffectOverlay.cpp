#include "TypingEffectOverlay.h"
#include "FullscreenForeground.h"
#include "InputModeHudWindow.h"
#include <algorithm>
#include <chrono>
#include <cmath>
#include <stdexcept>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.TypingEffect" MSIME_EDITION_NAME_SUFFIX;
// 动画进行时按帧重画；动画都停了、只剩静止的徽标时换成一次性的收起计时器。
constexpr UINT_PTR kFrameTimer = 0x4701;
constexpr UINT_PTR kSettleTimer = 0x4702;
// 和候选窗一样只在这次界面操作里切到每显示器 DPI v2，结束时恢复调用方的线程设置；窗口在整个生命期里保持 v2，坐标都是物理像素，和 TSF 报来的光标锚点一致。
struct DpiScope {
  DPI_AWARENESS_CONTEXT previous;
  DpiScope() : previous(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)) {
    if (!previous)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~DpiScope() { SetThreadDpiAwarenessContext(previous); }
};
D2D1_COLOR_F rgb(uint32_t value) {
  return D2D1::ColorF(static_cast<float>((value >> 16) & 0xFF) / 255.0f, static_cast<float>((value >> 8) & 0xFF) / 255.0f,
                      static_cast<float>(value & 0xFF) / 255.0f, 1.0f);
}
D2D1_RECT_F local(const TypingRect &rect, const TypingRect &frame) {
  return D2D1::RectF(rect.left - frame.left, rect.top - frame.top, rect.right - frame.left, rect.bottom - frame.top);
}
} // namespace

TypingEffectOverlay::TypingEffectOverlay() {
  DpiScope dpi_scope;
  WNDCLASSEXW type{};
  type.cbSize = sizeof(type);
  type.lpfnWndProc = procedure;
  type.hInstance = GetModuleHandleW(nullptr);
  type.lpszClassName = kClassName;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Typing effect class unavailable");
  // 工厂和预留都在建窗口之前：构造函数在建好窗口之后抛出时析构函数不会运行，窗口会留下来，GWLP_USERDATA 指着已经不存在的对象，下一条广播（WM_DISPLAYCHANGE）就会访问它。建窗口是最后一步。
  if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory_.GetAddressOf())))
    throw std::runtime_error("Typing effect Direct2D unavailable");
  if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                                 reinterpret_cast<IUnknown **>(write_factory_.GetAddressOf()))))
    throw std::runtime_error("Typing effect DirectWrite unavailable");
  // 一次预留够，画帧和发射火花时都不再扩容。
  sparks_.reserve(typing_max_sparks);
  // 不激活、不进任务栏和 Alt+Tab、置顶。分层窗口加 WS_EX_TRANSPARENT 才真正让点击穿到下面的应用，和 macOS 面板的 ignoresMouseEvents 一样只给人看。
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                            kClassName, L"MSIME typing effect", WS_POPUP, 0, 0, 1, 1, nullptr, nullptr, type.hInstance,
                            this);
  if (!window_)
    throw std::runtime_error("Typing effect window unavailable");
}
TypingEffectOverlay::~TypingEffectOverlay() {
  if (window_) {
    KillTimer(window_, kFrameTimer);
    KillTimer(window_, kSettleTimer);
  }
  brush_.Reset();
  target_.Reset();
  discard_surface();
  if (window_)
    DestroyWindow(window_);
}
uint64_t TypingEffectOverlay::now_millis() {
  return static_cast<uint64_t>(
      std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now().time_since_epoch()).count());
}
bool TypingEffectOverlay::power_saver() {
  SYSTEM_POWER_STATUS status{};
  return GetSystemPowerStatus(&status) && status.SystemStatusFlag == 1;
}
std::optional<TypingRect> TypingEffectOverlay::caret_rect(HWND foreground, const std::optional<POINT> &anchor) {
  if (const auto caret = InputModeHudWindow::system_caret(foreground))
    return TypingRect{static_cast<float>(caret->left), static_cast<float>(caret->top), static_cast<float>(caret->right),
                      static_cast<float>(caret->bottom)};
  // 没有系统光标的应用：用最近一次组字的锚点，它是文字底边的左端，往上估一行 20 个设备无关像素，和中英文提示的估法相同。
  if (!anchor)
    return std::nullopt;
  const float line = static_cast<float>(MulDiv(20, static_cast<int>(GetDpiForSystem()), USER_DEFAULT_SCREEN_DPI));
  const float x = static_cast<float>(anchor->x);
  const float y = static_cast<float>(anchor->y);
  return TypingRect{x, y - line, x + 1.0f, y};
}
void TypingEffectOverlay::fail(ComponentFailureSite site) {
  if (!failed_)
    failure_site_ = site;
  failed_ = true;
  settle();
}
void TypingEffectOverlay::settle() {
  if (!window_)
    return;
  KillTimer(window_, kFrameTimer);
  KillTimer(window_, kSettleTimer);
  ticking_ = false;
  sparks_.clear();
  caret_flash_.reset();
  badge_.reset();
  badge_text_.clear();
  bounce_ = TypingBadgeBounce::none;
  if (shown_)
    ShowWindow(window_, SW_HIDE);
  shown_ = false;
}
IDWriteTextFormat *TypingEffectOverlay::badge_format() {
  const float size = typing_badge_font_size * scale_;
  if (badge_format_ && badge_format_size_ == size)
    return badge_format_.Get();
  badge_format_.Reset();
  // 徽标的数字和 macOS 一样用半粗体；目标的单位是物理像素，所以字号乘上缩放。
  if (FAILED(write_factory_->CreateTextFormat(L"Microsoft YaHei UI", nullptr, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                                              DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, L"zh-cn",
                                              badge_format_.GetAddressOf())))
    throw std::runtime_error("Typing effect text format unavailable");
  badge_format_->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
  badge_format_->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
  badge_format_->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
  badge_format_size_ = size;
  return badge_format_.Get();
}
// 徽标文字的宽度，设备无关像素，加上两边的留白。
float TypingEffectOverlay::badge_text_width(const std::wstring &text) {
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  DWRITE_TEXT_METRICS metrics{};
  if (FAILED(write_factory_->CreateTextLayout(text.c_str(), static_cast<UINT32>(text.size()), badge_format(), 4096.0f,
                                              typing_badge_height * scale_, layout.GetAddressOf())) ||
      FAILED(layout->GetMetrics(&metrics)))
    return static_cast<float>(text.size()) * typing_badge_font_size + 2.0f * typing_badge_inset;
  return metrics.widthIncludingTrailingWhitespace / scale_ + 2.0f * typing_badge_inset;
}
void TypingEffectOverlay::discard_surface() {
  target_.Reset();
  brush_.Reset();
  if (surface_dc_) {
    if (surface_previous_)
      SelectObject(surface_dc_, surface_previous_);
    DeleteDC(surface_dc_);
  }
  if (surface_bitmap_)
    DeleteObject(surface_bitmap_);
  surface_dc_ = nullptr;
  surface_bitmap_ = nullptr;
  surface_previous_ = nullptr;
  surface_width_ = 0;
  surface_height_ = 0;
  bound_width_ = 0;
  bound_height_ = 0;
}
bool TypingEffectOverlay::ensure_surface(int width, int height) {
  if (width <= 0 || height <= 0)
    return false;
  if (width > surface_width_ || height > surface_height_) {
    // 只增不减：光标挪来挪去时窗口大小在一个范围里变，不必每次重建位图。
    const int next_width = (std::max)(width, surface_width_);
    const int next_height = (std::max)(height, surface_height_);
    discard_surface();
    // 自上而下、预乘透明度的 32 位位图，正是 UpdateLayeredWindow 的 ULW_ALPHA 和 Direct2D 的 PREMULTIPLIED 要的格式。
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = next_width;
    info.bmiHeader.biHeight = -next_height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    const HDC screen = GetDC(nullptr);
    if (!screen)
      return false;
    void *bits = nullptr;
    surface_bitmap_ = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    surface_dc_ = CreateCompatibleDC(screen);
    ReleaseDC(nullptr, screen);
    if (!surface_bitmap_ || !surface_dc_) {
      discard_surface();
      return false;
    }
    surface_previous_ = SelectObject(surface_dc_, surface_bitmap_);
    surface_width_ = next_width;
    surface_height_ = next_height;
  }
  if (!target_) {
    // 96 DPI：目标的单位就是物理像素，和窗口、光标的坐标同一个单位。
    const auto properties = D2D1::RenderTargetProperties(
        D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED),
        96.0f, 96.0f);
    if (FAILED(factory_->CreateDCRenderTarget(&properties, target_.GetAddressOf())))
      return false;
    bound_width_ = 0;
    bound_height_ = 0;
  }
  if (bound_width_ != width || bound_height_ != height) {
    const RECT bind{0, 0, width, height};
    if (FAILED(target_->BindDC(surface_dc_, &bind)))
      return false;
    bound_width_ = width;
    bound_height_ = height;
  }
  if (!brush_ && FAILED(target_->CreateSolidColorBrush(D2D1::ColorF(1.0f, 1.0f, 1.0f, 1.0f), brush_.GetAddressOf())))
    return false;
  return true;
}
bool TypingEffectOverlay::present(const TypingEffectPresentation &presentation) {
  if (failed_ || !window_)
    return false;
  try {
    DpiScope dpi_scope;
    const auto &effect = presentation.effect;
    const std::wstring combo = typing_effect_combo_text(effect.combo);
    if (effect.style == TypingEffectStyle::off && combo.empty()) {
      // 退格或停顿结束了连击：徽标立刻收起，不留过时的数字。
      if (badge_)
        settle();
      return true;
    }
    const HWND foreground = GetForegroundWindow();
    // 全屏应用（游戏、视频、演示）上什么都不画，连击照样计数。
    if (foreground_is_fullscreen(foreground)) {
      settle();
      return true;
    }
    const auto caret = caret_rect(foreground, presentation.anchor);
    const bool card = presentation.card && typing_rect_usable(*presentation.card);
    if (!(caret && typing_rect_usable(*caret)) && !card)
      return true;
    const POINT probe = caret ? POINT{static_cast<LONG>(caret->left), static_cast<LONG>(caret->top)}
                              : POINT{static_cast<LONG>(presentation.card->left), static_cast<LONG>(presentation.card->top)};
    const HMONITOR monitor = MonitorFromPoint(probe, MONITOR_DEFAULTTONEAREST);
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(monitor, &info))
      throw std::runtime_error("Typing effect monitor unavailable");
    // 先挪到目标显示器上，窗口的 DPI 才是那块显示器的。还显示着时先收起：分层窗口挪动时带着上一帧的画面，不收起的话旧火花会在那块显示器的左上角闪一下；在另一块显示器上飞的火花本来也不再画。
    if (MonitorFromWindow(window_, MONITOR_DEFAULTTONULL) != monitor) {
      if (shown_)
        settle();
      if (!SetWindowPos(window_, nullptr, info.rcMonitor.left, info.rcMonitor.top, 0, 0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW))
        throw std::runtime_error("Typing effect monitor move failed");
    }
    scale_ = static_cast<float>(GetDpiForWindow(window_)) / static_cast<float>(USER_DEFAULT_SCREEN_DPI);
    if (!(scale_ > 0.0f))
      scale_ = 1.0f;

    TypingOverlayPlacementInput input;
    if (caret && typing_rect_usable(*caret))
      input.caret = *caret;
    if (card)
      input.card = *presentation.card;
    input.monitor = {static_cast<float>(info.rcMonitor.left), static_cast<float>(info.rcMonitor.top),
                     static_cast<float>(info.rcMonitor.right), static_cast<float>(info.rcMonitor.bottom)};
    input.scale = scale_;
    input.badge_width = combo.empty() ? 0.0f : badge_text_width(combo);
    const auto placement = typing_overlay_placement(input);
    if (!placement)
      return true;

    const uint64_t now = now_millis();
    prune_typing_sparks(sparks_, now);
    TypingRect frame = placement->frame;
    // 上一次的火花还在飞时把它们也框进去，光标跳得太远就放弃它们。
    if (shown_ && !sparks_.empty() && typing_overlay_keeps_previous(frame_, frame))
      frame = typing_rect_union(frame_, frame);
    else
      sparks_.clear();

    const TypingEffectStyle drawn = typing_effect_drawn_style(effect.style, presentation.animations, presentation.power_saver);
    const bool power = drawn == TypingEffectStyle::power_mode;
    const uint32_t intensity = presentation.settings.intensity;
    const uint32_t count = drawn == TypingEffectStyle::sparks || power
                               ? typing_spark_count(power, effect.commit, intensity, presentation.palette.particles)
                               : 0u;
    if (count)
      emit_typing_sparks(sparks_, count, placement->origin_x, placement->origin_y, power, effect.commit, now,
                         typing_spark_color(presentation.palette, effect.combo, presentation.accent), scale_, seed_);
    flash_color_ = typing_flash_color(presentation.palette, presentation.accent);
    if (!combo.empty() && placement->badge) {
      badge_ = placement->badge;
      badge_text_ = combo;
      bounce_ = typing_badge_bounce(effect.tier_up, power, presentation.animations);
      bounce_started_ = now;
    } else {
      badge_.reset();
      badge_text_.clear();
      bounce_ = TypingBadgeBounce::none;
    }
    // 候选窗不在屏幕上时（比如刚上屏），闪光改画在光标所在的行；卡片在时由卡片自己闪。「显示动画」关掉时不闪。
    const bool flash = (drawn == TypingEffectStyle::flash || power) && presentation.animations;
    const uint32_t flash_millis = typing_flash_duration(presentation.settings.flash_millis, effect.commit);
    if (flash && placement->caret_flash) {
      caret_flash_ = placement->caret_flash;
      caret_flash_started_ = now;
      caret_flash_millis_ = flash_millis;
      caret_flash_peak_ = typing_caret_flash_peak(effect.commit, intensity);
    } else {
      caret_flash_.reset();
    }
    if (sparks_.empty() && !badge_ && !caret_flash_) {
      settle();
      return true;
    }
    settle_at_ = now + (badge_ ? typing_badge_settle_millis : (std::max)(typing_spark_settle_millis, flash_millis));
    frame_ = frame;
    draw(now);
    if (failed_)
      return false;
    // 排在候选窗下面，火花不挡候选；候选窗不在时就是普通的置顶。
    const HWND below = presentation.candidate_window && IsWindowVisible(presentation.candidate_window)
                           ? presentation.candidate_window
                           : HWND_TOPMOST;
    SetWindowPos(window_, below, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW);
    shown_ = true;
    KillTimer(window_, kSettleTimer);
    if (!ticking_) {
      if (!SetTimer(window_, kFrameTimer, typing_overlay_frame_millis, nullptr))
        throw std::runtime_error("Typing effect timer unavailable");
      ticking_ = true;
    }
    return true;
  } catch (...) {
    fail(failure_at_stage("present", static_cast<uint32_t>(GetLastError())));
    return false;
  }
}
void TypingEffectOverlay::tick() {
  const uint64_t now = now_millis();
  if (now >= settle_at_) {
    settle();
    return;
  }
  prune_typing_sparks(sparks_, now);
  const bool flashing = caret_flash_ && now - caret_flash_started_ < caret_flash_millis_;
  const bool bouncing = bounce_ != TypingBadgeBounce::none && now - bounce_started_ < typing_badge_bounce_millis(bounce_);
  if (!flashing)
    caret_flash_.reset();
  if (!bouncing)
    bounce_ = TypingBadgeBounce::none;
  if (sparks_.empty() && !badge_ && !caret_flash_) {
    settle();
    return;
  }
  // 这一帧画完之后若不再有动的东西，只剩静止的徽标，就停掉帧计时器，等收起的那一刻。
  draw(now);
  if (!typing_sparks_alive(sparks_, now) && !flashing && !bouncing) {
    KillTimer(window_, kFrameTimer);
    ticking_ = false;
    const uint64_t remaining = settle_at_ > now ? settle_at_ - now : 1;
    if (!SetTimer(window_, kSettleTimer, static_cast<UINT>((std::min)(remaining, uint64_t{60000})), nullptr))
      throw std::runtime_error("Typing effect timer unavailable");
  }
}
void TypingEffectOverlay::draw(uint64_t now) {
  const int width = static_cast<int>(frame_.width());
  const int height = static_cast<int>(frame_.height());
  if (!ensure_surface(width, height))
    throw std::runtime_error("Typing effect surface unavailable");
  auto *brush = brush_.Get();
  target_->BeginDraw();
  target_->SetTransform(D2D1::Matrix3x2F::Identity());
  target_->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
  // 光标行的闪光：一条淡出的强调色。
  if (caret_flash_) {
    const float alpha = typing_caret_flash_alpha(caret_flash_peak_, static_cast<uint32_t>(now - caret_flash_started_),
                                                 caret_flash_millis_);
    if (alpha > 0.0f) {
      brush->SetColor(rgb(flash_color_));
      brush->SetOpacity(alpha);
      const float radius = 3.0f * scale_;
      target_->FillRoundedRectangle(D2D1::RoundedRect(local(*caret_flash_, frame_), radius, radius), brush);
    }
  }
  // 火花：16 的软圆点，外圈淡、里圈实，近似 macOS 那张径向渐变的贴图。
  for (const auto &spark : sparks_) {
    const auto at = typing_spark_at(spark, now, scale_);
    if (!at)
      continue;
    const D2D1_POINT_2F center = D2D1::Point2F(at->x - frame_.left, at->y - frame_.top);
    brush->SetColor(rgb(spark.color));
    brush->SetOpacity(at->alpha * 0.35f);
    target_->FillEllipse(D2D1::Ellipse(center, at->radius, at->radius), brush);
    brush->SetOpacity(at->alpha);
    target_->FillEllipse(D2D1::Ellipse(center, at->radius * 0.5f, at->radius * 0.5f), brush);
  }
  // 连击徽标：强调色的胶囊，白色半粗体的「连击 ×N」，弹跳是以中心为原点的缩放。
  if (badge_) {
    const D2D1_RECT_F rect = local(*badge_, frame_);
    const float bounce = bounce_ == TypingBadgeBounce::none
                             ? 1.0f
                             : typing_badge_bounce_scale(bounce_, static_cast<uint32_t>(now - bounce_started_));
    target_->SetTransform(D2D1::Matrix3x2F::Scale(
        bounce, bounce, D2D1::Point2F((rect.left + rect.right) / 2.0f, (rect.top + rect.bottom) / 2.0f)));
    const float radius = (rect.bottom - rect.top) / 2.0f;
    brush->SetColor(rgb(flash_color_));
    brush->SetOpacity(0.92f);
    target_->FillRoundedRectangle(D2D1::RoundedRect(rect, radius, radius), brush);
    brush->SetColor(D2D1::ColorF(1.0f, 1.0f, 1.0f, 1.0f));
    brush->SetOpacity(1.0f);
    target_->DrawText(badge_text_.c_str(), static_cast<UINT32>(badge_text_.size()), badge_format(), rect, brush,
                      D2D1_DRAW_TEXT_OPTIONS_CLIP);
    target_->SetTransform(D2D1::Matrix3x2F::Identity());
  }
  const HRESULT drawn = target_->EndDraw();
  // 有符号的 HRESULT 和某些 SDK、MinGW 里无符号的宏比较，见 CandidateWindow.cpp 的同一处。
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    // 设备丢了：下一帧重建目标和画刷，这一帧跳过。
    target_.Reset();
    brush_.Reset();
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Typing effect drawing failed");
  POINT destination{static_cast<LONG>(frame_.left), static_cast<LONG>(frame_.top)};
  SIZE size{width, height};
  POINT source{0, 0};
  BLENDFUNCTION blend{AC_SRC_OVER, 0, 255, AC_SRC_ALPHA};
  // 位置和内容一次更新，窗口不会先挪过去再换画面。
  if (!UpdateLayeredWindow(window_, nullptr, &destination, &size, surface_dc_, &source, 0, &blend, ULW_ALPHA))
    throw std::runtime_error("Typing effect presentation failed");
}
LRESULT CALLBACK TypingEffectOverlay::procedure(HWND window, UINT message, WPARAM w, LPARAM l) noexcept {
  auto *self = reinterpret_cast<TypingEffectOverlay *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<TypingEffectOverlay *>(reinterpret_cast<CREATESTRUCTW *>(l)->lpCreateParams);
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
    case WM_TIMER:
      if (w == kFrameTimer || w == kSettleTimer) {
        DpiScope dpi_scope;
        if (w == kSettleTimer)
          self->settle();
        else
          self->tick();
        return 0;
      }
      break;
    case WM_DISPLAYCHANGE:
    case WM_DPICHANGED:
    case WM_DWMCOMPOSITIONCHANGED:
      // 换了显示器设置或 DPI：正在播的这次直接收起，下次按当时的设置重新摆放。
      self->settle();
      return 0;
    case WM_NCDESTROY:
      self->window_ = nullptr;
      SetWindowLongPtrW(window, GWLP_USERDATA, 0);
      break;
    }
  } catch (...) {
    self->fail(failure_in_message(message, static_cast<uint32_t>(GetLastError())));
    return 0;
  }
  return DefWindowProcW(window, message, w, l);
}
} // namespace msime::windows
