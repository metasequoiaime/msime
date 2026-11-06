#include "LocalTranslationWindow.h"
#include "../../common/LocalTranslationSettings.h"
#include <commctrl.h>
#include <commdlg.h>
#include <shellapi.h>
#include <memory>

namespace msime::settings {
namespace {
using Json = nlohmann::json;
enum Control { Enabled = 101, Language, Model, Status, Progress, Download, Import, Remove, Cancel, Source };
std::wstring wide(const std::string &text) {
  const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
  std::wstring result(static_cast<size_t>(length), L'\0');
  if (length) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), length);
  return result;
}
std::string utf8(const std::wstring &text) {
  const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  std::string result(static_cast<size_t>(length), '\0');
  if (length) WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), length, nullptr, nullptr);
  return result;
}
struct Window {
  explicit Window(const std::filesystem::path &root) : model(utf8(root.wstring())) {}
  msime::translation::SettingsModel model;
  HWND hwnd = nullptr;
  HFONT font = nullptr;
  bool closing = false;
  int dpi = 96;
  std::vector<std::string> languages;
  ~Window() { if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value, dpi, 96); }
  HWND control(int id) const { return GetDlgItem(hwnd, id); }
  void add(const wchar_t *type, const wchar_t *title, DWORD style, int id, int x, int y, int width, int height) {
    auto child = CreateWindowExW(0, type, title, WS_CHILD | WS_VISIBLE | style,
        px(x), px(y), px(width), px(height), hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), GetModuleHandleW(nullptr), nullptr);
    SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
  }
  void create(HWND window) {
    hwnd = window;
    HDC dc = GetDC(hwnd); dpi = GetDeviceCaps(dc, LOGPIXELSX); ReleaseDC(hwnd, dc);
    font = CreateFontW(-MulDiv(10, dpi, 72), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    RECT rect{0, 0, px(510), px(330)};
    AdjustWindowRectEx(&rect, static_cast<DWORD>(GetWindowLongPtrW(hwnd, GWL_STYLE)), FALSE, 0);
    SetWindowPos(hwnd, nullptr, 0, 0, rect.right - rect.left, rect.bottom - rect.top, SWP_NOMOVE | SWP_NOZORDER);
    add(L"BUTTON", L"启用本地翻译", BS_AUTOCHECKBOX | WS_TABSTOP, Enabled, 24, 20, 240, 30);
    add(L"STATIC", L"目标语言", 0, -1, 24, 65, 90, 26);
    add(L"COMBOBOX", L"目标语言", CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP, Language, 124, 61, 260, 360);
    add(L"STATIC", L"模型", 0, -1, 24, 108, 90, 24);
    add(L"STATIC", L"正在读取…", 0, Model, 124, 108, 365, 26);
    add(L"STATIC", L"正在读取…", 0, Status, 24, 150, 462, 40);
    add(PROGRESS_CLASSW, L"模型安装进度", 0, Progress, 24, 196, 462, 16);
    add(L"BUTTON", L"下载模型", BS_PUSHBUTTON | WS_TABSTOP, Download, 24, 232, 94, 30);
    add(L"BUTTON", L"从文件导入…", BS_PUSHBUTTON | WS_TABSTOP, Import, 128, 232, 114, 30);
    add(L"BUTTON", L"删除模型…", BS_PUSHBUTTON | WS_TABSTOP, Remove, 252, 232, 104, 30);
    add(L"BUTTON", L"取消", BS_PUSHBUTTON | WS_TABSTOP, Cancel, 366, 232, 90, 30);
    add(L"BUTTON", L"Built with PLaMo · 来源与许可", BS_PUSHBUTTON | WS_TABSTOP, Source, 24, 278, 380, 30);
    SendMessageW(control(Progress), PBM_SETRANGE32, 0, 100);
    SetTimer(hwnd, 1, 100, nullptr);
    update(false);
  }
  void update(bool values) {
    const auto &view = model.view;
    const bool loaded = view.contains("settings") && view.contains("catalog");
    const bool available = loaded && view.at("catalog").value("available", false);
    const bool installed = loaded && view.value("installed", false);
    if (values && loaded) {
      const auto &selection = view.at("settings").at("local_translation");
      SendMessageW(control(Enabled), BM_SETCHECK, selection.at("enabled").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
      SendMessageW(control(Language), CB_RESETCONTENT, 0, 0);
      languages.clear();
      for (const auto &language : view.at("catalog").at("languages")) {
        const auto name = wide(language.at("name").get<std::string>());
        const auto index = SendMessageW(control(Language), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(name.c_str()));
        languages.push_back(language.at("code").get<std::string>());
        if (language.at("code") == selection.at("target_language")) SendMessageW(control(Language), CB_SETCURSEL, static_cast<WPARAM>(index), 0);
      }
      const auto title = wide(view.at("catalog").at("model").get<std::string>()) + L" · " +
          std::to_wstring(view.at("catalog").at("size").get<uint64_t>() / 1'000'000) + L" MB";
      SetWindowTextW(control(Model), title.c_str());
    }
    const bool on = SendMessageW(control(Enabled), BM_GETCHECK, 0, 0) == BST_CHECKED;
    EnableWindow(control(Enabled), loaded && available && !model.busy() && (installed || on));
    EnableWindow(control(Language), loaded && available && !model.busy());
    EnableWindow(control(Download), available && !model.busy());
    EnableWindow(control(Import), available && !model.busy());
    EnableWindow(control(Remove), installed && !model.busy());
    ShowWindow(control(Cancel), model.installing() ? SW_SHOW : SW_HIDE);
    ShowWindow(control(Progress), model.installing() ? SW_SHOW : SW_HIDE);
    SendMessageW(control(Progress), PBM_SETPOS, static_cast<WPARAM>(model.progress()), 0);
    const wchar_t *status = model.busy() ? L"正在处理…" : !loaded ? L"无法读取设置，请重新打开窗口" :
        !available ? L"此构建未包含本地推理库" : installed ? L"已安装 · 翻译无需联网" : L"未安装";
    if (loaded && SendMessageW(control(Language), CB_GETCURSEL, 0, 0) == CB_ERR)
      status = L"当前目标语言不受此模型支持，请重新选择";
    if (!model.notice.empty()) {
      status = model.notice.find("cancelled") != std::string::npos ? L"已取消" :
          model.notice.find("checksum") != std::string::npos ? L"模型校验失败，请重新下载或导入" :
          model.notice.find("preferences changed") != std::string::npos ? L"设置已在其他窗口更新，请重新选择" : L"操作失败，请重试";
    }
    SetWindowTextW(control(Status), status);
  }
  void command(int id, int notification) {
    if (id == Source) { ShellExecuteW(hwnd, L"open", L"https://huggingface.co/pfnet/plamo-3-translate-0.8b-GGUF", nullptr, nullptr, SW_SHOWNORMAL); return; }
    if (id == Cancel) { model.cancel(); return; }
    if (model.busy()) return;
    if ((id == Enabled && notification == BN_CLICKED) || (id == Language && notification == CBN_SELCHANGE)) {
      const auto index = SendMessageW(control(Language), CB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < languages.size())
        model.save(SendMessageW(control(Enabled), BM_GETCHECK, 0, 0) == BST_CHECKED, languages[static_cast<size_t>(index)]);
      else if (id == Enabled && SendMessageW(control(Enabled), BM_GETCHECK, 0, 0) != BST_CHECKED && model.view.contains("settings"))
        model.save(false, model.view.at("settings").at("local_translation").at("target_language").get<std::string>());
    } else if (id == Download) model.install();
    else if (id == Import) {
      wchar_t path[32768]{};
      OPENFILENAMEW picker{}; picker.lStructSize = sizeof(picker); picker.hwndOwner = hwnd;
      picker.lpstrFilter = L"GGUF 模型\0*.gguf\0所有文件\0*.*\0\0";
      picker.lpstrFile = path; picker.nMaxFile = 32768;
      picker.lpstrTitle = L"导入 PLaMo 0.8B Q4_K_M 模型";
      picker.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
      if (GetOpenFileNameW(&picker)) model.install({utf8(path)});
    } else if (id == Remove && MessageBoxW(hwnd, L"删除后需重新下载或导入才能离线翻译。", L"删除本地翻译模型？", MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES) model.remove();
    update(false);
  }
};
INT_PTR CALLBACK procedure(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  auto *state = reinterpret_cast<Window *>(GetWindowLongPtrW(hwnd, DWLP_USER));
  try {
    if (message == WM_INITDIALOG) {
      state = reinterpret_cast<Window *>(lparam);
      SetWindowLongPtrW(hwnd, DWLP_USER, reinterpret_cast<LONG_PTR>(state));
      state->create(hwnd);
      return TRUE;
    }
    if (!state) return FALSE;
    if (message == WM_TIMER) {
      const bool changed = state->model.poll();
      state->update(changed);
      if (state->closing && !state->model.busy()) EndDialog(hwnd, 0);
      return TRUE;
    }
    if (message == WM_CLOSE || (message == WM_COMMAND && LOWORD(wparam) == IDCANCEL)) {
      state->closing = true; state->model.cancel();
      if (state->model.busy()) ShowWindow(hwnd, SW_HIDE); else EndDialog(hwnd, 0);
      return TRUE;
    }
    if (message == WM_COMMAND) { state->command(LOWORD(wparam), HIWORD(wparam)); return TRUE; }
  } catch (...) {
    SetWindowTextW(GetDlgItem(hwnd, Status), L"操作失败，请重试");
  }
  return FALSE;
}
}
void show_local_translation(HWND owner, const std::filesystem::path &directory) {
  if (!directory.is_absolute()) return;
  INITCOMMONCONTROLSEX controls{sizeof(controls), ICC_PROGRESS_CLASS}; InitCommonControlsEx(&controls);
  struct Template { DLGTEMPLATE dialog; WORD menu = 0, type = 0; wchar_t title[32] = L"本地模型翻译"; } data{};
  data.dialog.style = WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME;
  data.dialog.cx = 300; data.dialog.cy = 185;
  try {
    Window state(directory);
    DialogBoxIndirectParamW(GetModuleHandleW(nullptr), &data.dialog, owner, procedure, reinterpret_cast<LPARAM>(&state));
  } catch (...) { MessageBoxW(owner, L"无法打开本地翻译设置，请重试。", L"本地模型翻译", MB_OK | MB_ICONERROR); }
}
} // namespace msime::settings
