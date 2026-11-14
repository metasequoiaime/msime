#include "DictionaryCollectionsWindow.h"
#include "../../common/DictionaryCollectionsModel.h"
#include <commdlg.h>
#include <wincrypt.h>
#include <array>
#include <filesystem>
#include <fstream>
#include <set>

namespace msime::settings {
namespace {
using Json = nlohmann::json;
enum Control { Entries = 101, Name, Kind, Format, Target, Create, Rename, Import, Enable, Disable, Remove, Refresh, Status };
std::wstring wide(const std::string &text) {
  const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
  std::wstring value(static_cast<size_t>(count), L'\0');
  if (count) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), value.data(), count);
  return value;
}
std::string utf8(const std::wstring &text) {
  const int count = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  std::string value(static_cast<size_t>(count), '\0');
  if (count) WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), value.data(), count, nullptr, nullptr);
  return value;
}
struct Window {
  explicit Window(Json options) : model(std::move(options)) {}
  msime::dictionary_collections::Model model;
  HWND hwnd = nullptr;
  HFONT font = nullptr;
  int dpi = 96;
  bool closing = false;
  std::vector<std::string> rows, targets;
  const std::array<const char *, 7> kinds{{"pinyin", "wubi", "wubi98", "wubixsj", "english", "quickPhrase", "cantonese"}};
  const std::array<const char *, 5> formats{{"standard", "windows", "rime", "hans", "scel"}};
  ~Window() { if (font) DeleteObject(font); }
  HWND control(int id) const { return GetDlgItem(hwnd, id); }
  std::string text(int id) const {
    const int count = GetWindowTextLengthW(control(id));
    if (count < 0 || count > 1024) return {};
    std::wstring value(static_cast<size_t>(count) + 1, L'\0');
    GetWindowTextW(control(id), value.data(), count + 1); value.resize(static_cast<size_t>(count));
    return utf8(value);
  }
  size_t choice(int id) const {
    const auto selected = SendMessageW(control(id), CB_GETCURSEL, 0, 0);
    return selected < 0 ? 0 : static_cast<size_t>(selected);
  }
  std::vector<std::string> selected() const {
    std::vector<int> indexes(rows.size());
    const auto count = SendMessageW(control(Entries), LB_GETSELITEMS, indexes.size(), reinterpret_cast<LPARAM>(indexes.data()));
    std::vector<std::string> ids;
    for (LRESULT i = 0; i < count; ++i) {
      const auto index = static_cast<size_t>(indexes[static_cast<size_t>(i)]);
      if (index < rows.size()) ids.push_back(rows[index]);
    }
    return ids;
  }
  void add(const wchar_t *type, const wchar_t *title, DWORD style, int id, int x, int y, int width, int height) {
    const auto px = [&](int value) { return MulDiv(value, dpi, 96); };
    auto child = CreateWindowExW(0, type, title, WS_CHILD | WS_VISIBLE | style,
        px(x), px(y), px(width), px(height), hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), GetModuleHandleW(nullptr), nullptr);
    SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
  }
  void create(HWND window) {
    hwnd = window; HDC dc = GetDC(hwnd); dpi = GetDeviceCaps(dc, LOGPIXELSX); ReleaseDC(hwnd, dc);
    font = CreateFontW(-MulDiv(10, dpi, 72), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    RECT rect{0, 0, MulDiv(740, dpi, 96), MulDiv(530, dpi, 96)};
    AdjustWindowRectEx(&rect, static_cast<DWORD>(GetWindowLongPtrW(hwnd, GWL_STYLE)), FALSE, 0);
    SetWindowPos(hwnd, nullptr, 0, 0, rect.right - rect.left, rect.bottom - rect.top, SWP_NOMOVE | SWP_NOZORDER);
    add(L"LISTBOX", L"词库分组", WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_EXTENDEDSEL | LBS_NOINTEGRALHEIGHT, Entries, 20, 20, 700, 270);
    add(L"STATIC", L"分组名称", 0, -1, 20, 306, 85, 26);
    add(L"EDIT", L"", WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL, Name, 110, 302, 610, 28);
    SendMessageW(control(Name), EM_SETLIMITTEXT, 1024, 0);
    add(L"STATIC", L"词库类型", 0, -1, 20, 346, 85, 26);
    add(L"COMBOBOX", L"词库类型", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, Kind, 110, 342, 180, 250);
    add(L"COMBOBOX", L"文件格式", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, Format, 310, 342, 180, 220);
    add(L"COMBOBOX", L"导入目标", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, Target, 510, 342, 210, 350);
    for (const auto *title : {L"拼音", L"86 五笔", L"98 五笔", L"新世纪五笔", L"英文", L"快捷短语", L"粤语"})
      SendMessageW(control(Kind), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    for (const auto *title : {L"词在前（TSV）", L"编码在前（TSV）", L"Rime", L"汉字词表", L"搜狗细胞词库"})
      SendMessageW(control(Format), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    SendMessageW(control(Kind), CB_SETCURSEL, 0, 0); SendMessageW(control(Format), CB_SETCURSEL, 0, 0);
    int x = 20;
    for (auto item : {std::pair{Create, L"新建"}, {Rename, L"改名"}, {Import, L"导入…"}, {Enable, L"启用"},
        {Disable, L"停用"}, {Remove, L"删除…"}, {Refresh, L"刷新"}}) {
      add(L"BUTTON", item.second, WS_TABSTOP | BS_PUSHBUTTON, item.first, x, 394, 92, 32); x += 101;
    }
    add(L"STATIC", L"正在读取…", 0, Status, 20, 450, 700, 62);
    SetTimer(hwnd, 1, 100, nullptr); update(true);
  }
  void update(bool rebuild) {
    if (rebuild) {
      const auto ids = selected(); const std::set<std::string> preserved(ids.begin(), ids.end());
      SendMessageW(control(Entries), LB_RESETCONTENT, 0, 0); rows.clear();
      SendMessageW(control(Target), CB_RESETCONTENT, 0, 0); targets = {""};
      SendMessageW(control(Target), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"新建分组（默认文件名）"));
      for (const auto *source : {"user", "import", "community"}) for (const auto &item : model.view.at("collections")) {
        if (item.at("source").at("type") != source) continue;
        const auto id = item.at("id").get<std::string>();
        const auto title = wide(std::string(msime::dictionary_collections::Model::source_title(source)) + " · " +
            item.at("name").get<std::string>() + " · " + std::to_string(item.at("entry_count").get<unsigned>()) + " 条 · " +
            (item.at("enabled").get<bool>() ? "已启用" : "已停用"));
        const auto index = SendMessageW(control(Entries), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
        rows.push_back(id); if (preserved.count(id)) SendMessageW(control(Entries), LB_SETSEL, TRUE, index);
        if (item.at("kind") == kinds.at(choice(Kind)) && std::string(source) != "community") {
          targets.push_back(id); const auto label = wide(item.at("name").get<std::string>());
          SendMessageW(control(Target), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
        }
      }
      SendMessageW(control(Target), CB_SETCURSEL, 0, 0);
    }
    for (int id = Entries; id < Status; ++id) EnableWindow(control(id), !model.busy());
    std::string message = model.busy() ? "正在处理…" : msime::dictionary_collections::Model::message(model.error);
    if (!model.busy() && model.error.empty() && model.report.is_object()) {
      message = "导入 " + std::to_string(model.report.value("imported", 0)) + " 条，跳过重复 " +
          std::to_string(model.report.value("duplicates", 0)) + " 条，失败 " + std::to_string(model.report.value("failed", 0)) + " 条";
      if (model.report.value("truncated", false)) message += "；分组已满";
    }
    SetWindowTextW(control(Status), wide(message).c_str());
  }
  void perform(Json action) { model.perform(std::move(action)); update(false); }
  void import_file() {
    std::array<wchar_t, 32768> path{};
    OPENFILENAMEW picker{}; picker.lStructSize = sizeof(picker); picker.hwndOwner = hwnd;
    picker.lpstrFile = path.data(); picker.nMaxFile = static_cast<DWORD>(path.size());
    picker.lpstrFilter = L"词库文件\0*.txt;*.tsv;*.yaml;*.yml;*.scel\0所有文件\0*.*\0";
    picker.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if (!GetOpenFileNameW(&picker)) return;
    const auto format = formats.at(choice(Format));
    const size_t limit = std::string(format) == "scel" ? 8 * 1024 * 1024 : 16 * 1024 * 1024;
    std::ifstream input(std::filesystem::path(path.data()), std::ios::binary);
    if (!input.is_open()) { SetWindowTextW(control(Status), L"文件无法读取，请重新选择"); return; }
    std::string bytes(limit + 1, '\0'); input.read(bytes.data(), static_cast<std::streamsize>(bytes.size()));
    bytes.resize(static_cast<size_t>(input.gcount()));
    if (input.bad() || bytes.size() > limit) { SetWindowTextW(control(Status), L"文件无法读取或超出容量限制"); return; }
    std::string group = text(Name);
    if (group.empty()) group = utf8(std::filesystem::path(path.data()).stem().wstring());
    Json action{{"operation", "import"}, {"format", format}, {"kind", kinds.at(choice(Kind))}, {"name", group}};
    if (choice(Target) < targets.size() && !targets[choice(Target)].empty()) action["id"] = targets[choice(Target)];
    if (std::string(format) == "scel") {
      DWORD size = 0;
      if (!CryptBinaryToStringA(reinterpret_cast<const BYTE *>(bytes.data()), static_cast<DWORD>(bytes.size()),
          CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, nullptr, &size)) return;
      std::string encoded(size, '\0');
      if (!CryptBinaryToStringA(reinterpret_cast<const BYTE *>(bytes.data()), static_cast<DWORD>(bytes.size()),
          CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, encoded.data(), &size)) return;
      encoded.resize(size); action["bytes_base64"] = encoded;
    } else action["text"] = bytes;
    perform(std::move(action));
  }
  void command(int id, int notification) {
    if (model.busy()) return;
    if (id == Kind && notification == CBN_SELCHANGE) { update(true); return; }
    if (notification != BN_CLICKED) return;
    if (id == Refresh) { perform({{"operation", "load"}}); return; }
    if (id == Create) { perform({{"operation", "create"}, {"name", text(Name)}, {"kind", kinds.at(choice(Kind))}}); return; }
    if (id == Import) { import_file(); return; }
    const auto ids = selected();
    if (ids.empty()) { SetWindowTextW(control(Status), L"请先选择词库"); return; }
    if (id == Rename) {
      if (ids.size() == 1) perform({{"operation", "rename"}, {"id", ids[0]}, {"name", text(Name)}});
      else SetWindowTextW(control(Status), L"改名时请选择一个词库");
    } else if (id == Enable || id == Disable) perform({{"operation", "set_enabled_many"}, {"ids", ids}, {"enabled", id == Enable}});
    else if (id == Remove && MessageBoxW(hwnd, L"删除所选分组及其词条？其他已启用分组和原有个人词条会保留。", L"删除词库", MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2) == IDYES)
      perform({{"operation", "delete_many"}, {"ids", ids}});
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
      const bool changed = state->model.poll(); state->update(changed);
      if (state->closing && !state->model.busy()) EndDialog(hwnd, 0);
      return TRUE;
    }
    if (message == WM_CLOSE || (message == WM_COMMAND && LOWORD(wparam) == IDCANCEL)) {
      state->closing = true;
      if (state->model.busy()) ShowWindow(hwnd, SW_HIDE); else EndDialog(hwnd, 0);
      return TRUE;
    }
    if (message == WM_COMMAND) { state->command(LOWORD(wparam), HIWORD(wparam)); return TRUE; }
  } catch (...) { SetWindowTextW(GetDlgItem(hwnd, Status), L"操作失败，请重试"); }
  return FALSE;
}
}
void show_dictionary_collections(HWND owner, const std::string &options) {
  struct Template { DLGTEMPLATE dialog; WORD menu = 0, type = 0; wchar_t title[32] = L"词库分组"; } data{};
  data.dialog.style = WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME;
  data.dialog.cx = 430; data.dialog.cy = 310;
  try {
    Window state(Json::parse(options));
    DialogBoxIndirectParamW(GetModuleHandleW(nullptr), &data.dialog, owner, procedure, reinterpret_cast<LPARAM>(&state));
  } catch (...) { MessageBoxW(owner, L"词库还没准备好，请完成首次设置后重试。", L"词库分组", MB_OK | MB_ICONERROR); }
}
} // namespace msime::settings
