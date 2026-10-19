// 候选皮肤截图工具：用 Server 的候选窗（CandidateWindow）把皮肤目录里的每款皮肤画在一页固定的假候选上，从屏幕截下来存成 PNG，没有 Windows 机器也能在 CI 的产物里看皮肤画得对不对。用法和 CI 里的跑法见 platforms/windows/README.md「候选皮肤截图」。
//
// 皮肤怎么套用照 server_main.cpp：共享层的 msime_client_skin_catalog 扫目录、msime_client_resolve_theme 按排列和明暗解析主题，配色、最小宽度、装饰图、背景图、圆角、版式、字体和动效经同样的 setter 交给候选窗；共享默认偏好（msime_client_default_preferences）给出字体、字号、缩放、不透明度和首行设置。卷轴展开用 CandidateWindow::open_expansion 打开，Server 那一侧的「开着」和完整候选列表由这里的读取器扮演。
#include "CandidateSkinGallery.h"
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

// 铺满主显示器工作区的纯色窗口，放在候选窗下面：截图里阴影和透明的圆角外面是一块已知的颜色，而不是 runner 桌面上碰巧有的东西。
class Backdrop final {
public:
  explicit Backdrop(const RECT &area) {
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
  void set_color(uint32_t rgb) {
    color_ = RGB((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    InvalidateRect(window_, nullptr, TRUE);
    UpdateWindow(window_);
  }

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
      if (dc) {
        const HBRUSH brush = CreateSolidBrush(self->color_);
        FillRect(dc, &paint.rcPaint, brush);
        DeleteObject(brush);
      }
      EndPaint(window, &paint);
      return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
  HWND window_ = nullptr;
  COLORREF color_ = RGB(0xE6, 0xE6, 0xE6);
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
};

// 画一张截图并写进 output。成功返回 true；失败时日志里写清楚是哪一步。
bool render_shot(const GalleryShot &shot, const Look &look, const Defaults &defaults, POINT anchor,
                 Backdrop &backdrop, const std::filesystem::path &output) {
  const auto label = gallery::shot_label(shot);
  const uint32_t backdrop_color = shot.dark ? dark_backdrop : light_backdrop;
  backdrop.set_color(backdrop_color);
  const auto presentation = gallery::gallery_presentation(anchor.x, anchor.y);
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
    std::string card;
    if (const auto on_screen = window.card_on_screen())
      card = ", card " + std::to_string(std::lround(on_screen->right - on_screen->left)) + "x" +
             std::to_string(std::lround(on_screen->bottom - on_screen->top));
    std::string error;
    const auto pixels = capture_screen(rect, error);
    if (!pixels) {
      report(label + ": FAILED, capture of " + rect_text(rect) + ": " + error);
      return false;
    }
    const auto verdict = gallery::capture_verdict(*pixels, backdrop_color);
    char share[32];
    std::snprintf(share, sizeof(share), "%.1f%%", verdict.fraction() * 100.0);
    const auto file = output / std::filesystem::u8path(gallery::shot_file_name(shot));
    const auto summary = "window " + rect_text(rect) + card + ", " + std::to_string(frames) +
                         " frames, " + share + " of pixels differ from the backdrop";
    if (gallery::capture_blank(verdict)) {
      // 空白的也写出来，产物里能看到截到了什么。
      (void)write_png(file, rect.right - rect.left, rect.bottom - rect.top, *pixels, error);
      report(label + ": FAILED, the capture is blank (" + summary + ")");
      return false;
    }
    if (!write_png(file, rect.right - rect.left, rect.bottom - rect.top, *pixels, error)) {
      report(label + ": FAILED, writing " + utf8(file.wstring()) + ": " + error);
      return false;
    }
    report(label + ": " + summary + " -> " + utf8(file.filename().wstring()));
    return true;
  } catch (const std::exception &exception) {
    report(label + ": FAILED, " + exception.what());
    return false;
  }
}

int usage() {
  std::cout << "Usage: msime-candidate-skin-gallery [--skins <skin root>] [--output <directory>]\n"
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
  for (int index = 1; index < argc; ++index) {
    const std::wstring argument = argv[index];
    if ((argument == L"--skins" || argument == L"--output") && index + 1 < argc)
      (argument == L"--skins" ? skins : output) = argv[++index];
    else
      return usage();
  }
  // 自己的进程：整个进程按每显示器 DPI 感知，窗口矩形和屏幕截图都是物理像素。
  if (!SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
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
        line += ", canvas (border_image, insets and free decorations are not drawn on Windows)";
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
            look.palette = apply_candidate_skin_extra_slots(
                candidate_theme_palette(*theme, dark), look.assets.candidate_colors,
                candidate_theme_pickers(values), theme->dark.value_or(dark));
            const auto shots = horizontal && look.assets.layout.expand_panel.value_or(false) ? 2 : 1;
            for (int index = 0; index < shots; ++index) {
              shot.expanded = index == 1;
              if (render_shot(shot, look, defaults, anchor, backdrop, output)) {
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
