#include "../../settings/CloudQuickPhraseClearWindow.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <atomic>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <thread>

using Json = nlohmann::json;
namespace {
std::atomic<int> mode{0}, writes{0}, cancels{0}, failures{0};
constexpr const char *token = "a3be8604-3229-48e1-a1de-7316dc70cb43";
void expect(bool condition, const char *message) {
  if (!condition) { ++failures; std::fprintf(stderr, "%s\n", message); }
}
HWND dialog() {
  HWND result = nullptr;
  EnumWindows(+[](HWND window, LPARAM data) -> BOOL {
    DWORD process = 0; GetWindowThreadProcessId(window, &process);
    wchar_t title[64]{}; GetWindowTextW(window, title, 64);
    if (process == GetCurrentProcessId() && std::wstring(title) == L"清空云端快捷短语") {
      *reinterpret_cast<HWND *>(data) = window; return FALSE;
    }
    return TRUE;
  }, reinterpret_cast<LPARAM>(&result));
  return result;
}
std::wstring status(HWND window) {
  wchar_t value[256]{};
  GetWindowTextW(FindWindowExW(window, nullptr, L"STATIC", nullptr), value, 256);
  return value;
}
template<class Predicate> bool wait(Predicate predicate) {
  for (int i = 0; i < 600; ++i) { if (predicate()) return true; Sleep(20); }
  return false;
}
void capture(HWND window, const std::filesystem::path &path) {
  RECT rect{}; GetWindowRect(window, &rect);
  const int width = rect.right - rect.left, height = rect.bottom - rect.top;
  HDC screen = GetDC(nullptr), memory = CreateCompatibleDC(screen);
  BITMAPINFO info{}; info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  info.bmiHeader.biWidth = width; info.bmiHeader.biHeight = -height;
  info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
  void *pixels = nullptr;
  HBITMAP bitmap = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
  const auto previous = SelectObject(memory, bitmap);
  expect(PrintWindow(window, memory, 0) != FALSE, "原生窗口截图失败");
  BITMAPFILEHEADER file{}; file.bfType = 0x4d42;
  file.bfOffBits = sizeof(file) + sizeof(BITMAPINFOHEADER);
  file.bfSize = file.bfOffBits + static_cast<DWORD>(width * height * 4);
  std::ofstream output(path, std::ios::binary);
  output.write(reinterpret_cast<char *>(&file), sizeof(file));
  output.write(reinterpret_cast<char *>(&info.bmiHeader), sizeof(info.bmiHeader));
  output.write(static_cast<char *>(pixels), width * height * 4);
  expect(output.good(), "截图文件写入失败");
  SelectObject(memory, previous); DeleteObject(bitmap); DeleteDC(memory); ReleaseDC(nullptr, screen);
}
}

// 只替换云端传输边界，窗口、异步模型、确认和错误反馈均执行生产代码。
extern "C" char *msime_client_cloud_dictionary(const uint8_t *bytes, size_t length) {
  const auto request = Json::parse(bytes, bytes + length);
  const auto action = request.at("action");
  const auto operation = action.at("operation").get<std::string>();
  Json response;
  if (operation == "quick_clear_preview") {
    response = mode == 3 ? Json{{"ok", false}, {"error", "account_unauthorized"}} :
        Json{{"ok", true}, {"value", {{"token", token}, {"entries", mode == 2 ? 0 : 2}, {"revision", 7}}}};
  } else if (operation == "quick_clear_apply") {
    expect(action.at("token") == token, "确认未使用预览令牌"); ++writes;
    response = mode == 4 ? Json{{"ok", false}, {"error", "account_conflict"}} :
        Json{{"ok", true}, {"value", {{"cleared", 2}, {"revision", 8}}}};
  } else {
    expect(operation == "quick_clear_cancel", "出现非预期云端动作"); ++cancels;
    response = {{"ok", true}, {"value", nullptr}};
  }
  const auto text = response.dump();
  auto *result = new char[text.size() + 1]; std::memcpy(result, text.c_str(), text.size() + 1); return result;
}
extern "C" void msime_client_string_free(char *value) { delete[] value; }

int main(int argc, char **argv) {
  SetProcessDPIAware();
  for (int scenario = 0; scenario < 5; ++scenario) {
    mode = scenario; writes = 0; cancels = 0;
    std::thread driver([&] {
      HWND window = nullptr;
      if (!wait([&] { window = dialog(); return window != nullptr; })) { expect(false, "原生云端清空窗口没有打开"); return; }
      const auto apply = FindWindowExW(window, nullptr, L"BUTTON", L"确认清空云端快捷短语");
      const wchar_t *ready = scenario == 2 ? L"云端没有" : scenario == 3 ? L"请先" : L"将清空 2";
      expect(wait([&] { return status(window).find(ready) != std::wstring::npos; }), "预览条数或错误未显示");
      expect(IsWindowEnabled(apply) == (scenario != 2 && scenario != 3), "无条目或未登录仍允许确认");
      if (scenario == 0 && argc == 2) capture(window, std::filesystem::u8path(argv[1]) / "cloud-preview.bmp");
      if (scenario == 0 || scenario == 4) {
        SendMessageW(apply, BM_CLICK, 0, 0);
        const wchar_t *completed = scenario == 4 ? L"云端词库已变化" : L"已清空 2";
        expect(wait([&] { return status(window).find(completed) != std::wstring::npos; }), "提交结果未显示");
        expect(!IsWindowEnabled(apply), "提交后仍能复用旧预览");
        if (scenario == 0 && argc == 2) capture(window, std::filesystem::u8path(argv[1]) / "cloud-cleared.bmp");
      }
      SendMessageW(window, WM_CLOSE, 0, 0);
    });
    msime::settings::show_cloud_quick_phrase_clear(nullptr,
        Json{{"directory", "C:\\msime-synthetic-account"}, {"action", {{"operation", "status"}}}}.dump());
    driver.join();
    expect(writes == (scenario == 0 || scenario == 4 ? 1 : 0), "取消、无条目或未登录发起了写请求");
    expect(cancels == 1, "关闭窗口没有丢弃内存预览");
  }
  std::printf("原生云端快捷短语清空：5 个场景，%d 项失败\n", failures.load());
  return failures != 0;
}
