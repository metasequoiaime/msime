#include "msime_client.h"
#ifdef MSIME_FCITX5_TELEMETRY
#include "Telemetry.h"
#endif
#include "../src/system/ChineseTextConversion.h"
#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/utf8.h>
#include <fcitx-utils/event.h>
#include <fcitx-utils/misc.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/addoninstance.h>
#include <fcitx-config/configuration.h>
#include <fcitx-config/rawconfig.h>
#include <fcitx/action.h>
#include <fcitx/statusarea.h>
#include <fcitx/menu.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/surroundingtext.h>
#include <fcitx/userinterface.h>
#include "../src/candidates/CandidateActionPolicy.h"
#include "../src/candidates/CandidateLocalModeLabels.h"
#include "../src/candidates/CandidatePanelStatus.h"
#include "../src/candidates/CandidatePalette.h"
#include "../src/candidates/CandidateColors.h"
#include "../src/candidates/CandidateFcitxTheme.h"
#include "../src/candidates/FcitxThemeLogo.h"
#include "../src/candidates/CandidateFontPolicy.h"
#include "../src/candidates/CandidateWheelPaging.h"
#include "../src/candidates/PanelRestoreRecord.h"
#include "../src/candidates/ShuangpinProfileNames.h"
#include "../src/candidates/CandidateTranslationPolicy.h"
#include "../src/candidates/PairedPunctuation.h"
#include "../src/core/CandidateSkinCatalog.h"
#include "../src/core/GlobalTheme.h"
#include "../src/core/DictionaryQuiesceLease.h"
#include "../src/core/EmojiPluginGroups.h"
#include "../src/core/RuntimeOptionsRefresh.h"
#include "../src/core/FirstRunGuidance.h"
#include "../src/core/InputModeIndicator.h"
#include "../src/core/InputStatus.h"
#include "../src/core/ReplacedProgram.h"
#ifdef MSIME_FCITX5_MODE_BADGE
#include "../src/overlay/ModeBadgeSurface.h"
#endif
#include "../src/core/BackspaceHoldPolicy.h"
#include "../src/core/SmartPunctuationSpace.h"
#include "../src/core/SpellingSymbols.h"
#include "../src/core/LocalModeSwitches.h"
#include "../src/system/KeySound.h"
#include "../src/system/DiagnosticLog.h"
#include "../src/system/PanelInputChannel.h"
#include "../src/core/HelpcodeDefaults.h"
#include "../src/core/HelpcodeSchemaNames.h"
#include "../src/core/PhrasePreedit.h"
#include "../src/core/ClientInputModeMemory.h"
#include "../src/core/JapaneseConversion.h"
#include "../src/core/KoreanHanja.h"
#include "../src/core/InputSchemes.h"
#include "../src/system/TypingStatistics.h"
#include "SystemTheme.h"
#include "PrecedingCharacters.h"
#include "../src/voice/VoiceAction.h"
#include "../src/voice/VoiceHotwords.h"
#include "../src/voice/VoiceProviderOptions.h"
#include "../src/overlay/WaveOverlayModel.h"
#include "../src/overlay/WaveOverlaySurface.h"
#ifdef MSIME_LINUX_HAS_X11_SURFACE
#include "../src/overlay/WaveOverlayX11Surface.h"
#endif
#ifdef MSIME_LINUX_HAS_WAYLAND_SURFACE
#include "../src/overlay/WaveOverlayWaylandSurface.h"
#endif
#include <nlohmann/json.hpp>
#include <algorithm>
#include <array>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <initializer_list>
#include <memory>
#include <optional>
#include <stdexcept>
#include <future>
#include <chrono>
#include <cmath>
#include <spawn.h>
#include <unistd.h>
#include <string_view>
#include <utility>
#include <vector>
#include <cstring>
#include <cctype>
#include <mutex>
#include <thread>
#include <ctime>
#if __has_include(<fcitx/candidateaction.h>)
#include <fcitx/candidateaction.h>
#define MSIME_FCITX_ACTIONS 1
#endif

#ifndef MSIME_SYSTEM_OPTIONS
#define MSIME_SYSTEM_OPTIONS "/etc/msime-client/runtime-options.json"
#endif
#ifndef MSIME_BINDIR
#define MSIME_BINDIR "/usr/bin"
#endif
#ifndef MSIME_SOUND_PACKS
#define MSIME_SOUND_PACKS "/usr/share/msime-client/sound-packs"
#endif

extern char **environ;

namespace msime::fcitx_host {
using Json = nlohmann::json;
class FcitxEngine;
class FcitxMaintenanceAction;

// Unlike std::async, dropping the future never waits: close() and ~FcitxState run on the Fcitx5 loop and must not block on a provider. Workers copy their inputs; one still running at unload shares the statistics thread's risk.
template <class F> std::shared_future<Json> detachedJob(F work) {
  std::packaged_task<Json()> task(std::move(work));
  auto future = task.get_future().share();
  std::thread(std::move(task)).detach();
  return future;
}

struct PendingPreferenceSave {
  std::string directory;
  std::string section;
  std::string key;
  Json value;
  // The value is a 主题 menu change (theme_choice_change): it writes global_theme and custom_theme together rather than one key.
  bool theme_choice = false;
};

// ABI buffers and errors never escape into diagnostics or the panel.
Json response(char *raw) {
  std::unique_ptr<char, decltype(&msime_client_string_free)> owned(raw, msime_client_string_free);
  if (!raw) throw std::runtime_error("MSIME request failed");
  auto value = Json::parse(raw);
  if (!value.value("ok", false)) throw std::runtime_error("MSIME request failed");
  return value.at("value");
}

Json readOptions();

// 读已安装符号集插件的组（`list_plugin_symbol_groups`）。读不出来时返回空列表，表情面板照常只显示内置目录。
Json loadPluginSymbolGroups(const std::string &resources, const std::string &plugins) {
  if (plugins.empty() || resources.empty()) return Json::array();
  try {
    const auto query = Json{{"limit", 1}, {"list_plugin_symbol_groups", true}, {"plugins", plugins}}.dump();
    auto listed = response(msime_client_emoji_catalog_request(
        reinterpret_cast<const uint8_t *>(query.data()), query.size(),
        reinterpret_cast<const uint8_t *>(resources.data()), resources.size()));
    if (listed.is_object()) return listed.value("plugin_symbol_groups", Json::array());
  } catch (...) {}
  return Json::array();
}

Json savePreference(const PendingPreferenceSave &request) {
  // Save where the locator points now, not where the session was opened: moving the data directory rewrites it, and a save must neither land in the old root while it is copied nor recreate it afterwards (core/DictionaryQuiesceLease.h). A held save fails and stays queued for retry.
  const auto options = readOptions();
  const auto directory = options.value("preferences_directory", std::string{});
  if (directory.empty() ||
      msime::linux_host::preference_save_held(options.value("user_data", std::string{})))
    return Json::object();
  auto snapshot = response(msime_client_load_preferences(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
  if (!snapshot.is_object() || !snapshot.contains("revision") ||
      !snapshot.contains("preferences") || !snapshot.at("preferences").is_object())
    return Json::object();
  if (request.theme_choice) msime::linux_host::apply_theme_choice(snapshot["preferences"], request.value);
  else if (request.section.empty()) snapshot["preferences"][request.key] = request.value;
  else snapshot["preferences"][request.section][request.key] = request.value;
  const auto encoded = snapshot.dump();
  return response(msime_client_save_preferences(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size(),
      snapshot.at("revision").get<uint64_t>(),
      reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
}

std::string panelPreview(const std::string &text) {
  const auto length = fcitx::utf8::lengthValidated(text);
  if (length == fcitx::utf8::INVALID_LENGTH) return "…";
  if (length <= 40) return text;
  const auto end = fcitx::utf8::nextNChar(text.begin(), 40);
  return std::string(text.begin(), end) + "…";
}

struct FcitxVoiceMailbox {
  std::mutex mutex;
  std::string partial;
  std::string final;
  uint8_t phase = 0;
  bool phase_seen = false;
  uint8_t level = 0;
  bool level_seen = false;
  bool final_ready = false;
};

std::unique_ptr<msime::linux_host::WaveOverlaySurface>
create_fcitx_wave_overlay_surface(
    msime::linux_host::WaveOverlaySurface::ActionHandler handler) {
  const auto *requested = std::getenv("MSIME_WAVE_OVERLAY_BACKEND");
  const bool force_auxiliary = requested &&
      (std::strcmp(requested, "ibus") == 0 ||
       std::strcmp(requested, "auxiliary") == 0);
  const bool wayland_requested = requested && std::strcmp(requested, "wayland") == 0;
  const bool x11_requested = requested && std::strcmp(requested, "x11") == 0;
#ifdef MSIME_LINUX_HAS_WAYLAND_SURFACE
  if (!force_auxiliary &&
      (wayland_requested || (!x11_requested && std::getenv("WAYLAND_DISPLAY"))))
    return std::make_unique<msime::linux_host::WaveOverlayWaylandSurface>(
        std::move(handler));
#else
  (void)wayland_requested;
#endif
#ifdef MSIME_LINUX_HAS_X11_SURFACE
  if (!force_auxiliary && (x11_requested || std::getenv("DISPLAY")))
    return std::make_unique<msime::linux_host::WaveOverlayX11Surface>(
        std::move(handler));
#else
  (void)x11_requested;
#endif
  return nullptr;
}

extern "C" void fcitxVoiceUpdate(const uint8_t *text, size_t length,
                                  bool final, void *context) noexcept {
  if (!context || (!text && length != 0) || length > 4096) return;
  try {
    auto *mailbox = static_cast<FcitxVoiceMailbox *>(context);
    const std::string value(reinterpret_cast<const char *>(text), length);
    std::lock_guard lock(mailbox->mutex);
    if (final) {
      mailbox->final = value;
      mailbox->final_ready = true;
    } else {
      mailbox->partial = value;
    }
  } catch (...) {}
}

extern "C" void fcitxVoiceStatus(uint8_t phase, void *context) noexcept {
  if (!context || phase > 2) return;
  try {
    auto *mailbox = static_cast<FcitxVoiceMailbox *>(context);
    std::lock_guard lock(mailbox->mutex);
    mailbox->phase = phase;
    mailbox->phase_seen = true;
  } catch (...) {}
}

extern "C" void fcitxVoiceLevel(float level, void *context) noexcept {
  if (!context || !std::isfinite(level) || level < 0.0f || level > 1.0f) return;
  try {
    auto *mailbox = static_cast<FcitxVoiceMailbox *>(context);
    std::lock_guard lock(mailbox->mutex);
    mailbox->level = static_cast<uint8_t>(level * 10.0f + 0.5f);
    mailbox->level_seen = true;
  } catch (...) {}
}

// Neither the user nor the system runtime options exist: first-run setup has not run. Kept apart from every other load failure so the panel can say what to do about it.
struct OptionsNotConfigured : std::runtime_error {
  OptionsNotConfigured() : std::runtime_error("MSIME not configured") {}
};

std::filesystem::path optionsPath() {
  const auto located = msime::linux_host::locate_runtime_options(
      std::getenv("MSIME_FCITX5_OPTIONS"), std::getenv("XDG_CONFIG_HOME"), std::getenv("HOME"),
      MSIME_SYSTEM_OPTIONS);
  if (located.state == msime::linux_host::RuntimeOptionsState::NotConfigured)
    throw OptionsNotConfigured();
  if (located.state != msime::linux_host::RuntimeOptionsState::Found)
    throw std::runtime_error("MSIME configuration unavailable");
  return located.path;
}

Json readOptions() {
  std::ifstream file(optionsPath());
  std::array<char, 16385> data{};
  file.read(data.data(), data.size());
  if (file.bad() || file.gcount() <= 0 || file.gcount() >= static_cast<std::streamsize>(data.size()))
    throw std::runtime_error("MSIME configuration unavailable");
  return Json::parse(data.data(), data.data() + file.gcount());
}

using CandidateSkinCatalog = std::vector<msime::linux_host::CandidateSkin>;

// Fcitx5 gives each input context its own property, so keep the mode memory
// at addon scope just as the IBus host keeps it at engine scope.  The shared
// policy bounds this table and provides an anonymous fallback.
msime::linux_host::ClientInputModeMemory fcitx_app_input_modes;
std::optional<bool> fcitx_global_input_mode;
// Addon scope too: the statistics store is one per preferences directory, not one per input context.
msime::linux_host::TypingStatisticsSwitch fcitx_typing_statistics{msime_client_typing_statistics_enabled};
// Key press writes still running on their own threads, which ~FcitxEngine waits for so the addon library is not unloaded, nor the process ended, under them.
msime::linux_host::PendingWrites fcitx_key_press_writes;
// Set by ~FcitxEngine: Fcitx5 is unloading the addon, usually because it is exiting, so the last batches are written on the loop rather than handed to a thread that may not outlive it.
bool fcitx_key_presses_shutting_down = false;

CandidateSkinCatalog parseCandidateSkinCatalog(const Json &options) {
  return msime::linux_host::parse_configured_skins(options);
}

// 主题目录来自共享层，宿主不留 id 或标题的副本。ABI 的答案在进程内不变，取一次即可；取不到时主题菜单只剩外部皮肤，而不是在这里补一份会漂的表。
const Json &themeCatalog() {
  static const Json document = [] {
    try {
      return response(msime_client_theme_catalog());
    } catch (const std::exception &) {
      return Json::object();
    }
  }();
  return document;
}

// The candidate colours for one preferences document, resolved by the shared layer (msime_client_resolve_theme) in the given mode. The package is a catalogue entry the shared layer reads strictly, and one it refuses fails the whole call, so that costs only the package: the theme is resolved again without it. A call that still fails draws the native tokens.
msime::linux_host::CandidateTheme resolveThemeInMode(const Json &preferences, bool dark, const Json &catalog) {
  auto request = msime::linux_host::candidate_theme_request(preferences, dark, catalog);
  while (true) {
    try {
      const auto encoded = request.dump();
      return msime::linux_host::candidate_theme_colors(
          response(msime_client_resolve_theme(reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())),
          dark);
    } catch (const std::exception &) {
      if (!request.contains("package")) break;
      request.erase("package");
    }
  }
  return msime::linux_host::candidate_theme_colors(Json::object(), dark);
}

// The candidate window's theme, in the mode candidate_theme settles on.
msime::linux_host::CandidateTheme resolveCandidateTheme(const Json &preferences, bool system_dark,
                                                        const Json &catalog) {
  return resolveThemeInMode(preferences, msime::linux_host::candidate_dark_theme(preferences, system_dark), catalog);
}

// The voice overlay's theme: its mode from voice_theme by the rule it has always used (VoiceAction.h), its colours from the resolved theme as the floating toolbar takes them, so the bar MSIME draws matches the candidate window's theme rather than fixed greys. A fixed-appearance theme overrides the mode, as it does for the panel.
msime::linux_host::CandidateTheme resolveVoiceOverlayTheme(const Json &preferences, bool system_dark,
                                                           const Json &catalog) {
  const bool dark = !msime_voice_overlay_light_theme(preferences.value("voice_theme", "follow"),
                                                     preferences.value("theme", "dark"), system_dark);
  return resolveThemeInMode(preferences, dark, catalog);
}

std::string providerSocket(const Json &options, const char *option,
                           const char *environment, const char *filename) {
  auto value = options.value(option, std::string{});
  if (value.empty()) {
    if (const auto *env = std::getenv(environment)) value = env;
  }
  if (!value.empty()) return value;
  if (const auto *runtime = std::getenv("XDG_RUNTIME_DIR")) {
    const auto candidate = std::filesystem::path(runtime) / "msime-client" / filename;
    std::error_code error;
    if (std::filesystem::is_socket(candidate, error)) return candidate.string();
  }
  return {};
}

std::string onlineSocket(const Json &options) {
  return providerSocket(options, "online_provider_socket",
                        "MSIME_ONLINE_PROVIDER_SOCKET", "online.sock");
}

std::string cloudClipboardSocket(const Json &options) {
  return providerSocket(options, "cloud_clipboard_provider_socket",
                        "MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET", "cloud-clipboard.sock");
}

std::string translationSocket(const Json &options) {
  auto value = providerSocket(options, "translation_provider_socket",
                              "MSIME_TRANSLATION_PROVIDER_SOCKET", "translation.sock");
  return value.empty() ? onlineSocket(options) : value;
}

bool launchDesktopPanel(const char *panel) {
  if (!panel || !*panel) return false;
  const char *command = std::getenv("MSIME_CLIENT_SETTINGS_COMMAND");
  if (!command || !*command) command = "msime-linux-settings";
  // About, help, feedback and the local dictionary are settings sections, not desktop surfaces, so each travels as "settings:<category>" exactly as the IBus host sends it; the bare name is not a route head and the shared parser would reject it, leaving the window on its home page.
  const char *page = std::strcmp(panel, "about") == 0        ? "about"
                     : std::strcmp(panel, "help") == 0       ? "help"
                     : std::strcmp(panel, "feedback") == 0   ? "feedback"
                     : std::strcmp(panel, "dictionary") == 0 ? "dictionary"
                                                             : nullptr;
  std::string routeArgument = std::string("--route=") + (page ? std::string("settings:") + page : panel);
  char *arguments[] = {const_cast<char *>(command), routeArgument.data(), nullptr};
  pid_t child = 0;
  return posix_spawnp(&child, command, nullptr, nullptr, arguments, ::environ) == 0;
}

// Asks the user's Fcitx5 to reload its global configuration through its user-session helper, with a fixed argv that keeps the configurable settings launcher out of this service-control path. Fcitx5 does not pass that reload on to addons, so it never reset MSIME; the chord and the status-menu action now reset in process through FcitxEngine::resetSessions instead.
bool reloadFcitxService() {
  char command[] = "fcitx5-remote";
  char reload[] = "-r";
  char *arguments[] = {command, reload, nullptr};
  pid_t child = 0;
  return posix_spawnp(&child, command, nullptr, nullptr, arguments, ::environ) == 0;
}

class FcitxState : public fcitx::InputContextProperty {
public:
  // system_dark is the engine's last probed appearance, so a context opened between probes starts in it rather than in light until the next change.
  FcitxState(fcitx::InputContext &ic, FcitxEngine *engine, fcitx::EventLoop &loop, bool system_dark)
      : ic_(ic), engine_(engine), loop_(&loop), system_dark_(system_dark) {
    wave_overlay_surface_ = create_fcitx_wave_overlay_surface(
        [this](msime::linux_host::WaveOverlayModel::Action action) {
          if (action == msime::linux_host::WaveOverlayModel::Action::Cancel)
            cancelVoice();
          else
            stopVoice();
        });
    preferences_timer_ = loop.addTimeEvent(CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 250000,
        10000, [this](fcitx::EventSourceTime *timer, uint64_t) {
          refreshDictionaryQuiesce();
          refreshProviderSockets();
          refreshPreferences();
          refreshOnline();
          refreshTranslations();
          refreshClipboard();
          refreshCloudClipboard();
          refreshEmoji();
          refreshVoice();
          flushKeyPresses(key_presses_.take_due(static_cast<int64_t>(fcitx::now(CLOCK_MONOTONIC))));
          timer->setNextInterval(250000);
          timer->setOneShot();
          return true;
        });
  }
  ~FcitxState() override { close(); }
  void close() {
    // Every way out of a session passes here, focus loss, deactivation and teardown included, and options_path_ is still the batch's directory.
    flushKeyPresses(key_presses_.take());
    key_presses_.forget_held();
    hideVoiceOverlay();
    wave_overlay_.reset();
    if (session_) msime_linux_diagnostic_write("focus_out");
    music_.release(session_, msime_client_music_set_active);
    // The combo lives in the session; the next one starts from none.
    typing_combo_ = 0;
    key_repeat_.reset();
    if (session_) msime_client_string_free(msime_client_destroy(session_));
    session_ = 0;
    view_ = Json::object();
    preferences_ = Json::object();
    navigation_ = Json::object();
    options_path_.clear();
    resources_.clear();
    space_convert_mark_.clear();
    space_convert_preceding_.clear();
    last_smart_punctuation_ = 0;
    last_smart_punctuation_at_ = {};
    smart_punctuation_rejected_ = 0;
    paired_tracker_.clear();
    session_fullwidth_ = false;
    japanese_conversion_.reset();
    backspace_hold_.reset();
    maintenance_reload_held_ = false;
    toggle_chord_held_ = FcitxKey_None;
    preferences_job_session_ = 0;
    preferences_snapshot_ = Json();
    // A failed status-bar save outlives the focus change, as the IBus host keeps its failed menu save; settle one still in flight so the retry records whether it landed.
    waitForPreferenceSave();
    online_socket_.clear();
    online_query_.clear();
    online_job_session_ = 0;
    ++online_epoch_;
    online_due_ = {};
    ai_due_ = {};
    translation_query_.clear();
    translation_pending_.clear();
    translation_socket_.clear();
    clipboard_path_.clear();
    ++clipboard_generation_;
    clipboard_items_.clear();
    clipboard_loading_ = false;
    clipboard_job_ = {};
    clipboard_mutation_job_ = {};
    cloud_clipboard_socket_.clear();
    ++cloud_clipboard_generation_;
    cloud_clipboard_items_.clear();
    cloud_clipboard_enabled_ = true;
    cloud_clipboard_job_ = {};
    emoji_items_.clear();
    ++emoji_generation_;
    emoji_job_ = {};
    emoji_job_query_.clear();
    emoji_search_mode_ = false;
    emoji_search_.clear();
    emoji_category_.clear();
    emoji_group_.clear();
    emoji_groups_.clear();
    emoji_groups_job_ = {};
    emoji_groups_loaded_ = false;
    emoji_group_index_ = 0;
    emoji_plugin_group_.reset();
    emoji_plugin_groups_.clear();
    emoji_plugins_stale_ = true;
    emoji_offset_ = {};
    emoji_next_offset_ = {};
    emoji_complete_ = false;
    emoji_previous_offsets_.clear();
    if (voice_job_.valid() && !voice_socket_.empty() && voice_generation_ != 0) {
      const auto socket = voice_socket_;
      const auto generation = voice_generation_;
      msime_client_string_free(msime_client_voice_provider_cancel(
          reinterpret_cast<const uint8_t *>(socket.data()), socket.size(), generation));
    }
    voice_socket_.clear();
    voice_language_ = "zh-cn";
    voice_options_ = Json::object();
    voice_host_options_ = Json();
    voice_enabled_ = true;
    voice_hotkey_ctrl_f9_ = true;
    voice_hotkey_ralt_ = true;
    voice_hotkey_ctrl_win_ = false;
    voice_hotkey_rctrl_ralt_ = false;
    voice_hotkey_hold_space_lock_ = true;
    voice_ralt_held_ = false;
    voice_f9_held_ = false;
    voice_ctrl_win_held_ = false;
    voice_rctrl_ralt_held_ = false;
    voice_space_consumed_ = false;
    voice_space_locked_ = false;
    voice_preedit_.clear();
    voice_transcript_.clear();
    if (voice_job_.valid()) {
      if (voice_job_.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
        try { voice_job_.get(); } catch (...) {}
        voice_job_ = {};
        voice_cancelled_ = false;
      } else {
        // Keep the async state alive; destroying an async future here would
        // synchronously wait for a provider that is being cancelled.
        voice_cancelled_ = true;
      }
    }
    voice_mailbox_.reset();
    voice_generation_ = 0;
    voice_partial_seen_ = false;
    voice_phase_seen_ = false;
    voice_level_seen_ = false;
    voice_loading_ = false;
    chinese_punctuation_ = true;
    paired_punctuation_ = true;
    translation_candidates_active_ = false;
    translation_saved_view_ = Json::object();
    translation_options_.clear();
    translation_page_ = 0;
    translation_cursor_ = 0;
  }
  void clearPanel() {
    ic_.inputPanel().reset();
    ic_.updatePreedit();
    ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    refreshModeIndicator();
  }
  // The label Fcitx5 shows for the input method in its tray and panel (FcitxEngine::subModeLabelImpl), in the same words as the mode HUD.
  std::string modeIndicatorLabel() const {
    switch (msime::linux_host::input_mode_indicator(input_enabled_, effectiveScheme(), caps_lock_)) {
    case msime::linux_host::InputModeIndicator::Chinese: return "中";
    case msime::linux_host::InputModeIndicator::Japanese: return "日";
    case msime::linux_host::InputModeIndicator::Korean: return "한";
    case msime::linux_host::InputModeIndicator::Cantonese: return "粤";
    case msime::linux_host::InputModeIndicator::Zhuyin: return "注";
    case msime::linux_host::InputModeIndicator::Vietnamese: return "越";
    case msime::linux_host::InputModeIndicator::English: return "英";
    case msime::linux_host::InputModeIndicator::CapsLock: return "⇪";
    }
    return "中";
  }
  // The focused context's mode for a bar without a tray (see InputStatus.h); `active` is false when MSIME gives the focused context up.
  void publishInputStatus(bool active) const {
    msime::linux_host::publish_input_status(
        std::getenv("XDG_RUNTIME_DIR"),
        msime::linux_host::input_status_document(
            active, modeIndicatorLabel(), effectiveScheme()));
  }
  // Called wherever the mode can change; the status area is asked to redraw only when the label actually does.
  void refreshModeIndicator() {
    if (ic_.hasFocus()) publishInputStatus(true);
    auto label = modeIndicatorLabel();
    if (label == mode_indicator_label_) return;
    mode_indicator_label_ = std::move(label);
    ic_.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
  }
  // Fcitx5 sends no event when the lock changes; pressing CapsLock carries the old state and releasing it the new one, so every key event reports it.
  void noteCapsLock(bool caps_lock) {
    caps_lock_ = caps_lock;
    refreshModeIndicator();
  }
  bool restricted() const {
    return ic_.capabilityFlags().testAny(fcitx::CapabilityFlags{
      fcitx::CapabilityFlag::Password, fcitx::CapabilityFlag::Digit,
      fcitx::CapabilityFlag::Number, fcitx::CapabilityFlag::Dialable,
      fcitx::CapabilityFlag::Disable});
  }
  bool privateInput() const {
    return ic_.capabilityFlags().testAny(fcitx::CapabilityFlags{
      fcitx::CapabilityFlag::Sensitive, fcitx::CapabilityFlag::NoSpellCheck});
  }
  bool toggleEnglish() {
    if (!session_) return false;
    const bool enabled = !view_.value("dedicated_english", false);
    view_ = response(msime_client_set_english_mode(session_, enabled));
    render();
    return true;
  }
  // The schemes in the order of the view's scheme index, which is also the order the status action steps through them.
  static constexpr std::array<const char *, 8> kSchemes = {"quanpin", "shuangpin", "wubi", "japanese", "korean",
                                                           "cantonese", "zhuyin", "vietnamese"};
  static_assert(kSchemes.size() == msime::linux_host::kInputSchemeIds.size());
  // Whether a scheme can run with the runtime options this context last read: Cantonese and Zhuyin need their language dictionary (core/InputSchemes.h).
  bool schemeAvailable(const char *id) const {
    return msime::linux_host::input_scheme_available(id, scheme_dictionaries_);
  }
  // The scheme the Engine runs for the preferences, after host-api's fallback from a scheme whose data is missing; the indicator and the status file show this one.
  std::string effectiveScheme() const {
    return msime::linux_host::effective_input_scheme(
        scheme_override_.value_or(preferences_.value("scheme", std::string("quanpin"))),
        preferences_.value("last_chinese_scheme", std::string("quanpin")), scheme_dictionaries_);
  }
  // Works out from the runtime options just read which language dictionaries are installed, once per read rather than per key, and lists Cantonese and Zhuyin in the scheme menu only while theirs is.
  void noteSchemeOptions(const Json &options) {
    scheme_dictionaries_ = msime::linux_host::language_dictionary_availability(options);
    refreshSchemeMenu();
  }
  void refreshSchemeMenu();
  bool cycleScheme() {
    if (!session_ || restricted() || privateInput()) return false;
    const auto current = view_.value("scheme", 0u);
    // Steps past a scheme whose dictionary is missing; quanpin always runs, so the walk ends.
    for (size_t step = 1; step <= kSchemes.size(); ++step) {
      const auto *next = kSchemes[(current + step) % kSchemes.size()];
      if (schemeAvailable(next)) return selectScheme(next);
    }
    return false;
  }
  bool selectScheme(const char *next) {
    if (!session_ || restricted() || privateInput() || !schemeAvailable(next)) return false;
    if (!view_.value("editing_text", std::string{}).empty())
      command(MSIME_FINISH_COMPOSITION);
    // The shared settings page and the IBus host both offer "中文" as a way back to the scheme the user last typed Chinese with. Nothing records it here, so switching to Japanese, Korean or Vietnamese from the status area left that choice with nothing but the quanpin fallback to return to.
    if (msime::linux_host::scheme::IsChinese(msime::linux_host::scheme_number(next)))
      saveStringPreference("last_chinese_scheme", next);
    saveStringPreference("scheme", next);
    waitForPreferenceSave();
    scheme_override_ = next;
    scheme_unsaved_ = unsavedChoice("", "scheme");
    if (std::string(next) != "shuangpin") shuangpin_profile_override_.reset();
    // The helpcode schema is chosen per scheme; carried over, quanpin's choice would replace the one shuangpin keeps in the store. The IBus host clears it on a scheme switch the same way.
    helpcode_schema_override_.reset();
    close();
    if (!ensure()) return false;
    view_ = response(msime_client_focus(session_, true)).at("view");
    render();
    return true;
  }
  bool cycleShuangpinProfile() {
    if (!session_ || view_.value("scheme", 0u) != 1 || restricted() || privateInput())
      return false;
    const auto current = preferences_.value("shuangpin_profile", std::string("xiaohe"));
    auto it = std::find_if(msime::linux_host::kShuangpinProfileNames.begin(),
                           msime::linux_host::kShuangpinProfileNames.end(),
                           [&](const auto &profile) { return current == profile.value; });
    const auto next = it == msime::linux_host::kShuangpinProfileNames.end() ||
                              std::next(it) == msime::linux_host::kShuangpinProfileNames.end()
                          ? msime::linux_host::kShuangpinProfileNames.front()
                          : *std::next(it);
    if (!view_.value("editing_text", std::string{}).empty())
      command(MSIME_FINISH_COMPOSITION);
    saveStringPreference("shuangpin_profile", next.value);
    waitForPreferenceSave();
    // An earlier scheme choice keeps its own unsaved mark: this save replaces its retry but says nothing about whether the store holds shuangpin.
    scheme_override_ = "shuangpin";
    shuangpin_profile_override_ = next.value;
    shuangpin_profile_unsaved_ = unsavedChoice("", "shuangpin_profile");
    close();
    if (!ensure()) return false;
    view_ = response(msime_client_focus(session_, true)).at("view");
    render();
    return true;
  }
  bool cycleHelpcodeSchema() {
    const auto scheme = view_.value("scheme", 0u);
    if (!session_ || (scheme != 0 && scheme != 1) || restricted() || privateInput())
      return false;
    // The size follows the list rather than being written twice: jiajia was added as the sixth
    // schema and the count stayed at five, which stopped this addon compiling at all.
    static constexpr std::array schemas = {"lantian",     "ziranma", "shouyou2_0",
                                           "shouyouplus", "xiaohe",  "jiajia"};
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    const auto current = preferences_.value(section, Json::object()).value(
        "schema", scheme == 1 ? std::string("lantian") : std::string("ziranma"));
    const auto it = std::find(schemas.begin(), schemas.end(), current);
    const auto next = it == schemas.end() || std::next(it) == schemas.end()
        ? schemas.front() : *std::next(it);
    if (!view_.value("editing_text", std::string{}).empty())
      command(MSIME_FINISH_COMPOSITION);
    saveNestedStringPreference(section, "schema", next);
    waitForPreferenceSave();
    helpcode_schema_override_ = next;
    helpcode_schema_unsaved_ = unsavedChoice(section, "schema");
    close();
    if (!ensure()) return false;
    view_ = response(msime_client_focus(session_, true)).at("view");
    render();
    return true;
  }
  bool toggleNineKey() {
    if (!session_ || view_.value("scheme", 0u) != 0) return false;
    const bool enabled = !view_.value("nine_key", false);
    view_ = response(msime_client_set_nine_key_mode(session_, enabled));
    preferences_["touch_keyboard_layout"] = enabled ? "nine_key" : "twenty_six_key";
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["touch_keyboard_layout"] =
          enabled ? "nine_key" : "twenty_six_key";
    saveStringPreference("touch_keyboard_layout", enabled ? "nine_key" : "twenty_six_key");
    render();
    return true;
  }
  bool setCandidatePageSize(uint8_t size) {
    if (!session_ || size < 1 || size > 9 || restricted() || privateInput()) return false;
    if (view_.value("page_size", size_t{}) == size) return true;
    view_ = response(msime_client_set_candidate_page_size(session_, size)).at("view");
    preferences_["candidate_page_size"] = size;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["candidate_page_size"] = size;
    saveNumberPreference("candidate_page_size", size);
    render();
    return true;
  }
  bool chooseNineKeySpelling(size_t index) {
    if (!session_ || view_.value("scheme", 0u) != 0 ||
        !view_.value("nine_key", false) || restricted() || privateInput()) return false;
    const auto spellings = view_.value("nine_key_spellings", Json::array());
    if (!spellings.is_array() || index >= spellings.size() || !spellings.at(index).is_string())
      return false;
    view_ = response(msime_client_choose_nine_key_spelling(
        session_, view_.value("generation", uint64_t{}), index)).at("view");
    render();
    return true;
  }
  bool toggleHelpcode() {
    if (!session_ || (view_.value("scheme", 0u) != 0 && view_.value("scheme", 0u) != 1))
      return false;
    const std::string section = view_.value("scheme", 0u) == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    const bool enabled = !preferences_.value(section, Json::object()).value("enabled", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"][section]["enabled"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference(section.c_str(), "enabled", enabled);
    render();
    return true;
  }
  bool toggleQuanpinAutocorrect(const char *key) {
    if (!session_ || view_.value("scheme", 0u) != 0 || !key || !*key) return false;
    const bool enabled = !preferences_.value("quanpin", Json::object()).value(key, true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["quanpin"][key] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("quanpin", key, enabled);
    render();
    return true;
  }
  bool toggleMixedEnglish() {
    if (!session_) return false;
    const bool enabled = !preferences_.value("mixed_input", Json::object()).value("english", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["mixed_input"]["english"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("mixed_input", "english", enabled);
    render();
    return true;
  }
  bool toggleMixedCandidate(const char *key) {
    if (!session_ || !key || !*key) return false;
    const bool enabled = !preferences_.value("mixed_input", Json::object()).value(key, false);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["mixed_input"][key] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("mixed_input", key, enabled);
    render();
    return true;
  }
  bool toggleLocalMode(const char *key) {
    if (!session_ || !key || !*key || restricted() || privateInput()) return false;
    static constexpr std::array<std::string_view, 11> allowed = {
        "unicode", "date_time", "quick_phrase", "emoji", "kaomoji",
        "super_jianpin", "temporary_english", "temporary_japanese",
        "expression", "command", "mention"};
    if (std::find(allowed.begin(), allowed.end(), key) == allowed.end()) return false;
    const bool enabled = !preferences_.value("local_modes", Json::object())
                              .value(key, msime::linux_host::local_mode_enabled_by_default(key));
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["local_modes"][key] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("local_modes", key, enabled);
    render();
    return true;
  }
  bool toggleEnglishGloss() {
    if (!session_) return false;
    const bool enabled = !preferences_.value("candidate_english_gloss", false);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["candidate_english_gloss"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveBooleanPreference("candidate_english_gloss", enabled);
    translation_query_.clear();
    translation_pending_.clear();
    render();
    return true;
  }
  bool toggleTopLevelBoolean(const char *key, bool fallback = false) {
    if (!session_ || !key || !*key) return false;
    if (preferences_save_job_.valid()) {
      try { preferences_save_job_.get(); } catch (...) {}
      preferences_save_job_ = {};
      if (!options_path_.empty()) {
        try {
          auto latest = response(msime_client_load_preferences(
              reinterpret_cast<const uint8_t *>(options_path_.data()), options_path_.size()));
          if (latest.is_object() && latest.contains("revision") && latest.contains("preferences")) {
            preferences_snapshot_ = latest;
            preferences_ = latest.at("preferences");
            applyContextOverrides(preferences_);
          }
        } catch (...) {}
      }
    }
    const bool enabled = !preferences_.value(key, fallback);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"][key] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveBooleanPreference(key, enabled);
    render();
    return true;
  }
  bool toggleFloatingToolbar() {
    if (!session_ || restricted() || privateInput()) return false;
    const bool enabled = !preferences_.value("floating_toolbar", Json::object())
                              .value("enabled", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["floating_toolbar"]["enabled"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("floating_toolbar", "enabled", enabled);
    refreshToolbar();
    render();
    return true;
  }
  bool toggleVoiceEnabled() {
    if (!session_ || restricted() || privateInput()) return false;
    const bool enabled = !preferences_.value("voice_input", Json::object())
                              .value("enabled", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["voice_input"]["enabled"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    voice_enabled_ = enabled;
    if (!enabled && voice_loading_) cancelVoice();
    syncVoiceAction();
    saveNestedBooleanPreference("voice_input", "enabled", enabled);
    render();
    return true;
  }
  bool toggleClipboardHistory() {
    if (!session_ || restricted() || privateInput()) return false;
    const bool enabled = !preferences_.value("clipboard_history", false);
    if (!toggleTopLevelBoolean("clipboard_history", false)) return false;
    if (!enabled) {
      ++clipboard_generation_;
      clipboard_items_.clear();
      clipboard_loading_ = false;
    }
    return true;
  }
  void refreshToolbar();
  void refreshThemeMenu();
  void syncCandidatePanelFont();
  void syncCandidatePanelTheme();
  void syncVoiceAction();
  // 中英文切换后在光标附近短暂显示「中」或「英」，由 Fcitx5 面板绘制；定义在
  // FcitxEngine 之后，它需要那个类型完整。
  void showInputModeHud();
#ifdef MSIME_FCITX5_MODE_BADGE
  // 徽章显示约 1 秒后自行消失。用事件循环的定时器而不是线程：绘制和销毁都必须回到这条
  // 线程上，Wayland 连接不是线程安全的。
  void scheduleModeBadgeHide() {
    if (!loop_) return;
    mode_badge_timer_ = loop_->addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 1200000, 0,
        [this](fcitx::EventSourceTime *, uint64_t) {
          if (mode_badge_) mode_badge_->hide();
          return true;
        });
  }
#endif
  // Windows re-resolves punctuation on every Chinese/English switch: under the "follow" lock (0) it tracks the mode, and a pinned lock keeps its value. Session-only - the saved chinese_punctuation preference is not rewritten, so the next preference refresh restates it.
  void resyncPunctuationForMode() {
    english_punctuation_ = {};
    english_chinese_punctuation_ = false;
    if (punctuation_lock_ != 0) return;
    chinese_punctuation_ = input_enabled_;
    syncSessionChinesePunctuation();
  }
  bool toggleInputMode() {
    if (!session_ || restricted() || privateInput() || !ic_.hasFocus()) return false;
    input_enabled_ = !input_enabled_;
    ime_mode_chosen_ = true;
    if (!input_enabled_) {
      // 切到英文时上屏的是读入串而不是候选：用户敲了 nihao 再按 Shift，要的就是 nihao
      // 这几个字母，而不是它当前高亮的「你好」。Windows 是这个语义，IBus 宿主也照它写着
      // （见 ClientEngine.cpp 的 toggle_input_mode），这个宿主此前用的是结束组合，于是
      // 同一个手势在两个 Linux 宿主上给出不同的结果。
      if (!view_.value("editing_text", std::string{}).empty()) command(MSIME_COMMIT_RAW);
      resyncPunctuationForMode();
      clearPanel();
    } else {
      resyncPunctuationForMode();
      render();
    }
    // 提示放在面板更新之后：clearPanel()/render() 会刷新输入面板，先弹再刷会把它收掉。
    showInputModeHud();
    return true;
  }
  bool toggleWordCharacter() {
    if (!session_) return false;
    const bool enabled = !preferences_.value("word_character", Json::object()).value("enabled", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["word_character"]["enabled"] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedBooleanPreference("word_character", "enabled", enabled);
    render();
    return true;
  }
  bool toggleWidth() {
    if (!session_) return false;
    const auto width = view_.value("character_width", std::string("Halfwidth"));
    const bool fullwidth = !(width == "Fullwidth" || width == "fullwidth");
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["character_width"] = fullwidth ? "fullwidth" : "halfwidth";
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    view_ = response(msime_client_set_character_width(session_, fullwidth));
    session_fullwidth_ = fullwidth;
    saveStringPreference("character_width", fullwidth ? "fullwidth" : "halfwidth");
    render();
    return true;
  }
  void waitForPreferenceSave() {
    if (!preferences_save_job_.valid()) return;
    bool saved = false;
    try {
      const auto result = preferences_save_job_.get();
      saved = result.is_object() && !result.empty();
    } catch (...) {}
    msime_linux_diagnostic_write(saved ? "menu_save_succeeded" : "menu_save_failed");
    preferences_save_job_ = {};
    if (saved) preferences_save_retry_.reset();
  }
  void startPreferenceSave(PendingPreferenceSave request) {
    waitForPreferenceSave();
    // A store read already in flight predates the choice being saved; applied after the save it would put back what the status bar just changed (the width and punctuation it re-states to the session included), so refreshPreferences() drops it and reads again once the save lands, as refreshProviderSockets() fences a read from a moved store.
    preferences_job_session_ = 0;
    preferences_save_retry_ = request;
    preferences_save_job_ = detachedJob([request = std::move(request)] {
      try { return savePreference(request); }
      catch (...) { return Json::object(); }
    });
  }
  bool retryPreferenceSave() {
    if (preferences_save_job_.valid()) {
      if (preferences_save_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready)
        return false;
      waitForPreferenceSave();
    }
    if (!preferences_save_retry_ || options_path_.empty() || private_)
      return false;
    startPreferenceSave(*preferences_save_retry_);
    return true;
  }
  void saveBooleanPreference(const char *key, bool enabled) {
    if (!key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, {}, key, enabled});
  }
  void saveStringPreference(const char *key, const std::string &value) {
    if (!key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, {}, key, value});
  }
  void saveNumberPreference(const char *key, uint8_t value) {
    if (!key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, {}, key, value});
  }
  void saveNestedBooleanPreference(const char *object, const char *key, bool enabled) {
    if (!object || !*object || !key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, object, key, enabled});
  }
  void saveNestedStringPreference(const char *object, const char *key,
                                 const std::string &value) {
    if (!object || !*object || !key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, object, key, value});
  }
  void saveNestedNumberPreference(const char *object, const char *key, uint8_t value) {
    if (!object || !*object || !key || !*key || options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, object, key, value});
  }
  bool setFrequencyNumber(const char *key, uint8_t value) {
    if (!session_ || restricted() || privateInput() || value < 1 || value > 10)
      return false;
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["frequency"][key] = value;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedNumberPreference("frequency", key, value);
    render();
    return true;
  }
  bool cycleFrequencyNumber(const char *key) {
    const auto current = preferences_.value("frequency", Json::object()).value(key, 1u);
    return setFrequencyNumber(key, static_cast<uint8_t>(current >= 10 ? 1 : current + 1));
  }
  bool cycleFrequencyMode() {
    if (!session_ || restricted() || privateInput()) return false;
    static constexpr std::array<const char *, 5> modes = {
        "disabled", "pin", "halve", "linear", "promote"};
    const auto current = preferences_.value("frequency", Json::object())
                             .value("mode", std::string("promote"));
    const auto it = std::find(modes.begin(), modes.end(), current);
    const auto next = it == modes.end() || std::next(it) == modes.end()
        ? modes.front() : *std::next(it);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["frequency"]["mode"] = next;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveNestedStringPreference("frequency", "mode", next);
    render();
    return true;
  }
  bool toggleChinesePunctuation() {
    if (!session_) return false;
    // A pinned lock holds, as Windows resolves Ctrl+. and the toolbar switch through ResolvePunctuationOpen: the request is consumed, nothing changes and nothing is saved.
    if (punctuation_lock_ != 0) {
      render();
      return true;
    }
    chinese_punctuation_ = !chinese_punctuation_;
    view_ = response(msime_client_set_chinese_punctuation(session_, chinese_punctuation_));
    session_chinese_punctuation_ = chinese_punctuation_;
    preferences_["chinese_punctuation"] = chinese_punctuation_;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["chinese_punctuation"] = chinese_punctuation_;
    saveBooleanPreference("chinese_punctuation", chinese_punctuation_);
    render();
    return true;
  }
  bool togglePairedPunctuation() {
    if (!session_) return false;
    paired_punctuation_ = !paired_punctuation_;
    view_ = response(msime_client_set_paired_punctuation(session_, paired_punctuation_));
    preferences_["paired_punctuation"] = paired_punctuation_;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["paired_punctuation"] = paired_punctuation_;
    saveBooleanPreference("paired_punctuation", paired_punctuation_);
    render();
    return true;
  }
  bool toggleCandidateTranslations() {
    if (!session_) return false;
    const bool enabled = !preferences_.value("candidate_translations", false);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["candidate_translations"] = enabled;
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    if (!enabled && view_.contains("generation")) {
      const auto empty = std::string("[]");
      view_ = response(msime_client_apply_translations(
          session_, view_.at("generation"),
          reinterpret_cast<const uint8_t *>(empty.data()), empty.size())).at("view");
      translation_query_.clear();
      translation_pending_.clear();
    }
    saveBooleanPreference("candidate_translations", enabled);
    render();
    return true;
  }
  bool cyclePunctuationLock() {
    if (!session_) return false;
    punctuation_lock_ = static_cast<uint8_t>((punctuation_lock_ + 1) % 3);
    view_ = response(msime_client_set_punctuation_lock(session_, punctuation_lock_));
    const auto lock = punctuation_lock_ == 1 ? "chinese" : punctuation_lock_ == 2 ? "english" : "follow";
    preferences_["punctuation_lock"] = lock;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["punctuation_lock"] = lock;
    saveStringPreference("punctuation_lock", punctuation_lock_ == 1 ? "chinese" :
                                                     punctuation_lock_ == 2 ? "english" : "follow");
    render();
    return true;
  }
  bool cycleTranslationLanguage() {
    if (!session_) return false;
    static constexpr std::array<const char *, 7> languages = {
        "en", "fr", "ja", "es", "ru", "de", "ko"};
    const auto current = preferences_.value("translation_target_language", std::string("en"));
    auto it = std::find(languages.begin(), languages.end(), current);
    const auto next = it == languages.end() || std::next(it) == languages.end()
        ? languages.front() : *std::next(it);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["translation_target_language"] = next;
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    const auto empty = std::string("[]");
    view_ = response(msime_client_apply_translations(
        session_, view_.at("generation"),
        reinterpret_cast<const uint8_t *>(empty.data()), empty.size())).at("view");
    translation_query_.clear();
    translation_pending_.clear();
    saveStringPreference("translation_target_language", next);
    render();
    return true;
  }
  bool cycleCandidateLayout() {
    if (!session_) return false;
    const auto current = preferences_.value("candidate_layout", std::string("vertical"));
    const std::string next = current == "horizontal" ? "vertical" : "horizontal";
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["candidate_layout"] = next;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveStringPreference("candidate_layout", next);
    render();
    return true;
  }
  bool cycleCandidateTheme() {
    if (!session_ || restricted() || privateInput()) return false;
    static constexpr std::array<const char *, 3> themes = {"follow", "light", "dark"};
    const auto current = preferences_.value("candidate_theme", std::string("follow"));
    const auto it = std::find(themes.begin(), themes.end(), current);
    const auto next = it == themes.end() || std::next(it) == themes.end()
        ? themes.front() : *std::next(it);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["candidate_theme"] = next;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveStringPreference("candidate_theme", next);
    render();
    return true;
  }
  std::vector<msime::linux_host::ThemeChoice> themeChoices() const {
    return msime::linux_host::theme_choices(themeCatalog(), candidate_skin_catalog_);
  }
  std::string currentThemeChoice() const {
    return msime::linux_host::current_theme_choice(preferences_, themeChoices());
  }
  // Choose one 主题 menu entry: the session takes the new theme at once and the store is written behind it, as the other status-bar choices are.
  bool setThemeChoice(const std::string &id) {
    if (!session_ || restricted() || privateInput()) return false;
    const auto choices = themeChoices();
    if (msime::linux_host::current_theme_choice(preferences_, choices) == id) return false;
    const auto change = msime::linux_host::theme_choice_change(choices, id);
    if (!change) return false;
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    // 自定义 while the custom theme is drawn over a listed package changes no preference, and nothing is written.
    const auto before = snapshot["preferences"];
    msime::linux_host::apply_theme_choice(snapshot["preferences"], *change);
    if (snapshot["preferences"] == before) return false;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    if (!options_path_.empty() && !private_)
      startPreferenceSave({options_path_, {}, {}, *change, true});
    syncCandidatePanelTheme();
    render();
    return true;
  }
  bool cycleModeScope() {
    if (!session_) return false;
    const auto current = preferences_.value("ime_mode_scope", std::string("app"));
    const std::string next = current == "global" ? "app" : "global";
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["ime_mode_scope"] = next;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    preferences_snapshot_ = std::move(snapshot);
    saveStringPreference("ime_mode_scope", next);
    if (next == "global") fcitx_global_input_mode = input_enabled_;
    else fcitx_app_input_modes.remember(ic_.program(), input_enabled_);
    render();
    return true;
  }
  bool toggleCloudCandidates() {
    if (!session_) return false;
    const bool enabled = !preferences_.value("cloud_candidates", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["cloud_candidates"] = enabled;
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    if (!enabled && !online_query_.empty()) {
      const auto empty = std::string("[]");
      view_ = response(msime_client_apply_online_candidates(
          session_, reinterpret_cast<const uint8_t *>(online_query_.data()), online_query_.size(),
          reinterpret_cast<const uint8_t *>(empty.data()), empty.size(), 0)).at("view");
      ++online_epoch_;
      online_query_.clear();
      online_slots_[0].query.clear();
      online_slots_[1].query.clear();
    }
    saveBooleanPreference("cloud_candidates", enabled);
    render();
    return true;
  }
  bool toggleAiCandidates() {
    if (!session_) return false;
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("preferences")) return false;
    auto &assistant = snapshot["preferences"]["ai_assistant"];
    const bool enabled = !assistant.value("enabled", false);
    assistant["enabled"] = enabled;
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    if (!enabled && !online_query_.empty()) {
      const auto empty = std::string("[]");
      view_ = response(msime_client_apply_online_candidates(
          session_, reinterpret_cast<const uint8_t *>(online_query_.data()), online_query_.size(),
          reinterpret_cast<const uint8_t *>(empty.data()), empty.size(), 1)).at("view");
      ++online_epoch_;
      online_query_.clear();
      online_slots_[0].query.clear();
      online_slots_[1].query.clear();
    }
    saveNestedBooleanPreference("ai_assistant", "enabled", enabled);
    render();
    return true;
  }
  bool selectEdge(uint8_t edge) {
    if (!session_ || view_.value("candidates", Json::array()).empty()) return false;
    for (const auto &candidate : view_.at("candidates")) {
      if (!candidate.value("highlighted", false)) continue;
      const auto &id = candidate.at("id");
      return apply(msime_client_select_edge(session_, id.at("generation"),
                                             id.at("index"), edge));
    }
    return false;
  }
  void maintenance(int operation);
  bool reloadService();
  void rememberInputMode() {
    if (preferences_.value("ime_mode_scope", std::string("app")) == "global")
      fcitx_global_input_mode = input_enabled_;
    else
      fcitx_app_input_modes.remember(ic_.program(), input_enabled_);
  }
  void restoreInputMode() {
    const auto fallback = preferences_.value("default_ime_mode", "chinese") != "english";
    if (preferences_.value("ime_mode_scope", std::string("app")) == "global")
      input_enabled_ = fcitx_global_input_mode.value_or(fallback);
    else
      input_enabled_ = fcitx_app_input_modes.restore(ic_.program(), fallback);
    ime_mode_chosen_ = true;
    mode_restore_pending_ = false;
  }
  void applyContextOverrides(Json &preferences) const {
    if (scheme_override_) preferences["scheme"] = *scheme_override_;
    if (shuangpin_profile_override_) preferences["shuangpin_profile"] = *shuangpin_profile_override_;
    if (helpcode_schema_override_) {
      const auto section = preferences.value("scheme", std::string("quanpin")) == "shuangpin"
          ? "shuangpin_helpcode" : "quanpin_helpcode";
      preferences[section]["schema"] = *helpcode_schema_override_;
    }
  }
  // A status-bar save that has not landed: its retry is still pending for this very key.
  bool unsavedChoice(const char *section, const char *key) const {
    return preferences_save_retry_ && preferences_save_retry_->section == section &&
           preferences_save_retry_->key == key;
  }
  // The scheme, shuangpin and helpcode overrides bridge one gap: status-bar saves reach the preference store, but sessions are built from the runtime options file, which only the settings page rewrites. They must not outrank the store: once it holds a different value - the settings page or another window changed it - the override is dropped. A choice whose own save failed stays until the store holds it, as the IBus host keeps a failed menu choice, so the menu does not jump back; that mark belongs to the choice rather than to the single retry slot, which any later save replaces. Returns whether anything was dropped.
  bool expireContextOverrides(const Json &stored) {
    if (!stored.is_object()) return false;
    bool dropped = false;
    const auto expire = [&](std::optional<std::string> &choice, bool &unsaved, const Json &source,
                            const char *key) {
      if (!choice || !source.is_object() || !source.contains(key) || !source.at(key).is_string())
        return;
      if (source.at(key).get<std::string>() == *choice) {
        unsaved = false;
        return;
      }
      if (unsaved) return;
      choice.reset();
      dropped = true;
    };
    expire(scheme_override_, scheme_unsaved_, stored, "scheme");
    expire(shuangpin_profile_override_, shuangpin_profile_unsaved_, stored, "shuangpin_profile");
    // Checked against the section it would be written to, after the scheme above has settled, as applyContextOverrides picks it.
    const auto scheme = scheme_override_.value_or(stored.value("scheme", std::string("quanpin")));
    const auto section = scheme == "shuangpin" ? "shuangpin_helpcode" : "quanpin_helpcode";
    expire(helpcode_schema_override_, helpcode_schema_unsaved_, stored.value(section, Json::object()), "schema");
    return dropped;
  }
  // Every caller hands the result to the session. Store revisions belong to the
  // store: the same revision can carry two different documents once this host
  // edits one for a menu toggle, and the runtime rejects that as a conflicting
  // revision. The throw then reaches the action's catch, which closes the
  // session - so toggling learning, translations or AI from the status area
  // dropped the input method instead of changing a setting. Give the session
  // its own increasing revision, the way the IBus host does.
  Json effectiveContextSnapshot(Json snapshot) {
    if (snapshot.is_object() && snapshot.contains("preferences")) {
      auto preferences = snapshot.at("preferences");
      applyContextOverrides(preferences);
      snapshot["preferences"] = std::move(preferences);
      snapshot["revision"] = ++applied_preferences_revision_;
    }
    return snapshot;
  }
  // The runtime keeps the punctuation toggle as an override that outranks the
  // preferences it is handed, so a preference that moved the effective value
  // has to be re-stated or the session keeps converting with whatever the last
  // toggle left behind.
  // The settings page carries a diagnostic_log switch that this host did not
  // read, so turning it on logged the IBus session and nothing here. The sink
  // takes event labels only - never keys, text, candidates, paths or provider
  // replies - and stays closed unless the preference asks for it.
  void configureDiagnostics() const {
    const auto diagnostic = preferences_.value("diagnostic_log", Json::object());
    msime_linux_diagnostic_configure(
        options_path_, diagnostic.is_object() && diagnostic.value("server", false));
  }
  // "在候选窗中显示辅助码" and the wubi code hint both decide whether the
  // annotation belongs on the candidate row. This host appended it
  // unconditionally, so turning either off changed nothing here.
  bool showCandidateAnnotations() const {
    const auto scheme = view_.value("scheme", 0u);
    if (scheme == 2) return preferences_.value("wubi_code_hint", true);
    if (scheme != 0 && scheme != 1) return true;
    const std::string section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    return preferences_.value(section, Json::object())
        .value("show_in_candidate_window",
               msime::linux_host::default_show_helpcode(
                   scheme == 1 ? "shuangpin" : "quanpin"));
  }
  void syncSessionChinesePunctuation() {
    if (!session_ || session_chinese_punctuation_ == chinese_punctuation_) return;
    view_ = response(msime_client_set_chinese_punctuation(session_, chinese_punctuation_));
    session_chinese_punctuation_ = chinese_punctuation_;
  }
  // The runtime takes its width only from set_character_width, never from the preferences it is handed, and fullwidthOutput() reads it back from the view, so the saved character_width reaches this host only when it is stated here: when the session opens and whenever a reload moves it.
  void syncSessionCharacterWidth() {
    const bool fullwidth =
        preferences_.value("character_width", std::string("halfwidth")) == "fullwidth";
    if (!session_ || session_fullwidth_ == fullwidth) return;
    view_ = response(msime_client_set_character_width(session_, fullwidth));
    session_fullwidth_ = fullwidth;
    paired_tracker_.clear();
  }
  bool applyPreferenceSnapshot(Json snapshot) {
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    configureDiagnostics();
    chinese_punctuation_ = preferences_.value("chinese_punctuation", chinese_punctuation_);
    syncSessionChinesePunctuation();
    preferences_snapshot_ = std::move(snapshot);
    msime_linux_diagnostic_write("preferences_applied");
    return true;
  }
  bool ensure() {
    if (!ic_.hasFocus() || restricted()) { close(); clearPanel(); return false; }
    if (session_ && private_ != privateInput()) { close(); clearPanel(); }
    if (session_) return true;
    auto options = readOptions();
    dictionary_user_data_ = options.value("user_data", std::string{});
    // Dictionary maintenance is running from the settings window; keys go to the application until it is done.
    if (msime::linux_host::dictionary_quiesced(dictionary_user_data_)) return false;
    candidate_skin_catalog_ = parseCandidateSkinCatalog(options);
    candidate_skin_document_ = options.value("candidate_skin_catalog", Json());
    refreshThemeMenu();
    noteSchemeOptions(options);
    // The skin catalogue is for this host's own menu; the Host API rejects an
    // options document carrying a field it does not know, so leaving it in
    // means no session can ever open on a deployment that installed skins.
    options.erase("candidate_skin_catalog");
    private_ = privateInput();
    preferences_ = options.value("preferences", Json::object());
    applyContextOverrides(preferences_);
    traditional_ = preferences_.value("traditional_chinese_output", false);
    chinese_punctuation_ = preferences_.value("chinese_punctuation", true);
    paired_punctuation_ = preferences_.value("paired_punctuation", true);
    smart_punctuation_ = preferences_.value("smart_punctuation", true);
    smart_punctuation_repeat_ = preferences_.value("smart_punctuation_repeat", true);
    smart_punctuation_space_convert_ =
        preferences_.value("smart_punctuation_space_convert", false);
    const auto keybindings = preferences_.value("keybindings", Json::object());
    mode_shift_enabled_ = keybindings.value("switch_language_shift", true);
    mode_ctrl_enabled_ = keybindings.value("switch_language_ctrl", false);
    mode_ctrl_alt_space_enabled_ =
        keybindings.value("switch_language_ctrl_alt_space", true);
    character_set_shortcut_enabled_ =
        keybindings.value("toggle_character_set_ctrl_shift_f", true);
    // Windows starts a new context in Chinese unless the user said otherwise.
    // Applied once per input context, not once per session: refocusing or
    // rebuilding the Engine session must keep the mode the user chose rather than
    // putting the startup default back.
    bool restore_changed_mode = false;
    if (!ime_mode_chosen_) {
      if (mode_restore_pending_) {
        const bool before = input_enabled_;
        restoreInputMode();
        restore_changed_mode = input_enabled_ != before;
      } else {
        input_enabled_ = preferences_.value("default_ime_mode", "chinese") != "english";
        ime_mode_chosen_ = true;
      }
    }
    if (!smart_punctuation_ || !smart_punctuation_space_convert_ ||
        !chinese_punctuation_) {
      space_convert_mark_.clear();
      space_convert_preceding_.clear();
    }
    if (!smart_punctuation_ || !smart_punctuation_repeat_)
      forgetSmartPunctuationRepeat();
    const auto punctuationLock = preferences_.value("punctuation_lock", std::string("follow"));
    punctuation_lock_ = punctuationLock == "chinese" ? 1 : punctuationLock == "english" ? 2 : 0;
    navigation_ = preferences_.value("navigation", Json::object());
    const auto wordCharacter = preferences_.value("word_character", Json::object());
    word_character_enabled_ = wordCharacter.value("enabled", true);
    word_character_minus_equal_ = wordCharacter.value("keys", std::string("brackets")) == "minus_equal";
    options_path_ = options.value("preferences_directory", std::string());
    // The failed save kept across the focus change belongs to its store; once the runtime options point elsewhere it is not retried there, as refreshProviderSockets does while focused.
    if (preferences_save_retry_ && preferences_save_retry_->directory != options_path_)
      preferences_save_retry_.reset();
    if (!options_path_.empty()) {
      try {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(options_path_.data()), options_path_.size()));
        if (snapshot.is_object() && snapshot.contains("revision") && snapshot.contains("preferences")) {
          preferences_snapshot_ = std::move(snapshot);
          // Status-bar saves reach the store but never the runtime options file, so for the choices the status bar makes the store is the authority: the file can hold a value no window has chosen since, e.g. after another window's status bar or the settings page moved the store while this context had no session.
          const auto &stored = preferences_snapshot_.at("preferences");
          auto base = options.value("preferences", Json::object());
          for (const auto *key : {"scheme", "shuangpin_profile", "character_width"})
            if (stored.contains(key) && stored.at(key).is_string()) base[key] = stored.at(key);
          for (const auto *section : {"quanpin_helpcode", "shuangpin_helpcode"})
            if (stored.contains(section) && stored.at(section).is_object() &&
                stored.at(section).contains("schema") && stored.at(section).at("schema").is_string())
              base[section]["schema"] = stored.at(section).at("schema");
          expireContextOverrides(stored);
          preferences_ = std::move(base);
          applyContextOverrides(preferences_);
        }
      } catch (...) {
        // The prepared options remain usable for composition; preference actions will retry
        // through the normal save/reload path when the store becomes available.
      }
    }
    // Only now is options_path_ this session's store: configured any earlier, the sink saw the empty path close() left and stayed shut whatever the switch said.
    configureDiagnostics();
    resources_ = options.value("resources", std::string());
    auto clipboard_path = options.value("clipboard_history_path", std::string());
    if (!clipboard_path.empty() &&
        std::filesystem::path(clipboard_path).filename() == "clipboard_history.json")
      clipboard_path = std::filesystem::path(clipboard_path).parent_path().string();
    if (clipboard_path.empty()) clipboard_path = options.value("preferences_directory", std::string());
    if (clipboard_path != clipboard_path_) {
      ++clipboard_generation_;
      clipboard_items_.clear();
      clipboard_loading_ = false;
    }
    clipboard_path_ = std::move(clipboard_path);
    auto cloud_clipboard_socket = cloudClipboardSocket(options);
    if (cloud_clipboard_socket != cloud_clipboard_socket_) {
      ++cloud_clipboard_generation_;
      cloud_clipboard_items_.clear();
      cloud_clipboard_enabled_ = true;
    }
    cloud_clipboard_socket_ = std::move(cloud_clipboard_socket);
    voice_socket_ = providerSocket(options, "voice_provider_socket",
                                   "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
    const auto voicePreferences = preferences_.value("voice_input", Json::object());
    voice_enabled_ = voicePreferences.value("enabled", true);
    voice_hotkey_ctrl_f9_ = voicePreferences.value("hotkey_ctrl_f9", true);
    voice_hotkey_ralt_ = voicePreferences.value("hotkey_ralt", true);
    voice_hotkey_ctrl_win_ = voicePreferences.value("hotkey_ctrl_win", false);
    voice_hotkey_rctrl_ralt_ = voicePreferences.value("hotkey_rctrl_ralt", false);
    voice_hotkey_hold_space_lock_ =
        voicePreferences.value("hotkey_hold_space_lock", voice_hotkey_hold_space_lock_);
    voice_language_ = voicePreferences.value("language", std::string("zh-cn"));
    loadVoiceOptions();
    syncVoiceOverlayTheme();
    online_socket_ = onlineSocket(options);
    translation_socket_ = translationSocket(options);
    if (private_) {
      preferences_["learning"] = false;
      preferences_["cloud_candidates"] = false;
      preferences_["ai_assistant"]["enabled"] = false;
    }
    // 会话按本上下文实际生效的那份偏好建立，而不是文件里的原样。此前这一行只在私密
    // 上下文里执行，于是 applyContextOverrides 写进 preferences_ 的那几个 override
    // ——方案、皮肤、双拼方案案、辅助码方案——都进不了 Engine：状态栏切到双拼，偏好
    // 存下了，新建的会话却仍按文件里的全拼跑，再切一次又从全拼算下一格，于是循环卡在
    // 第一格，用户永远到不了五笔和日文。私密上下文是唯一没中招的，只是因为它顺手把同
    // 一份 preferences_ 回填了。
    options["preferences"] = preferences_;
    syncCandidatePanelFont();
    syncCandidatePanelTheme();
    // This front end draws view.phrase_prefix ahead of the reading, so a phrase assembled out of
    // several selections stays in the composition instead of reaching the document one piece at a
    // time. Requesting it and drawing it are one decision; see core/PhrasePreedit.h.
    options["phrase_preedit"] = true;
    // The built-in sound packs of this installation, unless the runtime options name others. The Host API's own fallback looks beside the configured resource directory, which is no longer the installed one once the user has downloaded a newer dictionary into their own data directory.
    if (std::error_code error; !options.contains("sound_packs") &&
                               std::filesystem::is_directory(MSIME_SOUND_PACKS, error))
      options["sound_packs"] = MSIME_SOUND_PACKS;
    // On-device recognition reads the user's dictionary words as hotwords with the same options; see requestVoice.
    voice_host_options_ = options;
    const auto document = options.dump();
    view_ = response(msime_client_create(reinterpret_cast<const uint8_t *>(document.data()), document.size()));
    session_ = view_.at("session").get<uint64_t>();
    applied_preferences_revision_ = 0;
    msime_linux_diagnostic_write("focus_in");
    session_chinese_punctuation_ =
        options.at("preferences").value("chinese_punctuation", true);
    syncSessionChinesePunctuation();
    session_fullwidth_ = false;
    syncSessionCharacterWidth();
    // A mode the focus restores is a Chinese/English switch like any other; resolved here, once the lock is read and the session exists.
    if (restore_changed_mode) resyncPunctuationForMode();
    view_ = response(msime_client_focus(session_, true)).at("view");
    return true;
  }
  void refreshPreferences() {
    try {
      if (preferences_save_job_.valid()) {
        if (preferences_save_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        waitForPreferenceSave();
      }
      if (preferences_job_.valid()) {
        if (preferences_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto snapshot = preferences_job_.get();
        preferences_job_ = {};
        if (session_ && session_ == preferences_job_session_ && ic_.hasFocus() &&
            !restricted() && private_ == privateInput() && !snapshot.is_null()) {
          if (private_) {
            snapshot["preferences"]["learning"] = false;
            snapshot["preferences"]["cloud_candidates"] = false;
            snapshot["preferences"]["ai_assistant"]["enabled"] = false;
          }
          if (snapshot != preferences_snapshot_) {
            // A status-bar choice the store no longer agrees with gives way to it.
            expireContextOverrides(snapshot.at("preferences"));
            // Same document the menu toggles send, so it carries the session's
            // own revision too; mixing store revisions with those would make
            // the next toggle look stale.
            auto effective = effectiveContextSnapshot(snapshot);
            auto effectivePreferences = effective.at("preferences");
            const auto encoded = effective.dump();
            view_ = response(msime_client_update_preferences(session_,
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
            preferences_ = std::move(effectivePreferences);
            configureDiagnostics();
            // A width chosen here that the store does not hold - its save failed, or a private window, which never saves - is not undone by the store, as a failed scheme choice is kept; the next session re-reads the store.
            if (!private_ && !unsavedChoice("", "character_width")) syncSessionCharacterWidth();
            traditional_ = preferences_.value("traditional_chinese_output", traditional_);
            chinese_punctuation_ = preferences_.value("chinese_punctuation", chinese_punctuation_);
            syncSessionChinesePunctuation();
            paired_punctuation_ = preferences_.value("paired_punctuation", paired_punctuation_);
            smart_punctuation_ = preferences_.value("smart_punctuation", smart_punctuation_);
            smart_punctuation_repeat_ = preferences_.value(
                "smart_punctuation_repeat", smart_punctuation_repeat_);
            smart_punctuation_space_convert_ = preferences_.value(
                "smart_punctuation_space_convert", smart_punctuation_space_convert_);
            const auto reloaded = preferences_.value("keybindings", Json::object());
            mode_shift_enabled_ = reloaded.value("switch_language_shift", mode_shift_enabled_);
            mode_ctrl_enabled_ = reloaded.value("switch_language_ctrl", mode_ctrl_enabled_);
            mode_ctrl_alt_space_enabled_ = reloaded.value(
                "switch_language_ctrl_alt_space", mode_ctrl_alt_space_enabled_);
            character_set_shortcut_enabled_ = reloaded.value(
                "toggle_character_set_ctrl_shift_f", character_set_shortcut_enabled_);
            // The toolbar follows the reloaded switches without waiting for the
            // next focus change, the way the IBus property menu does.
            refreshToolbar();
            syncCandidatePanelFont();
            syncCandidatePanelTheme();
            const auto punctuationLock = preferences_.value("punctuation_lock", std::string("follow"));
            punctuation_lock_ = punctuationLock == "chinese" ? 1 : punctuationLock == "english" ? 2 : 0;
            navigation_ = preferences_.value("navigation", Json::object());
            const auto wordCharacter = preferences_.value("word_character", Json::object());
            word_character_enabled_ = wordCharacter.value("enabled", true);
            word_character_minus_equal_ = wordCharacter.value("keys", std::string("brackets")) == "minus_equal";
            const auto voicePreferences = preferences_.value("voice_input", Json::object());
            voice_enabled_ = voicePreferences.value("enabled", voice_enabled_);
            voice_hotkey_ctrl_f9_ = voicePreferences.value("hotkey_ctrl_f9", voice_hotkey_ctrl_f9_);
            voice_hotkey_ralt_ = voicePreferences.value("hotkey_ralt", voice_hotkey_ralt_);
            voice_hotkey_ctrl_win_ = voicePreferences.value("hotkey_ctrl_win", voice_hotkey_ctrl_win_);
            voice_hotkey_rctrl_ralt_ = voicePreferences.value("hotkey_rctrl_ralt", voice_hotkey_rctrl_ralt_);
            voice_hotkey_hold_space_lock_ =
                voicePreferences.value("hotkey_hold_space_lock", voice_hotkey_hold_space_lock_);
            voice_language_ = voicePreferences.value("language", voice_language_);
            loadVoiceOptions();
            syncVoiceOverlayTheme();
            syncVoiceAction();
            preferences_snapshot_ = std::move(snapshot);
            render();
          }
        }
      }
      if (!session_ || options_path_.empty() || !ic_.hasFocus() || restricted()) return;
      preferences_job_session_ = session_;
      preferences_job_ = detachedJob([directory = options_path_] {
        fcitx_typing_statistics.refresh(directory);
        return response(msime_client_try_load_preferences(
            reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
      });
    } catch (...) {
      // Keep the active settings on malformed or concurrently written files.
    }
  }
  void refreshProviderSockets() {
    try {
      if (!session_ || !ic_.hasFocus() || restricted()) return;
      const auto options = readOptions();
      const auto nextPreferencesDirectory =
          options.value("preferences_directory", std::string{});
      if (nextPreferencesDirectory != options_path_) {
        // A runtime-options switch can move the shared store while this
        // context stays focused. Invalidate both an in-flight read and any
        // retry belonging to the old store; refreshPreferences() will queue
        // a fresh read from the new directory on its next tick.
        options_path_ = nextPreferencesDirectory;
        preferences_job_session_ = 0;
        preferences_snapshot_ = Json();
        preferences_save_retry_.reset();
      }
      candidate_skin_catalog_ = parseCandidateSkinCatalog(options);
      candidate_skin_document_ = options.value("candidate_skin_catalog", Json());
      refreshThemeMenu();
      noteSchemeOptions(options);
      syncCandidatePanelTheme();
      syncVoiceOverlayTheme();
      // Runtime options can move the shared clipboard history while this
      // input context remains focused. Keep the same path precedence as the
      // initial session setup and fence an in-flight read from the old file.
      auto nextClipboard = options.value("clipboard_history_path", std::string{});
      if (!nextClipboard.empty() &&
          std::filesystem::path(nextClipboard).filename() == "clipboard_history.json")
        nextClipboard = std::filesystem::path(nextClipboard).parent_path().string();
      if (nextClipboard.empty())
        nextClipboard = options.value("preferences_directory", std::string{});
      if (nextClipboard != clipboard_path_) {
        ++clipboard_generation_;
        clipboard_items_.clear();
        clipboard_loading_ = false;
        clipboard_path_ = std::move(nextClipboard);
      }
      auto nextCloudClipboard = cloudClipboardSocket(options);
      if (nextCloudClipboard != cloud_clipboard_socket_) {
        ++cloud_clipboard_generation_;
        cloud_clipboard_items_.clear();
        cloud_clipboard_enabled_ = true;
        cloud_clipboard_socket_ = std::move(nextCloudClipboard);
      }
      auto nextVoice = providerSocket(options, "voice_provider_socket",
                                      "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
      if (nextVoice != voice_socket_) {
        if (voice_loading_) cancelVoice();
        voice_socket_ = std::move(nextVoice);
      }
      const auto nextOnline = onlineSocket(options);
      if (nextOnline != online_socket_) {
        online_socket_ = nextOnline;
        ++online_epoch_;
        online_query_.clear();
        for (auto &slot : online_slots_) slot.query.clear();
      }
      auto nextTranslation = translationSocket(options);
      if (nextTranslation != translation_socket_) {
        translation_socket_ = std::move(nextTranslation);
        translation_query_.clear();
        translation_pending_.clear();
      }
    } catch (...) {
      // Keep the active provider endpoints when options are being atomically replaced.
    }
  }
  void refreshOnline() {
    try {
      for (uint8_t source = 0; source < 2; ++source) {
        auto &slot = online_slots_[source];
        if (!slot.job.valid() || slot.job.wait_for(std::chrono::seconds(0)) != std::future_status::ready)
          continue;
        auto result = slot.job.get();
        slot.job = {};
        if (session_ && session_ == online_job_session_ && slot.epoch == online_epoch_ &&
            !privateInput() && ic_.hasFocus() && result.is_object() &&
            result.value("query", "") == slot.query) {
          Json candidates = Json::array();
          for (const auto &item : result.value("candidates", Json::array())) {
            if (!item.is_object() || item.value("text", std::string{}).empty()) continue;
            if (item.value("source", 255u) == source)
              candidates.push_back(item.at("text"));
          }
          if (!candidates.empty()) {
            const auto encoded = candidates.dump();
            view_ = response(msime_client_apply_online_candidates(
                session_, reinterpret_cast<const uint8_t *>(slot.query.data()), slot.query.size(),
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(), source)).at("view");
            render();
          }
        }
      }
      if (!session_ || online_socket_.empty() || privateInput() || !ic_.hasFocus() || restricted()) return;
      const auto query = response(msime_client_online_query(session_));
      if (!query.is_object()) return;
      const bool cloud = query.value("cloud_eligible", false) &&
                         query.value("cloud_candidates", true);
      const auto aiConfig = query.value("ai_assistant", Json::object());
      const bool ai = query.value("ai_eligible", false) &&
                      aiConfig.is_object() && aiConfig.value("enabled", false);
      if (!cloud && !ai) return;
      const auto encoded = query.dump();
      const auto now = std::chrono::steady_clock::now();
      const bool changed = encoded != online_query_;
      if (changed) {
        // Match Windows cloud_ime's 500ms and ai_assistant's 650ms idle delays.
        online_query_ = encoded;
        online_due_ = now + std::chrono::milliseconds(500);
        ai_due_ = now + std::chrono::milliseconds(650);
      }
      for (uint8_t source = 0; source < 2; ++source) {
        const bool enabled = source == 0 ? cloud : ai;
        auto &slot = online_slots_[source];
        if (!enabled || slot.job.valid()) continue;
        // Windows AiAssistant shows a cached answer as soon as the input changes; only the network request waits for the idle delay. The probe never leaves the provider.
        const bool cacheOnly = source == 1 && changed;
        if (!cacheOnly && now < (source == 0 ? online_due_ : ai_due_)) continue;
        auto providerQuery = query;
        if (source == 0) {
          providerQuery.erase("ai_assistant");
          providerQuery.erase("ai_context");
        } else {
          providerQuery["cloud_candidates"] = false;
          if (cacheOnly) providerQuery["ai_cache_only"] = true;
        }
        const auto providerEncoded = providerQuery.dump();
        online_job_session_ = session_;
        slot.query = encoded;
        slot.epoch = online_epoch_;
        slot.job = detachedJob(
            [providerEncoded, encoded, socket = online_socket_] {
              auto raw = response(msime_client_online_provider_request(
                  reinterpret_cast<const uint8_t *>(providerEncoded.data()), providerEncoded.size(),
                  reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
              Json result = raw.is_object() ? raw : Json::object();
              result["query"] = encoded;
              return result;
            });
      }
    } catch (...) {
      online_query_.clear();
    }
  }
  // A non-English target whose offline dictionary is installed. The user's own translator outranks that dictionary, so after it answers the provider is asked about every candidate and its answers replace the dictionary's (prefer_online_glosses).
  static bool offlineDictionary(const Json &query) {
    const auto target = query.value("target_language", std::string{});
    const auto installed = query.value("offline_gloss_languages", Json::array());
    return target != "en" && installed.is_array() &&
           std::find(installed.begin(), installed.end(), target) != installed.end();
  }
  static Json preferOnline(const Json &glosses, const Json &online) {
    std::vector<std::pair<std::string, std::string>> merged, answers;
    merged.reserve(glosses.is_array() ? glosses.size() : 0);
    answers.reserve(online.is_array() ? online.size() : 0);
    const auto read = [](const Json &values, auto &into) {
      if (!values.is_array()) return;
      for (const auto &item : values)
        if (item.is_object())
          into.emplace_back(item.value("text", std::string{}),
                            item.value("translation", std::string{}));
    };
    read(glosses, merged);
    read(online, answers);
    msime::linux_host::prefer_online_glosses(merged, answers);
    auto result = Json::array();
    for (const auto &[text, translation] : merged)
      result.push_back({{"text", text}, {"translation", translation}});
    return result;
  }
  void startTranslation(const Json &query, bool offline, Json local = Json::array(),
                        bool manual_sentence = false) {
    const auto encoded = query.dump();
    translation_query_ = encoded;
    translation_manual_sentence_ = manual_sentence;
    translation_session_ = session_;
    auto candidates = Json::array();
    for (const auto &candidate : view_.at("candidates"))
      candidates.push_back({{"text", candidate.at("text")}, {"source", candidate.at("source")}});
    const bool dictionary = offlineDictionary(query);
    auto glossRequest = Json{{"generation", query.at("generation")},
                             {"user_data", query.value("user_data", Json())},
                             {"candidates", candidates}};
    if (dictionary) glossRequest["target_language"] = query.at("target_language");
    const auto gloss = glossRequest.dump();
    const auto socket = (manual_sentence || commandTranslation(query) ||
                         preferences_.value("candidate_translations", false))
                            ? translation_socket_ : std::string{};
    translation_job_ = detachedJob(
        [query, encoded, gloss, offline, local, socket, dictionary, resources = resources_] () mutable {
          if (offline) {
            try {
              local = response(msime_client_candidate_gloss_request(
                  reinterpret_cast<const uint8_t *>(gloss.data()), gloss.size(),
                  reinterpret_cast<const uint8_t *>(resources.data()), resources.size()))
                  .value("translations", Json::array());
            } catch (...) {} // A missing local dictionary must not prevent online fallback.
          } else if (!socket.empty()) {
            auto transport = query;
            auto missing = Json::array();
            for (const auto &candidate : query.at("candidates")) {
              const auto &text = candidate.at("text");
              if (dictionary || std::none_of(local.begin(), local.end(), [&](const Json &item) {
                    return item.at("text") == text;
                  })) missing.push_back(text);
            }
            transport["candidates"] = std::move(missing);
            if (!transport.at("candidates").empty()) {
              const auto request = transport.dump();
              try {
                auto result = response(msime_client_translation_provider_request(
                    reinterpret_cast<const uint8_t *>(request.data()), request.size(),
                    reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
                if (result.is_object() && dictionary) {
                  local = preferOnline(local, result.value("translations", Json::array()));
                } else if (result.is_object()) {
                  for (const auto &item : result.value("translations", Json::array()))
                    local.push_back(item);
                  const auto userData = query.value("user_data", std::string{});
                  const auto target = query.value("target_language", std::string{});
                  if (target == "en" && !userData.empty()) {
                    auto translations = result.value("translations", Json::array());
                    if (translations.is_array() && translations.size() > 9)
                      translations.erase(translations.begin() + 9, translations.end());
                    const auto save = Json{{"target_language", target},
                                           {"translations", std::move(translations)}}.dump();
                    msime_client_string_free(msime_client_translation_gloss_save(
                        reinterpret_cast<const uint8_t *>(save.data()), save.size(),
                        reinterpret_cast<const uint8_t *>(userData.data()), userData.size()));
                  }
                }
              } catch (...) {} // Retain local hits on provider failure.
            }
          }
          return Json{{"query", encoded}, {"translations", local},
                      {"continue_online", offline && !socket.empty()},
                      {"_socket", socket}};
        });
  }
  void translateSentence() {
    constexpr size_t kMaxSentenceChars = 512;
    if (!session_ || !ic_.hasFocus() || restricted() || privateInput() ||
        translation_socket_.empty() || translation_job_.valid())
      return;
    try {
      auto query = response(msime_client_translation_query(session_));
      const auto candidates = view_.value("candidates", Json::array());
      if (!query.is_object() || !candidates.is_array() || candidates.empty()) return;
      const Json *selected = &candidates.front();
      for (const auto &candidate : candidates)
        if (candidate.value("highlighted", false)) { selected = &candidate; break; }
      const auto text = selected->value("text", std::string{});
      if (text.empty() || msime::linux_host::utf8_scalar_count(text) > kMaxSentenceChars)
        return;
      query["sentence"] = true;
      query["target_language"] = preferences_.value("translation_target_language", std::string("en"));
      query["candidates"] = Json::array({text});
      translation_pending_.clear();
      startTranslation(query, false, Json::array(), true);
    } catch (...) {}
  }
  void refreshTranslations() {
    try {
      const bool allowed = session_ && ic_.hasFocus() && !restricted() && !privateInput();
      const auto query = allowed ? response(msime_client_translation_query(session_)) : Json();
      const auto encodedQuery = query.is_object() ? query.dump() : std::string{};
      if (translation_job_.valid()) {
        if (translation_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto result = translation_job_.get();
        translation_job_ = {};
        bool manual_query_matches = false;
        if (translation_manual_sentence_ && result.is_object()) {
          try {
            manual_query_matches = Json::parse(result.value("query", "{}"))
                                       .value("generation", uint64_t{0}) ==
                                   query.value("generation", uint64_t{0});
          } catch (...) {}
        }
        if (allowed && session_ == translation_session_ && query.is_object() &&
            result.is_object() &&
            (result.value("query", "") == encodedQuery || manual_query_matches) &&
            result.value("_socket", std::string{}) == translation_socket_) {
          const auto encoded = result.value("translations", Json::array()).dump();
          view_ = response(msime_client_apply_translations(
              session_, query.at("generation"), reinterpret_cast<const uint8_t *>(encoded.data()),
              encoded.size())).at("view");
          render();
          if (result.value("continue_online", false)) {
            startTranslation(query, false, result.at("translations"));
            return;
          }
          if (translation_manual_sentence_)
            translation_query_ = encodedQuery;
          translation_manual_sentence_ = false;
        }
      }
      if (!query.is_object()) { translation_pending_.clear(); return; }
      if (encodedQuery == translation_query_) return;
      if (encodedQuery != translation_pending_) {
        translation_pending_ = encodedQuery;
        translation_due_ = std::chrono::steady_clock::now() + std::chrono::milliseconds(500);
        return;
      }
      if (std::chrono::steady_clock::now() < translation_due_) return;
      startTranslation(query, !commandTranslation(query) &&
                                  (query.value("english_gloss", false) || offlineDictionary(query)));
    } catch (...) { /* Never expose candidate text or provider credentials in errors. */ }
  }
  // /fy asks the selected service alone, in the query's own target language, and answers with a row rather than a gloss (CandidateTranslationPolicy.h): no offline dictionary, no other service, no gloss cache.
  bool commandTranslation(const Json &query) const {
    return query.is_object() &&
           msime::linux_host::command_translation_query(
               view_.value("local_mode", std::string("none")), query.value("sentence", false));
  }
  void refreshClipboard() {
    try {
      if (clipboard_mutation_job_.valid()) {
        if (clipboard_mutation_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        clipboard_mutation_job_.get();
        clipboard_mutation_job_ = {};
        clipboard_items_.clear();
        clipboard_loading_ = false;
      }
      if (clipboard_job_.valid()) {
        if (clipboard_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto result = clipboard_job_.get();
        clipboard_job_ = {};
        clipboard_loading_ = false;
        if (preferences_.value("clipboard_history", false) && session_ && ic_.hasFocus() &&
            !restricted() && !privateInput() && result.is_object() &&
            result.value("_path", std::string{}) == clipboard_path_ &&
            result.value("_generation", uint64_t{}) == clipboard_generation_)
          clipboard_items_ = result.value("entries", Json::array());
      }
      if (!preferences_.value("clipboard_history", false)) {
        if (clipboard_loading_ || !clipboard_items_.empty()) ++clipboard_generation_;
        clipboard_items_.clear();
        clipboard_loading_ = false;
        return;
      }
      if (clipboard_loading_ || clipboard_path_.empty() || restricted() || privateInput() || !ic_.hasFocus()) return;
      clipboard_loading_ = true;
      const auto path = clipboard_path_;
      const auto generation = clipboard_generation_;
      clipboard_job_ = detachedJob([path, generation] {
        auto raw = response(msime_client_load_clipboard_history(
            reinterpret_cast<const uint8_t *>(path.data()), path.size()));
        if (!raw.is_object()) return Json::object();
        raw["_path"] = path;
        raw["_generation"] = generation;
        return raw;
      });
    } catch (...) { clipboard_loading_ = false; clipboard_items_.clear(); }
  }
  msime::linux_host::TypingSource typingSource() const {
    const auto profile = preferences_.value("shuangpin_profile", std::string("xiaohe"));
    return msime::linux_host::resolve_typing_source(
        view_.value("scheme", -1), view_.value("nine_key", false),
        view_.value("dedicated_english", false),
        view_.value("local_mode", std::string("none")), profile);
  }
  void recordTypingStatistics(const std::string &text,
                              msime::linux_host::TypingSource source) const {
    // With statistics off nothing below runs: no date, no request, no thread, no store lock.
    if (!fcitx_typing_statistics.enabled()) return;
    if (text.empty() || options_path_.empty() || privateInput()) return;
    const auto directory = options_path_;
    if (directory.front() != '/') return;
    std::time_t now = std::time(nullptr);
    std::tm local{};
    if (localtime_r(&now, &local) == nullptr) return;
    char day[11]{};
    if (std::strftime(day, sizeof(day), "%Y-%m-%d", &local) == 0) return;
    const auto sourceId = std::string(msime::linux_host::typing_source_id(source));
    fcitx_key_press_writes.begin();
    try {
      std::thread([directory, text, sourceId, day = std::string(day),
                   hour = local.tm_hour] {
        try {
          const auto request = Json{
              {"directory", directory},
              {"action", Json{{"operation", "record"}, {"text", text},
                                {"source", sourceId}, {"day", day},
                                {"hour", hour}}}}
                                    .dump();
          if (auto *raw = msime_client_typing_statistics(
                  reinterpret_cast<const uint8_t *>(request.data()), request.size()))
            msime_client_string_free(raw);
        } catch (...) {
          // Statistics are best effort and must never affect text commitment.
        }
        fcitx_key_press_writes.end();
      }).detach();
    } catch (...) {
      fcitx_key_press_writes.end();
    }
  }
  // `typingStatistics` is false for text the Engine generated rather than the user typed out (the expression, command and mention modes), which the statistics leave out.
  void commitText(const std::string &text,
                  std::optional<msime::linux_host::TypingSource> source = std::nullopt,
                  bool typingStatistics = true) {
    if (text.empty()) return;
    ic_.commitString(text);
    if (typingStatistics) recordTypingStatistics(text, source.value_or(typingSource()));
  }
  bool pasteClipboard(size_t index = 0) {
    if (restricted() || privateInput() || !ic_.hasFocus()) return false;
    refreshClipboard();
    if (index >= clipboard_items_.size()) return false;
    const auto &item = clipboard_items_.at(index);
    const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
    if (text.empty()) return false;
    commitText(text, msime::linux_host::TypingSource::Reply);
    return true;
  }
  bool removeClipboard(size_t index) {
    if (restricted() || privateInput() || !ic_.hasFocus() || clipboard_path_.empty()) return false;
    refreshClipboard();
    if (clipboard_mutation_job_.valid() || index >= clipboard_items_.size()) return false;
    const auto &item = clipboard_items_.at(index);
    const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
    if (text.empty()) return false;
    const auto path = clipboard_path_;
    clipboard_mutation_job_ = detachedJob([path, text] {
      const auto request = Json{{"directory", path}, {"text", text}}.dump();
      auto raw = response(msime_client_remove_clipboard_history(
          reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      return raw.is_object() ? raw : Json::object();
    });
    return true;
  }
  bool clearClipboard() {
    if (restricted() || privateInput() || !ic_.hasFocus() || clipboard_path_.empty()) return false;
    refreshClipboard();
    if (clipboard_mutation_job_.valid() || clipboard_items_.empty()) return false;
    std::vector<std::string> texts;
    texts.reserve(clipboard_items_.size());
    for (const auto &item : clipboard_items_) {
      const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
      if (!text.empty()) texts.push_back(text);
    }
    if (texts.empty()) return false;
    const auto path = clipboard_path_;
    clipboard_mutation_job_ = detachedJob([path, texts = std::move(texts)] {
      Json result = Json::object();
      for (const auto &text : texts) {
        const auto request = Json{{"directory", path}, {"text", text}}.dump();
        result = response(msime_client_remove_clipboard_history(
            reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      }
      return result;
    });
    return true;
  }
  void refreshCloudClipboard() {
    try {
      if (cloud_clipboard_job_.valid()) {
        if (cloud_clipboard_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto result = cloud_clipboard_job_.get();
        cloud_clipboard_job_ = {};
        if (ic_.hasFocus() && !restricted() && !privateInput() && result.is_object() &&
            result.value("_socket", std::string{}) == cloud_clipboard_socket_ &&
            result.value("_generation", uint64_t{}) == cloud_clipboard_generation_)
        {
          cloud_clipboard_enabled_ = result.value("enabled", true);
          cloud_clipboard_items_ = cloud_clipboard_enabled_
              ? result.value("items", Json::array())
              : Json::array();
        }
      }
    } catch (...) { cloud_clipboard_items_.clear(); }
  }
  bool pasteCloudClipboard(size_t index = 0) {
    if (cloud_clipboard_socket_.empty() || restricted() || privateInput() || !ic_.hasFocus()) return false;
    refreshCloudClipboard();
    if (!cloud_clipboard_enabled_) return false;
    if (index < cloud_clipboard_items_.size()) {
      const auto &item = cloud_clipboard_items_.at(index);
      const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
      if (!text.empty()) { commitText(text, msime::linux_host::TypingSource::Reply); return true; }
    }
    if (index != 0) return false;
    if (cloud_clipboard_job_.valid()) return false;
    const auto socket = cloud_clipboard_socket_;
    const auto generation = cloud_clipboard_generation_;
    cloud_clipboard_job_ = detachedJob([socket, generation] {
      const auto request = Json{{"operation", "list"}, {"search", ""}}.dump();
      auto raw = response(msime_client_cloud_clipboard_provider_request(
          reinterpret_cast<const uint8_t *>(request.data()), request.size(),
          reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
      if (!raw.is_object()) return Json::object();
      raw["_socket"] = socket;
      raw["_generation"] = generation;
      return raw;
    });
    return false;
  }
  bool requestCloudClipboard() { return pasteCloudClipboard(); }
  void refreshEmoji() {
    try {
      if (emoji_groups_job_.valid() &&
          emoji_groups_job_.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
        auto result = emoji_groups_job_.get();
        emoji_groups_job_ = {};
        if (result.is_object() &&
            result.value("_generation", uint64_t{}) == emoji_generation_) {
          emoji_groups_.clear();
          for (const auto &item : result.value("groups", Json::array()))
            if (item.is_string() && !item.get<std::string>().empty()) emoji_groups_.push_back(item.get<std::string>());
          if (result.contains("_plugin_groups"))
            emoji_plugin_groups_ = msime::linux_host::parse_plugin_symbol_groups(result.at("_plugin_groups"));
          emoji_groups_loaded_ = true;
        }
      }
      if (emoji_job_.valid()) {
        if (emoji_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto result = emoji_job_.get();
        emoji_job_ = {};
        const auto requestQuery = emoji_job_query_;
        emoji_job_query_.clear();
        const bool current = result.is_object() && result.value("_generation", uint64_t{}) == emoji_generation_;
        if (current && result.contains("_plugin_groups"))
          emoji_plugin_groups_ = msime::linux_host::parse_plugin_symbol_groups(result.at("_plugin_groups"));
        if (ic_.hasFocus() && !restricted() && !privateInput() &&
            requestQuery == emoji_search_ && current) {
          // 内置目录的页之后接上符号集插件的条目；内置目录读不出来时只剩插件条目，没有插件时与原来的内置分页一致。
          const auto pluginItems = msime::linux_host::plugin_emoji_items(
              emoji_plugin_groups_, emoji_category_, emojiBuiltinGroup(), emoji_plugin_group_, requestQuery);
          msime::linux_host::EmojiPage page;
          if (emoji_offset_.plugin || emoji_plugin_group_) {
            page = msime::linux_host::plugin_emoji_page(pluginItems, emoji_offset_.offset, kEmojiPageSize);
          } else if (result.value("_builtin_failed", false)) {
            page = msime::linux_host::merge_builtin_emoji_page(Json::array(), emoji_offset_.offset, true,
                                                               pluginItems, kEmojiPageSize);
          } else {
            const auto items = result.value("items", Json::array());
            page = msime::linux_host::merge_builtin_emoji_page(
                items, result.value("next_offset", emoji_offset_.offset + items.size()),
                result.value("complete", true), pluginItems, kEmojiPageSize);
          }
          emoji_items_ = std::move(page.items);
          emoji_next_offset_ = page.next;
          emoji_complete_ = page.complete;
        } else if (emoji_search_mode_ && requestQuery != emoji_search_ && ic_.hasFocus() &&
                   !restricted() && !privateInput()) {
          emoji_items_.clear();
          requestEmojiPage({});
        }
      }
    } catch (...) { emoji_items_.clear(); }
  }
  static constexpr size_t kEmojiPageSize = 5;
  // 交给 Host API 的内置分组名；选中插件组时内置目录不参与。
  std::string emojiBuiltinGroup() const { return emoji_plugin_group_ ? std::string{} : emoji_group_; }
  // 插件目录与 Host API 的 preferences_directory 是同一个状态目录；没有绝对路径时不读插件。
  std::string emojiPluginsDirectory() const {
    if (options_path_.empty() || options_path_.front() != '/') return {};
    return (std::filesystem::path(options_path_) / "plugins").string();
  }
  bool requestEmojiPage(msime::linux_host::EmojiPageCursor cursor) {
    if (emoji_job_.valid() || resources_.empty()) return false;
    const auto resources = resources_;
    const auto category = emoji_category_;
    const auto group = emojiBuiltinGroup();
    const auto search = emoji_search_;
    const auto generation = emoji_generation_;
    // 插件阶段的游标或选中了插件组时不再查内置目录。
    const bool builtin = !cursor.plugin && !emoji_plugin_group_;
    const bool reloadPlugins = emoji_plugins_stale_ && (category == "symbols" || category == "kaomoji");
    const auto plugins = reloadPlugins ? emojiPluginsDirectory() : std::string{};
    if (reloadPlugins) emoji_plugins_stale_ = false;
    emoji_offset_ = cursor;
    emoji_job_query_ = search;
    emoji_job_ = detachedJob([resources, plugins, reloadPlugins, builtin, category, group, search, offset = cursor.offset,
                              generation] {
      auto result = Json::object();
      if (reloadPlugins) result["_plugin_groups"] = loadPluginSymbolGroups(resources, plugins);
      if (builtin) {
        try {
          const auto query = Json{{"limit", kEmojiPageSize}, {"offset", offset}, {"cursor", true},
                                  {"category", category}, {"group", group}, {"search", search}}.dump();
          auto page = response(msime_client_emoji_catalog_request(
              reinterpret_cast<const uint8_t *>(query.data()), query.size(),
              reinterpret_cast<const uint8_t *>(resources.data()), resources.size()));
          if (page.is_object()) result.update(page);
          else result["_builtin_failed"] = true;
        } catch (...) {
          result["_builtin_failed"] = true;
        }
      }
      result["_generation"] = generation;
      return result;
    });
    return true;
  }
  bool beginEmojiSearch() {
    if (restricted() || privateInput() || !ic_.hasFocus() || resources_.empty()) return false;
    emoji_search_mode_ = true;
    emoji_search_.clear();
    emoji_items_.clear();
    emoji_offset_ = {};
    emoji_next_offset_ = {};
    emoji_complete_ = false;
    emoji_previous_offsets_.clear();
    emoji_plugins_stale_ = true;
    if (!emoji_job_.valid()) requestEmojiPage({});
    render();
    return true;
  }
  void endEmojiSearch() {
    if (!emoji_search_mode_) return;
    emoji_search_mode_ = false;
    emoji_search_.clear();
    emoji_items_.clear();
    render();
  }
  bool insertEmoji(size_t index = 0) {
    if (restricted() || privateInput() || !ic_.hasFocus()) return false;
    refreshEmoji();
    if (index < emoji_items_.size()) {
      const auto &item = emoji_items_.at(index);
      const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
      if (!text.empty()) { commitText(text, msime::linux_host::TypingSource::Local); return true; }
    }
    if (index != 0 || !emoji_items_.empty()) return false;
    return requestEmojiPage({});
  }
  bool nextEmojiPage() {
    if (restricted() || privateInput() || !ic_.hasFocus()) return false;
    refreshEmoji();
    if (emoji_complete_ || emoji_job_.valid()) return false;
    emoji_previous_offsets_.push_back(emoji_offset_);
    return requestEmojiPage(emoji_next_offset_);
  }
  bool previousEmojiPage() {
    if (restricted() || privateInput() || !ic_.hasFocus()) return false;
    refreshEmoji();
    if (emoji_job_.valid() || emoji_previous_offsets_.empty()) return false;
    const auto offset = emoji_previous_offsets_.back();
    emoji_previous_offsets_.pop_back();
    return requestEmojiPage(offset);
  }
  bool cycleEmojiCategory() {
    if (restricted() || privateInput() || !ic_.hasFocus() || emoji_job_.valid()) return false;
    static constexpr std::array<const char *, 3> categories = {"", "kaomoji", "symbols"};
    auto it = std::find(categories.begin(), categories.end(), emoji_category_);
    emoji_category_ = it == categories.end() || std::next(it) == categories.end()
        ? categories.front() : *std::next(it);
    emoji_group_.clear();
    emoji_groups_.clear();
    emoji_groups_loaded_ = false;
    emoji_group_index_ = 0;
    emoji_plugin_group_.reset();
    emoji_plugins_stale_ = true;
    emoji_items_.clear();
    emoji_offset_ = {};
    emoji_next_offset_ = {};
    emoji_complete_ = false;
    emoji_previous_offsets_.clear();
    return requestEmojiPage({});
  }
  bool cycleEmojiGroup() {
    if (restricted() || privateInput() || !ic_.hasFocus() || emoji_job_.valid() ||
        emoji_groups_job_.valid()) return false;
    const auto count = msime::linux_host::emoji_group_count(emoji_groups_, emoji_plugin_groups_, emoji_category_);
    if (!emoji_groups_loaded_ || count == 0) {
      if (resources_.empty()) return false;
      const auto resources = resources_;
      const auto category = emoji_category_;
      const auto generation = emoji_generation_;
      // 内置分组和插件组一起读，分组循环看到的是同一时刻的插件列表；内置目录读不出来时插件组照样可选。
      const bool withPlugins = category == "symbols" || category == "kaomoji";
      const auto plugins = withPlugins ? emojiPluginsDirectory() : std::string{};
      emoji_groups_job_ = detachedJob([resources, category, generation, withPlugins, plugins] {
        auto result = Json::object();
        try {
          const auto query = Json{{"limit", 1}, {"list_groups", true}, {"category", category}}.dump();
          auto listed = response(msime_client_emoji_catalog_request(
              reinterpret_cast<const uint8_t *>(query.data()), query.size(),
              reinterpret_cast<const uint8_t *>(resources.data()), resources.size()));
          if (listed.is_object()) result["groups"] = listed.value("groups", Json::array());
        } catch (...) {}
        if (withPlugins) result["_plugin_groups"] = loadPluginSymbolGroups(resources, plugins);
        result["_generation"] = generation;
        return result;
      });
      return false;
    }
    emoji_group_index_ = (emoji_group_index_ + 1) % (count + 1);
    const auto choice = msime::linux_host::emoji_group_choice(emoji_groups_, emoji_plugin_groups_,
                                                              emoji_category_, emoji_group_index_);
    emoji_group_ = choice.label;
    emoji_plugin_group_ = choice.plugin;
    emoji_items_.clear();
    emoji_offset_ = {};
    emoji_next_offset_ = {};
    emoji_complete_ = false;
    emoji_previous_offsets_.clear();
    return requestEmojiPage({});
  }
  void hideVoiceOverlay() {
    if (wave_overlay_surface_ && wave_overlay_visible_)
      wave_overlay_surface_->hide();
    wave_overlay_visible_ = false;
  }
  // Release the session, and with it the shared dictionary lock, when the settings window asks for maintenance (core/DictionaryQuiesceLease.h). The composition is finished first, so nothing typed is lost; ensure() opens a new session once the lease is gone.
  void refreshDictionaryQuiesce() {
    if (!session_ || !msime::linux_host::dictionary_quiesced(dictionary_user_data_)) return;
    if (!view_.value("editing_text", std::string{}).empty())
      command(MSIME_FINISH_COMPOSITION);
    close();
    clearPanel();
    msime_linux_diagnostic_write("dictionary_quiesce_released");
  }
  // The voice overlay's mode and palette, re-read wherever the preferences, the skin catalogue or the desktop appearance change.
  void syncVoiceOverlayTheme() {
    const auto theme = resolveVoiceOverlayTheme(preferences_, system_dark_, candidate_skin_document_);
    wave_overlay_.light_theme = !theme.dark;
    wave_overlay_.palette = msime::linux_host::floating_surface_colors(theme);
  }
  // Called by FcitxEngine::applySystemTheme on the loop when its addon-wide probe sees the desktop appearance change.
  void setSystemDark(bool dark) {
    if (dark == system_dark_) return;
    system_dark_ = dark;
    syncVoiceOverlayTheme();
    if (voice_loading_) updateVoiceOverlay();
    syncCandidatePanelTheme();
  }
  void updateVoiceOverlay() {
    wave_overlay_.status = voice_phase_;
    wave_overlay_.locked = voice_space_locked_ && wave_overlay_.listening;
    wave_overlay_.set_transcript(voice_transcript_);
    wave_overlay_.set_input_level(static_cast<float>(voice_level_) / 10.0f);
    if (wave_overlay_surface_ && !wave_overlay_failed_) {
      if (wave_overlay_visible_) {
        wave_overlay_surface_->update(wave_overlay_);
        return;
      }
      wave_overlay_visible_ = wave_overlay_surface_->show(wave_overlay_);
      if (wave_overlay_visible_) return;
      // No surface on this display (GNOME Wayland has no layer-shell): the rest of this recording uses the auxiliary text, as the IBus host's FallbackSurface does. The next recording tries the surface again.
      wave_overlay_failed_ = true;
    }
    if (voice_loading_) {
      std::string status = voice_phase_;
      if (voice_level_ != 0) status += " " + std::string(voice_level_, '#');
      if (!voice_transcript_.empty()) status += "：" + voice_transcript_;
      ic_.inputPanel().setAuxUp(fcitx::Text("语音：" + status));
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
  }
  // Tell the user why nothing was typed, as the Windows service does with a message box and the IBus host with show_voice_failure: the overlay (or the auxiliary text when there is no overlay surface) carries a fixed message for 1.2 seconds. The messages are fixed on purpose, since provider errors can carry private values.
  void showVoiceFailure(const char *message) {
    if (++voice_failure_id_ == 0) ++voice_failure_id_;
    const auto id = voice_failure_id_;
    const std::string aux = std::string("语音：") + message;
    voice_failure_visible_ = true;
    wave_overlay_.reset();
    wave_overlay_.status = message;
    wave_overlay_.show_transcript = false;
    wave_overlay_.actions_visible = false;
    wave_overlay_.listening = false;
    if (wave_overlay_surface_) {
      if (wave_overlay_visible_)
        wave_overlay_surface_->update(wave_overlay_);
      else
        wave_overlay_visible_ = wave_overlay_surface_->show(wave_overlay_);
    }
    ic_.inputPanel().setAuxUp(fcitx::Text(aux));
    ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    voice_failure_timer_ = loop_->addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 1200000, 0,
        [this, id, aux](fcitx::EventSourceTime *, uint64_t) {
          if (voice_loading_ || voice_failure_id_ != id) return true;
          voice_failure_visible_ = false;
          hideVoiceOverlay();
          wave_overlay_.reset();
          if (ic_.inputPanel().auxUp().toString() == aux) {
            ic_.inputPanel().setAuxUp(fcitx::Text());
            ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
          }
          return true;
        });
  }
  bool refreshVoice() {
    try {
      const auto mailbox = voice_mailbox_;
      if (mailbox && ic_.hasFocus() && !restricted() && !privateInput()) {
        std::string partial;
        uint8_t phase = 0, level = 0;
        bool phaseSeen = false, levelSeen = false;
        {
          std::lock_guard lock(mailbox->mutex);
          partial = mailbox->partial;
          phase = mailbox->phase;
          phaseSeen = mailbox->phase_seen;
          level = mailbox->level;
          levelSeen = mailbox->level_seen;
        }
        if (!partial.empty() || phaseSeen || levelSeen) {
          voice_partial_seen_ = true;
          voice_phase_seen_ = voice_phase_seen_ || phaseSeen;
          voice_level_seen_ = voice_level_seen_ || levelSeen;
          partial = msime_voice_bound_result(std::move(partial));
          if (!partial.empty()) {
            // Fcitx5 commit is the only voice commit path on Linux and the settings page offers no strategy, so a stored commit_mode must not turn the inline preedit off.
            const bool inlinePreedit = msime_voice_stream_inline_enabled(
                voice_options_.value("stream_inline_preedit", false),
                voice_options_.value("asr_provider", std::string{"doubao"}), "tsf");
            if (inlinePreedit) {
              voice_preedit_ = partial;
              voice_transcript_.clear();
            } else {
              voice_transcript_ = partial;
              voice_preedit_.clear();
            }
            render();
          }
          const char *phaseLabel[] = {"录音中", "识别中", "整理中"};
          if (phaseSeen) {
            voice_phase_ = phaseLabel[std::min<size_t>(phase, 2)];
            if (phase >= 2)
              wave_overlay_.compact_status =
                  msime::linux_host::WaveOverlayModel::CompactStatus::Processing;
            else if (phase == 1)
              wave_overlay_.compact_status =
                  msime::linux_host::WaveOverlayModel::CompactStatus::Recognizing;
          }
          if (levelSeen) voice_level_ = level;
          if (!partial.empty()) voice_transcript_ = std::move(partial);
          updateVoiceOverlay();
        }
      }
      if (!voice_job_.valid()) return false;
      if (voice_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return false;
      auto result = voice_job_.get();
      voice_job_ = {};
      const bool cancelled = voice_cancelled_;
      voice_cancelled_ = false;
      voice_loading_ = false;
      voice_space_consumed_ = false;
      voice_space_locked_ = false;
      if (cancelled) {
        voice_preedit_.clear();
        voice_transcript_.clear();
        voice_mailbox_.reset();
        // A refused stop cancels the recording and shows why; the worker finishing afterwards must not take that notice down early.
        if (!voice_failure_visible_) hideVoiceOverlay();
        render();
        return false;
      }
      if (session_ && ic_.hasFocus() && !restricted() && !privateInput() && result.is_object()) {
        auto text = result.value("text", std::string{});
        std::string latestPartial;
        if (mailbox) {
          std::lock_guard lock(mailbox->mutex);
          if (text.empty() && mailbox->final_ready) text = mailbox->final;
          latestPartial = mailbox->partial;
        }
        if (voice_preedit_.empty() && voice_transcript_.empty() && !latestPartial.empty()) {
          latestPartial = msime_voice_bound_result(std::move(latestPartial));
          const bool inlinePreedit = msime_voice_stream_inline_enabled(
              voice_options_.value("stream_inline_preedit", false),
              voice_options_.value("asr_provider", std::string{"doubao"}), "tsf");
          (inlinePreedit ? voice_preedit_ : voice_transcript_) = std::move(latestPartial);
        }
        text = msime_voice_result_or_transcript(
            std::move(text), voice_transcript_, voice_preedit_);
        voice_preedit_.clear();
        voice_transcript_.clear();
        hideVoiceOverlay();
        render();
        ic_.inputPanel().setAuxUp(fcitx::Text());
        ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        voice_mailbox_.reset();
        if (!text.empty()) { commitText(text, msime::linux_host::TypingSource::Voice); return true; }
        const auto providerError = result.find("provider_error");
        showVoiceFailure(providerError != result.end() && providerError->is_string()
                             ? msime_voice_provider_failure_notice(providerError->get<std::string>())
                             : "未识别到文字，请重新录音");
        return false;
      }
      voice_preedit_.clear();
      voice_transcript_.clear();
      voice_mailbox_.reset();
      hideVoiceOverlay();
      render();
    } catch (...) {
      voice_loading_ = false;
      voice_space_consumed_ = false;
      voice_space_locked_ = false;
      voice_preedit_.clear();
      voice_transcript_.clear();
      voice_mailbox_.reset();
      hideVoiceOverlay();
      render();
      if (ic_.hasFocus() && !restricted() && !privateInput())
        showVoiceFailure("语音输入失败，请检查语音服务、麦克风及提供商配置后重试");
    }
    return false;
  }
  // An oversized prompt must not take the rest of the configuration down with it; like the IBus host, the refusal surfaces when voice input starts.
  void loadVoiceOptions() {
    try {
      voice_options_ = msime::linux_host::voice_provider_options(preferences_);
      voice_options_valid_ = true;
    } catch (const std::runtime_error &) {
      voice_options_ = Json::object();
      voice_options_valid_ = false;
    }
  }
  bool requestVoice() {
    if (!voice_enabled_ || restricted() || privateInput() || !ic_.hasFocus() || voice_loading_) return false;
    if (voice_socket_.empty() || !voice_options_valid_) {
      showVoiceFailure("无法启动语音输入，请检查语音设置后重试");
      return false;
    }
    if (voice_job_.valid()) {
      if (refreshVoice()) return true;
      if (voice_job_.valid()) return false;
    }
    if (voice_loading_) return false;
    // Voice text takes the composition's place: Windows purges the typed keys when the voice composition starts, and the IBus host cancels the composition in voice_start. Left in place, the pinyin came back as preedit after the voice result was committed.
    if (session_ && (!view_.value("editing_text", std::string{}).empty() ||
                     !view_.value("candidates", Json::array()).empty()))
      command(MSIME_CANCEL);
    voice_loading_ = true;
    syncMusic();
    voice_cancelled_ = false;
    const auto socket = voice_socket_;
    const auto generation = view_.value("generation", uint64_t{});
    const auto language = voice_language_;
    const auto options = voice_options_;
    const auto host_options = msime::linux_host::voice_wants_hotwords(options) ? voice_host_options_ : Json();
    voice_generation_ = generation;
    voice_mailbox_ = std::make_shared<FcitxVoiceMailbox>();
    voice_preedit_.clear();
    voice_transcript_.clear();
    voice_partial_seen_ = false;
    voice_phase_seen_ = false;
    voice_level_seen_ = false;
    voice_transcript_.clear();
    voice_phase_ = "录音中";
    voice_level_ = 0;
    voice_failure_visible_ = false;
    wave_overlay_failed_ = false;
    wave_overlay_.reset();
    wave_overlay_.listening = true;
    wave_overlay_.show_transcript = true;
    wave_overlay_.actions_visible = true;
    updateVoiceOverlay();
    const auto mailbox = voice_mailbox_;
    voice_job_ = detachedJob([socket, generation, language, options, host_options, mailbox] {
      auto request = msime::linux_host::voice_query(language, generation, options, host_options);
      request["stream"] = true;
      const auto query = request.dump();
      std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
          msime_client_voice_provider_stream_feedback(
              reinterpret_cast<const uint8_t *>(query.data()), query.size(),
              reinterpret_cast<const uint8_t *>(socket.data()), socket.size(),
              fcitxVoiceUpdate, fcitxVoiceStatus, fcitxVoiceLevel, mailbox.get()),
          msime_client_string_free);
      if (!raw) throw std::runtime_error("MSIME request failed");
      // A provider that gave no result (value null) or named a missing dependency (ok:false) is a provider failure, as in the IBus host, not an empty recognition.
      const auto document = Json::parse(raw.get());
      if (!document.value("ok", false))
        return Json{{"provider_error", document.value("error", std::string{})}};
      const auto result = document.at("value");
      return result.is_object() ? result : Json{{"provider_error", std::string{}}};
    });
    return true;
  }
  bool stopVoice() {
    if (!voice_loading_ || voice_socket_.empty() || voice_generation_ == 0) return false;
    const auto socket = voice_socket_;
    bool stopped = false;
    try {
      stopped = response(msime_client_voice_provider_stop(
          reinterpret_cast<const uint8_t *>(socket.data()), socket.size(), voice_generation_))
          .get<bool>();
    } catch (...) {
      stopped = false;
    }
    if (!stopped) {
      cancelVoice();
      showVoiceFailure("结束录音失败，本次语音已取消，请检查语音服务后重试");
      return true;
    }
    voice_phase_ = "识别中";
    wave_overlay_.listening = false;
    wave_overlay_.compact_status =
        msime::linux_host::WaveOverlayModel::CompactStatus::Recognizing;
    updateVoiceOverlay();
    return true;
  }
  bool cancelVoice() {
    if (!voice_loading_) return false;
    const auto socket = voice_socket_;
    const auto generation = voice_generation_;
    if (!socket.empty() && generation != 0)
      msime_client_string_free(msime_client_voice_provider_cancel(
          reinterpret_cast<const uint8_t *>(socket.data()), socket.size(), generation));
    voice_cancelled_ = true;
    voice_mailbox_.reset();
    voice_loading_ = false;
    voice_space_consumed_ = false;
    voice_space_locked_ = false;
    voice_partial_seen_ = false;
    voice_phase_seen_ = false;
    voice_level_seen_ = false;
    voice_preedit_.clear();
    voice_transcript_.clear();
    voice_phase_ = "录音中";
    voice_level_ = 0;
    hideVoiceOverlay();
    render();
    ic_.inputPanel().setAuxUp(fcitx::Text());
    ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    return true;
  }
  bool apply(char *raw,
             std::optional<std::string> spaceConvertPreceding = std::nullopt,
             msime::linux_host::PunctuationPairMode pairMode =
                 msime::linux_host::PunctuationPairMode::Unpaired) {
    auto result = response(raw);
    pair_inserted_ = false;
    if (result.contains("commit") && result["commit"].is_string()) {
      auto text = result["commit"].get<std::string>();
      if (traditionalApplies()) text = msime_linux_simplified_to_traditional(text);
      const auto spaceConvertAscii =
          spaceConvertPreceding
              ? msime::linux_host::smart_punctuation_ascii_mark(text)
              : 0;
      pair_inserted_ = msime::linux_host::normalize_punctuation_pair(text, pairMode);
      // An ASCII mark smart punctuation kept can be taken back to Chinese by
      // typing the same key again. Engine applies the width itself here, so the
      // commit is matched in whichever width it went out as - and against the
      // width in effect when this key produced it, which is the one still in
      // view_ at this point.
      const auto committedMark =
          msime::linux_host::ascii_mark_from_text(text, fullwidthOutput());
      if (committedMark != 0 && smart_punctuation_) {
        last_smart_punctuation_ = committedMark;
        last_smart_punctuation_at_ = std::chrono::steady_clock::now();
      } else if (committedMark == 0) {
        // Anything that is not one of these marks ends the gesture; a mark
        // committed with the feature off leaves the record alone rather than
        // clearing a window the user is still inside.
        last_smart_punctuation_ = 0;
        last_smart_punctuation_at_ = {};
      }
      const auto context = result.value("commit_context", Json(nullptr));
      commitText(text, std::nullopt,
                 !context.is_object() || context.value("typing_statistics", true));
      // The key sound played when the key went down; this is the commit's own sound, or the next note of a melody that advances on commits. Never for a secure field.
      if (!text.empty() && !privateInput()) {
        msime_client_commit_sound(session_);
        // The commit counts nothing; it reports the combo as it stands, which is how one that lapsed while the user paused leaves the aux line drawn next.
        typing_combo_ = msime::linux_host::typing_effect_combo(
            msime_client_typing_effect(session_, msime::linux_host::kTypingEffectCommit));
      }
      if (pair_inserted_) {
        if (const auto closing = msime::linux_host::paired_closing_from_text(text))
          paired_tracker_.push(*closing);
      }
      if (spaceConvertAscii != 0 && !pair_inserted_) {
        // Preserve Engine's actual half (notably opening/closing quotes).
        space_convert_mark_ = text;
        space_convert_preceding_ = std::move(*spaceConvertPreceding);
      }
    }
    view_ = result.contains("view") ? result.at("view") : result;
    render();
    return result.value("handled", false);
  }
  bool command(uint32_t command) { return apply(msime_client_command(session_, command)); }
  // The `count` characters in front of the caret, oldest first. std::nullopt is
  // "this host publishes nothing usable there", which is not the same answer as
  // an empty vector: that one means the document starts at the caret.
  std::optional<std::vector<std::string>> precedingCharacters(size_t count) {
    const auto &surrounding = ic_.surroundingText();
    if (privateInput() || !ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) ||
        !surrounding.isValid() || surrounding.cursor() != surrounding.anchor())
      return std::nullopt;
    const auto &text = surrounding.text();
    return preceding_characters(text, surrounding.cursor(), count);
  }
  bool composingOrCandidates() const {
    return msime::linux_host::view_has_composition(
        view_.value("editing_text", std::string{}),
        !view_.value("candidates", Json::array()).empty(),
        view_.value("phrase_prefix", std::string{}));
  }
  bool fullwidthOutput() const {
    return view_.value("character_width", std::string{}) == "Fullwidth";
  }
  // The session types Korean: jamo compose in the preedit, punctuation is always half-width ASCII and none of the Chinese punctuation helpers apply. The dedicated English mode keeps its own rules in every scheme.
  bool korean() const {
    return view_.value("scheme", 0u) == 4 && !view_.value("dedicated_english", false);
  }
  // The view's scheme number outside the dedicated English mode, which keeps its own rules in every scheme; -1 there.
  int typingScheme() const {
    return view_.value("dedicated_english", false) ? -1 : msime::linux_host::view_scheme(view_);
  }
  // A Korean syllable, Zhuyin conversion or Vietnamese word is text the user already wrote: a key that leaves it writes it out rather than discarding it (`commits_on_blur`), and the caret stays at its end, so there are no segments to edit (`locks_caret`).
  bool commitsOnBlur() const {
    const int scheme = typingScheme();
    return scheme >= 0 && msime::linux_host::scheme::CommitsOnBlur(scheme);
  }
  // Korean and Vietnamese write half-width ASCII marks and are never widened (`widens_full_width`).
  bool narrowScheme() const {
    const int scheme = typingScheme();
    return scheme == msime::linux_host::scheme::Korean || scheme == msime::linux_host::scheme::Vietnamese;
  }
  // Korean, Zhuyin and Vietnamese take their punctuation from the Engine without the host's paired and smart helpers (`host_smart_punctuation`). Japanese keeps them, as it always has on this host.
  bool withoutHostPunctuation() const {
    const int scheme = typingScheme();
    return scheme == msime::linux_host::scheme::Korean || scheme == msime::linux_host::scheme::Zhuyin ||
           scheme == msime::linux_host::scheme::Vietnamese;
  }
  void forgetSmartPunctuationRepeat() {
    last_smart_punctuation_ = 0;
    last_smart_punctuation_at_ = {};
    smart_punctuation_rejected_ = 0;
  }
  // The same mark typed twice inside the window means the user wanted the
  // Chinese one after all. Returns true when it replaced the mark, in which case
  // the key is consumed and never reaches Engine.
  bool repeatSmartPunctuationToChinese(char ascii) {
    // Korean and Vietnamese punctuation is always ASCII, so ".." stays ".."; Zhuyin takes its marks from the Engine alone.
    if (withoutHostPunctuation() || !chinese_punctuation_ || !smart_punctuation_ || !smart_punctuation_repeat_ ||
        last_smart_punctuation_ != ascii ||
        last_smart_punctuation_at_ == std::chrono::steady_clock::time_point{} ||
        composingOrCandidates())
      return false;
    if (std::chrono::steady_clock::now() - last_smart_punctuation_at_ >
        std::chrono::seconds(2))
      return false;
    const auto chinese = msime::linux_host::chinese_punctuation_mark(ascii);
    const auto characters = precedingCharacters(1);
    if (chinese.empty() || !characters ||
        !msime::linux_host::repeat_conversion_matches_document(
            ascii, fullwidthOutput(), *characters))
      return false;
    ic_.deleteSurroundingText(-1, 1);
    commitText(std::string(chinese));
    forgetSmartPunctuationRepeat();
    space_convert_mark_.clear();
    space_convert_preceding_.clear();
    return true;
  }
  bool punctuation(uint8_t value,
                   msime::linux_host::PunctuationPairMode pairMode =
                       msime::linux_host::PunctuationPairMode::Unpaired) {
    uint32_t preceding = 0;
    const auto &surrounding = ic_.surroundingText();
    if (!privateInput() && ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) &&
        surrounding.isValid() && surrounding.cursor() > 0 &&
        surrounding.cursor() == surrounding.anchor()) {
      const auto &text = surrounding.text();
      const auto length = fcitx::utf8::lengthValidated(text);
      if (length != fcitx::utf8::INVALID_LENGTH && surrounding.cursor() <= length)
        preceding = fcitx::utf8::getChar(
            fcitx::utf8::nextNChar(text.begin(), surrounding.cursor() - 1), text.end());
    }
    // Engine is about to commit the Chinese mark for this key. Record what the
    // caret follows now, while the document still predates the commit; a Space
    // arriving next re-reads both characters before rewriting either.
    const auto ascii = static_cast<char>(value);
    const bool arm = smart_punctuation_ && smart_punctuation_space_convert_ &&
                     chinese_punctuation_ &&
                     msime::linux_host::is_space_conversion_key(ascii) &&
                     !(paired_punctuation_ &&
                       msime::linux_host::is_auto_paired_opening_key(ascii)) &&
                     !composingOrCandidates();
    std::string armedPreceding;
    if (arm) {
      if (const auto characters = precedingCharacters(1); characters && !characters->empty())
        armedPreceding = characters->front();
    }
    // The ASCII mark this key already produced once was deleted, so the shared
    // route must not keep it a second time. Withholding the preceding character
    // is how that route is told there is no ASCII letter or digit to stay beside.
    if (smart_punctuation_rejected_ == ascii)
      preceding = 0;
    std::optional<std::string> spaceConvertPreceding;
    if (arm)
      spaceConvertPreceding = armedPreceding;
    // The shared route keeps , . : ASCII beside an ASCII letter or digit only under the follow lock, with Chinese and smart punctuation on and nothing composing; idle, Engine then hands the key back and the editor types it, so apply() never sees a commit to arm the repeat gesture from. Decide that here, before the call changes the view, so the same key typed again inside the window still converts, as Windows arms it for the ASCII mark it resolved.
    const bool keptAscii = punctuation_lock_ == 0 && chinese_punctuation_ && smart_punctuation_ &&
                           msime::linux_host::is_smart_punctuation_key(ascii) &&
                           preceding < 0x80 && std::isalnum(static_cast<int>(preceding)) != 0 &&
                           !composingOrCandidates() && !view_.value("dedicated_english", false) &&
                           view_.value("local_mode", std::string("none")) == "none" &&
                           view_.value("scheme", 0u) != 3 && !withoutHostPunctuation();
    const bool handled =
        apply(msime_client_punctuation_with_context(session_, value, preceding),
              std::move(spaceConvertPreceding), pairMode);
    if (handled && smart_punctuation_rejected_ == ascii)
      forgetSmartPunctuationRepeat();
    if (!handled && keptAscii) {
      last_smart_punctuation_ = ascii;
      last_smart_punctuation_at_ = std::chrono::steady_clock::now();
    }
    return handled;
  }
  // The character right after the caret. std::nullopt when the host publishes nothing usable; an empty string when the document ends at the caret.
  std::optional<std::string> followingCharacter() {
    const auto &surrounding = ic_.surroundingText();
    if (privateInput() || !ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) ||
        !surrounding.isValid() || surrounding.cursor() != surrounding.anchor())
      return std::nullopt;
    const auto &text = surrounding.text();
    const auto length = fcitx::utf8::lengthValidated(text);
    if (length == fcitx::utf8::INVALID_LENGTH || surrounding.cursor() > length)
      return std::nullopt;
    if (surrounding.cursor() == length) return std::string{};
    const auto start = fcitx::utf8::nextNChar(text.begin(), surrounding.cursor());
    return std::string(start, fcitx::utf8::nextChar(start));
  }
  void forwardCaret(fcitx::KeySym sym) {
    ic_.forwardKey(fcitx::Key(sym), false);
    ic_.forwardKey(fcitx::Key(sym), true);
  }
  bool pairedPunctuationEnabled() const {
    return chinese_punctuation_ && paired_punctuation_ &&
           !msime::linux_host::paired_punctuation_excluded_client(ic_.program());
  }
  // Typing the closing mark of a pair this host completed steps over the one already in the document instead of adding a second, as Windows and the IBus host do. The tracker only agrees while that mark is still right after the caret.
  bool skipPairedClosing(fcitx::KeySym sym, fcitx::KeyStates states) {
    if (!pairedPunctuationEnabled()) return false;
    const auto typed = fcitx::Key::keySymToUTF8(sym);
    if (typed.size() != 1) return false;
    const auto closing = msime::linux_host::paired_closing_for_key(typed[0], fullwidthOutput());
    if (!closing) return false;
    using Modifier = msime::linux_host::PairedPunctuationModifier;
    std::uint32_t modifiers = 0;
    if (states.test(fcitx::KeyState::Ctrl)) modifiers |= static_cast<std::uint32_t>(Modifier::Control);
    if (states.test(fcitx::KeyState::Alt)) modifiers |= static_cast<std::uint32_t>(Modifier::Alt);
    if (states.test(fcitx::KeyState::Super)) modifiers |= static_cast<std::uint32_t>(Modifier::Super);
    if (states.test(fcitx::KeyState::Meta)) modifiers |= static_cast<std::uint32_t>(Modifier::Meta);
    if (states.test(fcitx::KeyState::Hyper)) modifiers |= static_cast<std::uint32_t>(Modifier::Hyper);
    if (states.test(fcitx::KeyState::Mod5)) modifiers |= static_cast<std::uint32_t>(Modifier::Mod5);
    const auto following = followingCharacter();
    if (!paired_tracker_.consume(*closing, following.value_or(""), following.has_value(),
                                 msime::linux_host::paired_closing_modifiers_allowed(modifiers)))
      return false;
    forgetSmartPunctuationRepeat();
    forwardCaret(FcitxKey_Right);
    return true;
  }
  // Brackets, the book title, braces and quotes are completed with their closing mark and the caret is stepped back between the two, following the Windows host. `{` goes out as the literal brace, as it does in the IBus host.
  // std::nullopt: not a paired key here, route it as ordinary punctuation.
  std::optional<bool> pairedPunctuation(char ascii, bool composing) {
    using Mode = msime::linux_host::PunctuationPairMode;
    Mode mode = Mode::Unpaired;
    if (ascii == '"') mode = Mode::DoubleQuote;
    else if (ascii == '\'' && !composing) mode = Mode::SingleQuote;
    else if (ascii == '(' || ascii == '[' || ascii == '<') mode = Mode::Bracket;
    else if (ascii == '{') mode = Mode::Brace;
    if (mode == Mode::Unpaired || !pairedPunctuationEnabled()) return std::nullopt;
    const auto value = static_cast<uint8_t>(ascii);
    bool handled = mode == Mode::Brace
                       ? apply(msime_client_punctuation_ascii(session_, value), std::nullopt, mode)
                       : punctuation(value, mode);
    if (handled && pair_inserted_ && ascii == '<')
      (void)response(msime_client_balance_paired_punctuation_after_auto_close(session_, value));
    if (!handled && mode == Mode::Brace) {
      const auto text = fullwidthOutput() ? std::string("｛｝") : std::string("{}");
      commitText(text);
      paired_tracker_.push(fullwidthOutput() ? "｝" : "}");
      pair_inserted_ = true;
      handled = true;
    }
    if (handled && pair_inserted_) forwardCaret(FcitxKey_Left);
    return handled;
  }
  // A Space right after a Chinese mark the user did not want takes that mark
  // back to ASCII, mirroring the source's space conversion and the IBus host.
  // A successful rewrite consumes the Space.
  bool convertSmartPunctuationSpace() {
    const auto mark = std::move(space_convert_mark_);
    const auto expected = std::move(space_convert_preceding_);
    space_convert_mark_.clear();
    space_convert_preceding_.clear();
    if (mark.empty() || !smart_punctuation_ ||
        !smart_punctuation_space_convert_ || !chinese_punctuation_ ||
        composingOrCandidates())
      return false;
    const auto replacement =
        msime::linux_host::space_conversion_ascii_text(mark);
    const auto characters = precedingCharacters(2);
    if (replacement.empty() || !characters ||
        !msime::linux_host::space_conversion_matches_document(mark, expected,
                                                              *characters))
      return false;
    ic_.deleteSurroundingText(-1, 1);
    commitText(replacement);
    return true;
  }
  void select(uint64_t session, uint64_t generation, size_t index) {
    if (translation_candidates_active_) {
      if (session_ == session && view_.value("generation", uint64_t{}) == generation)
        commitTranslationCandidate(index);
      return;
    }
    if (!ensure() || session_ != session || view_.value("generation", uint64_t{}) != generation) return;
    apply(msime_client_select(session_, generation, index));
  }
  bool translationCandidatesActive() const { return translation_candidates_active_; }
  void translationPage(uint32_t command) {
    if (!translation_candidates_active_) return;
    const auto pageSize = std::clamp(
        translation_saved_view_.value("page_size", size_t{9}), size_t{1}, size_t{9});
    const auto pageCount = (translation_options_.size() + pageSize - 1) / pageSize;
    if (command == MSIME_PREVIOUS_PAGE && translation_page_ > 0)
      --translation_page_;
    else if (command == MSIME_NEXT_PAGE && translation_page_ + 1 < pageCount)
      ++translation_page_;
    else
      return;
    translation_cursor_ = 0;
    renderTranslationCandidates();
  }
  bool enterTranslationCandidates(const std::string &gloss) {
    const auto senses = msime::linux_host::split_translation_gloss(gloss);
    if (senses.size() <= 1) return false;
    translation_saved_view_ = view_;
    translation_options_ = senses;
    translation_candidates_active_ = true;
    translation_page_ = 0;
    translation_cursor_ = 0;
    renderTranslationCandidates();
    return true;
  }
  void exitTranslationCandidates() {
    if (!translation_candidates_active_) return;
    translation_candidates_active_ = false;
    if (translation_saved_view_.is_object()) view_ = std::move(translation_saved_view_);
    translation_saved_view_ = Json::object();
    translation_options_.clear();
    translation_page_ = 0;
    translation_cursor_ = 0;
    render();
  }
  void renderTranslationCandidates() {
    if (!translation_candidates_active_ || !translation_saved_view_.is_object() ||
        translation_options_.empty()) return;
    const auto pageSize = std::clamp(
        translation_saved_view_.value("page_size", size_t{9}), size_t{1}, size_t{9});
    const auto pageCount = (translation_options_.size() + pageSize - 1) / pageSize;
    translation_page_ = std::min(translation_page_, pageCount - 1);
    const auto start = translation_page_ * pageSize;
    translation_cursor_ = std::min(translation_cursor_, translation_options_.size() - start - 1);
    auto overlay = translation_saved_view_;
    overlay["page"] = translation_page_;
    overlay["page_size"] = pageSize;
    overlay["page_count"] = pageCount;
    overlay["candidates"] = Json::array();
    const auto end = std::min(start + pageSize, translation_options_.size());
    for (size_t index = start; index < end; ++index) {
      Json candidate = Json::object();
      candidate["text"] = translation_options_.at(index);
      candidate["highlighted"] = index - start == translation_cursor_;
      candidate["source"] = 5;
      candidate["fixed_position"] = 0;
      candidate["annotation"] = "";
      candidate["id"] = {{"session", session_},
                          {"generation", overlay.value("generation", uint64_t{})},
                          {"index", index}};
      overlay["candidates"].push_back(std::move(candidate));
    }
    view_ = std::move(overlay);
    render();
  }
  void commitTranslationCandidate(size_t index) {
    if (!translation_candidates_active_ || index >= translation_options_.size()) return;
    const auto text = translation_options_.at(index);
    exitTranslationCandidates();
    commitText(text, msime::linux_host::TypingSource::Reply);
    command(MSIME_CANCEL);
  }
  void render();
  std::string candidateAux() const;
  bool removeCandidateSlot(size_t slot) {
    if (!ensure() || restricted() || privateInput() || !ic_.hasFocus()) return false;
    const auto candidates = view_.value("candidates", Json::array());
    if (!candidates.is_array() || slot >= candidates.size()) return false;
    const auto &candidate = candidates.at(slot);
    if (!candidate.is_object() ||
        !msime::linux_host::candidate_dictionary_removal_available(
            view_.value("scheme", 0u), candidate.value("source", 0u),
            candidate.value("text", std::string{}))) return false;
    const auto &id = candidate.value("id", Json::object());
    if (!id.is_object() || !id.contains("generation") || !id.contains("index")) return false;
    return apply(msime_client_remove_candidate(session_, id.at("generation"), id.at("index")));
  }
  bool resetCache() {
    if (!ensure() || restricted() || privateInput() || !ic_.hasFocus()) return false;
    return apply(msime_client_reset_cache(session_));
  }
  // The traditional-output conversion is for simplified Chinese text only (`script_conversion_applies`): Japanese (kana and the kanji the Engine chose) and Korean (Hangul and the Hanja the user picks) pass through as they are, Cantonese and Zhuyin are written in traditional characters already, and Vietnamese is not Chinese. The candidate rows, the commit and the status action all ask this one gate, so a row never shows a character other than the one it commits (s2t would draw the Hanja 后 as 後).
  bool scriptConversionApplies() const {
    return msime::linux_host::scheme::ScriptConversionApplies(view_.value("scheme", 0));
  }
  bool traditionalApplies() const {
    return traditional_ && scriptConversionApplies();
  }
  bool toggleTraditional() {
    if (!session_ || !scriptConversionApplies()) return false;
    traditional_ = !traditional_;
    preferences_["traditional_chinese_output"] = traditional_;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["traditional_chinese_output"] = traditional_;
    saveBooleanPreference("traditional_chinese_output", traditional_);
    render();
    return true;
  }
  // 裸修饰键的识别不能只看 keysym。xkb 选项会改写修饰键本身的符号：本机默认带的
  // shift:both_capslock_cancel（两个 Shift 一起按切大写锁定）就把 Shift 键的 keysym
  // 变成了 Caps_Lock，于是按 sym == Shift_L 比较永远不中，四个模式快捷键在这种布局下
  // 全是死的。键码是布局无关的：X11 键码 50/62 是左右 Shift，37/105 是左右 Ctrl
  // （evdev 键码加 8），两个条件取或。
  static bool isShiftKey(const fcitx::KeyEvent &event) {
    const auto sym = event.key().sym();
    const auto code = event.key().code();
    return sym == FcitxKey_Shift_L || sym == FcitxKey_Shift_R || code == 50 || code == 62;
  }
  static bool isCtrlKey(const fcitx::KeyEvent &event) {
    const auto sym = event.key().sym();
    const auto code = event.key().code();
    return sym == FcitxKey_Control_L || sym == FcitxKey_Control_R || code == 37 || code == 105;
  }
  bool key(fcitx::KeyEvent &event);
  // Characters the IME hands back to the application are still typed text: Windows counts them in the statistics (ShouldCountPassthroughChar), so English-mode letters and Chinese-mode keys the Engine declines show up in the daily totals. Keys the IME consumed already recorded their committed text.
  void countPassthroughKey(const fcitx::KeyEvent &event) const {
    if (event.isRelease() || !ic_.hasFocus() || restricted() || !fcitx_typing_statistics.enabled()) return;
    const auto states = event.key().states();
    msime::linux_host::PassthroughModifiers held;
    held.control = states.test(fcitx::KeyState::Ctrl);
    held.alt = states.test(fcitx::KeyState::Alt);
    held.super = states.test(fcitx::KeyState::Super);
    held.hyper = states.test(fcitx::KeyState::Hyper);
    held.meta = states.test(fcitx::KeyState::Meta);
    const auto character =
        static_cast<char32_t>(fcitx::Key::keySymToUnicode(event.key().sym()));
    if (!msime::linux_host::should_count_passthrough_character(character, held)) return;
    recordTypingStatistics(fcitx::utf8::UCS4ToUTF8(character),
                           input_enabled_ ? typingSource()
                                          : msime::linux_host::TypingSource::English);
  }
  // Background music may play while this input method is active in a focused field that is not a secure one, and not while a recording would pick it up. The player serves the whole fcitx5 process, so this only tells it about a change (KeySound.h); close() tells it the music is over before the session goes away.
  void syncMusic() {
    music_.sync(session_, ic_.hasFocus() && !restricted() && !privateInput() && !voice_loading_,
                msime_client_music_set_active);
  }
  // The key sound of one press: every typing key while Chinese input is on in a field that is not a secure one, whether the Engine or the application takes the key, and none while a recording is running. This runs inside the fcitx5 daemon, so it only posts a request: the Host API opens no audio device and starts no thread until it finds a sound switched on, and an audio failure turns sound off for the process with one line on stderr instead of reaching this addon (msime_client.h).
  void playKeySound(const fcitx::KeyEvent &event) {
    syncMusic();
    const auto sym = static_cast<std::uint32_t>(event.key().sym());
    if (event.isRelease()) key_repeat_.release(sym);
    if (!session_ || !input_enabled_ || !ic_.hasFocus() || restricted() || privateInput() ||
        voice_loading_)
      return;
    const bool shortcut = event.rawKey().states().testAny(fcitx::KeyStates{
        fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Super,
        fcitx::KeyState::Hyper, fcitx::KeyState::Meta});
    if (!msime::linux_host::key_press_sounds(event.isRelease(), event.key().isModifier(), shortcut))
      return;
    const auto keyClass = msime::linux_host::key_sound_class(sym);
    msime_client_key_sound(session_, keyClass);
    // The typing effect of the same press, from the same session: Linux shows only the combo count, in the candidate aux line (setAuxDown; the voice, emoji search and configuration notices own setAuxUp). The key was rendered before this point, so a count that moved redraws that line alone.
    const auto combo = msime::linux_host::typing_effect_combo(msime_client_typing_effect(
        session_, keyClass | (key_repeat_.press(sym) ? msime::linux_host::kTypingEffectRepeat : 0)));
    if (combo == typing_combo_) return;
    typing_combo_ = combo;
    if (view_.is_object() && view_.contains("candidates") && !view_.at("candidates").empty()) {
      ic_.inputPanel().setAuxDown(fcitx::Text(candidateAux()));
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
  }
  // What the process's sound player was last told about background music; see syncMusic.
  msime::linux_host::MusicActivity music_;
  // The combo count the session last answered msime_client_typing_effect with, and the key held down, so an auto-repeat is drawn but not counted.
  std::uint32_t typing_combo_ = 0;
  msime::linux_host::KeyRepeat key_repeat_;
  // Every key event MSIME receives passes through here before key() routes it, so the heatmap counts keys the IME consumes for a composition as well as keys it hands back to the application. Only key downs count, once per physical press; a raw code below 8 has no evdev key behind it (a synthetic event, such as a panel's typed key).
  void countKeyPress(const fcitx::KeyEvent &event) {
    const auto code = event.rawKey().code();
    if (code < 8) return;
    const auto evdev = static_cast<uint32_t>(code - 8);
    // The front end's event time (X server or Wayland milliseconds) tells a synthetic repeat pair apart exactly; a front end that sends none leaves 0, and then the arrival time stands in for it.
    const auto now = static_cast<int64_t>(fcitx::now(CLOCK_MONOTONIC));
    const bool stamped = event.time() != 0;
    const auto at = stamped ? static_cast<int64_t>(static_cast<uint32_t>(event.time())) * 1000 : now;
    if (event.isRelease()) {
      key_presses_.up(evdev, at);
      return;
    }
    // With statistics off nothing is buffered; restricted (password, number) and private contexts are never counted, the same contexts commits are not recorded in.
    if (!fcitx_typing_statistics.enabled() || !ic_.hasFocus() || restricted() || privateInput()) return;
    const auto id = key_presses_.down(evdev, at,
                                      stamped ? msime::linux_host::KeyPressCounter::kEventRepeatGapMicroseconds
                                              : msime::linux_host::KeyPressCounter::kArrivalRepeatGapMicroseconds);
    if (id.empty() || options_path_.empty() || options_path_.front() != '/') return;
    const auto day = msime::linux_host::local_day(std::time(nullptr));
    if (day.empty()) return;
    flushKeyPresses(key_presses_.add(id, options_path_, day, now));
  }
  // Writes the pending key presses now, for FcitxEngine's teardown.
  void flushPendingKeyPresses() { flushKeyPresses(key_presses_.take()); }
  // Sends one batch to the store's record_keys operation on the calling thread.
  static void writeKeyPresses(const msime::linux_host::KeyPressBatch &pending) {
    try {
      const auto request = Json{
          {"directory", pending.directory},
          {"action", Json{{"operation", "record_keys"}, {"day", pending.day}, {"keys", pending.keys}}}}
                               .dump();
      if (auto *raw = msime_client_typing_statistics(
              reinterpret_cast<const uint8_t *>(request.data()), request.size()))
        msime_client_string_free(raw);
    } catch (...) {
      // Statistics are best effort and must never affect typing.
    }
  }
  // Writes a batch of key press counts on its own thread, as recordTypingStatistics does for commits, or on the loop once the addon is unloading (see ~FcitxEngine).
  static void flushKeyPresses(std::optional<msime::linux_host::KeyPressBatch> batch) {
    // Presses counted before statistics were turned off are dropped rather than sent; the store would not write them either.
    if (!batch || !fcitx_typing_statistics.enabled()) return;
    if (fcitx_key_presses_shutting_down) {
      writeKeyPresses(*batch);
      return;
    }
    fcitx_key_press_writes.begin();
    std::thread([pending = std::move(*batch)] {
      writeKeyPresses(pending);
      fcitx_key_press_writes.end();
    }).detach();
  }
  uint64_t session_ = 0;
  Json view_ = Json::object();
  Json preferences_ = Json::object();
  Json navigation_ = Json::object();
  std::string options_path_;
  // Per-key press counts for the key heatmap, written in batches; see KeyPressCounter.
  msime::linux_host::KeyPressCounter key_presses_;
  std::string dictionary_user_data_;
  std::string resources_;
  std::optional<std::string> scheme_override_;
  // The language dictionaries the runtime options named when this context last read them, which decide whether Cantonese and Zhuyin can run (noteSchemeOptions).
  msime::linux_host::LanguageDictionaryAvailability scheme_dictionaries_;
  bool caps_lock_ = false;
  std::string mode_indicator_label_;
  std::optional<std::string> shuangpin_profile_override_;
  std::optional<std::string> helpcode_schema_override_;
  // Set while the matching override's own status-bar save has not reached the store.
  bool scheme_unsaved_ = false;
  bool shuangpin_profile_unsaved_ = false;
  bool helpcode_schema_unsaved_ = false;
  CandidateSkinCatalog candidate_skin_catalog_;
  // The catalogue as runtime-options.json carries it, palettes included: an installed skin's colours are read from here when the classic UI theme is built.
  Json candidate_skin_document_;
  Json preferences_snapshot_;
  uint64_t preferences_job_session_ = 0;
  std::shared_future<Json> preferences_job_;
  std::shared_future<Json> preferences_save_job_;
  std::optional<PendingPreferenceSave> preferences_save_retry_;
  std::unique_ptr<fcitx::EventSourceTime> preferences_timer_;
  fcitx::InputContext &ic_;
  FcitxEngine *engine_;
  bool private_ = false;
  bool traditional_ = false;
  bool chinese_punctuation_ = true;
  // What the session was last told; see syncSessionChinesePunctuation().
  bool session_chinese_punctuation_ = true;
  // The width the session was last told; see syncSessionCharacterWidth(). A new session starts halfwidth.
  bool session_fullwidth_ = false;
  // Monotonic per session; see effectiveContextSnapshot().
  uint64_t applied_preferences_revision_ = 0;
  bool paired_punctuation_ = true;
  // Closing marks this host inserted after the caret, innermost last; whether the last apply() completed a pair.
  msime::linux_host::PairedPunctuationTracker paired_tracker_;
  // Quote and book-title state for Chinese marks committed in English mode under the Chinese punctuation lock or after Ctrl+.; reset on every mode switch.
  msime::linux_host::EnglishPunctuationState english_punctuation_;
  // Ctrl+. pressed in English mode under the "follow" lock: English mode types Chinese punctuation until the next Chinese/English switch, as Windows does with its punctuation compartment on and the IME closed. Session-only and kept apart from chinese_punctuation_, which a preference refresh re-derives from the saved preference.
  bool english_chinese_punctuation_ = false;
  bool pair_inserted_ = false;
  // Japanese converts with Space and commits with Enter; see ../src/core/JapaneseConversion.h.
  msime::linux_host::JapaneseConversion japanese_conversion_;
  msime::linux_host::BackspaceHoldPolicy backspace_hold_;
  bool smart_punctuation_ = true;
  bool smart_punctuation_repeat_ = true;
  bool smart_punctuation_space_convert_ = false;
  // The ASCII mark smart punctuation just kept, and when. Pressing the same key
  // again inside the window replaces it with the Chinese one. `rejected` is the
  // other half of that gesture: after the ASCII mark is backspaced away,
  // retyping the same key must reach Engine's Chinese table instead of being
  // kept as ASCII a second time.
  char last_smart_punctuation_ = 0;
  std::chrono::steady_clock::time_point last_smart_punctuation_at_{};
  char smart_punctuation_rejected_ = 0;
  // The ASCII key whose Chinese mark Engine just committed with nothing
  // composing, and the character that stood in front of it at that moment. A
  // bare Space arriving next takes the mark back to ASCII; any other key
  // disarms. The fingerprint is there because the same mark usually appears more
  // than once and moving the caret inside a window is not a focus change.
  std::string space_convert_mark_;
  std::string space_convert_preceding_;
  uint8_t punctuation_lock_ = 0;
  std::string online_socket_, online_query_;
  uint64_t online_job_session_ = 0;
  struct OnlineSlot {
    std::shared_future<Json> job;
    std::string query;
    uint64_t epoch = 0;
  } online_slots_[2];
  uint64_t online_epoch_ = 0;
  std::chrono::steady_clock::time_point online_due_{};
  std::chrono::steady_clock::time_point ai_due_{};
  std::string translation_query_, translation_pending_, translation_socket_;
  bool translation_manual_sentence_ = false;
  std::chrono::steady_clock::time_point translation_due_{};
  uint64_t translation_session_ = 0;
  std::shared_future<Json> translation_job_;
  std::string clipboard_path_;
  Json clipboard_items_ = Json::array();
  uint64_t clipboard_generation_ = 0;
  bool clipboard_loading_ = false;
  std::shared_future<Json> clipboard_job_;
  std::shared_future<Json> clipboard_mutation_job_;
  std::string cloud_clipboard_socket_;
  uint64_t cloud_clipboard_generation_ = 0;
  Json cloud_clipboard_items_ = Json::array();
  bool cloud_clipboard_enabled_ = true;
  std::shared_future<Json> cloud_clipboard_job_;
  Json emoji_items_ = Json::array();
  std::shared_future<Json> emoji_job_;
  std::string emoji_job_query_;
  bool emoji_search_mode_ = false;
  std::string emoji_search_;
  std::string emoji_category_;
  std::string emoji_group_;
  std::vector<std::string> emoji_groups_;
  std::shared_future<Json> emoji_groups_job_;
  uint64_t emoji_generation_ = 0;
  bool emoji_groups_loaded_ = false;
  size_t emoji_group_index_ = 0;
  // 已安装符号集插件的组（插件目录是 preferences_directory 下的 plugins），在切换目录、开始搜索和读取分组时重新读取，装卸插件后下次打开就能看到。
  std::vector<msime::linux_host::PluginSymbolGroup> emoji_plugin_groups_;
  bool emoji_plugins_stale_ = true;
  // 分组循环选中的插件组；选中内置分组或「全部」时为空。
  std::optional<msime::linux_host::PluginGroupKey> emoji_plugin_group_;
  msime::linux_host::EmojiPageCursor emoji_offset_;
  msime::linux_host::EmojiPageCursor emoji_next_offset_;
  bool emoji_complete_ = false;
  std::vector<msime::linux_host::EmojiPageCursor> emoji_previous_offsets_;
  std::string voice_socket_;
  std::string voice_language_ = "zh-cn";
  Json voice_options_ = Json::object();
  // The HostOptions document of the current session, for msime_client_voice_hotwords.
  Json voice_host_options_;
  bool voice_enabled_ = true;
  bool voice_hotkey_ctrl_f9_ = true;
  bool voice_hotkey_ralt_ = true;
  bool voice_hotkey_ctrl_win_ = false;
  bool voice_hotkey_rctrl_ralt_ = false;
  bool voice_hotkey_hold_space_lock_ = true;
  bool voice_ralt_held_ = false;
  bool voice_f9_held_ = false;
  bool maintenance_reload_held_ = false;
  // The key of the toggle chord being held (Ctrl+Space, Ctrl+Alt+Space, Ctrl+Shift+Space, Ctrl+Shift+F); FcitxKey_None when none is.
  fcitx::KeySym toggle_chord_held_ = FcitxKey_None;
  bool voice_ctrl_win_held_ = false;
  bool voice_rctrl_ralt_held_ = false;
  bool voice_space_consumed_ = false;
  bool voice_space_locked_ = false;
  uint64_t voice_failure_id_ = 0;
  bool voice_options_valid_ = true;
  bool voice_failure_visible_ = false;
  std::unique_ptr<fcitx::EventSourceTime> voice_failure_timer_;
  // Which context starts in Chinese, and which of the four configurable mode
  // chords this host answers. Fcitx5 had the CN/EN toggle on its status area
  // only: the settings page showed all four switches for this platform and none
  // of them did anything here, and a user who chose to start in English got
  // Chinese anyway.
  bool input_enabled_ = true;
  bool mode_shift_enabled_ = true;
  bool mode_ctrl_enabled_ = false;
  bool mode_ctrl_alt_space_enabled_ = true;
  bool character_set_shortcut_enabled_ = true;
  // A bare modifier switches on release, and only if nothing else was typed
  // while it was held. Windows measures the same gesture; so does the IBus host.
  bool ime_mode_chosen_ = false;
  bool mode_restore_pending_ = false;
  bool pure_shift_candidate_ = false;
  bool pure_ctrl_candidate_ = false;
  bool shift_down_ = false;
  bool shift_in_combination_ = false;
  bool ctrl_in_combination_ = false;
  fcitx::EventLoop *loop_ = nullptr;
#ifdef MSIME_FCITX5_MODE_BADGE
  std::unique_ptr<msime::linux_host::ModeBadgeSurface> mode_badge_;
  bool mode_badge_unavailable_ = false;
  std::unique_ptr<fcitx::EventSourceTime> mode_badge_timer_;
#endif
  bool ctrl_down_ = false;
  std::chrono::steady_clock::time_point modifier_toggle_deadline_{};
  std::shared_future<Json> voice_job_;
  std::shared_ptr<FcitxVoiceMailbox> voice_mailbox_;
  std::string voice_preedit_;
  std::string voice_transcript_;
  uint64_t voice_generation_ = 0;
  bool voice_partial_seen_ = false;
  bool voice_phase_seen_ = false;
  bool voice_level_seen_ = false;
  bool voice_loading_ = false;
  bool voice_cancelled_ = false;
  std::string voice_phase_ = "录音中";
  uint8_t voice_level_ = 0;
  msime::linux_host::WaveOverlayModel wave_overlay_;
  std::unique_ptr<msime::linux_host::WaveOverlaySurface> wave_overlay_surface_;
  bool wave_overlay_visible_ = false;
  bool wave_overlay_failed_ = false;
  bool system_dark_ = false;
  bool word_character_enabled_ = true;
  bool word_character_minus_equal_ = false;
  bool translation_candidates_active_ = false;
  Json translation_saved_view_ = Json::object();
  std::vector<std::string> translation_options_;
  size_t translation_page_ = 0;
  size_t translation_cursor_ = 0;
};

// The row as the panel draws it.
fcitx::Text candidateRowText(const Json &candidate, bool traditional, bool annotations,
                             const std::string &hanjaGloss) {
  fcitx::Text row((traditional ? msime_linux_simplified_to_traditional(candidate.at("text").get<std::string>())
                               : candidate.at("text").get<std::string>()) +
      // Engine-corrected spellings carry the same light marker Windows and the IBus host draw. Only the displayed row gets it: selection goes by session/generation/index, and text_ below, which the candidate actions (dictionary removal) read, stays the Engine's text.
      (candidate.value("corrected", false) ? "*" : "") +
      (candidate.value("source", 0u) == 2 ? "  ☁️" :
       candidate.value("source", 0u) == 3 ? "  🤖" : "") +
      (!hanjaGloss.empty() || !annotations || candidate.value("annotation", std::string()).empty() ? "" :
       "  " + candidate.at("annotation").get<std::string>()));
  // A Hanja row's 훈음 takes the translation's place after the candidate whatever the translation settings say, and the classic UI sets it in italics, so it reads as the row's secondary gloss rather than as part of the Hanja; a Fcitx5 panel has no second line for it. It is display text only, which DontCommit states as well: the row is chosen by index.
  if (!hanjaGloss.empty())
    row.append("  " + hanjaGloss, fcitx::TextFormatFlags{fcitx::TextFormatFlag::Italic,
                                                         fcitx::TextFormatFlag::DontCommit});
  if (candidate.contains("translation") && candidate.at("translation").is_string())
    row.append("  " + candidate.at("translation").get<std::string>());
  return row;
}

class FcitxCandidate : public fcitx::CandidateWord {
public:
  FcitxCandidate(fcitx::FactoryFor<FcitxState> *factory, const Json &candidate, bool traditional,
                 bool annotations, const std::string &hanjaGloss)
      : CandidateWord(candidateRowText(candidate, traditional, annotations, hanjaGloss)), factory_(factory),
        session_(candidate.at("id").at("session")), generation_(candidate.at("id").at("generation")),
        index_(candidate.at("id").at("index")), source_(candidate.value("source", 0u)),
        text_(candidate.at("text").get<std::string>()),
        fixed_position_(candidate.value("fixed_position", 0u)) {}
  void select(fcitx::InputContext *ic) const override {
    try { ic->propertyFor(factory_)->select(session_, generation_, index_); } catch (...) {}
  }
  uint64_t session() const { return session_; }
  uint64_t generation() const { return generation_; }
  size_t index() const { return index_; }
  uint64_t source() const { return source_; }
  const std::string &text() const { return text_; }
  uint8_t fixedPosition() const { return fixed_position_; }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  uint64_t session_, generation_;
  size_t index_;
  uint64_t source_;
  std::string text_;
  uint8_t fixed_position_;
};

// The runtime already pages candidates. Never page its current page a second time.
class FcitxPage : public fcitx::CandidateList,
                  public fcitx::PageableCandidateList
#ifdef MSIME_FCITX_ACTIONS
                  , public fcitx::ActionableCandidateList
#endif
{
public:
  FcitxPage(FcitxState &state, fcitx::FactoryFor<FcitxState> *factory) : state_(state),
      session_(state.session_), generation_(state.view_.at("generation")),
      page_(state.view_.at("page")), pages_(state.view_.at("page_count")),
      layout_(state.preferences_.value("candidate_layout", std::string("vertical")) == "horizontal"
          ? fcitx::CandidateLayoutHint::Horizontal : fcitx::CandidateLayoutHint::Vertical) {
    const bool annotations = state.showCandidateAnnotations();
    setPageable(this);
#ifdef MSIME_FCITX_ACTIONS
    setActionable(this);
#endif
    for (const auto &candidate : state.view_.at("candidates")) {
      if (candidate.value("highlighted", false)) cursor_ = words_.size();
      words_.push_back(std::make_unique<FcitxCandidate>(
          factory, candidate, state.traditionalApplies(), annotations,
          msime::linux_host::korean_hanja_gloss(state.view_, candidate)));
      labels_.emplace_back(std::to_string(words_.size()) + ". ");
    }
  }
  const fcitx::Text &label(int index) const override { return labels_.at(index); }
  const fcitx::CandidateWord &candidate(int index) const override { return *words_.at(index); }
  int size() const override { return words_.size(); }
  int cursorIndex() const override { return cursor_; }
  fcitx::CandidateLayoutHint layoutHint() const override { return layout_; }
  bool hasPrev() const override { return page_ > 0; }
  bool hasNext() const override { return page_ + 1 < pages_; }
  bool usedNextBefore() const override { return page_ > 0; }
  int totalPages() const override { return pages_; }
  int currentPage() const override { return page_; }
  void prev() override { move(MSIME_PREVIOUS_PAGE); }
  void next() override { move(MSIME_NEXT_PAGE); }
#ifdef MSIME_FCITX_ACTIONS
  bool hasAction(const fcitx::CandidateWord &candidate) const override {
    const auto *item = dynamic_cast<const FcitxCandidate *>(&candidate);
    if (state_.translationCandidatesActive() || !item) return false;
    if (!state_.ic_.hasFocus() || !state_.input_enabled_ || state_.restricted() ||
        state_.privateInput() || state_.session_ != item->session() ||
        state_.view_.value("generation", uint64_t{}) != item->generation())
      return false;
    return msime::linux_host::candidate_dictionary_actions_available(state_.view_.value("scheme", 0u), item->source());
  }
  std::vector<fcitx::CandidateAction>
  candidateActions(const fcitx::CandidateWord &candidate) const override {
    std::vector<fcitx::CandidateAction> actions;
    if (state_.translationCandidatesActive()) return actions;
    const auto *item = dynamic_cast<const FcitxCandidate *>(&candidate);
    if (!item) return actions;
    if (!state_.ic_.hasFocus() || !state_.input_enabled_ || state_.restricted() ||
        state_.privateInput() || state_.session_ != item->session() ||
        state_.view_.value("generation", uint64_t{}) != item->generation()) return actions;
    const auto scheme = state_.view_.value("scheme", 0u);
    if (!msime::linux_host::candidate_dictionary_actions_available(scheme, item->source()))
      return actions;
    const auto make = [](int id, const char *text) {
      fcitx::CandidateAction action;
      action.setId(id);
      action.setText(text);
      return action;
    };
    actions.push_back(make(1, msime::linux_host::candidate_pin_label));
    const auto source = item->source();
    const auto fixedPosition = item->fixedPosition();
    if (msime::linux_host::candidate_dictionary_removal_available(
            scheme, source,
            item->text()))
      actions.push_back(make(2, "删除候选"));
    for (int slot = 1; slot <= 5; ++slot)
      actions.push_back(make(10 + slot, msime::linux_host::candidate_fix_label(slot).c_str()));
    if (fixedPosition > 0) actions.push_back(make(20, "取消固定"));
    return actions;
  }
  void triggerAction(const fcitx::CandidateWord &candidate, int action) override {
    const auto *item = dynamic_cast<const FcitxCandidate *>(&candidate);
    if (!item) return;
    // ensure() can clear the panel and destroy this page and its candidate.
    auto *state = &state_;
    const auto session = item->session();
    const auto generation = item->generation();
    const auto index = item->index();
    const auto actions = candidateActions(candidate);
    if (std::none_of(actions.begin(), actions.end(),
        [action](const auto &available) { return available.id() == action; })) return;
    try {
      if (!state->ensure() || state->session_ != session ||
          state->view_.value("generation", uint64_t{}) != generation) return;
      char *raw = nullptr;
      if (action == 1) raw = msime_client_pin_candidate(session, generation, index);
      else if (action == 2) raw = msime_client_remove_candidate(session, generation, index);
      else if (action >= 11 && action <= 15)
        raw = msime_client_fix_candidate_position(session, generation, index, static_cast<uint8_t>(action - 10));
      else if (action == 20) raw = msime_client_clear_candidate_position(session, generation, index);
      if (raw) state->apply(raw);
    } catch (...) {}
  }
#endif
private:
  void move(uint32_t command) {
    // render() replaces this list. Do not access members after dispatch.
    auto *state = &state_;
    if (state->translationCandidatesActive()) {
      // A stale Fcitx candidate list must not page a newer translation overlay.
      // Rendering replaces this list, but Fcitx may still dispatch an already
      // queued pageable callback after the replacement.
      if (state->session_ != session_ ||
          state->view_.value("generation", uint64_t{}) != generation_)
        return;
      state->translationPage(command);
      return;
    }
    const auto session = session_;
    const auto generation = generation_;
    try {
      if (state->ensure() && state->session_ == session &&
          state->view_.value("generation", uint64_t{}) == generation)
        state->command(command);
    } catch (...) {}
  }
  FcitxState &state_;
  uint64_t session_, generation_;
  int page_, pages_, cursor_ = -1;
  fcitx::CandidateLayoutHint layout_;
  std::vector<std::unique_ptr<FcitxCandidate>> words_;
  std::vector<fcitx::Text> labels_;
};

// Status actions are shared by the addon, but their values belong to the
// supplied context. Never cache one application's checked state globally.
class FcitxModeAction : public fcitx::Action {
public:
  enum class Mode { EnglishCandidates, Fullwidth };
  FcitxModeAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::EnglishCandidates ? "英文输入模式" : "全角字符";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    return mode_ == Mode::EnglishCandidates
        ? state->view_.value("dedicated_english", false)
        : state->view_.value("character_width", std::string{}) == "Fullwidth";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted()) return;
    try {
      if (!state->ensure()) return;
      if (!state->view_.value("editing_text", std::string{}).empty())
        state->command(MSIME_COMMIT_RAW);
      if (mode_ == Mode::EnglishCandidates) state->toggleEnglish();
      else state->toggleWidth();
      update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Mode mode_;
};

class FcitxInputModeAction : public fcitx::Action {
public:
  explicit FcitxInputModeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "中文"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    return ic && ic->propertyFor(factory_)->session_ && ic->propertyFor(factory_)->input_enabled_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->ensure() && state->toggleInputMode()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxSchemeAction : public fcitx::SimpleAction {
public:
  explicit FcitxSchemeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "输入方案";
    const auto scheme = ic->propertyFor(factory_)->view_.value("scheme", 0u);
    switch (scheme) {
    case 1: return "输入方案：双拼";
    case 2: return "输入方案：五笔";
    case 3: return "输入方案：日文";
    case 4: return "输入方案：韩文";
    case 5: return "输入方案：粤拼";
    case 6: return "输入方案：注音";
    case 7: return "输入方案：越南文";
    default: return "输入方案：全拼";
    }
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  // Front ends open the scheme menu instead of activating an action that has one; stepping stays for a caller that activates it directly.
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleScheme()) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

// One scheme in the 输入方案 menu, checked while the session types with it.
class FcitxSchemeItemAction : public fcitx::Action {
public:
  FcitxSchemeItemAction(fcitx::FactoryFor<FcitxState> *factory, unsigned index, const char *label)
      : factory_(factory), index_(index), label_(label) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return label_; }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->view_.value("scheme", 0u) == index_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus() || isChecked(ic)) return;
    try {
      auto *state = ic->propertyFor(factory_);
      // selectScheme refuses a scheme whose dictionary was removed after the menu last listed it.
      if (state->selectScheme(FcitxState::kSchemes[index_])) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  unsigned index_;
  const char *label_;
};

class FcitxShuangpinProfileAction : public fcitx::SimpleAction {
public:
  explicit FcitxShuangpinProfileAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "双拼方案";
    const auto *state = ic->propertyFor(factory_);
    const auto current = state->preferences_.value("shuangpin_profile", std::string("xiaohe"));
    for (const auto &profile : msime::linux_host::kShuangpinProfileNames)
      if (current == profile.value) return std::string("双拼方案：") + profile.label;
    return "双拼方案";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleShuangpinProfile()) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxNineKeyAction : public fcitx::SimpleAction {
public:
  explicit FcitxNineKeyAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  std::string shortText(fcitx::InputContext *) const override { return "九键"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->view_.value("scheme", 0u) == 0 &&
           state->view_.value("nine_key", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->view_.value("scheme", 0u) == 0) {
        state->toggleNineKey();
        update(ic);
      }
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxNineKeySpellingAction : public fcitx::SimpleAction {
public:
  FcitxNineKeySpellingAction(fcitx::FactoryFor<FcitxState> *factory, size_t index)
      : factory_(factory), index_(index) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) {
      const auto *state = ic->propertyFor(factory_);
      const auto spellings = state->view_.value("nine_key_spellings", Json::array());
      if (spellings.is_array() && index_ < spellings.size() && spellings.at(index_).is_string())
        return std::to_string(index_ + 1) + ". " + spellings.at(index_).get<std::string>();
    }
    return "九键拼写 " + std::to_string(index_ + 1);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->chooseNineKeySpelling(index_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  size_t index_;
};

class FcitxHelpcodeAction : public fcitx::Action {
public:
  explicit FcitxHelpcodeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "辅助码"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    const auto scheme = state->view_.value("scheme", 0u);
    if (scheme != 0 && scheme != 1) return false;
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    return state->preferences_.value(section, Json::object()).value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleHelpcode()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxSchemeBooleanAction : public fcitx::Action {
public:
  enum class Kind { ShuangpinPreedit, WubiCodeHint };
  FcitxSchemeBooleanAction(fcitx::FactoryFor<FcitxState> *factory, Kind kind)
      : factory_(factory), kind_(kind) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return kind_ == Kind::ShuangpinPreedit ? "双拼原始预编辑" : "五笔剩余编码";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    const auto scheme = state->view_.value("scheme", 0u);
    if (kind_ == Kind::ShuangpinPreedit && scheme != 1) return false;
    if (kind_ == Kind::WubiCodeHint && scheme != 2) return false;
    const auto key = kind_ == Kind::ShuangpinPreedit
        ? "shuangpin_preedit_uses_raw" : "wubi_code_hint";
    return state->preferences_.value(key, true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    const auto scheme = state->view_.value("scheme", 0u);
    if ((kind_ == Kind::ShuangpinPreedit && scheme != 1) ||
        (kind_ == Kind::WubiCodeHint && scheme != 2) ||
        state->restricted() || state->privateInput()) return;
    const auto key = kind_ == Kind::ShuangpinPreedit
        ? "shuangpin_preedit_uses_raw" : "wubi_code_hint";
    try {
      if (state->toggleTopLevelBoolean(key, true)) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Kind kind_;
};

class FcitxHelpcodeSchemaAction : public fcitx::SimpleAction {
public:
  explicit FcitxHelpcodeSchemaAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "辅助码方案";
    const auto *state = ic->propertyFor(factory_);
    const auto scheme = state->view_.value("scheme", 0u);
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    const auto value = state->preferences_.value(section, Json::object())
        .value("schema", scheme == 1 ? std::string("lantian") : std::string("ziranma"));
    return std::string("辅助码：") + std::string(msime::linux_host::helpcode_schema_label(value));
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleHelpcodeSchema()) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxAutocorrectAction : public fcitx::Action {
public:
  enum class Mode { Transposition, Neighbor };
  FcitxAutocorrectAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::Transposition ? "拼音错位纠错" : "拼音邻键纠错";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->view_.value("scheme", 0u) != 0) return false;
    const auto key = mode_ == Mode::Transposition ? "autocorrect_transposition" : "autocorrect_neighbor";
    return state->preferences_.value("quanpin", Json::object()).value(key, true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      const auto key = mode_ == Mode::Transposition ? "autocorrect_transposition" : "autocorrect_neighbor";
      if (state->ensure() && state->toggleQuanpinAutocorrect(key)) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Mode mode_;
};

class FcitxMixedEnglishAction : public fcitx::Action {
public:
  explicit FcitxMixedEnglishAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "混合英文"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("mixed_input", Json::object())
        .value("english", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleMixedEnglish()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxMixedCandidateAction : public fcitx::Action {
public:
  FcitxMixedCandidateAction(fcitx::FactoryFor<FcitxState> *factory, const char *key,
                            const char *label)
      : factory_(factory), key_(key), label_(label) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override { return label_; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("mixed_input", Json::object())
        .value(key_, false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleMixedCandidate(key_)) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  const char *key_;
  const char *label_;
};

class FcitxLocalModeAction : public fcitx::Action {
public:
  FcitxLocalModeAction(fcitx::FactoryFor<FcitxState> *factory, const char *key,
                       const char *label)
      : factory_(factory), key_(key), label_(label) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override { return label_; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("local_modes", Json::object())
        .value(key_, msime::linux_host::local_mode_enabled_by_default(key_));
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->toggleLocalMode(key_)) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  const char *key_;
  const char *label_;
};

class FcitxEnglishGlossAction : public fcitx::Action {
public:
  explicit FcitxEnglishGlossAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "英文释义"; }
  std::string icon(fcitx::InputContext *) const override { return "accessories-dictionary"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("candidate_english_gloss", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleEnglishGloss()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxWordCharacterAction : public fcitx::Action {
public:
  explicit FcitxWordCharacterAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "以词定字"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("word_character", Json::object())
        .value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleWordCharacter()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxNumberRowAction : public fcitx::Action {
public:
  explicit FcitxNumberRowAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "数字选词"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("number_row_selection", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->toggleTopLevelBoolean("number_row_selection", true)) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxPunctuationAction : public fcitx::Action {
public:
  enum class Mode { Chinese, Paired };
  FcitxPunctuationAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::Chinese ? "中文标点" : "成对标点";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    if (mode_ == Mode::Paired) return state->paired_punctuation_;
    // English mode types what its own flags say, not the saved chinese_punctuation preference.
    return state->input_enabled_ ? state->chinese_punctuation_
        : state->punctuation_lock_ == 1 ||
          (state->punctuation_lock_ == 0 && state->english_chinese_punctuation_);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted()) return;
    try {
      if (!state->ensure()) return;
      if (mode_ == Mode::Chinese && !state->input_enabled_) {
        // Same as Ctrl+. in English mode: session-only, and a pinned lock holds.
        if (state->punctuation_lock_ == 0)
          state->english_chinese_punctuation_ = !state->english_chinese_punctuation_;
      } else if (mode_ == Mode::Chinese) state->toggleChinesePunctuation();
      else state->togglePairedPunctuation();
      update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Mode mode_;
};

class FcitxSmartPunctuationAction : public fcitx::Action {
public:
  enum class Mode { Smart, Repeat };
  FcitxSmartPunctuationAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::Smart ? "智能标点" : "重复标点回切中文";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    const char *key = mode_ == Mode::Smart ? "smart_punctuation" : "smart_punctuation_repeat";
    return state->preferences_.value(key, true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      const char *key = mode_ == Mode::Smart ? "smart_punctuation" : "smart_punctuation_repeat";
      if (state->ensure() && state->toggleTopLevelBoolean(key, true)) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Mode mode_;
};

class FcitxCandidateLayoutAction : public fcitx::SimpleAction {
public:
  explicit FcitxCandidateLayoutAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("循环切换候选栏横向和纵向布局");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "候选布局";
    const auto *state = ic->propertyFor(factory_);
    return state->preferences_.value("candidate_layout", std::string("vertical")) == "horizontal"
        ? "候选：横向" : "候选：纵向";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->cycleCandidateLayout()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCandidateThemeAction : public fcitx::SimpleAction {
public:
  explicit FcitxCandidateThemeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "候选明暗";
    const auto theme = ic->propertyFor(factory_)->preferences_.value("candidate_theme", std::string("follow"));
    return theme == "light" ? "候选明暗：浅色" : theme == "dark" ? "候选明暗：深色" : "候选明暗：跟随颜色模式";
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleCandidateTheme()) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

// One entry of the 主题 menu: a global theme from the shared catalogue, which does not change while the process runs, or an installed skin package, which selects the custom theme drawn over it; the package entries are rebuilt when the catalogue changes (FcitxEngine::rebuildThemeMenu).
class FcitxGlobalThemeItemAction : public fcitx::Action {
public:
  FcitxGlobalThemeItemAction(fcitx::FactoryFor<FcitxState> *factory, std::string id, std::string title)
      : factory_(factory), id_(std::move(id)), title_(std::move(title)) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return title_; }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    return ic && ic->propertyFor(factory_)->currentThemeChoice() == id_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->setThemeChoice(id_)) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  std::string id_;
  std::string title_;
};

class FcitxGlobalThemeAction : public fcitx::SimpleAction {
public:
  explicit FcitxGlobalThemeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("选择候选窗口、菜单与工具栏的主题");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "主题";
    const auto *state = ic->propertyFor(factory_);
    const auto choices = state->themeChoices();
    const auto *current = msime::linux_host::find_theme_choice(choices, state->currentThemeChoice());
    return current ? "主题：" + current->title : "主题";
  }
  std::string icon(fcitx::InputContext *) const override { return "preferences-desktop-theme"; }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *) override {}
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCandidatePageSizeItemAction : public fcitx::SimpleAction {
public:
  FcitxCandidatePageSizeItemAction(fcitx::FactoryFor<FcitxState> *factory, uint8_t size)
      : factory_(factory), size_(size) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) {
      const auto *state = ic->propertyFor(factory_);
      if (state->view_.value("page_size", uint8_t{}) == size_)
        return std::to_string(size_) + " 个候选 ✓";
    }
    return std::to_string(size_) + " 个候选";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->setCandidatePageSize(size_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  uint8_t size_;
};

class FcitxCandidatePageSizeAction : public fcitx::SimpleAction {
public:
  FcitxCandidatePageSizeAction() {
    setShortText("候选数量");
    setLongText("选择每页显示的候选数量");
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *) override {}
};

class FcitxLearningAction : public fcitx::Action {
public:
  explicit FcitxLearningAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "学习用户词频"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("learning", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure()) {
        const bool enabled = !state->preferences_.value("learning", true);
        auto snapshot = state->preferences_snapshot_;
        if (!snapshot.is_object() || !snapshot.contains("preferences") ||
            !snapshot.contains("revision")) return;
        snapshot["preferences"]["learning"] = enabled;
        if (!state->applyPreferenceSnapshot(std::move(snapshot))) return;
        state->saveBooleanPreference("learning", enabled);
        state->render();
        update(ic);
      }
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxFrequencyAction : public fcitx::SimpleAction {
public:
  explicit FcitxFrequencyAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "词频调节";
    const auto mode = ic->propertyFor(factory_)->preferences_
        .value("frequency", Json::object()).value("mode", std::string("promote"));
    const auto label = mode == "disabled" ? "禁用" : mode == "pin" ? "固定" :
        mode == "halve" ? "减半" : mode == "linear" ? "线性" : "提升";
    return std::string("词频：") + label;
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleFrequencyMode()) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxFrequencyNumberAction : public fcitx::SimpleAction {
public:
  FcitxFrequencyNumberAction(fcitx::FactoryFor<FcitxState> *factory, const char *key,
                             const char *label)
      : factory_(factory), key_(key), label_(label) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    const auto value = ic ? ic->propertyFor(factory_)->preferences_
        .value("frequency", Json::object()).value(key_, 1u) : 1u;
    return std::string(label_) + "：" + std::to_string(value);
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->cycleFrequencyNumber(key_)) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  const char *key_;
  const char *label_;
};

class FcitxModeScopeAction : public fcitx::SimpleAction {
public:
  explicit FcitxModeScopeAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("循环切换应用级和全局输入模式");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "模式范围";
    const auto *state = ic->propertyFor(factory_);
    return state->preferences_.value("ime_mode_scope", std::string("app")) == "global"
        ? "模式：全局" : "模式：应用";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && state->cycleModeScope()) update(ic);
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCandidateTranslationAction : public fcitx::Action {
public:
  explicit FcitxCandidateTranslationAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override { return "显示译文"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("candidate_translations", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure()) {
        state->toggleCandidateTranslations();
        update(ic);
      }
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxSentenceTranslationAction : public fcitx::SimpleAction {
public:
  explicit FcitxSentenceTranslationAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) {
    setLongText("手动翻译当前首选候选句子");
  }
  std::string shortText(fcitx::InputContext *) const override { return "翻译当前句子"; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->session_ && !state->restricted() && !state->privateInput())
        state->translateSentence();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxPunctuationLockAction : public fcitx::SimpleAction {
public:
  explicit FcitxPunctuationLockAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("循环切换跟随、固定中文和固定英文标点");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "标点锁定";
    const auto *state = ic->propertyFor(factory_);
    switch (state->punctuation_lock_) {
    case 1: return "标点：中文";
    case 2: return "标点：英文";
    default: return "标点：跟随";
    }
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->session_ && !state->restricted() && !state->privateInput())
        state->cyclePunctuationLock();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxTranslationLanguageAction : public fcitx::SimpleAction {
public:
  explicit FcitxTranslationLanguageAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) { setLongText("循环切换候选翻译目标语言"); }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "翻译语言";
    const auto *state = ic->propertyFor(factory_);
    const auto language = state->preferences_.value("translation_target_language", std::string("en"));
    const std::array<std::pair<const char *, const char *>, 7> labels{{
        {"en", "翻译：英语"}, {"fr", "翻译：法语"}, {"ja", "翻译：日语"},
        {"es", "翻译：西班牙语"}, {"ru", "翻译：俄语"}, {"de", "翻译：德语"},
        {"ko", "翻译：韩语"}}};
    for (const auto &[value, label] : labels)
      if (language == value) return label;
    return "翻译语言";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->session_ && !state->restricted() && !state->privateInput())
        state->cycleTranslationLanguage();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCloudCandidatesAction : public fcitx::Action {
public:
  explicit FcitxCloudCandidatesAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "云联想"; }
  std::string icon(fcitx::InputContext *) const override { return "network-wireless"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("cloud_candidates", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure()) {
        state->toggleCloudCandidates();
        update(ic);
      }
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxAiCandidatesAction : public fcitx::Action {
public:
  explicit FcitxAiCandidatesAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "AI 联想"; }
  std::string icon(fcitx::InputContext *) const override { return "applications-science"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("ai_assistant", Json::object())
        .value("enabled", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure()) {
        state->toggleAiCandidates();
        update(ic);
      }
    } catch (...) {
      state->close();
      state->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxMaintenanceAction : public fcitx::SimpleAction {
public:
  FcitxMaintenanceAction(fcitx::FactoryFor<FcitxState> *factory, int operation,
                         const char *text)
      : factory_(factory), operation_(operation) {
    setShortText(text);
    setLongText(text);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->maintenance(operation_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  int operation_;
};

class FcitxReloadServiceAction : public fcitx::SimpleAction {
public:
  explicit FcitxReloadServiceAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) {
    setShortText("重载输入法服务");
    setLongText("重置水杉输入法：关闭所有输入会话并重新读取运行配置");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state) state->reloadService();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxDesktopPanelAction : public fcitx::SimpleAction {
public:
  FcitxDesktopPanelAction(fcitx::FactoryFor<FcitxState> *factory,
                          const char *panel, const char *text)
      : panel_(panel), factory_(factory) {
    setShortText(text);
    setLongText(text);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (!state || state->restricted() ||
          (std::strcmp(panel_, "voice") == 0 && !state->voice_enabled_) ||
          !ic->hasFocus()) return;
      launchDesktopPanel(panel_);
    } catch (...) {}
  }
private:
  const char *panel_;
  fcitx::FactoryFor<FcitxState> *factory_;
};

// The shared `floating_toolbar` preference and its eight component switches.
//
// Windows draws a floating window; the IBus host maps the same switches onto a
// property submenu because a window detached from the input context is not
// something an IBus engine owns. Fcitx5's status area is that surface here, so
// the switches decide what a "工具栏" submenu contains - and they decided nothing
// at all before, while the settings page showed all of them for this platform.
//
// The entries are the existing actions rather than copies: an entry that behaved
// slightly differently from the status-area action beside it would be a second
// implementation of the same toggle.
class FcitxToolbarAction : public fcitx::SimpleAction {
public:
  FcitxToolbarAction() {
    setShortText("工具栏");
    setLongText("按设置显示的输入法工具栏项目");
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *) override {}
};

class FcitxToolbarEnabledAction : public fcitx::Action {
public:
  explicit FcitxToolbarEnabledAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override { return "工具栏"; }
  std::string icon(fcitx::InputContext *) const override { return "view-restore"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("floating_toolbar", Json::object())
        .value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->toggleFloatingToolbar()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxVoiceEnabledAction : public fcitx::Action {
public:
  explicit FcitxVoiceEnabledAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override { return "启用语音输入"; }
  std::string icon(fcitx::InputContext *) const override { return "audio-input-microphone"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->voice_enabled_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->toggleVoiceEnabled()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxDesktopToolsAction : public fcitx::SimpleAction {
public:
  FcitxDesktopToolsAction() {
    setShortText("桌面工具");
    setLongText("打开手写、Emoji、剪贴板和帮助等桌面工具");
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *) override {}
};

// A status entry that only opens a menu of existing actions: the option groups of the design menu. The actions inside are the same objects the status area used to list, so each keeps its registered name and behaviour.
class FcitxMenuGroupAction : public fcitx::SimpleAction {
public:
  FcitxMenuGroupAction(const char *text, const char *description) {
    setShortText(text);
    setLongText(description);
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *) override {}
};

// A rule between parts of a menu. classicui and the StatusNotifierItem menu draw it; kimpanel leaves it out.
class FcitxMenuSeparatorAction : public fcitx::SimpleAction {
public:
  FcitxMenuSeparatorAction() { setSeparator(true); }
};

class FcitxPreferenceSaveRetryAction : public fcitx::SimpleAction {
public:
  explicit FcitxPreferenceSaveRetryAction(fcitx::FactoryFor<FcitxState> *factory)
      : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) {
      const auto *state = ic->propertyFor(factory_);
      if (state && state->preferences_save_job_.valid()) return "正在保存设置…";
      if (state && state->preferences_save_retry_) return "重试保存设置";
    }
    return "保存设置";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state && state->retryPreferenceSave()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxClipboardAction : public fcitx::SimpleAction {
public:
  explicit FcitxClipboardAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("剪贴板");
    setLongText("插入最近的剪贴板历史");
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->pasteClipboard(); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxClipboardHistoryAction : public fcitx::Action {
public:
  explicit FcitxClipboardHistoryAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "剪贴板历史"; }
  std::string icon(fcitx::InputContext *) const override { return "edit-paste"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("clipboard_history", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->ensure() && state->toggleClipboardHistory()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxClipboardItemAction : public fcitx::SimpleAction {
public:
  FcitxClipboardItemAction(fcitx::FactoryFor<FcitxState> *factory, size_t index)
      : factory_(factory), index_(index) { setLabel("剪贴板 " + std::to_string(index + 1)); }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) {
      const auto *state = ic->propertyFor(factory_);
      if (index_ < state->clipboard_items_.size()) {
        const auto &item = state->clipboard_items_.at(index_);
        const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
        if (!text.empty()) {
          return panelPreview(text);
        }
      }
    }
    return "剪贴板 " + std::to_string(index_ + 1);
  }
  void setLabel(const std::string &label) { setShortText(label); setLongText(label); }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->pasteClipboard(index_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  size_t index_;
};

class FcitxClipboardRemoveAction : public fcitx::SimpleAction {
public:
  FcitxClipboardRemoveAction(fcitx::FactoryFor<FcitxState> *factory, size_t index)
      : factory_(factory), index_(index) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic && index_ < ic->propertyFor(factory_)->clipboard_items_.size())
      return "删除 " + std::to_string(index_ + 1);
    return "删除剪贴板 " + std::to_string(index_ + 1);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->removeClipboard(index_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  size_t index_;
};

class FcitxClipboardClearAction : public fcitx::SimpleAction {
public:
  explicit FcitxClipboardClearAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("清空历史");
    setLongText("删除全部本地剪贴板历史");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->clearClipboard(); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCloudClipboardAction : public fcitx::SimpleAction {
public:
  explicit FcitxCloudClipboardAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("云剪贴板");
    setLongText("读取云剪贴板最近条目");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->requestCloudClipboard(); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxCloudClipboardItemAction : public fcitx::SimpleAction {
public:
  FcitxCloudClipboardItemAction(fcitx::FactoryFor<FcitxState> *factory, size_t index)
      : factory_(factory), index_(index) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    // Cloud text is never previewed in a password or private field, even when it was fetched before the field changed.
    if (ic && !ic->propertyFor(factory_)->restricted() && !ic->propertyFor(factory_)->privateInput() &&
        index_ < ic->propertyFor(factory_)->cloud_clipboard_items_.size()) {
      const auto &item = ic->propertyFor(factory_)->cloud_clipboard_items_.at(index_);
      const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
      if (!text.empty()) {
        return panelPreview(text);
      }
    }
    return "云剪贴板 " + std::to_string(index_ + 1);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->pasteCloudClipboard(index_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  size_t index_;
};

class FcitxEmojiAction : public fcitx::SimpleAction {
public:
  explicit FcitxEmojiAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("表情");
    setLongText("浏览并插入本地表情目录");
  }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->insertEmoji(); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxEmojiSearchAction : public fcitx::SimpleAction {
public:
  explicit FcitxEmojiSearchAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("搜索 Emoji");
    setLongText("在本地 Emoji、颜文字和符号目录中搜索");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->ensure()) state->beginEmojiSearch();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxEmojiCategoryAction : public fcitx::SimpleAction {
public:
  explicit FcitxEmojiCategoryAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("循环切换 Emoji、颜文字和符号目录");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "表情类别";
    const auto category = ic->propertyFor(factory_)->emoji_category_;
    if (category == "kaomoji") return "表情：颜文字";
    if (category == "symbols") return "表情：符号";
    return "表情：Emoji";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->ensure()) state->cycleEmojiCategory();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxEmojiGroupAction : public fcitx::SimpleAction {
public:
  explicit FcitxEmojiGroupAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setLongText("循环切换当前 Emoji 目录的分组");
  }
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return "表情分组";
    const auto *state = ic->propertyFor(factory_);
    return state->emoji_group_.empty() ? "表情：全部" : "表情：" + state->emoji_group_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->ensure()) state->cycleEmojiGroup();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxEmojiItemAction : public fcitx::SimpleAction {
public:
  FcitxEmojiItemAction(fcitx::FactoryFor<FcitxState> *factory, size_t index)
      : factory_(factory), index_(index) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) {
      const auto *state = ic->propertyFor(factory_);
      if (index_ < state->emoji_items_.size()) {
        const auto &item = state->emoji_items_.at(index_);
        const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
        const auto annotation = item.is_object() ? item.value("annotation", std::string{}) : std::string{};
        if (!text.empty()) return text + (annotation.empty() ? "" : "  " + annotation);
      }
    }
    return "表情 " + std::to_string(index_ + 1);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try { ic->propertyFor(factory_)->insertEmoji(index_); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  size_t index_;
};

class FcitxEmojiPageAction : public fcitx::SimpleAction {
public:
  FcitxEmojiPageAction(fcitx::FactoryFor<FcitxState> *factory, bool next)
      : factory_(factory), next_(next) {}
  std::string shortText(fcitx::InputContext *) const override {
    return next_ ? "下一页" : "上一页";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      if (next_) ic->propertyFor(factory_)->nextEmojiPage();
      else ic->propertyFor(factory_)->previousEmojiPage();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  bool next_;
};

class FcitxVoiceAction : public fcitx::SimpleAction {
public:
  explicit FcitxVoiceAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("语音");
    setLongText("开始或停止流式语音识别");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->voice_loading_) state->stopVoice();
      else state->requestVoice();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxVoiceCancelAction : public fcitx::SimpleAction {
public:
  explicit FcitxVoiceCancelAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setShortText("取消语音");
    setLongText("取消当前录音、识别或润色，不提交语音结果");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->voice_loading_) state->cancelVoice();
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

class FcitxTraditionalAction : public fcitx::Action {
public:
  explicit FcitxTraditionalAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return "繁体"; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->traditionalApplies();
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->hasFocus()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->restricted() || state->privateInput()) return;
      if (state->toggleTraditional()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

// classicui's options are shared by every input method, so before one changes, the value it replaces is recorded for msime-linux-setup --unregister to put back (see PanelRestoreRecord.h). A failed record does not hold the change back.
void record_classicui_takeover(const fcitx::RawConfig &current, const fcitx::RawConfig &written) {
  const auto file = msime::linux_host::panel_restore_file(std::getenv("XDG_STATE_HOME"), std::getenv("HOME"));
  if (!file) return;
  for (const auto &key : written.subItems()) {
    const auto *value = written.valueByPath(key);
    if (!value) continue;
    const auto *prior = current.valueByPath(key);
    const auto replaced = prior ? Json(*prior) : Json(nullptr);
    // MSIME only takes the theme over from Fcitx5's stock ones and uninstall removes its own, so a theme option already naming it is recorded as the stock theme it stands in for.
    auto restore = replaced;
    if (prior && *prior == msime::linux_host::kFcitxCandidateTheme && (key == "Theme" || key == "DarkTheme"))
      restore = key == "Theme" ? "default" : "default-dark";
    msime::linux_host::record_panel_takeover(*file, "fcitx5", key, replaced, *value, restore);
  }
}
void set_classicui_config(fcitx::AddonInstance &classicui, const fcitx::RawConfig &config) {
  fcitx::RawConfig current;
  if (const auto *existing = classicui.getConfig()) existing->save(current);
  record_classicui_takeover(current, config);
  classicui.setConfig(config);
}

// Each context owns a thread-bound Host API session. Fcitx never copies composing state.
class FcitxEngine : public fcitx::InputMethodEngineV2 {
public:
  fcitx::Instance *instance() const { return instance_; }
  // Fcitx5 draws the candidate list in its classic UI, which takes one Pango font description for
  // the whole panel. Writing it through the addon's own configuration applies it at once and keeps
  // it in classicui.conf, where Fcitx5's configuration tool shows the same value. A front end
  // without the classic UI (kimpanel on Plasma draws with the desktop's font) is left alone.
  void applyCandidatePanelFont(const Json &preferences) {
    const auto description = candidate_font_sync_.next(msime::linux_host::read_candidate_font(preferences));
    if (!description) return;
    auto *classicui = instance_->addonManager().addon("classicui", true);
    if (!classicui) return;
    fcitx::RawConfig config;
    config.setValueByPath("Font", *description);
    set_classicui_config(*classicui, config);
  }
  // The candidate colours reach the classic UI as a theme named "msime" in the user's Fcitx5 data directory (see candidates/CandidateFcitxTheme.h). The addon is pointed at it only while it shows one of Fcitx5's stock themes or MSIME's own; a theme the user chose is left in place and MSIME's colours simply don't apply. Setting the configuration also makes the addon read the theme file again, which is how a changed palette appears without a restart.
  void applyCandidatePanelTheme(const Json &preferences, bool system_dark, const Json &catalog) {
    namespace host = msime::linux_host;
    const auto resolved = resolveCandidateTheme(preferences, system_dark, catalog);
    const auto &colors = resolved.colors;
    const auto decoration = host::candidate_skin_decoration(catalog, resolved.candidate_skin);
    const auto corner_radius = host::candidate_corner_radius(preferences, catalog, resolved.candidate_skin);
    // Only the user's own radius pulls the highlight's corners in with the card.
    const bool user_radius = host::candidate_corner_radius_preference(preferences).has_value();
    // The decoration's stamp stands in for its image, so an unchanged skin costs a stat per refresh, not a copy.
    // Read once: the icon only changes with the package, and a reinstall restarts Fcitx5 with it.
    static const auto logo = host::load_fcitx_theme_logo(MSIME_ICON_DIR);
    auto theme = host::fcitx_candidate_theme(colors, resolved.dark, std::nullopt, corner_radius, logo, user_radius) +
                 host::fcitx_overlay_stamp(decoration);
    if (theme == candidate_theme_applied_) return;
    auto *classicui = instance_->addonManager().addon("classicui", true);
    if (!classicui || !classicui->getConfig()) return;
    fcitx::RawConfig current;
    classicui->getConfig()->save(current);
    const auto *selected = current.valueByPath("Theme");
    const auto *selected_dark = current.valueByPath("DarkTheme");
    if (!host::fcitx_theme_replaceable(selected ? *selected : std::string{})) return;
    const auto file = host::fcitx_theme_file(std::getenv("XDG_DATA_HOME"), std::getenv("HOME"));
    if (!file || !host::write_fcitx_candidate_theme(*file, colors, resolved.dark, decoration, corner_radius, logo, user_radius)) return;
    fcitx::RawConfig config;
    config.setValueByPath("Theme", std::string(host::kFcitxCandidateTheme));
    // Fcitx5 releases with a separate dark-mode theme would otherwise switch to their stock dark theme; MSIME already resolves "follow" against the system appearance itself.
    if (selected_dark && host::fcitx_theme_replaceable(*selected_dark))
      config.setValueByPath("DarkTheme", std::string(host::kFcitxCandidateTheme));
    set_classicui_config(*classicui, config);
    candidate_theme_applied_ = std::move(theme);
  }
  // Tell the settings page whether the classic UI draws the candidate font, colours and skin (see candidates/CandidatePanelStatus.h). Asked on every theme sync because the user can switch the UI or theme in fcitx5-configtool at any time; the file is rewritten only when the answer changes.
  void publishCandidatePanelStatus() {
    namespace host = msime::linux_host;
    const auto file = host::candidate_panel_status_file(std::getenv("XDG_RUNTIME_DIR"));
    if (!file) return;
    bool replaceable = true;
    auto *classicui = instance_->addonManager().addon("classicui", true);
    if (classicui && classicui->getConfig()) {
      fcitx::RawConfig current;
      classicui->getConfig()->save(current);
      const auto *selected = current.valueByPath("Theme");
      replaceable =
          host::fcitx_candidate_theme_drawn(selected ? *selected : std::string{}, current.valueByPath("DarkTheme"));
    }
    host::write_candidate_panel_status(
        *file, host::candidate_panel_status_document(
                   "fcitx5", host::fcitx_candidate_panel_limit(instance_->currentUI(), replaceable)));
  }
  // Runs before any session exists: a package upgrade leaves the user's options on the previous dictionary generation until this re-prepares it. The system-wide file belongs to the administrator and is not rewritten.
  static void refreshOptions() {
    try {
      const auto path = optionsPath();
      if (path == MSIME_SYSTEM_OPTIONS) return;
      if (msime::linux_host::refresh_runtime_options(path))
        msime_linux_diagnostic_write("dictionary_generation_refreshed");
    } catch (const OptionsNotConfigured &) {
      // Nothing to refresh before first-run setup; activation shows the setup hint.
    } catch (const msime::linux_host::DictionaryOutdated &) {
      // Downloaded dictionaries an upgrade did not replace: the previous generation keeps working, and the guide script (throttled to once per login session) tells the user how to fetch the new ones.
      msime_linux_diagnostic_write("operation_failed operation=dictionary_generation_refresh reason=dictionary_outdated");
      const auto guide = std::string(MSIME_BINDIR "/") + std::string(msime::linux_host::kFirstRunGuideProgram);
      if (access(guide.c_str(), X_OK) == 0) fcitx::startProcess({guide, "--reason", "dictionary-outdated"});
    } catch (...) {
      msime_linux_diagnostic_write("operation_failed operation=dictionary_generation_refresh");
    }
  }
  // At startup, before any context has a session: the first commits should not wait for a preference tick to learn whether statistics are on.
  static void refreshTypingStatistics() {
    try {
      const auto options = readOptions();
      const auto directory = options.find("preferences_directory");
      if (directory != options.end() && directory->is_string())
        fcitx_typing_statistics.refresh(directory->get<std::string>());
    } catch (...) {
      // No options yet (first run) or a document being replaced; the preference ticks refresh the switch once a context has a session.
    }
  }
  // Windows answers "restart the input method" by exiting its Server for the watchdog to start a fresh one. This addon shares the Fcitx5 process with every other input method, so the equivalent stays in process: end every MSIME composition and session the way focus-out does (close() also cancels a voice recording and fences in-flight online, AI and translation replies), bring the options up to date as startup does before the first session, and give the focused context a new session at once. Other contexts open theirs on their next key or activation. The input mode each context was in is kept.
  void resetSessions() {
    msime_linux_diagnostic_write("sessions_reset");
    std::vector<fcitx::InputContext *> focused;
    instance_->inputContextManager().foreach([this, &focused](fcitx::InputContext *ic) {
      auto *state = ic->propertyFor(&factory_);
      // A context another input method owns is left alone, panel included.
      if (!state->session_ && instance_->inputMethodEngine(ic) != this) return true;
      if (ic->hasFocus()) focused.push_back(ic);
      state->close();
      state->clearPanel();
      return true;
    });
    refreshOptions();
    refreshTypingStatistics();
    for (auto *ic : focused) {
      auto *state = ic->propertyFor(&factory_);
      try {
        if (state->ensure()) {
          state->syncVoiceAction();
          state->render();
        }
      } catch (const OptionsNotConfigured &) { notConfigured(*state, false); }
      catch (...) { unavailable(*state); }
    }
  }
  // Reached through the Fcitx5 controller's ReloadAddonConfig for this addon, which is what the settings page's restart button sends. Fcitx5's own ReloadConfig (fcitx5-remote -r) reloads only the global configuration and never calls addons.
  void reloadConfig() override { resetSessions(); }
  void applyCandidateWheelPaging(const Json &preferences) {
    const auto enabled =
        candidate_wheel_paging_sync_.next(msime::linux_host::read_candidate_wheel_paging(preferences));
    if (!enabled) return;
    auto *classicui = instance_->addonManager().addon("classicui", true);
    if (!classicui) return;
    fcitx::RawConfig config;
    config.setValueByPath("WheelForPaging", *enabled ? "True" : "False");
    set_classicui_config(*classicui, config);
  }
  explicit FcitxEngine(fcitx::Instance *instance) : instance_(instance) {
    // A library Fcitx5 kept loaded across an earlier engine's teardown keeps its globals; this engine writes off the loop again.
    fcitx_key_presses_shutting_down = false;
    refreshOptions();
    refreshTypingStatistics();
    instance->inputContextManager().registerProperty("msimeState", &factory_);
    english_action_.registerAction("msime-english-candidates", &instance->userInterfaceManager());
    input_mode_action_.registerAction("msime-input-mode", &instance->userInterfaceManager());
    scheme_action_.registerAction("msime-scheme", &instance->userInterfaceManager());
    shuangpin_profile_action_.registerAction("msime-shuangpin-profile", &instance->userInterfaceManager());
    width_action_.registerAction("msime-fullwidth", &instance->userInterfaceManager());
    nine_key_action_.registerAction("msime-nine-key", &instance->userInterfaceManager());
    nine_key_action_.setMenu(&nine_key_menu_);
    nine_key_menu_.addAction(&nine_key_spelling1_);
    nine_key_menu_.addAction(&nine_key_spelling2_);
    nine_key_menu_.addAction(&nine_key_spelling3_);
    nine_key_menu_.addAction(&nine_key_spelling4_);
    nine_key_menu_.addAction(&nine_key_spelling5_);
    nine_key_menu_.addAction(&nine_key_spelling6_);
    nine_key_menu_.addAction(&nine_key_spelling7_);
    nine_key_menu_.addAction(&nine_key_spelling8_);
    nine_key_menu_.addAction(&nine_key_spelling9_);
    helpcode_action_.registerAction("msime-helpcode", &instance->userInterfaceManager());
    mixed_english_action_.registerAction("msime-mixed-english", &instance->userInterfaceManager());
    mixed_emoji_action_.registerAction("msime-mixed-emoji", &instance->userInterfaceManager());
    mixed_kaomoji_action_.registerAction("msime-mixed-kaomoji", &instance->userInterfaceManager());
    local_unicode_action_.registerAction("msime-local-unicode", &instance->userInterfaceManager());
    local_date_time_action_.registerAction("msime-local-date-time", &instance->userInterfaceManager());
    local_quick_phrase_action_.registerAction("msime-local-quick-phrase", &instance->userInterfaceManager());
    local_emoji_action_.registerAction("msime-local-emoji", &instance->userInterfaceManager());
    local_kaomoji_action_.registerAction("msime-local-kaomoji", &instance->userInterfaceManager());
    local_super_jianpin_action_.registerAction("msime-local-super-jianpin", &instance->userInterfaceManager());
    local_temporary_english_action_.registerAction("msime-local-temporary-english", &instance->userInterfaceManager());
    local_temporary_japanese_action_.registerAction("msime-local-temporary-japanese", &instance->userInterfaceManager());
    local_expression_action_.registerAction("msime-local-expression", &instance->userInterfaceManager());
    local_command_action_.registerAction("msime-local-command", &instance->userInterfaceManager());
    local_mention_action_.registerAction("msime-local-mention", &instance->userInterfaceManager());
    english_gloss_action_.registerAction("msime-english-gloss", &instance->userInterfaceManager());
    word_character_action_.registerAction("msime-word-character", &instance->userInterfaceManager());
    number_row_action_.registerAction("msime-number-row", &instance->userInterfaceManager());
    shuangpin_preedit_action_.registerAction("msime-shuangpin-preedit", &instance->userInterfaceManager());
    wubi_code_hint_action_.registerAction("msime-wubi-code-hint", &instance->userInterfaceManager());
    helpcode_schema_action_.registerAction("msime-helpcode-schema", &instance->userInterfaceManager());
    maintenance_action_.registerAction("msime-candidate-tools", &instance->userInterfaceManager());
    clipboard_action_.registerAction("msime-clipboard", &instance->userInterfaceManager());
    clipboard_history_action_.registerAction("msime-clipboard-history", &instance->userInterfaceManager());
    cloud_clipboard_action_.registerAction("msime-cloud-clipboard", &instance->userInterfaceManager());
    emoji_action_.registerAction("msime-emoji", &instance->userInterfaceManager());
    emoji_search_action_.registerAction("msime-emoji-search", &instance->userInterfaceManager());
    emoji_category_action_.registerAction("msime-emoji-category", &instance->userInterfaceManager());
    emoji_group_action_.registerAction("msime-emoji-group", &instance->userInterfaceManager());
    voice_action_.registerAction("msime-voice", &instance->userInterfaceManager());
    voice_cancel_action_.registerAction("msime-voice-cancel", &instance->userInterfaceManager());
    desktop_tools_action_.registerAction("msime-desktop-tools", &instance->userInterfaceManager());
    toolbar_action_.registerAction("msime-toolbar", &instance->userInterfaceManager());
    traditional_action_.registerAction("msime-traditional", &instance->userInterfaceManager());
    chinese_punctuation_action_.registerAction("msime-chinese-punctuation", &instance->userInterfaceManager());
    paired_punctuation_action_.registerAction("msime-paired-punctuation", &instance->userInterfaceManager());
    smart_punctuation_action_.registerAction("msime-smart-punctuation", &instance->userInterfaceManager());
    smart_punctuation_repeat_action_.registerAction("msime-smart-punctuation-repeat", &instance->userInterfaceManager());
    candidate_layout_action_.registerAction("msime-candidate-layout", &instance->userInterfaceManager());
    candidate_theme_action_.registerAction("msime-candidate-theme", &instance->userInterfaceManager());
    global_theme_action_.registerAction("msime-global-theme", &instance->userInterfaceManager());
    global_theme_action_.setMenu(&global_theme_menu_);
    if (const auto themes = themeCatalog().find("themes"); themes != themeCatalog().end() && themes->is_array())
      for (const auto &theme : *themes) {
        if (!theme.is_object() || !theme.value("id", Json()).is_string() || !theme.value("title", Json()).is_string())
          continue;
        const auto id = theme.at("id").get<std::string>();
        global_theme_items_.push_back(
            std::make_unique<FcitxGlobalThemeItemAction>(&factory_, id, theme.at("title").get<std::string>()));
        // Registered so the D-Bus menus (StatusNotifierItem, kimpanel), which address items by their registered id, can trigger them too.
        global_theme_items_.back()->registerAction("msime-global-theme-" + id, &instance->userInterfaceManager());
        global_theme_menu_.addAction(global_theme_items_.back().get());
      }
    candidate_page_size_action_.registerAction("msime-candidate-page-size", &instance->userInterfaceManager());
    candidate_page_size_action_.setMenu(&candidate_page_size_menu_);
    candidate_page_size_menu_.addAction(&candidate_page_size1_);
    candidate_page_size_menu_.addAction(&candidate_page_size2_);
    candidate_page_size_menu_.addAction(&candidate_page_size3_);
    candidate_page_size_menu_.addAction(&candidate_page_size4_);
    candidate_page_size_menu_.addAction(&candidate_page_size5_);
    candidate_page_size_menu_.addAction(&candidate_page_size6_);
    candidate_page_size_menu_.addAction(&candidate_page_size7_);
    candidate_page_size_menu_.addAction(&candidate_page_size8_);
    candidate_page_size_menu_.addAction(&candidate_page_size9_);
    learning_action_.registerAction("msime-learning", &instance->userInterfaceManager());
    frequency_action_.registerAction("msime-frequency", &instance->userInterfaceManager());
    frequency_trigger_action_.registerAction("msime-frequency-trigger", &instance->userInterfaceManager());
    frequency_step_action_.registerAction("msime-frequency-step", &instance->userInterfaceManager());
    mode_scope_action_.registerAction("msime-mode-scope", &instance->userInterfaceManager());
    candidate_translation_action_.registerAction("msime-candidate-translations", &instance->userInterfaceManager());
    sentence_translation_action_.registerAction("msime-translate-sentence", &instance->userInterfaceManager());
    punctuation_lock_action_.registerAction("msime-punctuation-lock", &instance->userInterfaceManager());
    translation_language_action_.registerAction("msime-translation-language", &instance->userInterfaceManager());
    cloud_candidates_action_.registerAction("msime-cloud-candidates", &instance->userInterfaceManager());
    ai_candidates_action_.registerAction("msime-ai-candidates", &instance->userInterfaceManager());
    clipboard_action_.setMenu(&clipboard_menu_);
    clipboard_menu_.addAction(&clipboard_item1_);
    clipboard_menu_.addAction(&clipboard_item2_);
    clipboard_menu_.addAction(&clipboard_item3_);
    clipboard_menu_.addAction(&clipboard_item4_);
    clipboard_menu_.addAction(&clipboard_item5_);
    clipboard_menu_.addAction(&clipboard_remove1_);
    clipboard_menu_.addAction(&clipboard_remove2_);
    clipboard_menu_.addAction(&clipboard_remove3_);
    clipboard_menu_.addAction(&clipboard_remove4_);
    clipboard_menu_.addAction(&clipboard_remove5_);
    clipboard_menu_.addAction(&clipboard_clear_action_);
    cloud_clipboard_action_.setMenu(&cloud_clipboard_menu_);
    cloud_clipboard_menu_.addAction(&cloud_clipboard_item1_);
    cloud_clipboard_menu_.addAction(&cloud_clipboard_item2_);
    cloud_clipboard_menu_.addAction(&cloud_clipboard_item3_);
    cloud_clipboard_menu_.addAction(&cloud_clipboard_item4_);
    cloud_clipboard_menu_.addAction(&cloud_clipboard_item5_);
    toolbar_action_.setMenu(&toolbar_menu_);
    desktop_tools_action_.setMenu(&desktop_tools_menu_);
    desktop_tools_menu_.addAction(&handwriting_action_);
    desktop_tools_menu_.addAction(&keyboard_action_);
    desktop_tools_menu_.addAction(&desktop_emoji_action_);
    desktop_tools_menu_.addAction(&desktop_clipboard_action_);
    desktop_tools_menu_.addAction(&desktop_voice_action_);
    desktop_tools_menu_.addAction(&cloud_dictionary_action_);
    desktop_tools_menu_.addAction(&desktop_cloud_clipboard_action_);
    desktop_tools_menu_.addAction(&help_action_);
    desktop_tools_menu_.addAction(&feedback_action_);
    desktop_tools_menu_.addAction(&reload_service_action_);
    desktop_tools_menu_.addAction(&toolbar_enabled_action_);
    desktop_tools_menu_.addAction(&voice_enabled_action_);
    desktop_tools_menu_.addAction(&preference_save_retry_action_);
    // The D-Bus menus (StatusNotifierItem, kimpanel) address entries by their registered name and skip an unregistered one, so every entry of the menus below is registered, separators included.
    for (auto [action, name] : std::initializer_list<std::pair<fcitx::Action *, const char *>>{
             {&handwriting_action_, "msime-desktop-handwriting"},
             {&keyboard_action_, "msime-desktop-keyboard"},
             {&desktop_emoji_action_, "msime-desktop-emoji"},
             {&desktop_clipboard_action_, "msime-desktop-clipboard"},
             {&desktop_voice_action_, "msime-desktop-voice"},
             {&cloud_dictionary_action_, "msime-desktop-cloud-dictionary"},
             {&desktop_cloud_clipboard_action_, "msime-desktop-cloud-clipboard"},
             {&help_action_, "msime-desktop-help"},
             {&feedback_action_, "msime-desktop-feedback"},
             {&reload_service_action_, "msime-reload-service"},
             {&toolbar_enabled_action_, "msime-toolbar-enabled"},
             {&voice_enabled_action_, "msime-voice-enabled"},
             {&preference_save_retry_action_, "msime-preference-save-retry"},
             {&dictionary_action_, "msime-dictionary"},
             {&settings_action_, "msime-settings"},
             {&about_action_, "msime-about"},
             {&scheme_quanpin_action_, "msime-scheme-quanpin"},
             {&scheme_shuangpin_action_, "msime-scheme-shuangpin"},
             {&scheme_wubi_action_, "msime-scheme-wubi"},
             {&scheme_japanese_action_, "msime-scheme-japanese"},
             {&scheme_korean_action_, "msime-scheme-korean"},
             {&scheme_cantonese_action_, "msime-scheme-cantonese"},
             {&scheme_zhuyin_action_, "msime-scheme-zhuyin"},
             {&scheme_vietnamese_action_, "msime-scheme-vietnamese"},
             {&input_group_action_, "msime-group-input"},
             {&input_group_separator_, "msime-group-input-separator"},
             {&punctuation_group_action_, "msime-group-punctuation"},
             {&punctuation_group_separator_, "msime-group-punctuation-separator"},
             {&candidate_group_action_, "msime-group-candidate"},
             {&candidate_group_separator_, "msime-group-candidate-separator"}})
      action->registerAction(name, &instance->userInterfaceManager());
    // 输入方案 lists the schemes rather than stepping through them on each click. Cantonese and Zhuyin join it once a context has read runtime options naming their dictionaries (rebuildSchemeMenu).
    scheme_action_.setMenu(&scheme_menu_);
    rebuildSchemeMenu(nullptr, false, false);
    // The design menu keeps 中文/英文, 全角/标点/译文, 输入方案 and 主题/词库…/设置…/关于 at the top; every other switch the status area listed moves, as the same action, into one of three groups.
    input_group_action_.setMenu(&input_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &shuangpin_profile_action_, &helpcode_action_, &helpcode_schema_action_, &traditional_action_,
             &mixed_english_action_, &english_action_, &mixed_emoji_action_, &mixed_kaomoji_action_, &english_gloss_action_,
             &cloud_candidates_action_, &ai_candidates_action_, &number_row_action_, &word_character_action_,
             &mode_scope_action_, &clipboard_history_action_, &input_group_separator_, &local_unicode_action_,
             &local_date_time_action_, &local_quick_phrase_action_, &local_emoji_action_, &local_kaomoji_action_,
             &local_super_jianpin_action_, &local_temporary_english_action_, &local_temporary_japanese_action_,
             &local_expression_action_, &local_command_action_, &local_mention_action_})
      input_group_menu_.addAction(action);
    punctuation_group_action_.setMenu(&punctuation_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &paired_punctuation_action_, &smart_punctuation_action_, &smart_punctuation_repeat_action_,
             &punctuation_lock_action_, &punctuation_group_separator_, &sentence_translation_action_,
             &translation_language_action_})
      punctuation_group_menu_.addAction(action);
    candidate_group_action_.setMenu(&candidate_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &candidate_layout_action_, &candidate_page_size_action_, &candidate_theme_action_,
             &shuangpin_preedit_action_, &wubi_code_hint_action_, &candidate_group_separator_, &learning_action_,
             &frequency_action_, &frequency_trigger_action_, &frequency_step_action_})
      candidate_group_menu_.addAction(action);
    emoji_action_.setMenu(&emoji_menu_);
    emoji_menu_.addAction(&emoji_item1_);
    emoji_menu_.addAction(&emoji_item2_);
    emoji_menu_.addAction(&emoji_item3_);
    emoji_menu_.addAction(&emoji_item4_);
    emoji_menu_.addAction(&emoji_item5_);
    emoji_menu_.addAction(&emoji_previous_action_);
    emoji_menu_.addAction(&emoji_next_action_);
    maintenance_action_.setMenu(&maintenance_menu_);
    maintenance_menu_.addAction(&pin_action_);
    maintenance_menu_.addAction(&remove_action_);
    maintenance_menu_.addAction(&fix1_action_);
    maintenance_menu_.addAction(&fix2_action_);
    maintenance_menu_.addAction(&fix3_action_);
    maintenance_menu_.addAction(&fix4_action_);
    maintenance_menu_.addAction(&fix5_action_);
    maintenance_menu_.addAction(&clear_action_);
    capability_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextCapabilityChanged,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
          auto *ic = static_cast<fcitx::InputContextEvent &>(event).inputContext();
          auto *state = ic->propertyFor(&factory_);
          // Never activate MSIME or clear another input method's panel here.
          if (!state->session_) return;
          if (state->restricted() || state->private_ != state->privateInput()) {
            state->close();
            state->clearPanel();
          } else {
            // Client-side preedit support may also change while composing.
            try { state->render(); } catch (...) { unavailable(*state); }
          }
        });
    focus_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextFocusOut,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
          auto *ic = static_cast<fcitx::InputContextEvent &>(event).inputContext();
          auto *state = ic->propertyFor(&factory_);
          if (!state->session_) return;
          // Leaving the client commits an open Korean syllable, Zhuyin conversion or Vietnamese word. Fcitx5 commits a client preedit on focus out itself (or the client does, with ClientUnfocusCommit), so only a composition drawn in the panel, for a client without preedit support, is committed here.
          if (state->commitsOnBlur() && !state->view_.value("editing_text", std::string{}).empty()) {
            if (!ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) {
              try { state->apply(msime_client_focus(state->session_, false)); } catch (...) {}
            } else {
              // The composition reaches the document through the preedit rather than through commitText, so it is counted here as typed text.
              state->recordTypingStatistics(state->view_.value("preedit", std::string{}), state->typingSource());
            }
          }
          state->rememberInputMode();
          state->ime_mode_chosen_ = false;
          state->mode_restore_pending_ = true;
          state->close();
          state->clearPanel();
        });
    // Any focused context counts, whichever input method it uses: the panels type through Fcitx5 itself once this addon is loaded.
    panel_focus_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextFocusIn,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &) {
          ++panel_input_generation_;
          listenPanelInput();
        });
    listenPanelInput();
    // The first probe starts at addon load; stepSystemTheme sets every later interval.
    system_theme_timer_ = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC), 10000,
        [this](fcitx::EventSourceTime *timer, uint64_t) {
          timer->setNextInterval(stepSystemTheme());
          timer->setOneShot();
          return true;
        });
#ifdef MSIME_FCITX5_TELEMETRY
    startTelemetry();
#endif
  }
  // The theme worker runs addon code on a schedule rather than on user action, so it is the detached job most likely to be in flight when Fcitx5 unloads the addon. Waiting for it here (its portal call gives up after 1 s) keeps that code from running after the library is gone; the other detached jobs keep the risk their comment accepts.
  // The key press counts get the same care: every context's pending batch is written here, synchronously, and so is any a context still flushes when the factory destroys it; batches already handed to a thread (contexts Fcitx5 destroyed before unloading the addon) are waited for. A store write takes milliseconds; the bound only keeps a wedged store from holding the exit.
  ~FcitxEngine() override {
    fcitx_key_presses_shutting_down = true;
    instance_->inputContextManager().foreach([this](fcitx::InputContext *ic) {
      ic->propertyFor(&factory_)->flushPendingKeyPresses();
      return true;
    });
    fcitx_key_press_writes.wait_idle(std::chrono::seconds(2));
    if (system_theme_job_.valid()) system_theme_job_.wait_for(std::chrono::seconds(2));
#ifdef MSIME_FCITX5_TELEMETRY
    stopTelemetry();
#endif
  }
#ifdef MSIME_FCITX5_TELEMETRY
  // Usage reporting (see platforms/common/Telemetry.h): one session per addon lifetime in this Fcitx5 process, in a directory of its own so it never shares a session marker with an IBus host of the same user. Crash capture only writes the record and then hands the signal to whatever handler Fcitx5 installed before. Nothing touches the network on the loop: the session starts and the queue is sent on a worker, again every 30 minutes, and the switch is read from the shared preferences on every round.
  void startTelemetry() {
    std::filesystem::path preferences;
    try {
      const auto directory = readOptions().value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') preferences = directory;
    } catch (...) {
    }
    const auto root = msime::telemetry::default_directory();
    if (root.empty()) return;
    msime::telemetry::install_crash_handlers();
    telemetry_job_ = detachedJob([host = msime::telemetry::Host{"linux", MSIME_LINUX_VERSION, root / "fcitx5", std::nullopt, preferences}] {
      msime::telemetry::begin(host);
      msime::telemetry::flush();
      return Json();
    });
    telemetry_timer_ = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + kTelemetryIntervalUs, 0,
        [this](fcitx::EventSourceTime *timer, uint64_t) {
          // One round at a time; a slow endpoint just skips a turn.
          if (!telemetry_job_.valid() || telemetry_job_.wait_for(std::chrono::seconds(0)) == std::future_status::ready)
            telemetry_job_ = detachedJob([] {
              msime::telemetry::flush();
              return Json();
            });
          timer->setNextInterval(kTelemetryIntervalUs);
          timer->setOneShot();
          return true;
        });
  }
  // The addon's code must not run after Fcitx5 unloads it: wait (bounded, as for the theme worker) for a round in flight, close the session and give the signals back.
  void stopTelemetry() {
    telemetry_timer_.reset();
    if (telemetry_job_.valid()) telemetry_job_.wait_for(std::chrono::seconds(2));
    msime::telemetry::end();
    msime::telemetry::remove_crash_handlers();
  }
  static constexpr uint64_t kTelemetryIntervalUs = 30ull * 60 * 1000000;
  std::shared_future<Json> telemetry_job_;
  std::unique_ptr<fcitx::EventSourceTime> telemetry_timer_;
#endif
  // The desktop appearance (the portal's color-scheme) is probed once for the whole addon, not once per input context, and never on the loop: fcitx_system_dark_theme is a synchronous portal round trip that can block for its full 1 s D-Bus timeout. One worker is in flight at a time; the loop polls it every 250 ms and starts the next one 5 s after the last answer, so a theme switch reaches every context within about 5 s, as when each context probed on its own. Returns the microseconds until the next step.
  uint64_t stepSystemTheme() {
    constexpr uint64_t kPollUs = 250000;
    constexpr uint64_t kProbeIntervalUs = 5000000;
    if (!system_theme_job_.valid()) {
      system_theme_job_ = detachedJob([] {
        const auto dark = fcitx_system_dark_theme();
        return dark ? Json(*dark) : Json();
      });
      return kPollUs;
    }
    if (system_theme_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return kPollUs;
    const auto dark = system_theme_job_.get();
    system_theme_job_ = {};
    // No portal (null) keeps the previous appearance, as fcitx_system_dark_theme documents.
    if (dark.is_boolean()) applySystemTheme(dark.get<bool>());
    return kProbeIntervalUs;
  }
  // Runs on the loop. Every context, whichever input method owns it, keeps the value so that it is current when MSIME activates there; each one redraws its voice overlay and candidate theme exactly as its own probe used to.
  void applySystemTheme(bool dark) {
    if (dark == system_dark_) return;
    system_dark_ = dark;
    instance_->inputContextManager().foreach([this, dark](fcitx::InputContext *ic) {
      ic->propertyFor(&factory_)->setSystemDark(dark);
      return true;
    });
  }
  // The desktop panels type through the focused input context; see PanelInputChannel.h.
  void listenPanelInput() {
    if (panel_input_socket_.listening() ||
        !panel_input_socket_.open(msime::linux_host::panel_input_socket_path()))
      return;
    panel_input_io_ = instance_->eventLoop().addIOEvent(
        panel_input_socket_.fd(), fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) {
          if (auto accepted = panel_input_socket_.accept_request()) {
            if (auto request = msime::linux_host::parse_panel_input_request(accepted->second)) {
              if (!panel_input_broker_.submit(accepted->first, std::move(*request),
                                              msime::linux_host::panel_input_monotonic_us()))
                msime::linux_host::PanelInputSocket::reply_and_close(
                    accepted->first, msime::linux_host::panel_input_error_reply("no_focus"));
            } else
              msime::linux_host::PanelInputSocket::reply_and_close(
                  accepted->first, msime::linux_host::panel_input_error_reply("invalid"));
            pumpPanelInput();
          }
          return true;
        });
  }
  void pumpPanelInput() {
    panel_input_broker_.pump(
        msime::linux_host::panel_input_monotonic_us(),
        [this] {
          auto *ic = instance_->mostRecentInputContext();
          return msime::linux_host::PanelInputFocus{ic && ic->hasFocus(), panel_input_generation_};
        },
        [this](const msime::linux_host::PanelInputRequest &request) { return deliverPanelInput(request); },
        msime::linux_host::PanelInputSocket::reply_and_close);
    if (panel_input_broker_.empty()) return;
    if (!panel_input_timer_) {
      panel_input_timer_ = instance_->eventLoop().addTimeEvent(
          CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 50000, 10000,
          [this](fcitx::EventSourceTime *timer, uint64_t) {
            pumpPanelInput();
            if (!panel_input_broker_.empty()) {
              timer->setNextInterval(50000);
              timer->setOneShot();
            }
            return true;
          });
    } else if (!panel_input_timer_->isEnabled()) {
      panel_input_timer_->setNextInterval(50000);
      panel_input_timer_->setOneShot();
    }
  }
  msime::linux_host::PanelInputDelivery deliverPanelInput(
      const msime::linux_host::PanelInputRequest &request) {
    using msime::linux_host::PanelInputDelivery;
    auto *ic = instance_->mostRecentInputContext();
    if (!ic || !ic->hasFocus()) return PanelInputDelivery::NoFocus;
    if (ic->capabilityFlags().testAny(fcitx::CapabilityFlags{
            fcitx::CapabilityFlag::Password, fcitx::CapabilityFlag::Disable}))
      return PanelInputDelivery::Restricted;
    if (request.kind == msime::linux_host::PanelInputRequest::Kind::Text) {
      ic->commitString(request.text);
      return PanelInputDelivery::Delivered;
    }
    auto sym = fcitx::Key::keySymFromString(request.key);
    if (sym == FcitxKey_None) return PanelInputDelivery::Invalid;
    // The panel knows nothing of the lock; carry the one the last real key reported so this stroke does not flip the CapsLock indicator. Only this input method has been watching the lock.
    const bool caps = instance_->inputMethodEngine(ic) == this && ic->propertyFor(&factory_)->caps_lock_;
    fcitx::KeyStates states;
    if (request.shift) states |= fcitx::KeyState::Shift;
    if (caps) states |= fcitx::KeyState::CapsLock;
    // The panel sends letters lowercase; apply Shift and the lock the way xkb does for a physical key, so under CapsLock the stroke is an uppercase letter the editor gets, not the start of a composition.
    const bool upper = request.shift != caps;
    if (upper && sym >= FcitxKey_a && sym <= FcitxKey_z)
      sym = static_cast<fcitx::KeySym>(sym - FcitxKey_a + FcitxKey_A);
    else if (caps && !upper && sym >= FcitxKey_A && sym <= FcitxKey_Z)
      sym = static_cast<fcitx::KeySym>(sym - FcitxKey_A + FcitxKey_a);
    if (request.control) states |= fcitx::KeyState::Ctrl;
    if (request.alt) states |= fcitx::KeyState::Alt;
    if (request.super) states |= fcitx::KeyState::Super;
    // Fcitx5 key codes are X keycodes, the evdev code plus eight.
    const fcitx::Key key(sym, states, request.keycode ? static_cast<int>(request.keycode) + 8 : 0);
    // Through the context's input method first, the way SendInput passes through the IME on Windows: letters compose, and digits, Space and BackSpace act on an open composition.
    msime::linux_host::deliver_panel_key_stroke(
        [&](bool release) {
          fcitx::KeyEvent event(ic, key, release);
          return ic->keyEvent(event);
        },
        [&](bool release) { ic->forwardKey(key, release); });
    return PanelInputDelivery::Delivered;
  }
  void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
    auto &status = event.inputContext()->statusArea();
    // The design menu: 中文/英文; 全角/标点/译文; 输入方案; 主题/词库…/设置…/关于. 中文/英文 is the input-mode toggle Shift flips; the Engine's dedicated English mode is a different feature and sits in 输入选项. kimpanel lists status actions as they are, separators included, so the parts are not divided by rules here.
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &input_mode_action_, &width_action_, &chinese_punctuation_action_,
             &candidate_translation_action_, &scheme_action_, &global_theme_action_, &dictionary_action_,
             &settings_action_, &about_action_})
      status.addAction(fcitx::StatusGroup::InputMethod, action);
    // Tools that depend on the moment: an active recording, the highlighted candidate, nine-key spellings, clipboard and emoji pickers.
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &voice_cancel_action_, &maintenance_action_, &nine_key_action_, &clipboard_action_,
             &cloud_clipboard_action_, &emoji_action_, &emoji_search_action_, &emoji_category_action_,
             &emoji_group_action_, &input_group_action_, &punctuation_group_action_, &candidate_group_action_})
      status.addAction(fcitx::StatusGroup::InputMethod, action);
    // Rebuilt from the current preferences rather than assembled once: the
    // switches are a shared document that can change while a context is focused,
    // and the menu is shared too, so there is one place for it to follow.
    event.inputContext()->propertyFor(&factory_)->refreshToolbar();
    status.addAction(fcitx::StatusGroup::InputMethod, &desktop_tools_action_);
    try {
      if (state->ensure()) {
        if (state->voice_enabled_)
          event.inputContext()->statusArea().addAction(
              fcitx::StatusGroup::InputMethod, &voice_action_);
        state->render();
        state->syncMusic();
        // Moving into another text field shows the current 中/英 as a switch does (#2589), so the user knows the mode before typing there. Only a focus change: switching to this input method from another already gets Fcitx5's own input-method popup, and showInputModeHud keeps to the input_mode_hud preference and stays quiet in password and private fields.
        if (event.type() == fcitx::EventType::InputContextFocusIn) state->showInputModeHud();
      }
    } catch (const OptionsNotConfigured &) { notConfigured(*state, true); }
    catch (...) { unavailable(*state); }
    state->publishInputStatus(true);
    noticeReplacedAddon(*state);
  }
  void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
    // Switching to another input method ends an open Korean syllable, Zhuyin conversion or Vietnamese word as text, since it is already what the user wrote. Losing the focus needs nothing here: Fcitx5 commits the client preedit itself then (see focus_watch_).
    if (event.type() == fcitx::EventType::InputContextSwitchInputMethod && state->session_ &&
        state->commitsOnBlur() && !state->view_.value("editing_text", std::string{}).empty()) {
      try { state->command(MSIME_FINISH_COMPOSITION); } catch (...) {}
    }
    // Everything activate() or a later voice or toolbar refresh may have added.
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &input_mode_action_, &width_action_, &chinese_punctuation_action_,
             &candidate_translation_action_, &scheme_action_, &global_theme_action_, &dictionary_action_,
             &settings_action_, &about_action_, &voice_cancel_action_, &maintenance_action_, &nine_key_action_,
             &clipboard_action_, &cloud_clipboard_action_, &emoji_action_, &emoji_search_action_,
             &emoji_category_action_, &emoji_group_action_, &input_group_action_, &punctuation_group_action_,
             &candidate_group_action_, &voice_action_, &toolbar_action_, &desktop_tools_action_})
      event.inputContext()->statusArea().removeAction(action);
    state->close(); state->clearPanel();
    state->publishInputStatus(false);
  }
  void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
    state->backspace_hold_.reset();
    state->toggle_chord_held_ = FcitxKey_None;
    // An open Korean syllable, Zhuyin conversion or Vietnamese word drawn in the panel, for a client without preedit support, exists nowhere but here, so a reset writes it out instead of dropping text the user already typed (see focus_watch_ for the same rule on focus out).
    const bool koreanPanelSyllable =
        state->session_ && state->commitsOnBlur() && !state->view_.value("editing_text", std::string{}).empty() &&
        !event.inputContext()->capabilityFlags().test(fcitx::CapabilityFlag::Preedit);
    try {
      if (state->session_) state->command(koreanPanelSyllable ? MSIME_FINISH_COMPOSITION : MSIME_CANCEL);
    } catch (...) { state->close(); }
    state->clearPanel();
  }
  void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
    state->noteCapsLock(event.rawKey().states().test(fcitx::KeyState::CapsLock));
    try {
      if (state->ensure()) {
        state->countKeyPress(event);
        if (state->key(event)) event.filterAndAccept();
        else state->countPassthroughKey(event);
        state->playKeySound(event);
      }
    }
    catch (const OptionsNotConfigured &) { notConfigured(*state, false); }
    catch (...) { unavailable(*state); }
  }
  std::string subModeLabelImpl(const fcitx::InputMethodEntry &, fcitx::InputContext &ic) override {
    return ic.propertyFor(&factory_)->modeIndicatorLabel();
  }
  static void unavailable(FcitxState &state) {
    // The label names the host operation only; the error itself can carry input
    // or paths and is never written.
    msime_linux_diagnostic_write("operation_failed operation=fcitx_event");
    state.close(); state.clearPanel();
    state.ic_.inputPanel().setAuxUp(fcitx::Text("MSIME：请检查运行配置"));
    state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
  }
  // dpkg renames a new build over this addon's shared object on upgrade and deletes it on removal, and fcitx5 keeps running the one it loaded until it restarts. The addon cannot restart itself the way the IBus host does without taking every other input method down with the process, and the reload behind the settings page's restart button loads no new code, so the first activation that finds the addon replaced or removed tells the user how to finish, once for each change of state, so a reinstall after a removal is announced as an upgrade; the next composition's render replaces the message. It takes precedence over the configuration hints, which a removed or half-upgraded installation would otherwise show.
  static void noticeReplacedAddon(FcitxState &state) {
    using msime::linux_host::ProgramFileState;
    static ProgramFileState shown = ProgramFileState::Current;
    const auto current = msime::linux_host::mapped_file_state(reinterpret_cast<const void *>(&noticeReplacedAddon));
    if (current == ProgramFileState::Current || current == shown) return;
    shown = current;
    msime_linux_diagnostic_write(current == ProgramFileState::Replaced ? "addon_replaced_notice" : "addon_removed_notice");
    const auto restart = msime::linux_host::fcitx5_restart_command();
    state.ic_.inputPanel().setAuxUp(fcitx::Text(current == ProgramFileState::Replaced
        ? "水杉输入法已升级：执行 " + restart + " 或注销后重新登录即可使用新版本"
        : "水杉输入法已卸载：执行 " + restart + " 或注销后重新登录即可完成卸载"));
    state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
  }
  // Keys still reach the application: the addon never filters an event it could not route, so the user can keep typing while the hint is up. Only activation may open the settings window; a key never does, because a window that appears mid-typing can take the keyboard focus and swallow what follows.
  static void notConfigured(FcitxState &state, bool guide) {
    state.close(); state.clearPanel();
    state.ic_.inputPanel().setAuxUp(fcitx::Text(std::string(msime::linux_host::kFirstRunHint)));
    state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    if (guide) launchFirstRunGuide();
  }
  // Activation runs on every focus change, so the spawn itself is throttled here; the script owns the real limit (once per login session), shared with the IBus launcher. fcitx::startProcess double-forks, leaving no child for the addon to reap.
  static void launchFirstRunGuide() {
    static std::optional<std::chrono::steady_clock::time_point> last_launch;
    const auto now = std::chrono::steady_clock::now();
    if (last_launch && now - *last_launch < std::chrono::seconds(30)) return;
    last_launch = now;
    const auto guide = std::string(MSIME_BINDIR "/") + std::string(msime::linux_host::kFirstRunGuideProgram);
    if (access(guide.c_str(), X_OK) != 0) return;
    fcitx::startProcess({guide, "--host", "fcitx5"});
  }
  fcitx::Instance *instance_;
  msime::linux_host::CandidateFontSync candidate_font_sync_;
  std::string candidate_theme_applied_;
  msime::linux_host::CandidateWheelPagingSync candidate_wheel_paging_sync_;
  // Last appearance the addon-wide probe reported; see stepSystemTheme.
  bool system_dark_ = false;
  std::shared_future<Json> system_theme_job_;
  fcitx::FactoryFor<FcitxState> factory_{[this](fcitx::InputContext &ic) {
    return new FcitxState(ic, this, instance_->eventLoop(), system_dark_);
  }};
  std::unique_ptr<fcitx::EventSourceTime> system_theme_timer_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> capability_watch_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> focus_watch_;
  // Declared in this order so the event sources go before the socket and the connections they serve.
  msime::linux_host::PanelInputSocket panel_input_socket_;
  msime::linux_host::PanelInputBroker panel_input_broker_;
  uint64_t panel_input_generation_ = 0;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> panel_focus_watch_;
  std::unique_ptr<fcitx::EventSourceIO> panel_input_io_;
  std::unique_ptr<fcitx::EventSourceTime> panel_input_timer_;
  FcitxModeAction english_action_{&factory_, FcitxModeAction::Mode::EnglishCandidates};
  FcitxInputModeAction input_mode_action_{&factory_};
  FcitxSchemeAction scheme_action_{&factory_};
  fcitx::Menu scheme_menu_;
  FcitxSchemeItemAction scheme_quanpin_action_{&factory_, 0, "全拼"};
  FcitxSchemeItemAction scheme_shuangpin_action_{&factory_, 1, "双拼"};
  FcitxSchemeItemAction scheme_wubi_action_{&factory_, 2, "五笔"};
  FcitxSchemeItemAction scheme_japanese_action_{&factory_, 3, "日文"};
  FcitxSchemeItemAction scheme_korean_action_{&factory_, 4, "韩文"};
  FcitxSchemeItemAction scheme_cantonese_action_{&factory_, 5, "粤拼"};
  FcitxSchemeItemAction scheme_zhuyin_action_{&factory_, 6, "注音"};
  FcitxSchemeItemAction scheme_vietnamese_action_{&factory_, 7, "越南文"};
  // The entries scheme_menu_ holds, in menu order, and whether Cantonese and Zhuyin were among them when it was last built.
  std::vector<fcitx::Action *> scheme_menu_entries_;
  std::optional<std::pair<bool, bool>> scheme_menu_languages_;
  FcitxShuangpinProfileAction shuangpin_profile_action_{&factory_};
  FcitxModeAction width_action_{&factory_, FcitxModeAction::Mode::Fullwidth};
  fcitx::Menu nine_key_menu_;
  FcitxNineKeyAction nine_key_action_{&factory_};
  FcitxNineKeySpellingAction nine_key_spelling1_{&factory_, 0};
  FcitxNineKeySpellingAction nine_key_spelling2_{&factory_, 1};
  FcitxNineKeySpellingAction nine_key_spelling3_{&factory_, 2};
  FcitxNineKeySpellingAction nine_key_spelling4_{&factory_, 3};
  FcitxNineKeySpellingAction nine_key_spelling5_{&factory_, 4};
  FcitxNineKeySpellingAction nine_key_spelling6_{&factory_, 5};
  FcitxNineKeySpellingAction nine_key_spelling7_{&factory_, 6};
  FcitxNineKeySpellingAction nine_key_spelling8_{&factory_, 7};
  FcitxNineKeySpellingAction nine_key_spelling9_{&factory_, 8};
  FcitxHelpcodeAction helpcode_action_{&factory_};
  FcitxAutocorrectAction autocorrect_transposition_action_{&factory_, FcitxAutocorrectAction::Mode::Transposition};
  FcitxAutocorrectAction autocorrect_neighbor_action_{&factory_, FcitxAutocorrectAction::Mode::Neighbor};
  FcitxMixedEnglishAction mixed_english_action_{&factory_};
  FcitxMixedCandidateAction mixed_emoji_action_{&factory_, "emoji", "混合 Emoji"};
  FcitxMixedCandidateAction mixed_kaomoji_action_{&factory_, "kaomoji", "混合颜文字"};
  FcitxLocalModeAction local_unicode_action_{&factory_, "unicode", "Unicode（U 模式）"};
  FcitxLocalModeAction local_date_time_action_{&factory_, "date_time", "日期时间（T 模式）"};
  FcitxLocalModeAction local_quick_phrase_action_{&factory_, "quick_phrase", "快捷短语（K 模式）"};
  FcitxLocalModeAction local_emoji_action_{&factory_, "emoji", "Emoji（E 模式）"};
  FcitxLocalModeAction local_kaomoji_action_{&factory_, "kaomoji", "颜文字（M 模式）"};
  FcitxLocalModeAction local_super_jianpin_action_{&factory_, "super_jianpin", "超级简拼（J 模式）"};
  FcitxLocalModeAction local_temporary_english_action_{&factory_, "temporary_english", "临时英文（Y 模式）"};
  FcitxLocalModeAction local_temporary_japanese_action_{&factory_, "temporary_japanese", "临时日文（R 模式）"};
  FcitxLocalModeAction local_expression_action_{&factory_, "expression", "计算与数字（V 模式）"};
  FcitxLocalModeAction local_command_action_{&factory_, "command", "指令（/ 模式）"};
  FcitxLocalModeAction local_mention_action_{&factory_, "mention", "名单（@ 模式）"};
  FcitxEnglishGlossAction english_gloss_action_{&factory_};
  FcitxWordCharacterAction word_character_action_{&factory_};
  FcitxNumberRowAction number_row_action_{&factory_};
  FcitxSchemeBooleanAction shuangpin_preedit_action_{&factory_, FcitxSchemeBooleanAction::Kind::ShuangpinPreedit};
  FcitxSchemeBooleanAction wubi_code_hint_action_{&factory_, FcitxSchemeBooleanAction::Kind::WubiCodeHint};
  FcitxHelpcodeSchemaAction helpcode_schema_action_{&factory_};
  fcitx::Menu maintenance_menu_;
  FcitxMaintenanceAction maintenance_action_{&factory_, 0, "候选维护"};
  FcitxReloadServiceAction reload_service_action_{&factory_};
  FcitxClipboardAction clipboard_action_{&factory_};
  FcitxClipboardHistoryAction clipboard_history_action_{&factory_};
  FcitxCloudClipboardAction cloud_clipboard_action_{&factory_};
  FcitxEmojiAction emoji_action_{&factory_};
  FcitxEmojiSearchAction emoji_search_action_{&factory_};
  FcitxEmojiCategoryAction emoji_category_action_{&factory_};
  FcitxEmojiGroupAction emoji_group_action_{&factory_};
  FcitxVoiceAction voice_action_{&factory_};
  FcitxVoiceCancelAction voice_cancel_action_{&factory_};
  FcitxDesktopToolsAction desktop_tools_action_;
  FcitxTraditionalAction traditional_action_{&factory_};
  FcitxPunctuationAction chinese_punctuation_action_{&factory_, FcitxPunctuationAction::Mode::Chinese};
  FcitxPunctuationAction paired_punctuation_action_{&factory_, FcitxPunctuationAction::Mode::Paired};
  FcitxSmartPunctuationAction smart_punctuation_action_{&factory_, FcitxSmartPunctuationAction::Mode::Smart};
  FcitxSmartPunctuationAction smart_punctuation_repeat_action_{&factory_, FcitxSmartPunctuationAction::Mode::Repeat};
  FcitxCandidateLayoutAction candidate_layout_action_{&factory_};
  FcitxCandidateThemeAction candidate_theme_action_{&factory_};
  FcitxGlobalThemeAction global_theme_action_{&factory_};
  fcitx::Menu global_theme_menu_;
  std::vector<std::unique_ptr<FcitxGlobalThemeItemAction>> global_theme_items_;
  // One entry per installed skin package after the global themes, as IBus lists them, and the catalogue they were built from.
  std::vector<std::unique_ptr<FcitxGlobalThemeItemAction>> global_theme_package_items_;
  std::vector<std::pair<std::string, std::string>> global_theme_packages_;
  void rebuildThemeMenu(fcitx::InputContext *ic);
  void rebuildSchemeMenu(fcitx::InputContext *ic, bool cantonese, bool zhuyin);
  fcitx::Menu candidate_page_size_menu_;
  FcitxCandidatePageSizeAction candidate_page_size_action_;
  FcitxCandidatePageSizeItemAction candidate_page_size1_{&factory_, 1};
  FcitxCandidatePageSizeItemAction candidate_page_size2_{&factory_, 2};
  FcitxCandidatePageSizeItemAction candidate_page_size3_{&factory_, 3};
  FcitxCandidatePageSizeItemAction candidate_page_size4_{&factory_, 4};
  FcitxCandidatePageSizeItemAction candidate_page_size5_{&factory_, 5};
  FcitxCandidatePageSizeItemAction candidate_page_size6_{&factory_, 6};
  FcitxCandidatePageSizeItemAction candidate_page_size7_{&factory_, 7};
  FcitxCandidatePageSizeItemAction candidate_page_size8_{&factory_, 8};
  FcitxCandidatePageSizeItemAction candidate_page_size9_{&factory_, 9};
  FcitxLearningAction learning_action_{&factory_};
  FcitxFrequencyAction frequency_action_{&factory_};
  FcitxFrequencyNumberAction frequency_trigger_action_{&factory_, "trigger_count", "词频触发次数"};
  FcitxFrequencyNumberAction frequency_step_action_{&factory_, "linear_step", "线性调整步长"};
  FcitxModeScopeAction mode_scope_action_{&factory_};
  FcitxCandidateTranslationAction candidate_translation_action_{&factory_};
  FcitxSentenceTranslationAction sentence_translation_action_{&factory_};
  FcitxPunctuationLockAction punctuation_lock_action_{&factory_};
  FcitxTranslationLanguageAction translation_language_action_{&factory_};
  FcitxCloudCandidatesAction cloud_candidates_action_{&factory_};
  FcitxAiCandidatesAction ai_candidates_action_{&factory_};
  fcitx::Menu clipboard_menu_;
  FcitxClipboardItemAction clipboard_item1_{&factory_, 0};
  FcitxClipboardItemAction clipboard_item2_{&factory_, 1};
  FcitxClipboardItemAction clipboard_item3_{&factory_, 2};
  FcitxClipboardItemAction clipboard_item4_{&factory_, 3};
  FcitxClipboardItemAction clipboard_item5_{&factory_, 4};
  FcitxClipboardRemoveAction clipboard_remove1_{&factory_, 0};
  FcitxClipboardRemoveAction clipboard_remove2_{&factory_, 1};
  FcitxClipboardRemoveAction clipboard_remove3_{&factory_, 2};
  FcitxClipboardRemoveAction clipboard_remove4_{&factory_, 3};
  FcitxClipboardRemoveAction clipboard_remove5_{&factory_, 4};
  FcitxClipboardClearAction clipboard_clear_action_{&factory_};
  fcitx::Menu cloud_clipboard_menu_;
  FcitxCloudClipboardItemAction cloud_clipboard_item1_{&factory_, 0};
  FcitxCloudClipboardItemAction cloud_clipboard_item2_{&factory_, 1};
  FcitxCloudClipboardItemAction cloud_clipboard_item3_{&factory_, 2};
  FcitxCloudClipboardItemAction cloud_clipboard_item4_{&factory_, 3};
  FcitxCloudClipboardItemAction cloud_clipboard_item5_{&factory_, 4};
  FcitxMaintenanceAction pin_action_{&factory_, 1, msime::linux_host::candidate_pin_label};
  FcitxMaintenanceAction remove_action_{&factory_, 2, "删除候选"};
  FcitxMaintenanceAction fix1_action_{&factory_, 11, msime::linux_host::candidate_fix_label(1).c_str()};
  FcitxMaintenanceAction fix2_action_{&factory_, 12, msime::linux_host::candidate_fix_label(2).c_str()};
  FcitxMaintenanceAction fix3_action_{&factory_, 13, msime::linux_host::candidate_fix_label(3).c_str()};
  FcitxMaintenanceAction fix4_action_{&factory_, 14, msime::linux_host::candidate_fix_label(4).c_str()};
  FcitxMaintenanceAction fix5_action_{&factory_, 15, msime::linux_host::candidate_fix_label(5).c_str()};
  FcitxMaintenanceAction clear_action_{&factory_, 20, "取消固定"};
  fcitx::Menu desktop_tools_menu_;
  FcitxDesktopPanelAction handwriting_action_{&factory_, "handwriting", "手写识别板"};
  FcitxDesktopPanelAction keyboard_action_{&factory_, "keyboard", "屏幕键盘"};
  FcitxDesktopPanelAction desktop_emoji_action_{&factory_, "emoji", "表情与符号"};
  FcitxDesktopPanelAction desktop_clipboard_action_{&factory_, "clipboard", "本地剪贴板"};
  FcitxDesktopPanelAction desktop_voice_action_{&factory_, "voice", "语音面板"};
  FcitxDesktopPanelAction cloud_dictionary_action_{&factory_, "cloud-dictionary", "云词库"};
  FcitxDesktopPanelAction desktop_cloud_clipboard_action_{&factory_, "cloud-clipboard", "云剪贴板"};
  FcitxDesktopPanelAction dictionary_action_{&factory_, "dictionary", "词库…"};
  FcitxDesktopPanelAction settings_action_{&factory_, "settings", "设置…"};
  FcitxToolbarAction toolbar_action_;
  fcitx::Menu toolbar_menu_;
  // What is currently in the submenu, so a rebuild removes exactly what it added.
  std::vector<fcitx::Action *> toolbar_entries_;
  bool toolbarEnabled(fcitx::InputContext *ic);
  void rebuildToolbarMenu(fcitx::InputContext *ic);
  FcitxDesktopPanelAction about_action_{&factory_, "about", "关于水杉输入法"};
  FcitxDesktopPanelAction help_action_{&factory_, "help", "帮助"};
  FcitxDesktopPanelAction feedback_action_{&factory_, "feedback", "反馈"};
  FcitxToolbarEnabledAction toolbar_enabled_action_{&factory_};
  FcitxVoiceEnabledAction voice_enabled_action_{&factory_};
  FcitxPreferenceSaveRetryAction preference_save_retry_action_{&factory_};
  fcitx::Menu input_group_menu_;
  FcitxMenuGroupAction input_group_action_{"输入选项", "方案细节、混合候选、快捷模式与按键选项"};
  FcitxMenuSeparatorAction input_group_separator_;
  fcitx::Menu punctuation_group_menu_;
  FcitxMenuGroupAction punctuation_group_action_{"标点与翻译", "标点细节与候选翻译"};
  FcitxMenuSeparatorAction punctuation_group_separator_;
  fcitx::Menu candidate_group_menu_;
  FcitxMenuGroupAction candidate_group_action_{"候选与词频", "候选窗口、编码显示与词频学习"};
  FcitxMenuSeparatorAction candidate_group_separator_;
  fcitx::Menu emoji_menu_;
  FcitxEmojiItemAction emoji_item1_{&factory_, 0};
  FcitxEmojiItemAction emoji_item2_{&factory_, 1};
  FcitxEmojiItemAction emoji_item3_{&factory_, 2};
  FcitxEmojiItemAction emoji_item4_{&factory_, 3};
  FcitxEmojiItemAction emoji_item5_{&factory_, 4};
  FcitxEmojiPageAction emoji_previous_action_{&factory_, false};
  FcitxEmojiPageAction emoji_next_action_{&factory_, true};
};

// Defined here rather than in the class body because FcitxEngine is only forward-declared there, and the voice action it owns cannot be named until the definition above.
void FcitxState::syncVoiceAction() {
  if (!engine_) return;
  if (voice_enabled_)
    ic_.statusArea().addAction(fcitx::StatusGroup::InputMethod, &engine_->voice_action_);
  else
    ic_.statusArea().removeAction(&engine_->voice_action_);
  ic_.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

// The chord and the status-menu action reset in process; see FcitxEngine::resetSessions. Only a focused, unrestricted, non-private context may ask, as before.
bool FcitxState::reloadService() {
  if (!engine_ || !ic_.hasFocus() || restricted() || privateInput()) return false;
  engine_->resetSessions();
  return true;
}

void FcitxState::syncCandidatePanelFont() {
  if (!engine_) return;
  engine_->applyCandidatePanelFont(preferences_);
  engine_->applyCandidateWheelPaging(preferences_);
}

void FcitxState::syncCandidatePanelTheme() {
  if (engine_ && session_) engine_->applyCandidatePanelTheme(preferences_, system_dark_, candidate_skin_document_);
  if (engine_) engine_->publishCandidatePanelStatus();
}

void FcitxState::refreshThemeMenu() {
  if (engine_) engine_->rebuildThemeMenu(&ic_);
}

void FcitxState::refreshSchemeMenu() {
  if (engine_) engine_->rebuildSchemeMenu(&ic_, schemeAvailable("cantonese"), schemeAvailable("zhuyin"));
}

void FcitxState::refreshToolbar() {
  if (!engine_) return;
  engine_->rebuildToolbarMenu(&ic_);
  if (engine_->toolbarEnabled(&ic_))
    ic_.statusArea().addAction(fcitx::StatusGroup::InputMethod, &engine_->toolbar_action_);
  else
    ic_.statusArea().removeAction(&engine_->toolbar_action_);
  ic_.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

// The aux line below a candidate page: the page number, the local mode, the reading when the candidate preedit shows it, and the typing combo while there is one.
std::string FcitxState::candidateAux() const {
  std::string aux = std::to_string(view_.at("page").get<int>() + 1) +
      "/" + std::to_string(view_.at("page_count").get<int>());
  if (!preferences_.value("show_candidate_page_number", true)) aux.clear();
  const auto mode = view_.value("local_mode", std::string("none"));
  if (const char *modeLabel = msime::linux_host::candidate_local_mode_label(mode))
    aux += (aux.empty() ? "" : " · ") + std::string(modeLabel);
  if (preferences_.value("candidate_preedit_style", std::string("pinyin")) == "pinyin") {
    const auto candidatePreedit = view_.value("preedit", std::string{});
    if (!candidatePreedit.empty()) {
      const auto editing = view_.value("editing_text", std::string());
      const auto caret = std::min(editing.size(), view_.value("caret_position", editing.size()));
      const auto displayed = msime::linux_host::candidate_preedit_with_caret(
          candidatePreedit, editing, caret);
      if (!displayed.empty()) aux += (aux.empty() ? "" : " · ") + displayed;
    }
  }
  const auto combo = msime::linux_host::typing_combo_label(typing_combo_);
  if (!combo.empty()) aux += (aux.empty() ? "" : " · ") + combo;
  return aux;
}

void FcitxState::render() {
  if (engine_) {
    engine_->english_action_.update(&ic_);
    engine_->width_action_.update(&ic_);
  }
  refreshModeIndicator();
  ic_.inputPanel().reset();
  const auto editing = view_.value("editing_text", std::string());
  const auto style = preferences_.value("tsf_preedit_style", std::string("raw"));
  const int inlineScheme = typingScheme();
  const bool alwaysInline = inlineScheme >= 0 && msime::linux_host::scheme::AlwaysInlinePreedit(inlineScheme);
  if (!voice_preedit_.empty()) {
    fcitx::Text preedit(voice_preedit_, fcitx::TextFormatFlag::Underline);
    preedit.setCursor(static_cast<int>(voice_preedit_.size()));
    if (ic_.capabilityFlags().test(fcitx::CapabilityFlag::Preedit))
      ic_.inputPanel().setClientPreedit(preedit);
    else
      ic_.inputPanel().setPreedit(preedit);
  } else if (style != "empty" || alwaysInline) {
    // A Korean syllable, Zhuyin conversion or Vietnamese word is text the user is writing, so it is drawn inline whatever the preedit style: until a list opens there is no candidate window to show it in (core/InputSchemeTraits.h, AlwaysInlinePreedit).
    auto reading = style == "pinyin" || alwaysInline ? view_.value("preedit", editing) : editing;
    // A Japanese composition is かな, not the letters that produced it; see
    // ../src/core/PhrasePreedit.h for the one case that keeps the letters.
    const auto kana = view_.value("reading", std::string{});
    if (msime::linux_host::composition_shows_reading(
            kana, view_.value("caret_position", size_t{}), editing.size()))
      reading = kana;
    // The piece already picked for the phrase leads the reading, the way the reference draws
    // `word_for_creating_word`. fcitx5 takes the cursor as a byte offset into the string it is
    // given, which is why the offset comes from the same place the text does.
    const auto composed = msime::linux_host::compose_phrase_preedit(
        view_.value("phrase_prefix", std::string{}), reading,
        view_.value("caret_position", size_t{}));
    fcitx::Text preedit(composed.text, fcitx::TextFormatFlag::Underline);
    if (reading == editing)
      preedit.setCursor(static_cast<int>(composed.caret_bytes));
    // The caret always follows an inline composition; the runtime keeps no caret inside it.
    else if (alwaysInline)
      preedit.setCursor(static_cast<int>(composed.text.size()));
    if (ic_.capabilityFlags().test(fcitx::CapabilityFlag::Preedit))
      ic_.inputPanel().setClientPreedit(preedit);
    else ic_.inputPanel().setPreedit(preedit);
  }
  if (!view_.at("candidates").empty()) {
    // Look up the registered factory via the owning engine for stable candidate callbacks.
    if (engine_) ic_.inputPanel().setCandidateList(std::make_unique<FcitxPage>(*this, &engine_->factory_));
    ic_.inputPanel().setAuxDown(fcitx::Text(candidateAux()));
  }
  if (emoji_search_mode_)
    ic_.inputPanel().setAuxUp(fcitx::Text("Emoji 搜索：" + emoji_search_));
  if (voice_loading_ && (!wave_overlay_surface_ || wave_overlay_failed_))
    updateVoiceOverlay();
  ic_.updatePreedit();
  ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

void FcitxState::maintenance(int operation) {
  if (!ensure() || view_.value("candidates", Json::array()).empty()) return;
  for (const auto &candidate : view_.at("candidates")) {
    if (!candidate.value("highlighted", false)) continue;
    const auto &id = candidate.at("id");
    const auto generation = id.at("generation").get<uint64_t>();
    const auto index = id.at("index").get<size_t>();
    char *raw = nullptr;
    if (operation == 1) raw = msime_client_pin_candidate(session_, generation, index);
    else if (operation == 2 && msime::linux_host::candidate_dictionary_removal_available(
                 view_.value("scheme", 0u), candidate.value("source", 0u),
                 candidate.value("text", std::string{})))
      raw = msime_client_remove_candidate(session_, generation, index);
    else if (operation >= 11 && operation <= 15)
      raw = msime_client_fix_candidate_position(session_, generation, index,
                                                 static_cast<uint8_t>(operation - 10));
    else if (operation == 20)
      raw = msime_client_clear_candidate_position(session_, generation, index);
    if (raw) apply(raw);
    return;
  }
}

// The badge's theme, by the macOS badge's rule: its mode is toolbar_theme when that names one, otherwise the global mode, whose "system" (跟随系统) default follows the desktop. A global theme with a fixed appearance (水杉 is dark, 纸白 light) decides it either way, and the colours are that theme's palette as the floating toolbar takes it.
msime::linux_host::CandidateTheme fcitx_mode_badge_theme(const Json &preferences, bool system_dark, const Json &catalog) {
  const bool dark = msime::linux_host::surface_dark_theme(preferences, "toolbar_theme", system_dark);
  return resolveThemeInMode(preferences, dark, catalog);
}

void FcitxState::showInputModeHud() {
#ifdef MSIME_FCITX5_CUSTOM_IM_INFORMATION
  // 共享偏好 input_mode_hud 控制，默认开启。不自己画窗口——Fcitx5 的面板本来就提供这个
  // 弹出物，而且它明说是给「输入法内部开关」用的：由面板负责定位、不抢焦点、到时自动
  // 消失，正是这个提示需要的三件事。
  if (!preferences_.value("input_mode_hud", true)) return;
  if (restricted() || privateInput() || !ic_.hasFocus()) return;
  const std::string label = input_enabled_ ? "中" : "英";
#ifdef MSIME_FCITX5_MODE_BADGE
  // 自绘徽章带产品 logo，面板那个提示只能显示文字。连不上合成器或没有 layer-shell 时
  // 记下来不再重试，回退到文字提示——提示少一张图，好过没有提示。
  if (!mode_badge_ && !mode_badge_unavailable_) {
    mode_badge_ = msime::linux_host::ModeBadgeSurface::create();
    mode_badge_unavailable_ = !mode_badge_;
  }
  // 两个提示各补一半：面板那个由合成器按光标矩形定位，跟着输入点走，但只能显示文字；
  // 自绘徽章带得了 logo，却只能用屏幕坐标固定在一个角上。两者同时发是所有者的选择。
  // Sized by the shared floating toolbar preferences and coloured by the resolved theme, both read at every switch so a settings change shows at the next one.
  const msime::linux_host::ModeBadgeStyle style{
      msime::linux_host::mode_badge_metrics(preferences_),
      msime::linux_host::floating_surface_colors(
          fcitx_mode_badge_theme(preferences_, system_dark_, candidate_skin_document_))};
  if (mode_badge_ && mode_badge_->show(label, MSIME_MODE_BADGE_ICON, style)) scheduleModeBadgeHide();
#endif
  if (auto *instance = engine_->instance())
    instance->showCustomInputMethodInformation(&ic_, label);
#endif
}

bool FcitxState::key(fcitx::KeyEvent &event) {
  const auto &key = event.key();
  const auto sym = key.sym();
  const auto states = key.states();
  if (sym == FcitxKey_BackSpace && event.isRelease()) {
    const bool owned = backspace_hold_.armed();
    backspace_hold_.release();
    if (owned) return true;
  } else if (!event.isRelease() && sym != FcitxKey_BackSpace) {
    backspace_hold_.reset();
  }
  // An accepted stroke owns its repeats and release, even if modifiers or
  // preferences change while held. Unmatched releases must not stop voice.
  if (sym == FcitxKey_F9 && voice_f9_held_) {
    if (event.isRelease()) voice_f9_held_ = false;
    return true;
  }
  if ((sym == FcitxKey_r || sym == FcitxKey_R) && maintenance_reload_held_) {
    if (event.isRelease()) maintenance_reload_held_ = false;
    return true;
  }
  // A toggle chord flips its setting once per press, as on Windows: auto-repeat while it is held is swallowed instead of flipping the setting back and forth, and the release ends the hold. F and f are one key, because letting go of Shift first changes the keysym of the release.
  if (toggle_chord_held_ != FcitxKey_None &&
      (sym == FcitxKey_F ? FcitxKey_f : sym) == toggle_chord_held_) {
    if (event.isRelease()) toggle_chord_held_ = FcitxKey_None;
    return true;
  }
  if (sym == FcitxKey_Alt_R && voice_ralt_held_) {
    if (event.isRelease()) {
      voice_ralt_held_ = false;
      if (voice_loading_ && !voice_space_locked_) stopVoice();
    }
    return true;
  }
  const bool controlKey = sym == FcitxKey_Control_L || sym == FcitxKey_Control_R;
  const bool superKey = sym == FcitxKey_Super_L || sym == FcitxKey_Super_R;
  const bool rightControlKey = sym == FcitxKey_Control_R;
  const bool rightAltKey = sym == FcitxKey_Alt_R;
  if (voice_ctrl_win_held_ && (controlKey || superKey)) {
    if (event.isRelease()) {
      voice_ctrl_win_held_ = false;
      if (voice_loading_ && !voice_space_locked_) stopVoice();
    }
    return true;
  }
  if (voice_rctrl_ralt_held_ && (rightControlKey || rightAltKey)) {
    if (event.isRelease()) {
      voice_rctrl_ralt_held_ = false;
      if (voice_loading_ && !voice_space_locked_) stopVoice();
    }
    return true;
  }
  if (sym == FcitxKey_space && voice_space_consumed_) {
    if (event.isRelease()) voice_space_consumed_ = false;
    return true;
  }
  // A bare modifier switches on its release, which is the half this host never
  // saw: everything below returns before looking at releases.
  {
    const bool shiftRelease = isShiftKey(event);
    const bool ctrlRelease = !shiftRelease && isCtrlKey(event);
    if ((shiftRelease || ctrlRelease) && event.isRelease()) {
      // 有的前端只送来松开。同一套 xkb 选项下实测：Shift 的按下事件根本不会到达引擎，
      // 只有松开会（Ctrl 则两者都有）。没有按下就没有布防，判据改由按键自己的修饰位
      // 给出：这段时间里若有普通按键带着这个修饰位，它就是组合键的一半——按住 Shift
      // 打出的大写字母带 Shift 位，而敲完 ni 再点一下 Shift 不带。这比按时间窗口判断准，
      // 也正好容得下「组字途中切英文」这个最常用的操作。
      const bool armed = shiftRelease ? pure_shift_candidate_ : pure_ctrl_candidate_;
      const bool combined = shiftRelease ? shift_in_combination_ : ctrl_in_combination_;
      const bool candidate = armed || (!shift_down_ && !ctrl_down_ && !combined);
      if (shiftRelease) shift_in_combination_ = false;
      else ctrl_in_combination_ = false;
      const bool enabled = shiftRelease ? mode_shift_enabled_ : mode_ctrl_enabled_;
      if (shiftRelease) {
        shift_down_ = false;
        pure_shift_candidate_ = false;
      } else {
        ctrl_down_ = false;
        pure_ctrl_candidate_ = false;
      }
      // Held too long is a modifier being used, not a gesture; the other
      // modifiers being down says the same thing.
      // 按住时长这条判据来自按下事件；没有按下事件时它无从谈起，改由上面的「松开前
      // 500ms 内没有普通按键」承担同一件事，不能在这里再要求一个从未设过的截止时刻。
      const bool within_window =
          !armed || std::chrono::steady_clock::now() <= modifier_toggle_deadline_;
      if (candidate && enabled && within_window && ic_.hasFocus() && !restricted() &&
          !privateInput() &&
          !states.testAny(fcitx::KeyStates{fcitx::KeyState::Alt, fcitx::KeyState::Super,
                                          fcitx::KeyState::Hyper}) &&
          !(shiftRelease ? ctrl_down_ : shift_down_)) {
        try {
          // Same follow-up as the existing Ctrl+Shift+E and Ctrl+Shift+Space
          // chords: the toggle redraws the input panel, and the status area
          // re-reads its own checked state when Fcitx5 next draws it.
          // Windows toggles on the bare modifier release but still lets the application see it, so a program tracking Shift or Ctrl state does not keep it latched.
          if (ensure()) toggleInputMode();
        } catch (...) {
          close();
          clearPanel();
        }
      }
      return false;
    }
  }
  // 裸修饰键在按下时布防，松开时才切换。这段必须排在下面那两条返回之前——「松开或修饰
  // 键一律不处理」与「英文透传时不处理」：它本来写在后面，于是 Shift 按下每次都在那条 return 上结束，布防从未发生，松开
  // 那一侧看到的候选状态永远是 false——四个模式快捷键里的裸 Shift 与裸 Ctrl 因此在这个
  // 宿主上一次都没生效过，而设置页按能力位把它们全都显示着。切到英文之后同样要能切回
  // 来，所以也不能排在「英文透传时不处理」后面；IBus 宿主一直是这么做的。
  {
    const bool shiftKey = isShiftKey(event);
    const bool ctrlKey = !shiftKey && isCtrlKey(event);
    if ((shiftKey || ctrlKey) && !event.isRelease()) {
      const bool otherModifiers = states.testAny(fcitx::KeyStates{
          fcitx::KeyState::Alt, fcitx::KeyState::Super, fcitx::KeyState::Hyper});
      const bool wasDown = shiftKey ? shift_down_ : ctrl_down_;
      if (shiftKey) shift_down_ = true;
      if (ctrlKey) ctrl_down_ = true;
      if (!wasDown) {
        modifier_toggle_deadline_ =
            std::chrono::steady_clock::now() + std::chrono::milliseconds(500);
        pure_shift_candidate_ = shiftKey && mode_shift_enabled_ && !otherModifiers &&
                                !states.test(fcitx::KeyState::Ctrl);
        pure_ctrl_candidate_ = ctrlKey && mode_ctrl_enabled_ && !otherModifiers &&
                               !states.test(fcitx::KeyState::Shift);
      }
      return false;
    }
    // 普通按键带着哪个修饰位，就说明那个修饰键此刻是被按住用的，不是在做手势。这里同样
    // 必须排在下面两条返回之前：大写字母等分支在更后面就返回了，记在那里会漏掉。
    if (!shiftKey && !ctrlKey && !event.isRelease()) {
      // 这里要看 rawKey：fcitx5 会把按键归一化，Shift+a 变成符号 A 并且把 Shift 位抹掉，
      // 于是归一化之后的 states 看不出修饰键被按住过。判「是不是组合键」必须用原始事件。
      const auto raw = event.rawKey().states();
      if (raw.test(fcitx::KeyState::Shift)) shift_in_combination_ = true;
      if (raw.test(fcitx::KeyState::Ctrl)) ctrl_in_combination_ = true;
    }
  }
  if (event.isRelease()) return false;
  const bool bareBackspace = sym == FcitxKey_BackSpace &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                       fcitx::KeyState::Shift, fcitx::KeyState::Super,
                                       fcitx::KeyState::Hyper});
  if (!bareBackspace) {
    backspace_hold_.reset();
  } else {
    if (backspace_hold_.press(composingOrCandidates())) return true;
  }
  if (sym == FcitxKey_BackSpace) {
    if (last_smart_punctuation_ != 0) {
      smart_punctuation_rejected_ = last_smart_punctuation_;
      last_smart_punctuation_ = 0;
      last_smart_punctuation_at_ = {};
    }
  } else if (smart_punctuation_rejected_ != 0) {
    const auto typed = fcitx::Key::keySymToUTF8(sym);
    if (typed.size() != 1 || typed[0] != smart_punctuation_rejected_)
      smart_punctuation_rejected_ = 0;
  }
  // Only a Space arriving immediately after the mark, with nothing in between,
  // can take it back; anything else means the user moved on. A successful
  // conversion consumes the key.
  if (!space_convert_mark_.empty()) {
    if (sym == FcitxKey_space && states.testAny(fcitx::KeyStates{
                                     fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                     fcitx::KeyState::Shift, fcitx::KeyState::Super,
                                     fcitx::KeyState::Hyper}) == false) {
      if (convertSmartPunctuationSpace())
        return true;
    } else {
      space_convert_mark_.clear();
      space_convert_preceding_.clear();
    }
  }
  if ((sym == FcitxKey_k || sym == FcitxKey_K) &&
      states.test(fcitx::KeyState::Ctrl) && states.test(fcitx::KeyState::Shift) &&
      states.test(fcitx::KeyState::Super) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Alt, fcitx::KeyState::Hyper}) &&
      ic_.hasFocus() && !restricted() && !privateInput() && ensure())
    return launchDesktopPanel("keyboard");
  if (voice_hotkey_rctrl_ralt_ && voice_enabled_ && !voice_socket_.empty() &&
      (rightControlKey || rightAltKey) &&
      states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt}) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Shift, fcitx::KeyState::Super,
                                       fcitx::KeyState::Hyper}) &&
      !restricted() && !privateInput() && ic_.hasFocus()) {
    if (!voice_loading_ && !requestVoice()) return false;
    voice_rctrl_ralt_held_ = true;
    return true;
  }
  if (voice_hotkey_ctrl_win_ && voice_enabled_ && !voice_socket_.empty() &&
      (controlKey || superKey) &&
      states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Super}) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Alt, fcitx::KeyState::Shift,
                                       fcitx::KeyState::Hyper}) &&
      !restricted() && !privateInput() && ic_.hasFocus()) {
    if (!voice_loading_ && !requestVoice()) return false;
    voice_ctrl_win_held_ = true;
    return true;
  }
  if (sym == FcitxKey_Alt_R && voice_hotkey_ralt_ && voice_enabled_ && !voice_socket_.empty() &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper}) &&
      !restricted() && !privateInput() && ic_.hasFocus()) {
    if (!voice_loading_ && !requestVoice()) return false;
    voice_ralt_held_ = true;
    return true;
  }
  if (event.isRelease() || key.isModifier()) return false;
  // A held phrase piece with no reading left is still a composition (core/PhrasePreedit.h): Enter commits it, Escape discards it and Ctrl+Backspace deletes it.
  const bool composing = msime::linux_host::view_has_composition(
      view_.value("editing_text", std::string()), false,
      view_.value("phrase_prefix", std::string()));
  const bool ctrl = states.test(fcitx::KeyState::Ctrl);
  const bool alt = states.test(fcitx::KeyState::Alt);
  const bool shift = states.test(fcitx::KeyState::Shift);
  // Match the Windows/IBus hold-to-record interaction: pressing Space while
  // a modifier-held voice recording is active locks recognition and consumes
  // the complete Space stroke so it cannot leak into the editor. Ctrl+F9 is a
  // toggle recording shortcut, not a hold-to-record shortcut.
  const bool holdVoice = voice_loading_ && voice_hotkey_hold_space_lock_ &&
      (voice_ralt_held_ || voice_ctrl_win_held_ || voice_rctrl_ralt_held_);
  if (sym == FcitxKey_space && holdVoice) {
    voice_space_consumed_ = true;
    voice_space_locked_ = true;
    return true;
  }
  if (sym == FcitxKey_F9 && ctrl && !alt && !shift &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Mod5}) &&
      voice_hotkey_ctrl_f9_ && voice_enabled_ && !voice_socket_.empty() && !restricted() && !privateInput() &&
      ic_.hasFocus()) {
    if (voice_loading_) {
      if (!stopVoice()) return false;
    } else if (!requestVoice()) return false;
    voice_f9_held_ = true;
    return true;
  }
  if (ctrl && shift && alt &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper})) {
    if (sym == FcitxKey_c || sym == FcitxKey_C) return resetCache();
    if (sym == FcitxKey_r || sym == FcitxKey_R) {
      if (!ic_.hasFocus() || restricted() || privateInput()) return false;
      if (!reloadService()) return false;
      maintenance_reload_held_ = true;
      return true;
    }
    std::optional<size_t> slot;
    if (sym >= FcitxKey_1 && sym <= FcitxKey_8)
      slot = static_cast<size_t>(sym - FcitxKey_1);
    else if (sym >= FcitxKey_KP_1 && sym <= FcitxKey_KP_8)
      slot = static_cast<size_t>(sym - FcitxKey_KP_1);
    if (slot) return removeCandidateSlot(*slot);
  }
  // These host-level chords remain active while ordinary input is being passed
  // through in English mode. Keeping the passthrough gate above this block made
  // Ctrl+Space a one-way switch: it could leave Chinese mode but never return.
  pure_shift_candidate_ = false;
  pure_ctrl_candidate_ = false;
  if (sym == FcitxKey_space && ctrl && !shift &&
      (alt ? mode_ctrl_alt_space_enabled_ : true)) {
    if (composing) command(MSIME_COMMIT_RAW);
    if (!toggleInputMode()) return false;
    toggle_chord_held_ = sym;
    return true;
  }
  if (sym == FcitxKey_space && ctrl && shift && !alt) {
    if (composing) command(MSIME_COMMIT_RAW);
    if (!toggleWidth()) return false;
    toggle_chord_held_ = sym;
    return true;
  }
  // Windows eats Ctrl+. with the IME closed too and flips its punctuation compartment, which a pinned lock holds in place. Nothing is saved: the next Chinese/English switch (resyncPunctuationForMode) undoes it.
  if (!input_enabled_ && sym == FcitxKey_period && ctrl && !shift && !alt &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper})) {
    if (!ic_.hasFocus() || restricted()) return false;
    if (punctuation_lock_ == 0) {
      english_chinese_punctuation_ = !english_chinese_punctuation_;
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
    }
    return true;
  }
  if (!input_enabled_) {
    // English mode still honours fullwidth output, the "always Chinese punctuation" lock and a Ctrl+. choice made in English mode, as Windows does with the IME closed; everything else passes through.
    if (ctrl || alt ||
        states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                        fcitx::KeyState::Meta}) ||
        !ic_.hasFocus() || restricted())
      return false;
    const bool keypad = sym >= FcitxKey_KP_Space && sym <= FcitxKey_KP_9;
    const auto text = msime::linux_host::english_mode_output(
        static_cast<char32_t>(fcitx::Key::keySymToUnicode(sym)), keypad,
        punctuation_lock_ == 1 || (punctuation_lock_ == 0 && english_chinese_punctuation_),
        fullwidthOutput(), english_punctuation_);
    if (text.empty()) return false;
    commitText(text, msime::linux_host::TypingSource::English);
    return true;
  }
  if (character_set_shortcut_enabled_ && ctrl && shift && !alt &&
      (sym == FcitxKey_f || sym == FcitxKey_F)) {
    // The composition stays: the character set only changes how its candidates are written, as on Windows.
    if (!toggleTraditional()) return false;
    toggle_chord_held_ = FcitxKey_f;
    return true;
  }
  if (ctrl && shift && !alt && (sym == FcitxKey_e || sym == FcitxKey_E)) {
    if (composing) command(MSIME_COMMIT_RAW);
    return toggleEnglish();
  }
  // Ctrl+. switches Chinese and English punctuation, as it does on Windows and in the IBus engine. The composition is left alone: only punctuation typed from here on changes.
  if (sym == FcitxKey_period && ctrl && !shift && !alt &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper}))
    return toggleChinesePunctuation();
  if (emoji_search_mode_) {
    if (sym == FcitxKey_Escape) {
      endEmojiSearch();
      return true;
    }
    if (states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                        fcitx::KeyState::Super, fcitx::KeyState::Hyper}))
      return true;
    if (sym == FcitxKey_BackSpace) {
      if (!emoji_search_.empty()) emoji_search_.pop_back();
      emoji_items_.clear();
      emoji_offset_ = {};
      emoji_next_offset_ = {};
      emoji_complete_ = false;
      emoji_previous_offsets_.clear();
      if (!emoji_job_.valid()) requestEmojiPage({});
      render();
      return true;
    }
    if (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter) {
      refreshEmoji();
      if (!emoji_items_.empty()) {
        const auto &item = emoji_items_.front();
        const auto text = item.is_string() ? item.get<std::string>() : item.value("text", std::string{});
        if (!text.empty()) commitText(text, msime::linux_host::TypingSource::Local);
      }
      endEmojiSearch();
      return true;
    }
    const auto searchText = fcitx::Key::keySymToUTF8(sym);
    if (!shift && searchText.size() == 1 &&
        ((searchText[0] >= 'a' && searchText[0] <= 'z') ||
         (searchText[0] >= '0' && searchText[0] <= '9') || searchText == " ")) {
      if (emoji_search_.size() < 256) emoji_search_.append(searchText);
      emoji_items_.clear();
      emoji_offset_ = {};
      emoji_next_offset_ = {};
      emoji_complete_ = false;
      emoji_previous_offsets_.clear();
      if (!emoji_job_.valid()) requestEmojiPage({});
      render();
      return true;
    }
    return true;
  }
  if (sym == FcitxKey_Escape && voice_loading_) {
    cancelVoice();
    return composing ? command(MSIME_CANCEL) : true;
  }
  if (translation_candidates_active_) {
    if (ctrl && !alt && !shift &&
        (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter))
      return true;
    if (sym == FcitxKey_Escape) {
      exitTranslationCandidates();
      return command(MSIME_CANCEL);
    }
    const auto pageSize = std::clamp(
        translation_saved_view_.value("page_size", size_t{9}), size_t{1}, size_t{9});
    const auto pageStart = translation_page_ * pageSize;
    const auto pageEnd = std::min(pageStart + pageSize, translation_options_.size());
    if (!ctrl && !alt && !shift && (sym == FcitxKey_space ||
                                    (sym >= FcitxKey_1 && sym <= FcitxKey_9) ||
                                    (sym >= FcitxKey_KP_1 && sym <= FcitxKey_KP_9))) {
      const auto slot = sym == FcitxKey_space
                            ? translation_cursor_
                            : static_cast<size_t>(sym >= FcitxKey_KP_1
                                                      ? sym - FcitxKey_KP_1
                                                      : sym - FcitxKey_1);
      const auto index = pageStart + slot;
      if (index < translation_options_.size()) commitTranslationCandidate(index);
      return true;
    }
    if (!ctrl && !alt && !shift &&
        (sym == FcitxKey_Up || sym == FcitxKey_KP_Up ||
         sym == FcitxKey_Down || sym == FcitxKey_KP_Down)) {
      if (sym == FcitxKey_Up || sym == FcitxKey_KP_Up)
        translation_cursor_ = translation_cursor_ == 0 ? 0 : translation_cursor_ - 1;
      else if (translation_cursor_ + 1 < pageEnd - pageStart)
        ++translation_cursor_;
      renderTranslationCandidates();
      return true;
    }
    if (!ctrl && !alt && !shift &&
        (sym == FcitxKey_Page_Up || sym == FcitxKey_KP_Page_Up ||
         sym == FcitxKey_Page_Down || sym == FcitxKey_KP_Page_Down)) {
      translationPage(sym == FcitxKey_Page_Up || sym == FcitxKey_KP_Page_Up
                          ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE);
      return true;
    }
    // Tab pages the senses the way it pages an ordinary candidate list below; leaving the overlay first would page the Engine's hidden list instead. Fcitx normalises ISO_Left_Tab to Tab, so the raw key tells a Shift-less back-tab apart.
    if (!ctrl && !alt && navigation_.value("tab", true) &&
        !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Mod5}) &&
        (sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab)) {
      translationPage(shift || event.rawKey().sym() == FcitxKey_ISO_Left_Tab
                          ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE);
      return true;
    }
    // Other editing keys first restore the Engine-owned candidate page below.
    exitTranslationCandidates();
  }
  // Match the Windows candidate-translation shortcut. Fcitx owns the
  // candidate panel, so commit the currently highlighted rendered gloss
  // directly for one sense, or expose a temporary native candidate page for
  // multiple senses; without a valid gloss the chord remains an application shortcut.
  if (ctrl && !alt && !shift &&
      (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter) && composing &&
      preferences_.value("candidate_translations", false)) {
    const auto candidates = view_.value("candidates", Json::array());
    if (candidates.is_array()) {
      for (const auto &candidate : candidates) {
        if (!candidate.is_object() || !candidate.value("highlighted", false))
          continue;
        const auto translation = candidate.value("translation", std::string{});
        if (!translation.empty() && translation.size() <= 4096) {
          if (enterTranslationCandidates(translation)) return true;
          const auto senses = msime::linux_host::split_translation_gloss(translation);
          if (!senses.empty()) {
            commitText(senses.front(), msime::linux_host::TypingSource::Reply);
            command(MSIME_CANCEL);
            return true;
          }
        }
        break;
      }
    }
  }
  // A key the active local mode or scheme spells with is input before any binding below can claim it: a paired closing mark, a page or word-character key, a candidate digit, a paired bracket or smart punctuation (core/SpellingSymbols.h). Space is one of them only while a Zhuyin syllable composes, where it is the first tone.
  if (!states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                       fcitx::KeyState::Meta, fcitx::KeyState::Mod5})) {
    const auto spelled = static_cast<char32_t>(fcitx::Key::keySymToUnicode(sym));
    if (msime::linux_host::engine_spelling(view_, spelled) ||
        (spelled == U' ' && !states.test(fcitx::KeyState::Shift) && msime::linux_host::spelling_space(view_)))
      return apply(msime_client_character(session_, static_cast<uint8_t>(spelled),
                                          event.rawKey().states().test(fcitx::KeyState::Shift)));
  }
  if (skipPairedClosing(sym, states)) return true;
  switch (sym) {
  case FcitxKey_BackSpace: case FcitxKey_Delete: case FcitxKey_KP_Delete:
  case FcitxKey_Return: case FcitxKey_KP_Enter: case FcitxKey_Escape:
  case FcitxKey_Left: case FcitxKey_KP_Left: case FcitxKey_Right: case FcitxKey_KP_Right:
  case FcitxKey_Up: case FcitxKey_KP_Up: case FcitxKey_Down: case FcitxKey_KP_Down:
  case FcitxKey_Home: case FcitxKey_KP_Home: case FcitxKey_End: case FcitxKey_KP_End:
  case FcitxKey_Page_Up: case FcitxKey_KP_Page_Up: case FcitxKey_Page_Down:
  case FcitxKey_KP_Page_Down: case FcitxKey_Tab: case FcitxKey_KP_Tab:
  case FcitxKey_ISO_Left_Tab:
    paired_tracker_.clear();
    break;
  default: break;
  }
  // AltGr picks layout text (German @ or [, for example), so it is neither an application shortcut nor IME punctuation: finish the spelling and let the character through, as the IBus host and Windows do.
  if (states.test(fcitx::KeyState::Mod5) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper}) &&
      !fcitx::Key::keySymToUTF8(sym).empty()) {
    if (composing) command(MSIME_COMMIT_RAW);
    return false;
  }
  // Keep Ctrl-only segment editing consistent with IBus and the Windows composition editor. The shared runtime resolves the actual segment boundaries and falls back safely for local modes. A Korean syllable, Zhuyin conversion or Vietnamese word has no segments, so there the chord finishes it below and stays the application's shortcut.
  if (ctrl && !alt && !shift && composing && !commitsOnBlur()) {
    if (sym == FcitxKey_BackSpace) return command(MSIME_BACKSPACE_SEGMENT);
    if (sym == FcitxKey_Left || sym == FcitxKey_KP_Left)
      return command(MSIME_MOVE_LEFT_SEGMENT);
    if (sym == FcitxKey_Right || sym == FcitxKey_KP_Right)
      return command(MSIME_MOVE_RIGHT_SEGMENT);
  }
  if (states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                      fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                      fcitx::KeyState::Mod5})) {
    // A Korean syllable, Zhuyin conversion or Vietnamese word is already text, so a shortcut finishes it rather than throwing it away.
    if (composing) command(commitsOnBlur() ? MSIME_FINISH_COMPOSITION : MSIME_CANCEL);
    return false;
  }
  // CapsLock uppercase letters belong to the editor when a new composition has not started, matching the Windows and IBus host routers. Korean letters are jamo whatever CapsLock says, and a Vietnamese word starts in capitals, so both still compose.
  if (!(typingScheme() >= 0 && msime::linux_host::scheme::CapsLockBypassExempt(typingScheme())) &&
      states.test(fcitx::KeyState::CapsLock) && !shift &&
      sym >= FcitxKey_A && sym <= FcitxKey_Z &&
      view_.value("editing_text", std::string{}).empty() &&
      view_.value("candidates", Json::array()).empty())
    return false;
  // Hangul_Hanja, or a bare F9, converts the composing Korean syllable to Hanja, the keys of fcitx5-hangul and ibus-hangul; pressed again with the list open it closes it (msime_client.h, MSIME_OPEN_CANDIDATE_LIST). A composing Zhuyin conversion opens its candidate list with the same keys. Ctrl+F9 is the voice toggle above. While a composition is open the key stays the input method's whatever the Engine answers: a lone jamo has no Hanja, and the tail of this function would write the composition out and hand the key to the application. With nothing composing it is the application's as before.
  static_assert(msime::linux_host::kKeysymHangulHanja == FcitxKey_Hangul_Hanja &&
                msime::linux_host::kKeysymF9 == FcitxKey_F9);
  if (composing && msime::linux_host::korean_hanja_key(sym) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Shift,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Meta,
                                       fcitx::KeyState::Mod5}) &&
      msime::linux_host::candidate_list_composition(view_)) {
    command(MSIME_OPEN_CANDIDATE_LIST);
    return true;
  }
  // Down opens the list of a composing Zhuyin conversion, as in libchewing; with the list open it moves the highlight below like in any list.
  if (composing && (sym == FcitxKey_Down || sym == FcitxKey_KP_Down) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Shift,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Meta,
                                       fcitx::KeyState::Mod5}) &&
      msime::linux_host::zhuyin_list_down_key(view_)) {
    command(MSIME_OPEN_CANDIDATE_LIST);
    return true;
  }
  // With its Hanja list open a Korean syllable has candidates, and the candidate block below takes the keys as it does for any list; so does a Zhuyin conversion with its list open.
  const bool koreanHanjaList = msime::linux_host::korean_hanja_list_open(view_);
  const bool openedList = msime::linux_host::opened_candidate_list(view_);
  // Otherwise a Korean syllable has no candidates. The keys that end it send it to the application as a commit and then do their own work there (the transition is unhandled), as in every Korean input method; Escape discards it and Backspace takes back one jamo. Every other key falls through: a letter composes, a digit or a mark ends the syllable through the runtime, and anything else finishes it at the end of this function. A Zhuyin conversion with its list closed and a Vietnamese word end the same way.
  if (composing && commitsOnBlur() && !openedList) {
    switch (sym) {
    case FcitxKey_Escape: return command(MSIME_CANCEL);
    case FcitxKey_BackSpace: return command(MSIME_BACKSPACE);
    case FcitxKey_Delete: case FcitxKey_KP_Delete: return command(MSIME_DELETE_FORWARD);
    case FcitxKey_Return: case FcitxKey_KP_Enter: return command(MSIME_COMMIT_RAW);
    case FcitxKey_space: return command(MSIME_COMMIT_CANDIDATE);
    case FcitxKey_Left: case FcitxKey_KP_Left: return command(MSIME_MOVE_LEFT);
    case FcitxKey_Right: case FcitxKey_KP_Right: return command(MSIME_MOVE_RIGHT);
    case FcitxKey_Home: case FcitxKey_KP_Home: return command(MSIME_MOVE_HOME);
    case FcitxKey_End: case FcitxKey_KP_End: return command(MSIME_MOVE_END);
    default: break;
    }
  }
  if (composing && (!commitsOnBlur() || openedList)) {
    const bool japanese = view_.value("scheme", 0u) == 3;
    // The marks among these keys stay punctuation while a Korean Hanja list is open, as they are with no list (core/KoreanHanja.h): the Engine closes the list and writes the Hangul with the mark. Page Up, Page Down and Tab still page.
    if (!shift && !view_.at("candidates").empty() && !koreanHanjaList) {
      if (word_character_enabled_ && !japanese &&
          ((word_character_minus_equal_ && sym == FcitxKey_minus) ||
           (!word_character_minus_equal_ && sym == FcitxKey_bracketleft)))
        return selectEdge(MSIME_FIRST_HAN);
      if (word_character_enabled_ && !japanese &&
          ((word_character_minus_equal_ && sym == FcitxKey_equal) ||
           (!word_character_minus_equal_ && sym == FcitxKey_bracketright)))
        return selectEdge(MSIME_LAST_HAN);
      if ((sym == FcitxKey_minus && !japanese && navigation_.value("minus_equal", true)) ||
          (sym == FcitxKey_comma && navigation_.value("comma_period", true)) ||
          (sym == FcitxKey_bracketleft && navigation_.value("brackets", false)))
        return command(MSIME_PREVIOUS_PAGE);
      if ((sym == FcitxKey_equal && !japanese && navigation_.value("minus_equal", true)) ||
          (sym == FcitxKey_period && navigation_.value("comma_period", true)) ||
          (sym == FcitxKey_bracketright && navigation_.value("brackets", false)))
        return command(MSIME_NEXT_PAGE);
    }
    // Japanese converts with Space and commits the kana with Enter. Sending the Engine's
    // raw-input command here commits the romaji, and committing the first candidate on Space
    // leaves no way to reach the second. See ../src/core/JapaneseConversion.h.
    if (japanese && composing &&
        (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter || sym == FcitxKey_space)) {
      using Action = msime::linux_host::JapaneseConversion::Action;
      const auto reading = view_.value("editing_text", std::string{});
      const auto &candidates = view_.at("candidates");
      if (sym == FcitxKey_space) {
        const int first_source = candidates.empty() ? -1 : candidates[0].value("source", -1);
        const auto action = japanese_conversion_.space(reading, candidates.size(), first_source);
        if (action == Action::Start) return true;
        if (action == Action::StepNext || action == Action::StepFirst)
          return command(action == Action::StepFirst ? MSIME_FIRST_CANDIDATE
                                                      : MSIME_NEXT_CANDIDATE);
      } else {
        const auto action = japanese_conversion_.enter(reading);
        const auto index = japanese_conversion_.index();
        japanese_conversion_.reset();
        if (action == Action::CommitCandidate && candidates.is_array() &&
            index < candidates.size()) {
          const auto &id = candidates[index].value("id", Json::object());
          if (id.is_object())
            return apply(msime_client_select(session_, id.value("generation", uint64_t{0}),
                                             id.value("index", size_t{0})));
        }
        if (action == Action::CommitReading && command(MSIME_COMMIT_READING)) return true;
      }
    }
    switch (sym) {
    case FcitxKey_Escape: return command(MSIME_CANCEL);
    case FcitxKey_BackSpace: return command(MSIME_BACKSPACE);
    case FcitxKey_Delete: case FcitxKey_KP_Delete: return command(MSIME_DELETE_FORWARD);
    // With a Korean Hanja list or a Zhuyin list open Return chooses the highlighted candidate, as Space does; only the session knows the highlight, so the command is the candidate one (msime_client.h).
    case FcitxKey_Return: case FcitxKey_KP_Enter:
      return command(openedList ? MSIME_COMMIT_CANDIDATE : MSIME_COMMIT_RAW);
    case FcitxKey_space: return command(MSIME_COMMIT_CANDIDATE);
    case FcitxKey_Left: case FcitxKey_KP_Left: return command(MSIME_MOVE_LEFT);
    case FcitxKey_Right: case FcitxKey_KP_Right: return command(MSIME_MOVE_RIGHT);
    case FcitxKey_Home: case FcitxKey_KP_Home:
      return command(MSIME_FIRST_CANDIDATE);
    case FcitxKey_End: case FcitxKey_KP_End:
      return command(MSIME_LAST_CANDIDATE);
    case FcitxKey_Tab: case FcitxKey_KP_Tab:
      // Fcitx normalises ISO_Left_Tab to Tab, keeping Shift only when it was held, so a back-tab sent without Shift is recognised by its raw symbol.
      if (navigation_.value("tab", true))
        return command(shift || event.rawKey().sym() == FcitxKey_ISO_Left_Tab ? MSIME_PREVIOUS_PAGE
                                                                              : MSIME_NEXT_PAGE);
      break;
    case FcitxKey_ISO_Left_Tab:
      if (navigation_.value("tab", true)) return command(MSIME_PREVIOUS_PAGE);
      break;
    case FcitxKey_Page_Up: case FcitxKey_KP_Page_Up:
      if (navigation_.value("page_up_down", true)) return command(MSIME_PREVIOUS_PAGE);
      break;
    case FcitxKey_Page_Down: case FcitxKey_KP_Page_Down:
      if (navigation_.value("page_up_down", true)) return command(MSIME_NEXT_PAGE);
      break;
    case FcitxKey_Up: case FcitxKey_KP_Up:
      if (navigation_.value("candidate_arrow_navigation", navigation_.value("arrows", true)))
        return command(MSIME_PREVIOUS_CANDIDATE);
      break;
    case FcitxKey_Down: case FcitxKey_KP_Down:
      if (navigation_.value("candidate_arrow_navigation", navigation_.value("arrows", true)))
        return command(MSIME_NEXT_CANDIDATE);
      break;
    default: break;
    }
    // Windows selects by virtual key, which does not depend on the layout: the number row picks a candidate on AZERTY too, where it types & é " unshifted. The XKB keycode (evdev + 8) is that physical key. In the modes whose spelling has digits (unicode, expression: the Engine lists them in spelling_symbols) candidates use Shift plus the row, as on Windows and in the IBus host, because the bare digits are input there; a shifted symbol the mode also spells with was sent to the Engine above.
    const bool spellingDigits = msime::linux_host::spelling_digits(view_);
    // Fcitx5 drops Shift from a normalised symbol such as '!', so ask the raw event.
    const bool rawShift = event.rawKey().states().test(fcitx::KeyState::Shift);
    const auto number = [&]() -> std::optional<size_t> {
      if (rawShift != spellingDigits) return std::nullopt;
      const auto code = event.rawKey().code();
      if (code >= 10 && code <= 19) return code == 19 ? size_t{9} : static_cast<size_t>(code - 10);
      if (spellingDigits) return std::nullopt;
      if (sym >= FcitxKey_1 && sym <= FcitxKey_9)
        return static_cast<size_t>(sym - FcitxKey_1);
      if (sym == FcitxKey_0 || sym == FcitxKey_KP_0) return size_t{9};
      if (sym >= FcitxKey_KP_1 && sym <= FcitxKey_KP_9)
        return static_cast<size_t>(sym - FcitxKey_KP_1);
      return std::nullopt;
    }();
    if (number &&
        !view_.value("nine_key", false) &&
        preferences_.value("number_row_selection", true)) {
      const size_t index = *number;
      if (index < view_.at("candidates").size()) {
        const auto id = view_.at("candidates").at(index).at("id");
        return apply(msime_client_select(session_, id.at("generation"), id.at("index")));
      }
      // A digit past the end of a Hanja or Zhuyin page picks nothing and is swallowed, as the runtime swallows it, rather than typed beside the open composition.
      return openedList;
    }
    // With number-row selection off a digit is not a candidate shortcut. Sent to the Engine it would still pick a candidate, since the runtime turns a digit the Engine leaves unhandled into a page selection, so it ends the composition instead, as a digit does with no Hanja list: the composition is written and the digit goes to the application after it.
    if (number && openedList) {
      command(MSIME_FINISH_COMPOSITION);
      return false;
    }
  }
  // Keypad marks: the decimal point always stays ASCII (Windows keeps numpad '.' for numbers), and while composing the arithmetic keys finish the spelling with their ASCII mark instead of the Chinese one. The same rule as the IBus host.
  const auto keypad = [&]() -> char {
    switch (sym) {
    case FcitxKey_KP_Decimal: return '.';
    case FcitxKey_KP_Separator: return ',';
    case FcitxKey_KP_Subtract: return '-';
    case FcitxKey_KP_Add: return '+';
    case FcitxKey_KP_Divide: return '/';
    case FcitxKey_KP_Multiply: return '*';
    case FcitxKey_KP_Equal: return '=';
    default: return 0;
    }
  }();
  if (keypad && (keypad == '.' || composingOrCandidates())) {
    if (apply(msime_client_punctuation_ascii(session_, static_cast<uint8_t>(keypad))))
      return true;
    if (keypad != '.') return false;
    commitText(fullwidthOutput() && !narrowScheme() ? std::string("．") : std::string("."));
    return true;
  }
  const auto text = fcitx::Key::keySymToUTF8(sym);
  if (text.size() == 1 && text[0] >= 0x20 && text[0] <= 0x7e) {
    if (text[0] == ';' && !shift && view_.value("microsoft_shuangpin", false)) {
      const auto editing = view_.value("editing_text", std::string{});
      const auto caret = std::min(editing.size(), view_.value("caret_position", editing.size()));
      const auto separator = caret ? editing.rfind('\'', caret - 1) : std::string::npos;
      const auto start = separator == std::string::npos ? 0 : separator + 1;
      if ((caret - start) % 2 == 1)
        return apply(msime_client_character(session_, ';', false));
    }
    const bool asciiPunctuation =
        std::ispunct(static_cast<unsigned char>(text[0])) != 0 &&
        // An apostrophe in an active spelling is an Engine input character for emoji/kaomoji and Japanese modes, matching the IBus router. In Korean, Zhuyin and Vietnamese it is a mark that follows the open composition like any other.
        !(text[0] == '\'' && composing && !commitsOnBlur());
    const bool japaneseLongVowel = view_.value("scheme", 0u) == 3 && !shift &&
                                   (text[0] == '-' || text[0] == '=');
    if (japaneseLongVowel)
      return apply(msime_client_character(session_, static_cast<uint8_t>(text[0]), false));
    if (asciiPunctuation) {
      if (repeatSmartPunctuationToChinese(text[0]))
        return true;
      // Korean and Vietnamese punctuation is plain ASCII and Zhuyin's comes from the Engine alone, so none of them is completed into a pair.
      if (!keypad && !withoutHostPunctuation()) {
        if (const auto paired = pairedPunctuation(text[0], composing)) return *paired;
      }
      return punctuation(static_cast<uint8_t>(text[0]));
    }
    // Shift decides the jamo (Shift+R is ㄲ, R alone ㄱ) and CapsLock does not, so a Korean letter goes out in the case Shift gives it. Fcitx5 strips Shift from the normalised key, so ask the raw event.
    if (korean() && std::isalpha(static_cast<unsigned char>(text[0])) != 0) {
      const bool rawShift = event.rawKey().states().test(fcitx::KeyState::Shift);
      const auto letter = static_cast<unsigned char>(text[0]);
      return apply(msime_client_character(
          session_, static_cast<uint8_t>(rawShift ? std::toupper(letter) : std::tolower(letter)), rawShift));
    }
    return apply(msime_client_character(session_, static_cast<uint8_t>(text[0]), key.states().test(fcitx::KeyState::Shift)));
  }
  if (composing) command(MSIME_FINISH_COMPOSITION);
  return false;
}

bool FcitxEngine::toolbarEnabled(fcitx::InputContext *ic) {
  if (!ic) return false;
  const auto *state = ic->propertyFor(&factory_);
  return state->session_ &&
         state->preferences_.value("floating_toolbar", Json::object())
             .value("enabled", true);
}

// The package entries of the 主题 menu follow the skin catalogue in the runtime options, which can change while the process runs; the menu is shared by every context, so it follows the context that last read the catalogue, as the toolbar menu does. Nothing is rebuilt while the packages and their titles stay the same, so an entry is never replaced under a menu that shows it.
void FcitxEngine::rebuildThemeMenu(fcitx::InputContext *ic) {
  if (!ic) return;
  const auto choices = ic->propertyFor(&factory_)->themeChoices();
  std::vector<std::pair<std::string, std::string>> packages;
  packages.reserve(choices.size());
  for (const auto &choice : choices)
    if (choice.package_base) packages.emplace_back(choice.id, choice.title);
  if (packages == global_theme_packages_) return;
  for (const auto &item : global_theme_package_items_) global_theme_menu_.removeAction(item.get());
  global_theme_package_items_.clear();
  global_theme_package_items_.reserve(packages.size());
  for (const auto &[id, title] : packages) {
    global_theme_package_items_.push_back(std::make_unique<FcitxGlobalThemeItemAction>(&factory_, id, title));
    // Registered under their own prefix, so a package can never take a global theme's name, and reachable from the D-Bus menus like every other entry.
    global_theme_package_items_.back()->registerAction("msime-global-theme-package-" + id,
                                                       &instance_->userInterfaceManager());
    global_theme_menu_.addAction(global_theme_package_items_.back().get());
  }
  global_theme_packages_ = std::move(packages);
  ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

// The scheme menu is shared by every context, so it follows the runtime options the last context read, as the theme menu does. The Chinese schemes come first and the other input languages after them, as in the IBus menu; nothing is rebuilt while the languages stay the same, so an entry is never replaced under a menu that shows it.
void FcitxEngine::rebuildSchemeMenu(fcitx::InputContext *ic, bool cantonese, bool zhuyin) {
  const std::pair languages{cantonese, zhuyin};
  if (scheme_menu_languages_ == languages) return;
  for (auto *entry : scheme_menu_entries_) scheme_menu_.removeAction(entry);
  scheme_menu_entries_ = {&scheme_quanpin_action_, &scheme_shuangpin_action_, &scheme_wubi_action_};
  if (cantonese) scheme_menu_entries_.push_back(&scheme_cantonese_action_);
  if (zhuyin) scheme_menu_entries_.push_back(&scheme_zhuyin_action_);
  scheme_menu_entries_.insert(scheme_menu_entries_.end(),
                              {&scheme_japanese_action_, &scheme_korean_action_, &scheme_vietnamese_action_});
  for (auto *entry : scheme_menu_entries_) scheme_menu_.addAction(entry);
  scheme_menu_languages_ = languages;
  if (ic) ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

void FcitxEngine::rebuildToolbarMenu(fcitx::InputContext *ic) {
  for (auto *action : toolbar_entries_)
    toolbar_menu_.removeAction(action);
  toolbar_entries_.clear();
  if (!ic) return;
  const auto *state = ic->propertyFor(&factory_);
  if (!state->session_) return;
  const auto toolbar = state->preferences_.value("floating_toolbar", Json::object());
  if (!toolbar.value("enabled", true)) return;
  // The mode entry is always present, as it is on Windows and on the IBus host;
  // the rest follow their own switch. The defaults match those two hosts, which is
  // why the screen keyboard is the one that starts hidden.
  const auto append = [&](bool present, fcitx::Action *action) {
    if (!present) return;
    toolbar_menu_.addAction(action);
    toolbar_entries_.push_back(action);
  };
  append(true, &input_mode_action_);
  append(toolbar.value("english_mode", true), &english_action_);
  append(toolbar.value("fullwidth", true), &width_action_);
  append(toolbar.value("punctuation", true), &chinese_punctuation_action_);
  append(toolbar.value("character_set", true), &traditional_action_);
  append(toolbar.value("emoji", true), &desktop_emoji_action_);
  append(toolbar.value("screen_keyboard", false), &keyboard_action_);
  append(toolbar.value("settings", true), &settings_action_);
}

class FcitxFactory : public fcitx::AddonFactory {
public:
  fcitx::AddonInstance *create(fcitx::AddonManager *manager) override { return new FcitxEngine(manager->instance()); }
};
} // namespace msime::fcitx_host

FCITX_ADDON_FACTORY(msime::fcitx_host::FcitxFactory)
