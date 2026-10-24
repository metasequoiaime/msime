#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ThemeData.h"
#include "../../settings/win7/CommunitySkinData.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/StatisticsData.h"
#include "../../settings/InputSettingsModel.h"
#include "../../../../shared/contracts/msime_edition.h"

#include <commctrl.h>
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <string>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>

namespace {
using namespace msime::settings::win7;

struct TemporaryDirectory {
  TemporaryDirectory() {
    wchar_t root[MAX_PATH]{};
    wchar_t unique[MAX_PATH]{};
    const DWORD length = GetTempPathW(MAX_PATH, root);
    assert(length && length < MAX_PATH);
    assert(GetTempFileNameW(root, L"m7s", 0, unique));
    assert(DeleteFileW(unique));
    assert(CreateDirectoryW(unique, nullptr));
    path = unique;
  }
  ~TemporaryDirectory() {
    std::error_code error;
    std::filesystem::remove_all(path, error);
  }
  std::filesystem::path path;
};

struct InjectedDirectory {
  explicit InjectedDirectory(const std::filesystem::path& path, const wchar_t* variable = L"MSIME_CLIENT_STATE_DIR") : name(variable) {
    const DWORD required = GetEnvironmentVariableW(name, nullptr, 0);
    present = required != 0;
    if (present) {
      previous.resize(required);
      const DWORD length = GetEnvironmentVariableW(name, previous.data(), required);
      assert(length < required);
      previous.resize(length);
    }
    assert(SetEnvironmentVariableW(name, path.c_str()));
  }
  ~InjectedDirectory() {
    SetEnvironmentVariableW(name, present ? previous.c_str() : nullptr);
  }
  const wchar_t* name;
  bool present = false;
  std::wstring previous;
};

void pump_until(const std::function<bool()>& condition) {
  const ULONGLONG deadline = GetTickCount64() + 15000;
  for (;;) {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      assert(message.message != WM_QUIT);
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
    if (condition()) return;
    assert(GetTickCount64() < deadline);
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 25, QS_ALLINPUT);
  }
}

std::wstring caption(HWND window) {
  std::wstring value(static_cast<size_t>(GetWindowTextLengthW(window)) + 1, L'\0');
  const int copied = GetWindowTextW(window, value.data(), static_cast<int>(value.size()));
  value.resize(static_cast<size_t>(copied));
  return value;
}

} // namespace

int main() {
#ifdef _MSC_VER
  _set_error_mode(_OUT_TO_STDERR);
#endif
  using namespace msime::settings::win7;
  TemporaryDirectory temporary;
  InjectedDirectory injected(temporary.path);
#if !MSIME_EDITION_IS_FULL
  // 模拟安装准备阶段写入的状态目录身份；偏好按目录记录取默认值，不按进程包声明取。
  {
    std::ofstream record(temporary.path / L"edition", std::ios::binary);
    record << MSIME_EDITION_ID << '\n';
    assert(record.good());
  }
#endif
  PreferencesStore store(temporary.path.u8string());
  auto loaded = store.load();
  assert(loaded.outcome == Outcome::loaded && loaded.snapshot);
  // Rust 按状态目录的版本记录选默认值，必须与原生编译期产品身份一致。
  assert(loaded.snapshot->preferences().at("scheme") == MSIME_EDITION_DEFAULT_SCHEME);
  auto tokens = loaded.snapshot->preferences().at("ai_assistant").at("tokens");
  tokens["https://compat.example.invalid"] = "synthetic-token-not-a-real-credential";
  auto seeded = store.save(*loaded.snapshot, {
      {{"ai_assistant", "tokens"}, tokens},
      {{"navigation", "tab"}, false}, {{"navigation", "page_up_down"}, false}});
  assert(seeded.outcome == Outcome::saved && seeded.snapshot);
  auto peer = store.load();
  assert(peer.snapshot && peer.snapshot->revision() == seeded.snapshot->revision());
  const auto unchanged = seeded.snapshot->document;
  auto changed = store.save(*seeded.snapshot, {{{"keybindings", "switch_language_caps_lock"}, true}});
  assert(changed.outcome == Outcome::saved && changed.snapshot);
  assert(seeded.snapshot->document == unchanged);
  auto conflict = store.save(*peer.snapshot, {{{"keybindings", "switch_language_caps_lock"}, false}});
  assert(conflict.outcome == Outcome::conflict && !conflict.snapshot);
  assert(store.load().snapshot->revision() == changed.snapshot->revision());
  const auto exported = store.export_document();
  assert(exported && exported->find("synthetic-token-not-a-real-credential") == std::string::npos);
  auto local_only = store.save(*changed.snapshot, {{{"keybindings", "switch_language_caps_lock"}, false},
      {{"diagnostic_log", "server"}, true}});
  assert(local_only.outcome == Outcome::saved && local_only.snapshot);
  auto restored = store.import_document(*local_only.snapshot, *exported);
  assert(restored.outcome == Outcome::saved && restored.snapshot);
  assert(restored.snapshot->preferences().at("keybindings").at("switch_language_caps_lock") == true);
  assert(restored.snapshot->preferences().at("diagnostic_log").at("server") == true);
  assert(restored.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  const auto import_conflict = store.import_document(*changed.snapshot, *exported);
  assert(import_conflict.outcome == Outcome::conflict && import_conflict.error == "settings_conflict");
  assert(store.import_document(*restored.snapshot, "{}").outcome == Outcome::storage);
  assert(store.load().snapshot->revision() == restored.snapshot->revision());
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  auto invalid = store.save(*restored.snapshot, {{{"keybindings", "synthetic_unknown_key"}, true}});
  assert(invalid.outcome == Outcome::invalid_change);
  auto rejected = store.save(*restored.snapshot, {{{"keybindings", "switch_language_caps_lock"}, "invalid"}});
  assert(rejected.outcome == Outcome::storage);
  assert(store.load().snapshot->revision() == restored.snapshot->revision());

  // 真实 HWND、系统控件通知和工作线程贯通到同源 Rust，不触碰测试目录之外的数据。
  const auto file = temporary.path / L"synthetic-settings.json";
  const auto resources = temporary.path / L"resources";
  const auto custom_directory = resources / L"helpcodes" / L"custom";
  const auto plugin_directory = temporary.path / L"plugins" / L"helpcode" / L"synthetic-codes";
  std::filesystem::create_directories(custom_directory);
  std::filesystem::create_directories(plugin_directory);
  {
    std::ofstream custom(custom_directory / L"synthetic.txt", std::ios::binary);
    custom << "# name: 合成辅助码\n# name_en: Synthetic helper\n字=ab\n";
    std::ofstream manifest(plugin_directory / L"plugin.toml", std::ios::binary);
    manifest << "schema_version = 1\nkind = 'helpcode'\nid = 'synthetic-codes'\nname = '合成插件'\nversion = '1'\nlicense = 'CC0-1.0'\n[helpcode]\ntable = 'table.txt'\n";
    std::ofstream table(plugin_directory / L"table.txt", std::ios::binary);
    table << "字=cd\n";
    std::ofstream options(temporary.path / L"runtime-options.json", std::ios::binary);
    options << Json{{"api_version", 1}, {"resources", resources.u8string()},
        {"user_data", (temporary.path / L"user").u8string()}, {"cache", (temporary.path / L"cache").u8string()},
        {"dictionaries", (temporary.path / L"dictionaries").u8string()},
        {"preferences_directory", temporary.path.u8string()}, {"preferences", restored.snapshot->preferences()}}.dump();
    assert(custom.good() && manifest.good() && table.good() && options.good());
  }
  InjectedDirectory host_options(temporary.path / L"runtime-options.json", L"MSIME_CLIENT_HOST_OPTIONS");
  const auto skins_root = temporary.path / L"skins";
  const auto imported_skin = temporary.path / L"import-source" / L"synthetic-light";
  const auto make_skin = [](const std::filesystem::path& directory, const std::string& id, const std::string& base, const std::string& mode) {
    std::filesystem::create_directories(directory);
    std::ofstream manifest(directory / L"skin.toml", std::ios::binary);
    manifest << "schema_version = 1\nid = '" << id << "'\nname = '合成皮肤 " << id << "'\nversion = '1'\nbase = '" << base
        << "'\n[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['" << mode
        << "']\n[candidate_window]\nmin_width_dip = 180\n[candidate_window.background]\nimage = 'sample.gif'\nfit = 'stretch'\nopacity = 0.5\n"
        << "[candidate_window.decoration]\nimage = 'sample.gif'\ntop_inset_dip = 24\nwidth_dip = 24\n";
    assert(manifest.good());
    // 一帧、一个红色像素的合成 GIF，同时验证背景与装饰的正式资源读取和解码。
    const unsigned char gif[]{'G','I','F','8','9','a',1,0,1,0,128,0,0,255,0,0,0,0,255,
        0x21,0xf9,4,4,1,0,0,0,0x2c,0,0,0,0,1,0,1,0,0,2,2,0x44,1,0,0x3b};
    std::ofstream image(directory / L"sample.gif", std::ios::binary);
    image.write(reinterpret_cast<const char*>(gif), sizeof(gif));
    assert(image.good());
  };
  make_skin(imported_skin, "synthetic-light", "light", "light");
  make_skin(skins_root / L"synthetic-dark", "synthetic-dark", "night", "dark");
  bool cancel_skin_folder = false;
  bool allow_skin_import = false;
  unsigned skin_confirmations = 0;
  unsigned selections = 0;
  bool cancel_selection = false;
  bool allow_fuzzy_reset = false;
  unsigned reset_confirmations = 0;
  bool allow_community = false, preview_selection = false, community_unauthorized = false, malformed_community = false;
  unsigned community_confirmations = 0;
  unsigned installs = 0;
  const DWORD ui_thread = GetCurrentThreadId();
  std::vector<Json> community_calls;
  bool allow_statistics = false, statistics_upload_enabled = false, fail_statistics_upload = false;
  unsigned statistics_confirmations = 0;
  std::vector<Json> statistics_upload_calls;
  std::vector<Json> account_calls;
  bool account_signed_in = false, allow_account = false, account_status_failed = false;
  bool malformed_account_status = false, fail_account_apply = false;
  bool fail_avatar_upload = false, malformed_avatar = false;
  bool cloud_export_selection = false, malformed_devices = false, fail_account_delete = true;
  const auto cloud_export_file = temporary.path / L"synthetic-cloud.zip";
  const auto other_device_id = std::string(64, 'b');
  Json account_devices = Json::array({
      {{"id", std::string(64, 'a')}, {"name", "合成当前设备"}, {"platform", "windows"}, {"app_version", "1.0.0"}, {"current", true}, {"created_at", "2026-01-02T03:04:05Z"}, {"last_active", "2026-02-03T04:05:06Z"}},
      {{"id", other_device_id}, {"name", "合成其他设备"}, {"platform", "web"}, {"app_version", ""}, {"current", false}, {"created_at", "2026-01-02T03:04:05Z"}, {"last_active", "2026-02-03T04:05:06Z"}}});
  Json qq_links = Json::array({{{"id", "1"}, {"created_at", "2026-01-02T03:04:05Z"}}});
  std::mutex account_transport_lock;
  std::condition_variable account_google_ready;
  std::atomic<bool> account_google_started{false}, account_google_cancelled{false};
  unsigned google_cancellations = 0;
  unsigned account_confirmations = 0;
  Json account_user{{"id", "synthetic-account-0001"}, {"displayName", "合成账号"}, {"email", "synthetic@example.invalid"}, {"createdAt", "2026-01-02T03:04:05Z"}};
  const std::string synthetic_id = "00000000-0000-4000-8000-000000000001";
  const std::string other_id = "00000000-0000-4000-8000-000000000002";
  Json community_item{{"id", synthetic_id}, {"package_id", "synthetic-light"}, {"name", "合成社区皮肤"},
      {"description", "合成测试说明"}, {"author", "合成作者"}, {"version", "1"}, {"owned", false},
      {"category", "nature"}, {"visibility", "public"}, {"downloads", 3}, {"rating_count", 2}, {"rating_average", 4.5},
      {"license", {{"code", "MIT"}, {"assets", "CC0-1.0"}}}};
  const std::string preview_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
  const auto preview_file = temporary.path / L"synthetic-preview.png";
  {
    const unsigned char png[]{137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,6,0,0,0,31,21,196,137,0,0,0,13,73,68,65,84,120,156,99,248,207,192,240,31,0,5,0,1,255,137,153,61,29,0,0,0,0,73,69,78,68,174,66,96,130};
    std::ofstream image(preview_file, std::ios::binary);
    image.write(reinterpret_cast<const char*>(png), sizeof(png)); assert(image.good());
  }
  const SettingsWindow::CommunityTransport account_transport = [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(body);
    assert(request.at("directory").is_string() && request.at("legacy_directory").is_string());
    assert(request.at("client_agent").get<std::string>().find("windows") != std::string::npos);
    std::unique_lock<std::mutex> transport_guard(account_transport_lock);
    const auto action = request.at("action");
    account_calls.push_back(action);
    const auto operation = action.at("operation").get<std::string>();
    Json value;
    if (operation == "providers") value = {{"email", true}, {"phone", true}, {"google", true}};
    else if (operation == "status") {
      if (account_status_failed) return Json{{"ok", false}, {"error", "account_storage"}}.dump();
      if (malformed_account_status) return Json{{"ok", true}, {"value", {{"user", {{"id", 12}}}}}}.dump();
      value = {{"user", account_signed_in ? account_user : Json()}};
    } else if (operation == "request_code") {
      assert(action.at("target") == "synthetic@example.invalid");
      value = {{"challengeId", "synthetic-challenge"}, {"expiresIn", 3600}};
    } else if (operation == "google_login") {
      account_google_cancelled = false;
      account_google_started = true;
      assert(account_google_ready.wait_for(transport_guard, std::chrono::seconds(5), [&] { return account_google_cancelled.load(); }));
      account_google_started = false;
      return Json{{"ok", false}, {"error", "account_cancelled"}}.dump();
    } else if (operation == "google_cancel") {
      // 第一次模拟共享层尚未建立等待标志，第二次才完成取消。
      if (++google_cancellations >= 2) { account_google_cancelled = true; account_google_ready.notify_one(); }
    } else if (operation == "login") {
      assert(action.at("challenge_id") == "synthetic-challenge" && action.at("code") == "123456");
      account_signed_in = true; value = {{"user", account_user}};
    } else if (operation == "profile" || operation == "rename") {
      if (operation == "rename") account_user["displayName"] = action.at("display_name");
      value = {{"user", account_user}, {"providers", Json::array({"email", "google"})}};
    } else if (operation == "upload_avatar" || operation == "remove_avatar") {
      if (operation == "upload_avatar") {
        assert(action.at("path") == preview_file.u8string());
        if (fail_avatar_upload) return Json{{"ok", false}, {"error", "account_invalid"}}.dump();
        account_user["avatarUrl"] = "https://media.msime.app/avatars/synthetic.png";
        account_user["avatarUploaded"] = true;
      } else { account_user["avatarUrl"] = nullptr; account_user["avatarUploaded"] = false; }
      value = {{"user", account_user}, {"providers", Json::array({"email", "google"})}};
    } else if (operation == "avatar") {
      value = malformed_avatar ? "https://example.invalid/synthetic-avatar.png" : "data:image/png;base64," + preview_base64;
    } else if (operation == "preferences_schema") value = Json::object();
    else if (operation == "preferences_load" || operation == "preferences_upload") {
      if (operation == "preferences_upload") assert(action.at("preferences_directory") == temporary.path.u8string());
      value = {{"revision", 2}, {"settings", {{"input.traditional_chinese_output", true}}}};
    } else if (operation == "preferences_apply") {
      assert(action.at("preferences_directory") == temporary.path.u8string());
      assert(action.at("user_id") == account_user.at("id") && action.at("preferences").at("revision") == 2);
      const auto current = store.load();
      const auto applied = store.save(*current.snapshot, {{{"traditional_chinese_output"}, !fail_account_apply}});
      assert(applied.outcome == Outcome::saved); value = applied.snapshot->document;
      if (fail_account_apply) return Json{{"ok", false}, {"error", "account_storage"}}.dump();
    } else if (operation == "sessions_list") value = {{"sessions", malformed_devices ? Json(false) : account_devices}};
    else if (operation == "session_revoke") {
      const auto id = action.at("id");
      const bool current = id == std::string(64, 'a');
      assert(current || id == other_device_id);
      for (auto it = account_devices.begin(); it != account_devices.end(); ++it) if (it->at("id") == id) { account_devices.erase(it); break; }
      if (current) account_signed_in = false;
      value = {{"signed_out", current}};
    } else if (operation == "qq_links") value = {{"links", qq_links}};
    else if (operation == "qq_link_redeem") {
      assert(action.at("code") == "12345678");
      qq_links.push_back({{"id", "2"}, {"created_at", "2026-01-03T03:04:05Z"}});
    } else if (operation == "qq_link_delete") {
      assert(action.at("id") == "1"); qq_links.erase(qq_links.begin());
    } else if (operation == "data_summary") value = {{"bytes", 123}, {"anonymous_voice", "unknown"}, {"sections", Json::array({{{"id", "voice"}, {"bytes", 123}, {"items", 2}}})}};
    else if (operation == "data_export") {
      assert(action.at("destination") == cloud_export_file.u8string());
      value = {{"path", cloud_export_file.u8string()}, {"bytes", 0}, {"filename", "msime-data.zip"}};
    } else if (operation == "data_delete") {
      assert(action.at("sections") == Json::array({"voice"}));
      value = {{"voice_samples", {{"ok", false}, {"error", "account_rate_limited"}, {"retry_after_seconds", 60}}}};
    } else if (operation == "voice_delete") value = nullptr;
    else if (operation == "account_delete") {
      if (fail_account_delete) return Json{{"ok", false}, {"error", "account_recent_login_required"}}.dump();
      account_signed_in = false;
      value = {{"voice_samples", {{"ok", false}, {"error", "account_unavailable"}}}};
    }
    else if (operation == "logout" || operation == "forget") { account_signed_in = false; account_status_failed = false; }
    else { assert(false); }
    return Json{{"ok", true}, {"value", value}}.dump();
  };
  SettingsWindow window([&](HWND owner, bool save) -> std::optional<std::filesystem::path> {
    DWORD process = 0;
    GetWindowThreadProcessId(owner, &process);
    assert(process == GetCurrentProcessId());
    if (cloud_export_selection) return cancel_selection ? std::nullopt : std::optional<std::filesystem::path>(cloud_export_file);
    if (preview_selection) return cancel_selection ? std::nullopt : std::optional<std::filesystem::path>(preview_file);
    if (cancel_selection) return std::nullopt;
    assert(save == (selections == 0));
    ++selections;
    return file;
  }, [&](HWND owner, const wchar_t* title, const wchar_t* text) {
    assert(IsWindow(owner));
    if (std::wstring_view(title) == L"账号与同步") {
      assert(text && *text); ++account_confirmations; return allow_account;
    }
    if (std::wstring_view(title) == L"打字统计") {
      assert(text && *text); ++statistics_confirmations; return allow_statistics;
    }
    if (std::wstring_view(title) == L"社区皮肤") {
      assert(text && *text); ++community_confirmations; return allow_community;
    }
    if (std::wstring_view(title) == L"导入皮肤") {
      assert(std::wstring_view(text).find(imported_skin.wstring()) != std::wstring_view::npos);
      ++skin_confirmations;
      return allow_skin_import;
    }
    assert(std::wstring_view(title) == L"关闭模糊音");
    assert(std::wstring_view(text).find(L"清空") != std::wstring_view::npos);
    ++reset_confirmations;
    return allow_fuzzy_reset;
  }, [&](HWND owner) -> std::optional<std::filesystem::path> {
    assert(IsWindow(owner));
    return cancel_skin_folder ? std::nullopt : std::optional<std::filesystem::path>(imported_skin);
  }, [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(body);
    assert(request.at("state_root") == temporary.path.u8string());
    assert(request.at("skins_directory") == skins_root.u8string());
    assert(std::filesystem::u8path(request.at("directory").get<std::string>()).filename() == L"account");
    const auto action = request.at("action");
    community_calls.push_back(action);
    const auto operation = action.at("operation").get<std::string>();
    if (community_unauthorized) return Json{{"ok", false}, {"error", "community_unauthorized"}}.dump();
    if (malformed_community) return std::string("{broken");
    Json value;
    if (operation == "candidate_skin_list") {
      assert(action.at("offset").is_number_unsigned() || action.at("offset").is_number_integer());
      auto item = community_item;
      const auto offset = action.at("offset").get<size_t>();
      if (offset) { item["id"] = other_id; item["name"] = "合成第二款"; }
      value = {{"skins", Json::array({item})}, {"has_more", offset == 0}};
    } else if (operation == "candidate_skin_detail") value = community_item;
    else if (operation == "candidate_skin_preview") value = {{"dataUrl", "data:image/png;base64," + preview_base64}};
    else if (operation == "candidate_skin_install") {
      ++installs;
      if (!action.at("replace").get<bool>()) return Json{{"ok", false}, {"error", "candidate_skin_exists"}}.dump();
      value = theme_data::value(theme_data::catalog(skins_root)).value();
    } else if (operation == "candidate_skin_rate") value = {{"stars", action.at("stars")}};
    else if (operation == "candidate_skin_set_visibility" || operation == "candidate_skin_set_category") {
      community_item[operation == "candidate_skin_set_visibility" ? "visibility" : "category"] = action.at(operation == "candidate_skin_set_visibility" ? "visibility" : "category");
      value = community_item;
    } else if (operation == "candidate_skin_unpublish") value = {{"deleted", true}};
    else if (operation == "candidate_skin_sync") value = {{"uploaded", Json::array()}, {"downloaded", Json::array()}, {"deleted_local", Json::array()}, {"skipped", Json::array()}, {"stopped", false}};
    else if (operation == "report") { assert(action.at("kind") == "candidate-skins"); }
    else if (operation == "candidate_skin_pack_preview" || operation == "candidate_skin_add_license" || operation == "candidate_skin_add_preview") {
      auto local_request = request;
      local_request["directory"] = (temporary.path / L"synthetic-account").u8string();
      local_request.erase("legacy_directory");
      return theme_data::call(msime_client_community, local_request).dump();
    } else if (operation == "candidate_skin_publish") {
      assert(action.at("id").get<std::string>().size() == 36); value = community_item;
    } else { assert(false); }
    return Json{{"ok", true}, {"value", value}}.dump();
  }, [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(body);
    const auto action = request.at("action");
    assert(action.at("statistics_directory") == temporary.path.u8string());
    statistics_upload_calls.push_back(action);
    const auto operation = action.at("operation").get<std::string>();
    assert(operation == "typing_upload" || operation == "typing_upload_set" || operation == "typing_upload_status");
    if (operation == "typing_upload_set") {
      statistics_upload_enabled = action.at("enabled").get<bool>();
      if (fail_statistics_upload) return Json{{"ok", false}, {"error", "account_unavailable"}}.dump();
    }
    return Json{{"ok", true}, {"value", {{"enabled", statistics_upload_enabled}, {"signed_in", true}, {"last_upload_ms", nullptr}}}}.dump();
  }, account_transport);
  assert(window.create(GetModuleHandleW(nullptr)));
  const HWND host = window.handle();
  const HWND body = GetDlgItem(host, page_body);
  const HWND language = GetDlgItem(body, language_switch);
  const HWND caps = GetDlgItem(body, caps_lock);
  assert(body && language && caps);
#if MSIME_EDITION_IS_FULL
  assert(GetDlgItem(body, japanese_toggle) != nullptr);
#else
  assert(GetDlgItem(body, japanese_toggle) == nullptr);
#endif
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(SendMessageW(caps, BM_GETCHECK, 0, 0) == BST_CHECKED);
  const auto before_controls = store.load().snapshot->preferences();
  const auto select_page = [&](int index) {
    const HWND selector = GetDlgItem(host, page_selector);
    assert(selector);
    SendMessageW(selector, CB_SETCURSEL, index, 0);
    SendMessageW(host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(selector));
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  };
  SendMessageW(language, CB_SETCURSEL, 3, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(language_switch, CBN_SELCHANGE), reinterpret_cast<LPARAM>(language));
  assert(!IsWindowEnabled(language));
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  auto saved = store.load();
  assert(saved.snapshot);
  const auto& keys = saved.snapshot->preferences().at("keybindings");
  assert(!keys.at("switch_language_shift").get<bool>());
  assert(!keys.at("switch_language_ctrl").get<bool>());
  assert(!keys.at("switch_language_ctrl_alt_space").get<bool>());
  assert(saved.snapshot->preferences().at("ai_assistant") == before_controls.at("ai_assistant"));

  SendMessageW(caps, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(caps) != FALSE; });
  saved = store.load();
  assert(!saved.snapshot->preferences().at("keybindings").at("switch_language_caps_lock").get<bool>());
  const bool previous_width = saved.snapshot->preferences().at("keybindings").at("toggle_fullwidth_option_shift_h").get<bool>();
  const auto external = store.save(*saved.snapshot, {{{"keybindings", "toggle_fullwidth_option_shift_h"}, !previous_width}});
  assert(external.outcome == Outcome::saved);
  SendMessageW(caps, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(caps) != FALSE; });
  assert(caption(GetDlgItem(host, status_text)).find(L"其他窗口") != std::wstring::npos);
  assert(SendMessageW(caps, BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  saved = store.load();
  assert(saved.snapshot->revision() == external.snapshot->revision());
  assert(saved.snapshot->preferences().at("keybindings").at("toggle_fullwidth_option_shift_h").get<bool>() != previous_width);

  const HWND tabs = GetDlgItem(host, page_tabs);
  const auto select_tab = [&](const wchar_t* title) {
    for (int index = 0; index < TabCtrl_GetItemCount(tabs); ++index) {
      wchar_t text[128]{};
      TCITEMW item{};
      item.mask = TCIF_TEXT;
      item.pszText = text;
      item.cchTextMax = 128;
      assert(TabCtrl_GetItem(tabs, index, &item));
      if (std::wstring_view(text) != title) continue;
      TabCtrl_SetCurSel(tabs, index);
      NMHDR changed_tab{tabs, page_tabs, TCN_SELCHANGE};
      SendMessageW(host, WM_NOTIFY, page_tabs, reinterpret_cast<LPARAM>(&changed_tab));
      return;
    }
    assert(false && "Requested settings tab does not exist");
  };
  TabCtrl_SetCurSel(tabs, 1);
  NMHDR notification{tabs, page_tabs, TCN_SELCHANGE};
  SendMessageW(host, WM_NOTIFY, page_tabs, reinterpret_cast<LPARAM>(&notification));
  const HWND reference = GetDlgItem(body, reference_text);
  assert((GetWindowLongPtrW(reference, GWL_STYLE) & ES_READONLY) != 0);
  assert(caption(reference).find(L"Page Up / Page Down") != std::wstring::npos);
  assert(caption(reference).find(L"Shift+Tab / Tab") != std::wstring::npos);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  // 以词定字与翻页共用键组时，复合修改必须一次落盘，不能先保存出非法中间态。
  select_page(3);
  select_tab(L"选词与翻页");
  const HWND word = GetDlgItem(body, word_character_keys);
  const HWND brackets = GetDlgItem(body, navigation_brackets);
  const HWND backspace = GetDlgItem(body, backspace_reverts);
  assert(word && brackets && backspace);
  assert((GetWindowLongPtrW(word, GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  auto selection_seed = store.save(*saved.snapshot, {{{"word_character", "enabled"}, false},
      {{"navigation", "brackets"}, false}, {{"navigation", "minus_equal"}, false}});
  assert(selection_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(word) != FALSE; });
  const auto select_word = [&](const wchar_t* title) {
    const auto index = SendMessageW(word, CB_FINDSTRINGEXACT, static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(title));
    assert(index != CB_ERR);
    SendMessageW(word, CB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(word_character_keys, CBN_SELCHANGE), reinterpret_cast<LPARAM>(word));
    pump_until([&] { return IsWindowEnabled(word) != FALSE; });
  };
  select_word(L"[ / ]");
  saved = store.load();
  assert(saved.snapshot->revision() == selection_seed.snapshot->revision() + 1);
  assert(saved.snapshot->preferences().at("word_character").at("enabled") == true);
  assert(saved.snapshot->preferences().at("word_character").at("keys") == "brackets");
  const auto word_revision = saved.snapshot->revision();
  SendMessageW(brackets, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(brackets) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->revision() == word_revision + 1);
  assert(saved.snapshot->preferences().at("navigation").at("brackets") == true);
  assert(saved.snapshot->preferences().at("word_character").at("enabled") == false);
  assert(SendMessageW(word, CB_FINDSTRINGEXACT, static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(L"[ / ]")) == CB_ERR);
  select_word(L"- / =");
  saved = store.load();
  assert(saved.snapshot->preferences().at("word_character").at("keys") == "minus_equal");
  assert(saved.snapshot->preferences().at("word_character").at("enabled") == true);
  const bool previous_backspace = saved.snapshot->preferences().value("backspace_reverts_selection", true);
  SendMessageW(backspace, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(backspace) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->preferences().value("backspace_reverts_selection", true) != previous_backspace);
  assert(saved.snapshot->preferences().at("word_character").at("enabled") == true);
  const HWND mouse = GetDlgItem(body, navigation_mouse_wheel);
  assert(mouse && SendMessageW(mouse, BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  SendMessageW(mouse, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(mouse) != FALSE; });
  assert(store.load().snapshot->preferences().at("navigation").at("mouse_wheel") == true);
  SendMessageW(mouse, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(mouse) != FALSE; });
  assert(!store.load().snapshot->preferences().at("navigation").contains("mouse_wheel"));
  select_word(L"关闭");
  saved = store.load();
  assert(saved.snapshot->preferences().at("word_character").at("keys") == "minus_equal");
  assert(saved.snapshot->preferences().at("word_character").at("enabled") == false);
  assert(saved.snapshot->preferences().at("navigation").at("tab") == false);
  assert(saved.snapshot->preferences().at("navigation").at("page_up_down") == false);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  const auto select_value = [&](int id, int index) {
    const HWND control = GetDlgItem(body, id);
    assert(control && IsWindowEnabled(control));
    SendMessageW(control, CB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, CBN_SELCHANGE), reinterpret_cast<LPARAM>(control));
    pump_until([&] { return IsWindowEnabled(control) != FALSE; });
  };
  const auto click = [&](int id) {
    const HWND control = GetDlgItem(body, id);
    assert(control && IsWindowEnabled(control));
    SendMessageW(control, BM_CLICK, 0, 0);
    pump_until([&] { return IsWindowEnabled(GetDlgItem(body, punctuation_mode)) != FALSE; });
  };
  // 输入页的标签切换不落盘；繁简只修改对应开关，不改其他宿主的繁体变体。
  select_tab(L"输出设置");
  assert((GetWindowLongPtrW(GetDlgItem(body, traditional_output), GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  const auto before_output = saved.snapshot->preferences();
  const bool old_traditional = before_output.at("traditional_chinese_output").get<bool>();
  select_value(traditional_output, old_traditional ? 0 : 1);
  saved = store.load();
  assert(saved.snapshot->preferences().at("traditional_chinese_output").get<bool>() != old_traditional);
  assert(saved.snapshot->preferences().value("traditional_chinese_variant", "s2t") ==
      before_output.value("traditional_chinese_variant", "s2t"));
  assert(caption(GetDlgItem(body, output_description)).find(old_traditional ? L"简体" : L"繁体") != std::wstring::npos);
  const bool old_spacing = saved.snapshot->preferences().value("chinese_english_spacing", false);
  click(chinese_english_spacing);
  assert(store.load().snapshot->preferences().value("chinese_english_spacing", false) != old_spacing);
  click(chinese_english_spacing);
  assert(store.load().snapshot->preferences().value("chinese_english_spacing", false) == old_spacing);
  const auto old_width = store.load().snapshot->preferences().at("character_width");
  click(fullwidth_output);
  assert(store.load().snapshot->preferences().at("character_width") != old_width);
  saved = store.load();
  select_page(4);
  assert((GetWindowLongPtrW(GetDlgItem(body, punctuation_mode), GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  select_value(punctuation_mode, 0);
  saved = store.load();
  const auto punctuation_revision = saved.snapshot->revision();
  select_value(punctuation_mode, 3);
  saved = store.load();
  assert(saved.snapshot->revision() == punctuation_revision + 1);
  assert(saved.snapshot->preferences().at("chinese_punctuation") == true);
  assert(saved.snapshot->preferences().at("punctuation_lock") == "english");
  select_value(punctuation_mode, 1);
  saved = store.load();
  assert(saved.snapshot->revision() == punctuation_revision + 2);
  assert(saved.snapshot->preferences().at("chinese_punctuation") == false);
  assert(saved.snapshot->preferences().at("punctuation_lock") == "follow");
  select_value(quote_style, 1);
  assert(store.load().snapshot->preferences().at("quote_style") == "corner");
  select_value(quote_style, 0);
  assert(!store.load().snapshot->preferences().contains("quote_style"));
  select_value(bracket_style, 1);
  assert(store.load().snapshot->preferences().at("bracket_style") == "hollow");
  select_value(bracket_style, 0);
  assert(!store.load().snapshot->preferences().contains("bracket_style"));
  saved = store.load();
  const auto bad_style = store.save(*saved.snapshot, {{{"quote_style"}, "synthetic-invalid-style"}});
  assert(bad_style.outcome == Outcome::storage);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  const auto bool_style = store.save(*saved.snapshot, {{{"quote_style"}, true}});
  assert(bool_style.outcome == Outcome::invalid_change);
  const auto repeat = saved.snapshot->preferences().at("smart_punctuation_repeat");
  const bool old_smart = saved.snapshot->preferences().at("smart_punctuation").get<bool>();
  click(smart_punctuation);
  saved = store.load();
  assert(saved.snapshot->revision() == punctuation_revision + 7);
  for (const auto* field : {"smart_punctuation", "smart_punctuation_direct_digit", "smart_punctuation_direct_letter"})
    assert(saved.snapshot->preferences().at(field).get<bool>() != old_smart);
  assert(saved.snapshot->preferences().at("smart_punctuation_repeat") == repeat);
  assert((IsWindowEnabled(GetDlgItem(body, smart_repeat)) != FALSE) == !old_smart);
  if (old_smart) click(smart_punctuation);
  click(smart_direct_digit);
  click(smart_direct_letter);
  assert(caption(GetDlgItem(body, smart_description)).find(L"暂不生效") != std::wstring::npos);
  click(smart_punctuation);
  assert(!IsWindowEnabled(GetDlgItem(body, smart_direct_digit)));
  assert(!IsWindowEnabled(GetDlgItem(body, smart_direct_letter)));
  click(smart_punctuation);
  saved = store.load();
  assert(saved.snapshot->preferences().at("smart_punctuation_direct_digit") == true);
  assert(saved.snapshot->preferences().at("smart_punctuation_direct_letter") == true);
  assert(saved.snapshot->preferences().at("smart_punctuation_repeat") == repeat);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  // 混输总开关仅改变可用状态，保留子项与其他宿主使用的最短触发长度。
  select_page(3);
  select_tab(L"候选与联想");
  assert((GetWindowLongPtrW(GetDlgItem(body, mixed_enabled), GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
#if MSIME_EDITION_TEMPORARY_JAPANESE
  assert(GetDlgItem(body, mixed_japanese));
#else
  assert(!GetDlgItem(body, mixed_japanese));
#endif
  const auto before_mixed = saved.snapshot->preferences().at("mixed_input");
  const bool was_mixed = before_mixed.value("enabled", true);
  click(mixed_enabled);
  saved = store.load();
  assert(saved.snapshot->preferences().at("mixed_input").value("enabled", true) != was_mixed);
  assert(saved.snapshot->preferences().at("mixed_input").at("english") == before_mixed.at("english"));
  assert(saved.snapshot->preferences().at("mixed_input").at("emoji") == before_mixed.at("emoji"));
  assert(saved.snapshot->preferences().at("mixed_input").at("minimum_prefix") == before_mixed.at("minimum_prefix"));
  assert((IsWindowEnabled(GetDlgItem(body, mixed_english)) != FALSE) == !was_mixed);
  if (was_mixed) click(mixed_enabled);
  select_value(mixed_first_page_count, 2);
  assert(store.load().snapshot->preferences().at("mixed_input").at("first_page_count") == 3);
  select_value(mixed_first_page_count, 0);
  assert(!store.load().snapshot->preferences().at("mixed_input").contains("first_page_count"));
  const bool old_symbols = store.load().snapshot->preferences().at("mixed_input").value("symbols", true);
  click(mixed_symbols);
  assert(store.load().snapshot->preferences().at("mixed_input").value("symbols", true) != old_symbols);
  click(mixed_symbols);
  assert(store.load().snapshot->preferences().at("mixed_input").value("symbols", true) == old_symbols);
  // 选择来源和编辑文本只改草稿；失败后无关偏好保存也不能把草稿抹掉。
  saved = store.load();
  const auto before_draft = saved.snapshot->revision();
  select_value(cloud_source, 1);
  assert(store.load().snapshot->revision() == before_draft);
  assert(IsWindowEnabled(GetDlgItem(body, cloud_endpoint)));
  assert(!IsWindowEnabled(GetDlgItem(body, cloud_body)));
  SetWindowTextW(GetDlgItem(body, cloud_endpoint), L"https://synthetic.example.invalid/candidates");
  SetWindowTextW(GetDlgItem(body, cloud_headers), L"{");
  click(cloud_save);
  assert(store.load().snapshot->revision() == before_draft);
  assert(caption(GetDlgItem(host, status_text)).find(L"未保存的内容已保留") != std::wstring::npos);
  assert(caption(GetDlgItem(body, cloud_headers)) == L"{");
  click(learn_candidates);
  assert(caption(GetDlgItem(body, cloud_headers)) == L"{");
  assert(SendMessageW(GetDlgItem(body, cloud_source), CB_GETCURSEL, 0, 0) == 1);
  select_value(cloud_method, 1);
  select_value(cloud_response_format, 2);
  assert(IsWindowEnabled(GetDlgItem(body, cloud_body)));
  assert(IsWindowEnabled(GetDlgItem(body, cloud_response_path)));
  SetWindowTextW(GetDlgItem(body, cloud_headers), L"{\"X-Synthetic\":\"synthetic-value\"}");
  SetWindowTextW(GetDlgItem(body, cloud_body), L"{\"query\":\"{query}\",\"test\":\"合成\"}");
  SetWindowTextW(GetDlgItem(body, cloud_response_path), L"choices.0.message.content");
  SetWindowTextW(GetDlgItem(body, cloud_timeout), L"2500");
  const auto before_cloud_save = store.load().snapshot->revision();
  click(cloud_save);
  saved = store.load();
  assert(saved.snapshot->revision() == before_cloud_save + 1);
  const auto cloud_config = saved.snapshot->preferences().at("cloud_candidate_provider");
  assert(cloud_config.at("source") == "custom" && cloud_config.at("method") == "POST");
  assert(cloud_config.at("headers_json") == "{\"X-Synthetic\":\"synthetic-value\"}");
  assert(cloud_config.at("body_template") == "{\"query\":\"{query}\",\"test\":\"合成\"}");
  assert(cloud_config.at("response_path") == "choices.0.message.content");
  assert(cloud_config.at("timeout_ms") == 2500);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  SetWindowTextW(GetDlgItem(body, cloud_timeout), L"invalid");
  click(cloud_save);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  assert(caption(GetDlgItem(body, cloud_timeout)) == L"invalid");
  // 新的服务快照胜过过期草稿，CAS 冲突不能静默覆盖别处的选择。
  auto external_cloud = cloud_config;
  external_cloud["timeout_ms"] = 3500;
  const auto cloud_peer = store.save(*saved.snapshot, {{{"cloud_candidate_provider"}, external_cloud}});
  assert(cloud_peer.outcome == Outcome::saved);
  SetWindowTextW(GetDlgItem(body, cloud_timeout), L"3000");
  click(cloud_save);
  saved = store.load();
  assert(saved.snapshot->revision() == cloud_peer.snapshot->revision());
  assert(saved.snapshot->preferences().at("cloud_candidate_provider").at("timeout_ms") == 3500);
  assert(caption(GetDlgItem(body, cloud_timeout)) == L"3500");
  assert(caption(GetDlgItem(host, status_text)).find(L"其他窗口") != std::wstring::npos);
  select_tab(L"中英模式");
  const auto mode_revision = saved.snapshot->revision();
  assert(store.load().snapshot->revision() == mode_revision);
  const auto old_mode = saved.snapshot->preferences().at("default_ime_mode");
  select_value(default_ime_mode, old_mode == "english" ? 0 : 1);
  assert(store.load().snapshot->preferences().at("default_ime_mode") != old_mode);
  select_value(ime_mode_scope, 1);
  assert(store.load().snapshot->preferences().at("ime_mode_scope") == "global");
  const bool old_hud = store.load().snapshot->preferences().at("input_mode_hud").get<bool>();
  click(input_mode_hud);
  assert(store.load().snapshot->preferences().at("input_mode_hud").get<bool>() != old_hud);
  saved = store.load();
  // 同一张规则表里其他平台的标识必须保留，新增 Windows 规则沿用现有路径与大小写规范化。
  auto rule_seed = store.save(*saved.snapshot, {{{"app_input_mode_rules"}, Json{{"com.synthetic.editor", "english"}}}});
  assert(rule_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(GetDlgItem(body, app_rule_add)) != FALSE; });
  assert(!IsWindowEnabled(GetDlgItem(body, app_rule_mode)));
  SetWindowTextW(GetDlgItem(body, app_rule_name), L"  \"C:\\synthetic\\合成EDITOR.EXE\"  ");
  click(app_rule_add);
  saved = store.load();
  assert(saved.snapshot->revision() == rule_seed.snapshot->revision() + 1);
  assert(saved.snapshot->preferences().at("app_input_mode_rules").at("合成editor.exe") == "chinese");
  assert(saved.snapshot->preferences().at("app_input_mode_rules").at("com.synthetic.editor") == "english");
  assert(saved.snapshot->preferences().at("ime_mode_scope") == "global");
  SetWindowTextW(GetDlgItem(body, app_rule_name), L"合成Editor.EXE");
  click(app_rule_add);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  assert(caption(GetDlgItem(host, status_text)).find(L"已经有例外") != std::wstring::npos);
  const auto select_rule = [&](const wchar_t* prefix) {
    const HWND list = GetDlgItem(body, app_rule_list);
    const auto index = SendMessageW(list, LB_FINDSTRING, static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(prefix));
    assert(index != LB_ERR);
    SendMessageW(list, LB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(app_rule_list, LBN_SELCHANGE), reinterpret_cast<LPARAM>(list));
  };
  select_rule(L"合成editor.exe");
  select_value(app_rule_mode, 1);
  saved = store.load();
  assert(saved.snapshot->preferences().at("app_input_mode_rules").at("合成editor.exe") == "english");
  assert(saved.snapshot->preferences().at("app_input_mode_rules").at("com.synthetic.editor") == "english");
  SetWindowTextW(GetDlgItem(body, app_rule_name), L"synthetic.txt");
  click(app_rule_add);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  assert(caption(GetDlgItem(host, status_text)).find(L".exe") != std::wstring::npos);
  click(app_rule_remove);
  saved = store.load();
  assert(!saved.snapshot->preferences().at("app_input_mode_rules").contains("合成editor.exe"));
  assert(saved.snapshot->preferences().at("app_input_mode_rules").at("com.synthetic.editor") == "english");
  select_rule(L"com.synthetic.editor");
  click(app_rule_remove);
  saved = store.load();
  assert(!saved.snapshot->preferences().contains("app_input_mode_rules"));
  assert(!IsWindowEnabled(GetDlgItem(body, app_rule_remove)));
  assert(!IsWindowEnabled(GetDlgItem(body, app_rule_mode)));
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  // 快捷模式的总开关不清除单项；/ 和 @ 独立，地名只从属于 @。
  select_tab(L"高级选项");
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  const auto advanced_seed = store.save(*saved.snapshot, {
      {{"local_modes", "shift_letter_entry"}, true}, {{"local_modes", "quick_phrase"}, true},
      {{"local_modes", "mention"}, false}, {{"local_modes", "mention_places"}, false},
      {{"fuzzy_pinyin", "enabled"}, false}, {{"fuzzy_pinyin", "rules"}, Json::array()},
      {{"fuzzy_pinyin", "seeded"}, false}, {{"learning"}, true},
      {{"frequency", "mode"}, "promote"}, {{"frequency", "trigger_count"}, 9}, {{"frequency", "linear_step"}, 8}});
  assert(advanced_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(GetDlgItem(body, shift_letter_modes)) != FALSE; });
  const auto local_control = [&](std::wstring_view key) {
    for (size_t index = 0; index < msime::settings::input::local_modes.size(); ++index)
      if (msime::settings::input::local_modes[index].id == key) return local_mode_first + static_cast<int>(index);
    assert(false && "Requested local mode does not exist");
    return -1;
  };
#if MSIME_EDITION_TEMPORARY_JAPANESE
  assert(GetDlgItem(body, local_control(L"temporary_japanese")));
#else
  assert(!GetDlgItem(body, local_control(L"temporary_japanese")));
#endif
  click(shift_letter_modes);
  assert(!IsWindowEnabled(GetDlgItem(body, local_control(L"quick_phrase"))));
  assert(IsWindowEnabled(GetDlgItem(body, local_control(L"command"))));
  assert(IsWindowEnabled(GetDlgItem(body, local_control(L"mention"))));
  assert(store.load().snapshot->preferences().at("local_modes").at("quick_phrase") == true);
  click(shift_letter_modes);
  click(local_control(L"chaizi"));
  assert(store.load().snapshot->preferences().at("local_modes").at("chaizi") == true);
  assert(!IsWindowEnabled(GetDlgItem(body, local_control(L"mention_places"))));
  click(local_control(L"mention"));
  assert(IsWindowEnabled(GetDlgItem(body, local_control(L"mention_places"))));
  click(local_control(L"mention_places"));
  click(local_control(L"mention"));
  assert(!IsWindowEnabled(GetDlgItem(body, local_control(L"mention_places"))));
  assert(store.load().snapshot->preferences().at("local_modes").at("mention_places") == true);
  const auto before_fuzzy = store.load().snapshot->revision();
  click(fuzzy_enabled);
  saved = store.load();
  assert(saved.snapshot->revision() == before_fuzzy + 1);
  assert(saved.snapshot->preferences().at("fuzzy_pinyin").at("seeded") == true);
  assert(saved.snapshot->preferences().at("fuzzy_pinyin").at("rules").size() == 11);
  click(fuzzy_rule_first);
  click(fuzzy_enabled);
  assert(!IsWindowEnabled(GetDlgItem(body, fuzzy_rule_first)));
  assert(store.load().snapshot->preferences().at("fuzzy_pinyin").at("rules").size() == 10);
  click(fuzzy_enabled);
  assert(store.load().snapshot->preferences().at("fuzzy_pinyin").at("rules").size() == 10);
  saved = store.load();
  click(fuzzy_reset);
  assert(reset_confirmations == 1 && store.load().snapshot->revision() == saved.snapshot->revision());
  allow_fuzzy_reset = true;
  click(fuzzy_reset);
  assert(reset_confirmations == 2);
  saved = store.load();
  assert(saved.snapshot->preferences().at("fuzzy_pinyin").at("enabled") == false);
  assert(saved.snapshot->preferences().at("fuzzy_pinyin").at("rules").empty());
  assert(saved.snapshot->preferences().at("fuzzy_pinyin").at("seeded") == true);
  click(fuzzy_enabled);
  assert(store.load().snapshot->preferences().at("fuzzy_pinyin").at("rules").empty());
  // 超出界面常用档但仍合法的既有调频值照实展示，不因刷新归一化。
  assert(SendMessageW(GetDlgItem(body, frequency_trigger), CB_GETCOUNT, 0, 0) == 7);
  assert(SendMessageW(GetDlgItem(body, frequency_trigger), CB_GETCURSEL, 0, 0) == 6);
  assert(store.load().snapshot->preferences().at("frequency").at("trigger_count") == 9);
  select_value(frequency_mode, 0);
  assert(!IsWindowEnabled(GetDlgItem(body, frequency_trigger)));
  assert(!IsWindowEnabled(GetDlgItem(body, frequency_step)));
  select_value(frequency_mode, 3);
  assert(IsWindowEnabled(GetDlgItem(body, frequency_trigger)));
  assert(IsWindowEnabled(GetDlgItem(body, frequency_step)));
  select_value(frequency_trigger, 2);
  select_value(frequency_step, 4);
  assert(store.load().snapshot->preferences().at("frequency").at("trigger_count") == 3);
  assert(store.load().snapshot->preferences().at("frequency").at("linear_step") == 5);
  select_value(frequency_mode, 4);
  assert(!IsWindowEnabled(GetDlgItem(body, frequency_step)));
  select_tab(L"候选与联想");
  click(learn_candidates);
  select_tab(L"高级选项");
  assert(!IsWindowEnabled(GetDlgItem(body, frequency_mode)));
  saved = store.load();
  assert(saved.snapshot->preferences().at("frequency").at("linear_step") == 5);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  bool offers_helpcode = false;
  for (std::string_view scheme : {MSIME_EDITION_INPUT_SCHEMES})
    offers_helpcode = offers_helpcode || scheme == "quanpin" || scheme == "shuangpin";
  assert((GetDlgItem(body, helpcode_first) != nullptr) == offers_helpcode);
  if (offers_helpcode) {
    // 当前未安装的插件仍可见；换回内置方案时同次清除插件，其他插件配置与另一族不动。
    auto plugins = saved.snapshot->preferences().value("plugins", Json::object());
    plugins["helpcode_pack_quanpin"] = "synthetic-missing";
    plugins["helpcode_pack_shuangpin"] = "synthetic-other";
    plugins["combo_counter"] = true;
    const auto before_helpcode = store.save(*saved.snapshot, {{{"plugins"}, plugins},
        {{"quanpin_helpcode", "enabled"}, true}, {{"quanpin_helpcode", "schema"}, "custom/synthetic-missing"}});
    assert(before_helpcode.outcome == Outcome::saved);
    SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
    const HWND schema = GetDlgItem(body, helpcode_first + 1);
    const auto find_option = [&](const wchar_t* label) {
      return static_cast<int>(SendMessageW(schema, CB_FINDSTRINGEXACT, static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(label)));
    };
    const auto selected_text = [&] {
      const auto selected = SendMessageW(schema, CB_GETCURSEL, 0, 0);
      const auto length = SendMessageW(schema, CB_GETLBTEXTLEN, selected, 0);
      assert(length >= 0);
      std::wstring value(static_cast<size_t>(length) + 1, L'\0');
      SendMessageW(schema, CB_GETLBTEXT, selected, reinterpret_cast<LPARAM>(value.data()));
      value.resize(static_cast<size_t>(length));
      return value;
    };
    assert(selected_text() == L"synthetic-missing（插件，未找到）");
    assert(find_option(L"custom/synthetic-missing") >= 0);
    assert(find_option(L"合成辅助码 (Synthetic helper)") >= 0);
    assert(find_option(L"合成插件（插件）") >= 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(helpcode_first + 1, CBN_SETFOCUS), reinterpret_cast<LPARAM>(schema));
    assert(GetScrollPos(body, SB_VERT) > 0);
    assert(store.load().snapshot->revision() == before_helpcode.snapshot->revision());
    select_value(helpcode_first + 1, find_option(L"小鹤"));
    saved = store.load();
    assert(saved.snapshot->revision() == before_helpcode.snapshot->revision() + 1);
    assert(saved.snapshot->preferences().at("quanpin_helpcode").at("schema") == "xiaohe");
    assert(!saved.snapshot->preferences().at("plugins").contains("helpcode_pack_quanpin"));
    assert(saved.snapshot->preferences().at("plugins").at("helpcode_pack_shuangpin") == "synthetic-other");
    assert(saved.snapshot->preferences().at("plugins").at("combo_counter") == true);
    select_value(helpcode_first + 1, find_option(L"合成插件（插件）"));
    saved = store.load();
    assert(saved.snapshot->preferences().at("plugins").at("helpcode_pack_quanpin") == "synthetic-codes");
    assert(saved.snapshot->preferences().at("quanpin_helpcode").at("schema") == "xiaohe");
    click(helpcode_first);
    assert(!IsWindowEnabled(schema));
    assert(IsWindowEnabled(GetDlgItem(body, helpcode_first + 2)));
    click(helpcode_first + 2);
    assert(store.load().snapshot->preferences().at("plugins").at("helpcode_pack_quanpin") == "synthetic-codes");
    click(helpcode_first);
    select_value(helpcode_first + 1, find_option(L"合成辅助码 (Synthetic helper)"));
    saved = store.load();
    assert(saved.snapshot->preferences().at("quanpin_helpcode").at("schema") == "custom/synthetic");
    assert(!saved.snapshot->preferences().at("plugins").contains("helpcode_pack_quanpin"));
    const auto revision = saved.snapshot->revision();
    std::filesystem::remove_all(custom_directory);
    std::filesystem::remove_all(plugin_directory);
    select_page(0);
    select_page(3);
    assert(selected_text() == L"custom/synthetic");
    assert(find_option(L"合成插件（插件）") == CB_ERR);
    assert(store.load().snapshot->revision() == revision);
    assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  }
  select_tab(L"输入方案");
  const auto find_combo = [&](int id, const wchar_t* title) {
    return static_cast<int>(SendMessageW(GetDlgItem(body, id), CB_FINDSTRINGEXACT,
        static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(title)));
  };
  const auto offered = [](std::string_view value) {
    for (std::string_view scheme : {MSIME_EDITION_INPUT_SCHEMES}) if (scheme == value) return true;
    return false;
  };
  assert((GetDlgItem(body, scheme_wubi_profile) != nullptr) == offered("wubi"));
  assert((GetDlgItem(body, scheme_shuangpin_profile) != nullptr) == offered("shuangpin"));
  const auto scheme_revision = store.load().snapshot->revision();
  select_tab(L"高级选项");
  select_tab(L"输入方案");
  assert(store.load().snapshot->revision() == scheme_revision);
  if (offered("quanpin") && offered("japanese")) {
    select_value(scheme_chinese, find_combo(scheme_chinese, L"双拼"));
    assert((GetWindowLongPtrW(GetDlgItem(body, scheme_wubi_profile), GWL_STYLE) & WS_VISIBLE) == 0);
    const auto before_japanese = store.load().snapshot->revision();
    select_value(scheme_language, find_combo(scheme_language, L"日文"));
    saved = store.load();
    assert(saved.snapshot->revision() == before_japanese + 1);
    assert(saved.snapshot->preferences().at("scheme") == "japanese");
    assert(saved.snapshot->preferences().at("last_chinese_scheme") == "shuangpin");
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_chinese)));
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_shuangpin_profile)));
    select_value(scheme_language, find_combo(scheme_language, L"中文"));
    assert(store.load().snapshot->preferences().at("scheme") == "shuangpin");
    select_value(scheme_shuangpin_profile, 1);
    assert(store.load().snapshot->preferences().at("shuangpin_profile") == "ziranma");
    click(scheme_shuangpin_hint);
    assert(store.load().snapshot->preferences().at("shuangpin_keymap_hint") == true);
    select_value(scheme_shuangpin_preedit, 1);
    assert(store.load().snapshot->preferences().at("shuangpin_preedit_uses_raw") == false);
    // 已有自定义表只读展示；选择内置方案保留自定义表本身。
    saved = store.load();
    auto custom_profile = store.save(*saved.snapshot, {{{"shuangpin_profile"}, "custom"}});
    assert(custom_profile.outcome == Outcome::saved);
    SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
    assert(find_combo(scheme_shuangpin_profile, L"自定义双拼（当前值）") == 5);
    assert(SendMessageW(GetDlgItem(body, scheme_shuangpin_profile), CB_GETCURSEL, 0, 0) == 5);
    assert(store.load().snapshot->revision() == custom_profile.snapshot->revision());
    const auto custom_map = store.load().snapshot->preferences().value("shuangpin_custom_profile", Json());
    select_value(scheme_shuangpin_profile, 0);
    assert(store.load().snapshot->preferences().value("shuangpin_custom_profile", Json()) == custom_map);
  }
  if (offered("wubi")) {
    if (IsWindowEnabled(GetDlgItem(body, scheme_chinese))) select_value(scheme_chinese, find_combo(scheme_chinese, L"五笔"));
    assert(store.load().snapshot->preferences().at("scheme") == "wubi");
    assert((GetWindowLongPtrW(GetDlgItem(body, scheme_wubi_profile), GWL_STYLE) & WS_VISIBLE) != 0);
    if (GetDlgItem(body, scheme_shuangpin_profile))
      assert((GetWindowLongPtrW(GetDlgItem(body, scheme_shuangpin_profile), GWL_STYLE) & WS_VISIBLE) == 0);
    select_value(scheme_wubi_profile, 1);
    assert(store.load().snapshot->preferences().at("wubi_profile") == "wubi98");
    const bool mixed_before = store.load().snapshot->preferences().at("wubi_mixed_pinyin").get<bool>();
    click(scheme_wubi_mixed);
    assert(store.load().snapshot->preferences().at("wubi_mixed_pinyin").get<bool>() != mixed_before);
    const bool hint_before = store.load().snapshot->preferences().at("wubi_code_hint").get<bool>();
    click(scheme_wubi_hint);
    assert(store.load().snapshot->preferences().at("wubi_code_hint").get<bool>() != hint_before);
    const bool unique_before = store.load().snapshot->preferences().at("wubi_auto_commit_unique").get<bool>();
    click(scheme_wubi_unique);
    assert(store.load().snapshot->preferences().at("wubi_auto_commit_unique").get<bool>() != unique_before);
    saved = store.load();
    const auto conflict_setup = store.save(*saved.snapshot, {{{"navigation", "minus_equal"}, true}, {{"word_character", "enabled"}, false}});
    assert(conflict_setup.outcome == Outcome::saved);
    SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_wubi_second)));
    click(scheme_wubi_selection);
    assert(IsWindowEnabled(GetDlgItem(body, scheme_wubi_second)));
    SetWindowTextW(GetDlgItem(body, scheme_wubi_second), L"'");
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_wubi_save)));
    SetWindowTextW(GetDlgItem(body, scheme_wubi_third), L";");
    assert(IsWindowEnabled(GetDlgItem(body, scheme_wubi_save)));
    const auto before_pair = store.load().snapshot->revision();
    click(scheme_wubi_save);
    saved = store.load();
    assert(saved.snapshot->revision() == before_pair + 1);
    assert(saved.snapshot->preferences().at("wubi_candidate_selection").at("second_key") == "'");
    assert(saved.snapshot->preferences().at("wubi_candidate_selection").at("third_key") == ";");
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_wubi_save)));
    // 翻页冲突交给 Rust 拒绝，表单草稿保留；无关偏好修改也不能把草稿抹掉。
    const auto before_invalid = saved.snapshot->revision();
    SetWindowTextW(GetDlgItem(body, scheme_wubi_second), L"-");
    click(scheme_wubi_save);
    assert(store.load().snapshot->revision() == before_invalid);
    assert(caption(GetDlgItem(body, scheme_wubi_second)) == L"-");
    click(scheme_wubi_hint);
    assert(caption(GetDlgItem(body, scheme_wubi_second)) == L"-");
    SetWindowTextW(GetDlgItem(body, scheme_wubi_second), L"'");
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_wubi_save)));
    const auto before_lookup = store.load().snapshot->revision();
    SetWindowTextW(GetDlgItem(body, scheme_wubi_lookup_text), L"字\r\n文");
    click(scheme_wubi_lookup);
    const auto lookup_text = caption(GetDlgItem(body, scheme_wubi_result));
    assert(lookup_text.find(L"字") != std::wstring::npos && lookup_text.find(L"文") != std::wstring::npos);
    assert(lookup_text.find(L"86 五笔") != std::wstring::npos && lookup_text.find(L"98 五笔") != std::wstring::npos);
    assert(lookup_text.find(L"失败") == std::wstring::npos);
    assert(store.load().snapshot->revision() == before_lookup);
    click(scheme_wubi_selection);
    assert(!IsWindowEnabled(GetDlgItem(body, scheme_wubi_second)));
    assert(store.load().snapshot->preferences().at("wubi_candidate_selection").at("second_key") == "'");
  }
  if (offered("vietnamese")) {
    if (IsWindowEnabled(GetDlgItem(body, scheme_language))) select_value(scheme_language, find_combo(scheme_language, L"越南文"));
    select_value(scheme_vietnamese_method, 1);
    select_value(scheme_vietnamese_tone, 1);
    saved = store.load();
    assert(saved.snapshot->preferences().at("vietnamese").at("input_method") == "vni");
    assert(saved.snapshot->preferences().at("vietnamese").at("tone_style") == "classic");
  }
  saved = store.load();
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  select_page(4);
  select_tab(L"候选释义");
  saved = store.load();
  auto gloss_seed = store.save(*saved.snapshot, {{{"candidate_english_gloss"}, false},
      {{"candidate_translations"}, false}, {{"candidate_pronunciation"}, true},
      {{"translation_target_language"}, "en"}, {{"translation_secondary_language"}, nullptr}});
  assert(gloss_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(!IsWindowEnabled(GetDlgItem(body, gloss_target)));
  assert(!IsWindowEnabled(GetDlgItem(body, gloss_pronunciation)));
  click(gloss_english);
  assert(IsWindowEnabled(GetDlgItem(body, gloss_target)));
  assert(IsWindowEnabled(GetDlgItem(body, gloss_pronunciation)));
  select_value(gloss_target, 2);
  select_value(gloss_secondary, 7);
  saved = store.load();
  assert(saved.snapshot->preferences().at("translation_target_language") == "ja");
  assert(saved.snapshot->preferences().at("translation_secondary_language") == "ko");
  click(gloss_english);
  assert(!IsWindowEnabled(GetDlgItem(body, gloss_secondary)));
  assert(store.load().snapshot->preferences().at("candidate_pronunciation") == true);
  assert(store.load().snapshot->preferences().at("translation_secondary_language") == "ko");
  click(gloss_translations);
  assert(IsWindowEnabled(GetDlgItem(body, gloss_secondary)));
  click(gloss_pronunciation);
  select_value(gloss_secondary, 0);
  saved = store.load();
  assert(saved.snapshot->preferences().at("candidate_pronunciation") == false);
  assert(!saved.snapshot->preferences().contains("translation_secondary_language"));
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  const auto expression_revision = saved.snapshot->revision();
  select_page(0);
  select_page(4);
  assert((GetWindowLongPtrW(GetDlgItem(body, gloss_target), GWL_STYLE) & WS_VISIBLE) != 0);
  select_tab(L"标点设置");
  assert((GetWindowLongPtrW(GetDlgItem(body, punctuation_mode), GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->revision() == expression_revision);
  select_tab(L"翻译服务");
  saved = store.load();
  auto translation_seed = store.save(*saved.snapshot, {{{"tencent_tmt", "enabled"}, true},
      {{"tencent_tmt", "secret_id"}, ""}, {{"tencent_tmt", "secret_key"}, ""}, {{"tencent_tmt", "region"}, "ap-guangzhou"},
      {{"niutrans", "enabled"}, false}, {{"niutrans", "app_id"}, ""}, {{"niutrans", "apikey"}, ""},
      {{"custom_translation", "enabled"}, false}, {{"custom_translation", "endpoint"}, ""},
      {{"custom_translation", "api_key"}, ""}, {{"translation_account"}, false}});
  assert(translation_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(SendMessageW(GetDlgItem(body, translation_provider_control), CB_GETCURSEL, 0, 0) == 0);
  select_tab(L"候选释义");
  click(gloss_translations);
  select_tab(L"翻译服务");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_provider_control)));
  select_tab(L"候选释义");
  click(gloss_translations);
  select_tab(L"翻译服务");
  const auto before_provider = store.load().snapshot->revision();
  select_value(translation_provider_control, 1);
  saved = store.load();
  assert(saved.snapshot->revision() == before_provider);
  assert(SendMessageW(GetDlgItem(body, translation_provider_control), CB_GETCURSEL, 0, 0) == 1);
  assert((GetWindowLongPtrW(GetDlgItem(body, translation_tencent_first), GWL_STYLE) & WS_VISIBLE) != 0);
  assert((GetWindowLongPtrW(GetDlgItem(body, translation_custom_first), GWL_STYLE) & WS_VISIBLE) == 0);
  // 空腾讯配置由真实 Rust 凭据接口在联网之前回答，测试不请求外部服务。
  const auto before_probe = saved.snapshot->revision();
  click(translation_tencent_first + 11);
  assert(caption(GetDlgItem(body, translation_tencent_first + 14)).find(L"凭据") != std::wstring::npos);
  assert(store.load().snapshot->revision() == before_probe);
  SetWindowTextW(GetDlgItem(body, translation_tencent_first), L"bad!");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_tencent_first + 10)));
  assert(!IsWindowEnabled(GetDlgItem(body, translation_tencent_first + 11)));
  assert(caption(GetDlgItem(body, translation_tencent_first + 14)).empty());
  SetWindowTextW(GetDlgItem(body, translation_tencent_first), L"SYNTHETIC_ID");
  SetWindowTextW(GetDlgItem(body, translation_tencent_first + 1), L"synthetic-not-a-real-secret");
  assert(SendMessageW(GetDlgItem(body, translation_tencent_first + 1), EM_GETPASSWORDCHAR, 0, 0) == L'*');
  click(translation_tencent_first + 12);
  assert(SendMessageW(GetDlgItem(body, translation_tencent_first + 1), EM_GETPASSWORDCHAR, 0, 0) == 0);
  click(translation_tencent_first + 12);
  SetWindowTextW(GetDlgItem(body, translation_tencent_first + 2), L"bad region");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_tencent_first + 10)));
  SetWindowTextW(GetDlgItem(body, translation_tencent_first + 2), L"ap-guangzhou");
  assert(IsWindowEnabled(GetDlgItem(body, translation_tencent_first + 10)));
  const auto before_credentials = store.load().snapshot->revision();
  click(translation_tencent_first + 10);
  saved = store.load();
  assert(saved.snapshot->revision() == before_credentials + 1);
  assert(saved.snapshot->preferences().at("tencent_tmt").at("secret_id") == "SYNTHETIC_ID");
  assert(saved.snapshot->preferences().at("tencent_tmt").at("secret_key") == "synthetic-not-a-real-secret");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_tencent_first + 10)));
  SetWindowTextW(GetDlgItem(body, translation_tencent_first), L"SYNTHETIC_DRAFT");
  const auto before_niutrans = store.load().snapshot->revision();
  select_value(translation_provider_control, 2);
  assert(store.load().snapshot->revision() == before_niutrans + 1);
  assert(caption(GetDlgItem(body, translation_tencent_first)) == L"SYNTHETIC_DRAFT");
  assert(store.load().snapshot->preferences().at("tencent_tmt").at("secret_id") == "SYNTHETIC_ID");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_niutrans_first + 11)));
  SetWindowTextW(GetDlgItem(body, translation_niutrans_first), L"FAKESECRET_example");
  SetWindowTextW(GetDlgItem(body, translation_niutrans_first + 1), L"synthetic-not-a-real-key");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_niutrans_first + 10)));
  SetWindowTextW(GetDlgItem(body, translation_niutrans_first), L"synthetic-app");
  click(translation_niutrans_first + 10);
  assert(store.load().snapshot->preferences().at("niutrans").at("app_id") == "synthetic-app");
  select_value(translation_provider_control, 1);
  assert(caption(GetDlgItem(body, translation_tencent_first)) == L"SYNTHETIC_DRAFT");
  select_value(translation_provider_control, 3);
  SetWindowTextW(GetDlgItem(body, translation_custom_first), L"http://translation.example.invalid/translate");
  assert(!IsWindowEnabled(GetDlgItem(body, translation_custom_first + 10)));
  assert(!IsWindowEnabled(GetDlgItem(body, translation_custom_first + 11)));
  assert(caption(GetDlgItem(body, translation_custom_first + 13)).find(L"HTTPS") != std::wstring::npos);
  SetWindowTextW(GetDlgItem(body, translation_custom_first), L"https://translation.example.invalid/translate");
  SetWindowTextW(GetDlgItem(body, translation_custom_first + 1), L"合成-key");
  const auto before_custom_credentials = store.load().snapshot->revision();
  click(translation_custom_first + 10);
  saved = store.load();
  assert(saved.snapshot->revision() == before_custom_credentials + 1);
  assert(saved.snapshot->preferences().at("custom_translation").at("api_key") == "合成-key");
  select_value(translation_provider_control, 4);
  saved = store.load();
  assert(saved.snapshot->preferences().at("translation_account") == true);
  assert(saved.snapshot->preferences().at("custom_translation").at("enabled") == false);
  assert(saved.snapshot->preferences().at("niutrans").at("enabled") == false);
  assert(saved.snapshot->preferences().at("tencent_tmt").at("enabled") == false);
  assert(saved.snapshot->preferences().at("custom_translation").at("api_key") == "合成-key");
  select_value(translation_provider_control, 0);
  assert(store.load().snapshot->preferences().at("translation_account") == false);
  assert(store.load().snapshot->preferences().at("tencent_tmt").at("secret_id") == "SYNTHETIC_ID");
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  // AI 输入辅助的原生控件复用地址校验和 CAS 存储；测试只写合成地址与密钥，不发网络请求。
  select_page(6);
  assert((GetWindowLongPtrW(GetDlgItem(body, ai_enabled), GWL_STYLE) & WS_VISIBLE) != 0);
  const auto ai_revision = store.load().snapshot->revision();
  SetWindowTextW(GetDlgItem(body, ai_endpoint), L"https://ai.example.invalid/v1/chat/completions");
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(ai_endpoint, EN_CHANGE), reinterpret_cast<LPARAM>(GetDlgItem(body, ai_endpoint)));
  assert(IsWindowEnabled(GetDlgItem(body, ai_service_save)) != FALSE);
  SetWindowTextW(GetDlgItem(body, ai_model), L"synthetic-model");
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(ai_model, CBN_EDITCHANGE), reinterpret_cast<LPARAM>(GetDlgItem(body, ai_model)));
  click(ai_service_save);
  saved = store.load();
  assert(saved.snapshot->revision() == ai_revision + 1);
  assert(saved.snapshot->preferences().at("ai_assistant").at("endpoint") == "https://ai.example.invalid/v1/chat/completions");
  assert(saved.snapshot->preferences().at("ai_assistant").at("model") == "synthetic-model");
  SetWindowTextW(GetDlgItem(body, ai_token), L"synthetic-ai-token");
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(ai_token, EN_CHANGE), reinterpret_cast<LPARAM>(GetDlgItem(body, ai_token)));
  click(ai_token_save);
  saved = store.load();
  const auto ai_origin = saved.snapshot->preferences().at("ai_assistant").at("tokens");
  assert(!ai_origin.empty());
  assert(std::any_of(ai_origin.begin(), ai_origin.end(), [](const auto& entry) {
    return entry.is_string() && entry.get<std::string>() == "synthetic-ai-token";
  }));
  assert(saved.snapshot->preferences().at("ai_assistant").at("token") == "");
  tokens = ai_origin;
  assert(IsWindowEnabled(GetDlgItem(body, ai_models_fetch)) != FALSE);
  const bool ai_was_enabled = saved.snapshot->preferences().at("ai_assistant").at("enabled").get<bool>();
  assert((IsWindowEnabled(GetDlgItem(body, ai_test)) != FALSE) == ai_was_enabled); // AI 总开关仍按原偏好控制测试按钮。
  const auto ai_before_toggle = saved.snapshot->revision();
  click(ai_enabled);
  saved = store.load();
  assert(saved.snapshot->revision() == ai_before_toggle + 1);
  assert(saved.snapshot->preferences().at("ai_assistant").at("enabled") == !ai_was_enabled);
  assert((IsWindowEnabled(GetDlgItem(body, ai_test)) != FALSE) == !ai_was_enabled);
  saved = store.load();
  // 候选外观的可选字段及已有范围外展示值不能因页面切换被静默归一。
  const auto appearance_seed = store.save(*saved.snapshot, {{{"candidate_page_size"}, 2},
      {{"candidate_scale_percent"}, 173}, {{"candidate_corner_radius"}, 24}});
  assert(appearance_seed.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(GetDlgItem(body, appearance_layout)) != FALSE; });
  const auto before_appearance = store.load().snapshot->revision();
  select_page(5);
  assert((GetWindowLongPtrW(GetDlgItem(body, appearance_layout), GWL_STYLE) & WS_VISIBLE) != 0);
  assert((GetWindowLongPtrW(tabs, GWL_STYLE) & WS_VISIBLE) == 0);
  const auto selected_number = [&](int id) {
    const auto control = GetDlgItem(body, id);
    return SendMessageW(control, CB_GETITEMDATA, SendMessageW(control, CB_GETCURSEL, 0, 0), 0);
  };
  assert(selected_number(appearance_page_size) == 2);
  assert(selected_number(appearance_scale) == 173);
  assert(selected_number(appearance_radius) == 24);
  assert(store.load().snapshot->revision() == before_appearance);
  select_value(appearance_layout, 0);
  assert(store.load().snapshot->preferences().at("candidate_layout") == "horizontal");
  assert(selected_number(appearance_scale) == 173);
  select_value(appearance_radius, 0);
  assert(!store.load().snapshot->preferences().contains("candidate_corner_radius"));
  select_value(appearance_radius, 9);
  assert(store.load().snapshot->preferences().at("candidate_corner_radius") == 8);
  select_value(appearance_preedit, 1);
  select_value(appearance_inline, 1);
  assert(store.load().snapshot->preferences().at("candidate_preedit_style") == "empty");
  assert(store.load().snapshot->preferences().at("tsf_preedit_style") == "pinyin");
  const bool previous_logo = store.load().snapshot->preferences().at("show_app_logo").get<bool>();
  click(appearance_logo);
  assert(store.load().snapshot->preferences().at("show_app_logo").get<bool>() != previous_logo);
  const auto before_fonts = store.load().snapshot->revision();
  select_value(appearance_font_preset, 1);
  saved = store.load();
  assert(saved.snapshot->revision() == before_fonts + 1);
  assert(saved.snapshot->preferences().at("candidate_font_family") == "SimSun");
  assert(saved.snapshot->preferences().at("candidate_fallback_fonts").at(0) == "SimSun");
  const auto font = GetDlgItem(body, appearance_font_family);
  const auto english_font = GetDlgItem(body, appearance_english_font);
  assert(SetWindowTextW(font, L"合成字体"));
  assert(SetWindowTextW(english_font, L"Synthetic Latin"));
  click(appearance_logo);
  assert(caption(font) == L"合成字体");
  click(appearance_fonts_refresh);
  assert(caption(font) == L"合成字体");
  assert(SendMessageW(font, CB_GETCOUNT, 0, 0) > 0);
  assert(caption(GetDlgItem(body, appearance_fonts_status)).find(L"已读取") != std::wstring::npos);
  const auto font_revision = store.load().snapshot->revision();
  click(appearance_fonts_save);
  saved = store.load();
  assert(saved.snapshot->revision() == font_revision + 1);
  assert(saved.snapshot->preferences().at("candidate_font_family") == "合成字体");
  assert(saved.snapshot->preferences().at("candidate_fallback_fonts").at(0) == "合成字体");
  assert(saved.snapshot->preferences().at("candidate_english_font") == "Synthetic Latin");
  assert(SetWindowTextW(font, L""));
  click(appearance_fonts_save);
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  assert(caption(font).empty());
  assert(caption(GetDlgItem(host, status_text)).find(L"字体名不能为空") != std::wstring::npos);
  // 外部写入后保存不能覆盖新快照，未提交字体草稿仍保留。
  assert(SetWindowTextW(font, L"合成草稿字体"));
  const auto concurrent = store.save(*saved.snapshot, {{{"candidate_follow_cursor"}, false}});
  assert(concurrent.outcome == Outcome::saved);
  click(appearance_fonts_save);
  assert(caption(GetDlgItem(host, status_text)).find(L"其他窗口") != std::wstring::npos);
  assert(caption(font) == L"合成草稿字体");
  assert(store.load().snapshot->revision() == concurrent.snapshot->revision());
  // 两张游戏程序表共享重复与数量约束；开关关闭不丢失表项。
  const auto game_name = GetDlgItem(body, appearance_game_name);
  assert(SetWindowTextW(game_name, L"  SYNTHETIC.EXE  "));
  click(appearance_game_add);
  assert(store.load().snapshot->preferences().at("game_compatibility").at("overlay_processes").at(0) == "synthetic.exe");
  const auto game_revision = store.load().snapshot->revision();
  select_value(appearance_game_list, 1);
  click(appearance_game_add);
  assert(store.load().snapshot->revision() == game_revision);
  assert(caption(GetDlgItem(host, status_text)).find(L"已经在") != std::wstring::npos);
  assert(SetWindowTextW(game_name, L"合成游戏.exe"));
  click(appearance_game_add);
  const auto games = store.load().snapshot->preferences().at("game_compatibility");
  assert(games.at("excluded_processes").at(0) == "合成游戏.exe");
  click(appearance_game_overlay);
  assert(store.load().snapshot->preferences().at("game_compatibility").at("overlay_processes") == games.at("overlay_processes"));
  const auto game_list = GetDlgItem(body, appearance_game_never);
  SendMessageW(game_list, LB_SETCURSEL, 0, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(appearance_game_never, LBN_SELCHANGE), reinterpret_cast<LPARAM>(game_list));
  click(appearance_game_never_remove);
  assert(store.load().snapshot->preferences().at("game_compatibility").at("excluded_processes").empty());
  assert(!IsWindowEnabled(GetDlgItem(body, appearance_game_never_remove)));
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  saved = store.load();
  // 主题目录、皮肤、配色和预览走实际控件、串行 worker 与同源 Rust。
  const auto before_themes = store.load().snapshot->document;
  select_page(8);
  assert((GetWindowLongPtrW(GetDlgItem(body, theme_global), GWL_STYLE) & WS_VISIBLE) != 0);
  assert(store.load().snapshot->document == before_themes);
  const auto theme_response = theme_data::value(theme_data::response(msime_client_theme_catalog()));
  assert(theme_response && theme_response->at("themes").is_array());
  assert(SendMessageW(GetDlgItem(body, theme_global), CB_GETCOUNT, 0, 0) == static_cast<LRESULT>(theme_response->at("themes").size()));
  const auto select_theme = [&](const char* id) {
    const auto& themes = theme_response->at("themes");
    for (size_t index = 0; index < themes.size(); ++index) if (themes[index].at("id") == id) {
      select_value(theme_global, static_cast<int>(index)); return;
    }
    assert(false);
  };
  select_theme("paper");
  assert(store.load().snapshot->preferences().at("global_theme") == "paper");
  const auto paper = store.load().snapshot->revision();
  select_theme("paper");
  assert(store.load().snapshot->revision() == paper);
  select_value(theme_mode, 2);
  select_value(theme_candidate_mode, 1);
  assert(store.load().snapshot->preferences().at("theme") == "dark");
  assert(store.load().snapshot->preferences().at("candidate_theme") == "light");
  const auto before_preview = store.load().snapshot->document;
  select_value(theme_preview_mode, 2);
  assert(store.load().snapshot->document == before_preview);
  select_value(theme_preview_mode, 0);
  assert(store.load().snapshot->document == before_preview);
  const auto skin_list = GetDlgItem(body, theme_skins);
  assert(SendMessageW(skin_list, LB_GETCOUNT, 0, 0) == 1);
  click(theme_skin_import);
  assert(skin_confirmations == 1 && !std::filesystem::exists(skins_root / L"synthetic-light"));
  allow_skin_import = true;
  click(theme_skin_import);
  if (skin_confirmations != 2 || !std::filesystem::is_regular_file(skins_root / L"synthetic-light" / L"skin.toml")) {
    std::fprintf(stderr, "Synthetic skin import: confirmations=%u catalog=%s\n", skin_confirmations,
        theme_data::catalog(imported_skin.parent_path()).dump().c_str());
    std::fwprintf(stderr, L"Synthetic skin import status: %ls\n", caption(GetDlgItem(host, status_text)).c_str());
    std::abort();
  }
  assert(SendMessageW(skin_list, LB_GETCOUNT, 0, 0) == 2);
  assert(store.load().snapshot->document == before_preview);
  {
    std::ofstream stale(skins_root / L"synthetic-light" / L"stale.txt");
    stale << "synthetic obsolete file";
    assert(stale.good());
  }
  click(theme_skin_import);
  assert(skin_confirmations == 3 && !std::filesystem::exists(skins_root / L"synthetic-light" / L"stale.txt"));
  cancel_skin_folder = true;
  click(theme_skin_import);
  assert(skin_confirmations == 3);
  const auto choose_skin = [&](const wchar_t* id) {
    for (LRESULT index = 0; index < SendMessageW(skin_list, LB_GETCOUNT, 0, 0); ++index) {
      wchar_t title[256]{};
      assert(SendMessageW(skin_list, LB_GETTEXTLEN, index, 0) < 256);
      SendMessageW(skin_list, LB_GETTEXT, index, reinterpret_cast<LPARAM>(title));
      if (std::wstring(title).find(id) == std::wstring::npos) continue;
      SendMessageW(skin_list, LB_SETCURSEL, index, 0);
      SendMessageW(body, WM_COMMAND, MAKEWPARAM(theme_skins, LBN_SELCHANGE), reinterpret_cast<LPARAM>(skin_list));
      return;
    }
    assert(false);
  };
  choose_skin(L"synthetic-light");
  assert(IsWindowEnabled(GetDlgItem(body, theme_skin_use)));
  const auto before_skin = store.load().snapshot->revision();
  click(theme_skin_use);
  saved = store.load();
  assert(saved.snapshot->revision() == before_skin + 1);
  assert(saved.snapshot->preferences().at("global_theme") == "custom");
  assert(saved.snapshot->preferences().at("custom_theme").at("candidate_skin") == "synthetic-light");
  const auto resolved_skin = theme_data::preview(saved.snapshot->preferences(), skins_root, 1);
  assert(resolved_skin.resolved.at("candidate_skin") == "synthetic-light");
  assert(resolved_skin.background.width == 1 && resolved_skin.decoration.width == 1 && !resolved_skin.image_failed);
  assert(resolved_skin.background.bgra == std::vector<uint8_t>({0, 0, 255, 255}));
  const auto before_custom_choice = saved.snapshot->revision();
  select_theme("custom");
  saved = store.load();
  assert(saved.snapshot->revision() == before_custom_choice + 1);
  assert(!saved.snapshot->preferences().at("custom_theme").contains("candidate_skin"));
  choose_skin(L"synthetic-light");
  click(theme_skin_use);
  choose_skin(L"synthetic-dark");
  click(theme_skin_use);
  saved = store.load();
  assert(saved.snapshot->preferences().at("custom_theme").at("candidate_skin") == "synthetic-light");
  assert(saved.snapshot->preferences().at("custom_theme").at("candidate_skin_dark") == "synthetic-dark");
  assert(caption(GetDlgItem(body, theme_slot_summary)).find(L"synthetic-light") != std::wstring::npos);
  assert(caption(GetDlgItem(body, theme_slot_summary)).find(L"synthetic-dark") != std::wstring::npos);
  click(theme_skin_stop);
  saved = store.load();
  assert(saved.snapshot->preferences().at("custom_theme").at("candidate_skin") == "synthetic-light");
  assert(!saved.snapshot->preferences().at("custom_theme").contains("candidate_skin_dark"));
  const auto surface_edit = GetDlgItem(body, theme_color_first);
  SetWindowTextW(surface_edit, L"#12XX56");
  const auto before_invalid_color = saved.snapshot->revision();
  click(theme_color_first + 1);
  assert(store.load().snapshot->revision() == before_invalid_color);
  assert(caption(GetDlgItem(host, status_text)).find(L"#RRGGBB") != std::wstring::npos);
  select_theme("paper");
  SetWindowTextW(surface_edit, L"#123ABC");
  const auto before_color = store.load().snapshot->revision();
  click(theme_color_first + 1);
  saved = store.load();
  assert(saved.snapshot->revision() == before_color + 1);
  assert(saved.snapshot->preferences().at("global_theme") == "custom");
  assert(saved.snapshot->preferences().at("custom_theme").at("base") == "paper");
  assert(saved.snapshot->preferences().at("custom_theme").at("candidate_colors").at("surface") == "#123abc");
  assert(!saved.snapshot->preferences().at("custom_theme").contains("candidate_skin"));
  assert(!saved.snapshot->preferences().at("custom_theme").contains("candidate_skin_dark"));
  // 从解析出来的调色板绘制，确认没有继续画固定灰色占位。
  const auto preview_dc = CreateCompatibleDC(nullptr);
  BITMAPINFO preview_info{};
  preview_info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  preview_info.bmiHeader.biWidth = 600; preview_info.bmiHeader.biHeight = -228;
  preview_info.bmiHeader.biPlanes = 1; preview_info.bmiHeader.biBitCount = 32;
  void* preview_bits = nullptr;
  const auto preview_bitmap = CreateDIBSection(preview_dc, &preview_info, DIB_RGB_COLORS, &preview_bits, nullptr, 0);
  assert(preview_dc && preview_bitmap && preview_bits);
  const auto old_preview_bitmap = SelectObject(preview_dc, preview_bitmap);
  DRAWITEMSTRUCT draw{};
  draw.CtlType = ODT_STATIC; draw.CtlID = theme_preview; draw.hwndItem = GetDlgItem(body, theme_preview);
  draw.hDC = preview_dc; draw.rcItem = RECT{0, 0, 600, 228};
  assert(SendMessageW(body, WM_DRAWITEM, theme_preview, reinterpret_cast<LPARAM>(&draw)));
  // 本用例前面可能调整过不透明度，取当前数值计算合成后的表面色。
  const int alpha = saved.snapshot->preferences().at("candidate_opacity_percent").get<int>() * 255 / 100;
  const auto native = GetSysColor(COLOR_BTNFACE);
  const auto channel = [alpha](int a, int b) { return (a * alpha + b * (255 - alpha) + 127) / 255; };
  assert(GetPixel(preview_dc, 570, 200) == RGB(channel(0x12, GetRValue(native)), channel(0x3a, GetGValue(native)), channel(0xbc, GetBValue(native))));
  SelectObject(preview_dc, old_preview_bitmap); DeleteObject(preview_bitmap); DeleteDC(preview_dc);
  click(theme_color_first + 2);
  assert(!store.load().snapshot->preferences().at("custom_theme").contains("candidate_colors"));
  SetWindowTextW(surface_edit, L"#654321");
  click(theme_skin_refresh);
  assert(caption(surface_edit) == L"#654321");
  // CAS 冲突不能覆盖另一个窗口刚选择的主题。
  saved = store.load();
  auto theme_peer = store.save(*saved.snapshot, {{{"global_theme"}, "night"}});
  assert(theme_peer.outcome == Outcome::saved);
  select_theme("light");
  assert(store.load().snapshot->revision() == theme_peer.snapshot->revision());
  assert(store.load().snapshot->preferences().at("global_theme") == "night");
  assert(caption(GetDlgItem(host, status_text)).find(L"其他窗口") != std::wstring::npos);
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  saved = store.load();
  // 原有自定义尺寸只在用户选择标准档位后才改变；两个尺寸字段原子保存。
  select_page(1);
  const HWND size = GetDlgItem(body, toolbar_size);
  assert((GetWindowLongPtrW(size, GWL_STYLE) & WS_VISIBLE) != 0);
  assert((GetWindowLongPtrW(tabs, GWL_STYLE) & WS_VISIBLE) == 0);
#if MSIME_EDITION_HANDWRITING
  assert(GetDlgItem(body, toolbar_handwriting) != nullptr);
#else
  assert(GetDlgItem(body, toolbar_handwriting) == nullptr);
#endif
  auto custom = store.save(*saved.snapshot, {{{"floating_toolbar", "scale_percent"}, 110},
      {{"floating_toolbar", "font_size"}, 20}});
  assert(custom.outcome == Outcome::saved);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(reload_settings, BN_CLICKED), 0);
  pump_until([&] { return IsWindowEnabled(size) != FALSE; });
  assert(SendMessageW(size, CB_GETCOUNT, 0, 0) == 6);
  assert(SendMessageW(size, CB_GETCURSEL, 0, 0) == 5);
  assert(store.load().snapshot->revision() == custom.snapshot->revision());
  const auto custom_toolbar_dimensions = caption(GetDlgItem(body, toolbar_preview_description));
  assert(custom_toolbar_dimensions.find(L"DIP") != std::wstring::npos);
  SendMessageW(size, CB_SETCURSEL, 4, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(toolbar_size, CBN_SELCHANGE), reinterpret_cast<LPARAM>(size));
  pump_until([&] { return IsWindowEnabled(size) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->revision() == custom.snapshot->revision() + 1);
  assert(saved.snapshot->preferences().at("floating_toolbar").at("scale_percent") == 150);
  assert(saved.snapshot->preferences().at("floating_toolbar").at("font_size") == 28);
  assert(SendMessageW(size, CB_GETCOUNT, 0, 0) == 5);
  const auto standard_toolbar_dimensions = caption(GetDlgItem(body, toolbar_preview_description));
  assert(standard_toolbar_dimensions != custom_toolbar_dimensions);
  const HWND emoji = GetDlgItem(body, toolbar_emoji);
  const bool previous_emoji = saved.snapshot->preferences().at("floating_toolbar").at("emoji").get<bool>();
  SendMessageW(emoji, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(emoji) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->preferences().at("floating_toolbar").at("emoji").get<bool>() != previous_emoji);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  assert(caption(GetDlgItem(body, toolbar_preview_description)) != standard_toolbar_dimensions);
  {
    const auto toolbar_dc = CreateCompatibleDC(nullptr);
    void* bits = nullptr;
    const auto bitmap = CreateDIBSection(toolbar_dc, &preview_info, DIB_RGB_COLORS, &bits, nullptr, 0);
    assert(toolbar_dc && bitmap && bits);
    const auto previous = SelectObject(toolbar_dc, bitmap);
    DRAWITEMSTRUCT drawing{};
    drawing.CtlType = ODT_STATIC; drawing.CtlID = toolbar_preview; drawing.hwndItem = GetDlgItem(body, toolbar_preview);
    drawing.hDC = toolbar_dc; drawing.rcItem = RECT{0,0,600,228};
    assert(SendMessageW(body, WM_DRAWITEM, toolbar_preview, reinterpret_cast<LPARAM>(&drawing)));
    assert(GetPixel(toolbar_dc, 100, 20) != GetSysColor(COLOR_BTNFACE));
    SelectObject(toolbar_dc, previous); DeleteObject(bitmap); DeleteDC(toolbar_dc);
  }
  const auto before_toolbar_jump = saved.snapshot->revision();
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(toolbar_theme, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(body, toolbar_theme)));
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(SendMessageW(GetDlgItem(host, page_selector), CB_GETCURSEL, 0, 0) == 8);
  assert(store.load().snapshot->revision() == before_toolbar_jump);
  select_page(2);
  const HWND server_log = GetDlgItem(body, diagnostic_server);
  const HWND tsf_log = GetDlgItem(body, diagnostic_tsf);
  assert(server_log && tsf_log);
  assert((GetWindowLongPtrW(server_log, GWL_STYLE) & WS_VISIBLE) != 0);
  assert(caption(GetDlgItem(body, data_directory_text)) == temporary.path.wstring());
  const bool previous_server = saved.snapshot->preferences().at("diagnostic_log").at("server").get<bool>();
  SendMessageW(server_log, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(server_log) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->preferences().at("diagnostic_log").at("server").get<bool>() != previous_server);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(export_settings, BN_CLICKED),
      reinterpret_cast<LPARAM>(GetDlgItem(body, export_settings)));
  pump_until([&] { return IsWindowEnabled(server_log) != FALSE; });
  if (!std::filesystem::is_regular_file(file)) {
    std::fwprintf(stderr, L"Export status: %ls\n", caption(GetDlgItem(host, status_text)).c_str());
    std::abort();
  }
  const auto exported_revision = saved.snapshot->revision();
  assert(store.load().snapshot->revision() == exported_revision);
  const bool exported_caps = saved.snapshot->preferences().at("keybindings").at("switch_language_caps_lock").get<bool>();
  select_page(0);
  SendMessageW(caps, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(caps) != FALSE; });
  assert(store.load().snapshot->preferences().at("keybindings").at("switch_language_caps_lock").get<bool>() != exported_caps);
  select_page(2);
  // 文件选择边界使用合成路径；实际导入不能恢复文件中不应携带的本机日志开关。
  SendMessageW(tsf_log, BM_CLICK, 0, 0);
  pump_until([&] { return IsWindowEnabled(tsf_log) != FALSE; });
  saved = store.load();
  const bool current_tsf = saved.snapshot->preferences().at("diagnostic_log").at("tsf").get<bool>();
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(import_settings, BN_CLICKED),
      reinterpret_cast<LPARAM>(GetDlgItem(body, import_settings)));
  pump_until([&] { return IsWindowEnabled(tsf_log) != FALSE; });
  saved = store.load();
  assert(saved.snapshot->revision() == exported_revision + 3);
  assert(saved.snapshot->preferences().at("keybindings").at("switch_language_caps_lock").get<bool>() == exported_caps);
  assert(saved.snapshot->preferences().at("diagnostic_log").at("tsf").get<bool>() == current_tsf);
  assert(saved.snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  assert(selections == 2);
  cancel_selection = true;
  const auto before_cancel = saved.snapshot->revision();
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(import_settings, BN_CLICKED),
      reinterpret_cast<LPARAM>(GetDlgItem(body, import_settings)));
  assert(selections == 2 && IsWindowEnabled(tsf_log));
  assert(store.load().snapshot->revision() == before_cancel);
  // 社区传输用合成服务响应；实际控件、串行 worker、预览解码和本地包修改使用正式路径。
  const auto community_revision = store.load().snapshot->revision();
  const auto community_click = [&](int id) {
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(body, id)));
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  };
  select_page(9);
  assert(community_calls.size() == 1 && community_calls.back().at("operation") == "candidate_skin_list");
  assert(SendMessageW(GetDlgItem(body, community_list), LB_GETCOUNT, 0, 0) == 1);
  assert(IsWindowEnabled(GetDlgItem(body, community_more)));
  assert(!IsWindowEnabled(GetDlgItem(body, community_unpublish)));
  community_click(community_more);
  assert(community_calls.back().at("offset") == 1);
  assert(SendMessageW(GetDlgItem(body, community_list), LB_GETCOUNT, 0, 0) == 2);
  assert(!IsWindowEnabled(GetDlgItem(body, community_more)));
  SetWindowTextW(GetDlgItem(body, community_search), L"合成搜索");
  SendMessageW(GetDlgItem(body, community_filter), CB_SETCURSEL, 2, 0);
  SendMessageW(GetDlgItem(body, community_mine), BM_SETCHECK, BST_CHECKED, 0);
  community_click(community_reload);
  assert(community_calls.back().at("offset") == 0 && community_calls.back().at("category") == "guofeng");
  assert(community_calls.back().at("search") == "合成搜索" && community_calls.back().at("mine") == true);
  community_click(community_detail);
  community_click(community_preview_load);
  const auto frame = community_data::preview(Json{{"ok", true}, {"value", {{"dataUrl", "data:image/png;base64," + preview_base64}}}});
  assert(frame.width == 1 && frame.height == 1 && frame.bgra == std::vector<uint8_t>({0,0,255,255}));
  const auto community_dc = CreateCompatibleDC(nullptr);
  void* community_bits = nullptr;
  const auto community_bitmap = CreateDIBSection(community_dc, &preview_info, DIB_RGB_COLORS, &community_bits, nullptr, 0);
  assert(community_dc && community_bitmap && community_bits);
  const auto previous_community_bitmap = SelectObject(community_dc, community_bitmap);
  DRAWITEMSTRUCT community_draw{};
  community_draw.CtlType = ODT_STATIC; community_draw.CtlID = community_preview_image;
  community_draw.hwndItem = GetDlgItem(body, community_preview_image); community_draw.hDC = community_dc;
  community_draw.rcItem = RECT{0,0,600,228};
  assert(SendMessageW(body, WM_DRAWITEM, community_preview_image, reinterpret_cast<LPARAM>(&community_draw)));
  assert(GetPixel(community_dc, 300, 114) == RGB(255,0,0));
  SelectObject(community_dc, previous_community_bitmap); DeleteObject(community_bitmap); DeleteDC(community_dc);
  assert(community_data::preview(Json{{"ok", true}, {"value", {{"dataUrl", "https://invalid.example/preview.png"}}}}).bgra.empty());
  assert(community_data::preview(Json{{"ok", true}, {"value", {{"dataUrl", "data:image/jpeg;base64," + preview_base64}}}}).bgra.empty());
  community_click(community_install);
  assert(installs == 1 && community_calls.back().at("replace") == false);
  const auto before_denial = community_calls.size();
  community_click(community_install);
  assert(community_calls.size() == before_denial && community_confirmations == 1);
  allow_community = true;
  community_click(community_install);
  assert(installs == 2 && community_calls.back().at("replace") == true);
  SendMessageW(GetDlgItem(body, community_stars), CB_SETCURSEL, 3, 0);
  community_click(community_rate);
  assert(community_calls.back().at("stars") == 4);
  SetWindowTextW(GetDlgItem(body, community_report_detail), L"合成举报说明");
  allow_community = false;
  auto call_count = community_calls.size();
  community_click(community_report);
  assert(community_calls.size() == call_count);
  allow_community = true;
  community_click(community_report);
  assert(community_calls.back().at("detail") == "合成举报说明");
  community_item["owned"] = true;
  community_click(community_detail);
  assert(IsWindowEnabled(GetDlgItem(body, community_unpublish)) && !IsWindowEnabled(GetDlgItem(body, community_rate)));
  SendMessageW(GetDlgItem(body, community_visibility), CB_SETCURSEL, 1, 0);
  community_click(community_visibility_save);
  assert(community_item.at("visibility") == "private");
  SendMessageW(GetDlgItem(body, community_category), CB_SETCURSEL, 3, 0);
  community_click(community_category_save);
  assert(community_item.at("category") == "cute");
  allow_community = false;
  call_count = community_calls.size();
  community_click(community_sync);
  community_click(community_unpublish);
  assert(community_calls.size() == call_count);
  allow_community = true;
  community_click(community_sync);
  assert(community_calls.back().at("operation") == "candidate_skin_sync");
  community_click(community_unpublish);
  assert(SendMessageW(GetDlgItem(body, community_list), LB_GETCOUNT, 0, 0) == 0);
  assert(std::filesystem::is_directory(skins_root / L"synthetic-light"));
  community_unauthorized = true;
  community_click(community_reload);
  assert(IsWindowEnabled(GetDlgItem(body, community_login)));
  community_unauthorized = false;
  malformed_community = true;
  community_click(community_reload);
  assert(SendMessageW(GetDlgItem(body, community_list), LB_GETCOUNT, 0, 0) == 0);
  malformed_community = false;
  community_click(community_reload);
  assert(SendMessageW(GetDlgItem(body, community_list), LB_GETCOUNT, 0, 0) == 1);
  assert(SendMessageW(GetDlgItem(body, community_publish_skin), CB_GETCOUNT, 0, 0) == 2);
  // 未确认权利或名称无效时不请求发布；失败后重试沿用同一 UUID。
  call_count = community_calls.size();
  community_click(community_publish);
  assert(community_calls.size() == call_count);
  SetWindowTextW(GetDlgItem(body, community_publish_name), L"合成发布名称");
  SetWindowTextW(GetDlgItem(body, community_publish_description), L"合成发布说明");
  SendMessageW(GetDlgItem(body, community_publish_agree), BM_SETCHECK, BST_CHECKED, 0);
  allow_community = false;
  community_click(community_publish);
  assert(community_calls.size() == call_count);
  allow_community = true;
  community_click(community_publish);
  assert(community_calls.back().at("operation") == "candidate_skin_publish");
  const auto publication_id = community_calls.back().at("id");
  community_click(community_publish);
  assert(community_calls.back().at("id") == publication_id);
  SetWindowTextW(GetDlgItem(body, community_publish_license), L"CC0-1.0");
  community_click(community_license_save);
  assert(community_calls.back().at("operation") == "candidate_skin_add_license");
  const auto publication_skin = community_calls.back().at("skin_id").get<std::string>();
  const auto local_package = [&] {
    return theme_data::value(theme_data::call(msime_client_skin_package, {{"directory", skins_root.u8string()}, {"id", publication_skin}})).value();
  };
  assert(local_package().at("license").at("assets") == "CC0-1.0");
  preview_selection = true;
  cancel_selection = true;
  call_count = community_calls.size();
  community_click(community_preview_add);
  assert(community_calls.size() == call_count);
  cancel_selection = false;
  community_click(community_preview_add);
  assert(community_calls.back().at("image") == preview_base64);
  const auto preview_path = local_package().at("preview");
  assert(preview_path.is_string());
  assert(std::filesystem::is_regular_file(skins_root / std::filesystem::u8path(publication_skin) / std::filesystem::u8path(preview_path.get<std::string>())));
  preview_selection = false;
  community_click(community_pack_check);
  assert(community_calls.back().at("operation") == "candidate_skin_pack_preview");
  assert(control_text::read(GetDlgItem(body, community_notice))->find("检查通过") != std::string::npos);
  const auto signed_out_sync = theme_data::call(msime_client_community,
      {{"directory", (temporary.path / L"synthetic-account").u8string()}, {"skins_directory", skins_root.u8string()},
       {"action", {{"operation", "candidate_skin_sync"}}}});
  assert(signed_out_sync.at("ok") == false && signed_out_sync.at("error") == "community_unauthorized");
  assert(store.load().snapshot->revision() == community_revision);
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  community_click(community_local);
  assert(SendMessageW(GetDlgItem(host, page_selector), CB_GETCURSEL, 0, 0) == 8);
  // 统计由正式共享接口读写合成状态；云端只注入合成响应，不访问账号。
  const auto statistics_revision = store.load().snapshot->revision();
  const auto statistics_call = [&](const Json& action) {
    return theme_data::call(msime_client_typing_statistics, {{"directory", temporary.path.u8string()}, {"action", action}});
  };
  const auto statistics_value = [&] { return theme_data::value(statistics_call({{"operation", "load"}})).value(); };
  select_page(10);
  // 进入页面必须读取上传状态；同一消息循环也可能执行到期的自动刷新。
  assert(!statistics_upload_calls.empty() && statistics_upload_calls.front().at("operation") == "typing_upload");
  assert(SendMessageW(GetDlgItem(body, statistics_enabled), BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  const auto statistics_click = [&](int id) {
    SendMessageW(GetDlgItem(body, id), BM_CLICK, 0, 0);
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  };
  statistics_click(statistics_enabled);
  assert(statistics_value().at("enabled") == true);
  const auto today = statistics_data::model::day_key(statistics_data::local_day());
  const auto old_day = statistics_data::model::day_key(statistics_data::model::shift(statistics_data::local_day(), -400));
  assert(theme_data::value(statistics_call({{"operation", "record"}, {"text", "合成abcdef123!"}, {"source", "quanpin"}, {"day", today}, {"hour", 10}})));
  assert(theme_data::value(statistics_call({{"operation", "record"}, {"text", "合成旧日"}, {"source", "quanpin"}, {"day", old_day}})));
  assert(theme_data::value(statistics_call({{"operation", "record_keys"}, {"day", today}, {"keys", {{"KeyA", 4}, {"Space", 2}}}})));
  {
    // 活跃时长由输入会话采样；合成磁盘快照继续经正式读取校验与页面派生。
    const auto file_path = temporary.path / L"typing-statistics.json";
    std::ifstream input(file_path, std::ios::binary);
    auto fixture = Json::parse(input); input.close();
    fixture["dailyActiveMs"][today] = 120000;
    std::ofstream output(file_path, std::ios::binary | std::ios::trunc); output << fixture.dump(); assert(output.good());
  }
  SendMessageW(host, WM_TIMER, 72, 0);
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(control_text::read(GetDlgItem(body, statistics_chart_text))->find("4") != std::string::npos);
  assert(SendMessageW(GetDlgItem(body, statistics_days), LB_GETCOUNT, 0, 0) == 2);
  const auto statistics_dc = CreateCompatibleDC(nullptr);
  void* statistics_bits = nullptr;
  const auto statistics_bitmap = CreateDIBSection(statistics_dc, &preview_info, DIB_RGB_COLORS, &statistics_bits, nullptr, 0);
  assert(statistics_dc && statistics_bitmap && statistics_bits);
  const auto old_statistics_bitmap = SelectObject(statistics_dc, statistics_bitmap);
  const auto statistics_pixels = [&](int control = statistics_chart) {
    DRAWITEMSTRUCT drawing{};
    drawing.CtlType = ODT_STATIC; drawing.CtlID = control; drawing.hwndItem = GetDlgItem(body, control);
    drawing.hDC = statistics_dc; drawing.rcItem = RECT{0,0,600,228};
    assert(SendMessageW(body, WM_DRAWITEM, control, reinterpret_cast<LPARAM>(&drawing)));
    unsigned colored = 0;
    for (int y = 0; y < 228; ++y) for (int x = 0; x < 600; ++x)
      if (GetPixel(statistics_dc, x, y) == RGB(25,167,141)) ++colored;
    return colored;
  };
  assert(statistics_pixels() > 100);
  assert(statistics_pixels(statistics_calendar) > 100);
  assert(statistics_pixels(statistics_speed) > 100);
  SendMessageW(GetDlgItem(body, statistics_chart_kind), CB_SETCURSEL, 1, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_chart_kind, CBN_SELCHANGE), reinterpret_cast<LPARAM>(GetDlgItem(body, statistics_chart_kind)));
  assert(statistics_pixels() > 100);
  SelectObject(statistics_dc, old_statistics_bitmap); DeleteObject(statistics_bitmap); DeleteDC(statistics_dc);
  const auto stats_count_before_view = statistics_value().at("total").get<uint64_t>();
  const auto upload_count_before_view = statistics_upload_calls.size();
  for (int chart = 0; chart < 6; ++chart) {
    const auto combo = GetDlgItem(body, statistics_chart_kind);
    SendMessageW(combo, CB_SETCURSEL, chart, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_chart_kind, CBN_SELCHANGE), reinterpret_cast<LPARAM>(combo));
    assert(IsWindowEnabled(language));
  }
  assert(statistics_upload_calls.size() == upload_count_before_view && statistics_value().at("total") == stats_count_before_view);
  SendMessageW(GetDlgItem(body, statistics_days), LB_SETCURSEL, 0, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_days, LBN_SELCHANGE), reinterpret_cast<LPARAM>(GetDlgItem(body, statistics_days)));
  assert(control_text::read(GetDlgItem(body, statistics_day_detail))->find(today) != std::string::npos);
  const auto retention = GetDlgItem(body, statistics_retention);
  SendMessageW(retention, CB_SETCURSEL, 1, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_retention, CBN_SELCHANGE), reinterpret_cast<LPARAM>(retention));
  assert(statistics_value().at("days").contains(old_day));
  assert(SendMessageW(retention, CB_GETCURSEL, 0, 0) == 0);
  allow_statistics = true;
  SendMessageW(retention, CB_SETCURSEL, 1, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_retention, CBN_SELCHANGE), reinterpret_cast<LPARAM>(retention));
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(!statistics_value().at("days").contains(old_day) && statistics_value().at("days").contains(today));
  assert(statistics_value().at("retention") == "30d");
  const auto confirmations_before_extending = statistics_confirmations;
  SendMessageW(retention, CB_SETCURSEL, 2, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(statistics_retention, CBN_SELCHANGE), reinterpret_cast<LPARAM>(retention));
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(statistics_value().at("retention") == "90d" && statistics_confirmations == confirmations_before_extending);
  allow_statistics = false;
  auto upload_count = statistics_upload_calls.size();
  statistics_click(statistics_upload);
  assert(statistics_upload_calls.size() == upload_count && SendMessageW(GetDlgItem(body, statistics_upload), BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  allow_statistics = true;
  fail_statistics_upload = true;
  statistics_click(statistics_upload);
  assert(statistics_upload_enabled);
  assert(statistics_upload_calls.back().at("operation") == "typing_upload_status");
  assert(SendMessageW(GetDlgItem(body, statistics_upload), BM_GETCHECK, 0, 0) == BST_CHECKED);
  assert(control_text::read(GetDlgItem(body, statistics_upload_notice))->find("已打开上传") != std::string::npos);
  fail_statistics_upload = false;
  statistics_click(statistics_upload);
  assert(!statistics_upload_enabled);
  allow_statistics = false;
  statistics_click(statistics_reset);
  assert(statistics_value().at("total") == stats_count_before_view - 4);
  allow_statistics = true;
  statistics_click(statistics_reset);
  assert(statistics_value().at("total") == 0 && statistics_value().at("days").empty());
  assert(statistics_confirmations >= 6);
  statistics_click(statistics_enabled);
  assert(statistics_value().at("enabled") == false);
  const auto statistics_file = temporary.path / L"typing-statistics.json";
  std::string statistics_document;
  {
    std::ifstream input(statistics_file, std::ios::binary);
    statistics_document.assign(std::istreambuf_iterator<char>(input), {});
    assert(statistics_document.find("合成abcdef123!") == std::string::npos);
    std::ofstream broken(statistics_file, std::ios::binary | std::ios::trunc); broken << "{synthetic-broken"; assert(broken.good());
  }
  SendMessageW(host, WM_TIMER, 72, 0);
  pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  assert(control_text::read(GetDlgItem(body, statistics_notice))->find("不会被自动重置") != std::string::npos);
  {
    std::ifstream input(statistics_file, std::ios::binary);
    assert(std::string(std::istreambuf_iterator<char>(input), {}) == "{synthetic-broken");
    input.close();
    std::ofstream restore(statistics_file, std::ios::binary | std::ios::trunc); restore << statistics_document; assert(restore.good());
  }
  assert(store.load().snapshot->revision() == statistics_revision);
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  select_page(0);
  upload_count = statistics_upload_calls.size();
  SendMessageW(host, WM_TIMER, 72, 0);
  assert(statistics_upload_calls.size() == upload_count);
  // 账号操作只接合成传输，正式控件与 worker 串起资料和同步；不读取真实会话。
  const auto account_click = [&](int id) {
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(body, id)));
    pump_until([&] { return IsWindowEnabled(language) != FALSE; });
  };
  const auto account_revision = store.load().snapshot->revision();
  select_page(7);
  assert(account_calls.size() == 2 && account_calls.back().at("operation") == "status");
  assert(IsWindowEnabled(GetDlgItem(body, account_email)) && !IsWindowEnabled(GetDlgItem(body, account_sync_upload)));
  account_click(account_email);
  assert(SetWindowTextW(GetDlgItem(body, account_target), L"  synthetic@example.invalid  "));
  account_click(account_request_code);
  assert(!IsWindowEnabled(GetDlgItem(body, account_request_code)));
  assert(SetWindowTextW(GetDlgItem(body, account_code), L"12345"));
  assert(!IsWindowEnabled(GetDlgItem(body, account_login)));
  assert(SetWindowTextW(GetDlgItem(body, account_code), L"123456"));
  assert(IsWindowEnabled(GetDlgItem(body, account_login)));
  assert(SetWindowTextW(GetDlgItem(body, account_target), L"synthetic@example.invalid"));
  assert(IsWindowEnabled(GetDlgItem(body, account_login)));
  account_click(account_login);
  assert(account_signed_in && account_calls.back().at("operation") == "preferences_load");
  assert(caption(GetDlgItem(body, account_profile_text)).find(L"synthetic-account-0001") != std::wstring::npos);
  assert(IsWindowEnabled(GetDlgItem(body, account_sync_apply)));
  assert(SetWindowTextW(GetDlgItem(body, account_name), L"  合成昵称  "));
  account_click(account_rename);
  assert(account_user.at("displayName") == "合成昵称");
  assert(caption(GetDlgItem(body, account_name)) == L"合成昵称");
  preview_selection = true; cancel_selection = true;
  auto avatar_count = account_calls.size();
  account_click(account_avatar_upload);
  assert(account_calls.size() == avatar_count);
  cancel_selection = false; fail_avatar_upload = true;
  account_click(account_avatar_upload);
  assert(caption(GetDlgItem(body, account_notice)).find(L"1 MiB") != std::wstring::npos);
  fail_avatar_upload = false;
  account_click(account_avatar_upload);
  assert(account_calls.back().at("operation") == "avatar" && IsWindowEnabled(GetDlgItem(body, account_avatar_remove)));
  preview_selection = false;
  const auto avatar_dc = CreateCompatibleDC(nullptr);
  const auto avatar_screen = GetDC(nullptr);
  const auto avatar_bitmap = CreateCompatibleBitmap(avatar_screen, 16, 16);
  ReleaseDC(nullptr, avatar_screen);
  const auto avatar_previous = SelectObject(avatar_dc, avatar_bitmap);
  DRAWITEMSTRUCT avatar_drawing{};
  avatar_drawing.CtlType = ODT_STATIC; avatar_drawing.CtlID = account_avatar;
  avatar_drawing.hwndItem = GetDlgItem(body, account_avatar); avatar_drawing.hDC = avatar_dc;
  avatar_drawing.rcItem = {0,0,16,16};
  assert(SendMessageW(body, WM_DRAWITEM, account_avatar, reinterpret_cast<LPARAM>(&avatar_drawing)));
  assert(GetPixel(avatar_dc, 8, 8) == RGB(255,0,0));
  malformed_avatar = true;
  account_click(account_avatar_load);
  assert(caption(GetDlgItem(body, account_notice)).find(L"PNG") != std::wstring::npos);
  malformed_avatar = false;
  account_click(account_avatar_load);
  assert(SendMessageW(body, WM_DRAWITEM, account_avatar, reinterpret_cast<LPARAM>(&avatar_drawing)));
  assert(GetPixel(avatar_dc, 8, 8) == RGB(255,0,0));
  SelectObject(avatar_dc, avatar_previous); DeleteObject(avatar_bitmap); DeleteDC(avatar_dc);
  avatar_count = account_calls.size();
  account_click(account_avatar_remove);
  assert(account_calls.size() == avatar_count && account_user.at("avatarUploaded") == true);
  allow_account = true;
  account_click(account_avatar_remove);
  assert(account_user.at("avatarUploaded") == false && !IsWindowEnabled(GetDlgItem(body, account_avatar_remove)));
  allow_account = false;
  assert(store.load().snapshot->revision() == account_revision);
  auto account_count = account_calls.size();
  account_click(account_sync_upload);
  assert(account_calls.size() == account_count && store.load().snapshot->revision() == account_revision);
  allow_account = true;
  account_click(account_sync_upload);
  assert(account_calls.back().at("operation") == "preferences_upload");
  account_click(account_sync_apply);
  assert(store.load().snapshot->preferences().at("traditional_chinese_output") == true);
  assert(SendMessageW(GetDlgItem(body, traditional_output), CB_GETCURSEL, 0, 0) == 1);
  assert(store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  fail_account_apply = true;
  account_click(account_sync_apply);
  assert(store.load().snapshot->preferences().at("traditional_chinese_output") == false);
  assert(SendMessageW(GetDlgItem(body, traditional_output), CB_GETCURSEL, 0, 0) == 0);
  assert(caption(GetDlgItem(body, account_sync_notice)).find(L"无法安全读取") != std::wstring::npos);
  fail_account_apply = false;
  malformed_account_status = true;
  account_click(account_refresh);
  assert(IsWindowEnabled(GetDlgItem(body, account_logout)));
  assert(caption(GetDlgItem(body, account_status)).find(L"合成昵称") != std::wstring::npos);
  assert(caption(GetDlgItem(body, account_notice)).find(L"无法读取") != std::wstring::npos);
  malformed_account_status = false;
  account_click(account_devices_refresh);
  const auto devices_list = GetDlgItem(body, account_devices_list);
  assert(SendMessageW(devices_list, LB_GETCOUNT, 0, 0) == 2);
  const auto select_device = [&](int index) {
    SendMessageW(devices_list, LB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(account_devices_list, LBN_SELCHANGE), reinterpret_cast<LPARAM>(devices_list));
  };
  select_device(1);
  assert(IsWindowEnabled(GetDlgItem(body, account_device_revoke)));
  assert(caption(GetDlgItem(body, account_device_detail)).find(L"2026-02-03") != std::wstring::npos);
  allow_account = false;
  account_count = account_calls.size();
  account_click(account_device_revoke);
  assert(account_calls.size() == account_count && account_devices.size() == 2);
  allow_account = true;
  account_click(account_device_revoke);
  assert(account_devices.size() == 1 && account_signed_in && account_calls.back().at("operation") == "sessions_list");
  select_device(0);
  malformed_devices = true;
  account_click(account_devices_refresh);
  assert(!IsWindowEnabled(GetDlgItem(body, account_device_revoke)));
  assert(caption(GetDlgItem(body, account_devices_notice)).find(L"无法读取") != std::wstring::npos);
  malformed_devices = false;
  account_click(account_devices_refresh);
  account_click(account_qq_refresh);
  const auto qq_list = GetDlgItem(body, account_qq_list);
  assert(SendMessageW(qq_list, LB_GETCOUNT, 0, 0) == 1);
  SetWindowTextW(GetDlgItem(body, account_qq_code), L"1234567");
  assert(!IsWindowEnabled(GetDlgItem(body, account_qq_redeem)));
  SetWindowTextW(GetDlgItem(body, account_qq_code), L"12345678");
  assert(IsWindowEnabled(GetDlgItem(body, account_qq_redeem)));
  account_click(account_qq_redeem);
  assert(qq_links.size() == 2 && caption(GetDlgItem(body, account_qq_code)).empty());
  SendMessageW(qq_list, LB_SETCURSEL, 0, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(account_qq_list, LBN_SELCHANGE), reinterpret_cast<LPARAM>(qq_list));
  allow_account = false;
  account_count = account_calls.size();
  account_click(account_qq_remove);
  assert(account_calls.size() == account_count && qq_links.size() == 2);
  allow_account = true;
  account_click(account_qq_remove);
  assert(qq_links.size() == 1 && account_calls.back().at("operation") == "qq_links");
  account_click(account_data_refresh);
  assert(caption(GetDlgItem(body, account_data_summary)).find(L"语音样本的数量暂时无法读取") != std::wstring::npos);
  cloud_export_selection = true; cancel_selection = true;
  account_count = account_calls.size();
  account_click(account_data_export);
  assert(account_calls.size() == account_count && !std::filesystem::exists(cloud_export_file));
  cancel_selection = false;
  account_click(account_data_export);
  assert(account_calls.back().at("operation") == "data_export");
  assert(caption(GetDlgItem(body, account_data_notice)).find(L"ZIP") != std::wstring::npos);
  cloud_export_selection = false;
  const auto voice_section = GetDlgItem(body, account_data_section_first + 4);
  SendMessageW(voice_section, BM_SETCHECK, BST_CHECKED, 0);
  account_click(account_data_section_first + 4);
  allow_account = false;
  account_count = account_calls.size();
  account_click(account_data_delete);
  assert(account_calls.size() == account_count && SendMessageW(voice_section, BM_GETCHECK, 0, 0) == BST_CHECKED);
  allow_account = true;
  account_click(account_data_delete);
  assert(IsWindowEnabled(GetDlgItem(body, account_voice_retry)));
  assert(SendMessageW(voice_section, BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  assert(caption(GetDlgItem(body, account_data_notice)).find(L"仍有未删除") != std::wstring::npos);
  account_count = account_calls.size();
  account_click(account_voice_retry);
  assert(account_calls.size() == account_count + 1 && account_calls.back().at("operation") == "voice_delete");
  assert(!IsWindowEnabled(GetDlgItem(body, account_voice_retry)));
  account_click(account_delete);
  assert(account_signed_in && IsWindowEnabled(GetDlgItem(body, account_logout)));
  assert(caption(GetDlgItem(body, account_notice)).find(L"重新登录") != std::wstring::npos);
  allow_account = false;
  account_count = account_calls.size();
  account_click(account_logout_all);
  assert(account_calls.size() == account_count && account_signed_in);
  allow_account = true;
  account_click(account_logout_all);
  assert(!account_signed_in && account_calls.back().at("all") == true);
  account_status_failed = true;
  account_click(account_refresh);
  assert(IsWindowEnabled(GetDlgItem(body, account_forget)));
  account_click(account_forget);
  assert(!account_status_failed && !IsWindowEnabled(GetDlgItem(body, account_forget)));
  assert(account_confirmations >= 6);
  // 注销已完成、匿名样本清理失败时，允许在未登录状态单独重试。
  account_signed_in = true;
  account_click(account_refresh);
  account_click(account_devices_refresh);
  select_device(0);
  account_click(account_device_revoke);
  assert(!account_signed_in && !IsWindowEnabled(GetDlgItem(body, account_logout)));
  assert(account_calls.back().at("operation") == "status");
  account_signed_in = true;
  account_click(account_refresh);
  fail_account_delete = false;
  account_click(account_delete);
  assert(!account_signed_in && !IsWindowEnabled(GetDlgItem(body, account_logout)));
  assert(IsWindowEnabled(GetDlgItem(body, account_voice_retry)));
  account_click(account_voice_retry);
  assert(!IsWindowEnabled(GetDlgItem(body, account_voice_retry)));
  fail_account_delete = true;
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(account_google, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(body, account_google)));
  pump_until([&] { return account_google_started.load(); });
  assert(!IsWindowEnabled(language) && IsWindowEnabled(GetDlgItem(body, account_google_cancel)));
  account_click(account_google_cancel);
  assert(google_cancellations >= 2 && !account_signed_in && !account_google_started.load());
  assert(!IsWindowEnabled(GetDlgItem(body, account_google_cancel)));
  assert(caption(GetDlgItem(body, account_notice)).find(L"取消") != std::wstring::npos);
  select_page(0);
  TabCtrl_SetCurSel(tabs, 0);
  SendMessageW(host, WM_NOTIFY, page_tabs, reinterpret_cast<LPARAM>(&notification));
  SendMessageW(language, CB_SETCURSEL, 1, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(language_switch, CBN_SELCHANGE), reinterpret_cast<LPARAM>(language));
  assert(!IsWindowEnabled(language));
  // 关闭会完成刚受理的修改并等待工作线程退出，不丢掉排队写入。
  DestroyWindow(host);
  assert(window.handle() == nullptr);
  saved = store.load();
  assert(saved.snapshot->preferences().at("keybindings").at("switch_language_ctrl").get<bool>());
  assert(!saved.snapshot->preferences().at("keybindings").at("switch_language_shift").get<bool>());
  assert(!saved.snapshot->preferences().at("keybindings").at("switch_language_ctrl_alt_space").get<bool>());
  MSG quit{};
  assert(PeekMessageW(&quit, nullptr, WM_QUIT, WM_QUIT, PM_REMOVE));
  // 新窗口真实接收验证码，检查目标/渠道失效；最终在浏览器等待中关闭。
  SettingsWindow closing_window({}, {}, {}, {}, {}, account_transport);
  assert(closing_window.create(GetModuleHandleW(nullptr)));
  const auto closing_host = closing_window.handle();
  const auto closing_body = GetDlgItem(closing_host, page_body);
  const auto closing_language = GetDlgItem(closing_body, language_switch);
  pump_until([&] { return IsWindowEnabled(closing_language) != FALSE; });
  const auto closing_selector = GetDlgItem(closing_host, page_selector);
  SendMessageW(closing_selector, CB_SETCURSEL, 7, 0);
  SendMessageW(closing_host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(closing_selector));
  pump_until([&] { return IsWindowEnabled(closing_language) != FALSE; });
  const auto closing_click = [&](int id) {
    SendMessageW(closing_body, WM_COMMAND, MAKEWPARAM(id, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(closing_body, id)));
    pump_until([&] { return IsWindowEnabled(closing_language) != FALSE; });
  };
  closing_click(account_email);
  SetWindowTextW(GetDlgItem(closing_body, account_target), L"synthetic@example.invalid");
  closing_click(account_request_code);
  SetWindowTextW(GetDlgItem(closing_body, account_code), L"123456");
  assert(IsWindowEnabled(GetDlgItem(closing_body, account_login)));
  SetWindowTextW(GetDlgItem(closing_body, account_target), L"other@example.invalid");
  assert(!IsWindowEnabled(GetDlgItem(closing_body, account_login)) && caption(GetDlgItem(closing_body, account_code)).empty());
  SetWindowTextW(GetDlgItem(closing_body, account_target), L"synthetic@example.invalid");
  SetWindowTextW(GetDlgItem(closing_body, account_code), L"123456");
  assert(!IsWindowEnabled(GetDlgItem(closing_body, account_login)));
  closing_click(account_phone);
  closing_click(account_email);
  assert(!IsWindowEnabled(GetDlgItem(closing_body, account_login)) && caption(GetDlgItem(closing_body, account_code)).empty());
  account_count = account_calls.size();
  closing_click(account_login);
  assert(account_calls.size() == account_count);
  SendMessageW(closing_body, WM_COMMAND, MAKEWPARAM(account_google, BN_CLICKED), reinterpret_cast<LPARAM>(GetDlgItem(closing_body, account_google)));
  pump_until([&] { return account_google_started.load(); });
  const auto close_started = GetTickCount64();
  DestroyWindow(closing_host);
  assert(GetTickCount64() - close_started < 2000 && !account_google_started.load() && closing_window.handle() == nullptr);
  assert(!account_signed_in && store.load().snapshot->preferences().at("ai_assistant").at("tokens") == tokens);
  std::puts("Windows 7 settings: native controls, CAS, file roundtrip, local state retention and cancellation passed");
}
