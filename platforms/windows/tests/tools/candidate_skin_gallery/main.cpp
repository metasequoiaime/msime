// 候选皮肤截图工具：用 Server 的候选窗（CandidateWindow）把皮肤目录里的每款皮肤画在一页固定的假候选上，从屏幕截下来存成 PNG，没有 Windows 机器也能在 CI 的产物里看皮肤画得对不对。用法和 CI 里的跑法见 platforms/windows/README.md「候选皮肤截图」。
//
// 皮肤怎么套用照 server_main.cpp：共享层的 msime_client_skin_catalog 扫目录、msime_client_resolve_theme 按排列和明暗解析主题，canvas 包按明暗选面板图、确认图片解得出来，面板画得出来时再带 `canvas_panel_drawn` 解析一次取贴图原配色，配色、最小宽度、装饰图、背景图、圆角、版式、字体、动效和 canvas 画布经同样的 setter 交给候选窗；共享默认偏好（msime_client_default_preferences）给出字体、字号、缩放、不透明度和首行设置。卷轴展开用 CandidateWindow::open_expansion 打开，Server 那一侧的「开着」和完整候选列表由这里的读取器扮演。
#include "CandidateSkinGallery.h"
#include "../../../compat/WindowsDpi.h"
#include "CandidateCanvasImages.h"
#include "CandidateFontSettings.h"
#include "CandidateLayoutSettings.h"
#include "CandidateSkinAssets.h"
#include "CandidateThemeSettings.h"
#include "CandidateWindow.h"
#include "CandidateWindowStyleSettings.h"
#include "ComponentFailure.h"
#include "HostApiString.h"
#include <windows.h>
#include <dwmapi.h>
#include <wincodec.h>
#include <wrl/client.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <exception>
#include <filesystem>
#include <iostream>
#include <memory>
#include <optional>
#include <string>
#include <vector>

namespace {
using namespace msime::windows;
using msime::windows::gallery::GalleryShot;
using Microsoft::WRL::ComPtr;

// 浅色和深色截图各自的底色（0xRRGGBB），近似 Windows 浅色、深色应用窗口的背景。截图的空白判断也拿它比。
constexpr uint32_t light_backdrop = 0xE6E6E6;
constexpr uint32_t dark_backdrop = 0x202020;

void report(const std::string &line) {
  std::cout << "[candidate-skin-gallery] " << line << std::endl;
}
std::string utf8(const std::wstring &text) {
  if (text.empty())
    return {};
  const int count = WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                                        nullptr, 0, nullptr, nullptr);
  std::string result(static_cast<size_t>(count > 0 ? count : 0), '\0');
  if (count > 0)
    WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), result.data(),
                        count, nullptr, nullptr);
  return result;
}
std::string hresult(HRESULT value) {
  char text[16];
  std::snprintf(text, sizeof(text), "0x%08lX", static_cast<unsigned long>(value));
  return text;
}
std::string rect_text(const RECT &rect) {
  return std::to_string(rect.right - rect.left) + "x" + std::to_string(rect.bottom - rect.top) +
         " at (" + std::to_string(rect.left) + "," + std::to_string(rect.top) + ")";
}

// host-api 的回答 {"ok":true,"value":...}：拿 value，失败时 error 里是共享层给的原因。
std::optional<nlohmann::json> host_value(char *raw, std::string &error) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) {
    error = "host-api returned no response";
    return std::nullopt;
  }
  auto document = nlohmann::json::parse(owned.get(), nullptr, false);
  if (document.is_discarded() || !document.is_object()) {
    error = "host-api returned unreadable JSON";
    return std::nullopt;
  }
  if (!document.value("ok", false) || !document.contains("value")) {
    const auto found = document.find("error");
    error = found != document.end() && found->is_string() ? found->get<std::string>()
                                                          : std::string("host-api refused the request");
    return std::nullopt;
  }
  return document.at("value");
}
std::optional<nlohmann::json> host_call(char *(*call)(const uint8_t *, size_t),
                                        const std::string &body, std::string &error) {
  return host_value(call(reinterpret_cast<const uint8_t *>(body.data()), body.size()), error);
}

// 具名的 CALLBACK 而不是 lambda：FONTENUMPROCW 是 __stdcall，无捕获的 lambda 在 x86 上转成的是 __cdecl 指针（CandidateWindow.cpp 的 note_font_found 同理）。
int CALLBACK note_font_found(const LOGFONTW *, const TEXTMETRICW *, DWORD, LPARAM data) {
  *reinterpret_cast<bool *>(data) = true;
  return 0;
}
bool font_installed(const std::string &family) {
  const HDC dc = GetDC(nullptr);
  if (!dc)
    return false;
  LOGFONTW logfont{};
  const int count = MultiByteToWideChar(CP_UTF8, 0, family.data(), static_cast<int>(family.size()),
                                        logfont.lfFaceName, LF_FACESIZE - 1);
  bool found = false;
  if (count > 0)
    EnumFontFamiliesExW(dc, &logfont, note_font_found, reinterpret_cast<LPARAM>(&found), 0);
  ReleaseDC(nullptr, dc);
  return found;
}

// 处理完排队的消息，最多等 wait 毫秒再来一批；每 50 毫秒让候选窗刷新一次，和 Server 主循环轮询快照一样。
void pump(CandidateWindow *window, DWORD wait) {
  const ULONGLONG deadline = GetTickCount64() + wait;
  ULONGLONG next_refresh = 0;
  for (;;) {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
    const ULONGLONG now = GetTickCount64();
    if (window && now >= next_refresh) {
      window->refresh();
      next_refresh = now + 50;
    }
    if (now >= deadline)
      return;
    MsgWaitForMultipleObjects(0, nullptr, FALSE, static_cast<DWORD>((std::min)(deadline - now, ULONGLONG{10})),
                              QS_ALLINPUT);
  }
}

// 铺满主显示器工作区的纯色窗口，放在候选窗下面：截图里阴影和透明的圆角外面是一块已知的颜色，而不是 runner 桌面上碰巧有的东西。要系统材质的皮肤铺彩色竖条纹（gallery::backdrop_stripe_color），看得出卡片后面有没有模糊。
class Backdrop final {
public:
  explicit Backdrop(const RECT &area) : area_(area) {
    WNDCLASSEXW descriptor{};
    descriptor.cbSize = sizeof(descriptor);
    descriptor.lpfnWndProc = procedure;
    descriptor.hInstance = GetModuleHandleW(nullptr);
    descriptor.lpszClassName = L"MSIME.CandidateSkinGallery.Backdrop";
    if (!RegisterClassExW(&descriptor) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
      throw std::runtime_error("backdrop class unavailable");
    window_ = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                              descriptor.lpszClassName, L"", WS_POPUP, area.left, area.top,
                              area.right - area.left, area.bottom - area.top, nullptr, nullptr,
                              descriptor.hInstance, this);
    if (!window_)
      throw std::runtime_error("backdrop window unavailable");
    ShowWindow(window_, SW_SHOWNOACTIVATE);
    UpdateWindow(window_);
  }
  ~Backdrop() {
    if (window_)
      DestroyWindow(window_);
  }
  Backdrop(const Backdrop &) = delete;
  Backdrop &operator=(const Backdrop &) = delete;
  // stripes 有值时铺深色（true）或浅色（false）的条纹，没有时铺 rgb 纯色。
  void set_color(uint32_t rgb, std::optional<bool> stripes = std::nullopt) {
    color_ = RGB((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    stripes_ = stripes;
    InvalidateRect(window_, nullptr, TRUE);
    UpdateWindow(window_);
  }
  const RECT &area() const { return area_; }

private:
  static LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    if (message == WM_NCCREATE)
      SetWindowLongPtrW(window, GWLP_USERDATA,
                        reinterpret_cast<LONG_PTR>(reinterpret_cast<CREATESTRUCTW *>(lparam)->lpCreateParams));
    auto *self = reinterpret_cast<Backdrop *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (self && message == WM_ERASEBKGND)
      return 1;
    if (self && message == WM_PAINT) {
      PAINTSTRUCT paint{};
      const HDC dc = BeginPaint(window, &paint);
      if (dc && self->stripes_) {
        for (LONG left = paint.rcPaint.left - paint.rcPaint.left % gallery::stripe_width; left < paint.rcPaint.right;
             left += gallery::stripe_width) {
          const uint32_t rgb = gallery::backdrop_stripe_color(static_cast<int>(left), *self->stripes_);
          const HBRUSH brush = CreateSolidBrush(RGB((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff));
          const RECT stripe{left, paint.rcPaint.top, left + gallery::stripe_width, paint.rcPaint.bottom};
          FillRect(dc, &stripe, brush);
          DeleteObject(brush);
        }
      } else if (dc) {
        const HBRUSH brush = CreateSolidBrush(self->color_);
        FillRect(dc, &paint.rcPaint, brush);
        DeleteObject(brush);
      }
      EndPaint(window, &paint);
      return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
  RECT area_{};
  HWND window_ = nullptr;
  COLORREF color_ = RGB(0xE6, 0xE6, 0xE6);
  std::optional<bool> stripes_;
};

// 从屏幕 DC 截下 rect（物理像素）。截的是 DWM 合成之后的桌面，所以 DirectComposition 交换链和分层窗口都在里面；返回逐行的 BGRA，alpha 填成不透明。
std::optional<std::vector<uint8_t>> capture_screen(const RECT &rect, std::string &error) {
  const int width = rect.right - rect.left, height = rect.bottom - rect.top;
  if (width <= 0 || height <= 0) {
    error = "empty capture rectangle";
    return std::nullopt;
  }
  const HDC screen = GetDC(nullptr);
  if (!screen) {
    error = "screen DC unavailable";
    return std::nullopt;
  }
  const HDC memory = CreateCompatibleDC(screen);
  BITMAPINFO info{};
  info.bmiHeader.biSize = sizeof(info.bmiHeader);
  info.bmiHeader.biWidth = width;
  info.bmiHeader.biHeight = -height;
  info.bmiHeader.biPlanes = 1;
  info.bmiHeader.biBitCount = 32;
  info.bmiHeader.biCompression = BI_RGB;
  void *bits = nullptr;
  const HBITMAP bitmap =
      memory ? CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, nullptr, 0) : nullptr;
  std::optional<std::vector<uint8_t>> pixels;
  if (bitmap && bits) {
    const HGDIOBJ previous = SelectObject(memory, bitmap);
    if (BitBlt(memory, 0, 0, width, height, screen, rect.left, rect.top, SRCCOPY | CAPTUREBLT)) {
      GdiFlush();
      const auto *source = static_cast<const uint8_t *>(bits);
      pixels.emplace(source, source + static_cast<size_t>(width) * static_cast<size_t>(height) * 4);
      for (size_t offset = 3; offset < pixels->size(); offset += 4)
        (*pixels)[offset] = 0xff;
    } else {
      error = "BitBlt failed with " + std::to_string(GetLastError());
    }
    SelectObject(memory, previous);
  } else {
    error = "capture bitmap unavailable";
  }
  if (bitmap)
    DeleteObject(bitmap);
  if (memory)
    DeleteDC(memory);
  ReleaseDC(nullptr, screen);
  return pixels;
}

// 用 WIC 把 BGRA 像素写成 PNG。
bool write_png(const std::filesystem::path &file, int width, int height,
               const std::vector<uint8_t> &pixels, std::string &error) {
  ComPtr<IWICImagingFactory> factory;
  ComPtr<IWICStream> stream;
  ComPtr<IWICBitmapEncoder> encoder;
  ComPtr<IWICBitmapFrameEncode> frame;
  ComPtr<IPropertyBag2> properties;
  HRESULT result = CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                    IID_PPV_ARGS(factory.GetAddressOf()));
  if (SUCCEEDED(result))
    result = factory->CreateStream(stream.GetAddressOf());
  if (SUCCEEDED(result))
    result = stream->InitializeFromFilename(file.c_str(), GENERIC_WRITE);
  if (SUCCEEDED(result))
    result = factory->CreateEncoder(GUID_ContainerFormatPng, nullptr, encoder.GetAddressOf());
  if (SUCCEEDED(result))
    result = encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache);
  if (SUCCEEDED(result))
    result = encoder->CreateNewFrame(frame.GetAddressOf(), properties.GetAddressOf());
  if (SUCCEEDED(result))
    result = frame->Initialize(properties.Get());
  if (SUCCEEDED(result))
    result = frame->SetSize(static_cast<UINT>(width), static_cast<UINT>(height));
  WICPixelFormatGUID format = GUID_WICPixelFormat32bppBGRA;
  if (SUCCEEDED(result))
    result = frame->SetPixelFormat(&format);
  if (SUCCEEDED(result) && !IsEqualGUID(format, GUID_WICPixelFormat32bppBGRA)) {
    error = "PNG encoder does not take 32bpp BGRA";
    return false;
  }
  if (SUCCEEDED(result))
    result = frame->WritePixels(static_cast<UINT>(height), static_cast<UINT>(width) * 4,
                                static_cast<UINT>(pixels.size()),
                                const_cast<BYTE *>(pixels.data()));
  if (SUCCEEDED(result))
    result = frame->Commit();
  if (SUCCEEDED(result))
    result = encoder->Commit();
  if (FAILED(result))
    error = "WIC PNG encoding failed with " + hresult(result);
  return SUCCEEDED(result);
}

// 共享默认偏好里候选窗用到的几项，和 Server 从偏好文档里取它们用的是同一组函数。
struct Defaults {
  CandidateFontSettings fonts;
  CandidateWindowStyle style;
  CandidateLayoutSettings layout;
  nlohmann::json theme_values;
};

// 一张截图要套用的外观：共享层解析出的主题叠上包的细分槽位，和包的其余资源。
struct Look {
  CandidatePalette palette;
  CandidateSkinAssets assets;
  // 这一种明暗下要画的 canvas 画布（candidate_canvas_drawable）；不是 canvas 包时为空。
  CandidateSkinCanvas canvas;
};

// 只在显式 --verify-image-animation 的专用测试桌面上切换非持久化系统偏好。
class AnimationPreference final {
public:
  AnimationPreference() {
    if (!SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &original_, 0))
      throw std::runtime_error("animation preference unavailable");
  }
  ~AnimationPreference() {
    SystemParametersInfoW(SPI_SETCLIENTAREAANIMATION, 0,
                            reinterpret_cast<void *>(static_cast<UINT_PTR>(original_ != FALSE)), 0);
  }
  void set(bool enabled, HWND window) {
    if (!SystemParametersInfoW(SPI_SETCLIENTAREAANIMATION, 0,
                                reinterpret_cast<void *>(static_cast<UINT_PTR>(enabled)), 0))
      throw std::runtime_error("animation preference cannot be changed on the test desktop");
    SendMessageW(window, WM_SETTINGCHANGE, SPI_SETCLIENTAREAANIMATION, 0);
  }
private:
  BOOL original_ = TRUE;
};

void verify_image_animation(CandidateWindow &window, const GalleryShot &shot, size_t &frames,
                            const std::filesystem::path &output) {
  AnimationPreference preference;
  window.set_skin_animation({});
  preference.set(true, window.handle());
  pump(nullptr, 100);
  RECT rect{};
  if (!GetWindowRect(window.handle(), &rect))
    throw std::runtime_error("animation capture rectangle unavailable");
  std::string error;
  const auto capture = [&]() {
    DwmFlush();
    const auto pixels = capture_screen(rect, error);
    if (!pixels)
      throw std::runtime_error("animation capture failed: " + error);
    return *pixels;
  };
  const auto save = [&](const char *stage, const std::vector<uint8_t> &pixels) {
    auto file = output / std::filesystem::u8path(gallery::shot_file_name(shot));
    file.replace_filename(file.stem().wstring() + L"__" + std::filesystem::path(stage).wstring() + L".png");
    if (!write_png(file, rect.right - rect.left, rect.bottom - rect.top, pixels, error))
      throw std::runtime_error("animation evidence write failed: " + error);
  };
  const auto initial = capture();
  save("animation-start", initial);
  const auto before = frames;
  bool changed = false;
  for (size_t index = 0; index < 12; ++index) {
    // 只派发 Win32 消息；不调用 refresh，也不主动让候选窗失效。
    pump(nullptr, 75);
    const auto current = capture();
    size_t differences = 0;
    for (size_t pixel = 0; pixel + 3 < current.size(); pixel += 4)
      if (current[pixel] != initial[pixel] || current[pixel + 1] != initial[pixel + 1] ||
          current[pixel + 2] != initial[pixel + 2]) ++differences;
    if (differences > 20 && frames > before + 1) {
      save("animation-changed", current);
      changed = true;
      break;
    }
  }
  if (!changed)
    throw std::runtime_error("image timer did not change the native window pixels without input");

  SendMessageW(window.handle(), WM_POWERBROADCAST, PBT_APMSUSPEND, 0);
  pump(nullptr, 50);
  const auto sleeping = frames;
  pump(nullptr, 300);
  if (frames != sleeping)
    throw std::runtime_error("image timer continued while suspended");
  SendMessageW(window.handle(), WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC, 0);
  pump(nullptr, 400);
  if (frames <= sleeping + 1)
    throw std::runtime_error("image animation did not recover after resume");

  preference.set(false, window.handle());
  pump(nullptr, 100);
  const auto still = capture();
  save("reduced-motion", still);
  const auto stopped = frames;
  pump(nullptr, 350);
  if (frames != stopped || capture() != still)
    throw std::runtime_error("reduced motion continued changing frames or pixels");

  preference.set(true, window.handle());
  pump(nullptr, 200);
  window.hide();
  pump(nullptr, 50);
  const auto hidden = frames;
  pump(nullptr, 300);
  if (IsWindowVisible(window.handle()) || frames != hidden)
    throw std::runtime_error("hidden candidate window retained its image timer");
  window.refresh();
  pump(nullptr, 400);
  if (!IsWindowVisible(window.handle()) || frames <= hidden + 1)
    throw std::runtime_error("image animation did not restart after show");
  report(gallery::shot_label(shot) + ": image animation pixels, suspend/resume, reduced motion and hide/show passed");
}

// 专用桌面里复用合成锦鲤图片，检查真实候选窗的文字像素及刷新、失败清理。
void verify_decoration_overlay(CandidateWindow &window, CandidateSkinDecoration decoration,
    CandidatePresentation &presentation, const GalleryShot &shot, const std::filesystem::path &output) {
  AnimationPreference preference;
  window.set_skin_animation({});
  preference.set(false, window.handle());
  const auto sample = [&](const char *stage) {
    ++presentation.render_serial;
    pump(&window, 150);
    DwmFlush();
    RECT rect{};
    if (window.failed() || !GetWindowRect(window.handle(), &rect))
      throw std::runtime_error("decoration capture rectangle unavailable");
    std::string error;
    const auto pixels = capture_screen(rect, error);
    if (!pixels) throw std::runtime_error("decoration capture failed: " + error);
    auto file = output / std::filesystem::u8path(gallery::shot_file_name(shot));
    file.replace_filename(file.stem().wstring() + L"__overlay-" + std::filesystem::path(stage).wstring() + L".png");
    if (!write_png(file, rect.right - rect.left, rect.bottom - rect.top, *pixels, error))
      throw std::runtime_error("decoration evidence write failed: " + error);
    size_t colored = 0;
    for (size_t at = 0; at + 3 < pixels->size(); at += 4)
      if ((*pixels)[at] > 180 && (*pixels)[at + 1] < 100 && (*pixels)[at + 2] > 180) ++colored;
    return colored;
  };
  decoration.candidate.reset();
  window.set_skin_decoration(decoration);
  if (sample("off") != 0) throw std::runtime_error("decoration fixture contains the reserved word color");
  msime::candidate::DecorationCandidate style;
  style.index = 0; style.fontSizeDip = 20; style.color = "#FF00FFFF";
  decoration.candidate = style;
  presentation.candidates[0].text = "WWW";
  presentation.candidates[1].text = "i";
  window.set_skin_decoration(decoration);
  const auto first = sample("first");
  if (first < 20) throw std::runtime_error("decoration word has no visible pixels");
  presentation.candidates[0].highlighted = false;
  presentation.candidates[1].highlighted = true;
  const auto selected = sample("highlight");
  if (selected == 0 || selected >= first) throw std::runtime_error("decoration retained the previous highlight");
  ++presentation.page;
  presentation.candidates[1].text = "MMMM";
  if (sample("page") <= selected) throw std::runtime_error("decoration retained the previous page");
  decoration.candidate->index = 100;
  window.set_skin_decoration(decoration);
  if (sample("outside") != 0) throw std::runtime_error("out of page decoration word was not cleared");
  decoration.candidate->index = 2;
  presentation.candidates[1].text = "i";
  presentation.candidates[1].badge = "WW";
  window.set_skin_decoration(decoration);
  const auto stripped = sample("strip-badge");
  decoration.candidate->stripBadge = false;
  window.set_skin_decoration(decoration);
  if (stripped == 0 || sample("badge") <= stripped) throw std::runtime_error("decoration badge is not independent");
  const auto image = decoration.image;
  decoration.image += L".missing";
  window.set_skin_decoration(decoration);
  if (sample("broken") != 0) throw std::runtime_error("broken decoration image retained its word");
  decoration.image = image;
  decoration.candidate.reset();
  window.set_skin_decoration(decoration);
  if (sample("reset") != 0) throw std::runtime_error("decoration reset retained its word");
  report(gallery::shot_label(shot) + ": decoration word pixels, highlight, page, badge, bounds and image failure passed");
}

// 画一张截图并写进 output。成功返回 true；失败时日志里写清楚是哪一步。
bool render_shot(const GalleryShot &shot, const Look &look, const Defaults &defaults, POINT anchor,
                 Backdrop &backdrop, const std::filesystem::path &output, bool verify_images) {
  const auto label = gallery::shot_label(shot);
  const uint32_t backdrop_color = shot.dark ? dark_backdrop : light_backdrop;
  // 要系统材质的皮肤铺条纹，先截下候选窗出现之前的屏幕，空白和裁边的自检拿它比，而不是和纯色比。
  const bool striped = look.assets.layout.backdrop != CandidateSkinBackdrop::none;
  backdrop.set_color(backdrop_color, striped ? std::optional<bool>(shot.dark) : std::nullopt);
  std::optional<std::vector<uint8_t>> behind_screen;
  if (striped) {
    pump(nullptr, 100);
    DwmFlush();
    std::string error;
    behind_screen = capture_screen(backdrop.area(), error);
    if (!behind_screen) {
      report(label + ": FAILED, capture of the striped backdrop: " + error);
      return false;
    }
  }
  auto presentation = gallery::gallery_presentation(anchor.x, anchor.y);
  const auto expanded =
      std::make_shared<const std::vector<ExpandedCandidate>>(gallery::gallery_expanded_candidates());
  // Server 那一侧：展开打开后快照带上 expansion_open 和这一代的完整候选（CandidateMailbox::snapshot）。
  bool server_open = false;
  size_t frames = 0;
  try {
    CandidateWindow window(
        [&]() -> std::optional<CandidatePresentation> {
          auto value = presentation;
          value.expansion_open = server_open;
          if (server_open)
            value.expanded = expanded;
          return value;
        },
        [](const CandidateClick &) {}, defaults.fonts.size, defaults.fonts.preedit_size,
        defaults.fonts.family, defaults.fonts.fallback, shot.dark, shot.horizontal,
        defaults.layout.show_preedit, [](const CandidatePage &) {},
        [&](const CandidatePresentation &) { ++frames; }, false);
    window.set_expansion_handler([&](const FocusLease &, bool open) { server_open = open; });
    window.set_fonts(defaults.fonts);
    window.set_style(defaults.style);
    auto layout = defaults.layout;
    layout.horizontal = shot.horizontal;
    window.set_layout(layout);
    window.set_palette(look.palette);
    window.set_skin_min_width(look.assets.min_width);
    window.set_skin_decoration(look.assets.decoration);
    window.set_skin_background(look.assets.background);
    window.set_skin_corner_radius(look.assets.corner_radius);
    window.set_skin_layout(look.assets.layout);
    window.set_skin_font_family(look.assets.font_family);
    window.set_skin_animation(look.assets.animation);
    window.set_skin_canvas(look.canvas);
    const auto wait_for_frame = [&](size_t after, DWORD timeout) {
      const ULONGLONG deadline = GetTickCount64() + timeout;
      while (frames <= after && !window.failed() && GetTickCount64() < deadline)
        pump(&window, 20);
      return frames > after && !window.failed();
    };
    if (!wait_for_frame(0, 10000)) {
      report(label + ": FAILED, no frame was presented" +
          (window.failed() ? " (" + component_failure("candidate window", window.failure_site()) + ")"
                           : std::string(" within 10 s")));
      return false;
    }
    if (shot.expanded) {
      const size_t before = frames;
      if (!window.open_expansion()) {
        report(label + ": FAILED, the skin has expand_panel but the card drew no expand button");
        return false;
      }
      if (!server_open) {
        report(label + ": FAILED, opening the expansion did not notify the Server side");
        return false;
      }
      if (!wait_for_frame(before, 10000)) {
        report(label + ": FAILED, no expanded frame was presented");
        return false;
      }
    }
    // 动效（进入动画）放完、DWM 把最后一帧合成上屏之后再截。
    pump(&window, look.assets.animation.duration_ms + 300);
    DwmFlush();
    pump(&window, 50);
    DwmFlush();
    if (window.failed()) {
      report(label + ": FAILED, " + component_failure("candidate window", window.failure_site()));
      return false;
    }
    if (!IsWindowVisible(window.handle())) {
      report(label + ": FAILED, the candidate window is not visible");
      return false;
    }
    RECT rect{};
    if (!GetWindowRect(window.handle(), &rect)) {
      report(label + ": FAILED, GetWindowRect failed with " + std::to_string(GetLastError()));
      return false;
    }
    // 自检一：上一帧画出的卡片要落在客户区里，并且和按窗口尺寸推出的卡片一样大。画图和定窗口用的不是同一个尺寸时（例如按比窗口大的渲染目标排版），卡片会画出窗口被裁掉。
    RECT client{};
    GetClientRect(window.handle(), &client);
    const auto painted = window.painted_card();
    const auto on_screen = window.card_on_screen();
    std::string geometry_problem;
    std::string card;
    if (!painted || !on_screen) {
      geometry_problem = "the window reports no painted card";
    } else {
      const gallery::PixelRect drawn{painted->left, painted->top, painted->right, painted->bottom};
      const gallery::PixelRect expected{std::lround(on_screen->left) - rect.left,
                                        std::lround(on_screen->top) - rect.top,
                                        std::lround(on_screen->right) - rect.left,
                                        std::lround(on_screen->bottom) - rect.top};
      card = ", card " + std::to_string(drawn.right - drawn.left) + "x" +
             std::to_string(drawn.bottom - drawn.top) + " at (" + std::to_string(drawn.left) + "," +
             std::to_string(drawn.top) + ")";
      if (!gallery::card_fits(drawn, client.right, client.bottom, expected))
        geometry_problem = "the painted card (" + std::to_string(drawn.left) + "," + std::to_string(drawn.top) +
                           ")-(" + std::to_string(drawn.right) + "," + std::to_string(drawn.bottom) +
                           ") does not fit the " + std::to_string(client.right) + "x" +
                           std::to_string(client.bottom) + " client area or differs from the card the window was sized for (" +
                           std::to_string(expected.left) + "," + std::to_string(expected.top) + ")-(" +
                           std::to_string(expected.right) + "," + std::to_string(expected.bottom) + ")";
    }
    std::string error;
    const auto pixels = capture_screen(rect, error);
    if (!pixels) {
      report(label + ": FAILED, capture of " + rect_text(rect) + ": " + error);
      return false;
    }
    const int width = rect.right - rect.left, height = rect.bottom - rect.top;
    std::vector<uint8_t> behind;
    if (behind_screen) {
      const RECT &area = backdrop.area();
      behind = gallery::crop(*behind_screen, area.right - area.left, area.bottom - area.top,
                             {rect.left - area.left, rect.top - area.top, rect.right - area.left,
                              rect.bottom - area.top});
      if (behind.empty()) {
        report(label + ": FAILED, the window " + rect_text(rect) + " is not inside the striped work area");
        return false;
      }
    }
    const auto verdict = behind_screen ? gallery::capture_verdict(*pixels, behind)
                                       : gallery::capture_verdict(*pixels, backdrop_color);
    // 自检二：截图最外一圈应该是底色（阴影到边上已经淡没了），卡片画到边上就是被窗口裁掉了。
    const auto bleed = behind_screen ? gallery::edge_bleed(*pixels, behind, width, height)
                                     : gallery::edge_bleed(*pixels, width, height, backdrop_color);
    char share[96];
    std::snprintf(share, sizeof(share),
                  "%.1f%% of pixels differ from the backdrop, edges L%.0f%% T%.0f%% R%.0f%% B%.0f%%",
                  verdict.fraction() * 100.0, bleed.left * 100.0, bleed.top * 100.0,
                  bleed.right * 100.0, bleed.bottom * 100.0);
    const auto file = output / std::filesystem::u8path(gallery::shot_file_name(shot));
    auto summary = "window " + rect_text(rect) + card + ", " + std::to_string(frames) + " frames, " + share;
    // 要系统材质的皮肤：卡片背后实际垫了什么（或为什么是不透明底色），见 CandidateBackdrop.h。
    if (striped)
      summary += ", backdrop " + window.painted_backdrop();
    std::string problem;
    if (gallery::capture_blank(verdict))
      problem = "the capture is blank";
    else if (!geometry_problem.empty())
      problem = geometry_problem;
    else if (bleed.worst() > gallery::maximum_edge_fraction)
      problem = "the card reaches the window edge and is clipped";
    if (!problem.empty()) {
      // 有问题的也写出来，产物里能看到截到了什么。
      (void)write_png(file, width, height, *pixels, error);
      report(label + ": FAILED, " + problem + " (" + summary + ")");
      return false;
    }
    if (!write_png(file, width, height, *pixels, error)) {
      report(label + ": FAILED, writing " + utf8(file.wstring()) + ": " + error);
      return false;
    }
    report(label + ": " + summary + " -> " + utf8(file.filename().wstring()));
    if (verify_images && shot.horizontal && !shot.dark && !shot.expanded &&
        (shot.skin == "verify-canvas" || shot.skin == "verify-canvas-frame" ||
         shot.skin == "verify-v2-motion-system"))
      verify_image_animation(window, shot, frames, output);
    if (verify_images && shot.skin == "verify-v2-motion-system" && !shot.expanded)
      verify_decoration_overlay(window, look.assets.decoration, presentation, shot, output);
    return true;
  } catch (const std::exception &exception) {
    report(label + ": FAILED, " + exception.what());
    return false;
  }
}

int usage() {
  std::cout << "Usage: msime-candidate-skin-gallery [--skins <skin root>] [--output <directory>] [--verify-image-animation]\n"
               "Renders the Windows candidate window for every skin under the skin root (default "
               "crates/client-services/tests/fixtures/candidate-skins) and for no skin, horizontal "
               "and vertical, light and dark, plus the expanded panel of expand_panel skins, and "
               "writes one PNG per capture into the output directory (default "
               "target/windows-candidate-skin-gallery). Needs an interactive desktop; it covers the "
               "primary screen with a plain backdrop while it runs. Exits 1 when a skin fails to "
               "load, a capture fails or comes out blank."
            << std::endl;
  return 2;
}
} // namespace

int wmain(int argc, wchar_t **argv) {
  std::filesystem::path skins = L"crates/client-services/tests/fixtures/candidate-skins";
  std::filesystem::path output = L"target/windows-candidate-skin-gallery";
  bool verify_images = false;
  for (int index = 1; index < argc; ++index) {
    const std::wstring argument = argv[index];
    if ((argument == L"--skins" || argument == L"--output") && index + 1 < argc)
      (argument == L"--skins" ? skins : output) = argv[++index];
    else if (argument == L"--verify-image-animation")
      verify_images = true;
    else
      return usage();
  }
  // 自己的进程：整个进程按每显示器 DPI 感知，窗口矩形和屏幕截图都是物理像素。
  if (!msime::windows::compat::process_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
    report("SetProcessDpiAwarenessContext failed with " + std::to_string(GetLastError()) +
        "; window rectangles may be scaled");
  const HRESULT apartment = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  if (FAILED(apartment)) {
    report("FAILED, COM is unavailable: " + hresult(apartment));
    return 1;
  }
  int status = 0;
  {
    std::error_code ignored;
    skins = std::filesystem::absolute(skins, ignored);
    output = std::filesystem::absolute(output, ignored);
    report("skin root " + utf8(skins.wstring()) + ", output " + utf8(output.wstring()));
    std::string error;
    // 共享默认偏好：新装时候选窗的字体、字号、缩放、不透明度、首行和主题。
    const auto preferences = host_value(msime_client_default_preferences(), error);
    if (!preferences || !preferences->is_object()) {
      report("FAILED, default preferences: " + error);
      return 1;
    }
    Defaults defaults;
    defaults.fonts = candidate_font_settings(*preferences).value_or(CandidateFontSettings{});
    defaults.style = candidate_window_style(*preferences).value_or(CandidateWindowStyle{});
    defaults.layout = candidate_layout_settings(*preferences).value_or(CandidateLayoutSettings{});
    defaults.theme_values = candidate_theme_values(*preferences);
    std::string fonts = defaults.fonts.family + (font_installed(defaults.fonts.family) ? "" : " (not installed)");
    for (const auto &family : defaults.fonts.fallback)
      fonts += ", " + family + (font_installed(family) ? "" : " (not installed)");
    report("fonts " + fonts + "; size " + std::to_string(defaults.fonts.size) + ", preedit " +
        std::to_string(defaults.fonts.preedit_size) + "; scale " +
        std::to_string(defaults.style.scale_percent) + "%, opacity " +
        std::to_string(defaults.style.opacity_percent) + "%; theme " +
        defaults.theme_values.dump());
    if (!std::filesystem::is_directory(skins, ignored)) {
      report("FAILED, the skin root is not a directory");
      return 1;
    }
    const auto root = skins.u8string();
    auto catalog = host_value(msime_client_skin_catalog(reinterpret_cast<const uint8_t *>(root.data()),
                                                        root.size()),
                              error);
    if (!catalog || !catalog->is_object() || !catalog->contains("packages") ||
        !catalog->at("packages").is_array()) {
      report("FAILED, skin catalog: " + (catalog ? std::string("unexpected shape") : error));
      return 1;
    }
    if (catalog->contains("issues") && catalog->at("issues").is_array())
      for (const auto &issue : catalog->at("issues")) {
        report("FAILED, skin folder " + issue.value("folder", std::string("?")) +
            " did not load: " + issue.value("reason", std::string("?")));
        status = 1;
      }
    std::vector<std::string> ids;
    for (const auto &package : catalog->at("packages")) {
      const auto id = package.value("id", std::string{});
      if (!valid_candidate_skin_id(id) || id == gallery::baseline_name) {
        report("FAILED, the catalog lists a package whose id is unusable here: " + package.value("id", nlohmann::json()).dump());
        status = 1;
        continue;
      }
      ids.push_back(id);
      std::string line = "package " + id + " \"" + package.value("name", std::string{}) +
                         "\" base " + package.value("base", nlohmann::json()).dump() + ", layouts " +
                         package.value("layouts", nlohmann::json::array()).dump() + ", themes " +
                         package.value("themes", nlohmann::json::array()).dump();
      if (package.value("canvas", false))
        line += ", canvas";
      report(line);
      if (package.contains("notices") && package.at("notices").is_array())
        for (const auto &notice : package.at("notices"))
          report("  notice: " + (notice.is_string() ? notice.get<std::string>() : notice.dump()));
    }
    std::sort(ids.begin(), ids.end());
    if (ids.empty()) {
      report("FAILED, the skin root has no loadable packages");
      return 1;
    }
    std::error_code created;
    std::filesystem::create_directories(output, created);
    if (created) {
      report("FAILED, cannot create the output directory: " + created.message());
      return 1;
    }
    MONITORINFO monitor{};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoW(MonitorFromPoint(POINT{0, 0}, MONITOR_DEFAULTTOPRIMARY), &monitor)) {
      report("FAILED, the primary monitor is unavailable");
      return 1;
    }
    const RECT work = monitor.rcWork;
    report("primary work area " + rect_text(work));
    {
      // 系统材质跟着这两个开关走：透明效果关着或高对比度时，要材质的皮肤画不透明底色。
      DWORD transparency = 1, size = sizeof(transparency);
      const bool transparency_known =
          RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                       L"EnableTransparency", RRF_RT_REG_DWORD, nullptr, &transparency, &size) == ERROR_SUCCESS;
      HIGHCONTRASTW contrast{};
      contrast.cbSize = sizeof(contrast);
      const bool high_contrast = SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast), &contrast, 0) &&
                                 (contrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
      OSVERSIONINFOW version{};
      version.dwOSVersionInfoSize = sizeof(version);
      using RtlGetVersion = LONG(WINAPI *)(OSVERSIONINFOW *);
      const HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
      const auto get_version = ntdll ? reinterpret_cast<RtlGetVersion>(
                                           reinterpret_cast<void *>(GetProcAddress(ntdll, "RtlGetVersion")))
                                     : nullptr;
      if (get_version)
        get_version(&version);
      report("Windows " + std::to_string(version.dwMajorVersion) + "." + std::to_string(version.dwMinorVersion) +
             " build " + std::to_string(version.dwBuildNumber) + "; transparency effects " +
             (transparency_known ? (transparency ? "on" : "off") : "unset (on)") + ", high contrast " +
             (high_contrast ? "on" : "off"));
    }
    // 光标底边的锚点放在工作区左上部，候选窗出现在它下面；鼠标挪到右下角，免得悬停底色落在候选上。
    const POINT anchor{work.left + 64, work.top + 112};
    SetCursorPos(work.right - 2, work.bottom - 2);
    try {
      Backdrop backdrop(work);
      size_t written = 0, skipped = 0, failed = 0;
      std::vector<std::string> targets{std::string{}};
      targets.insert(targets.end(), ids.begin(), ids.end());
      for (const auto &skin : targets) {
        auto values = defaults.theme_values;
        if (!skin.empty()) {
          // 和在设置里选了这款皮肤一样：自定义主题，浅色槽位指名它，深色没设时也取它。
          values["global_theme"] = "custom";
          auto custom = values.contains("custom_theme") && values.at("custom_theme").is_object()
                            ? values.at("custom_theme")
                            : nlohmann::json::object();
          custom["candidate_skin"] = skin;
          custom.erase("candidate_skin_dark");
          values["custom_theme"] = std::move(custom);
        }
        size_t drawn = 0;
        for (const bool horizontal : {true, false})
          for (const bool dark : {false, true}) {
            GalleryShot shot{skin, horizontal, dark, false};
            const auto request = candidate_theme_request(values, dark, horizontal, skins);
            const auto value = host_call(msime_client_resolve_theme, request.dump(), error);
            const auto theme = value ? candidate_theme_resolution(*value) : std::nullopt;
            if (!theme) {
              report(gallery::shot_label(shot) + ": FAILED, theme resolution: " +
                  (value ? "unreadable answer " + value->dump() : error));
              ++failed;
              continue;
            }
            if (theme->candidate_skin != skin) {
              // 包没有声明这个排列或明暗，或它的 base 是另一种明暗：Server 在这里也不画它。
              report(gallery::shot_label(shot) + ": skipped, the package does not draw in this layout and mode");
              ++skipped;
              continue;
            }
            Look look;
            if (!skin.empty())
              look.assets = candidate_skin_assets(*catalog, skin, skins);
            // canvas：按卡片实际的明暗选面板图，解不出来的面板和挂件不画，并写进日志。
            look.canvas = candidate_canvas_drawable(look.assets.canvas_skin, !theme->dark.value_or(dark),
                                                    candidate_canvas_image_decodes);
            auto palette_theme = *theme;
            if (look.assets.canvas_skin.active) {
              const bool panel = candidate_canvas_panel_drawn(look.canvas);
              report(gallery::shot_label(shot) + ": canvas panel " +
                     (look.assets.canvas_skin.border ? (panel ? "drawn" : "does not decode, opaque canvas look")
                                                     : std::string("none")) +
                     ", " + std::to_string(look.canvas.decorations.size()) + " of " +
                     std::to_string(look.assets.canvas_skin.decorations.size()) + " decorations decode");
              if (look.assets.canvas_skin.border && !panel) {
                report(gallery::shot_label(shot) + ": FAILED, the canvas border_image does not decode");
                ++failed;
              }
              if (look.canvas.decorations.size() != look.assets.canvas_skin.decorations.size()) {
                report(gallery::shot_label(shot) + ": FAILED, a canvas decoration image does not decode");
                ++failed;
              }
              if (panel) {
                // 和 Server 一样：面板画得出来时取包为贴图挑的原配色。
                const auto drawn_request = candidate_theme_request(values, dark, horizontal, skins, true);
                const auto drawn_value = host_call(msime_client_resolve_theme, drawn_request.dump(), error);
                const auto drawn_theme = drawn_value ? candidate_theme_resolution(*drawn_value) : std::nullopt;
                if (!drawn_theme) {
                  report(gallery::shot_label(shot) + ": FAILED, canvas panel theme resolution: " +
                      (drawn_value ? "unreadable answer " + drawn_value->dump() : error));
                  ++failed;
                  continue;
                }
                palette_theme = *drawn_theme;
              }
            }
            look.palette = apply_candidate_skin_extra_slots(
                candidate_theme_palette(palette_theme, dark), look.assets.candidate_colors,
                candidate_theme_pickers(values), palette_theme.dark.value_or(dark));
            const auto shots = horizontal && look.assets.layout.expand_panel.value_or(false) ? 2 : 1;
            for (int index = 0; index < shots; ++index) {
              shot.expanded = index == 1;
              if (render_shot(shot, look, defaults, anchor, backdrop, output, verify_images)) {
                ++written;
                ++drawn;
              } else {
                ++failed;
              }
            }
          }
        if (!skin.empty() && drawn == 0) {
          report(skin + ": FAILED, the package was not drawn in any layout and mode");
          ++failed;
        }
      }
      report(std::to_string(written) + " captures written, " + std::to_string(skipped) +
          " skipped, " + std::to_string(failed) + " failed");
      if (failed)
        status = 1;
    } catch (const std::exception &exception) {
      report(std::string("FAILED, ") + exception.what());
      status = 1;
    }
  }
  CoUninitialize();
  return status;
}
