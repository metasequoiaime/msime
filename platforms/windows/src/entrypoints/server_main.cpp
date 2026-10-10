#include "../../../shared/contracts/windows_ipc.h"
#include "AuxListener.h"
#include "VoiceTheme.h"
#include "CandidateAppearance.h"
#include "CandidateSkin.h"
#include "CandidateThemeSettings.h"
#include "SkinResourceRevision.h"
#include "CandidateWindow.h"
#include "CandidateWindowStyleSettings.h"
#include "ClipboardHistory.h"
#include "ComponentFailure.h"
#include "DiagnosticListener.h"
#include "DedicatedEnglishMailbox.h"
#include "DiagnosticLog.h"
#include "FloatingToolbarVisibilityPolicy.h"
#include "FirstRun.h"
#include "FloatingToolbarWindow.h"
#include "FocusedSession.h"
#include "InputSchemeTraits.h"
#include "FullscreenForeground.h"
#include "SoundPackRoot.h"
#include "MaintenanceHotkey.h"
#include "ModeAuthority.h"
#include "ModeMailbox.h"
#include "PreviewConfig.h"
#include "PreviewDispatcher.h"
#include "ProductionDispatcher.h"
#include "ProductionPipeNames.h"
#include "ProviderToken.h"
#include "ServerLaunch.h"
#include "ShellLauncher.h"
#include "StateRootLease.h"
#include "SystemAudioMuter.h"
#include "ToolbarModeCommand.h"
#include "TrayMenuDispatch.h"
#include "TrayMenuWindow.h"
#include "VoiceControllerListener.h"
#include "VoiceHotkey.h"
#include "VoiceInputSession.h"
#include "WatchdogPolicy.h"
#include "Telemetry.h"
#include "TelemetryConsent.h"
#include "RevisionFence.h"
#include "WindowsServer.h"
#include "ipc_negotiation.h"
#include <windows.h>
#include <iostream>
#include <map>
#include <memory>
#include <mutex>
#include <unordered_map>
#include <cstdlib>
#include <exception>
#include <thread>
#include <curl/curl.h>
#ifdef _WIN32
#include "StateDirectory.h"
#endif

namespace {
constexpr std::size_t kMaxConfigBytes = 16 * 1024;

// The desktop shell is packaged beside this Server; a development build points
// at another copy with the same variable the Linux host reads.
std::filesystem::path executable_directory() {
  std::vector<wchar_t> path(32768);
  const DWORD length =
      GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
  if (!length || length == path.size())
    return {};
  return std::filesystem::path(std::wstring(path.data(), length)).parent_path();
}
std::wstring voice_audio_path(const msime::windows::PreviewConfig &config,
                              const wchar_t *filename) {
  const auto executable = executable_directory();
  const std::array<std::filesystem::path, 5> candidates = {
      config.state_root / "audios" / filename,
      config.resources / "audios" / filename,
      config.resources / "assets" / "audios" / filename,
      executable / "assets" / "audios" / filename,
      executable.parent_path() / "share" / "msime" / "audios" / filename};
  std::error_code error;
  for (const auto &candidate : candidates)
    if (std::filesystem::is_regular_file(candidate, error))
      return candidate.wstring();
  return {};
}
std::wstring configured_shell_command() {
  std::vector<wchar_t> value(32768);
  const DWORD length = GetEnvironmentVariableW(
      L"MSIME_CLIENT_SETTINGS_COMMAND", value.data(),
      static_cast<DWORD>(value.size()));
  return length && length < value.size() ? std::wstring(value.data(), length)
                                         : std::wstring{};
}
// Resolve the global theme for one surface through the shared layer. Appearance is not worth failing a running Server over, so a refused request or an unreadable answer draws the native tokens.
msime::windows::CandidateThemeResolution
resolve_theme(const nlohmann::json &request) {
  try {
    const auto body = request.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_resolve_theme(
            reinterpret_cast<const uint8_t *>(body.data()), body.size()),
        msime_client_string_free);
    if (!owned)
      return {};
    const auto document = nlohmann::json::parse(owned.get(), nullptr, false);
    if (document.is_discarded() || !document.is_object() ||
        !document.value("ok", false) || !document.contains("value"))
      return {};
    if (auto theme =
            msime::windows::candidate_theme_resolution(document.at("value")))
      return *theme;
  } catch (const std::exception &) {
  }
  return {};
}
// The global theme picker's ids and titles. The catalog is built into the shared layer and cannot change while the Server runs, so it is read once; an unreadable answer leaves the tray's 主题 row without a title.
nlohmann::json theme_catalog() {
  try {
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_theme_catalog(), msime_client_string_free);
    if (owned)
      return nlohmann::json::parse(owned.get(), nullptr, false);
  } catch (const std::exception &) {
  }
  return nlohmann::json();
}
// The artwork and minimum width of the package a resolved theme draws. The theme names the package only when it is drawn in this layout and mode, so there is no gate here.
msime::windows::CandidateSkinAssets
resolve_skin_assets(const std::filesystem::path &root, const std::string &id) {
  if (root.empty() || id.empty())
    return {};
  try {
    const auto directory = root.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_skin_catalog(
            reinterpret_cast<const uint8_t *>(directory.data()),
            directory.size()),
        msime_client_string_free);
    if (owned) {
      const auto catalog = nlohmann::json::parse(owned.get(), nullptr, false);
      if (!catalog.is_discarded() && catalog.value("ok", false))
        return msime::windows::candidate_skin_assets(catalog.at("value"), id,
                                                     root);
    }
  } catch (const std::exception &) {
  }
  return {};
}
std::atomic<bool> stopping{false};
std::atomic<bool> restart_requested{false};
// Set by the maintenance stop shortcut. The Watchdog reads any other exit as a crash and starts the Server again, so a user's stop has to leave with stop_exit_code, as the reference's window hook does.
std::atomic<bool> stop_requested{false};
BOOL WINAPI console_control(DWORD event) {
  if (event != CTRL_C_EVENT && event != CTRL_BREAK_EVENT)
    return FALSE;
  stopping.store(true);
  return TRUE;
}
struct ConsoleControl {
  ConsoleControl() {
    if (!SetConsoleCtrlHandler(console_control, TRUE))
      throw std::runtime_error("Console control unavailable");
  }
  ~ConsoleControl() { SetConsoleCtrlHandler(console_control, FALSE); }
};
// The Server is a windows-subsystem program, so a managed launch (Watchdog or TSF DLL) never opens a console window. A preview or --help run from a terminal attaches to that terminal instead, so its status lines and Ctrl+C still work there. A managed launch never attaches: the TSF DLL starts the Server from inside whatever application has focus, and that may itself be a console program whose window must not receive our output.
void attach_launching_console(const msime::windows::ServerLaunch &launch) {
  if (launch.kind == msime::windows::ServerLaunchKind::Managed ||
      !AttachConsole(ATTACH_PARENT_PROCESS))
    return;
  FILE *stream = nullptr;
  freopen_s(&stream, "CONOUT$", "w", stdout);
  freopen_s(&stream, "CONOUT$", "w", stderr);
  std::cout.clear();
  std::cerr.clear();
}
bool contains(const std::filesystem::path &parent,
              const std::filesystem::path &child) {
  auto p = parent.begin(), c = child.begin();
  for (; p != parent.end(); ++p, ++c)
    if (c == child.end() || CompareStringOrdinal(p->c_str(), -1, c->c_str(), -1,
                                                 TRUE) != CSTR_EQUAL)
      return false;
  return true;
}
std::filesystem::path production_state_directory() {
#ifdef _WIN32
  return msime::windows::resolve_state_directory();
#else
  return {};
#endif
}
// The anonymous account's secret and tokens belong to the Windows user running this Server, so they live in that user's %LOCALAPPDATA%\<本版本的用户目录>\account（full 是 %LOCALAPPDATA%\MSIME\account，版本表 platforms.windows.user_data_directory）。The state root is no place for them: an installed Server's is the installer's DataDir, one directory for the whole machine that every user may modify. 每个版本各自登录，退出一个版本的账号不会删掉另一个版本的令牌。
std::filesystem::path anonymous_account_directory() {
#ifdef _WIN32
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    CoTaskMemFree(local);
    return {};
  }
  const auto directory = std::filesystem::path(local) / MSIME_EDITION_USER_DATA_DIRECTORY / L"account";
  CoTaskMemFree(local);
  return directory;
#else
  return {};
#endif
}
// 使用统计的目录：msime::telemetry::default_directory() 是 %LOCALAPPDATA%\MSIME，本版本换成同级的用户目录（版本表 platforms.windows.user_data_directory），各版本的安装 id 和事件队列互不相干。full 的目录名就是 MSIME，结果与 default_directory() 相同。
std::filesystem::path edition_telemetry_directory() {
  const auto shared = msime::telemetry::default_directory();
  if (shared.empty())
    return {};
  return shared.parent_path() / MSIME_EDITION_USER_DATA_DIRECTORY;
}
std::string read_document(const std::filesystem::path &path) {
  const auto document = msime::windows::read_private_file(path, kMaxConfigBytes);
  if (!document)
    throw std::runtime_error("Configuration unavailable");
  return *document;
}
void write_document_atomic(const std::filesystem::path &path, const std::string &document) {
  if (document.size() > kMaxConfigBytes)
    throw std::runtime_error("Configuration document oversized");
#ifdef _WIN32
  msime::windows::reject_reparse_ancestors(path.parent_path());
#endif
  wchar_t temporary_name[MAX_PATH] = {};
  if (!GetTempFileNameW(path.parent_path().c_str(), L"msi", 0, temporary_name))
    throw std::runtime_error("Configuration temporary file unavailable");
  const std::filesystem::path temporary(temporary_name);
  HANDLE handle = CreateFileW(temporary.c_str(), GENERIC_WRITE, 0, nullptr,
                              OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                              nullptr);
  if (handle == INVALID_HANDLE_VALUE || !msime::windows::handle_is_trusted_file(handle)) {
    if (handle != INVALID_HANDLE_VALUE)
      CloseHandle(handle);
    (void)msime::windows::remove_private_file(temporary);
    throw std::runtime_error("Configuration temporary file unavailable");
  }
  DWORD written = 0;
  const bool complete = document.size() <= MAXDWORD &&
                        WriteFile(handle, document.data(),
                                  static_cast<DWORD>(document.size()), &written,
                                  nullptr) &&
                        written == static_cast<DWORD>(document.size()) &&
                        FlushFileBuffers(handle);
  CloseHandle(handle);
  if (!complete) {
    (void)msime::windows::remove_private_file(temporary);
    throw std::runtime_error("Configuration write failed");
  }
  if (!MoveFileExW(temporary.c_str(), path.c_str(),
                   MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
    (void)msime::windows::remove_private_file(temporary);
    throw std::runtime_error("Configuration replace failed");
  }
}
// The native toolbar and TSF shortcut use the same revisioned store as the
// settings shell. Read and write on their shared single action worker so
// neither the UI thread nor the input queue waits on the preferences lock.
bool persist_traditional_output(const std::filesystem::path &directory,
                                std::atomic<bool> &state,
                                std::optional<bool> desired = std::nullopt) {
  try {
    const auto root = directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> loaded(
        msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!loaded)
      return false;
    const auto response = nlohmann::json::parse(loaded.get());
    if (!response.value("ok", false) || !response.at("value").is_object())
      return false;
    auto snapshot = response.at("value");
    const auto revision = snapshot.at("revision").get<uint64_t>();
    auto &preferences = snapshot.at("preferences");
    const bool enabled = preferences.value("traditional_chinese_output", false);
    const bool next = desired.value_or(!enabled);
    if (next == enabled) {
      state.store(next, std::memory_order_release);
      return true;
    }
    preferences["traditional_chinese_output"] = next;
    const auto serialized = snapshot.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> saved(
        msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size(),
            revision,
            reinterpret_cast<const uint8_t *>(serialized.data()),
            serialized.size()),
        msime_client_string_free);
    if (!saved)
      return false;
    const auto saved_response = nlohmann::json::parse(saved.get());
    if (!saved_response.value("ok", false) ||
        !saved_response.at("value").is_object())
      return false;
    state.store(next, std::memory_order_release);
    return true;
  } catch (...) {
    return false;
  }
}
// 把安装器「联网功能」页的选择写进刚准备好的共享偏好（Linux 的 msime-linux-prepare 做同样的事）。失败时保持共享默认值，也就是关闭，不阻止输入法启动。
void record_installer_cloud_choice(const std::filesystem::path &directory, bool enabled) {
  try {
    const auto root = directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> loaded(
        msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!loaded)
      return;
    const auto response = nlohmann::json::parse(loaded.get());
    if (!response.value("ok", false) || !response.at("value").is_object())
      return;
    auto snapshot = response.at("value");
    const auto revision = snapshot.at("revision").get<uint64_t>();
    snapshot.at("preferences")["cloud_candidates"] = enabled;
    const auto serialized = snapshot.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> saved(
        msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size(),
            revision,
            reinterpret_cast<const uint8_t *>(serialized.data()),
            serialized.size()),
        msime_client_string_free);
  } catch (...) {
  }
}
// Read the stored preferences block, or nothing if it cannot be read. The
// shipped card has usable built-in defaults, so an unreadable store degrades
// to those rather than stopping the IME from starting.
std::optional<nlohmann::json>
load_preference_block(const std::filesystem::path &directory) {
  try {
    const auto root = directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> loaded(
        msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!loaded)
      return std::nullopt;
    const auto response = nlohmann::json::parse(loaded.get());
    if (!response.value("ok", false) || !response.at("value").is_object())
      return std::nullopt;
    const auto &snapshot = response.at("value");
    if (!snapshot.contains("preferences") ||
        !snapshot.at("preferences").is_object())
      return std::nullopt;
    return snapshot.at("preferences");
  } catch (...) {
    return std::nullopt;
  }
}
// "system" follows Windows. Absent or unreadable, keep the shipped dark card
// rather than guessing light and flashing a white panel over a dark desktop.
bool system_prefers_dark() {
  DWORD light = 0;
  DWORD size = sizeof(light);
  if (RegGetValueW(HKEY_CURRENT_USER,
                   L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\"
                   L"Personalize",
                   L"AppsUseLightTheme", RRF_RT_REG_DWORD, nullptr, &light,
                   &size) != ERROR_SUCCESS)
    return true;
  return light == 0;
}
// Flip a stored boolean through the same revisioned store the settings shell
// uses. The tray rows used to change only an in-process flag, so the choice was
// forgotten on every Server restart and disagreed with the settings page.
bool toggle_stored_flag(const std::filesystem::path &directory,
                        const char *section, const char *field, bool fallback,
                        bool &result) {
  try {
    const auto root = directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> loaded(
        msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!loaded)
      return false;
    const auto response = nlohmann::json::parse(loaded.get());
    if (!response.value("ok", false) || !response.at("value").is_object())
      return false;
    auto snapshot = response.at("value");
    const auto revision = snapshot.at("revision").get<uint64_t>();
    auto &preferences = snapshot.at("preferences");
    bool current = fallback;
    if (section) {
      if (!preferences.contains(section) || !preferences.at(section).is_object())
        preferences[section] = nlohmann::json::object();
      current = preferences.at(section).value(field, fallback);
      preferences[section][field] = !current;
    } else {
      current = preferences.value(field, fallback);
      preferences[field] = !current;
    }
    const auto serialized = snapshot.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> saved(
        msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size(),
            revision, reinterpret_cast<const uint8_t *>(serialized.data()),
            serialized.size()),
        msime_client_string_free);
    if (!saved)
      return false;
    const auto saved_response = nlohmann::json::parse(saved.get());
    if (!saved_response.value("ok", false) ||
        !saved_response.at("value").is_object())
      return false;
    result = !current;
    return true;
  } catch (...) {
    return false;
  }
}
// 通过带版本的存储选择输入方案，并像设置页一样维护 last_chinese_scheme：中文方案（包括粤拼、注音和笔画）也是日文、韩文、越南文和藏文切回时回到的方案，选择这些语言之一时记住被替换的中文方案。在它们之间切换保留记住的方案，因为它们都不是存储会接受的中文方案。
bool store_input_scheme(const std::filesystem::path &directory,
                        const std::string &scheme) {
  try {
    const auto root = directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> loaded(
        msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!loaded)
      return false;
    const auto response = nlohmann::json::parse(loaded.get());
    if (!response.value("ok", false) || !response.at("value").is_object())
      return false;
    auto snapshot = response.at("value");
    const auto revision = snapshot.at("revision").get<uint64_t>();
    auto &preferences = snapshot.at("preferences");
    const auto current =
        preferences.contains("scheme") && preferences.at("scheme").is_string()
            ? preferences.at("scheme").get<std::string>()
            : std::string("quanpin");
    if (current == scheme)
      return true;
    // 粤拼、注音和笔画是中文方案，和其他中文方案一样被记住；日文、韩文、越南文和藏文各是独立的语言（client-core 的 ChineseScheme）。
    if (msime::windows::scheme::is_chinese_scheme_name(scheme))
      preferences["last_chinese_scheme"] = scheme;
    else if (msime::windows::scheme::is_chinese_scheme_name(current))
      preferences["last_chinese_scheme"] = current;
    preferences["scheme"] = scheme;
    const auto serialized = snapshot.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> saved(
        msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(root.data()), root.size(),
            revision, reinterpret_cast<const uint8_t *>(serialized.data()),
            serialized.size()),
        msime_client_string_free);
    if (!saved)
      return false;
    const auto saved_response = nlohmann::json::parse(saved.get());
    return saved_response.value("ok", false) &&
           saved_response.at("value").is_object();
  } catch (...) {
    return false;
  }
}
// The Cantonese, Zhuyin and Stroke dictionaries the package installed beside the resources, where host-api looks for them (language_dictionaries_beside). They arrive with a package, so one look at startup holds for the process.
msime::windows::scheme::LanguageDictionaryPresence
installed_language_dictionaries(const std::filesystem::path &resources) {
  const auto directory = resources.parent_path() / L"language-dictionaries";
  std::error_code error;
  return {std::filesystem::is_regular_file(directory / L"msime-cantonese.db", error),
          std::filesystem::is_regular_file(directory / L"msime-zhuyin.db", error),
          std::filesystem::is_regular_file(directory / L"msime-stroke.db", error)};
}
// The scheme the Engine runs for the stored preferences, which is the stored one unless it needs a dictionary that is not installed.
std::string running_scheme(
    const nlohmann::json &preferences,
    msime::windows::scheme::LanguageDictionaryPresence installed) {
  return std::string(msime::windows::scheme::scheme_name(
      msime::windows::scheme::effective_scheme(
          preferences.value("scheme", std::string("quanpin")),
          preferences.value("last_chinese_scheme", std::string("quanpin")),
          installed)));
}
// The stored preferences the tray card shows. The preference monitor publishes them and the UI thread reads them whenever the card is built.
struct TrayMenuPreferences {
  bool translations = true;
  std::string scheme = "quanpin";
  std::string shuangpin_profile = "xiaohe";
  std::string wubi_profile = "wubi86";
  std::string language_hint;
};
TrayMenuPreferences tray_menu_preferences(
    const nlohmann::json &preferences,
    msime::windows::scheme::LanguageDictionaryPresence installed) {
  TrayMenuPreferences result;
  result.translations = preferences.value("candidate_translations", true);
  // The scheme that runs, so a Cantonese, Zhuyin or Stroke choice made before its dictionary was installed checks the scheme the Engine fell back to, as the macOS input menu does.
  result.scheme = running_scheme(preferences, installed);
  result.shuangpin_profile =
      preferences.value("shuangpin_profile", std::string("xiaohe"));
  result.wubi_profile = preferences.value("wubi_profile", std::string("wubi86"));
  // The same defaults the TIP reads (FanyUtils::ReadConfiguredSwitchLanguageHotkeys).
  const auto bindings =
      preferences.value("keybindings", nlohmann::json::object());
  result.language_hint = msime::windows::tray_menu_language_hint(
      bindings.value("switch_language_shift", true),
      bindings.value("switch_language_ctrl", false),
      bindings.value("switch_language_ctrl_alt_space", true));
  return result;
}
// Map the shared preferences onto the settings the TIP keeps in its own
// globals. The TIP consumes every one of these, but nothing ever sent them, so
// they sat at their compiled defaults: turning smart or paired punctuation off
// did nothing, the Microsoft shuangpin ';' key was never enabled, and the
// inline preedit style stayed "raw" whatever the user picked.
// The token for the provider actually in use.
//
// Tokens are kept one per provider so switching provider restores the matching key instead of sending the previous provider's key to the new endpoint.
msime::windows::TsfLocalConfig tsf_local_config(
    const nlohmann::json &preferences,
    msime::windows::scheme::LanguageDictionaryPresence installed) {
  msime::windows::TsfLocalConfig config;
  // The TIP keys the scheme the Engine runs: Zhuyin chosen without msime-zhuyin.db runs a pinyin scheme, and keying it as Zhuyin would swallow the tone digits.
  const auto scheme = running_scheme(preferences, installed);
  const auto navigation =
      preferences.value("navigation", nlohmann::json::object());
  config.paging_comma_period = navigation.value("comma_period", true);
  config.preedit_style = msime::windows::tsf_preedit_style(preferences);
  // PreviewConfig spells the pass-through case "local"; the TIP spells it "raw".
  if (config.preedit_style == "local")
    config.preedit_style = "raw";
  // Windows follows the upstream split smart-punctuation policy: the feature is opt-in, so a profile without the key must not enable punctuation rewriting.
  config.smart_punctuation = preferences.value("smart_punctuation", false);
  config.smart_punctuation_repeat_to_chinese =
      preferences.value("smart_punctuation_repeat", false);
  config.smart_punctuation_space_convert =
      preferences.value("smart_punctuation_space_convert", false);
  config.smart_punctuation_direct_digit =
      preferences.value("smart_punctuation_direct_digit", false);
  config.smart_punctuation_direct_letter =
      preferences.value("smart_punctuation_direct_letter", false);
  config.paired_punctuation = preferences.value("paired_punctuation", true);
  config.microsoft_shuangpin =
      scheme == "shuangpin" &&
      preferences.value("shuangpin_profile", std::string("xiaohe")) == "microsoft";
  config.input_mode = msime::windows::scheme::input_mode(scheme);
  config.tsf_diagnostic_log =
      preferences.value("diagnostic_log", nlohmann::json::object())
          .value("tsf", false);
  const auto lock = preferences.value("punctuation_lock", std::string("follow"));
  config.punctuation_lock = lock == "chinese" ? 1 : lock == "english" ? 2 : 0;
  // The Engine opens V only in the pinyin schemes (`opens_local_modes`), and "/" and "@" in the pinyin schemes and Wubi (`opens_table_modes`); apply_local_mode_switches gates each switch on the common/InputSchemeTraits.h mirror that scripts/test-scheme-traits-parity.py checks. The switches are left out of the stored document while off.
  const auto local_modes =
      preferences.value("local_modes", nlohmann::json::object());
  msime::windows::apply_local_mode_switches(
      config, msime::windows::scheme::scheme_from_name(scheme),
      local_modes.value("expression", false),
      local_modes.value("command", false),
      local_modes.value("mention", false));
  return config;
}

// 会话控制器停下的原因，写进停止那一行；与 ControllerFailure 一一对应。
const char *controller_failure_name(msime::windows::ControllerFailure failure) {
  using msime::windows::ControllerFailure;
  switch (failure) {
  case ControllerFailure::None:
    return "none";
  case ControllerFailure::Service:
    return "service";
  case ControllerFailure::InputQueue:
    return "input queue";
  case ControllerFailure::SessionWorkers:
    return "session workers";
  case ControllerFailure::Control:
    return "control";
  case ControllerFailure::Preferences:
    return "preferences";
  }
  return "unknown";
}

void apply_diagnostic_log(msime::windows::DiagnosticLog &log,
                          const nlohmann::json &preferences) {
  const auto switches =
      preferences.value("diagnostic_log", nlohmann::json::object());
  log.set_enabled(switches.value("server", false), switches.value("tsf", false));
}

std::string production_preview_document(const std::string &runtime_document,
                                        const std::filesystem::path &fallback) {
  const auto host = nlohmann::json::parse(runtime_document);
  if (!host.is_object() || !host.at("resources").is_string())
    throw std::invalid_argument("Invalid production host options");
  const auto state = host.value("preferences_directory", fallback.u8string());
  if (state.empty())
    throw std::invalid_argument("Production state directory unavailable");
  nlohmann::json document{
      {"format_version", 1},
      {"resources", host.at("resources")},
      {"state_root", state},
      {"pipe_namespace", "production"},
      {"preedit_style", "local"},
  };
  const auto preferences = load_preference_block(std::filesystem::u8path(state));
  if (!preferences)
    return document.dump();
  document["preedit_style"] =
      msime::windows::tsf_preedit_style(*preferences);
  document["appearance"] = msime::windows::candidate_appearance(
      std::filesystem::u8path(state), *preferences, system_prefers_dark());
  msime::windows::apply_floating_toolbar(document, *preferences);
  // Last line of defence. The field filtering above is deliberately
  // conservative, but a preference shape nobody anticipated must still not
  // cost the user their IME: if the assembled document would not load, drop
  // the appearance and start with the built-in card.
  try {
    msime::windows::PreviewConfig::parse(document.dump());
  } catch (...) {
    document.erase("appearance");
    document.erase("floating_toolbar_enabled");
    document.erase("floating_toolbar_scale");
    document.erase("floating_toolbar_font_size");
    document.erase("floating_toolbar_items");
    document["preedit_style"] = "local";
  }
  return document.dump();
}
// 本版本的 TIP 有没有活动的输入模式，用一个命名的手动重置事件告诉别的版本的 Server：有信号表示活动。名字后面接版本后缀（full 是 .full）。只有生产 Server 发布它，预览实例不碰。
constexpr wchar_t server_mode_active_event_prefix[] = L"Local\\MetasequoiaImeServer_ModeActive";
// 另一个版本的 TIP 是否有活动的输入模式：看那个版本的 Server 发布的事件。那个版本没在运行时事件不存在，按不活动处理。
bool other_edition_mode_active() {
  for (const wchar_t *suffix : {MSIME_EDITIONS_NAME_SUFFIXES}) {
    if (std::wstring_view(suffix) == MSIME_EDITION_NAME_SUFFIX)
      continue;
    const std::wstring name = std::wstring(server_mode_active_event_prefix) + suffix;
    if (HANDLE event = OpenEventW(SYNCHRONIZE, FALSE, name.c_str())) {
      const bool active = WaitForSingleObject(event, 0) == WAIT_OBJECT_0;
      CloseHandle(event);
      if (active)
        return true;
    }
  }
  return false;
}
class ProductionInstance final {
public:
  ProductionInstance() {
    handle_ = CreateMutexW(nullptr, FALSE,
                           L"Local\\MetasequoiaImeServer_SingleInstance" MSIME_EDITION_NAME_SUFFIX);
    if (!handle_)
      throw std::runtime_error("Server instance guard unavailable");
    already_running_ = GetLastError() == ERROR_ALREADY_EXISTS;
    // 建不出来时别的版本只是看不到本版本的模式，维护快捷键在没有任何版本活动时照样有人处理，所以不算启动失败。
    if (!already_running_)
      mode_active_ = CreateEventW(nullptr, TRUE, FALSE,
                                  (std::wstring(server_mode_active_event_prefix) + MSIME_EDITION_NAME_SUFFIX).c_str());
  }
  ~ProductionInstance() {
    if (mode_active_) {
      ResetEvent(mode_active_);
      CloseHandle(mode_active_);
    }
    if (handle_)
      CloseHandle(handle_);
  }
  bool already_running() const { return already_running_; }
  // 主循环每一轮发布一次本版本的模式是否活动，只在变化时改事件。
  void publish_mode_active(bool active) {
    if (!mode_active_ || active == mode_active_published_)
      return;
    mode_active_published_ = active;
    if (active)
      SetEvent(mode_active_);
    else
      ResetEvent(mode_active_);
  }
private:
  HANDLE handle_ = nullptr;
  HANDLE mode_active_ = nullptr;
  bool mode_active_published_ = false;
  bool already_running_ = false;
};
// A Server that TSF revived after a crash (--production) has no Watchdog above it, so it starts the one packaged beside it, as the reference Server does. The Watchdog adopts this running Server instead of launching a second one, holds its own single-instance mutex, and exits on its own when the TIP profile is not enabled.
void start_watchdog(const std::filesystem::path &directory) {
  const auto watchdog = directory / L"MetasequoiaImeWatchdog.exe";
  if (directory.empty() || GetFileAttributesW(watchdog.c_str()) == INVALID_FILE_ATTRIBUTES)
    return;
  std::wstring command = L"\"" + watchdog.wstring() + L"\"";
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  if (CreateProcessW(watchdog.c_str(), command.data(), nullptr, nullptr, FALSE, 0,
                     nullptr, directory.c_str(), &startup, &process)) {
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
  }
}
// preferences.plugins.effect_intensity, how bright the candidate card's typing flash is. Only the hosts read it, so this is where an out-of-range or mistyped value falls back: clamped to 0-100, and anything not a number is the default 50.
unsigned typing_effect_intensity(const nlohmann::json &preferences) {
  const auto plugins = preferences.value("plugins", nlohmann::json::object());
  if (!plugins.is_object())
    return 50u;
  const auto found = plugins.find("effect_intensity");
  if (found == plugins.end() || !found->is_number())
    return 50u;
  const double value = found->get<double>();
  return static_cast<unsigned>(value <= 0.0 ? 0.0 : (value >= 100.0 ? 100.0 : value));
}
// 诊断日志里前台呈现方式的名字。
const char *foreground_presentation_name(
    msime::windows::ForegroundPresentation value) {
  switch (value) {
  case msime::windows::ForegroundPresentation::Fullscreen:
    return "fullscreen";
  case msime::windows::ForegroundPresentation::ExclusiveFullscreen:
    return "exclusive";
  case msime::windows::ForegroundPresentation::Windowed:
    break;
  }
  return "windowed";
}
// 一次候选窗抑制变化在诊断日志里的写法，只有固定标签和 pid。
std::string candidate_suppression_line(
    const msime::windows::CandidateSuppressionChange &change) {
  std::string line =
      change.reason == msime::windows::CandidateSuppression::ExclusiveFullscreen
          ? "Candidate exclusive-fullscreen suppression "
          : "Candidate reactive latch ";
  line += change.active ? "on" : "off";
  line += " pid=" + std::to_string(change.pid);
  if (change.cause)
    line += std::string(" cause=") + change.cause;
  return line;
}
// 候选窗所在的 z 带，只用来测量 uiAccess 窗口在游戏全屏下排在哪一层，不改变行为。GetWindowBand 没有文档，取不到时返回空。
std::optional<DWORD> candidate_window_band(HWND window) {
  using GetWindowBandFn = BOOL(WINAPI *)(HWND, DWORD *);
  const HMODULE user32 = GetModuleHandleW(L"user32.dll");
  if (!user32)
    return std::nullopt;
  // 经 void* 转换：GetProcAddress 返回通用的 FARPROC，直接转成真实签名会被 -Wcast-function-type 拒绝。
  const auto query = reinterpret_cast<GetWindowBandFn>(
      reinterpret_cast<void *>(GetProcAddress(user32, "GetWindowBand")));
  DWORD band = 0;
  if (!query || !query(window, &band))
    return std::nullopt;
  return band;
}
} // namespace
int wmain(int argc, wchar_t **argv) {
  // Before any thread exists: libcurl's global init is not thread-safe, and the online workers use it.
  curl_global_init(CURL_GLOBAL_DEFAULT);
  // Crash capture only writes this session's crash record to disk, and only once telemetry::begin armed it with the user's consent; the next start reports it.
  std::set_terminate([] {
    msime::telemetry::record_terminate();
    std::abort();
  });
  msime::telemetry::install_crash_handlers();
  using namespace msime::windows;
  const auto launch = parse_server_arguments(argc, argv);
  attach_launching_console(launch);
  if (launch.kind == ServerLaunchKind::Help) {
    std::cout << "MSIME Server: --config <absolute-json-path>\n"
                 "Managed launches use the installed TSF pipe names; preview "
                 "launches use names from the config. Ctrl+C stops.\n"
                 "Unsupported routes (including unobserved Enter) disconnect.\n";
    return 0;
  }
  const bool production = launch.kind == ServerLaunchKind::Managed;
  if (launch.kind == ServerLaunchKind::Invalid)
    return 2;
  try {
    const auto default_state = production_state_directory();
    std::unique_ptr<ProductionInstance> instance;
    if (production) {
      instance = std::make_unique<ProductionInstance>();
      if (instance->already_running())
        return 0;
      if (!launch.supervised)
        start_watchdog(executable_directory());
      const bool prepared_now = prepare_first_run(executable_directory(), default_state,
                       [](const std::string &request) {
        std::unique_ptr<char, decltype(&msime_client_string_free)> response(
            msime_client_prepare_host(
                reinterpret_cast<const uint8_t *>(request.data()), request.size()),
            msime_client_string_free);
        if (!response)
          throw std::runtime_error("Host preparation failed");
        return std::string(response.get());
      });
      // 只在这次刚准备好状态时采用安装器的选择；已有状态属于用户，文件照样删掉。
      if (const auto cloud = take_installer_cloud_choice(default_state); prepared_now && cloud)
        record_installer_cloud_choice(default_state, *cloud);
    }
    const std::filesystem::path config_path =
        production ? default_state / L"runtime-options.json"
                    : std::filesystem::path(launch.config);
    if (!config_path.is_absolute())
      throw std::invalid_argument("Relative config path");
    auto document = read_document(config_path);
    if (production)
      document = production_preview_document(document, default_state);
    auto config = PreviewConfig::parse(document);
    config.resources = std::filesystem::canonical(config.resources);
    reject_reparse_ancestors(config.state_root);
    config.state_root = std::filesystem::weakly_canonical(config.state_root);
    if (contains(config.resources, config.state_root) ||
        contains(config.state_root, config.resources))
      throw std::invalid_argument("Resources and state must be disjoint");
    StateRootLease lease(config.state_root);
    DiagnosticLog diagnostic_log(config.state_root / L"logs" / L"server.log");
    // Operator notices go to the terminal of a preview run and, when the server switch is on, to the diagnostic file - the only place a managed Server's notices can be read.
    const auto notice = [&diagnostic_log](const std::string &line) {
      std::cerr << line << "\n";
      diagnostic_log.server(line);
    };
    ConsoleControl console;
    auto bootstrap_document =
        nlohmann::json{{"resources", config.resources.u8string()},
                       {"state_root", config.state_root.u8string()}};
    // 不是 full 的版本把版本 id 交给宿主库：它按版本选资源锁、收窄方案，并在状态根里记下版本。full 不带这个键，请求与引入版本之前相同。
    if constexpr (!MSIME_EDITION_IS_FULL)
      bootstrap_document["edition"] = MSIME_EDITION_ID;
    const auto bootstrap = bootstrap_document.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> response(
        msime_client_prepare_host(
            reinterpret_cast<const uint8_t *>(bootstrap.data()),
            bootstrap.size()),
        msime_client_string_free);
    if (!response)
      throw std::runtime_error("Host preparation failed");
    const auto prepared = nlohmann::json::parse(response.get());
    if (!prepared.at("ok").get<bool>())
      throw std::runtime_error("Host preparation failed");
    if (stopping.load())
      return 0;
    apply_diagnostic_log(diagnostic_log, prepared.at("value").at("preferences"));
    // Usage reporting, on unless the user turned usage_reporting off: one session per Server process, kept in this Windows user's %LOCALAPPDATA%\MSIME. begin closes the previous session (session_crash only when it left a crash record) and queues today's active; it is file I/O only. Delivery runs on a thread that is never joined, so an unreachable endpoint cannot delay the Server and exiting mid-request only leaves the events queued for the next start.
    const bool usage_reporting = msime::windows::usage_reporting_enabled(prepared.at("value").at("preferences"));
    if (const auto telemetry_directory = edition_telemetry_directory(); !telemetry_directory.empty()) {
      msime::telemetry::begin({"windows", MSIME_WINDOWS_VERSION, telemetry_directory, usage_reporting, {}});
      msime::telemetry::start_flushing();
    }
    // The user's anonymous MSIME account is registered on the first run after install, as on every other platform; once anonymous-session.json exists this is a file read. It runs off the main thread for the same reason as the telemetry event, and a failure (offline, rate limited) is simply retried on the next start.
    if (const auto account = anonymous_account_directory(); production && !account.empty()) {
      std::thread([directory = account.u8string()] {
        std::unique_ptr<char, decltype(&msime_client_string_free)> result(
            msime_client_ensure_anonymous_account(
                reinterpret_cast<const uint8_t *>(directory.data()), directory.size()),
            msime_client_string_free);
      }).detach();
    }
    diagnostic_log.server(std::string(production ? "Production" : "Preview") +
                          " Server starting");
    const auto language_dictionaries =
        installed_language_dictionaries(config.resources);
    auto traditional_output = std::make_shared<std::atomic<bool>>(
        prepared.at("value").at("preferences")
            .value("traditional_chinese_output", false));
    auto tsf_config = std::make_shared<msime::windows::TsfLocalConfig>(
        tsf_local_config(prepared.at("value").at("preferences"),
                         language_dictionaries));
    auto tsf_config_mutex = std::make_shared<std::mutex>();
    auto tray_preferences = std::make_shared<TrayMenuPreferences>(
        tray_menu_preferences(prepared.at("value").at("preferences"),
                              language_dictionaries));
    auto tray_preferences_mutex = std::make_shared<std::mutex>();
    // Set on every publication and on each focus session, so a TIP that
    // registers later is not left holding compiled defaults. A revision is
    // used instead of a bool so an older send cannot clear a newer update.
    auto tsf_config_revision = std::make_shared<msime::windows::RevisionFence>();
    // The toolbar resolves light/dark from its own preference, independently
    // of the candidate card: toolbar_theme is honoured on macOS and in the
    // settings preview but was ignored by the Windows surface, which simply
    // took the card's palette.
    // The tray and candidate context menus. Windows draws its own menus, so
    // this override only ever mattered here, and it was the one surface theme
    // the client did not have.
    // Cross-application CN/EN authority, when the user asked for one state to
    // follow them between applications.
    auto mode_scope_global = std::make_shared<std::atomic<bool>>(
        prepared.at("value").at("preferences")
            .value("ime_mode_scope", std::string("app")) == "global");
    auto menu_theme = std::make_shared<std::atomic<SurfaceThemeMode>>([&] {
      const auto &stored = prepared.at("value").at("preferences");
      const auto theme = stored.value("menu_theme", std::string("follow"));
      const auto global = stored.value("theme", std::string("dark"));
      return surface_theme_mode(theme, global);
    }());
    auto toolbar_theme = std::make_shared<std::atomic<SurfaceThemeMode>>([&] {
      const auto &stored = prepared.at("value").at("preferences");
      const auto theme = stored.value("toolbar_theme", std::string("follow"));
      // "follow" defers to the global theme, and that in turn to Windows.
      const auto global = stored.value("theme", std::string("dark"));
      return surface_theme_mode(theme, global);
    }());
    auto voice_theme = std::make_shared<std::atomic<SurfaceThemeMode>>([&] {
      const auto &stored = prepared.at("value").at("preferences");
      const auto theme = stored.value("voice_theme", std::string("follow"));
      return surface_theme_mode(
          theme, stored.value("theme", std::string("dark")));
    }());
    auto toolbar_enabled = std::make_shared<std::atomic<bool>>(
        prepared.at("value").at("preferences")
            .value("floating_toolbar", nlohmann::json::object())
            .value("enabled", true));
    // 候选窗口跟随光标, likewise published rather than read once.
    auto follow_cursor = std::make_shared<std::atomic<bool>>(
        prepared.at("value").at("preferences")
            .value("candidate_follow_cursor", true));
    // The typing flash's strength, published the same way. The effect itself comes with each key from the input thread.
    auto effect_intensity = std::make_shared<std::atomic<unsigned>>(
        typing_effect_intensity(prepared.at("value").at("preferences")));
    // The TSF Ctrl+Shift+F route is delivered through the same bounded worker
    // as the toolbar button. It must exist before WindowsServer construction:
    // a newly connected client may dispatch its first key immediately.
    CharacterSetClickWorker character_set_clicks(
        [traditional_output, directory = config.state_root](
            const CharacterSetClick &click) {
          (void)persist_traditional_output(directory, *traditional_output,
                                           click.desired);
        });
    auto candidate_fonts = std::make_shared<CandidateFontMailbox>();
    auto candidate_style = std::make_shared<CandidateWindowStyleMailbox>();
    auto toolbar_settings = std::make_shared<FloatingToolbarMailbox>();
    auto candidate_theme = std::make_shared<CandidateThemeMailbox>();
    auto candidate_layout = std::make_shared<std::atomic<unsigned>>(
        CandidateLayoutSettings{config.horizontal_candidates,
                                config.candidate_show_preedit}.encode());
    // Keep the native listener on the same file used by the shared desktop
    // shell; this is the cross-process handoff for the clipboard panel.
    ClipboardHistory clipboard_history(config.state_root / "clipboard_history.json");
    clipboard_history.set_enabled(
        prepared.at("value").at("preferences").value("clipboard_history", false));
    auto voice_config = std::make_shared<VoiceInputConfig>();
    auto voice_config_mutex = std::make_shared<std::mutex>();
    // Local recognition reads the user's dictionary words through these options. Listing the dictionary needs only the paths, which do not change while the Server runs, so the startup document serves every later preference snapshot.
    const auto voice_host_options =
        std::make_shared<const std::string>(prepared.at("value").dump());
    voice_config->host_options = voice_host_options;
    WindowsServerOptions options;
    options.pipes.names = production ? production_pipe_names()
                                     : config.pipe_names();
    // GameHostCandidate 在开发版和预览版里也宣告，否则它们永远走不到游戏会话的兜底定位和独占抑制。
    options.pipes.capabilities =
        production ? (FanyImeProtocol::Capabilities |
                      FanyImeProtocol::CharacterSetShortcut |
                      FanyImeProtocol::GameHostCandidate)
                   : (FanyImeProtocol::RequiredCapabilities |
                      FanyImeProtocol::GameHostCandidate);
    // 管道对端身份对不上时记下 pid 和错误码，用来区分「DLL 加载了但 Server 拒了连接」和「DLL 根本没加载」，比如反作弊剥掉了 Server 打开游戏进程所需的权限。
    // 在握手线程上运行，而 notice 会写盘：反作弊持续拒绝时 TSF 每次重连都会走到这里，所以同一个 pid 一分钟只记一条，免得拖慢排队中的握手。
    auto identity_rejected_logged =
        std::make_shared<std::pair<std::mutex, std::unordered_map<DWORD, uint64_t>>>();
    options.pipes.identity_rejected = [&notice, identity_rejected_logged](DWORD pid, DWORD error) {
      const uint64_t now = GetTickCount64();
      {
        std::lock_guard<std::mutex> lock(identity_rejected_logged->first);
        auto &logged = identity_rejected_logged->second;
        const auto found = logged.find(pid);
        if (found != logged.end() && now - found->second < 60000)
          return;
        if (logged.size() >= 64)
          logged.clear();
        logged[pid] = now;
      }
      notice("Pipe identity rejected: pid=" + std::to_string(pid) +
             " error=" + std::to_string(error));
    };
    options.preferences_directory = config.state_root.u8string();
    options.preferences_published =
        [&, voice_config, voice_config_mutex, voice_host_options, traditional_output,
         toolbar_enabled, follow_cursor, effect_intensity, voice_theme, candidate_fonts, candidate_style,
         toolbar_theme, menu_theme, mode_scope_global, tsf_config, candidate_layout,
         tsf_config_mutex, tray_preferences, tray_preferences_mutex,
         tsf_config_revision, candidate_theme, toolbar_settings,
         language_dictionaries](const PreferenceSnapshot &snapshot) {
          const auto preferences =
              nlohmann::json::parse(snapshot.serialized()).at("preferences");
          candidate_theme->publish(preferences);
          apply_diagnostic_log(diagnostic_log, preferences);
          // Turning reporting off clears what is queued and disarms crash capture at once; turning it on starts a session as a Server start would.
          msime::telemetry::set_enabled(msime::windows::usage_reporting_enabled(preferences));
          if (auto settings = floating_toolbar_settings(preferences))
            toolbar_settings->publish(snapshot.revision(), *settings);
          if (auto fonts = candidate_font_settings(preferences))
            candidate_fonts->publish(snapshot.revision(), std::move(*fonts));
          if (auto style = candidate_window_style(preferences))
            candidate_style->publish(snapshot.revision(), *style);
          if (auto layout = candidate_layout_settings(preferences))
            candidate_layout->store(layout->encode(), std::memory_order_release);
          traditional_output->store(
              preferences.value("traditional_chinese_output", false),
              std::memory_order_release);
          clipboard_history.set_enabled(
              preferences.value("clipboard_history", false));
          mode_scope_global->store(
              preferences.value("ime_mode_scope", std::string("app")) ==
                  "global",
              std::memory_order_release);
          {
            const auto theme =
                preferences.value("menu_theme", std::string("follow"));
            const auto global = preferences.value("theme", std::string("dark"));
            menu_theme->store(surface_theme_mode(theme, global),
                              std::memory_order_release);
          }
          {
            const auto theme =
                preferences.value("toolbar_theme", std::string("follow"));
            const auto global = preferences.value("theme", std::string("dark"));
            toolbar_theme->store(surface_theme_mode(theme, global),
                                 std::memory_order_release);
          }
          // 语音面板主题: follow / dark / light. The overlay has had the setter
          // all along, but nothing read the preference, so it was always dark.
          // The overlay is built later, so publish through a flag the loop
          // applies.
          const auto voice_surface_theme =
              preferences.value("voice_theme", std::string("follow"));
          // Publish the TSF-local settings; the loop pushes them to the
          // focused TIP, since the server is constructed after this handler.
          {
            std::lock_guard<std::mutex> lock(*tsf_config_mutex);
            const bool dedicated_english = tsf_config->dedicated_english;
            *tsf_config = tsf_local_config(preferences, language_dictionaries);
            tsf_config->dedicated_english = dedicated_english;
            tsf_config_revision->mark_changed();
          }
          {
            auto menu = tray_menu_preferences(preferences, language_dictionaries);
            std::lock_guard<std::mutex> lock(*tray_preferences_mutex);
            *tray_preferences = std::move(menu);
          }
          voice_theme->store(
              surface_theme_mode(
                  voice_surface_theme,
                  preferences.value("theme", std::string("dark"))),
              std::memory_order_release);
          // The settings page owns this too; without reconciling it here the
          // toolbar only followed the preference across a restart.
          const auto toolbar_preferences =
              preferences.value("floating_toolbar", nlohmann::json::object());
          toolbar_enabled->store(toolbar_preferences.value("enabled", true),
                                 std::memory_order_release);
          follow_cursor->store(
              preferences.value("candidate_follow_cursor", true),
              std::memory_order_release);
          effect_intensity->store(typing_effect_intensity(preferences),
                                  std::memory_order_release);
          const auto input = preferences.value("voice_input", nlohmann::json::object());
          VoiceInputConfig next;
          next.capture = voice_capture_selection(input);
          next.enabled = input.value("enabled", true);
          next.start_sound = input.value("start_sound", true);
          next.end_sound = input.value("end_sound", true);
          next.sound_enabled = input.value("sound_enabled", true);
          next.mute_system_audio = input.value("mute_system_audio", false);
          next.hotkey_ralt = input.value("hotkey_ralt", true);
          next.hotkey_ctrl_f9 = input.value("hotkey_ctrl_f9", true);
          next.hotkey_ctrl_win = input.value("hotkey_ctrl_win", false);
          next.hotkey_rctrl_ralt = input.value("hotkey_rctrl_ralt", false);
          next.hotkey_hold_space_lock = input.value("hotkey_hold_space_lock", true);
          next.stream_inline_preedit = input.value("stream_inline_preedit", true);
          next.commit_mode = input.value("commit_mode", std::string{"tsf"});
          next.endpoint = input.value("asr_endpoint", std::string{});
          next.model = input.value("asr_model", std::string{});
          // Read before the tokens: the slot lookup is keyed on them.
          next.asr_provider = input.value("asr_provider", std::string{"doubao"});
          next.asr_model_path = input.value("asr_model_path", std::string{});
          next.host_options = voice_host_options;
          next.polish_provider = input.value("polish_provider", std::string{});
          next.token = provider_token(input, "asr_tokens", "asr_token",
                                      next.asr_provider);
          next.app_key = input.value("asr_app_key", std::string{});
          next.doubao_auth_mode = input.value("doubao_auth_mode", std::string{});
          next.resource_id = input.value("asr_resource_id", std::string{});
          next.enable_itn = input.value("doubao_enable_itn", true);
          next.enable_punc = input.value("doubao_enable_punc", true);
          next.enable_ddc = input.value("doubao_enable_ddc", false);
          next.boosting_table_id = input.value("doubao_boosting_table_id", std::string{});
          next.language = input.value("language", std::string{"zh-cn"});
          next.polish_enabled = input.value("polish_enabled", false);
          next.polish_text = input.value("polish_text", false);
          next.polish_token = provider_token(input, "polish_tokens",
                                             "polish_token",
                                             next.polish_provider);
          next.polish_endpoint = input.value("polish_endpoint", std::string{});
          next.polish_model = input.value("polish_model", std::string{});
          next.polish_prompt_id = input.value("polish_prompt_id", std::string{"cleanup"});
          next.polish_prompt_custom_1 = input.value("polish_prompt_custom_1", std::string{});
          next.polish_prompt_custom_2 = input.value("polish_prompt_custom_2", std::string{});
          next.polish_prompt_custom_3 = input.value("polish_prompt_custom_3", std::string{});
          std::lock_guard lock(*voice_config_mutex);
          *voice_config = std::move(next);
        };
    auto session_options = prepared.at("value");
    name_builtin_sound_packs(session_options, config.state_root);
    WindowsServer server(
        options, session_options.dump(),
        production
              ? production_key_handler([&character_set_clicks](bool desired) {
                return character_set_clicks.submit(CharacterSetClick{desired});
              })
            : preview_key_handler(config),
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    // Its palette is applied once the theme is resolved, alongside the toolbar's.
    WaveOverlay voice_overlay;
    VoiceInputSession *voice_session = nullptr;
    if (!voice_overlay.init(
            GetModuleHandleW(nullptr), [&voice_session](WaveOverlay::Action action) {
              if (!voice_session)
                return;
              if (action == WaveOverlay::Action::Cancel)
                voice_session->cancel();
              else if (voice_session->recording())
                voice_session->stop();
            }))
      throw std::runtime_error("Voice overlay unavailable");
    auto voice = std::make_unique<VoiceInputSession>(
        voice_overlay,
        [&] {
          const auto view = server.mode_view();
          return view ? std::optional<FocusLease>(view->lease) : std::nullopt;
        },
        [&](const FocusLease &lease, uint32_t message, std::wstring_view text,
            wchar_t generation) {
          return server.send_voice_composition(lease, message, text, generation);
        },
        [voice_config, voice_config_mutex] {
          std::lock_guard lock(*voice_config_mutex);
          return *voice_config;
        });
    voice_session = voice.get();
    VoiceControllerMailbox voice_controller_mailbox;
    VoiceControllerDispatch voice_controller_dispatch(
        {[&]() -> std::optional<FocusLease> {
           const auto view = server.mode_view();
           return view ? std::optional<FocusLease>(view->lease) : std::nullopt;
         },
         [&](const FocusLease &lease) { return server.focus_current(lease); },
         [&](std::string_view language) {
           return voice->start_review(language);
         },
         [&](const auto &result) { return voice->stop_review(result); },
         [&](const auto &result) { return voice->cancel_review(result); }});
    DWORD voice_controller_error = ERROR_SUCCESS;
    auto voice_controller = VoiceControllerListener::create(
        voice_controller_mailbox, voice_controller_error);
    if (!voice_controller)
      notice("Voice controller unavailable; native input remains enabled");
    configure_audio_mute_state_path(
        (config.state_root / "voice_system_audio_mute_state.txt").wstring());
    (void)voice->init_cues(voice_audio_path(config, L"start.mp3"),
                           voice_audio_path(config, L"end.mp3"));
    VoiceHotkeyController voice_hotkeys(
        *voice,
        [voice_config, voice_config_mutex] {
          std::lock_guard lock(*voice_config_mutex);
          return *voice_config;
        },
        [&] { return server.mode_view().has_value(); });
    ClipboardMonitor clipboard_monitor(
        clipboard_history, [&](std::string text) {
          const auto request = nlohmann::json{
              {"directory", config.state_root.u8string()},
              {"text", normalize_clipboard_text(std::move(text))}}.dump();
          // The shared writer preserves pinned entries, timestamps and the
          // preference/history locking used by the Tauri panel.
          std::unique_ptr<char, decltype(&msime_client_string_free)> reply(
              msime_client_capture_clipboard_history(
                  reinterpret_cast<const uint8_t *>(request.data()), request.size()),
              msime_client_string_free);
        });
    // Clipboard history is an optional convenience, so a monitor that cannot
    // start leaves it inert rather than taking the IME down with it. Failing
    // here used to cost the user all text input because a message-only window
    // or a class registration failed.
    if (!clipboard_monitor.start())
      notice("Clipboard history unavailable; continuing without it");
    CandidateClickWorker clicks([&](const CandidateClick &click) {
      if (click.action == CandidateAction::Select) {
        if (server.request_selection(click.lease, click.session,
                                     click.generation, click.index) ==
            SelectionRequestResult::Failed)
          throw std::runtime_error("Candidate selection failed");
        return;
      }
      if (server.request_candidate_action(
              click.lease, click.session, click.generation, click.index,
              click.action, click.position) ==
          CandidateActionRequestResult::Failed)
          throw std::runtime_error("Candidate action failed");
    });
    CandidatePageWorker pages([&](const CandidatePage &page) {
      if (server.request_page(page) == CandidatePageRequestResult::Failed)
        throw std::runtime_error("Candidate paging failed");
    });
    ModeClickWorker mode_clicks([&](const ModeClick &click) {
      if (click.mode == WorkerMode::English && server.dedicated_english_state(click.lease).value_or(false)) {
        if (!server.exit_dedicated_english(click.lease))
          throw std::runtime_error("Dedicated English exit failed");
        return;
      }
      if (server.request_mode(click.lease, click.mode) == ModeRequestResult::WriteFailed)
        throw std::runtime_error("Mode request failed");
    });
    DedicatedEnglishMailbox english_state;
    SingleClickWorker<FocusLease> english_reads([&](const FocusLease &lease) {
      if (auto value = server.dedicated_english_state(lease))
        english_state.publish(lease, *value);
    });
    uint64_t english_read_at = 0;
    struct ClickShutdown {
      WindowsServer &server;
      CandidateClickWorker &clicks;
      CandidatePageWorker &pages;
      ModeClickWorker &modes;
      CharacterSetClickWorker &character_sets;
      SingleClickWorker<FocusLease> &english;
      ~ClickShutdown() {
        clicks.request_stop();
        pages.request_stop();
        modes.request_stop();
        character_sets.request_stop();
        english.request_stop();
        server.request_stop();
        clicks.stop();
        pages.stop();
        modes.stop();
        character_sets.stop();
        english.stop();
      }
    } click_shutdown{server, clicks, pages, mode_clicks, character_set_clicks, english_reads};
    CandidateWindow candidates(
        [&] {
          auto view = server.candidate_view();
          if (view)
            *view = with_wubi_code_hints(
                std::move(*view),
                CandidateLayoutSettings::decode(candidate_layout->load(std::memory_order_acquire))
                    .wubi_code_hint);
          return view;
        },
        [&](const CandidateClick &click) { (void)clicks.submit(click); },
        static_cast<unsigned>(config.candidate_font_size),
        static_cast<unsigned>(config.candidate_preedit_font_size),
        config.candidate_font, config.candidate_fallback_fonts, config.dark_theme,
        config.horizontal_candidates, config.candidate_show_preedit,
        [&](const CandidatePage &page) { (void)pages.submit(page); },
        [&](const CandidatePresentation &value) {
          server.candidate_rendered(value.lease, value.render_serial);
        },
        config.navigation.mouse_wheel);
    // The global theme colours the card, the toolbar and the menus. Each surface resolves it in its own mode, and the answers are kept until the theme, the layout or the package on disk changes, so the shared layer's disk read never runs inside a draw.
    auto current_candidate_theme = candidate_theme_values(
        prepared.at("value").at("preferences"));
    std::map<std::string, CandidateThemeResolution> resolved_themes;
    // Package assets by id, read from the catalog once per package and forgotten with the resolved themes.
    std::map<std::string, msime::windows::CandidateSkinAssets> skin_assets;
    auto package_assets = [&](const std::string &id)
        -> const msime::windows::CandidateSkinAssets & {
      auto found = skin_assets.find(id);
      if (found == skin_assets.end())
        found = skin_assets.emplace(id, resolve_skin_assets(config.skin_directory, id))
                    .first;
      return found->second;
    };
    auto resolved_theme = [&](bool dark,
                              bool horizontal) -> const CandidateThemeResolution & {
      const auto request = candidate_theme_request(
          current_candidate_theme, dark, horizontal, config.skin_directory);
      auto key = request.dump();
      auto found = resolved_themes.find(key);
      if (found == resolved_themes.end())
        found = resolved_themes.emplace(std::move(key), resolve_theme(request))
                    .first;
      return found->second;
    };
    bool candidate_theme_dirty = true;
    bool candidate_dark_applied = config.dark_theme;
    bool candidate_horizontal_applied = config.horizontal_candidates;
    std::string candidate_skin_applied;
    // Bumped whenever the card's theme is re-resolved, so the toolbar and the menu follow it.
    uint64_t candidate_theme_generation = 0;
    uint64_t candidate_theme_check_at = 0;
    bool system_dark = system_prefers_dark();
    SkinResourceRevision candidate_skin_revision;
    {
      const auto &theme =
          resolved_theme(config.dark_theme, config.horizontal_candidates);
      auto palette = candidate_theme_palette(theme, config.dark_theme);
      if (config.candidate_selected_bar)
        palette.show_selected_bar = *config.candidate_selected_bar;
      candidates.set_palette(palette);
      // An external package may ask for a wider card than the font implies; the artwork is drawn against that width.
      const auto &assets = package_assets(theme.candidate_skin);
      candidates.set_skin_min_width(assets.min_width);
      candidates.set_skin_decoration(assets.decoration);
      candidates.set_skin_background(assets.background);
      candidates.set_skin_corner_radius(assets.corner_radius);
      candidate_skin_applied = theme.candidate_skin;
    }
    candidates.set_follow_cursor(follow_cursor->load(std::memory_order_acquire));
    candidates.set_effect_intensity(effect_intensity->load(std::memory_order_acquire));
    // The toolbar and the menus draw the card's theme in their own light/dark mode, with the card's layout deciding whether a package is drawn.
    auto surface_palette = [&](bool dark) {
      return candidate_theme_palette(
          resolved_theme(dark, candidate_horizontal_applied), dark);
    };
    // The toolbar's palette in `dark`: the theme's, with the toolbar colours and radius of the package that theme draws in that mode.
    auto toolbar_surface_palette = [&](bool dark) {
      const auto &theme = resolved_theme(dark, candidate_horizontal_applied);
      return apply_toolbar_skin(toolbar_palette(candidate_theme_palette(theme, dark)),
                                package_assets(theme.candidate_skin).toolbar, dark);
    };
    bool toolbar_visible = toolbar_enabled->load(std::memory_order_acquire);
    FloatingToolbarWindow toolbar(
        [&] { return server.mode_view(); },
        [&](const ModeClick &click) { (void)mode_clicks.submit(click); });
    // The toolbar follows the global theme in its own light/dark mode.
    bool toolbar_dark_applied = !surface_theme_is_light(
        toolbar_theme->load(std::memory_order_acquire), system_dark);
    uint64_t toolbar_theme_applied = candidate_theme_generation;
    toolbar.set_palette(toolbar_surface_palette(toolbar_dark_applied));
    // The voice overlay draws the theme too, in its own light/dark mode, as the tray menu does.
    bool voice_dark_applied = !surface_theme_is_light(
        voice_theme->load(std::memory_order_acquire), system_dark);
    uint64_t voice_theme_applied = candidate_theme_generation;
    voice_overlay.set_palette(surface_palette(voice_dark_applied));
    toolbar.set_scale(config.floating_toolbar_scale);
    toolbar.set_font_size(config.floating_toolbar_font_size);
    toolbar.set_items(config.floating_toolbar_items);
    if (config.floating_toolbar_x && config.floating_toolbar_y)
      toolbar.set_position(POINT{*config.floating_toolbar_x, *config.floating_toolbar_y});
    if (!production) {
      toolbar.set_position_changed([&document, &config_path](POINT position) {
        try {
          auto updated = nlohmann::json::parse(document);
          updated["floating_toolbar_x"] = position.x;
          updated["floating_toolbar_y"] = position.y;
          const auto serialized = updated.dump();
          write_document_atomic(config_path, serialized);
          document = serialized;
        } catch (...) {
          // A transient write failure must not tear down the input server.
        }
      });
    }
    toolbar.set_active_reader([&] { return server.mode_active(); });
    toolbar.set_character_set_reader([traditional_output] {
      return std::optional<bool>(
          traditional_output->load(std::memory_order_acquire));
    });
    // The Server owns the floating toolbar. Every other row opens a surface in
    // the shared desktop shell, which is a separate process: with no shell
    // installed beside this Server those rows stay visible and disabled rather
    // than accepting a click that does nothing.
    const auto shell_directory = executable_directory();
    const auto configured_shell = configured_shell_command();
    const auto settings_request = shell_surface_request(TrayMenuCommand::OpenSettings);
    const auto preview_request = shell_surface_request(TrayMenuCommand::OpenEmojiPanel);
    const auto settings_shell = settings_request
                                    ? shell_executable(shell_directory, configured_shell,
                                                       *settings_request)
                                    : std::nullopt;
    const auto preview_shell = preview_request
                                   ? shell_executable(shell_directory, {}, *preview_request)
                                   : std::nullopt;
    const ShellLaunchContext shell_context{
        config.state_root, config.state_root / L"runtime-options.json"};
    const auto launch_shell = [&](const ShellSurfaceRequest &request) {
      const auto executable = shell_executable(shell_directory, configured_shell, request);
      return executable && launch_shell_surface(*executable, request, shell_context);
    };
    toolbar.set_character_set_action([&] {
      (void)character_set_clicks.submit(CharacterSetClick{});
    });
    toolbar.set_shell_available(settings_shell.has_value() || preview_shell.has_value());
    toolbar.set_settings_action([&] {
      const auto request = shell_surface_request(TrayMenuCommand::OpenSettings);
      if (request) (void)launch_shell(*request);
    });
    toolbar.set_emoji_action([&] {
      const auto request = shell_surface_request(TrayMenuCommand::OpenEmojiPanel);
      if (request) (void)launch_shell(*request);
    });
    toolbar.set_keyboard_action([&] {
      const auto request = shell_surface_request(TrayMenuCommand::OpenKeyboardPanel);
      if (request) (void)launch_shell(*request);
    });
    toolbar.set_hide_action([&] {
      toolbar_visible = false;
      toolbar.hide();
    });
    TrayMenuCapabilities menu_capabilities;
    menu_capabilities.emoji_panel = preview_shell.has_value();
    // 手写模型只认汉字，不提供手写的版本（日文、越南文和藏文版）托盘菜单里没有手写，安装包里也没有手写模型。
    menu_capabilities.handwriting_panel = preview_shell.has_value() && MSIME_EDITION_HANDWRITING != 0;
    menu_capabilities.keyboard_panel = preview_shell.has_value();
    menu_capabilities.voice_input = true;
    menu_capabilities.settings = settings_shell.has_value();
    menu_capabilities.cantonese = language_dictionaries.cantonese;
    menu_capabilities.zhuyin = language_dictionaries.zhuyin;
    menu_capabilities.stroke = language_dictionaries.stroke;
    const auto themes = theme_catalog();
    TrayMenuWindow tray(
        menu_capabilities,
        [&](TrayMenuCommand command) {
          // The mode rows send what the toolbar button for the same mode sends, to the focused TIP, through the same worker. The card never takes focus, so the session the rows were drawn for is still the focused one.
          if (tray_menu_mode_row(command)) {
            const auto view = server.mode_view();
            if (!view)
              return false;
            const auto request = tray_menu_mode_command(
                command, view->chinese, view->fullwidth,
                view->chinese_punctuation,
                english_state.snapshot(view->lease).value_or(false));
            if (!request.known)
              return false;
            if (!request.mode)
              return true;
            return mode_clicks.submit(ModeClick{view->lease, *request.mode});
          }
          // Stored preferences go through the revisioned store the settings app uses, and are published here at once, as the toolbar row does, so a card reopened before the file monitor reports the change does not show the old value.
          if (command == TrayMenuCommand::ToggleTranslations) {
            bool current = true;
            {
              std::lock_guard<std::mutex> lock(*tray_preferences_mutex);
              current = tray_preferences->translations;
            }
            bool next = !current;
            if (!toggle_stored_flag(config.state_root, nullptr,
                                    "candidate_translations", current, next))
              return false;
            std::lock_guard<std::mutex> lock(*tray_preferences_mutex);
            tray_preferences->translations = next;
            return true;
          }
          if (const char *scheme = tray_menu_scheme(command)) {
            if (!store_input_scheme(config.state_root, scheme))
              return false;
            std::lock_guard<std::mutex> lock(*tray_preferences_mutex);
            tray_preferences->scheme = scheme;
            return true;
          }
          if (command == TrayMenuCommand::ToggleFloatingToolbar) {
            // Write it back, so the choice survives a restart and the settings
            // page and this row cannot disagree. A store that refuses the write
            // leaves the row unhandled rather than showing a state that was
            // never saved.
            bool next = !toolbar_visible;
            if (!toggle_stored_flag(config.state_root, "floating_toolbar",
                                    "enabled", toolbar_visible, next))
              return false;
            // Publish immediately as well as writing the store: the file
            // monitor reports the change a moment later, and the refresh loop
            // reads this flag, so without it the toolbar would flip back until
            // the monitor caught up.
            toolbar_enabled->store(next, std::memory_order_release);
            toolbar_visible = next;
            if (!toolbar_visible)
              toolbar.hide();
            return true;
          }
          if (command == TrayMenuCommand::ToggleVoiceInput)
            return voice->toggle();
          const auto request = shell_surface_request(command);
          // Report only what was observed: a row that could not start the
          // shell stays unhandled, so the menu does not close on a promise.
          return request && launch_shell(*request);
        },
        [&] {
          TrayMenuState state;
          state.floating_toolbar = toolbar_visible;
          if (const auto view = server.mode_view()) {
            state.chinese = view->chinese;
            state.fullwidth = view->fullwidth;
            state.chinese_punctuation = view->chinese_punctuation;
            state.dedicated_english =
                english_state.snapshot(view->lease).value_or(false);
          }
          {
            std::lock_guard<std::mutex> lock(*tray_preferences_mutex);
            state.translations = tray_preferences->translations;
            state.scheme = tray_preferences->scheme;
            state.shuangpin_profile = tray_preferences->shuangpin_profile;
            state.wubi_profile = tray_preferences->wubi_profile;
            state.language_hint = tray_preferences->language_hint;
          }
          // candidate_theme_values keeps global_theme only when it is a string.
          state.theme_title = theme_catalog_title(
              themes,
              current_candidate_theme.value("global_theme", std::string("system")));
          return state;
        });
    // The menu follows the global theme in its own light/dark mode, like the toolbar.
    bool menu_dark_applied = !surface_theme_is_light(
        menu_theme->load(std::memory_order_acquire), system_dark);
    uint64_t menu_theme_applied = candidate_theme_generation;
    tray.set_palette(tray_menu_palette(surface_palette(menu_dark_applied)));
    // The Server is the Caps Lock authority: the TIP only sampled GetKeyState
    // at activation, so pressing Caps mid-session left its indicator stale.
    ModeAuthorityState mode_authority;
    // Seeded from the configured default, and marked as seeded so the first
    // client does not silently become the authority. 默认输入状态 is documented
    // as the state a new focus session starts in, so pushing it to the first
    // client is the behaviour that option promises.
    mode_authority.chinese =
        prepared.at("value").at("preferences")
            .value("default_ime_mode", std::string("chinese")) != "english";
    mode_authority.seeded = true;
    std::atomic<bool> caps_lock{(GetKeyState(VK_CAPITAL) & 1) != 0};
    msime::windows::RevisionFence caps_lock_revision;
    // Starts true: the Server is launched by the TIP, so the IME is active by
    // the time this runs, and waiting for the first edge would hide the toolbar
    // until the user switched focus once.
    std::atomic<bool> ime_active{true};
    // The language bar sends a right click over the Aux pipe; without a
    // listener the tray menu - and with it every shared-shell entry - is
    // unreachable. A failure here costs the menu, never the IME.
    TrayMenuMailbox tray_mailbox;
    // Both statistics sinks below answer from this, never from the store: a key batch split over several Aux messages would otherwise have each message wait on the previous one's detached write for the store lock, past the DLL's 150 ms answer window. Declared before the listener so it outlives every sink call.
    const TypingStatisticsSwitch statistics_switch(
        [directory = config.state_root.u8string()] {
          return msime_client_typing_statistics_enabled(
              reinterpret_cast<const uint8_t *>(directory.data()),
              directory.size());
        },
        std::chrono::seconds(5));
    DWORD aux_error = ERROR_SUCCESS;
    const std::wstring aux_name =
        production ? FANY_IME_AUX_NAMED_PIPE : config.aux_pipe_name();
    auto aux = AuxListener::create(
        aux_name,
        [&tray_mailbox](const TrayMenuAnchor &anchor) {
          tray_mailbox.publish(anchor);
        },
        aux_error,
        [](const std::wstring &message) {
          if (message == L"RestartServer") {
            restart_requested.store(true);
            stopping.store(true);
          }
        },
        [&ime_active](AuxActivation activation) {
          ime_active.store(activation == AuxActivation::Activated,
                           std::memory_order_release);
        },
        // The DLL falls back to this when its Main-pipe deactivate write
        // fails, then polls for a literal "OK" for up to 150 ms - blocking the
        // sending TSF thread for that whole window when nobody answers. The
        // answer is only sent once the named client really is not focused
        // under that token, so an "OK" never reports a teardown that did not
        // happen.
        [&server](const AuxTerminalDeactivation &terminal) {
          return server.deactivate_terminal(terminal.client_id,
                                            terminal.focus_token);
        },
        // Dictionary maintenance runs in the settings process and needs the
        // exclusive lock every Engine session holds a share of. Releasing it
        // means dropping the sessions: the Engine has those files open, and
        // leaving it alive while they are swapped underneath would have it
        // reading a tree that no longer exists. The clients stay connected
        // and get a session back on resume.
        [&server](AuxDictionaryMaintenance request) {
          return request == AuxDictionaryMaintenance::Quiesce
                     ? server.quiesce_dictionaries()
                     : server.resume_dictionaries();
        },
        // Keys the TIP passed straight to the application - English mode, digits and punctuation the Engine declined - never reach a session, so the commit path cannot count them. The DLL batches them here instead. Refusing while statistics are off makes the DLL back off rather than keep sending characters nobody records.
        [statistics_directory = config.state_root.u8string(),
         &statistics_switch](const AuxTypingStatistics &batch) {
          if (!statistics_switch.enabled())
            return false;
          const int wide_size = static_cast<int>(batch.characters.size());
          const int size = WideCharToMultiByte(
              CP_UTF8, WC_ERR_INVALID_CHARS, batch.characters.data(), wide_size,
              nullptr, 0, nullptr, nullptr);
          if (size <= 0)
            return false;
          std::string text(static_cast<size_t>(size), '\0');
          if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS,
                                  batch.characters.data(), wide_size,
                                  text.data(), size, nullptr, nullptr) != size)
            return false;
          // A milestone's jingle stays quiet behind a full-screen application, as every other effect sound does.
          record_typing_statistics_async(
              statistics_directory, text,
              batch.english ? TypingSource::English : TypingSource::Unknown,
              foreground_is_fullscreen(GetForegroundWindow()));
          return true;
        },
        // Per-key press counts for the key heatmap. The DLL counts them because only it sees the scan code, and it buffers nothing until a probe is answered here, so refusing while statistics are off is what keeps every host process from counting at all. The ids are ASCII by the time the parser accepted them; which ones are canonical is the shared store's rule.
        [statistics_directory = config.state_root.u8string(),
         &statistics_switch](const AuxTypingKeys &batch) {
          if (!statistics_switch.enabled())
            return false;
          if (batch.counts.empty())
            return true;
          // Narrowed unit by unit, which is lossless only because the parser admitted nothing but ASCII; constructing std::string from wide iterators narrows implicitly, and MSVC's /WX rejects that (C4244).
          const auto ascii = [](const std::wstring &value) {
            std::string narrow;
            narrow.reserve(value.size());
            for (const wchar_t unit : value)
              narrow.push_back(static_cast<char>(unit));
            return narrow;
          };
          std::map<std::string, uint64_t> keys;
          for (const auto &[key, count] : batch.counts)
            keys.emplace(ascii(key), count);
          record_typing_keys_async(statistics_directory, ascii(batch.day), keys);
          return true;
        });
    // The fifth pipe: TIP diagnostics. The TIP has always produced batches on
    // it; nothing ever listened, so enabling diagnostic logging produced
    // nothing at all. Session-less like the Aux endpoint, because a TIP that
    // is failing to compose is exactly the one whose diagnostics matter.
    DWORD diagnostic_error = ERROR_SUCCESS;
    auto diagnostics = DiagnosticListener::create(
        FANY_IME_TSF_DIAGNOSTIC_NAMED_PIPE,
        [&diagnostic_log](const DiagnosticBatch &batch) {
          std::string header = "TSF diagnostics pid=" +
                               std::to_string(batch.source_process_id) +
                               " records=" + std::to_string(batch.record_count);
          // A gap in the log is worth saying out loud rather than leaving the
          // reader to wonder why the sequence jumps.
          if (batch.dropped_count)
            header += " dropped=" + std::to_string(batch.dropped_count);
          std::cerr << header << "\n" << batch.payload << "\n";
          diagnostic_log.tsf(header + "\r\n" + batch.payload);
        },
        diagnostic_error);
    if (!diagnostics)
      notice("TSF diagnostics unavailable; continuing without them");
    if (!aux)
      notice("Tray menu unavailable: language bar endpoint not started");
    // The four shortcuts the shared settings page documents. They must work
    // while another application has focus, so they sit on a low-level keyboard
    // hook rather than the TSF key sink.
    MaintenanceHotkeyController maintenance([&](MaintenanceHotkey hotkey) {
      // 几个版本的 Server 同时运行时，各自的低级键盘钩子都会看到这个按键，后装的钩子先看到，处理了就吞掉。焦点上的 TIP 属于别的版本时交给下一个钩子，让那个版本的 Server 处理；没有任何版本的模式活动时（焦点在别的输入法上，或 TIP 会话在崩溃后断开，正是要用重启快捷键的时候）谁先看到谁处理，不能都放过。只装一个版本时与引入版本之前相同。
      if (hotkey.action != MaintenanceAction::DeleteCandidate &&
          !server.mode_active() && other_edition_mode_active())
        return false;
      switch (hotkey.action) {
      case MaintenanceAction::Restart:
        restart_requested.store(true);
        stopping.store(true);
        return true;
      case MaintenanceAction::Stop:
        stop_requested.store(true);
        stopping.store(true);
        return true;
      case MaintenanceAction::ClearCache: {
        return server.reset_cache();
      }
      case MaintenanceAction::OpenScreenKeyboard: {
        const auto request =
            shell_surface_request(TrayMenuCommand::OpenKeyboardPanel);
        return request && launch_shell(*request);
      }
      case MaintenanceAction::DeleteCandidate: {
        // Only meaningful while a candidate list is on screen; otherwise the
        // stroke belongs to the focused application and must not be eaten.
        const auto view = server.candidate_view();
        if (!view || !view->visible || hotkey.slot >= view->candidates.size())
          return false;
        const auto &candidate = view->candidates[hotkey.slot];
        return server.request_candidate_action(
                   view->lease, candidate.session, candidate.generation,
                   candidate.index, CandidateAction::Remove) ==
               CandidateActionRequestResult::Sent;
      }
      }
      return false;
    },
    [&](bool caps) {
      // The Server owns the indicator; publish and let the loop deliver it, so
      // the hook callback never touches the transport.
      caps_lock.store(caps, std::memory_order_release);
      caps_lock_revision.mark_changed();
    });
    if (!maintenance.installed())
      notice("Maintenance shortcuts unavailable; continuing without them");
    uint64_t tray_shown_at = 0;
    uint64_t pointer_left_at = 0;
    HWND tray_foreground = nullptr;
    // Keep the operator-facing status truthful in both launch modes. The
    // managed Server uses the production TSF pipe, while the preview binary
    // uses an isolated endpoint; conflating them makes support logs suggest
    // that a preview instance is serving the installed input method.
    std::cout << (production ? "Production" : "Preview")
              << " Server running; candidate selection and mode controls enabled.\n";
    // 工具栏失败不结束 Server：它只是方便切换模式的附件，Server 记一条诊断、去掉工具栏继续服务输入。曾经它也在这个条件里，某台 Windows 11 上工具栏一失败 Server 就在启动后约 100 ms 退出，日志却只写了一句正常停止。
    bool toolbar_failure_reported = false;
    // 前台呈现方式的诊断：变化时记一行，QUNS 的耗时和候选窗的 z 带各记一次。
    auto logged_presentation = ForegroundPresentation::Windowed;
    bool quns_cost_logged = false;
    bool window_band_logged = false;
    uint64_t tsf_config_applied_revision = 0;
    uint64_t caps_lock_applied_revision = 0;
    while (!stopping.load() && server.failure() == ControllerFailure::None &&
           !candidates.failed() && !clicks.failed() && !pages.failed() &&
           !mode_clicks.failed() &&
           !character_set_clicks.failed() && !english_reads.failed()) {
      if (!toolbar_failure_reported && toolbar.failed()) {
        toolbar_failure_reported = true;
        toolbar.hide();
        notice(component_failure("Floating toolbar", toolbar.failure_site()) +
               "; continuing without it");
      }
      MSG message{};
      // Bound each batch so a message flood cannot starve stop/focus polling.
      for (size_t i = 0;
           i < 64 && PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE); ++i) {
        if (message.message == WM_QUIT) {
          stopping.store(true);
          break;
        }
        TranslateMessage(&message);
        DispatchMessageW(&message);
      }
      if (stopping.load())
        break;
      voice_hotkeys.refresh();
      voice->maintain();
      voice_controller_dispatch.maintain();
      if (auto request = voice_controller_mailbox.take())
        request->complete(voice_controller_dispatch.dispatch(request->channel,
                                                             request->request));
      if (auto fonts = candidate_fonts->take())
        candidates.set_fonts(*fonts);
      if (auto style = candidate_style->take())
        candidates.set_style(*style);
      const auto next_candidate_layout = CandidateLayoutSettings::decode(
          candidate_layout->load(std::memory_order_acquire));
      candidates.set_layout(next_candidate_layout);
      if (auto theme = candidate_theme->take()) {
        if (*theme != current_candidate_theme) {
          current_candidate_theme = std::move(*theme);
          resolved_themes.clear();
          skin_assets.clear();
          candidate_theme_dirty = true;
        }
      }
      const bool candidate_horizontal = next_candidate_layout.horizontal;
      const auto theme_now = GetTickCount64();
      if (candidate_theme_dirty || candidate_horizontal != candidate_horizontal_applied ||
          theme_now >= candidate_theme_check_at) {
        candidate_theme_check_at = theme_now + 500;
        system_dark = system_prefers_dark();
        // An edited package is resolved again: its colours through the shared layer and its artwork from the catalog.
        const bool skin_resources_changed = candidate_skin_revision.changed(
            config.skin_directory,
            candidate_theme_package(current_candidate_theme));
        if (skin_resources_changed) {
          resolved_themes.clear();
          skin_assets.clear();
        }
        const bool dark =
            candidate_theme_dark(current_candidate_theme, system_dark);
        if (candidate_theme_dirty || skin_resources_changed || dark != candidate_dark_applied ||
            candidate_horizontal != candidate_horizontal_applied) {
          const auto &theme = resolved_theme(dark, candidate_horizontal);
          auto next_palette = candidate_theme_palette(theme, dark);
          if (config.candidate_selected_bar)
            next_palette.show_selected_bar = *config.candidate_selected_bar;
          candidates.set_theme_palette(next_palette);
          if (skin_resources_changed || theme.candidate_skin != candidate_skin_applied) {
            const auto &assets = package_assets(theme.candidate_skin);
            candidates.invalidate_skin_images();
            candidates.set_skin_min_width(assets.min_width);
            candidates.set_skin_decoration(assets.decoration);
            candidates.set_skin_background(assets.background);
            candidates.set_skin_corner_radius(assets.corner_radius);
            candidate_skin_applied = theme.candidate_skin;
          }
          candidate_dark_applied = dark;
          candidate_horizontal_applied = candidate_horizontal;
          candidate_theme_dirty = false;
          ++candidate_theme_generation;
        }
      }
      // 前台和它的呈现方式每轮只算一次：候选窗的兜底定位、独占抑制、锁存、置顶和工具栏的全屏判断都用这一份。
      const HWND foreground = GetForegroundWindow();
      std::optional<uint64_t> quns_microseconds;
      const auto presentation = foreground_presentation(
          foreground, GetTickCount64(),
          quns_cost_logged ? nullptr : &quns_microseconds);
      if (quns_microseconds) {
        quns_cost_logged = true;
        notice("Foreground QUNS query took " +
               std::to_string(*quns_microseconds) + " us");
      }
      if (presentation != logged_presentation) {
        logged_presentation = presentation;
        notice(std::string("Foreground presentation: ") +
               foreground_presentation_name(presentation));
      }
      candidates.set_foreground(foreground, presentation);
      candidates.refresh();
      candidates.keep_on_top();
      for (const auto &change : candidates.take_suppression_changes())
        notice(candidate_suppression_line(change));
      if (!window_band_logged &&
          presentation != ForegroundPresentation::Windowed &&
          IsWindowVisible(candidates.handle())) {
        window_band_logged = true;
        const auto band = candidate_window_band(candidates.handle());
        notice("Candidate window band over fullscreen: " +
               (band ? std::to_string(*band) : std::string("unavailable")));
      }
      // The settings page may have published a new value since the last pass.
      toolbar_visible = toolbar_enabled->load(std::memory_order_acquire);
      if (auto settings = toolbar_settings->take())
        toolbar.set_settings(*settings);
      if (const bool dark = !surface_theme_is_light(
              voice_theme->load(std::memory_order_acquire), system_dark);
          dark != voice_dark_applied || voice_theme_applied != candidate_theme_generation) {
        voice_dark_applied = dark;
        voice_theme_applied = candidate_theme_generation;
        voice_overlay.set_palette(surface_palette(dark));
      }
      if (const bool dark = !surface_theme_is_light(
              menu_theme->load(std::memory_order_acquire), system_dark);
          dark != menu_dark_applied || menu_theme_applied != candidate_theme_generation) {
        menu_dark_applied = dark;
        menu_theme_applied = candidate_theme_generation;
        tray.set_palette(tray_menu_palette(surface_palette(dark)));
      }
      if (const bool dark = !surface_theme_is_light(
              toolbar_theme->load(std::memory_order_acquire), system_dark);
          dark != toolbar_dark_applied || toolbar_theme_applied != candidate_theme_generation) {
        toolbar_dark_applied = dark;
        toolbar_theme_applied = candidate_theme_generation;
        toolbar.set_palette(toolbar_surface_palette(dark));
      }
      // The toolbar is topmost, so without this it floats over full-screen
      // video and presentations. ShouldShowFloatingToolbar was ported long ago
      // but nothing ever supplied its fullscreen argument, leaving the whole
      // predicate dead outside its unit test.
      // Push the TSF-local settings whenever they changed, so turning smart
      // punctuation off takes effect on the text being typed now.
      const auto current_tsf_config_revision = tsf_config_revision->snapshot();
      if (current_tsf_config_revision != tsf_config_applied_revision) {
        if (const auto view = server.mode_view()) {
          msime::windows::TsfLocalConfig pending;
          {
            std::lock_guard<std::mutex> lock(*tsf_config_mutex);
            pending = *tsf_config;
          }
          if (server.send_tsf_config(pending))
            if (tsf_config_revision->is_current(current_tsf_config_revision))
              tsf_config_applied_revision = current_tsf_config_revision;
        }
      }
      // One CN/EN state follows the user between applications when the scope
      // is global. Each TSF client keeps its own mode, so a newly focused one
      // reports whatever it holds and the Server pushes its own back.
      {
        const auto view = server.mode_view();
        const auto decision = mode_authority_step(
            mode_authority, mode_scope_global->load(std::memory_order_acquire),
            view.has_value() && view->chinese.has_value(),
            view ? view->lease.token : 0,
            view && view->chinese ? *view->chinese : true);
        mode_authority = decision.next;
        if (decision.push && view)
          (void)server.request_mode(view->lease,
                                    decision.push_chinese ? WorkerMode::Chinese
                                                          : WorkerMode::English);
      }
      candidates.set_follow_cursor(
          follow_cursor->load(std::memory_order_acquire));
      candidates.set_effect_intensity(
          effect_intensity->load(std::memory_order_acquire));
      // 语言按钮在 Caps Lock 开着时显示 'A'，日文模式显示 日，韩文模式显示 한，粤拼、注音、越南文、藏文、笔画分别显示 粤、注、越、藏、笔，引擎自己的英文模式显示带下划线的 "En"，所以它要跟随这些状态。Caps Lock 开着时显示 中 会让用户误判下一个字母键的作用。
      {
        ToolbarLanguageState language;
        language.caps_lock = caps_lock.load(std::memory_order_acquire);
        if (const auto view = server.mode_view()) {
          language.dedicated_english = english_state.snapshot(view->lease).value_or(false);
          const auto now = GetTickCount64();
          if (now >= english_read_at && english_reads.submit(view->lease))
            english_read_at = now + 250;
        }
        {
          std::lock_guard<std::mutex> lock(*tsf_config_mutex);
          language.mode = tsf_config->input_mode;
          // The TIP's V-mode key rule follows the focused session's English mode (Ctrl+Shift+E, the toolbar exit, a focus change): the next pass pushes the trigger frame again.
          if (tsf_config->dedicated_english != language.dedicated_english) {
            tsf_config->dedicated_english = language.dedicated_english;
            tsf_config_revision->mark_changed();
          }
        }
        toolbar.set_language_state(language);
      }
      const auto current_caps_lock_revision = caps_lock_revision.snapshot();
      if (current_caps_lock_revision != caps_lock_applied_revision) {
        if (const auto view = server.mode_view()) {
          const bool enabled = caps_lock.load(std::memory_order_acquire);
          if (server.send_caps_lock(view->lease, enabled) &&
              caps_lock_revision.is_current(current_caps_lock_revision))
            caps_lock_applied_revision = current_caps_lock_revision;
        }
      }
      const bool fullscreen = presentation != ForegroundPresentation::Windowed;
      // The DLL's activation edges, not the mode view: a temporary focus
      // suspension (Win+. for instance) empties the view without deactivating
      // anything, and gating on the view made the toolbar blink away each time.
      const bool show_toolbar = ShouldShowFloatingToolbar(
          toolbar_visible, fullscreen,
          ime_active.load(std::memory_order_acquire));
      toolbar.refresh(show_toolbar);
      // The listener thread owns no window; the anchor is applied here, on the
      // thread that created the tray card.
      const uint64_t now = GetTickCount64();
      if (const auto anchor = tray_mailbox.take()) {
        switch (tray_menu_request_action(tray.visible(), now, tray_shown_at)) {
        case TrayMenuRequestAction::Show: {
          // The reference presenter resolves its theme on every opening.
          // Sample Windows now rather than relying on the periodic candidate
          // theme check, so a menu opened immediately after a system theme
          // change never flashes the previous palette.
          const bool dark = !surface_theme_is_light(
              menu_theme->load(std::memory_order_acquire),
              system_prefers_dark());
          if (dark != menu_dark_applied ||
              menu_theme_applied != candidate_theme_generation) {
            menu_dark_applied = dark;
            menu_theme_applied = candidate_theme_generation;
            tray.set_palette(tray_menu_palette(surface_palette(dark)));
          }
          if (tray.open(anchor->center_x, anchor->top)) {
            tray_shown_at = now;
            pointer_left_at = now;
            tray_foreground = GetForegroundWindow();
          }
          break;
        }
        case TrayMenuRequestAction::Hide:
          tray.hide();
          break;
        case TrayMenuRequestAction::None:
          break;
        }
      }
      if (tray.visible()) {
        const bool inside = tray.pointer_inside();
        if (inside)
          pointer_left_at = now;
        const bool button_down =
            (GetAsyncKeyState(VK_LBUTTON) | GetAsyncKeyState(VK_RBUTTON)) &
            0x8000;
        if (tray_menu_dismissal(true, now, tray_shown_at, pointer_left_at,
                                inside, button_down != 0,
                                GetForegroundWindow() != tray_foreground))
          tray.hide();
      }
      if (instance)
        instance->publish_mode_active(server.mode_active());
      if (MsgWaitForMultipleObjectsEx(0, nullptr, 50, QS_ALLINPUT,
                                      MWMO_INPUTAVAILABLE) == WAIT_FAILED)
        throw std::runtime_error("Candidate message wait failed");
    }
    candidates.hide();
    // Stop I/O first; it invalidates queued work without waiting for this
    // thread. Retire the matching review before Server/focus teardown.
    if (voice_controller)
      voice_controller->stop();
    voice_controller_dispatch.retire();
    toolbar.hide();
    // Stop the listener and close the mailbox before the window goes away, so a
    // late anchor cannot reach a card that is being destroyed.
    if (aux)
      aux->stop();
    tray_mailbox.stop();
    tray.hide();
    clicks.request_stop();
    character_set_clicks.request_stop();
    mode_clicks.request_stop();
    english_reads.request_stop();
    server.stop();
    clicks.stop();
    character_set_clicks.stop();
    mode_clicks.stop();
    english_reads.stop();
    // Every way out of the message loop is a normal end of this session.
    msime::telemetry::end();
    if (restart_requested.load()) {
      diagnostic_log.server("Server stopping: restart requested");
      return msime::windows::watchdog::restart_exit_code;
    }
    if (stop_requested.load()) {
      diagnostic_log.server("Server stopping: stop requested");
      return msime::windows::watchdog::stop_exit_code;
    }
    // 每个能结束主循环的组件都要在这一行里点名，否则日志只说"有组件失败"，无从查起。翻页曾经结束主循环却被记成正常停止。
    std::vector<std::string> failures;
    if (const auto failure = server.failure(); failure != ControllerFailure::None)
      failures.push_back(std::string("session controller failed (") +
                         controller_failure_name(failure) + ")");
    if (candidates.failed())
      failures.push_back(component_failure("candidate window", candidates.failure_site()));
    if (clicks.failed())
      failures.push_back(component_failure("candidate click worker", std::nullopt));
    if (pages.failed())
      failures.push_back(component_failure("candidate page worker", std::nullopt));
    if (mode_clicks.failed())
      failures.push_back(component_failure("mode click worker", std::nullopt));
    if (character_set_clicks.failed())
      failures.push_back(component_failure("character set click worker", std::nullopt));
    if (english_reads.failed())
      failures.push_back(component_failure("English state reader", std::nullopt));
    diagnostic_log.server(server_stop_line(failures));
    return failures.empty() ? 0 : 1;
  } catch (...) {
    std::cerr << "Preview Server failed; verify configuration, resources, "
                 "state ownership and pipe availability.\n";
    return 1;
  }
}
