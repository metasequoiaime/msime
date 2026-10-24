#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/AboutForm.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <cassert>
#include <cstdio>
#include <fstream>

namespace {
using namespace msime::settings::win7;
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) { MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT); }
}
}
int main() {
  wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7a", 0, file));
  assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr)); const std::filesystem::path path(file);
  const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); std::wstring previous(length, L'\0');
  if (length) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), length));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", path.c_str()));
  { std::ofstream edition(path / L"edition", std::ios::binary); edition << MSIME_EDITION_ID; assert(edition.good()); }
  PreferencesStore store(path.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  std::string edition = MSIME_EDITION_ID; edition[0] = static_cast<char>(std::toupper(static_cast<unsigned char>(edition[0])));
  const auto architecture = msime::settings::build_architecture() == L"x86_64" ? "x64" : "x86";
  const auto name = "MetasequoiaIME-" + edition + "_Setup_v999.0.0_win7_" + architecture + ".exe";
  Json response{{"ok", true}, {"value", {{"status", "none"}}}};
  unsigned requests = 0, copies = 0;
  const auto thread = GetCurrentThreadId(); std::wstring copied;
  SettingsWindow::ClipboardServices clipboard; clipboard.copy = [&](HWND, std::wstring_view value) { ++copies; copied = value; return true; };
  {
    SettingsWindow window({}, {}, {}, {}, {}, {}, clipboard, {}, {}, [&](const std::string& bytes) {
      assert(GetCurrentThreadId() != thread); const auto request = Json::parse(bytes); ++requests;
      assert(request.at("arch") == control_text::utf8(msime::settings::build_architecture()).value());
      assert(request.at("platform") == "windows7" && request.at("edition") == MSIME_EDITION_ID && !request.at("current_version").get<std::string>().empty());
      return response.dump();
    });
    assert(window.open_route("settings:about") && window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host, page_body), selector = GetDlgItem(host, page_selector);
    const auto wait = [&] { pump([&] { return SendMessageW(selector, CB_GETCURSEL, 0, 0) == 16 && IsWindowEnabled(GetDlgItem(body, AboutForm::check_update)); }); }; wait();
    const auto control = [&](int id) { const auto result = GetDlgItem(body, id); assert(result); return result; };
    const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
    const auto text = [&](int id) { return control_text::read(control(id)).value(); };
    assert(!IsWindowEnabled(control(AboutForm::download)) && requests == 0);
    assert(store.load().snapshot->revision() == initial.snapshot->revision());
    click(AboutForm::check_update); assert(requests == 1 && text(AboutForm::update_status).find("暂无可用") != std::string::npos);
    Json update{{"version", {{"display", "999.0.0"}, {"parts", {999u, 0u, 0u}}}}, {"release_url", "https://github.com/metasequoiaime/msime/releases/tag/windows7-v999.0.0"},
        {"installer_name", name}, {"installer_sha256", std::string(64, 'a')}, {"signed", false}};
    response["value"] = {{"status", "available"}, {"update", update}};
    click(AboutForm::check_update); assert(IsWindowEnabled(control(AboutForm::download)) && IsWindowEnabled(control(AboutForm::mirror)));
    assert(text(AboutForm::update_detail).find(name) != std::string::npos && text(AboutForm::update_detail).find("certutil -hashfile") != std::string::npos);
    assert(text(AboutForm::update_detail).find(std::string(64, 'a')) != std::string::npos);
    response["value"]["update"]["installer_name"] = "MetasequoiaIME-" + edition + "_Setup_v999.0.0_win7_" + (std::string(architecture) == "x64" ? "x86" : "x64") + ".exe";
    click(AboutForm::check_update); assert(!IsWindowEnabled(control(AboutForm::download)) && text(AboutForm::update_detail).empty());
    response["value"]["update"]["installer_name"] = "MetasequoiaIME-Full_Setup_v999.0.0.exe";
    click(AboutForm::check_update); assert(!IsWindowEnabled(control(AboutForm::download)) && text(AboutForm::update_detail).empty());
    response["value"]["update"] = update; response["value"]["status"] = "current";
    click(AboutForm::check_update); assert(text(AboutForm::update_status).find("已是最新") != std::string::npos && !IsWindowEnabled(control(AboutForm::download)));
    response = {{"ok", false}, {"error", "synthetic_unavailable"}};
    click(AboutForm::check_update); assert(text(AboutForm::update_status).find("检查失败") != std::string::npos);
    click(AboutForm::copy_system); assert(copies == 1 && copied.find(L"水杉 IME 系统信息") != std::wstring::npos && copied.find(path.wstring()) == std::wstring::npos);
    assert(SetWindowTextW(control(AboutForm::feedback_detail), L"合成🙂\r\n第二行"));
    click(AboutForm::copy_feedback); assert(copies == 2 && copied.find(L"合成🙂\n第二行") != std::wstring::npos && copied.find(path.wstring()) == std::wstring::npos);
    const auto old = store.load(); const bool reporting = old.snapshot->preferences().at("usage_reporting").get<bool>();
    click(AboutForm::usage_reporting); assert(store.load().snapshot->preferences().at("usage_reporting") == !reporting);
    assert(window.open_route("settings:help") && GetScrollPos(body, SB_VERT) > 0);
    assert(window.open_route("settings:feedback") && GetScrollPos(body, SB_VERT) > 0);
    DestroyWindow(host);
  }
  SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", length ? previous.c_str() : nullptr);
  std::error_code error; std::filesystem::remove_all(path, error);
  std::puts("Windows 7 about: dedicated update requests, installer refusal, stale-result clearing, system/report copies, privacy preference and help routing passed");
}
