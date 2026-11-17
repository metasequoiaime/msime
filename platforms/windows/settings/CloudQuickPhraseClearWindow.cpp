#include "CloudQuickPhraseClearWindow.h"
#include "../../common/CloudQuickPhraseClearModel.h"
#include <filesystem>

namespace msime::settings {
namespace {
using Json = nlohmann::json;
enum Control { Status = 101, Preview, Apply };
std::wstring wide(const std::string &text) {
  const int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
  std::wstring result(static_cast<size_t>(size), L'\0');
  if (size) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), size);
  return result;
}
struct Window {
  explicit Window(Json envelope) : model(std::move(envelope)) {}
  cloud_quick_clear::Model model;
  HWND hwnd = nullptr;
  HFONT font = nullptr;
  bool closing = false;
  std::string token;
  ~Window() { if (font) DeleteObject(font); }
  void update() {
    EnableWindow(GetDlgItem(hwnd, Preview), !model.busy() && !closing);
    EnableWindow(GetDlgItem(hwnd, Apply), !model.busy() && !closing && !token.empty());
  }
  void create(HWND handle) {
    hwnd = handle;
    HDC dc = GetDC(hwnd); const int dpi = GetDeviceCaps(dc, LOGPIXELSX); ReleaseDC(hwnd, dc);
    font = CreateFontW(-MulDiv(10, dpi, 72), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    for (const auto id : {Status, Preview, Apply}) {
      const auto style = id == Status ? SS_LEFT : WS_TABSTOP | BS_PUSHBUTTON;
      HWND control = CreateWindowExW(0, id == Status ? L"STATIC" : L"BUTTON",
          id == Status ? L"正在读取云端快捷短语…" : id == Preview ? L"重新预览" : L"确认清空云端快捷短语",
          WS_VISIBLE | WS_CHILD | style, MulDiv(16, dpi, 96), MulDiv(id == Status ? 16 : id == Preview ? 88 : 128, dpi, 96),
          MulDiv(360, dpi, 96), MulDiv(id == Status ? 64 : 32, dpi, 96), hwnd,
          reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), GetModuleHandleW(nullptr), nullptr);
      SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
    }
    RECT frame{0, 0, MulDiv(392, dpi, 96), MulDiv(178, dpi, 96)};
    AdjustWindowRectEx(&frame, static_cast<DWORD>(GetWindowLongPtrW(hwnd, GWL_STYLE)), FALSE, 0);
    SetWindowPos(hwnd, nullptr, 0, 0, frame.right - frame.left, frame.bottom - frame.top, SWP_NOMOVE | SWP_NOZORDER);
    model.preview(); update(); SetTimer(hwnd, 1, 100, nullptr);
  }
  void poll() {
    if (!model.poll()) return;
    std::wstring status;
    if (!model.error.empty()) { token.clear(); status = wide(cloud_quick_clear::Model::message(model.error)); }
    else if (model.operation == "quick_clear_preview") {
      const auto entries = model.value.at("entries").get<uint64_t>();
      token = entries > 0 ? model.value.at("token").get<std::string>() : std::string{};
      status = entries == 0 ? L"云端没有可清空的快捷短语。" :
          L"将清空 " + std::to_wstring(entries) + L" 条云端快捷短语；其他词库保留。此操作无法撤销。";
    } else if (model.operation == "quick_clear_apply") {
      token.clear(); status = L"已清空 " + std::to_wstring(model.value.at("cleared").get<uint64_t>()) + L" 条云端快捷短语。";
    }
    if (!status.empty()) SetWindowTextW(GetDlgItem(hwnd, Status), status.c_str());
    update();
  }
};
INT_PTR CALLBACK procedure(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  auto *state = reinterpret_cast<Window *>(GetWindowLongPtrW(hwnd, DWLP_USER));
  try {
    if (message == WM_INITDIALOG) {
      state = reinterpret_cast<Window *>(lparam); SetWindowLongPtrW(hwnd, DWLP_USER, lparam); state->create(hwnd); return TRUE;
    }
    if (!state) return FALSE;
    if (message == WM_TIMER) {
      state->poll();
      if (state->closing && !state->model.busy()) {
        if (state->model.operation == "quick_clear_cancel") EndDialog(hwnd, 0); else state->model.cancel();
      }
      return TRUE;
    }
    if (message == WM_CLOSE || (message == WM_COMMAND && LOWORD(wparam) == IDCANCEL)) {
      state->closing = true; state->token.clear();
      ShowWindow(hwnd, SW_HIDE);
      if (!state->model.busy()) state->model.cancel();
      return TRUE;
    }
    if (message == WM_COMMAND && !state->model.busy() && !state->closing) {
      if (LOWORD(wparam) == Preview) { state->token.clear(); state->model.preview(); }
      else if (LOWORD(wparam) == Apply && !state->token.empty()) { state->model.apply(state->token); state->token.clear(); }
      state->update(); return TRUE;
    }
  } catch (...) { SetWindowTextW(GetDlgItem(hwnd, Status), L"无法读取云端结果，请重新预览。"); }
  return FALSE;
}
}
void show_cloud_quick_phrase_clear(HWND owner, const std::string &account) {
  struct Template { DLGTEMPLATE dialog; WORD menu = 0, type = 0; wchar_t title[32] = L"清空云端快捷短语"; } data{};
  data.dialog.style = WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME;
  data.dialog.cx = 240; data.dialog.cy = 110;
  try {
    auto envelope = Json::parse(account); envelope.erase("client_agent");
    const auto directory = std::filesystem::u8path(envelope.at("directory").get<std::string>());
    envelope["snapshot_directory"] = (directory / ("native-quick-clear-" + std::to_string(GetCurrentProcessId()))).u8string();
    Window state(std::move(envelope));
    DialogBoxIndirectParamW(GetModuleHandleW(nullptr), &data.dialog, owner, procedure, reinterpret_cast<LPARAM>(&state));
  } catch (...) { MessageBoxW(owner, L"无法读取账号目录，请重新打开设置。", L"云端快捷短语", MB_OK | MB_ICONERROR); }
}
} // 命名空间 msime::settings
