#include "../../panels/win7/VocabularyWindow.h"
#include "../../settings/win7/VocabularyForm.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include <cassert>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <functional>

namespace {
using namespace msime::settings::win7;
using namespace msime::windows::win7;
struct Environment {
  const wchar_t* name;
  std::wstring previous;
  bool present = false;
  Environment(const wchar_t* key, const std::filesystem::path& value) : name(key) {
    const auto size = GetEnvironmentVariableW(name, nullptr, 0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(name, previous.data(), size)); }
    assert(SetEnvironmentVariableW(name, value.c_str()));
  }
  ~Environment() { SetEnvironmentVariableW(name, present ? previous.c_str() : nullptr); }
};
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() {
    wchar_t root[MAX_PATH]{}, unique[MAX_PATH]{};
    assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7v", 0, unique));
    assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); path = unique;
  }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path, error); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message);
    }
    if (condition()) return;
    assert(GetTickCount64() < deadline);
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
  }
}
std::string read(const std::filesystem::path& path) {
  std::ifstream input(path, std::ios::binary); assert(input.good());
  return std::string(std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>());
}
void write(const std::filesystem::path& path, const std::string& value) {
  std::ofstream output(path, std::ios::binary); output << value; assert(output.good());
}
}

int main() {
  TemporaryState temporary;
  Environment state(L"MSIME_CLIENT_STATE_DIR", temporary.path);
  const auto options_path = temporary.path / L"runtime-options.json";
  Environment options(L"MSIME_CLIENT_HOST_OPTIONS", options_path);
  std::filesystem::create_directories(temporary.path / L"resources");
  write(options_path, Json{{"resources", (temporary.path / L"resources").u8string()}}.dump());
  const auto import_path = temporary.path / L"合成词表.csv";
  write(import_path, "alpha,/alfa/,合成释义🙂\nbeta,合成释义二\n");
  PreferencesStore preferences(temporary.path.u8string());
  const auto before = preferences.load(); assert(before.snapshot);
  bool allow = false, cancel_picker = false;
  unsigned picks = 0, confirms = 0;
  VocabularyWindow window([&](HWND, bool save) -> std::optional<std::filesystem::path> {
    assert(!save); ++picks; return cancel_picker ? std::nullopt : std::make_optional(import_path);
  }, [&](HWND, const wchar_t* title, const wchar_t* message) {
    assert(std::wstring_view(title) == L"背单词" && message && *message); ++confirms; return allow;
  });
  assert(window.create(GetModuleHandleW(nullptr)));
  const auto host = window.handle(), body = GetDlgItem(host, vocabulary_panel_body);
  const auto wait = [&] { pump([&] { return IsWindowEnabled(GetDlgItem(body, VocabularyForm::refresh_button)) != FALSE; }); }; wait();
  SendMessageW(body, WM_VSCROLL, SB_BOTTOM, 0); const auto bottom = GetScrollPos(body, SB_VERT); assert(bottom > 0);
  SendMessageW(body, WM_VSCROLL, SB_ENDSCROLL, 0); assert(GetScrollPos(body, SB_VERT) == bottom);
  SendMessageW(body, WM_VSCROLL, SB_TOP, 0); assert(GetScrollPos(body, SB_VERT) == 0);
  const auto control = [&](int id) { return GetDlgItem(body, id); };
  const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
  const auto text = [&](int id) { return control_text::read(control(id)).value(); };
  const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(control(id), value)); };
  assert(!IsWindowEnabled(control(VocabularyForm::start_review)));
  cancel_picker = true; click(VocabularyForm::import_book); assert(picks == 1);
  cancel_picker = false; click(VocabularyForm::import_book);
  assert(SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0) == 1);
  assert(IsWindowEnabled(control(VocabularyForm::start_review)) && text(VocabularyForm::notice).empty());
  set(VocabularyForm::daily, L"2"); set(VocabularyForm::session_limit, L"2"); click(VocabularyForm::save_settings);
  click(VocabularyForm::start_review);
  assert(text(VocabularyForm::card).find("alpha") != std::string::npos && text(VocabularyForm::card).find("合成释义") == std::string::npos);
  assert(!IsWindowEnabled(control(VocabularyForm::known)));
  click(VocabularyForm::reveal); assert(text(VocabularyForm::card).find("合成释义🙂") != std::string::npos);
  click(VocabularyForm::known); assert(text(VocabularyForm::card).find("beta") != std::string::npos);
  click(VocabularyForm::reveal); click(VocabularyForm::unknown);
  assert(text(VocabularyForm::card).find("beta") != std::string::npos && !IsWindowEnabled(control(VocabularyForm::known)));
  click(VocabularyForm::reveal); click(VocabularyForm::known);
  assert(text(VocabularyForm::card).find("本次复习已完成") != std::string::npos);
  click(VocabularyForm::refresh_button); assert(text(VocabularyForm::card).find("本次复习已完成") != std::string::npos);
  const auto progress = temporary.path / L"vocabulary-progress.json";
  const auto saved = read(progress);
  click(VocabularyForm::reset_progress); assert(confirms == 1 && read(progress) == saved);
  allow = true; click(VocabularyForm::reset_progress);
  assert(SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0) == 1);
  click(VocabularyForm::start_review); assert(text(VocabularyForm::card).find("alpha") != std::string::npos);
  // 坏进度文件不能被刷新、作答或清空按钮静默重置。
  const auto clean = read(progress); write(progress, "{synthetic-invalid");
  click(VocabularyForm::refresh_button);
  assert(!IsWindowEnabled(control(VocabularyForm::start_review)) && !IsWindowEnabled(control(VocabularyForm::known)));
  click(VocabularyForm::reset_progress); assert(read(progress) == "{synthetic-invalid");
  write(progress, clean); click(VocabularyForm::refresh_button);
  const auto count = SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0);
  allow = false; click(VocabularyForm::remove_book);
  assert(SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0) == count);
  allow = true; click(VocabularyForm::remove_book);
  assert(SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0) == 0 && !IsWindowEnabled(control(VocabularyForm::start_review)));
  // 超限文件拒绝，真实存储中不产生部分词表。
  { std::ofstream output(import_path, std::ios::binary); output.seekp(1048576); output.put('x'); assert(output.good()); }
  click(VocabularyForm::import_book);
  assert(SendMessageW(control(VocabularyForm::books), CB_GETCOUNT, 0, 0) == 0 && !text(VocabularyForm::notice).empty());
  // UTF-16 词表保留代理对，不改共享偏好默认值。
  { std::ofstream output(import_path, std::ios::binary); const std::wstring record = L"gamma,合成🙂\r\n";
    output.put(static_cast<char>(0xff)); output.put(static_cast<char>(0xfe)); output.write(reinterpret_cast<const char*>(record.data()), static_cast<std::streamsize>(record.size() * sizeof(wchar_t))); assert(output.good()); }
  click(VocabularyForm::import_book); click(VocabularyForm::start_review); click(VocabularyForm::reveal);
  assert(text(VocabularyForm::card).find("合成🙂") != std::string::npos);
  assert(preferences.load().snapshot->revision() == before.snapshot->revision());
  DestroyWindow(host);
  std::puts("Windows 7 vocabulary panel: real imports, schedule, HWND reveal/answers, confirmations and damaged-file preservation passed");
}
