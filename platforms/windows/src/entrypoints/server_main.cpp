#include "../../../vendor/MSIME-Engine/contracts/windows_ipc.h"
#include "AuxListener.h"
#include "VoiceTheme.h"
#include "CandidateAppearance.h"
#include "CandidateSkin.h"
#include "SkinResourceRevision.h"
#include "CandidateWindow.h"
#include "ClipboardHistory.h"
#include "DiagnosticListener.h"
#include "DedicatedEnglishMailbox.h"
#include "DiagnosticLog.h"
#include "FloatingToolbarVisibilityPolicy.h"
#include "FirstRun.h"
#include "FloatingToolbarWindow.h"
#include "FocusedSession.h"
#include "FullscreenForeground.h"
#include "MaintenanceHotkey.h"
#include "ModeAuthority.h"
#include "ModeMailbox.h"
#include "PreviewConfig.h"
#include "PreviewDispatcher.h"
#include "ProductionDispatcher.h"
#include "ProductionPipeNames.h"
#include "ProviderToken.h"
#include "ServerLaunch.h"
#include "SharedConfigKeybindings.h"
#include "ShellLauncher.h"
#include "StateRootLease.h"
#include "SystemAudioMuter.h"
#include "TrayMenuDispatch.h"
#include "TrayMenuWindow.h"
#include "VoiceControllerListener.h"
#include "VoiceHotkey.h"
#include "VoiceInputSession.h"
#include "WatchdogPolicy.h"
#include "Telemetry.h"
#include "TelemetryConsent.h"
#include "WindowsServer.h"
#include "ipc_negotiation.h"
#include <fstream>
#include <iostream>
#include <memory>
#include <mutex>
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
// Resolve the configured skin through the shared catalog. Appearance is not
// worth failing a running Server over, so an unreadable root or an unknown
// package leaves the built-in theme in place.
msime::windows::CandidatePalette
resolve_palette(const msime::windows::PreviewConfig &config) {
  const bool dark = config.dark_theme;
  auto builtin = msime::windows::candidate_builtin_palette(config.skin_id, dark);
  if (config.skin_directory.empty() || config.skin_id.empty())
    return builtin;
  // The shipped skins are resolved from the table above, never from disk - the
  // shared catalog refuses to load a package under one of their names, so
  // asking it would only ever come back empty and fall through to fluent.
  if (msime::windows::candidate_builtin_skin(config.skin_id))
    return builtin;
  try {
    const auto root = config.skin_directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_skin_catalog(
            reinterpret_cast<const uint8_t *>(root.data()), root.size()),
        msime_client_string_free);
    if (!owned)
      return builtin;
    const auto document = nlohmann::json::parse(owned.get(), nullptr, false);
    if (document.is_discarded() || !document.value("ok", false))
      return builtin;
    // Compatibility is checked against the layout actually being rendered.
    return msime::windows::candidate_skin_palette(
        document.at("value"), config.skin_id, dark,
        config.horizontal_candidates ? "horizontal" : "vertical");
  } catch (const std::exception &) {
    return builtin;
  }
}
msime::windows::CandidateSkinAssets
resolve_skin_assets(const msime::windows::PreviewConfig &config) {
  if (config.skin_directory.empty() || config.skin_id.empty() ||
      msime::windows::candidate_builtin_skin(config.skin_id))
    return {};
  try {
    const auto root = config.skin_directory.u8string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_skin_catalog(reinterpret_cast<const uint8_t *>(root.data()),
                                  root.size()), msime_client_string_free);
    if (owned) {
      const auto catalog = nlohmann::json::parse(owned.get(), nullptr, false);
      if (!catalog.is_discarded() && catalog.value("ok", false))
        return msime::windows::candidate_skin_assets(
            catalog.at("value"), config.skin_id, config.skin_directory);
    }
  } catch (const std::exception &) {
  }
  return {};
}
std::atomic<bool> stopping{false};
std::atomic<bool> restart_requested{false};
// Set by the maintenance stop shortcut. The Watchdog reads any other exit as a crash and starts the Server again, so a user's stop has to leave with stop_exit_code, as the reference's window hook does.
std::atomic<bool> stop_requested{false};
// The user's `telemetry_enabled` preference, read by the terminate hook on whatever thread fails. Off until the stored preferences say otherwise, so a Server that dies before reading them reports nothing.
std::atomic<bool> telemetry_allowed{false};
static_assert(std::atomic<bool>::is_always_lock_free);
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
std::string read_document(const std::filesystem::path &path) {
  std::ifstream input(path, std::ios::binary);
  if (!input)
    throw std::runtime_error("Configuration unavailable");
  std::string document(kMaxConfigBytes + 1, '\0');
  input.read(document.data(), static_cast<std::streamsize>(document.size()));
  if (input.bad() || input.gcount() > static_cast<std::streamsize>(kMaxConfigBytes))
    throw std::runtime_error("Configuration read failed");
  document.resize(static_cast<size_t>(input.gcount()));
  return document;
}
void write_document_atomic(const std::filesystem::path &path, const std::string &document) {
  if (document.size() > kMaxConfigBytes)
    throw std::runtime_error("Configuration document oversized");
  const auto temporary = path.wstring() + L".tmp";
  {
    std::ofstream output(std::filesystem::path(temporary), std::ios::binary | std::ios::trunc);
    if (!output) throw std::runtime_error("Configuration temporary file unavailable");
    output.write(document.data(), static_cast<std::streamsize>(document.size()));
    output.flush();
    if (!output) throw std::runtime_error("Configuration write failed");
  }
  if (!MoveFileExW(temporary.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH))
    throw std::runtime_error("Configuration replace failed");
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
// Map the shared preferences onto the settings the TIP keeps in its own
// globals. The TIP consumes every one of these, but nothing ever sent them, so
// they sat at their compiled defaults: turning smart or paired punctuation off
// did nothing, the Microsoft shuangpin ';' key was never enabled, and the
// inline preedit style stayed "raw" whatever the user picked.
// The token for the provider actually in use.
//
// Tokens are kept one per provider so switching provider restores the matching
// key instead of sending the previous provider's key to the new endpoint. The
// flat field remains the value the box currently holds, so it is the right
// fallback for a store written before the slots existed.
msime::windows::TsfLocalConfig tsf_local_config(const nlohmann::json &preferences) {
  msime::windows::TsfLocalConfig config;
  const auto navigation =
      preferences.value("navigation", nlohmann::json::object());
  config.paging_comma_period = navigation.value("comma_period", true);
  config.preedit_style = msime::windows::tsf_preedit_style(preferences);
  // PreviewConfig spells the pass-through case "local"; the TIP spells it "raw".
  if (config.preedit_style == "local")
    config.preedit_style = "raw";
  // Windows follows the upstream split smart-punctuation policy: the feature
  // is opt-in, and legacy profiles without the key must not silently enable
  // punctuation rewriting.
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
      preferences.value("scheme", std::string("quanpin")) == "shuangpin" &&
      preferences.value("shuangpin_profile", std::string("xiaohe")) == "microsoft";
  config.japanese_input_mode =
      preferences.value("scheme", std::string("quanpin")) == "japanese";
  config.tsf_diagnostic_log =
      preferences.value("diagnostic_log", nlohmann::json::object())
          .value("tsf", false);
  const auto lock = preferences.value("punctuation_lock", std::string("follow"));
  config.punctuation_lock = lock == "chinese" ? 1 : lock == "english" ? 2 : 0;
  return config;
}

void apply_diagnostic_log(msime::windows::DiagnosticLog &log,
                          const nlohmann::json &preferences) {
  const auto switches =
      preferences.value("diagnostic_log", nlohmann::json::object());
  log.set_enabled(switches.value("server", false), switches.value("tsf", false));
}

// Mirror the CN/EN and 简繁 hotkeys into the shared config.toml.
//
// These four do not ride the worker pipe: the TIP reads them straight off disk
// at activation. Without this the settings toggles would save and do nothing,
// which is why they were hidden on Windows. Writing is best effort - a config
// we cannot update costs the user their hotkey choice, never the IME.
void publish_switch_language_keybindings(const nlohmann::json &preferences) {
  // The same folder the TIP resolves. With no root there is nowhere to write, and a bare relative config.toml would land in the working directory.
  const auto state = production_state_directory();
  if (state.empty())
    return;
  const std::filesystem::path path = state / L"config.toml";
  const auto bindings =
      preferences.value("keybindings", nlohmann::json::object());
  msime::windows::SwitchLanguageKeybindings values;
  values.shift = bindings.value("switch_language_shift", true);
  values.ctrl = bindings.value("switch_language_ctrl", false);
  values.ctrl_alt_space = bindings.value("switch_language_ctrl_alt_space", true);
  values.character_set_ctrl_shift_f =
      bindings.value("toggle_character_set_ctrl_shift_f", true);
  try {
    std::string existing;
    {
      std::ifstream input(path, std::ios::binary);
      if (input) {
        existing.resize(kMaxConfigBytes + 1);
        input.read(existing.data(), static_cast<std::streamsize>(existing.size()));
        if (input.bad() || input.gcount() > static_cast<std::streamsize>(kMaxConfigBytes))
          return;
        existing.resize(static_cast<std::size_t>(input.gcount()));
      }
    }
    const auto updated = msime::windows::update_keybindings(existing, values);
    if (updated == existing)
      return;
    std::error_code ignored;
    std::filesystem::create_directories(path.parent_path(), ignored);
    // Write beside the target and rename over it: a crash mid-write must not
    // leave the user with a truncated config the TIP then reads as defaults.
    const auto temporary = std::filesystem::path(path).concat(L".new");
    {
      std::ofstream output(temporary, std::ios::binary | std::ios::trunc);
      if (!output)
        return;
      output.write(updated.data(),
                   static_cast<std::streamsize>(updated.size()));
      if (!output)
        return;
    }
    std::filesystem::rename(temporary, path, ignored);
    if (ignored)
      std::filesystem::remove(temporary, ignored);
  } catch (const std::exception &) {
    // A read-only or roaming profile is the user's business, not a fatal error.
  }
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
class ProductionInstance final {
public:
  ProductionInstance() {
    handle_ = CreateMutexW(nullptr, FALSE,
                           L"Local\\MetasequoiaImeServer_SingleInstance");
    if (!handle_)
      throw std::runtime_error("Server instance guard unavailable");
    already_running_ = GetLastError() == ERROR_ALREADY_EXISTS;
  }
  ~ProductionInstance() {
    if (handle_)
      CloseHandle(handle_);
  }
  bool already_running() const { return already_running_; }
private:
  HANDLE handle_ = nullptr;
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
} // namespace
int wmain(int argc, wchar_t **argv) {
  // Before any thread exists: libcurl's global init is not thread-safe, and the startup event's thread, a crash report on any thread and the online workers all use it.
  curl_global_init(CURL_GLOBAL_DEFAULT);
  std::set_terminate([] {
    if (telemetry_allowed.load(std::memory_order_acquire))
      msime::telemetry::crash("windows", MSIME_WINDOWS_VERSION, "std::terminate");
    std::abort();
  });
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
      prepare_first_run(executable_directory(), default_state,
                       [](const std::string &request) {
        std::unique_ptr<char, decltype(&msime_client_string_free)> response(
            msime_client_prepare_host(
                reinterpret_cast<const uint8_t *>(request.data()), request.size()),
            msime_client_string_free);
        if (!response)
          throw std::runtime_error("Host preparation failed");
        return std::string(response.get());
      });
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
    const auto bootstrap =
        nlohmann::json{{"resources", config.resources.u8string()},
                       {"state_root", config.state_root.u8string()}}
            .dump();
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
    // Opt-in: nothing is sent and telemetry.json is not written unless the stored preferences turn it on. The startup event runs off the main thread so an unreachable endpoint (up to the 8 s request timeout) cannot delay the Server; the thread is never joined, so exiting mid-request only drops this event.
    if (msime::windows::telemetry_consented(prepared.at("value").at("preferences"))) {
      telemetry_allowed.store(true, std::memory_order_release);
      std::thread([] { msime::telemetry::start("windows", MSIME_WINDOWS_VERSION); }).detach();
    }
    diagnostic_log.server(std::string(production ? "Production" : "Preview") +
                          " Server starting");
    auto traditional_output = std::make_shared<std::atomic<bool>>(
        prepared.at("value").at("preferences")
            .value("traditional_chinese_output", false));
    auto tsf_config = std::make_shared<msime::windows::TsfLocalConfig>(
        tsf_local_config(prepared.at("value").at("preferences")));
    auto tsf_config_mutex = std::make_shared<std::mutex>();
    // Set on every publication and on each focus session, so a TIP that
    // registers later is not left holding compiled defaults.
    auto tsf_config_dirty = std::make_shared<std::atomic<bool>>(true);
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
    options.pipes.capabilities =
        production ? (FanyImeProtocol::Capabilities |
                      FanyImeProtocol::CharacterSetShortcut)
                   : FanyImeProtocol::RequiredCapabilities;
    options.preferences_directory = config.state_root.u8string();
    options.preferences_published =
        [&, voice_config, voice_config_mutex, voice_host_options, traditional_output,
         toolbar_enabled, follow_cursor, voice_theme, candidate_fonts,
         toolbar_theme, menu_theme, mode_scope_global, tsf_config, candidate_layout,
         tsf_config_mutex,
         tsf_config_dirty, candidate_theme, toolbar_settings](const PreferenceSnapshot &snapshot) {
          const auto preferences =
              nlohmann::json::parse(snapshot.serialized()).at("preferences");
          candidate_theme->publish(preferences);
          apply_diagnostic_log(diagnostic_log, preferences);
          // A change applies to crash reports straight away; the startup event is sent at the next Server start.
          telemetry_allowed.store(msime::windows::telemetry_consented(preferences),
                                  std::memory_order_release);
          if (auto settings = floating_toolbar_settings(preferences))
            toolbar_settings->publish(snapshot.revision(), *settings);
          if (auto fonts = candidate_font_settings(preferences))
            candidate_fonts->publish(snapshot.revision(), std::move(*fonts));
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
            *tsf_config = tsf_local_config(preferences);
            tsf_config_dirty->store(true, std::memory_order_release);
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
          publish_switch_language_keybindings(preferences);
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
          next.polish_prompt = input.value("polish_prompt", std::string{});
          next.polish_prompt_custom_1 = input.value("polish_prompt_custom_1", std::string{});
          next.polish_prompt_custom_2 = input.value("polish_prompt_custom_2", std::string{});
          next.polish_prompt_custom_3 = input.value("polish_prompt_custom_3", std::string{});
          std::lock_guard lock(*voice_config_mutex);
          *voice_config = std::move(next);
        };
    WindowsServer server(
        options, prepared.at("value").dump(),
        production
              ? production_key_handler([&character_set_clicks](bool desired) {
                return character_set_clicks.submit(CharacterSetClick{desired});
              })
            : preview_key_handler(config),
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    WaveOverlay voice_overlay;
    voice_overlay.set_light_theme(surface_theme_is_light(
        voice_theme->load(std::memory_order_acquire), system_prefers_dark()));
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
    // Keep the pre-dedicated endpoint alive during rolling upgrades. Both
    // listeners feed the same authenticated mailbox and dispatcher; a client
    // still using VoiceControllerV2 therefore receives identical ownership
    // and generation checks.
    DWORD legacy_voice_controller_error = ERROR_SUCCESS;
    auto legacy_voice_controller = VoiceControllerListener::create(
        voice_controller_mailbox, legacy_voice_controller_error,
        FanyImeVoiceController::PipeName);
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
    std::optional<COLORREF> candidate_text_color;
    if (!config.candidate_text_color.empty() && config.candidate_text_color != "auto" &&
        config.candidate_text_color != "none") {
      const auto color = parse_css_color(config.candidate_text_color, {});
      candidate_text_color = RGB(static_cast<BYTE>(color.r * 255.0f),
                                 static_cast<BYTE>(color.g * 255.0f),
                                 static_cast<BYTE>(color.b * 255.0f));
    }
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
        static_cast<unsigned>(config.candidate_preedit_font_size), candidate_text_color,
        config.candidate_font, config.candidate_fallback_fonts, config.dark_theme,
        config.horizontal_candidates, config.candidate_show_preedit,
        [&](const CandidatePage &page) { (void)pages.submit(page); },
        [&](const CandidatePresentation &value) {
          server.candidate_rendered(value.lease, value.render_serial);
        },
        config.navigation.mouse_wheel);
    const auto palette = resolve_palette(config);
    // An external package may ask for a wider card than the font implies; the
    // artwork is drawn against that width.
    const auto skin_assets = resolve_skin_assets(config);
    const double skin_min_width = skin_assets.min_width;
    const auto &skin_decoration = skin_assets.decoration;
    auto resolved_palette = palette;
    if (!config.candidate_number_color.empty() && config.candidate_number_color != "auto" &&
        config.candidate_number_color != "none")
      resolved_palette.number = parse_css_color(config.candidate_number_color, resolved_palette.number);
    if (!config.candidate_surface_color.empty() && config.candidate_surface_color != "auto" &&
        config.candidate_surface_color != "none")
      resolved_palette.surface = parse_css_color(config.candidate_surface_color, resolved_palette.surface);
    if (!config.candidate_border_color.empty() && config.candidate_border_color != "auto" &&
        config.candidate_border_color != "none")
      resolved_palette.border = parse_css_color(config.candidate_border_color, resolved_palette.border);
    if (!config.candidate_selected_color.empty() && config.candidate_selected_color != "auto" &&
        config.candidate_selected_color != "none")
      resolved_palette.selected = parse_css_color(config.candidate_selected_color, resolved_palette.selected);
    if (!config.candidate_hover_color.empty() && config.candidate_hover_color != "auto" &&
        config.candidate_hover_color != "none")
      resolved_palette.hover = parse_css_color(config.candidate_hover_color, resolved_palette.hover);
    if (!config.candidate_accent_color.empty() && config.candidate_accent_color != "auto" &&
        config.candidate_accent_color != "none")
      resolved_palette.accent = parse_css_color(config.candidate_accent_color, resolved_palette.accent);
    if (config.candidate_selected_bar)
      resolved_palette.show_selected_bar = *config.candidate_selected_bar;
    candidates.set_palette(resolved_palette);
    candidates.set_skin_min_width(skin_min_width);
    candidates.set_follow_cursor(follow_cursor->load(std::memory_order_acquire));
    candidates.set_skin_decoration(skin_decoration.image, skin_decoration.top_dip,
                                   skin_decoration.width_dip);
    auto current_candidate_theme = candidate_theme_values(
        prepared.at("value").at("preferences"));
    bool candidate_theme_dirty = true;
    bool candidate_dark_applied = config.dark_theme;
    bool candidate_horizontal_applied = config.horizontal_candidates;
    std::string candidate_skin_applied = config.skin_id;
    uint64_t candidate_theme_check_at = 0;
    bool system_dark = system_prefers_dark();
    SkinResourceRevision candidate_skin_revision;
    bool toolbar_visible = toolbar_enabled->load(std::memory_order_acquire);
    FloatingToolbarWindow toolbar(
        [&] { return server.mode_view(); },
        [&](const ModeClick &click) { (void)mode_clicks.submit(click); });
    // The toolbar draws the shipped native presenter's fixed neutral colours; only its own light/dark preference changes them.
    bool toolbar_dark_applied = !surface_theme_is_light(
        toolbar_theme->load(std::memory_order_acquire), system_dark);
    std::string toolbar_skin_applied = config.skin_id;
    toolbar.set_palette(toolbar_palette(toolbar_dark_applied));
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
    menu_capabilities.handwriting_panel = preview_shell.has_value();
    menu_capabilities.keyboard_panel = preview_shell.has_value();
    menu_capabilities.voice_input = true;
    menu_capabilities.settings = settings_shell.has_value();
    TrayMenuWindow tray(
        menu_capabilities,
        [&](TrayMenuCommand command) {
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
        [&] { return toolbar_visible; });
    // The menu follows its own theme with the shipped native presenter's fixed neutral colours, like the toolbar; the candidate skin does not reach it.
    bool menu_dark_applied = !surface_theme_is_light(
        menu_theme->load(std::memory_order_acquire), system_dark);
    std::string menu_skin_applied = config.skin_id;
    tray.set_palette(tray_menu_palette(menu_dark_applied));
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
    std::atomic<bool> caps_lock_dirty{true};
    // Starts true: the Server is launched by the TIP, so the IME is active by
    // the time this runs, and waiting for the first edge would hide the toolbar
    // until the user switched focus once.
    std::atomic<bool> ime_active{true};
    // The language bar sends a right click over the Aux pipe; without a
    // listener the tray menu - and with it every shared-shell entry - is
    // unreachable. A failure here costs the menu, never the IME.
    TrayMenuMailbox tray_mailbox;
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
        [statistics_directory = config.state_root.u8string()](
            const AuxTypingStatistics &batch) {
          if (msime_client_typing_statistics_enabled(
                  reinterpret_cast<const uint8_t *>(statistics_directory.data()),
                  statistics_directory.size()) != 1)
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
          record_typing_statistics_async(
              statistics_directory, text,
              batch.english ? TypingSource::English : TypingSource::Unknown);
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
      caps_lock_dirty.store(true, std::memory_order_release);
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
    while (!stopping.load() && server.failure() == ControllerFailure::None &&
           !candidates.failed() && !clicks.failed() && !pages.failed() &&
           !mode_clicks.failed() &&
           !character_set_clicks.failed() && !english_reads.failed() && !toolbar.failed()) {
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
      const auto next_candidate_layout = CandidateLayoutSettings::decode(
          candidate_layout->load(std::memory_order_acquire));
      candidates.set_layout(next_candidate_layout);
      if (auto theme = candidate_theme->take()) {
        if (*theme != current_candidate_theme) {
          current_candidate_theme = std::move(*theme);
          candidate_theme_dirty = true;
        }
      }
      const bool candidate_horizontal = next_candidate_layout.horizontal;
      const auto theme_now = GetTickCount64();
      if (candidate_theme_dirty || candidate_horizontal != candidate_horizontal_applied ||
          theme_now >= candidate_theme_check_at) {
        candidate_theme_check_at = theme_now + 500;
        system_dark = system_prefers_dark();
        const auto selected_skin = current_candidate_theme.value(
            "candidate_skin", candidate_skin_applied);
        const bool skin_resources_changed = candidate_skin_revision.changed(
            config.skin_directory, selected_skin);
        const bool dark =
            candidate_theme_dark(current_candidate_theme, system_dark);
        if (candidate_theme_dirty || skin_resources_changed || dark != candidate_dark_applied ||
            candidate_horizontal != candidate_horizontal_applied) {
          auto theme_config = config;
          theme_config.skin_id = current_candidate_theme.value(
              "candidate_skin", candidate_skin_applied);
          theme_config.dark_theme = dark;
          theme_config.horizontal_candidates = candidate_horizontal;
          auto next_palette = candidate_theme_palette(resolve_palette(theme_config),
                                                        current_candidate_theme);
          if (config.candidate_selected_bar)
            next_palette.show_selected_bar = *config.candidate_selected_bar;
          candidates.set_theme_palette(next_palette);
          if (skin_resources_changed || theme_config.skin_id != candidate_skin_applied) {
            const auto assets = resolve_skin_assets(theme_config);
            candidates.invalidate_skin_images();
            candidates.set_skin_min_width(assets.min_width);
            candidates.set_skin_decoration(assets.decoration.image,
                assets.decoration.top_dip, assets.decoration.width_dip);
            candidate_skin_applied = theme_config.skin_id;
          }
          candidate_dark_applied = dark;
          candidate_horizontal_applied = candidate_horizontal;
          candidate_theme_dirty = false;
        }
      }
      candidates.refresh();
      // The settings page may have published a new value since the last pass.
      toolbar_visible = toolbar_enabled->load(std::memory_order_acquire);
      if (auto settings = toolbar_settings->take())
        toolbar.set_settings(*settings);
      voice_overlay.set_light_theme(surface_theme_is_light(
          voice_theme->load(std::memory_order_acquire), system_dark));
      if (const bool dark = !surface_theme_is_light(
              menu_theme->load(std::memory_order_acquire), system_dark);
          dark != menu_dark_applied || menu_skin_applied != candidate_skin_applied) {
        menu_dark_applied = dark;
        menu_skin_applied = candidate_skin_applied;
        tray.set_palette(tray_menu_palette(dark));
      }
      if (const bool dark = !surface_theme_is_light(
              toolbar_theme->load(std::memory_order_acquire), system_dark);
          dark != toolbar_dark_applied || toolbar_skin_applied != candidate_skin_applied) {
        toolbar_dark_applied = dark;
        toolbar_skin_applied = candidate_skin_applied;
        toolbar.set_palette(toolbar_palette(dark));
      }
      // The toolbar is topmost, so without this it floats over full-screen
      // video and presentations. ShouldShowFloatingToolbar was ported long ago
      // but nothing ever supplied its fullscreen argument, leaving the whole
      // predicate dead outside its unit test.
      // Push the TSF-local settings whenever they changed, so turning smart
      // punctuation off takes effect on the text being typed now.
      if (tsf_config_dirty->load(std::memory_order_acquire)) {
        if (const auto view = server.mode_view()) {
          msime::windows::TsfLocalConfig pending;
          {
            std::lock_guard<std::mutex> lock(*tsf_config_mutex);
            pending = *tsf_config;
          }
          if (server.send_tsf_config(pending))
            tsf_config_dirty->store(false, std::memory_order_release);
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
      // The language button shows 'A' while Caps Lock is on, 日 in Japanese
      // mode and an underlined "En" in the Engine's own English mode, so it
      // has to follow all three. Showing 中 with Caps Lock on tells the user
      // the wrong thing about what the next letter key will do.
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
          language.japanese = tsf_config->japanese_input_mode;
        }
        toolbar.set_language_state(language);
      }
      if (caps_lock_dirty.load(std::memory_order_acquire)) {
        if (const auto view = server.mode_view())
          if (server.send_caps_lock(view->lease,
                                    caps_lock.load(std::memory_order_acquire)))
            caps_lock_dirty.store(false, std::memory_order_release);
      }
      const bool fullscreen = foreground_is_fullscreen(GetForegroundWindow());
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
              menu_skin_applied != candidate_skin_applied) {
            menu_dark_applied = dark;
            menu_skin_applied = candidate_skin_applied;
            tray.set_palette(tray_menu_palette(dark));
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
      if (MsgWaitForMultipleObjectsEx(0, nullptr, 50, QS_ALLINPUT,
                                      MWMO_INPUTAVAILABLE) == WAIT_FAILED)
        throw std::runtime_error("Candidate message wait failed");
    }
    candidates.hide();
    // Stop I/O first; it invalidates queued work without waiting for this
    // thread. Retire the matching review before Server/focus teardown.
    if (voice_controller)
      voice_controller->stop();
    if (legacy_voice_controller)
      legacy_voice_controller->stop();
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
    if (restart_requested.load()) {
      diagnostic_log.server("Server stopping: restart requested");
      return msime::windows::watchdog::restart_exit_code;
    }
    if (stop_requested.load()) {
      diagnostic_log.server("Server stopping: stop requested");
      return msime::windows::watchdog::stop_exit_code;
    }
    const bool clean = server.failure() == ControllerFailure::None &&
                       !candidates.failed() && !clicks.failed() &&
                       !mode_clicks.failed() &&
                       !character_set_clicks.failed() && !english_reads.failed();
    diagnostic_log.server(clean ? "Server stopping" : "Server stopping: a component failed");
    return clean ? 0 : 1;
  } catch (...) {
    std::cerr << "Preview Server failed; verify configuration, resources, "
                 "state ownership and pipe availability.\n";
    return 1;
  }
}
