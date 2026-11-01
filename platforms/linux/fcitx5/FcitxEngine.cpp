#include "msime_client.h"
#include "../../common/HostApiString.h"
#include "../../common/HostApiString.h"
#ifdef MSIME_FCITX5_TELEMETRY
#include "Telemetry.h"
#endif
#include "../src/system/ChineseTextConversion.h"
#include "../src/system/CapsLockRelease.h"
#include "../../../shared/input/CapsLockTap.h"
#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/utf8.h>
#include <fcitx-utils/event.h>
#include <fcitx-utils/misc.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/addoninstance.h>
#include <fcitx-config/configuration.h>
#include <fcitx-config/iniparser.h>
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
#include "../src/candidates/PersonalWordbook.h"
#include "../src/candidates/ReadAloud.h"
#include "../src/candidates/CandidateLocalModeLabels.h"
#include "../src/candidates/CandidatePanelStatus.h"
#include "../src/candidates/CandidateScriptAnnotation.h"
#include "../src/candidates/CandidateRowText.h"
#include "../src/candidates/CandidatePalette.h"
#include "../src/candidates/CandidateColors.h"
#include "../src/candidates/CandidateFcitxTheme.h"
#include "../src/candidates/FcitxThemeLogo.h"
#include "../src/candidates/FcitxThemeOverlayScale.h"
#include "../src/candidates/FcitxCanvasPanel.h"
#ifdef MSIME_FCITX5_NATIVE_CANVAS
#include "../src/candidates/NativeCanvasX11.h"
#include "../src/candidates/NativeCanvasSnapshot.h"
#include "../src/candidates/ReduceMotion.h"
#include "../../../shared/candidate/CandidateExpandedGrid.h"
#endif
#include "../src/candidates/CandidateFontPolicy.h"
#include "../src/candidates/CandidateWheelPaging.h"
#include "../src/candidates/PanelRestoreRecord.h"
#include "../src/candidates/ShuangpinProfileNames.h"
#include "../src/candidates/WubiProfileNames.h"
#include "../src/candidates/CandidatePronunciation.h"
#include "../src/candidates/CandidateTranslationPolicy.h"
#include "../src/candidates/CandidateTranslationScope.h"
#include "../src/candidates/PairedPunctuation.h"
#include "../src/core/CandidateSkinCatalog.h"
#include "../src/core/GlobalTheme.h"
#include "../src/core/ToggleHotkeyChord.h"
#include "../src/core/VocabularyDue.h"
#include "../src/core/DictionaryQuiesceLease.h"
#include "../src/core/EmojiPluginGroups.h"
#include "../src/core/RuntimeOptionsRefresh.h"
#include "../src/core/RuntimeOptionsFile.h"
#include "../src/core/FirstRunGuidance.h"
#include "../src/core/InputModeIndicator.h"
#include "../src/core/InputStatus.h"
#include "../src/core/MenuLabels.h"
#include "../src/core/ReplacedProgram.h"
#include "../src/overlay/ModeBadgePlacement.h"
#ifdef MSIME_FCITX5_MODE_BADGE
#include "../src/overlay/ModeBadgeSurface.h"
#endif
#include "../src/overlay/FloatingPanelColors.h"
#include "../src/overlay/FloatingToolbarModel.h"
#include "../src/overlay/ShuangpinKeymapModel.h"
#ifdef MSIME_LINUX_FLOATING_PANELS
#include "../src/overlay/FloatingToolbar.h"
#include "../src/overlay/ShuangpinKeymapX11Surface.h"
#endif
#include "../src/core/BackspaceHoldPolicy.h"
#include "../src/core/SmartPunctuationSpace.h"
#include "../src/core/SmartPunctuationChoice.h"
#include "../src/core/SpellingSymbols.h"
#include "../src/core/LocalModeSwitches.h"
#include "../src/system/KeySound.h"
#include "../src/system/LifecycleFeedback.h"
#include "../src/system/DiagnosticLog.h"
#include "../src/system/PanelInputChannel.h"
#include "../src/core/HelpcodeDefaults.h"
#include "../src/core/HelpcodePack.h"
#include "../src/core/HelpcodeSchemaNames.h"
#include "../src/core/PhrasePreedit.h"
#include "../src/core/CaretEditSuffix.h"
#include "../src/core/ClientInputModeMemory.h"
#include "../src/core/AppInputModeRules.h"
#include "../src/core/JapaneseConversion.h"
#include "../src/core/KoreanHanja.h"
#include "../src/core/InputSchemes.h"
#include "../src/core/JsonInteger.h"
#include "../src/clipboard/CloudClipboardRoute.h"
#include "../src/system/TypingStatistics.h"
#include "SystemTheme.h"
#include "PrecedingCharacters.h"
#include "../src/voice/VoiceAction.h"
#include "../src/voice/VoiceHotwords.h"
#include "../src/voice/VoiceProviderOptions.h"
#include "../src/overlay/WaveOverlayFeedback.h"
#include "../src/overlay/WaveOverlayModel.h"
#include "../src/overlay/WaveOverlaySurface.h"
#ifdef MSIME_LINUX_HAS_X11_OVERLAY
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
#include <condition_variable>
#include <cmath>
#include <spawn.h>
#include <unistd.h>
#include <string_view>
#include <utility>
#include <vector>
#include <cstring>
#include <cctype>
#include <atomic>
#include <mutex>
#include <thread>
#include <tuple>
#include <ctime>
#if __has_include(<fcitx/candidateaction.h>)
#include <fcitx/candidateaction.h>
#define MSIME_FCITX_ACTIONS 1
#endif

#include "../src/core/LinuxEdition.h"

#ifndef MSIME_SYSTEM_OPTIONS
#define MSIME_SYSTEM_OPTIONS "/etc/" MSIME_EDITION_CLIENT_DIRECTORY "/runtime-options.json"
#endif
#ifndef MSIME_BINDIR
#define MSIME_BINDIR "/usr/bin"
#endif
#ifndef MSIME_SOUND_PACKS
#define MSIME_SOUND_PACKS "/usr/share/" MSIME_EDITION_CLIENT_DIRECTORY "/sound-packs"
#endif

extern char **environ;

#ifdef MSIME_FCITX5_HINT_FONT
#include <pango/pangocairo.h>
#endif

namespace msime::fcitx_host {
using Json = nlohmann::json;
class FcitxEngine;
class FcitxMaintenanceAction;

// Context teardown drops futures without waiting so the Fcitx5 loop never blocks on a provider.
// The addon still has to keep every worker inside the loaded module until unload, so the addon
// waits for this tracker in ~FcitxEngine before Fcitx5 can dlclose the plugin.
class DetachedJobTracker {
public:
  template <class F> std::shared_future<Json> start(F work) {
    {
      std::lock_guard lock(mutex_);
      ++active_;
    }
    try {
      std::packaged_task<Json()> task(
          [this, work = std::move(work)]() mutable -> Json {
            try {
              auto result = work();
              finish();
              return result;
            } catch (...) {
              finish();
              throw;
            }
          });
      auto future = task.get_future().share();
      std::thread(std::move(task)).detach();
      return future;
    } catch (...) {
      finish();
      throw;
    }
  }

  void wait_idle() {
    std::unique_lock lock(mutex_);
    idle_.wait(lock, [this] { return active_ == 0; });
  }

private:
  void finish() {
    std::lock_guard lock(mutex_);
    if (--active_ == 0)
      idle_.notify_all();
  }

  std::mutex mutex_;
  std::condition_variable idle_;
  size_t active_ = 0;
};

DetachedJobTracker fcitx_detached_jobs;

template <class F> std::shared_future<Json> detachedJob(F work) {
  return fcitx_detached_jobs.start(std::move(work));
}

struct PendingPreferenceSave {
  std::string directory;
  std::string section;
  std::string key;
  Json value;
  // The value is a 主题 menu change (theme_choice_change): it writes global_theme and custom_theme together rather than one key.
  bool theme_choice = false;
  // The value is a 辅助码方案 status-bar choice for `section` (quanpin_helpcode or shuangpin_helpcode): besides `section.schema` it clears that scheme's helpcode pack, as the settings page does, or the pack would keep overriding the schema just chosen (core/HelpcodePack.h).
  bool helpcode_schema_choice = false;
  // The value is the 智能标点 menu toggle: smart_punctuation is written together with smart_punctuation_direct_digit and smart_punctuation_direct_letter in one save (core/SmartPunctuationChoice.h), not as three jobs that could each land on a different revision.
  bool smart_punctuation_choice = false;
};

// ABI buffers and errors never escape into diagnostics or the panel.
// 整份回复，失败时也原样返回，供需要分辨失败原因的调用方使用（见 ensure 里的建会话）。
Json hostReply(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) throw std::runtime_error("MSIME request failed");
  return Json::parse(owned.get());
}

Json response(char *raw) {
  auto value = hostReply(raw);
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
  else if (request.helpcode_schema_choice)
    msime::linux_host::apply_helpcode_schema_choice(
        snapshot["preferences"], request.section == "shuangpin_helpcode" ? "shuangpin" : "quanpin",
        request.value.get<std::string>());
  else if (request.smart_punctuation_choice)
    msime::linux_host::apply_smart_punctuation_choice(snapshot["preferences"], request.value.get<bool>());
  else if (request.section.empty()) snapshot["preferences"][request.key] = request.value;
  else snapshot["preferences"][request.section][request.key] = request.value;
  const auto encoded = snapshot.dump();
  return response(msime_client_save_preferences(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size(),
      msime::linux_host::strict_json_required_integer<uint64_t>(snapshot.at("revision")),
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
  std::atomic_bool cancelled{false};
  std::string partial;
  std::string final;
  uint8_t phase = 0;
  bool phase_seen = false;
  uint8_t level = 0;
  bool level_seen = false;
  bool final_ready = false;
};

extern "C" bool fcitxVoiceCancelled(void *context) noexcept {
  if (!context) return true;
  auto *mailbox = static_cast<FcitxVoiceMailbox *>(context);
  return mailbox->cancelled.load(std::memory_order_relaxed);
}

std::unique_ptr<msime::linux_host::WaveOverlaySurface>
create_fcitx_wave_overlay_surface(
    [[maybe_unused]] msime::linux_host::WaveOverlaySurface::ActionHandler handler) {
  const auto *requested = std::getenv("MSIME_WAVE_OVERLAY_BACKEND");
  [[maybe_unused]] const bool force_auxiliary = requested &&
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
#ifdef MSIME_LINUX_HAS_X11_OVERLAY
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
  return Json::parse(msime::linux_host::read_runtime_options(optionsPath()));
}

// 「我的词条」的网盘自动同步在 Rust 的后台线程里跑（msime_client.h 的 msime_client_dictionary_sync_start），这里只交出运行时配置文件和与设置应用共用的状态目录：设置应用用的是共享状态目录（runtime options 的 preferences_directory）下的 dictionary-sync（apps/desktop/src-tauri/src/platform/dictionary_sync.rs）。后台线程每一轮重读配置和偏好，没配置网盘、设置应用还没生成设备身份时什么也不做，从不自己生成身份。插件加载时还没做首次配置的，等第一个会话读到配置时再起。起它只是开一个线程，不在主循环上读写文件或联网。插件被卸载后库可能还留着，所以停下时把旗子放回去。
bool fcitx_dictionary_sync_started = false;

void startDictionarySync(const Json &options) {
  if (fcitx_dictionary_sync_started) return;
  try {
    const auto directory = options.value("preferences_directory", std::string{});
    if (directory.empty() || directory.front() != '/') return;
    const auto request = Json{{"options_path", optionsPath().string()},
                              {"directory", (std::filesystem::path(directory) / "dictionary-sync").string()}}
                             .dump();
    response(msime_client_dictionary_sync_start(reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    fcitx_dictionary_sync_started = true;
  } catch (...) {
  }
}

// 插件的代码在 Fcitx5 卸载它之后不能再运行：与 fcitx_detached_jobs 一样等后台线程真正退出（正在跑的那一次同步受锁的 5 秒等待和每个请求的超时约束）。
void stopDictionarySync() {
  if (!fcitx_dictionary_sync_started) return;
  (void)msime_client_dictionary_sync_stop(UINT32_MAX);
  fcitx_dictionary_sync_started = false;
}

using CandidateSkinCatalog = std::vector<msime::linux_host::CandidateSkin>;

// Fcitx5 gives each input context its own property, so keep the mode memory
// at addon scope just as the IBus host keeps it at engine scope.  The shared
// policy bounds this table and provides an anonymous fallback.
msime::linux_host::ClientInputModeMemory fcitx_app_input_modes;
std::optional<bool> fcitx_global_input_mode;
// 应用例外的停留也在插件范围：同一个程序的几个输入上下文属于同一次停留（AppInputModeRules.h）。
msime::linux_host::AppInputModeRuleStay fcitx_app_rule_stay;
// 焦点进入 `program` 的上下文。Fcitx5 的上下文建起来时就带着前端报告的程序名，没有 IBus 那种先匿名、稍后补名的协商，所以程序名为空的上下文（不报告程序名的前端，例如部分 Wayland text-input 客户端）获得焦点就是换了客户端，和 macOS、Windows 切到别的程序一样结束这次停留。
void enterAppRuleStay(const std::string &program) {
  if (program.empty())
    fcitx_app_rule_stay.leave();
  else
    fcitx_app_rule_stay.enter(program);
}
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

// 语音浮层的主题：明暗跟随颜色模式（各界面一律跟随，存下的 voice_theme 不再读），按 VoiceAction.h 一直用的规则解析；颜色取解析出的主题，与悬浮工具栏取法相同，所以 MSIME 画的浮层与候选窗是同一套主题而不是固定的灰色。固定明暗的主题照样盖过它，与候选窗相同。
msime::linux_host::CandidateTheme resolveVoiceOverlayTheme(const Json &preferences, bool system_dark,
                                                           const Json &catalog) {
  const bool dark = !msime_voice_overlay_light_theme("follow", preferences.value("theme", "dark"), system_dark);
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
    const auto candidate = std::filesystem::path(runtime) / MSIME_EDITION_CLIENT_DIRECTORY / filename;
    std::error_code error;
    if (std::filesystem::is_socket(candidate, error)) return candidate.string();
  }
  return {};
}

std::string onlineSocket(const Json &options) {
  return providerSocket(options, "online_provider_socket",
                        "MSIME_ONLINE_PROVIDER_SOCKET", "online.sock");
}

std::string explicitCloudClipboardSocket(const Json &options) {
  auto value = options.value("cloud_clipboard_provider_socket", std::string{});
  if (value.empty()) {
    if (const auto *env = std::getenv("MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET")) value = env;
  }
  return value;
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

bool emojiGroupsResultCurrent(const Json &result, uint64_t generation,
                              const std::string &category) {
  return result.is_object() &&
         msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == generation &&
         result.value("_category", std::string{}) == category;
}

bool launchDesktopPanel(const char *panel) {
  if (!panel || !*panel) return false;
  const char *command = std::getenv("MSIME_CLIENT_SETTINGS_COMMAND");
  if (!command || !*command) command = MSIME_EDITION_SETTINGS_PROGRAM;
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

// `mixed_input` 里混输开关缺键时的值：符号混输默认开，共享文档开着时不写这个键；emoji、颜文字默认关。读值时回退必须按键给，否则缺键的符号混输会显示成关，第一次点击反而把它「打开」成关。
bool mixedCandidateDefault(std::string_view key) { return key == "symbols"; }

class FcitxState : public fcitx::InputContextProperty {
public:
  // system_dark is the engine's last probed appearance, so a context opened between probes starts in it rather than in light until the next change.
  FcitxState(fcitx::InputContext &ic, FcitxEngine *engine, fcitx::EventLoop &loop, bool system_dark)
      : ic_(ic), engine_(engine), loop_(&loop), system_dark_(system_dark) {
    wave_overlay_surface_ = create_fcitx_wave_overlay_surface(
        [this](msime::linux_host::WaveOverlayModel::Action action) { handleVoiceOverlayAction(action); });
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
  void cancelModifierTaps() {
    // 旧手势的松开可能落在别的窗口；清除按住状态，但迟到的松开仍算取消的手势。
    shift_in_combination_ = shift_in_combination_ || shift_down_;
    ctrl_in_combination_ = ctrl_in_combination_ || ctrl_down_;
    shift_down_ = false;
    ctrl_down_ = false;
    pure_shift_candidate_ = false;
    pure_ctrl_candidate_ = false;
    modifier_toggle_deadline_ = {};
  }
  // 离开输入框时不论会话关不关都要做的：结束修饰键手势，写出按键统计，收起语音浮层，停掉音乐，连击与按键重复从头算（连击属于会话，下一个从零开始）。
  void releaseFocusResources() {
    cancelModifierTaps();
    candidate_theme_refresh_at_.reset();
    flushKeyPresses(key_presses_.take());
    key_presses_.forget_held();
    hideVoiceOverlay();
    wave_overlay_.reset();
    music_.release(session_, msime_client_set_active);
    typing_combo_ = 0;
    key_repeat_.reset();
  }
  ~FcitxState() override { close(); }
  void close() {
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    if (native_canvas_) native_canvas_->hide();
    discardExpanded();
#endif
    // Every way out of a session passes here, focus loss, deactivation and teardown included, and options_path_ is still the batch's directory.
    releaseFocusResources();
    if (session_) msime_linux_diagnostic_write("focus_out");
    if (session_) msime::host_api::discard_string(msime_client_destroy(session_));
    session_ = 0;
    field_traits_.reset();
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
    forgetSmartPunctuationShadow();
    paired_tracker_.clear();
    pending_caret_.clear();
    caret_forward_event_.reset();
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
    invalidateOnlineRequests();
    online_due_ = {};
    ai_due_ = {};
    invalidateTranslationRequests();
    translation_socket_.clear();
    clipboard_path_.clear();
    ++clipboard_generation_;
    clipboard_items_.clear();
    clipboard_loading_ = false;
    clipboard_job_ = {};
    clipboard_mutation_job_ = {};
    cloud_clipboard_socket_.clear();
    cloud_clipboard_explicit_socket_.clear();
    cloud_clipboard_directory_.clear();
    cloud_clipboard_relay_ = Json::object();
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
      if (voice_mailbox_) voice_mailbox_->cancelled.store(true, std::memory_order_relaxed);
      const auto socket = voice_socket_;
      const auto generation = voice_generation_;
      msime::host_api::discard_string(msime_client_voice_provider_cancel(
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
    voice_toggle_held_ = 0;
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
  // 失焦后会话留给托盘菜单时用它代替 close()：托盘读到的勾选和点下的开关都落在这个会话上，到下一次有上下文获得焦点才真正关掉（见 FcitxEngine::holdMenuContext）。除了 releaseFocusResources，还要停掉录音、按失焦结束组字和候选，与 IBus 宿主失焦时一样只取视图：要上屏的组字已由 focus_watch_ 或 Fcitx5 自己提交，留着的话之后的菜单动作会把它当成正在输入的文字再提交一次。
  void leaveFocus() {
    if (!session_) return;
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    // 失焦收起展开的候选（网格或卷轴），候选窗立即撤下。
    discardExpanded();
    if (native_canvas_) native_canvas_->hide();
#endif
    cancelVoice();
    releaseFocusResources();
    if (composingOrCandidates()) {
      auto result = response(focusSession(false));
      view_ = result.contains("view") ? result.at("view") : result;
    }
  }
  // 菜单动作能否作用到这个上下文：有焦点，或者它是失焦后托盘仍然指向的那一个。FcitxEngine 在这里只是前向声明，定义在它之后。
  bool menuTarget() const;
  void clearPanel() {
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    discardExpanded();
    if (native_canvas_) native_canvas_->hide();
#endif
    hideShuangpinKeymap();
    ic_.inputPanel().reset();
    ic_.updatePreedit();
    ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    refreshModeIndicator();
  }
  // 方案在菜单里的名字（MenuLabels.h）：双拼带当前键位方案，五笔带码表版本，与 macOS 一致。
  std::string schemeMenuTitle(std::string_view scheme) const {
    return msime::linux_host::scheme_menu_title(scheme, preferences_.value("shuangpin_profile", std::string("xiaohe")),
                                                preferences_.value("wubi_profile", std::string("wubi86")));
  }
  // The label Fcitx5 shows for the input method in its tray and panel (FcitxEngine::subModeLabelImpl), in the same words as the mode HUD.
  std::string modeIndicatorLabel() const {
    switch (msime::linux_host::input_mode_indicator(input_enabled_, effectiveScheme(), caps_lock_)) {
    case msime::linux_host::InputModeIndicator::Chinese: return "中";
    case msime::linux_host::InputModeIndicator::Shuangpin: return "双";
    case msime::linux_host::InputModeIndicator::Wubi: return "五";
    case msime::linux_host::InputModeIndicator::Japanese: return "日";
    case msime::linux_host::InputModeIndicator::Korean: return "한";
    case msime::linux_host::InputModeIndicator::Cantonese: return "粤";
    case msime::linux_host::InputModeIndicator::Zhuyin: return "注";
    case msime::linux_host::InputModeIndicator::Stroke: return "笔";
    case msime::linux_host::InputModeIndicator::Vietnamese: return "越";
    case msime::linux_host::InputModeIndicator::Tibetan: return "藏";
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
    // 模式可能变化的地方都会走到这里，悬浮工具栏跟着刷新（只有属主的状态会画出来）。
    syncFloatingToolbar();
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
  // 把大写锁定报告给会话（msime_client_set_caps_lock），「大写锁定时使用英文标点」才在这个宿主上生效。会话不从别处得知它、新会话从关开始，所以每个键分发之前比较会话上次收到的值，变了才报告；打开新会话的那个键也在分发之前报告。
  void syncSessionCapsLock() {
    if (!session_ || session_caps_lock_ == caps_lock_) return;
    response(msime_client_set_caps_lock(session_, caps_lock_));
    session_caps_lock_ = caps_lock_;
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
  // 把输入框的属性报告给生命周期核心，变了才调用。逐个标志映射，不复用 privateInput() 的布尔值：Sensitive 按最严处理成 SECURE 加 PRIVATE，NoSpellCheck 只报 NO_SPELLCHECK，这样只带 NoSpellCheck 的普通字段照常出声（LifecycleFeedback.h）。restricted() 的字段根本不建会话。
  void syncFieldTraits() {
    field_traits_.sync(session_, fieldTraits(), msime_client_set_field_traits);
  }
  std::uint32_t fieldTraits() const {
    const auto flags = ic_.capabilityFlags();
    return msime::linux_host::lifecycle_field_traits(
        flags.test(fcitx::CapabilityFlag::Sensitive),
        flags.test(fcitx::CapabilityFlag::NoSpellCheck));
  }
  // 本插件对 msime_client_focus 的唯一调用。获得焦点（true）之前不论属性变没变都先报告一次（FieldTraitsReport::confirm）：切换方案、双拼方案或辅助码时 ensure() 刚为新会话报告过、也刚 focus(true) 过，syncFieldTraits 的去重会省掉紧接着这次 focus(true) 之前的报告，而脚本运行时要在每次获得焦点之后先看到属性确认才往脚本投递事件。
  char *focusSession(bool focused) {
    if (focused) field_traits_.confirm(session_, fieldTraits(), msime_client_set_field_traits);
    return msime_client_focus(session_, focused);
  }
  // 指针点选和翻页同样先开启新的一轮（msime_client.h 的顺序契约）：MSIME_KEY_POINTER 不出声、不画。
  void pointerTurn() {
    if (!session_) return;
    syncFieldTraits();
    msime_client_key(session_, MSIME_KEY_POINTER);
  }
  bool toggleEnglish() {
    if (!session_) return false;
    const bool enabled = !view_.value("dedicated_english", false);
    view_ = response(msime_client_set_english_mode(session_, enabled));
    render();
    return true;
  }
  // The schemes in the order of the view's scheme index, which is also the order the status action steps through them.
  static constexpr std::array<const char *, 10> kSchemes = {"quanpin", "shuangpin", "wubi", "japanese", "korean",
                                                            "cantonese", "zhuyin", "vietnamese", "tibetan",
                                                            "stroke"};
  static_assert(kSchemes.size() == msime::linux_host::kInputSchemeIds.size());
  // Whether a scheme can run with the runtime options this context last read: Cantonese, Zhuyin and Stroke need their language dictionary (core/InputSchemes.h).
  bool schemeAvailable(const char *id) const {
    return msime::linux_host::input_scheme_available(id, scheme_dictionaries_);
  }
  // The scheme the Engine runs for the preferences, after host-api's fallback from a scheme whose data is missing; the indicator and the status file show this one.
  std::string effectiveScheme() const {
    return msime::linux_host::effective_input_scheme(
        scheme_override_.value_or(preferences_.value("scheme", std::string("quanpin"))),
        preferences_.value("last_chinese_scheme", std::string("quanpin")), scheme_dictionaries_);
  }
  // Works out from the runtime options just read which language dictionaries are installed, once per read rather than per key, and lists Cantonese, Zhuyin and Stroke in the scheme menu only while theirs is.
  void noteSchemeOptions(const Json &options) {
    scheme_dictionaries_ = msime::linux_host::language_dictionary_availability(options);
    refreshSchemeMenu();
  }
  void refreshSchemeMenu();
  bool cycleScheme() {
    if (!session_ || restricted() || privateInput()) return false;
    const auto current = msime::linux_host::strict_json_value(view_, "scheme", 0u);
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
    // 共享设置页和 IBus 宿主都提供「中文」入口，回到用户最后用来打中文的方案。这里如果不记录它，从状态区切到日文、韩文、越南文或藏文后，这个选择就只剩全拼这个兜底可回。
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
    view_ = response(focusSession(true)).at("view");
    render();
    return true;
  }
  // Ctrl+Shift+J：当前跑的是中文方案就切到日语，是日语等外语就回到 last_chinese_scheme（core/InputSchemes.h 的 japanese_toggle_target）。换方案走状态区菜单的 selectScheme，组字中先上屏原文。selectScheme 只在目标是中文方案时记 last_chinese_scheme，所以离开中文时这里先记下当前方案，回来时才回到它。
  bool toggleJapanese() {
    if (!session_ || restricted() || privateInput()) return false;
    const auto running = effectiveScheme();
    const auto next = msime::linux_host::japanese_toggle_target(
        running, preferences_.value("last_chinese_scheme", std::string("quanpin")), scheme_dictionaries_);
    if (next.empty()) return false;
    if (msime::linux_host::scheme::IsChinese(msime::linux_host::scheme_number(running))) {
      saveStringPreference("last_chinese_scheme", running);
      // 存储读回之前紧接着再按一次，也要回到这个方案。
      preferences_["last_chinese_scheme"] = running;
    }
    return selectScheme(next.c_str());
  }
  bool cycleShuangpinProfile() {
    if (!session_ || msime::linux_host::strict_json_value(view_, "scheme", 0u) != 1 || restricted() || privateInput())
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
    view_ = response(focusSession(true)).at("view");
    render();
    return true;
  }
  bool cycleHelpcodeSchema() {
    const auto scheme = msime::linux_host::strict_json_value(view_, "scheme", 0u);
    if (!session_ || (scheme != 0 && scheme != 1) || restricted() || privateInput())
      return false;
    // The size follows the list rather than being written twice: jiajia was added as the sixth
    // schema and the count stayed at five, which stopped this addon compiling at all.
    static constexpr std::array schemas = {"lantian", "ziranma", "shouyou2_0", "shouyouplus",
                                           "xiaohe",  "jiajia",  "wubi86"};
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    const auto current = preferences_.value(section, Json::object()).value(
        "schema", scheme == 1 ? std::string("lantian") : std::string("ziranma"));
    const auto it = std::find(schemas.begin(), schemas.end(), current);
    // 辅助码表包生效时，第一次点按先回到存着的那个内置方案（停用插件），之后再按顺序轮换。存着的不是已知方案时回到第一个方案，不把无效值写回去。
    const auto next = !msime::linux_host::helpcode_pack(preferences_, scheme == 1 ? "shuangpin" : "quanpin").empty()
        ? (it == schemas.end() ? std::string(schemas.front()) : current)
        : it == schemas.end() || std::next(it) == schemas.end() ? std::string(schemas.front()) : std::string(*std::next(it));
    return selectHelpcodeSchema(next);
  }
  // The 辅助码方案 menu's choice. Choosing a built-in schema also turns this scheme's helpcode pack off, as the settings page does, so choosing the schema the store already holds still saves while a pack is active.
  bool selectHelpcodeSchema(const std::string &schema) {
    const auto scheme = msime::linux_host::strict_json_value(view_, "scheme", 0u);
    if (!session_ || (scheme != 0 && scheme != 1) || restricted() || privateInput())
      return false;
    if (std::none_of(msime::linux_host::kHelpcodeSchemaNames.begin(), msime::linux_host::kHelpcodeSchemaNames.end(),
                     [&](const auto &name) { return schema == name.value; }))
      return false;
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    if (!view_.value("editing_text", std::string{}).empty())
      command(MSIME_FINISH_COMPOSITION);
    saveHelpcodeSchemaChoice(section, schema);
    waitForPreferenceSave();
    helpcode_schema_override_ = schema;
    helpcode_schema_unsaved_ = unsavedChoice(section, "schema");
    close();
    if (!ensure()) return false;
    view_ = response(focusSession(true)).at("view");
    render();
    return true;
  }
  bool toggleNineKey() {
    if (!session_ || msime::linux_host::strict_json_value(view_, "scheme", 0u) != 0) return false;
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
    if (!session_ || size < 1 || size > 10 || restricted() || privateInput()) return false;
    if (msime::linux_host::strict_json_value(view_, "page_size", size_t{}) == size) return true;
    view_ = response(msime_client_set_candidate_page_size(session_, size)).at("view");
    preferences_["candidate_page_size"] = size;
    if (preferences_snapshot_.is_object() && preferences_snapshot_.contains("preferences"))
      preferences_snapshot_["preferences"]["candidate_page_size"] = size;
    saveNumberPreference("candidate_page_size", size);
    render();
    return true;
  }
  bool chooseNineKeySpelling(size_t index) {
    if (!session_ || msime::linux_host::strict_json_value(view_, "scheme", 0u) != 0 ||
        !view_.value("nine_key", false) || restricted() || privateInput()) return false;
    const auto spellings = view_.value("nine_key_spellings", Json::array());
    if (!spellings.is_array() || index >= spellings.size() || !spellings.at(index).is_string())
      return false;
    view_ = response(msime_client_choose_nine_key_spelling(
        session_, msime::linux_host::strict_json_value(view_, "generation", uint64_t{}), index)).at("view");
    render();
    return true;
  }
  bool toggleHelpcode() {
    if (!session_ || (msime::linux_host::strict_json_value(view_, "scheme", 0u) != 0 && msime::linux_host::strict_json_value(view_, "scheme", 0u) != 1))
      return false;
    const std::string section = msime::linux_host::strict_json_value(view_, "scheme", 0u) == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
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
    if (!session_ || msime::linux_host::strict_json_value(view_, "scheme", 0u) != 0 || !key || !*key) return false;
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
    const bool enabled = !preferences_.value("mixed_input", Json::object()).value(key, mixedCandidateDefault(key));
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
    static constexpr std::array<std::string_view, 12> allowed = {
        "unicode", "date_time", "quick_phrase", "emoji", "kaomoji",
        "super_jianpin", "temporary_english", "temporary_japanese",
        "chaizi", "expression", "command", "mention"};
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
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
    saveBooleanPreference("candidate_english_gloss", enabled);
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
    // 智能标点总开关连同数字后、字母后直出一起写，与设置页相同；否则设置页关过一次之后，这里打开的只是一个不起作用的总开关。
    const bool smart_choice = std::string_view(key) == "smart_punctuation";
    if (smart_choice) msime::linux_host::apply_smart_punctuation_choice(snapshot["preferences"], enabled);
    else snapshot["preferences"][key] = enabled;
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    // 智能标点两项由宿主自己处理，按键读的是宿主副本；快照已在本地改好，下一拍读到的存储与它相同、不会重算，所以这里直接跟上，否则托盘关掉智能标点后本会话仍照旧替换。
    smart_punctuation_ = preferences_.value("smart_punctuation", true);
    smart_punctuation_repeat_ = preferences_.value("smart_punctuation_repeat", true);
    preferences_snapshot_ = std::move(snapshot);
    if (smart_choice) saveSmartPunctuationChoice(enabled);
    else saveBooleanPreference(key, enabled);
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
  void retryCandidatePanelSettings();
  // `chosen` 只标记用户刚在菜单或设置页主动选主题；重读相同偏好不能夺回第三方 classicui 主题。
  void syncCandidatePanelTheme(bool chosen);
  bool candidateThemeSelectionChanged(const Json &preferences);
  void syncVoiceAction();
  // 中英文切换后在光标附近短暂显示「中」或「英」：能放到光标旁时是自绘徽章，否则是 Fcitx5 面板的文字提示（见 ModeBadgePlacement.h 的 mode_badge_hint）；定义在 FcitxEngine 之后，它需要那个类型完整。
  // `trigger` 区分真正的切换和获得焦点（#2589）：获得焦点时输入框要过一会儿才报光标矩形，光标徽章先等它（见 settleFocusInputModeHud），FocusSettled 是等完之后的那一次。
  enum class ModeHudTrigger { Switch, FocusEntered, FocusSettled };
  void showInputModeHud(ModeHudTrigger trigger = ModeHudTrigger::Switch);
  // 悬浮工具栏和双拼键位图（定义在 FcitxEngine 之后，它们用 FcitxEngine 的成员）。syncFloatingToolbar 把开关、外观和这个上下文的状态交给整个插件共用的工具栏，focus_entered 时这个上下文接手成为属主；没有 X11 绘制的构建里都是空操作。
  void syncFloatingToolbar(bool focus_entered = false);
  msime::linux_host::FloatingToolbarState floatingToolbarState() const;
  void renderShuangpinKeymap();
  void hideShuangpinKeymap();
#ifdef MSIME_FCITX5_MODE_BADGE
  // 获得焦点后的光标徽章在等光标矩形：输入框报来新矩形（FcitxEngine 的 mode_hud_cursor_watch_）或等满 ModeBadgeTiming::focus_caret_wait_ms 时调用，只放一次。GTK、Qt 的输入法模块在获得焦点之后才在空闲回调里补报光标，新建的上下文这时的矩形还是全 0，立刻放会把徽章放到屏幕下方。
  void settleFocusInputModeHud() {
    if (!mode_hud_after_focus_) return;
    mode_hud_after_focus_ = false;
    if (mode_hud_focus_timer_) mode_hud_focus_timer_->setEnabled(false);
    showInputModeHud(ModeHudTrigger::FocusSettled);
  }
  // 撤掉还在等光标的获得焦点提示：用户已经开始打字（候选窗马上出现在光标下方，徽章这时再放上去会盖住它），或者已经离开这个输入框。
  void cancelFocusInputModeHud() {
    mode_hud_after_focus_ = false;
    if (mode_hud_focus_timer_) mode_hud_focus_timer_->setEnabled(false);
  }
#endif
  // 中英模式切换时，「跟随」让英文模式用英文标点，中文模式恢复已保存的起始标点；固定锁定值保持不变。这只改会话状态，不改偏好文件。
  void resyncPunctuationForMode() {
    english_punctuation_ = {};
    english_chinese_punctuation_ = false;
    if (punctuation_lock_ != 0) return;
    chinese_punctuation_ = input_enabled_ && preferences_.value("chinese_punctuation", true);
    syncSessionChinesePunctuation();
  }
  bool toggleInputMode() {
    // 不检查拼写的地址栏允许本地模式切换，学习和联网仍由 privateInput() 限制。
    if (!session_ || restricted() || ic_.capabilityFlags().test(fcitx::CapabilityFlag::Sensitive) ||
        !menuTarget()) return false;
    input_enabled_ = !input_enabled_;
    ime_mode_chosen_ = true;
    // 用户手动切换：这个程序的应用例外在这次停留里让位。
    fcitx_app_rule_stay.yield(ic_.program());
    app_rule_active_ = false;
    // 从托盘切换时上下文已经失焦，失焦那一刻记下的是切换前的模式；回到这个应用时按记下的恢复，所以这里重记一次。
    if (!ic_.hasFocus()) rememberInputMode();
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
    // 按键处理读的是这份宿主副本；快照已在本地改好，下一拍读到的存储与它相同、不会重算，所以这里直接跟上，否则菜单勾掉了，`[`/`]` 在本会话里仍按旧值以词定字。
    word_character_enabled_ = enabled;
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
  void saveSmartPunctuationChoice(bool enabled) {
    if (options_path_.empty() || private_) return;
    PendingPreferenceSave request{options_path_, {}, "smart_punctuation", enabled};
    request.smart_punctuation_choice = true;
    startPreferenceSave(std::move(request));
  }
  void saveHelpcodeSchemaChoice(const char *section, const std::string &schema) {
    if (options_path_.empty() || private_) return;
    startPreferenceSave({options_path_, section, "schema", schema, false, true});
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
    const bool enabled = !preferences_.value("candidate_translations", true);
    auto snapshot = preferences_snapshot_;
    if (!snapshot.is_object() || !snapshot.contains("preferences")) return false;
    snapshot["preferences"]["candidate_translations"] = enabled;
    if (!applyPreferenceSnapshot(std::move(snapshot))) return false;
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
  // 两个槽位各放一款皮肤时，勾选候选窗当前明暗下画的那款。
  std::string currentThemeChoice() const {
    return msime::linux_host::current_theme_choice(
        preferences_, themeChoices(), msime::linux_host::candidate_dark_theme(preferences_, system_dark_));
  }
  // Choose one 主题 menu entry: the session takes the new theme at once and the store is written behind it, as the other status-bar choices are.
  bool setThemeChoice(const std::string &id) {
    if (!session_ || restricted() || privateInput()) return false;
    const auto choices = themeChoices();
    if (msime::linux_host::current_theme_choice(
            preferences_, choices, msime::linux_host::candidate_dark_theme(preferences_, system_dark_)) == id)
      return false;
    const auto change = msime::linux_host::theme_choice_change(preferences_, choices, id);
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
    candidateThemeSelectionChanged(preferences_);
    syncCandidatePanelTheme(true);
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
    // 按新作用域记下当前模式；应用例外定下、还没被手动切换过的模式不写进全局状态，和失焦时一样。
    rememberInputMode();
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
    saveNestedBooleanPreference("ai_assistant", "enabled", enabled);
    render();
    return true;
  }
  bool selectEdge(uint8_t edge) {
    if (!session_ || view_.value("candidates", Json::array()).empty()) return false;
    for (const auto &candidate : view_.at("candidates")) {
      if (!candidate.value("highlighted", false)) continue;
      const auto &id = candidate.at("id");
      return apply(msime_client_select_edge(
          session_, msime::linux_host::strict_json_required_integer<uint64_t>(id.at("generation")),
          msime::linux_host::strict_json_required_integer<size_t>(id.at("index")), edge));
    }
    return false;
  }
  void maintenance(int operation);
  bool reloadService();
  void rememberInputMode() {
    if (preferences_.value("ime_mode_scope", std::string("app")) == "global") {
      // 应用例外定下的模式不改全局状态，和 macOS、Windows 一样；用户在这个程序里手动切换过才算全局的选择。看的是这个上下文自己的 `app_rule_active_`，不是插件范围的停留：失焦时停留可能已经被别的焦点组里先获得焦点的程序换走，再按停留判断就会把用户在这里的手动切换当成规则的模式丢掉。
      if (!app_rule_active_) fcitx_global_input_mode = input_enabled_;
    } else {
      fcitx_app_input_modes.remember(ic_.program(), input_enabled_);
    }
  }
  void restoreInputMode() {
    const auto fallback = preferences_.value("default_ime_mode", "chinese") != "english";
    if (preferences_.value("ime_mode_scope", std::string("app")) == "global")
      input_enabled_ = fcitx_global_input_mode.value_or(fallback);
    else
      input_enabled_ = fcitx_app_input_modes.restore(ic_.program(), fallback);
    applyAppInputModeRule();
    ime_mode_chosen_ = true;
    mode_restore_pending_ = false;
  }
  // 应用例外（`app_input_mode_rules`）排在作用域、记忆和 `default_ime_mode` 前面：焦点从别的程序进入有规则的程序时从规则的模式开始，两种作用域下都是；用户在这次停留里手动切换后让位，离开再回来重新生效。程序名是 Fcitx5 前端报告的 `program()`，报告不了的前端（空程序名）照旧。
  void applyAppInputModeRule() {
    enterAppRuleStay(ic_.program());
    const auto rule =
        msime::linux_host::app_input_mode_rule_start(preferences_, ic_.program(), fcitx_app_rule_stay);
    app_rule_active_ = rule.has_value();
    if (rule) input_enabled_ = *rule;
  }
  void applyContextOverrides(Json &preferences) const {
    if (scheme_override_) preferences["scheme"] = *scheme_override_;
    if (shuangpin_profile_override_) preferences["shuangpin_profile"] = *shuangpin_profile_override_;
    if (helpcode_schema_override_)
      msime::linux_host::apply_helpcode_schema_choice(
          preferences, preferences.value("scheme", std::string("quanpin")) == "shuangpin" ? "shuangpin" : "quanpin",
          *helpcode_schema_override_);
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
    // The status-bar choice clears this scheme's helpcode pack in the same save, so the store holds it only once the schema matches and the pack is gone: a choice made while a pack was active saves the schema the store already has, and its failed save must not read as landed. A pack in the store after a landed choice was chosen since, on the settings page, and outranks the schema override as it does there.
    if (helpcode_schema_override_ &&
        !msime::linux_host::helpcode_pack(stored, scheme == "shuangpin" ? "shuangpin" : "quanpin").empty()) {
      if (!helpcode_schema_unsaved_) {
        helpcode_schema_override_.reset();
        dropped = true;
      }
    } else {
      expire(helpcode_schema_override_, helpcode_schema_unsaved_, stored.value(section, Json::object()), "schema");
    }
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
  // "在候选窗中显示辅助码" and the wubi code hint both decide whether the annotation belongs on the candidate row. This host appended it unconditionally, so turning either off changed nothing here.
  // In / and @ the annotation is the command title or the place's province and city, part of the row rather than a reading aid, so neither setting hides it; Wubi opens these modes too. The other local modes (super jianpin, quick phrase and the rest) still carry helpcodes there and follow both settings.
  bool showCandidateAnnotations() const {
    const auto local_mode = view_.value("local_mode", std::string("none"));
    if (local_mode == "command" || local_mode == "mention") return true;
    const auto scheme = msime::linux_host::strict_json_value(view_, "scheme", 0u);
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
  void invalidateOnlineRequests() {
    ++online_epoch_;
    online_query_.clear();
    for (auto &query : online_dispatched_queries_) query.clear();
    for (auto &slot : online_slots_) {
      // Provider workers are detached and their results are fenced by the
      // epoch. Drop the old handles here so a slow request from a previous
      // composition or session cannot block the next query from starting.
      slot.job = {};
      slot.query.clear();
    }
  }
  // 告诉 Host API 这次 AI 没有回答（#6888）。共享内核已经不放 AI 占位行，这个调用答 applied:false、不改候选列表；调用和接口留着不变；答 applied:false 时什么都不画。
  void resolveAiSeat(const std::string &query) {
    if (!session_ || query.empty()) return;
    try {
      const auto resolved = response(msime_client_resolve_online_candidates(
          session_, reinterpret_cast<const uint8_t *>(query.data()), query.size(), 1));
      if (!resolved.value("applied", false)) return;
      view_ = resolved.at("view");
      render();
    } catch (...) {}
  }
  // 不会向 AI 发问的状态下，直接告诉 Host API 当前组字没有 AI 回答；同一个查询只说一次。
  void resolveAiSeatWithoutProvider() {
    try {
      const auto query = response(msime_client_online_query(session_));
      if (!query.is_object() || !query.value("ai_eligible", false)) return;
      const auto aiConfig = query.value("ai_assistant", Json::object());
      if (!aiConfig.is_object() || !aiConfig.value("enabled", false)) return;
      const auto encoded = query.dump();
      if (encoded == ai_resolved_query_) return;
      ai_resolved_query_ = encoded;
      resolveAiSeat(encoded);
    } catch (...) {}
  }
  void clearOnlineCandidates(uint8_t source) {
    if (!session_ || source >= 2) return;
    view_ = response(msime_client_clear_online_candidates(session_, source)).at("view");
    online_dispatched_queries_[source].clear();
  }
  static bool translationPreferencesChanged(const Json &before, const Json &after) {
    for (const auto *key : {"candidate_translations", "candidate_translation_scope", "candidate_english_gloss",
                            "candidate_pronunciation",
                            "translation_target_language", "translation_secondary_language",
                            "translation_account", "custom_translation", "niutrans",
                            "tencent_tmt"}) {
      if (before.value(key, Json(nullptr)) != after.value(key, Json(nullptr))) return true;
    }
    return false;
  }
  void invalidateTranslationRequests() {
    ++translation_epoch_;
    translation_query_.clear();
    translation_pending_.clear();
    translation_answers_.clear();
    pronunciations_.clear();
    translation_manual_sentence_ = false;
    translation_session_ = 0;
    // Translation workers are detached and their results are fenced by the
    // epoch. Drop the old handle so a slow provider cannot block the next
    // session from starting a fresh translation request.
    translation_job_ = {};
  }
  void clearCandidateTranslations() {
    if (session_ && view_.contains("generation")) {
      const auto empty = std::string("[]");
      view_ = response(msime_client_apply_translations(
          session_, msime::linux_host::strict_json_required_integer<uint64_t>(view_.at("generation")),
          reinterpret_cast<const uint8_t *>(empty.data()), empty.size())).at("view");
    }
    invalidateTranslationRequests();
  }
  bool applyPreferenceSnapshot(Json snapshot) {
    if (!snapshot.is_object() || !snapshot.contains("revision") ||
        !snapshot.contains("preferences")) return false;
    const auto previousAi = preferences_.value("ai_assistant", Json::object());
    const auto nextAi = snapshot.at("preferences").value("ai_assistant", Json::object());
    const bool aiChanged = previousAi != nextAi;
    const bool previousCloud = preferences_.value("cloud_candidates", true);
    const bool nextCloud = snapshot.at("preferences").value("cloud_candidates", true);
    const bool cloudChanged = previousCloud != nextCloud;
    const bool aiChangedWhileEnabled = aiChanged &&
                                       previousAi.value("enabled", false);
    const bool translationChanged = translationPreferencesChanged(
        preferences_, snapshot.at("preferences"));
    // 先用旧偏好清掉已显示的在线候选；更新偏好后 Host API 会拒绝旧查询，旧行会因此残留。
    if (cloudChanged && previousCloud && !nextCloud) clearOnlineCandidates(0);
    if (aiChangedWhileEnabled) clearOnlineCandidates(1);
    const auto encoded = effectiveContextSnapshot(snapshot).dump();
    view_ = response(msime_client_update_preferences(
        session_, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
    preferences_ = snapshot.at("preferences");
    applyContextOverrides(preferences_);
    configureDiagnostics();
    chinese_punctuation_ = preferences_.value("chinese_punctuation", chinese_punctuation_);
    syncSessionChinesePunctuation();
    preferences_snapshot_ = std::move(snapshot);
    if (translationChanged)
      clearCandidateTranslations();
    if (aiChanged || cloudChanged)
      invalidateOnlineRequests();
    msime_linux_diagnostic_write("preferences_applied");
    return true;
  }
  bool ensure() {
    if (!menuTarget() || restricted()) { close(); clearPanel(); return false; }
    if (session_ && private_ != privateInput()) { close(); clearPanel(); }
    if (session_) return true;
    auto options = readOptions();
    startDictionarySync(options);
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
    options_path_ = options.value("preferences_directory", std::string());
    // The failed save kept across the focus change belongs to its store; once the runtime options point elsewhere it is not retried there, as refreshProviderSockets does while focused.
    if (preferences_save_retry_ && preferences_save_retry_->directory != options_path_)
      preferences_save_retry_.reset();
    // 先定下这次会话用的那一份偏好，再从它读出宿主自己保存的各项（繁体、中文标点、配对标点、智能标点、标点锁定、模式快捷键、启动模式、导航、以词定字）。这几项若在换成存储之前就按 runtime options 读好，之后不再重算：下一拍读到的存储与会话记下的快照相同，不会纠正，于是托盘里切的繁体、中文标点等换个程序又回到文件里的值，`syncSessionChinesePunctuation` 还会把文件里的中文标点推回已按存储建好的会话。
    auto session_preferences = options.value("preferences", Json::object());
    if (!options_path_.empty()) {
      try {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(options_path_.data()), options_path_.size()));
        if (snapshot.is_object() && snapshot.contains("revision") && snapshot.contains("preferences")) {
          preferences_snapshot_ = std::move(snapshot);
          // 状态栏的每一项选择（方案、主题、候选窗明暗、候选布局、云候选、AI 候选、标点、繁体和其余开关）都只写偏好存储，runtime options 文件只有设置页保存时才重写，所以新会话以存储为准，与 refreshPreferences 每一拍和 IBus 宿主（`configured["preferences"]` 跟随存储）一致。此前这里只从存储取方案、双拼方案、全半角和辅助码几项，其余仍取文件里的旧值：托盘里关掉的 AI 候选、换的皮肤在下一个新会话又回到文件里的值，而那一拍读到的存储与会话记下的快照相同，不会再纠正（#6540）。
          // 存储里还没有文件时（修订号 0）读到的只是版本缺省值，没有哪一项是状态栏选过的，仍以 runtime options 为准。
          // 键盘自定义照片只给屏幕键盘用，体积可达几百 KiB，设置页发布 runtime options 时同样去掉它（`sync_runtime_options`），这里也不带进 `msime_client_create`，免得整份选项超过 `HOST_OPTIONS_DOCUMENT_LIMIT`。
          const auto &stored = preferences_snapshot_.at("preferences");
          auto base = session_preferences;
          if (stored.is_object() &&
              msime::linux_host::strict_json_value(preferences_snapshot_, "revision", uint64_t{0}) > 0) {
            base = stored;
            if (auto theme = base.find("custom_theme"); theme != base.end() && theme->is_object())
              if (auto keyboard = theme->find("keyboard"); keyboard != theme->end() && keyboard->is_object())
                keyboard->erase("photo");
          }
          // 下面这几项不论修订号都按存储取（#618 起的规则）；有存储文件时它们已在 base 里，这里只是原样再写一遍。
          for (const auto *key : {"scheme", "shuangpin_profile", "character_width"})
            if (stored.contains(key) && stored.at(key).is_string()) base[key] = stored.at(key);
          for (const auto *section : {"quanpin_helpcode", "shuangpin_helpcode"})
            if (stored.contains(section) && stored.at(section).is_object() &&
                stored.at(section).contains("schema") && stored.at(section).at("schema").is_string())
              base[section]["schema"] = stored.at(section).at("schema");
          // A 辅助码方案 choice clears the scheme's helpcode pack in the store alone, so the pack follows the store too.
          for (const auto *scheme : {"quanpin", "shuangpin"})
            msime::linux_host::set_helpcode_pack(base, scheme, msime::linux_host::helpcode_pack(stored, scheme));
          expireContextOverrides(stored);
          session_preferences = std::move(base);
        }
      } catch (...) {
        // The prepared options remain usable for composition; preference actions will retry
        // through the normal save/reload path when the store becomes available.
      }
    }
    preferences_ = std::move(session_preferences);
    applyContextOverrides(preferences_);
    traditional_ = preferences_.value("traditional_chinese_output", false);
    traditional_variant_ = msime_linux_traditional_variant(
        preferences_.value("traditional_chinese_variant", std::string("s2t")));
    chinese_punctuation_ = preferences_.value("chinese_punctuation", true);
    paired_punctuation_ = preferences_.value("paired_punctuation", true);
    smart_punctuation_ = preferences_.value("smart_punctuation", true);
    smart_punctuation_repeat_ = preferences_.value("smart_punctuation_repeat", true);
    smart_punctuation_space_convert_ =
        preferences_.value("smart_punctuation_space_convert", false);
    const auto keybindings = preferences_.value("keybindings", Json::object());
    mode_shift_enabled_ = keybindings.value("switch_language_shift", true);
    mode_ctrl_enabled_ = keybindings.value("switch_language_ctrl", false);
    mode_ctrl_space_enabled_ = keybindings.value("switch_language_ctrl_space", true);
    mode_ctrl_alt_space_enabled_ =
        keybindings.value("switch_language_ctrl_alt_space", true);
    mode_caps_lock_enabled_ = keybindings.value("switch_language_caps_lock", false);
    character_set_shortcut_enabled_ =
        keybindings.value("toggle_character_set_ctrl_shift_f", true);
    japanese_toggle_enabled_ = keybindings.value("toggle_japanese_ctrl_shift_j", false);
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
        applyAppInputModeRule();
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
      clipboard_job_ = {};
      clipboard_mutation_job_ = {};
    }
    clipboard_path_ = std::move(clipboard_path);
    updateCloudClipboardRoute(options);
    voice_socket_ = providerSocket(options, "voice_provider_socket",
                                   "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
    const auto voicePreferences = preferences_.value("voice_input", Json::object());
    voice_enabled_ = voicePreferences.value("enabled", true);
    voice_hotkey_ctrl_f9_ = voicePreferences.value("hotkey_ctrl_f9", true);
    voice_hotkey_ralt_ = voicePreferences.value("hotkey_ralt", true);
    voice_hotkey_ctrl_win_ = voicePreferences.value("hotkey_ctrl_win", false);
    voice_hotkey_rctrl_ralt_ = voicePreferences.value("hotkey_rctrl_ralt", false);
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
    // This front end draws view.phrase_prefix ahead of the reading, so a phrase assembled out of
    // several selections stays in the composition instead of reaching the document one piece at a
    // time. Requesting it and drawing it are one decision; see core/PhrasePreedit.h.
    options["phrase_preedit"] = true;
    options["korean_romaja"] = true;
    // 和 macOS 一样让快捷短语编码参与普通候选：中文模式下直接打编码，展开就混在普通候选里（没学过时排在首页最后），仍可按 Shift+K 进入 K 模式。这是宿主能力，不写进偏好；设置页据 `HostCapabilities.quick_phrase_candidates` 说明这一点。
    options["quick_phrase_candidates"] = true;
    // The built-in sound packs of this installation, unless the runtime options name others. The Host API's own fallback looks beside the configured resource directory, which is no longer the installed one once the user has downloaded a newer dictionary into their own data directory.
    if (std::error_code error; !options.contains("sound_packs") &&
                               std::filesystem::is_directory(MSIME_SOUND_PACKS, error))
      options["sound_packs"] = MSIME_SOUND_PACKS;
    // On-device recognition reads the user's dictionary words as hotwords with the same options; see requestVoice.
    voice_host_options_ = options;
    const auto document = options.dump();
    auto created = hostReply(msime_client_create(reinterpret_cast<const uint8_t *>(document.data()), document.size()));
    // 设置页持着独占词库锁（导入、清除学习数据），而租约还没挂上或已经过期：与上面看到租约时一样把按键交给应用，等维护做完再建会话，不报“请检查运行配置”。日志每段忙碌只记一条，不再每个按键一条（#9918）。
    if (msime::linux_host::defer_session_for_dictionary_maintenance(created, dictionary_busy_noted_, [] {
          msime_linux_diagnostic_write("dictionary_busy operation=session_create");
        }))
      return false;
    if (!created.value("ok", false)) throw std::runtime_error("MSIME request failed");
    view_ = created.at("value");
    session_ = msime::linux_host::strict_json_required_integer<uint64_t>(view_.at("session"));
    // 按键音、上屏音、打字特效和背景音乐都交给生命周期核心（msime_client.h 的 MSIME_LIFECYCLE_FEEDBACK）；字段属性在第一个键之前就报告。
    msime_client_set_lifecycle(session_, MSIME_LIFECYCLE_FEEDBACK);
    syncFieldTraits();
    session_caps_lock_ = false;
    applied_preferences_revision_ = 0;
    msime_linux_diagnostic_write("focus_in");
    session_chinese_punctuation_ =
        options.at("preferences").value("chinese_punctuation", true);
    syncSessionChinesePunctuation();
    session_fullwidth_ = false;
    syncSessionCharacterWidth();
    // A mode the focus restores is a Chinese/English switch like any other; resolved here, once the lock is read and the session exists.
    if (restore_changed_mode) resyncPunctuationForMode();
    view_ = response(focusSession(true)).at("view");
    // 会话建立成功后处理尚未见过的存储选择；首次启动只建基线，已处理的选择不重复接管。
    const auto chosen = preferences_snapshot_.is_object() &&
        candidateThemeSelectionChanged(preferences_snapshot_.at("preferences"));
    syncCandidatePanelTheme(chosen);
    return true;
  }
  void refreshPreferences() {
    try {
      if (preferences_save_job_.valid()) {
        if (preferences_save_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        waitForPreferenceSave();
      }
      retryCandidatePanelSettings();
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
            const auto previousAi = preferences_.value("ai_assistant", Json::object());
            const auto nextAi = effectivePreferences.value("ai_assistant", Json::object());
            const bool aiChanged = previousAi != nextAi;
            const bool previousCloud = preferences_.value("cloud_candidates", true);
            const bool nextCloud = effectivePreferences.value("cloud_candidates", true);
            const bool cloudChanged = previousCloud != nextCloud;
            const bool aiChangedWhileEnabled = aiChanged &&
                                               previousAi.value("enabled", false);
            const bool translationChanged = translationPreferencesChanged(
                preferences_, effectivePreferences);
            // 先用旧偏好清掉已显示的在线候选；更新偏好后 Host API 会拒绝旧查询，旧行会因此残留。
            if (cloudChanged && previousCloud && !nextCloud) clearOnlineCandidates(0);
            if (aiChangedWhileEnabled) clearOnlineCandidates(1);
            const auto encoded = effective.dump();
            view_ = response(msime_client_update_preferences(session_,
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
            // 声音设置变了而本会话正激活着：再声明一次激活。进程里什么声音都没开时核心不起播放器，激活那一刻的音乐请求就没人收到；不重新声明，刚在设置里打开的背景音乐要等到下一次换焦点才响。
            if (music_.told() && preferences_.value("plugins", Json(nullptr)) !=
                                     effectivePreferences.value("plugins", Json(nullptr)))
              msime_client_set_active(session_, true);
            // 与插件共用的已见选择比较，失焦窗口补读同一份存储不能再次接管。
            const auto chosen = candidateThemeSelectionChanged(snapshot.at("preferences"));
            preferences_ = std::move(effectivePreferences);
            configureDiagnostics();
            // A width chosen here that the store does not hold - its save failed, or a private window, which never saves - is not undone by the store, as a failed scheme choice is kept; the next session re-reads the store.
            if (!private_ && !unsavedChoice("", "character_width")) syncSessionCharacterWidth();
            traditional_ = preferences_.value("traditional_chinese_output", traditional_);
            traditional_variant_ = msime_linux_traditional_variant(
                preferences_.value("traditional_chinese_variant", std::string("s2t")));
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
            mode_ctrl_space_enabled_ = reloaded.value("switch_language_ctrl_space", true);
            mode_ctrl_alt_space_enabled_ = reloaded.value(
                "switch_language_ctrl_alt_space", mode_ctrl_alt_space_enabled_);
            mode_caps_lock_enabled_ = reloaded.value("switch_language_caps_lock", mode_caps_lock_enabled_);
            character_set_shortcut_enabled_ = reloaded.value(
                "toggle_character_set_ctrl_shift_f", character_set_shortcut_enabled_);
            japanese_toggle_enabled_ = reloaded.value("toggle_japanese_ctrl_shift_j", japanese_toggle_enabled_);
            // The toolbar follows the reloaded switches without waiting for the
            // next focus change, the way the IBus property menu does.
            refreshToolbar();
            syncCandidatePanelFont();
            syncCandidatePanelTheme(chosen);
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
            voice_language_ = voicePreferences.value("language", voice_language_);
            loadVoiceOptions();
            syncVoiceOverlayTheme();
            syncVoiceAction();
            preferences_snapshot_ = std::move(snapshot);
            if (translationChanged)
              clearCandidateTranslations();
            if (aiChanged || cloudChanged)
              invalidateOnlineRequests();
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
        preferences_job_ = {};
        preferences_save_job_ = {};
        preferences_save_retry_.reset();
      }
      candidate_skin_catalog_ = parseCandidateSkinCatalog(options);
      candidate_skin_document_ = options.value("candidate_skin_catalog", Json());
      refreshThemeMenu();
      noteSchemeOptions(options);
      // 运行时选项的一次刷新（皮肤目录、提供方 socket）不是用户主动选主题。
      syncCandidatePanelTheme(false);
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
        clipboard_job_ = {};
        clipboard_mutation_job_ = {};
        clipboard_path_ = std::move(nextClipboard);
      }
      updateCloudClipboardRoute(options);
      auto nextVoice = providerSocket(options, "voice_provider_socket",
                                      "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
      if (nextVoice != voice_socket_) {
        if (voice_loading_) cancelVoice();
        voice_socket_ = std::move(nextVoice);
      }
      const auto nextOnline = onlineSocket(options);
      if (nextOnline != online_socket_) {
        online_socket_ = nextOnline;
        clearOnlineCandidates(0);
        clearOnlineCandidates(1);
        invalidateOnlineRequests();
      }
      auto nextTranslation = translationSocket(options);
      if (nextTranslation != translation_socket_) {
        translation_socket_ = std::move(nextTranslation);
        clearCandidateTranslations();
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
            if (msime::linux_host::strict_json_value(item, "source", uint64_t{255}) == source)
              candidates.push_back(item.at("text"));
          }
          bool applied = false;
          if (!candidates.empty()) {
            const auto encoded = candidates.dump();
            const auto reply = response(msime_client_apply_online_candidates(
                session_, reinterpret_cast<const uint8_t *>(slot.query.data()), slot.query.size(),
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(), source));
            view_ = reply.at("view");
            applied = reply.value("applied", false);
            render();
          }
          // AI 的网络请求落空（失败、空回答、被拒）：告诉 Host API 这次没有回答（#6888）。
          if (source == 1 && !slot.cache_only && !applied) resolveAiSeat(slot.query);
        }
      }
      if (session_ && ic_.hasFocus() && !restricted() &&
          (online_socket_.empty() || privateInput())) {
        resolveAiSeatWithoutProvider();
        return;
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
        // shortcut: 同查询失败后等输入或配置变化再试；需要后台恢复时改为有界退避。
        if (!enabled || slot.job.valid() || encoded == online_dispatched_queries_[source]) continue;
        // AI 缓存探测立即显示旧答案；它不占用防抖后网络刷新的分发记录。
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
        slot.cache_only = cacheOnly;
        slot.job = detachedJob(
            [providerEncoded, encoded, socket = online_socket_] {
              auto raw = response(msime_client_online_provider_request(
                  reinterpret_cast<const uint8_t *>(providerEncoded.data()), providerEncoded.size(),
                  reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
              Json result = raw.is_object() ? raw : Json::object();
              result["query"] = encoded;
              return result;
            });
        if (!cacheOnly) online_dispatched_queries_[source] = encoded;
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
  // 算一份查询文档：整句翻译、/fy、候选词翻译的主查询，或「仅当前高亮」时的 gloss_query（整页离线释义）。identity 是它所属的那份共享查询，refreshTranslations 拿它比对过期和去重，空时就是 query 本身；glossStage 为真时这份是 gloss_query：走同一条路径，但不连 provider，回答交给 Rust 时标 "gloss": true。
  void startTranslation(const Json &query, bool offline, Json local = Json::array(),
                        bool manual_sentence = false, std::string identity = {}, bool glossStage = false) {
    const auto encoded = identity.empty() ? query.dump() : std::move(identity);
    translation_query_ = encoded;
    translation_manual_sentence_ = manual_sentence;
    translation_session_ = session_;
    translation_job_epoch_ = translation_epoch_;
    // 离线词典查询里的候选取自这份查询：「仅当前高亮」时主查询只有高亮那一个候选，不能换成视图的整页。
    const bool dictionary = offlineDictionary(query);
    auto glossRequest = Json{{"generation", msime::linux_host::strict_json_required_integer<uint64_t>(query.at("generation"))},
                             {"user_data", query.value("user_data", Json())},
                             {"candidates", msime::linux_host::translation_lookup_candidates(query.at("candidates"))}};
    if (dictionary) glossRequest["target_language"] = query.at("target_language");
    const auto gloss = glossRequest.dump();
    // 只认文字的 provider 协议丢掉来源之前，按每个候选自己带的来源剔除只展示原文的行（CandidateTranslationScope.h）；查询只有高亮那一个候选时不能再按视图的下标去对。
    std::vector<bool> providerAllowed;
    providerAllowed.reserve(query.at("candidates").size());
    const bool bypassSourceGate = manual_sentence || commandTranslation(query);
    for (const auto &candidate : query.at("candidates"))
      providerAllowed.push_back(bypassSourceGate ||
                                msime::linux_host::translation_candidate_reaches_provider(candidate));
    // gloss_query 和只剩离线释义的那份文档不点名服务，不连 provider：非高亮候选到不了它。
    const auto socket = !glossStage &&
                        (manual_sentence || commandTranslation(query) ||
                         (preferences_.value("candidate_translations", true) &&
                          msime::linux_host::translation_query_reaches_service(query)))
                            ? translation_socket_ : std::string{};
    // 结果按开始时配置的 socket 核对，不按这一段实际连不连它：只查离线词典的那一段也要能交回来。
    const auto fence = translation_socket_;
    // 打开「显示读音」时，这一段做完后在同一个工作线程上为它的回答查读音（CandidatePronunciation.h）；整句翻译和 /fy 的回答不是释义，不查。
    const bool pronounce = !manual_sentence && !commandTranslation(query) && pronunciationTargets().size() > 0;
    translation_job_ = detachedJob(
        [query, encoded, gloss, offline, local, socket, fence, dictionary, providerAllowed, glossStage, pronounce,
         targets = pronunciationTargets(),
         pronunciationRoot = msime::linux_host::pronunciation_supplementary_root(options_path_),
         resources = resources_] () mutable {
          // 这一段的回答能不能记下来（主查询做完了，要问 provider 时它也答上了）：refreshTranslations 只记这样的，失败的下一次照常重试。
          bool answered = offline && socket.empty();
          if (offline) {
            try {
              local = preferOnline(response(msime_client_candidate_gloss_request(
                  reinterpret_cast<const uint8_t *>(gloss.data()), gloss.size(),
                  reinterpret_cast<const uint8_t *>(resources.data()), resources.size()))
                  .value("translations", Json::array()), local);
            } catch (...) {} // A missing local dictionary must not prevent online fallback.
          } else if (!socket.empty()) {
            auto transport = query;
            auto missing = Json::array();
            size_t candidateIndex = 0;
            for (const auto &candidate : query.at("candidates")) {
              const auto &text = candidate.at("text");
              const bool sourceAllowed = candidateIndex >= providerAllowed.size() ||
                  providerAllowed[candidateIndex];
              if (sourceAllowed && (dictionary || std::none_of(local.begin(), local.end(), [&](const Json &item) {
                    return item.at("text") == text;
                  }))) missing.push_back(text);
              ++candidateIndex;
            }
            transport["candidates"] = std::move(missing);
            answered = transport.at("candidates").empty();
            if (!answered) {
              const auto request = transport.dump();
              try {
                auto result = response(msime_client_translation_provider_request(
                    reinterpret_cast<const uint8_t *>(request.data()), request.size(),
                    reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
                answered = result.is_object() && result.contains("translations") &&
                           result.at("translations").is_array() && !result.at("translations").empty();
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
                    msime::host_api::discard_string(msime_client_translation_gloss_save(
                        reinterpret_cast<const uint8_t *>(save.data()), save.size(),
                        reinterpret_cast<const uint8_t *>(userData.data()), userData.size()));
                  }
                }
              } catch (...) {} // Retain local hits on provider failure.
            }
          }
          auto pronunciations = Json::object();
          if (pronounce) {
            try {
              pronunciations = msime::linux_host::lookup_translation_pronunciations(
                  local, targets, msime::linux_host::strict_json_required_integer<uint64_t>(query.at("generation")),
                  pronunciationRoot, [&](const std::string &lookup) {
                    try {
                      return response(msime_client_pronunciation_request(
                          reinterpret_cast<const uint8_t *>(lookup.data()), lookup.size(),
                          reinterpret_cast<const uint8_t *>(resources.data()), resources.size()));
                    } catch (...) { return Json(); }
                  });
            } catch (...) {} // 读音只是附加的显示，查不到时照常交回释义。
          }
          return Json{{"query", encoded}, {"translations", local},
                      {"continue_online", offline && !socket.empty()},
                      {"gloss", glossStage}, {"answered", answered && !glossStage},
                      {"pronunciations", std::move(pronunciations)},
                      {"_socket", fence}};
        });
  }
  static Json sentenceTranslationQuery(Json query, const std::string &text,
                                       const std::string &target_language) {
    query["sentence"] = true;
    query["target_language"] = target_language;
    // startTranslation reads each candidate's text before sending the provider
    // its string-only wire format. Keep the Host API candidate shape here.
    query["candidates"] = Json::array({Json{{"text", text}}});
    // 整句翻译只发这一句；「仅当前高亮」时查询带着的整页离线释义不跟着发给 provider。
    query.erase("gloss_query");
    return query;
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
      query = sentenceTranslationQuery(
          std::move(query), text,
          preferences_.value("translation_target_language", std::string("en")));
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
        const bool request_is_current = translation_job_epoch_ == translation_epoch_;
        bool manual_query_matches = false;
        if (translation_manual_sentence_ && result.is_object()) {
          try {
            const auto parsed_query = Json::parse(result.value("query", "{}"));
            manual_query_matches = msime::linux_host::strict_json_value(
                                       parsed_query, "generation", uint64_t{0}) ==
                                   msime::linux_host::strict_json_value(query, "generation", uint64_t{0});
          } catch (...) {}
        }
        if (request_is_current && allowed && session_ == translation_session_ && query.is_object() &&
            result.is_object() &&
            (result.value("query", "") == encodedQuery || manual_query_matches) &&
            result.value("_socket", std::string{}) == translation_socket_) {
          const auto translations = result.value("translations", Json::array());
          if (!pronunciationTargets().empty()) pronunciations_.store(result.value("pronunciations", Json::object()));
          if (!translations.empty()) {
            // gloss_query 的回答是整页的离线释义，标上 "gloss": true，「仅当前高亮」时 Rust 照旧画在每一行。
            const auto encoded = (result.value("gloss", false)
                                      ? msime::linux_host::mark_gloss_answers(translations)
                                      : translations).dump();
            view_ = response(msime_client_apply_translations(
                session_, msime::linux_host::strict_json_required_integer<uint64_t>(query.at("generation")), reinterpret_cast<const uint8_t *>(encoded.data()),
                encoded.size())).at("view");
          }
          render();
          const auto primary = msime::linux_host::split_translation_query(query).main;
          if (result.value("continue_online", false)) {
            startTranslation(primary, false, result.at("translations"), false, encodedQuery);
            return;
          }
          // gloss_query 查完，接着发主查询。
          if (result.value("gloss", false)) {
            startPrimaryTranslation(primary, encodedQuery);
            return;
          }
          // 主查询做完：「仅当前高亮」时记下回答，高亮移回来时复用。
          if (result.value("answered", false) && !translation_manual_sentence_ && !commandTranslation(primary) &&
              translationScopeHighlighted())
            translation_answers_.store(msime::linux_host::translation_answer_key(primary), translations);
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
      // 「仅当前高亮」：有 gloss_query 时先查整页离线释义（只读本机词典，很快），查完再发只有高亮那一个候选的主查询（CandidateTranslationScope.h）；范围是「全部」时没有 gloss_query，主查询就是整份查询。
      const auto parts = msime::linux_host::split_translation_query(query);
      if (parts.gloss.is_object())
        startTranslation(parts.gloss, translationOffline(parts.gloss), Json::array(), false, encodedQuery, true);
      else
        startPrimaryTranslation(parts.main, encodedQuery);
    } catch (...) { /* Never expose candidate text or provider credentials in errors. */ }
  }
  // 要不要先查离线词典：英文离线释义开着，或目标语言装了离线词典；/fy 从不查。
  bool translationOffline(const Json &query) const {
    return !commandTranslation(query) && (query.value("english_gloss", false) || offlineDictionary(query));
  }
  // 为还没到的释义预留几行：开了译文且有译文服务、开了离线英文释义时按配置的目标语言一到两行（macOS 的 reservedGlossHeightForFont），否则不留。原生画布横排整张卡片的预留和「仅当前高亮」时高亮行（视图的 translation_reserved）的预留都用它。
  int translationReservedLines() const {
    const bool glosses = preferences_.value("candidate_english_gloss", false) ||
        (preferences_.value("candidate_translations", true) && !translation_socket_.empty());
    if (!glosses) return 0;
    const auto language = [this](const char *key, const char *fallback) {
      const auto at = preferences_.find(key);
      return at != preferences_.end() && at->is_string() ? at->get<std::string>() : std::string(fallback);
    };
    return static_cast<int>(msime::linux_host::translation_reserved_lines(
        language("translation_target_language", "en"), language("translation_secondary_language", "")));
  }
  // 「显示读音」（偏好 candidate_pronunciation，默认关闭）打开时释义各行的目标语言，Linux 只画主目标语言那一行；关着时为空。
  std::vector<std::string> pronunciationTargets() const {
    if (!preferences_.value("candidate_pronunciation", false)) return {};
    const auto target = preferences_.find("translation_target_language");
    return {target != preferences_.end() && target->is_string() ? target->get<std::string>() : std::string("en")};
  }
  // 一个候选的读音，"\n" 分行，与它的释义逐行对应；没有时为空。
  std::string candidatePronunciation(const Json &candidate) const {
    const auto text = candidate.find("text"), translation = candidate.find("translation");
    if (text == candidate.end() || !text->is_string() || translation == candidate.end() || !translation->is_string())
      return {};
    return pronunciations_.candidate(text->get<std::string>(), translation->get_ref<const std::string &>(),
                                     pronunciationTargets());
  }
  // 候选词翻译开着且范围是「仅当前高亮」（CandidateTranslationScope.h）。
  bool translationScopeHighlighted() const {
    return msime::linux_host::translation_scope_highlighted(
        preferences_.value("candidate_translations", true), preferences_);
  }
  // 发候选词翻译的主查询。「仅当前高亮」时高亮移回一个问过的候选：代次推进了，查询的其余部分不变，记下的回答直接交给 Rust，不再问 provider。
  void startPrimaryTranslation(const Json &primary, const std::string &identity) {
    if (!primary.is_object()) {
      translation_query_ = identity;
      return;
    }
    if (!commandTranslation(primary) && translationScopeHighlighted()) {
      if (const auto *answers = translation_answers_.find(msime::linux_host::translation_answer_key(primary))) {
        const auto encoded = answers->dump();
        view_ = response(msime_client_apply_translations(
            session_, msime::linux_host::strict_json_required_integer<uint64_t>(primary.at("generation")),
            reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())).at("view");
        translation_query_ = identity;
        render();
        return;
      }
    }
    startTranslation(primary, translationOffline(primary), Json::array(), false, identity);
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
            msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == clipboard_generation_)
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
        msime::linux_host::strict_json_value(view_, "scheme", -1), view_.value("nine_key", false),
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
            msime::host_api::discard_string(raw);
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
    // 提交前的文档不再描述新光标；先作废，保留同步前端在提交回调内发布的新前文。
    ic_.surroundingText().invalidate();
    ic_.commitString(text);
    if (typingStatistics) recordTypingStatistics(text, source.value_or(typingSource()));
    rememberSmartPunctuationCommit(text);
  }
  // 「宣传水杉输入法」：先把正在输入的组字上屏，再插入固定的产品介绍（MenuLabels.h 的 kPromotionText），与 IBus 宿主相同。Fcitx5 只从工具栏菜单项进入，不占用 Ctrl+Shift+P：Linux 应用普遍把它用作命令面板或隐私窗口。
  void commitPromotion() {
    if (session_ && !view_.value("editing_text", std::string{}).empty()) command(MSIME_FINISH_COMPOSITION);
    commitText(msime::linux_host::kPromotionText, msime::linux_host::TypingSource::Local);
  }
  // surrounding text 不可用时，只用本会话由输入法自己上屏的末尾文本判断智能标点；交给应用的按键或光标编辑会使它失效。
  void rememberSmartPunctuationCommit(const std::string &text) {
    if (!ic_.hasFocus() || restricted() || privateInput()) {
      forgetSmartPunctuationShadow();
      return;
    }
    smart_punctuation_shadow_ += text;
    if (smart_punctuation_shadow_.size() > 1024) {
      size_t cut = smart_punctuation_shadow_.size() - 1024;
      while (cut < smart_punctuation_shadow_.size() &&
             (static_cast<unsigned char>(smart_punctuation_shadow_[cut]) & 0xc0) == 0x80)
        ++cut;
      smart_punctuation_shadow_.erase(0, cut);
    }
    smart_punctuation_shadow_valid_ = true;
  }
  void forgetSmartPunctuationShadow() {
    smart_punctuation_shadow_.clear();
    smart_punctuation_shadow_valid_ = false;
  }
  void eraseSmartPunctuationShadowLast() {
    if (!smart_punctuation_shadow_valid_ || smart_punctuation_shadow_.empty()) {
      forgetSmartPunctuationShadow();
      return;
    }
    size_t last = smart_punctuation_shadow_.size() - 1;
    while (last > 0 &&
           (static_cast<unsigned char>(smart_punctuation_shadow_[last]) & 0xc0) == 0x80)
      --last;
    smart_punctuation_shadow_.erase(last);
  }
  std::optional<uint32_t> smartPunctuationShadowLast() const {
    if (!smart_punctuation_shadow_valid_ || smart_punctuation_shadow_.empty())
      return std::nullopt;
    auto start = smart_punctuation_shadow_.end();
    do {
      --start;
    } while (start != smart_punctuation_shadow_.begin() &&
             (static_cast<unsigned char>(*start) & 0xc0) == 0x80);
    return fcitx::utf8::getChar(start, smart_punctuation_shadow_.end());
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
  void invalidateCloudClipboard() {
    ++cloud_clipboard_generation_;
    cloud_clipboard_items_.clear();
    cloud_clipboard_enabled_ = true;
    cloud_clipboard_job_ = {};
  }
  void updateCloudClipboardPreferences() {
    const auto relay = preferences_.value("cloud_clipboard_relay", Json::object());
    if (cloud_clipboard_directory_ == options_path_ && cloud_clipboard_relay_ == relay) return;
    invalidateCloudClipboard();
    cloud_clipboard_directory_ = options_path_;
    cloud_clipboard_relay_ = relay;
  }
  void updateCloudClipboardRoute(const Json &options) {
    const auto nextCloudClipboard = cloudClipboardSocket(options);
    const auto explicitSocket = explicitCloudClipboardSocket(options);
    if (nextCloudClipboard != cloud_clipboard_socket_ || explicitSocket != cloud_clipboard_explicit_socket_) {
      invalidateCloudClipboard();
      cloud_clipboard_socket_ = nextCloudClipboard;
      cloud_clipboard_explicit_socket_ = explicitSocket;
    }
    updateCloudClipboardPreferences();
  }
  void refreshCloudClipboard() {
    updateCloudClipboardPreferences();
    try {
      if (cloud_clipboard_job_.valid()) {
        if (cloud_clipboard_job_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return;
        auto result = cloud_clipboard_job_.get();
        cloud_clipboard_job_ = {};
        if (ic_.hasFocus() && !restricted() && !privateInput() && result.is_object() &&
            result.value("_socket", std::string{}) == cloud_clipboard_socket_ &&
            result.value("_directory", std::string{}) == options_path_ &&
            msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == cloud_clipboard_generation_)
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
    if ((cloud_clipboard_socket_.empty() && options_path_.empty()) || restricted() || privateInput() || !ic_.hasFocus()) return false;
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
    const auto explicitSocket = cloud_clipboard_explicit_socket_;
    const auto directory = options_path_;
    cloud_clipboard_job_ = detachedJob([socket, explicitSocket, directory, generation] {
      auto raw = msime::linux_host::cloud_clipboard_list(directory, explicitSocket, socket,
          [](const std::string &path) {
            return response(msime_client_cloud_clipboard_source(
                reinterpret_cast<const uint8_t *>(path.data()), path.size()));
          },
          [](const Json &request) {
            const auto encoded = request.dump();
            return response(msime_client_relay_cloud_clipboard(
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
          },
          [](const std::string &path, const Json &action) {
            const auto encoded = action.dump();
            return response(msime_client_cloud_clipboard_provider_request(
                reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
                reinterpret_cast<const uint8_t *>(path.data()), path.size()));
          });
      if (!raw.is_object()) return Json::object();
      raw["_socket"] = socket;
      raw["_directory"] = directory;
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
        if (emojiGroupsResultCurrent(result, emoji_generation_, emoji_category_)) {
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
        const bool current = result.is_object() && msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == emoji_generation_;
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
        result["_category"] = category;
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
  // 浮层上圆形操作按钮的一次点击。浮层在自己的 `update()` 或 `dispatch_events()` 里报告它，所以这里可能运行在帧定时器或显示连接监听的回调之内，并把那个事件源拆掉。
  void handleVoiceOverlayAction(msime::linux_host::WaveOverlayModel::Action action) {
    if (action == msime::linux_host::WaveOverlayModel::Action::Cancel) {
      cancelVoice();
    } else if (voice_loading_ && wave_overlay_.processing()) {
      // 识别或润色进行中点确认只收起浮条，与 macOS 的 `dismissProcessing` 一样；结果到达后照常上屏。
      wave_overlay_dismissed_ = true;
      hideVoiceOverlay();
    } else {
      stopVoice();
    }
  }
  void hideVoiceOverlay() {
    if (wave_overlay_surface_ && wave_overlay_visible_)
      wave_overlay_surface_->hide();
    wave_overlay_visible_ = false;
    syncVoiceOverlaySources();
  }
  // 按屏幕上的实际情况启停 16 ms 的波形动画和浮层显示连接的监听：画出来的浮层显示波形时跑动画，只要画出来的浮层在显示就监听。辅助文本回退两者都不需要。
  void syncVoiceOverlaySources() {
    auto *surface = wave_overlay_surface_.get();
    const bool shown = surface && wave_overlay_visible_ && !wave_overlay_failed_;
    const bool animate = shown && surface->animates() && wave_overlay_.animates();
    if (animate && !wave_overlay_frame_timer_ && loop_) {
      constexpr uint64_t interval = msime::linux_host::kWaveOverlayFrameMilliseconds * 1000;
      wave_overlay_frame_timer_ = loop_->addTimeEvent(
          CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + interval, 1000,
          [this](fcitx::EventSourceTime *timer, uint64_t now) {
            // `update()` 还会读取 Xlib 或 libwayland 已经排进队列的指针和 expose 事件，在那里处理的操作可能重置 `wave_overlay_frame_timer_`，在调用尚未返回时就销毁这个闭包和它捕获的 `this`。因此 `update()` 之后的一切都经过栈上的这份副本，绝不经过捕获。
            auto *const self = this;
            if (self->wave_overlay_surface_ && self->wave_overlay_visible_ && self->wave_overlay_.animates()) {
              // 提供商的音量在它自己的线程上到达；每一帧读取最新的值，不等 250 ms 的轮询。
              if (const auto mailbox = self->voice_mailbox_) {
                std::lock_guard lock(mailbox->mutex);
                if (mailbox->level_seen) self->wave_overlay_.set_input_level(static_cast<float>(mailbox->level) / 10.0f);
              }
              self->wave_overlay_.advance(static_cast<double>(now) / 1e6);
              self->wave_overlay_surface_->update(self->wave_overlay_);
            }
            if (self->wave_overlay_frame_timer_.get() == timer) {
              timer->setNextInterval(interval);
              timer->setOneShot();
            }
            return true;
          });
    } else if (!animate && wave_overlay_frame_timer_) {
      wave_overlay_frame_timer_.reset();
      // 下一段波形从圆点开始，与 macOS 在动画停止时复位波形条一样。
      wave_overlay_.levels.fill(0.0f);
    }
    const int fd = shown ? surface->event_fd() : -1;
    if (fd != wave_overlay_io_fd_) {
      wave_overlay_io_.reset();
      wave_overlay_io_fd_ = fd;
      if (fd >= 0 && loop_)
        wave_overlay_io_ = loop_->addIOEvent(fd, fcitx::IOEventFlag::In,
            [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) {
              // 在 `dispatch_events()` 里处理的操作可能收起浮层，从而重置 `wave_overlay_io_` 并销毁这个闭包；调用之后不得再读取捕获。
              if (wave_overlay_surface_ && wave_overlay_visible_) wave_overlay_surface_->dispatch_events();
              return true;
            });
    }
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
  // `FcitxEngine::applySystemTheme` 在主循环收到全插件明暗探测结果后调用；明暗变化不是主动选主题，不能接管第三方主题。
  void setSystemDark(bool dark) {
    if (dark == system_dark_) return;
    system_dark_ = dark;
    syncVoiceOverlayTheme();
    if (voice_loading_) updateVoiceOverlay();
    syncCandidatePanelTheme(false);
  }
  void updateVoiceOverlay() {
    wave_overlay_.status = voice_phase_;
    wave_overlay_.listening = voice_loading_ && !wave_overlay_.processing();
    wave_overlay_.locked = voice_space_locked_ && wave_overlay_.listening;
    wave_overlay_.set_transcript(voice_transcript_);
    wave_overlay_.set_input_level(static_cast<float>(voice_level_) / 10.0f);
    if (wave_overlay_dismissed_) {
      hideVoiceOverlay();
      return;
    }
    if (wave_overlay_surface_ && !wave_overlay_failed_) {
      if (wave_overlay_visible_) {
        wave_overlay_surface_->update(wave_overlay_);
        syncVoiceOverlaySources();
        return;
      }
      wave_overlay_visible_ = wave_overlay_surface_->show(wave_overlay_);
      syncVoiceOverlaySources();
      if (wave_overlay_visible_) return;
      // No surface on this display (GNOME Wayland has no layer-shell): the rest of this recording uses the auxiliary text, as the IBus host's FallbackSurface does. The next recording tries the surface again.
      wave_overlay_failed_ = true;
    }
    if (voice_loading_) {
      // 与 IBus 宿主回退显示的文字相同：状态或锁定提示、麦克风音量条、最新的转写文字。
      ic_.inputPanel().setAuxUp(fcitx::Text(msime::linux_host::wave_overlay_feedback_text(wave_overlay_)));
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
  }
  // 告诉用户为什么什么也没输入，就像 Windows 服务用消息框、macOS 用浮层、IBus 宿主用 `show_voice_failure` 做的那样：浮层（没有浮层时就是辅助文本）显示一条固定消息六秒，或者直到按 Esc、开始下一次录音。消息刻意固定，因为提供商的错误可能带有隐私内容。
  void showVoiceFailure(const char *message) {
    if (++voice_failure_id_ == 0) ++voice_failure_id_;
    const auto id = voice_failure_id_;
    voice_failure_message_ = message;
    voice_failure_visible_ = true;
    wave_overlay_dismissed_ = false;
    wave_overlay_.reset();
    wave_overlay_.status = message;
    wave_overlay_.show_transcript = false;
    wave_overlay_.failure = true;
    wave_overlay_.listening = false;
    if (wave_overlay_surface_) {
      if (wave_overlay_visible_)
        wave_overlay_surface_->update(wave_overlay_);
      else
        wave_overlay_visible_ = wave_overlay_surface_->show(wave_overlay_);
      syncVoiceOverlaySources();
    }
    // 画得出浮层时提示只在浮层上，与 macOS 和 IBus 宿主一样；画不出时才写辅助行，`dismissVoiceFailure` 按内容认领后收掉它。
    if (!(wave_overlay_visible_ && wave_overlay_surface_->animates())) {
      ic_.inputPanel().setAuxUp(fcitx::Text(voice_failure_message_));
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
    voice_failure_timer_ = loop_->addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + uint64_t{msime::linux_host::kWaveOverlayFailureMilliseconds} * 1000, 0,
        [this, id](fcitx::EventSourceTime *, uint64_t) {
          if (voice_loading_ || voice_failure_id_ != id) return true;
          dismissVoiceFailure();
          return true;
        });
  }
  void dismissVoiceFailure() {
    if (!voice_failure_visible_) return;
    voice_failure_visible_ = false;
    hideVoiceOverlay();
    wave_overlay_.reset();
    if (ic_.inputPanel().auxUp().toString() == voice_failure_message_) {
      ic_.inputPanel().setAuxUp(fcitx::Text());
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
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
                voice_options_.value("asr_provider", std::string{"local"}), "tsf");
            if (inlinePreedit) {
              voice_preedit_ = partial;
              voice_transcript_.clear();
            } else {
              voice_transcript_ = partial;
              voice_preedit_.clear();
            }
            render();
          }
          const char *phaseLabel[] = {msime::linux_host::kVoiceStatusRecording,
                                      msime::linux_host::kVoiceStatusRecognizing,
                                      msime::linux_host::kVoiceStatusProcessing};
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
              voice_options_.value("asr_provider", std::string{"local"}), "tsf");
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
    voice_toggle_chord_ = msime::host_api::decode_toggle_hotkey(preferences_, "voice_input");
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
    const auto generation = msime::linux_host::strict_json_value(view_, "generation", uint64_t{});
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
    voice_phase_ = msime::linux_host::kVoiceStatusRecording;
    voice_level_ = 0;
    voice_failure_visible_ = false;
    wave_overlay_failed_ = false;
    wave_overlay_dismissed_ = false;
    wave_overlay_.reset();
    wave_overlay_.listening = true;
    wave_overlay_.show_transcript = true;
    updateVoiceOverlay();
    const auto mailbox = voice_mailbox_;
    voice_job_ = detachedJob([socket, generation, language, options, host_options, mailbox] {
      auto request = msime::linux_host::voice_query(language, generation, options, host_options);
      request["stream"] = true;
      const auto query = request.dump();
      auto raw = msime::host_api::own_string(
          msime_client_voice_provider_stream_feedback_cancelled(
              reinterpret_cast<const uint8_t *>(query.data()), query.size(),
              reinterpret_cast<const uint8_t *>(socket.data()), socket.size(),
              fcitxVoiceUpdate, fcitxVoiceStatus, fcitxVoiceLevel,
              fcitxVoiceCancelled, mailbox.get()));
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
    // 已经停止录音、正在等识别：再次确认、按快捷键或点菜单都不能再向提供商发送停止，否则它的拒绝会取消这次录音。
    if (wave_overlay_.processing()) return true;
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
    voice_phase_ = msime::linux_host::kVoiceStatusRecognizing;
    wave_overlay_.listening = false;
    wave_overlay_.compact_status =
        msime::linux_host::WaveOverlayModel::CompactStatus::Recognizing;
    updateVoiceOverlay();
    return true;
  }
  bool cancelVoice() {
    if (!voice_loading_) return false;
    if (voice_mailbox_) voice_mailbox_->cancelled.store(true, std::memory_order_relaxed);
    const auto socket = voice_socket_;
    const auto generation = voice_generation_;
    if (!socket.empty() && generation != 0)
      msime::host_api::discard_string(msime_client_voice_provider_cancel(
          reinterpret_cast<const uint8_t *>(socket.data()), socket.size(), generation));
    voice_cancelled_ = true;
    voice_mailbox_.reset();
    // The worker is detached and owns its mailbox; dropping this handle lets
    // a new recording start even if the cancelled provider takes a long time
    // to acknowledge cancellation.
    voice_job_ = {};
    voice_loading_ = false;
    voice_space_consumed_ = false;
    voice_space_locked_ = false;
    voice_partial_seen_ = false;
    voice_phase_seen_ = false;
    voice_level_seen_ = false;
    voice_preedit_.clear();
    voice_transcript_.clear();
    voice_phase_ = msime::linux_host::kVoiceStatusRecording;
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
                 msime::linux_host::PunctuationPairMode::Unpaired,
             char spaceConvertKey = 0) {
    auto result = response(raw);
    pair_inserted_ = false;
    if (result.contains("commit") && result["commit"].is_string()) {
      auto text = result["commit"].get<std::string>();
      const auto context = result.value("commit_context", Json(nullptr));
      if (traditionalApplies() && (!context.is_object() || context.value("script_conversion", true)))
        text = msime_linux_simplified_to_traditional(text, traditional_variant_);
      // `、` 的 ASCII 孪生按产生它的键和 View 的顿号开关定（enumerationComma 段）。
      const auto spaceConvertAscii =
          spaceConvertPreceding
              ? msime::linux_host::smart_punctuation_ascii_mark(
                    text, spaceConvertKey, viewPunctuationMarks())
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
      commitText(text, std::nullopt,
                 !context.is_object() || context.value("typing_statistics", true));
      // 上屏音（或随上屏前进的旋律音符）由生命周期核心在这次派发里出过了，排在按键音之后；隐私字段里核心什么都不出。回复的 feedback 带着上屏之后的打字特效：上屏不计数，只报告连击现在的样子，停顿后断掉的连击就这样让接着画出的辅助行跟着清掉。focus 等不是派发的调用没有它，连击照旧。
      if (const auto effect = msime::linux_host::reply_typing_effect(result); effect && !text.empty())
        typing_combo_ = msime::linux_host::typing_effect_combo(*effect);
      if (pair_inserted_) {
        if (const auto closing = msime::linux_host::paired_closing_from_text(text))
          paired_tracker_.push(*closing);
      }
      if (spaceConvertAscii != 0 && !pair_inserted_) {
        // Preserve Engine's actual half (notably opening/closing quotes).
        space_convert_mark_ = text;
        space_convert_key_ = spaceConvertKey;
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
  std::optional<std::vector<std::string>> precedingCharacters(
      size_t count, std::optional<size_t> validatedLength = std::nullopt) {
    const auto &surrounding = ic_.surroundingText();
    if (privateInput() || !ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) ||
        !surrounding.isValid() || surrounding.cursor() != surrounding.anchor())
      return std::nullopt;
    const auto &text = surrounding.text();
    if (validatedLength)
      return preceding_characters_with_validated_length<std::vector<std::string>>(
          text, surrounding.cursor(), count, *validatedLength);
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
  // 英文模式下的中文标点按 View 报告的样式出字（host-api 已解析好 corner_when_traditional）。
  msime::linux_host::PunctuationMarks viewPunctuationMarks() const {
    const auto marks = view_.value("punctuation_marks", Json::object());
    if (!marks.is_object()) return {};
    return msime::linux_host::punctuation_marks_from_view(
        marks.value("quotes", std::string{}), marks.value("brackets", std::string{}),
        marks.value("enumeration_comma_backslash", true), marks.value("enumeration_comma_slash", false));
  }
  // The session types Korean: jamo compose in the preedit, punctuation is always half-width ASCII and none of the Chinese punctuation helpers apply. The dedicated English mode keeps its own rules in every scheme.
  bool korean() const {
    return msime::linux_host::strict_json_value(view_, "scheme", 0u) == 4 && !view_.value("dedicated_english", false);
  }
  // The view's scheme number outside the dedicated English mode, which keeps its own rules in every scheme; -1 there.
  int typingScheme() const {
    return view_.value("dedicated_english", false) ? -1 : msime::linux_host::view_scheme(view_);
  }
  // 韩文音节、注音转换、越南文单词或藏文音节串是用户已经写下的文字：离开它的按键把它写出去而不是丢弃（`commits_on_blur`），光标停在末尾，所以没有可编辑的分段（`locks_caret`）。
  bool commitsOnBlur() const {
    const int scheme = typingScheme();
    return scheme >= 0 && msime::linux_host::scheme::CommitsOnBlur(scheme);
  }
  // 韩文、越南文和藏文写半角 ASCII 标点，从不变成全角（`widens_full_width`）。
  bool narrowScheme() const {
    const int scheme = typingScheme();
    return scheme == msime::linux_host::scheme::Korean || scheme == msime::linux_host::scheme::Vietnamese ||
           scheme == msime::linux_host::scheme::Tibetan;
  }
  // 韩文、注音、越南文和藏文的标点取自 Engine，不用宿主的成对标点和智能标点辅助（`host_smart_punctuation`）。日文保留这些辅助，本宿主一直如此。
  bool withoutHostPunctuation() const {
    const int scheme = typingScheme();
    return scheme == msime::linux_host::scheme::Korean || scheme == msime::linux_host::scheme::Zhuyin ||
           scheme == msime::linux_host::scheme::Vietnamese || scheme == msime::linux_host::scheme::Tibetan;
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
    // 韩文、越南文和藏文的标点总是 ASCII，所以 ".." 仍是 ".."；注音的标点只取自 Engine。
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
    eraseSmartPunctuationShadowLast();
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
    std::optional<size_t> surroundingLength;
    const auto &surrounding = ic_.surroundingText();
    const bool surroundingPublished =
        !privateInput() &&
        ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) &&
        surrounding.isValid();
    if (surroundingPublished && surrounding.cursor() == surrounding.anchor()) {
      const auto &text = surrounding.text();
      const auto length = fcitx::utf8::lengthValidated(text);
      if (length != fcitx::utf8::INVALID_LENGTH) {
        surroundingLength = length;
        if (surrounding.cursor() > 0 && surrounding.cursor() <= length)
          preceding = fcitx::utf8::getChar(
              fcitx::utf8::nextNChar(text.begin(), surrounding.cursor() - 1), text.end());
      }
    }
    // 有效 surrounding 即使光标在文档开头、位于选择中或光标位置越界，也代表宿主文档的权威状态；只有宿主没有发布可用 surrounding 时才回退本会话影子。
    if (!surroundingPublished) {
      if (const auto shadow = smartPunctuationShadowLast())
        preceding = *shadow;
    }
    // Engine is about to commit the Chinese mark for this key. Record what the
    // caret follows now, while the document still predates the commit; a Space
    // arriving next re-reads both characters before rewriting either.
    const auto ascii = static_cast<char>(value);
    const bool arm = smart_punctuation_ && smart_punctuation_space_convert_ &&
                     chinese_punctuation_ &&
                     msime::linux_host::is_space_conversion_key(ascii, viewPunctuationMarks()) &&
                     !(paired_punctuation_ &&
                       msime::linux_host::is_auto_paired_opening_key(ascii)) &&
                     !composingOrCandidates();
    std::string armedPreceding;
    if (arm) {
      if (const auto characters = precedingCharacters(1, surroundingLength);
          characters && !characters->empty())
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
                           msime::linux_host::strict_json_value(view_, "scheme", 0u) != 3 && !withoutHostPunctuation();
    const bool handled =
        apply(msime_client_punctuation_with_context(session_, value, preceding),
              std::move(spaceConvertPreceding), pairMode, ascii);
    if (handled && smart_punctuation_rejected_ == ascii)
      forgetSmartPunctuationRepeat();
    if (!handled && keptAscii) {
      last_smart_punctuation_ = ascii;
      last_smart_punctuation_at_ = std::chrono::steady_clock::now();
    }
    return handled;
  }
  // 逻辑光标右侧的字符，计入尚未转发的左右移；宿主信息不可用时返回 nullopt，文档末尾返回空串。
  std::optional<std::string> followingCharacter() {
    const auto &surrounding = ic_.surroundingText();
    if (privateInput() || !ic_.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) ||
        !surrounding.isValid() || surrounding.cursor() != surrounding.anchor())
      return std::nullopt;
    const auto &text = surrounding.text();
    const auto length = fcitx::utf8::lengthValidated(text);
    auto cursor = static_cast<int64_t>(surrounding.cursor());
    for (const auto sym : pending_caret_) cursor += sym == FcitxKey_Left ? -1 : 1;
    if (length == fcitx::utf8::INVALID_LENGTH || cursor < 0 || static_cast<size_t>(cursor) > length)
      return std::nullopt;
    if (static_cast<size_t>(cursor) == length) return std::string{};
    const auto start = fcitx::utf8::nextNChar(text.begin(), static_cast<size_t>(cursor));
    return std::string(start, fcitx::utf8::nextChar(start));
  }
  std::optional<bool> caretShiftHeld() const;
  void forwardCaret(fcitx::KeySym sym) {
    forgetSmartPunctuationShadow();
    // Wayland 虚拟键盘沿用物理修饰状态；Shift 尚未松开时不能发送方向键，否则会选中文字。
    if (std::string_view(ic_.frontend()).find("wayland") == 0 && caretShiftHeld().value_or(caret_shift_)) {
      pending_caret_.push_back(sym);
#ifdef MSIME_FCITX5_XKB_STATE_MASK
      if (!caret_forward_event_) {
        caret_forward_event_ = loop_->addTimeEvent(CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC), 0,
            [this](fcitx::EventSourceTime *timer, uint64_t) {
              if (!ic_.hasFocus() || restricted() || privateInput()) {
                pending_caret_.clear();
              } else if (const auto shift = caretShiftHeld()) {
                if (*shift) {
                  // 松开按键与 modifiers 更新会跨轮到达；只轮询状态，不按固定延时盲发。
                  timer->setNextInterval(1000);
                  timer->setOneShot();
                } else {
                  flushPendingCaret();
                }
              }
              // 无可回读状态时停止轮询，交给下一次无 Shift 按键处理。
              return true;
            });
      } else {
        caret_forward_event_->setNextInterval(1000);
      }
      caret_forward_event_->setOneShot();
#endif
      return;
    }
    ic_.forwardKey(fcitx::Key(sym), false);
    ic_.forwardKey(fcitx::Key(sym), true);
  }
  void flushPendingCaret() {
    const auto pending = std::exchange(pending_caret_, {});
    for (const auto sym : pending) forwardCaret(sym);
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
    const auto closing =
        msime::linux_host::paired_closing_for_key(typed[0], fullwidthOutput(), paired_tracker_,
                                                  viewPunctuationMarks().corner_quotes);
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
  // 方括号、圆括号、书名号、`{` 和引号都补上后半个，再把光标退回两半之间，与 Windows 宿主一致。`{` 和 IBus 宿主一样走引擎的中文标点表：中文标点下出「、补」（契约第 5 版）；引擎没有处理时（专用英文空闲）仍补花括号。
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
    bool handled = punctuation(value, mode);
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
    const auto convertKey = space_convert_key_;
    const auto expected = std::move(space_convert_preceding_);
    space_convert_mark_.clear();
    space_convert_key_ = 0;
    space_convert_preceding_.clear();
    if (mark.empty() || !smart_punctuation_ ||
        !smart_punctuation_space_convert_ || !chinese_punctuation_ ||
        composingOrCandidates())
      return false;
    const auto replacement = msime::linux_host::space_conversion_ascii_text(
        mark, convertKey, viewPunctuationMarks());
    const auto characters = precedingCharacters(2);
    if (replacement.empty() || !characters ||
        !msime::linux_host::space_conversion_matches_document(mark, expected,
                                                              *characters))
      return false;
    ic_.deleteSurroundingText(-1, 1);
    eraseSmartPunctuationShadowLast();
    commitText(replacement);
    return true;
  }
  void select(uint64_t session, uint64_t generation, size_t index) {
    if (translation_candidates_active_) {
      if (session_ == session && msime::linux_host::strict_json_value(view_, "generation", uint64_t{}) == generation)
        commitTranslationCandidate(index);
      return;
    }
    if (!ensure() || session_ != session || msime::linux_host::strict_json_value(view_, "generation", uint64_t{}) != generation) return;
    apply(msime_client_select(session_, generation, index));
  }
  bool translationCandidatesActive() const { return translation_candidates_active_; }
  void translationPage(uint32_t command) {
    if (!translation_candidates_active_) return;
    const auto pageSize = std::clamp(
        msime::linux_host::strict_json_value(translation_saved_view_, "page_size", size_t{9}), size_t{1}, size_t{9});
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
        msime::linux_host::strict_json_value(translation_saved_view_, "page_size", size_t{9}), size_t{1}, size_t{9});
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
                          {"generation", msime::linux_host::strict_json_value(
                                             overlay, "generation", uint64_t{})},
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
    // 译文是宿主自己写进文档的，丢弃组字时告诉生命周期核心组字是这样结束的，而不是被取消（对会话的作用同 MSIME_CANCEL）。
    command(MSIME_CANCEL_AFTER_HOST_COMMIT);
  }
  void render();
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  bool renderNativeCanvas();
  std::unique_ptr<msime::linux_host::NativeCanvasX11> native_canvas_;
  // 原生画布画得出来：X11、classicui、有焦点、客户端不自己画面板。
  bool nativeCanvasDrawable() const;
  // 原生画布正按 macOS 的结构画（水杉主题或非 canvas 皮肤包），首行的展开键和网格只在这时提供。
  bool nativeChromeActive() const;
  // ---- 展开候选：两种样式共用一份状态 ----
  // 横排候选窗可以展开成这一代的全部候选，样式跟着原生画布的画法走：
  // - 网格（chrome，macOS 的「展开候选」，#7307）：`expand_candidates` 打开、不止一页时，↓ 或 Tab（或点首行的展开键）把全部候选排成多行，一行是每页候选数，一次最多看见 kExpandedGridVisibleRows 行（或皮肤的 expand_rows 加 1），高亮所在的行始终可见。网格开着时它接管键盘：←/→ 在行内移动，↑/↓ 换行并保持列，翻页键和 Tab 一次滚过可见的几行，空格和回车选高亮的那一个，数字键选高亮那一行里的第几个，Esc 或第一行再按 ↑ 收起网格而组字保留；它不认的键先收起网格，再照常处理。组字变了就收起。
  // - 卷轴（canvas 皮肤包的 expandPanel，Windows 的卷轴）：横排右侧固定的展开／收起键，首行仍是当前页，下面排完整列表里当前页之后的候选，画布自己按像素滚动（滚轮、拖滑块、点轨道）。不接管键盘，继续输入时留着并按新的一代重载，滚动随换代、翻页回到顶部。
  // 两种样式共用：完整候选经 expandedEntries 复制一次（all_candidates 可能推进代次，复制后重读当前页），选择用 msime_client_select_any_candidate 带着复制时的全局身份与一代（selectExpanded），组字没变或卷轴继续输入时候选换了一代就按新的一代重载；失焦、上屏、关掉面板、换皮肤、换竖排、进释义页或打开候选列表时收起。只有 X11 上的原生画布画得出来，classicui 主题、Wayland 和 IBus 不提供。
  enum class ExpandStyle { none, grid, scroll };
  ExpandStyle expandStyle() const;
  std::vector<Json> expanded_;
  ExpandStyle expanded_style_ = ExpandStyle::none;
  // 展开时的皮肤资产：换了皮肤（资产重建）就收起。
  std::weak_ptr<msime::linux_host::NativeCanvasAssets> expanded_assets_;
  uint64_t expanded_session_ = 0, expanded_generation_ = 0;
  std::string expanded_composition_;
  size_t expanded_cursor_ = 0, expanded_first_row_ = 0;
  size_t expandedColumns() const {
    return std::max<size_t>(1, msime::linux_host::strict_json_value(view_, "page_size", size_t{1}));
  }
  // 网格一次看得见几行：皮肤包的共享版式写了 expand_rows（2–8，只数首行之后的网格）时是它加上当前页这一行，与 macOS 的 windowRows 相同；没有时是 kExpandedGridVisibleRows。卷轴的视口高度由画布按同一个 expand_rows 算。
  size_t expandedVisibleRows(size_t rows) const;
  // 这时候还能不能展开着（两种样式）。
  bool expandAllowed() const;
  // 网格能不能打开：上面的条件之外还要是网格样式、不止一页。
  bool expandOffered() const;
  std::vector<Json> expandedEntries();
  bool openExpanded();
  bool expandedStillApplies();
  void discardExpanded() {
    expanded_.clear();
    expanded_style_ = ExpandStyle::none;
    expanded_assets_.reset();
    expanded_session_ = expanded_generation_ = 0;
    expanded_composition_.clear();
    expanded_cursor_ = expanded_first_row_ = 0;
  }
  void closeExpanded() {
    if (expanded_.empty()) return;
    discardExpanded();
    render();
  }
  void selectExpanded(size_t index, bool pointer);
  void scrollExpanded(bool backwards);
  bool expandedKey(fcitx::KeyEvent &event);
#endif
  std::string candidateAux() const;
  bool removeCandidateSlot(size_t slot) {
    if (!ensure() || restricted() || privateInput() || !ic_.hasFocus()) return false;
    const auto candidates = view_.value("candidates", Json::array());
    if (!candidates.is_array() || slot >= candidates.size()) return false;
    const auto &candidate = candidates.at(slot);
    if (!candidate.is_object() ||
        !msime::linux_host::candidate_dictionary_removal_available(
            msime::linux_host::strict_json_value(view_, "scheme", 0u), msime::linux_host::strict_json_value(candidate, "source", uint64_t{}),
            candidate.value("text", std::string{}))) return false;
    const auto &id = candidate.value("id", Json::object());
    if (!id.is_object() || !id.contains("generation") || !id.contains("index")) return false;
    return apply(msime_client_remove_candidate(
        session_, msime::linux_host::strict_json_required_integer<uint64_t>(id.at("generation")),
        msime::linux_host::strict_json_required_integer<size_t>(id.at("index"))));
  }
  bool resetCache() {
    if (!ensure() || restricted() || privateInput() || !ic_.hasFocus()) return false;
    return apply(msime_client_reset_cache(session_));
  }
  // 繁体输出转换只用于简体中文（`script_conversion_applies`）：日文（假名和 Engine 选的汉字）与韩文（谚文和用户选的汉字）原样通过，粤拼和注音本来就写繁体字，笔画候选按 msime-stroke.db 里存的字形原样取用，越南文和藏文不是中文。候选行、上屏和状态区动作都问这同一道关口，所以候选行显示的字永远就是它上屏的字（s2t 会把汉字 后 画成 後）。
  bool scriptConversionApplies() const {
    return msime::linux_host::scheme::ScriptConversionApplies(msime::linux_host::strict_json_value(view_, "scheme", 0));
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
  // 认的是 Caps_Lock 这个符号：被 xkb 选项改成 Control 或 Escape 的 Caps Lock 键不算，被改成 Caps_Lock 的 Shift 键（见上，X11 键码 50、62）也不算。
  static bool isCapsLockKey(const fcitx::KeyEvent &event) {
    const auto code = event.key().code();
    return event.key().sym() == FcitxKey_Caps_Lock && code != 50 && code != 62;
  }
  // 单按 Caps Lock 切换中英文。按下时布防（按下事件带的是按下前的锁定状态），别的键按下取消，松开时认定为轻点就先撤回大写锁定、成功后切换。返回 true 表示这个按键到此为止、交给应用：切换了，或者是布防了的按下。fcitx 不把 Caps_Lock 算作修饰键，组字时按下它会照普通按键先把首选上屏，那样轻点切换上屏的就是候选而不是原文，和单按 Shift、IBus 宿主都不一样，所以布防了的按下不再往下走，组字留到松开时由切换上屏原文。
  bool observeCapsLockTap(const fcitx::KeyEvent &event) {
    const auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
                         std::chrono::steady_clock::now().time_since_epoch())
                         .count();
    if (!isCapsLockKey(event)) {
      if (!event.isRelease()) caps_lock_tap_.interrupt();
      return false;
    }
    if (!event.isRelease()) {
      if (caps_lock_down_) return caps_lock_tap_.armed();
      caps_lock_down_ = true;
      const auto raw = event.rawKey().states();
      const bool enabled = mode_caps_lock_enabled_ && ic_.hasFocus() && !restricted() &&
                           !privateInput() && msime::linux_host::caps_lock_release_available();
      const bool others = raw.testAny(fcitx::KeyStates{fcitx::KeyState::Shift, fcitx::KeyState::Ctrl,
                                                      fcitx::KeyState::Alt, fcitx::KeyState::Super,
                                                      fcitx::KeyState::Hyper});
      caps_lock_tap_.press(now, enabled, raw.test(fcitx::KeyState::CapsLock), others);
      return caps_lock_tap_.armed();
    }
    caps_lock_down_ = false;
    // 撤不回大写锁定就不切换：切到英文却留着大写锁定，比什么都不做更糟。
    if (!caps_lock_tap_.release(now) || !msime::linux_host::release_caps_lock()) return false;
    noteCapsLock(false);
    return ensure() && toggleInputMode();
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
  // 背景音乐只在本输入法获得焦点、没有录音时可以播放。这里只告诉生命周期核心本会话是否激活（msime_client_set_active），变了才说；核心记着哪个会话是音乐的所有者，再按字段属性决定：Sensitive 字段（报成 SECURE）里不放，只带 NoSpellCheck 的普通字段照常放。releaseFocusResources() 在离开输入框和会话销毁前告诉核心本会话停用。
  void syncMusic() {
    music_.sync(session_, ic_.hasFocus() && !restricted() && !voice_loading_, msime_client_set_active);
  }
  // 一次按下交给生命周期核心：每个打字键，只要中文输入开着、没有在录音，不论之后是引擎还是应用拿走这个键。先比较并报告字段属性，再调 msime_client_key，都在 key() 处理这个键之前，这是顺序契约（msime_client.h），按键音因此排在这个键带来的上屏音前面；Sensitive 字段里核心不出声也不画。松开、单独的修饰键和快捷键不算打字（key_press_sounds），不开新的一轮。录音中按 Esc 取消录音的那个键也不出声：这里读到的是 key() 处理前的状态，录音还在。这里跑在 fcitx5 守护进程里，所以只投递请求：Host API 在发现某个声音打开之前不开音频设备、不起线程，音频出错时只在 stderr 写一行并在本进程关掉声音，不会落到插件里。打字特效的连击数现在就记下，key() 里画出的辅助行用的就是它。
  void keyDown(const fcitx::KeyEvent &event) {
    const auto sym = static_cast<std::uint32_t>(event.key().sym());
    // 按硬件键码认自动重复：修饰键先松开时松开事件的 keysym 和按下时不同（见 KeyRepeat）。
    const auto keycode = static_cast<std::uint32_t>(event.rawKey().code());
    if (event.isRelease()) key_repeat_.release(keycode);
    if (!session_ || !input_enabled_ || !ic_.hasFocus() || restricted() || voice_loading_)
      return;
    const bool shortcut = event.rawKey().states().testAny(fcitx::KeyStates{
        fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Super,
        fcitx::KeyState::Hyper, fcitx::KeyState::Meta});
    if (!msime::linux_host::key_press_sounds(event.isRelease(), event.key().isModifier(), shortcut))
      return;
    const bool repeat = key_repeat_.press(keycode);
    syncFieldTraits();
    // 自动重复的按下不出按键音、不画特效、不动连击，与 macOS 在调用处用 `!event.isARepeat` 挡掉自动重复相同，按住退格或空格时只在第一次按下响一声；但仍开新的一轮，否则按住期间的每次派发共用第一次按下那一轮的脚本 fuel（见 lifecycle_silent_turn）。
    if (repeat) {
      msime_client_key(session_, msime::linux_host::lifecycle_silent_turn());
      return;
    }
    // Linux 只显示打字特效里的连击数，在候选下方的辅助行（setAuxDown；setAuxUp 归语音、表情搜索和配置提示）。
    typing_combo_ = msime::linux_host::typing_effect_combo(msime_client_key(
        session_, msime::linux_host::lifecycle_key(msime::linux_host::key_sound_class(sym), false)));
  }
  // key() 处理完这个键之后：焦点、录音这些状态已经定下来，同步背景音乐。连击数变了就单独重画辅助行，这个键交给了应用、没有经过 render() 时也一样。
  void keySettled(std::uint32_t comboBefore) {
    syncMusic();
    if (typing_combo_ == comboBefore) return;
    if (view_.is_object() && view_.contains("candidates") && !view_.at("candidates").empty()) {
      ic_.inputPanel().setAuxDown(fcitx::Text(candidateAux()));
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
  }
  // 背景音乐经 msime_client_set_active 最近一次告诉核心的激活状态，见 syncMusic。
  msime::linux_host::MusicActivity music_;
  // 生命周期核心现在知道的字段属性，变了才重新报告，见 syncFieldTraits。
  msime::linux_host::FieldTraitsReport field_traits_;
  // 生命周期核心最近一次报告的连击数（msime_client_key 的返回值或派发回复的 feedback），以及按住的那个键：它的自动重复只开新的一轮、不出声不画，见 keyDown。
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
        msime::host_api::discard_string(raw);
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
  // 这一段词库忙碌已经记过日志（ensure）；会话建起来后清掉。
  bool dictionary_busy_noted_ = false;
  std::string resources_;
  std::optional<std::string> scheme_override_;
  // The language dictionaries the runtime options named when this context last read them, which decide whether Cantonese, Zhuyin and Stroke can run (noteSchemeOptions).
  msime::linux_host::LanguageDictionaryAvailability scheme_dictionaries_;
  bool caps_lock_ = false;
  // The lock the session was last told; see syncSessionCapsLock(). A new session starts unlocked.
  bool session_caps_lock_ = false;
  std::string mode_indicator_label_;
  // 悬浮工具栏和键位图的颜色，连同算它们时的偏好、系统明暗和皮肤目录；输入没变就不重新解析主题（解析要经 host-api，按键时不做）。
  struct ResolvedPanelColors {
    Json preferences;
    bool dark = false;
    Json catalog;
    bool resolved = false;
    msime::linux_host::FloatingToolbarColors toolbar{};
    msime::linux_host::ShuangpinKeymapColors keymap{};
    // 输入变了就记下新的输入并回答 true，调用方随即重算。
    bool stale(const Json &next_preferences, bool next_dark, const Json &next_catalog) {
      if (resolved && dark == next_dark && preferences == next_preferences && catalog == next_catalog) return false;
      preferences = next_preferences;
      dark = next_dark;
      catalog = next_catalog;
      resolved = true;
      return true;
    }
  };
  ResolvedPanelColors floating_colors_;
  ResolvedPanelColors keymap_colors_;
  std::optional<std::string> shuangpin_profile_override_;
  std::optional<std::string> helpcode_schema_override_;
  // Set while the matching override's own status-bar save has not reached the store.
  bool scheme_unsaved_ = false;
  bool shuangpin_profile_unsaved_ = false;
  bool helpcode_schema_unsaved_ = false;
  CandidateSkinCatalog candidate_skin_catalog_;
  // The catalogue as runtime-options.json carries it, palettes included: an installed skin's colours are read from here when the classic UI theme is built.
  Json candidate_skin_document_;
  std::optional<std::chrono::steady_clock::time_point> candidate_theme_refresh_at_;
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
  // 偏好 `traditional_chinese_variant` 的编号（0 s2t、1 s2tw、2 s2twp）。
  uint32_t traditional_variant_ = 0;
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
  bool caret_shift_ = false;
  std::vector<fcitx::KeySym> pending_caret_;
  std::unique_ptr<fcitx::EventSourceTime> caret_forward_event_;
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
  std::string smart_punctuation_shadow_;
  bool smart_punctuation_shadow_valid_ = false;
  // The ASCII key whose Chinese mark Engine just committed with nothing
  // composing, and the character that stood in front of it at that moment. A
  // bare Space arriving next takes the mark back to ASCII; any other key
  // disarms. The fingerprint is there because the same mark usually appears more
  // than once and moving the caret inside a window is not a focus change.
  std::string space_convert_mark_;
  // `、` 上屏时产生它的键：顿号有两个可配键，空格反查按它和 View 的顿号开关定 ASCII 孪生。
  char space_convert_key_ = 0;
  std::string space_convert_preceding_;
  uint8_t punctuation_lock_ = 0;
  std::string online_socket_, online_query_;
  std::array<std::string, 2> online_dispatched_queries_;
  uint64_t online_job_session_ = 0;
  struct OnlineSlot {
    std::shared_future<Json> job;
    std::string query;
    uint64_t epoch = 0;
    // 只查缓存的 AI 探测：它落空只是还没到网络请求，不算 AI 没有回答。
    bool cache_only = false;
  } online_slots_[2];
  // 不会向 AI 发问时（没有在线服务、私密输入框）最近一次告诉 Host API 没有 AI 回答的查询，避免每个定时周期都再说一遍。
  std::string ai_resolved_query_;
  uint64_t online_epoch_ = 0;
  std::chrono::steady_clock::time_point online_due_{};
  std::chrono::steady_clock::time_point ai_due_{};
  std::string translation_query_, translation_pending_, translation_socket_;
  // 「仅当前高亮」时主查询的回答，高亮移开再移回来时直接交给 Rust（CandidateTranslationScope.h）。
  msime::linux_host::TranslationAnswerCache translation_answers_;
  // 翻译工作线程查到的读音（CandidatePronunciation.h），按发给读音表的文本存；换焦点、换偏好时随 invalidateTranslationRequests 一起清掉。
  msime::linux_host::CandidatePronunciations pronunciations_;
  bool translation_manual_sentence_ = false;
  std::chrono::steady_clock::time_point translation_due_{};
  uint64_t translation_session_ = 0;
  uint64_t translation_epoch_ = 0;
  uint64_t translation_job_epoch_ = 0;
  std::shared_future<Json> translation_job_;
  std::string clipboard_path_;
  Json clipboard_items_ = Json::array();
  uint64_t clipboard_generation_ = 0;
  bool clipboard_loading_ = false;
  std::shared_future<Json> clipboard_job_;
  std::shared_future<Json> clipboard_mutation_job_;
  std::string cloud_clipboard_socket_;
  std::string cloud_clipboard_explicit_socket_;
  std::string cloud_clipboard_directory_;
  Json cloud_clipboard_relay_ = Json::object();
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
  // 「桌面工具」子菜单里「背单词（今日待复习 N）…」的文字，获得焦点和失焦留给托盘时各读一次（FcitxEngine::refreshVocabularyDue），没有这一行时为空。
  std::optional<std::string> vocabulary_due_;
  bool voice_hotkey_ctrl_f9_ = true;
  msime::host_api::ToggleHotkeyChord voice_toggle_chord_;
  unsigned voice_toggle_held_ = 0;
  bool voice_hotkey_ralt_ = true;
  bool voice_hotkey_ctrl_win_ = false;
  bool voice_hotkey_rctrl_ralt_ = false;
  // 空格锁定固定为开，不读存下的 `hotkey_hold_space_lock`：空格只在按住录音组合键期间才被拦下，平常打字不受影响，设置页也不再给这个开关（`voice_space_lock_switch`）。
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
  std::string voice_failure_message_;
  std::unique_ptr<fcitx::EventSourceTime> voice_failure_timer_;
  // Which context starts in Chinese, and which of the four configurable mode
  // chords this host answers. Fcitx5 had the CN/EN toggle on its status area
  // only: the settings page showed all four switches for this platform and none
  // of them did anything here, and a user who chose to start in English got
  // Chinese anyway.
  bool input_enabled_ = true;
  // 当前模式来自这次聚焦的应用例外，用户还没有手动切换：全局作用域下不把它写进全局状态（和 IBus 宿主的 `app_rule_active` 一样按上下文记）。
  bool app_rule_active_ = false;
  bool mode_shift_enabled_ = true;
  bool mode_ctrl_enabled_ = false;
  bool mode_ctrl_space_enabled_ = true;
  bool mode_ctrl_alt_space_enabled_ = true;
  // 单按 Caps Lock 切换中英文（缺省关）、判定状态，以及 Caps Lock 是否按着（自动重复的按下不重新计时）。
  bool mode_caps_lock_enabled_ = false;
  msime::input::CapsLockTap caps_lock_tap_;
  bool caps_lock_down_ = false;
  bool character_set_shortcut_enabled_ = true;
  // Ctrl+Shift+J 在中文方案和日语之间切换（keybindings.toggle_japanese_ctrl_shift_j），缺省关。
  bool japanese_toggle_enabled_ = false;
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
  // 获得焦点时的光标徽章正在等光标矩形（见 settleFocusInputModeHud）。徽章表面本身属于 FcitxEngine，整个插件共用一块。
  bool mode_hud_after_focus_ = false;
  std::unique_ptr<fcitx::EventSourceTime> mode_hud_focus_timer_;
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
  std::string voice_phase_ = msime::linux_host::kVoiceStatusRecording;
  uint8_t voice_level_ = 0;
  msime::linux_host::WaveOverlayModel wave_overlay_;
  std::unique_ptr<msime::linux_host::WaveOverlaySurface> wave_overlay_surface_;
  bool wave_overlay_visible_ = false;
  bool wave_overlay_failed_ = false;
  // 识别进行中点了确认：这次录音余下的时间里浮条一直收着。
  bool wave_overlay_dismissed_ = false;
  // 声明在浮层之后，这样它们会在浮层关闭显示连接之前销毁。
  std::unique_ptr<fcitx::EventSourceTime> wave_overlay_frame_timer_;
  std::unique_ptr<fcitx::EventSourceIO> wave_overlay_io_;
  int wave_overlay_io_fd_ = -1;
  bool system_dark_ = false;
  bool word_character_enabled_ = true;
  bool word_character_minus_equal_ = false;
  bool translation_candidates_active_ = false;
  Json translation_saved_view_ = Json::object();
  std::vector<std::string> translation_options_;
  size_t translation_page_ = 0;
  size_t translation_cursor_ = 0;
};

// classicui 的文字和原生画布的分段来自同一份候选快照。
struct FcitxCandidateDisplay {
  fcitx::Text classic;
  std::string body, translation, suffix;
  Json translation_runs = Json::array();
  // 释义各行的读音（"\n" 分行，与 translation 逐行对应，CandidatePronunciation.h）；classicui 已经把它拼在每行释义下面，原生画布另外画淡。
  std::string pronunciation;
  // 固定了排位（1–5）：原生画布照 macOS 用 #379AD3 画正文，classicui 画不出单行颜色，改在正文后加「  固定N」。
  bool fixed = false;
  // 原生画布的 macOS 结构分开画的几段：不带训音的正文、韩文汉字的训音（画在释义行第一行）、收拾过的插件注释（释义行最后一行，斜体），以及 AI 占位行。
  std::string word, reading, script_annotation;
  bool pending = false;
  std::optional<std::string> decoration_text;
  std::string decoration_badge;
  // 光标后编辑里首选开头固定的那几个字占 word 和 body 开头多少字节，原生画布把它们画淡。classicui 的 fcitx::Text 只有下划线、高亮、粗体、斜体这些格式，没有颜色，斜体在这里又表示「次要、不上屏」（AI 占位行、训音），而这几个字是要上屏的，所以 classic 照常画。
  std::size_t dim_prefix_bytes = 0;
  // 「仅当前高亮」时高亮这一行（视图的 translation_reserved）一直留出的释义行数，回答还没到也留，移动高亮时候选窗高度不跳；其他行为 0。
  int reserved_gloss_lines = 0;
};
// `variant` 是繁体变体的编号，只在 `traditional` 为真时用到；`dimScalars` 是光标后编辑要画淡的字数（caret_edit_dim_scalars），不是首选时为 0。`reservedGlossLines` 是候选窗为还没到的释义预留的行数（FcitxState::translationReservedLines），只给视图标了 translation_reserved 的那一行。`pronunciation` 是释义各行的读音（FcitxState::candidatePronunciation），没有打开「显示读音」时为空。
FcitxCandidateDisplay candidateDisplay(const Json &candidate, bool traditional, bool annotations,
                                     const std::string &hanjaGloss, uint32_t variant, std::size_t dimScalars = 0,
                                     int reservedGlossLines = 0, const std::string &pronunciation = {}) {
  FcitxCandidateDisplay display;
  // Engine 候选来源 12 是 AI 占位行（「…」或「—」）：Fcitx5 的文本格式没有颜色，用斜体表示它是次要的，并标明不上屏（#6888）。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。
  if (msime::linux_host::strict_json_value(candidate, "source", uint64_t{}) == 12) {
    display.body = display.word = candidate.at("text").get<std::string>();
    display.pending = true;
    display.classic = fcitx::Text(display.body,
        fcitx::TextFormatFlags{fcitx::TextFormatFlag::Italic, fcitx::TextFormatFlag::DontCommit});
    return display;
  }
  display.decoration_text = traditional && candidate.value("source", 0) != 18 ? msime_linux_simplified_to_traditional(candidate.at("text").get<std::string>(), variant)
                                       : candidate.at("text").get<std::string>();
  const auto source = msime::linux_host::strict_json_value(candidate, "source", uint64_t{});
  display.decoration_badge = source == 2 ? "☁️" : source == 3 ? "🤖" : "";
  // 按转换以后的文字算字节：word 和 body 都以它开头。
  display.dim_prefix_bytes = msime::linux_host::utf8_scalar_prefix_bytes(*display.decoration_text, dimScalars);
  fcitx::Text row(*display.decoration_text +
      // 纠错标记只属于普通显示行；装饰词保留完成简繁转换的原文，选择仍沿用候选身份。
      (candidate.value("corrected", false) ? "*" : "") +
      // 注释括起来紧贴候选字，角标隔一个空格跟在后面，与 macOS 候选窗的写法相同（CandidateRowText.h）。
      (!hanjaGloss.empty() || !annotations ? std::string() :
       msime::linux_host::candidate_annotation_text(candidate.value("annotation", std::string()))) +
      std::string(msime::linux_host::candidate_source_badge(
          msime::linux_host::strict_json_value(candidate, "source", uint64_t{}))));
  display.word = row.toString();
  display.reading = hanjaGloss;
  // 训音不受翻译开关影响，紧跟汉字并以斜体标作次要释义；它只供显示，选词仍按候选身份。
  if (!hanjaGloss.empty())
    row.append("  " + hanjaGloss, fcitx::TextFormatFlags{fcitx::TextFormatFlag::Italic,
                                                         fcitx::TextFormatFlag::DontCommit});
  display.body = row.toString();
  const auto fixedMarker = msime::linux_host::candidate_fixed_position_marker(
      msime::linux_host::strict_json_value(candidate, "fixed_position", uint64_t{}));
  display.fixed = !fixedMarker.empty();
  if (display.fixed) row.append(fixedMarker, fcitx::TextFormatFlags{fcitx::TextFormatFlag::DontCommit});
  if (candidate.contains("translation") && candidate.at("translation").is_string()) {
    display.translation = candidate.at("translation").get<std::string>();
    display.translation_runs = candidate.value("translation_runs", Json::array());
    if (!display.translation.empty()) display.pronunciation = pronunciation;
    // classicui 按换行测量整项高度与点击区域；原生画布仍消费未加工的独立译文。读音在它那一行释义下面单独一行，只供显示。
    if (!display.translation.empty())
      row.append("\n" + msime::linux_host::candidate_gloss_readings(display.translation, display.pronunciation).text);
  }
  if (reservedGlossLines > 0 && candidate.value("translation_reserved", false)) {
    display.reserved_gloss_lines = reservedGlossLines;
    // classicui 按有没有译文变高、没有统一预留：回答还没到时用不上屏的全角空格行占住译文的位置。
    if (display.translation.empty()) {
      std::string placeholder;
      for (int line = 0; line < reservedGlossLines; ++line) placeholder += "\n\u3000";
      row.append(placeholder, fcitx::TextFormatFlags{fcitx::TextFormatFlag::DontCommit});
    }
  }
  // 脚本插件的注释排在整行最后，单独一段：不受辅助码设置影响，不做繁体转换（那是插件自己写的文字），只供显示，所以和训音一样标 DontCommit。
  const auto scriptAnnotation = msime::linux_host::candidate_script_annotation(candidate);
  if (!scriptAnnotation.empty()) {
    display.script_annotation = scriptAnnotation;
    display.suffix = msime::linux_host::candidate_script_annotation_suffix(scriptAnnotation);
    row.append(display.suffix,
               fcitx::TextFormatFlags{fcitx::TextFormatFlag::DontCommit});
  }
  display.classic = std::move(row);
  return display;
}

class FcitxCandidate : public fcitx::CandidateWord {
public:
  FcitxCandidate(fcitx::FactoryFor<FcitxState> *factory, const Json &candidate, bool traditional,
                 bool annotations, const std::string &hanjaGloss, uint32_t variant = 0, std::size_t dimScalars = 0,
                 int reservedGlossLines = 0, const std::string &pronunciation = {})
      : FcitxCandidate(factory, candidate, candidateDisplay(candidate, traditional, annotations, hanjaGloss, variant, dimScalars,
                                                            reservedGlossLines, pronunciation)) {}
  const FcitxCandidateDisplay &display() const { return display_; }
private:
  FcitxCandidate(fcitx::FactoryFor<FcitxState> *factory, const Json &candidate, FcitxCandidateDisplay display)
      : CandidateWord(std::move(display.classic)), factory_(factory),
        session_(msime::linux_host::strict_json_required_integer<uint64_t>(candidate.at("id").at("session"))),
        generation_(msime::linux_host::strict_json_required_integer<uint64_t>(candidate.at("id").at("generation"))),
        index_(msime::linux_host::strict_json_required_integer<size_t>(candidate.at("id").at("index"))),
        source_(msime::linux_host::strict_json_value(candidate, "source", uint64_t{})),
        text_(candidate.at("text").get<std::string>()),
        fixed_position_(msime::linux_host::strict_json_value(candidate, "fixed_position", uint8_t{})),
        display_(std::move(display)) {}
public:
  void select(fcitx::InputContext *ic) const override {
    try {
      auto *state = ic->propertyFor(factory_);
      state->pointerTurn();
      state->select(session_, generation_, index_);
    } catch (...) {}
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
  FcitxCandidateDisplay display_;
};

// 新旧 Fcitx5 共用同一份操作列表和身份校验；旧版没有 ActionableCandidateList。
std::vector<std::pair<int, std::string>> candidateMenuActions(FcitxState &state, const FcitxCandidate &item) {
  std::vector<std::pair<int, std::string>> actions;
  if (state.translationCandidatesActive() || !state.ic_.hasFocus() || !state.input_enabled_ ||
      state.restricted() || state.privateInput() || state.session_ != item.session() ||
      msime::linux_host::strict_json_value(state.view_, "generation", uint64_t{}) != item.generation()) return actions;
  const auto scheme = msime::linux_host::strict_json_value(state.view_, "scheme", 0u);
  if (msime::linux_host::candidate_dictionary_actions_available(scheme, item.source())) {
    // 顺序照 macOS 的「候选操作」菜单：置顶、固定排位（第 1–5 位、取消固定）、删除（CandidateActionPolicy.h）。
    actions.emplace_back(1, msime::linux_host::candidate_pin_label);
    for (int slot = 1; slot <= 5; ++slot)
      actions.emplace_back(10 + slot, msime::linux_host::candidate_fix_label(slot));
    if (item.fixedPosition() > 0) actions.emplace_back(20, msime::linux_host::candidate_clear_fix_label);
    if (msime::linux_host::candidate_dictionary_removal_available(scheme, item.source(), item.text()))
      actions.emplace_back(2, msime::linux_host::candidate_remove_label);
  }
  // 「朗读」「加入生词本」在最后，和 macOS、Windows 的候选菜单一样；都不是词库操作。「朗读」在有能读的英文、本机又装着 spd-say 时出现，「加入生词本」只要这一行显示着释义就有。
  if (msime::linux_host::read_aloud_available() &&
      msime::read_aloud::text_for(item.text(), item.display().translation))
    actions.emplace_back(31, msime::read_aloud::kMenuLabel);
  if (!item.display().translation.empty())
    actions.emplace_back(30, msime::linux_host::candidate_personal_wordbook_label);
  return actions;
}
void triggerCandidateMenuAction(FcitxState &state, const FcitxCandidate &item, int action) {
  // ensure() 可能替换当前页；先保留身份，再让运行时复核会话、代次和全局序号。
  const auto session = item.session();
  const auto generation = item.generation();
  const auto index = item.index();
  const auto text = item.text();
  const auto translation = item.display().translation;
  const auto actions = candidateMenuActions(state, item);
  if (std::none_of(actions.begin(), actions.end(),
      [action](const auto &available) { return available.first == action; })) return;
  try {
    if (!state.ensure() || state.session_ != session ||
        msime::linux_host::strict_json_value(state.view_, "generation", uint64_t{}) != generation) return;
    if (action == 31) {
      // 不经过 Engine，组字不变；起不来 spd-say 时把提示放在候选上方的辅助行。
      const auto spoken = msime::read_aloud::text_for(text, translation);
      if (spoken && !msime::linux_host::read_aloud(*spoken)) {
        state.ic_.inputPanel().setAuxUp(fcitx::Text(msime::linux_host::read_aloud_failure_notice));
        state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
      }
      return;
    }
    if (action == 30) {
      // 不经过 Engine，组字不变；共享层答的那句提示放在候选上方的辅助行，下一次刷新时消失。
      const auto notice = msime::linux_host::add_to_personal_wordbook(
          state.options_path_, text, translation);
      state.ic_.inputPanel().setAuxUp(fcitx::Text(notice));
      state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
      return;
    }
    char *raw = nullptr;
    if (action == 1) raw = msime_client_pin_candidate(session, generation, index);
    else if (action == 2) raw = msime_client_remove_candidate(session, generation, index);
    else if (action >= 11 && action <= 15)
      raw = msime_client_fix_candidate_position(session, generation, index, static_cast<uint8_t>(action - 10));
    else if (action == 20) raw = msime_client_clear_candidate_position(session, generation, index);
    if (raw) state.apply(raw);
  } catch (...) {}
}

// The runtime already pages candidates. Never page its current page a second time.
class FcitxPage : public fcitx::CandidateList,
                  public fcitx::PageableCandidateList
#ifdef MSIME_FCITX_ACTIONS
                  , public fcitx::ActionableCandidateList
#endif
{
public:
  FcitxPage(FcitxState &state, fcitx::FactoryFor<FcitxState> *factory) : state_(state),
      session_(state.session_),
      generation_(msime::linux_host::strict_json_required_integer<uint64_t>(state.view_.at("generation"))),
      page_(msime::linux_host::strict_json_required_integer<int>(state.view_.at("page"))),
      pages_(msime::linux_host::strict_json_required_integer<int>(state.view_.at("page_count"))),
      layout_(state.preferences_.value("candidate_layout", std::string("vertical")) == "horizontal"
          ? fcitx::CandidateLayoutHint::Horizontal : fcitx::CandidateLayoutHint::Vertical) {
    const bool annotations = state.showCandidateAnnotations();
    setPageable(this);
#ifdef MSIME_FCITX_ACTIONS
    setActionable(this);
#endif
    const auto &candidates = state.view_.at("candidates");
    const auto caretEdit = msime::linux_host::caret_edit_suffix(state.view_);
    const int reservedGlossLines = state.translationReservedLines();
    words_.reserve(candidates.size());
    labels_.reserve(candidates.size());
    for (const auto &candidate : candidates) {
      if (candidate.value("highlighted", false)) cursor_ = words_.size();
      words_.push_back(std::make_unique<FcitxCandidate>(
          factory, candidate, state.traditionalApplies(), annotations,
          msime::linux_host::korean_hanja_gloss(state.view_, candidate), state.traditional_variant_,
          msime::linux_host::caret_edit_dim_scalars(caretEdit, candidate), reservedGlossLines,
          state.candidatePronunciation(candidate)));
      // 序号与选它的数字键一致：第十个由 0 键选，标 0。macOS 的序号不带句点，按 0.8 倍字号画、与候选隔 1.5pt；classicui 序号与候选之间没有间距，留一个空格代替，字号由主题的 LabelTextSizeFactor 缩到 80%。
      labels_.emplace_back(msime::linux_host::candidate_index_label(words_.size() - 1) + " ");
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
        msime::linux_host::strict_json_value(state_.view_, "generation", uint64_t{}) != item->generation())
      return false;
    return msime::linux_host::candidate_dictionary_actions_available(msime::linux_host::strict_json_value(state_.view_, "scheme", 0u), item->source()) ||
           !item->display().translation.empty();
  }
  std::vector<fcitx::CandidateAction>
  candidateActions(const fcitx::CandidateWord &candidate) const override {
    std::vector<fcitx::CandidateAction> actions;
    const auto *item = dynamic_cast<const FcitxCandidate *>(&candidate);
    if (!item) return actions;
    for (const auto &[id, text] : candidateMenuActions(state_, *item)) {
      fcitx::CandidateAction action;
      action.setId(id); action.setText(text);
      actions.push_back(std::move(action));
    }
    return actions;
  }
  void triggerAction(const fcitx::CandidateWord &candidate, int action) override {
    if (const auto *item = dynamic_cast<const FcitxCandidate *>(&candidate))
      triggerCandidateMenuAction(state_, *item, action);
  }
#endif
private:
  void move(uint32_t command) {
    // render() replaces this list. Do not access members after dispatch.
    auto *state = &state_;
    state->pointerTurn();
    if (state->translationCandidatesActive()) {
      // A stale Fcitx candidate list must not page a newer translation overlay.
      // Rendering replaces this list, but Fcitx may still dispatch an already
      // queued pageable callback after the replacement.
      if (state->session_ != session_ ||
          msime::linux_host::strict_json_value(state->view_, "generation", uint64_t{}) != generation_)
        return;
      state->translationPage(command);
      return;
    }
    const auto session = session_;
    const auto generation = generation_;
    try {
      if (state->ensure() && state->session_ == session &&
          msime::linux_host::strict_json_value(state->view_, "generation", uint64_t{}) == generation)
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
// 图标：macOS 的菜单项没有图标。直接挂在状态区的动作保留图标，因为 kimpanel 把它们画成独立按钮，没有图标就只剩文字；只出现在子菜单里的动作返回空图标，托盘菜单（SNI/dbusmenu）不再每行都画同一个键盘图标。
class FcitxModeAction : public fcitx::Action {
public:
  enum class Mode { EnglishCandidates, Fullwidth };
  FcitxModeAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::EnglishCandidates ? msime::linux_host::menu_label::kEnglishCandidateMode
                                            : msime::linux_host::menu_label::kFullwidth;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string shortText(fcitx::InputContext *) const override { return msime::linux_host::menu_label::kChineseInput; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    return ic && ic->propertyFor(factory_)->session_ && ic->propertyFor(factory_)->input_enabled_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  // 这一行带上正在用的方案的名字，与 macOS 的「输入方案（双拼（小鹤））」相同（MenuLabels.h）。
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return msime::linux_host::menu_label::kScheme;
    const auto *state = ic->propertyFor(factory_);
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    return msime::linux_host::menu_heading(
        msime::linux_host::menu_label::kScheme,
        state->schemeMenuTitle(FcitxState::kSchemes[scheme < FcitxState::kSchemes.size() ? scheme : 0]));
  }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  // Front ends open the scheme menu instead of activating an action that has one; stepping stays for a caller that activates it directly.
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  // 名字与 macOS 子菜单相同：双拼带键位方案，五笔跟随存储的码表版本显示「五笔 86」或「五笔 98」。
  std::string shortText(fcitx::InputContext *ic) const override {
    if (ic) return ic->propertyFor(factory_)->schemeMenuTitle(FcitxState::kSchemes[index_]);
    return label_;
  }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && msime::linux_host::strict_json_value(state->view_, "scheme", 0u) == index_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget() || isChecked(ic)) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    return state->session_ && msime::linux_host::strict_json_value(state->view_, "scheme", 0u) == 0 &&
           state->view_.value("nine_key", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
    auto *state = ic->propertyFor(factory_);
    if (!state->session_ || state->restricted() || state->privateInput()) return;
    try {
      if (state->ensure() && msime::linux_host::strict_json_value(state->view_, "scheme", 0u) == 0) {
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    if (scheme != 0 && scheme != 1) return false;
    const auto section = scheme == 1 ? "shuangpin_helpcode" : "quanpin_helpcode";
    return state->preferences_.value(section, Json::object()).value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  enum class Kind { ShuangpinPreedit, WubiCodeHint, WubiAutoCommitUnique };
  FcitxSchemeBooleanAction(fcitx::FactoryFor<FcitxState> *factory, Kind kind)
      : factory_(factory), kind_(kind) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    switch (kind_) {
      case Kind::ShuangpinPreedit: return "双拼原始预编辑";
      case Kind::WubiCodeHint: return "五笔剩余编码";
      case Kind::WubiAutoCommitUnique: return "五笔四码唯一自动上屏";
    }
    return {};
  }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  /// 本项所属的方案，以及它在共享偏好文档里的键；三项都属于某个方案，只有该方案生效时才列出和切换。
  bool appliesTo(unsigned scheme) const {
    switch (kind_) {
      case Kind::ShuangpinPreedit: return scheme == 1;
      case Kind::WubiCodeHint:
      case Kind::WubiAutoCommitUnique: return scheme == 2;
    }
    return false;
  }
  const char *key() const {
    switch (kind_) {
      case Kind::ShuangpinPreedit: return "shuangpin_preedit_uses_raw";
      case Kind::WubiCodeHint: return "wubi_code_hint";
      case Kind::WubiAutoCommitUnique: return "wubi_auto_commit_unique";
    }
    return "";
  }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    if (!appliesTo(scheme)) return false;
    return state->preferences_.value(key(), true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
    auto *state = ic->propertyFor(factory_);
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    if (!appliesTo(scheme) || state->restricted() || state->privateInput()) return;
    try {
      if (state->toggleTopLevelBoolean(key(), true)) update(ic);
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
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    // 选了辅助码表包时写明插件在生效，而不是那个只作回退的内置方案。
    return msime::linux_host::helpcode_status_label(state->preferences_, scheme == 1 ? "shuangpin" : "quanpin");
  }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void setMenu(fcitx::Menu *menu) { fcitx::SimpleAction::setMenu(menu); }
  // Front ends open the schema menu instead of activating an action that has one; stepping stays for a caller that activates it directly.
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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

// One built-in schema in the 辅助码方案 menu, checked while the current scheme uses it. While a helpcode pack is active none is checked, as in the IBus menu, and choosing any of them turns the pack off.
class FcitxHelpcodeSchemaItemAction : public fcitx::Action {
public:
  FcitxHelpcodeSchemaItemAction(fcitx::FactoryFor<FcitxState> *factory, const char *schema, const char *label)
      : factory_(factory), schema_(schema), label_(label) {
    setCheckable(true);
  }
  std::string shortText(fcitx::InputContext *) const override { return label_; }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    const auto scheme = msime::linux_host::strict_json_value(state->view_, "scheme", 0u);
    if (scheme != 0 && scheme != 1) return false;
    const auto name = scheme == 1 ? "shuangpin" : "quanpin";
    if (!msime::linux_host::helpcode_pack(state->preferences_, name).empty()) return false;
    return state->preferences_.value(std::string(name) + "_helpcode", Json::object())
               .value("schema", std::string(msime::linux_host::default_helpcode_schema(name))) == schema_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget() || isChecked(ic)) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->selectHelpcodeSchema(schema_)) update(ic);
    } catch (...) {
      ic->propertyFor(factory_)->close();
      ic->propertyFor(factory_)->clearPanel();
    }
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  const char *schema_;
  const char *label_;
};

class FcitxAutocorrectAction : public fcitx::Action {
public:
  enum class Mode { Transposition, Neighbor };
  FcitxAutocorrectAction(fcitx::FactoryFor<FcitxState> *factory, Mode mode)
      : factory_(factory), mode_(mode) { setCheckable(true); }
  std::string shortText(fcitx::InputContext *) const override {
    return mode_ == Mode::Transposition ? "拼音错位纠错" : "拼音邻键纠错";
  }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_ || msime::linux_host::strict_json_value(state->view_, "scheme", 0u) != 0) return false;
    const auto key = mode_ == Mode::Transposition ? "autocorrect_transposition" : "autocorrect_neighbor";
    return state->preferences_.value("quanpin", Json::object()).value(key, true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string shortText(fcitx::InputContext *) const override { return msime::linux_host::menu_label::kMixedEnglish; }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("mixed_input", Json::object())
        .value("english", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("mixed_input", Json::object())
        .value(key_, mixedCandidateDefault(key_));
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("local_modes", Json::object())
        .value(key_, msime::linux_host::local_mode_enabled_by_default(key_));
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("candidate_english_gloss", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("word_character", Json::object())
        .value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("number_row_selection", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    return mode_ == Mode::Chinese ? msime::linux_host::menu_label::kChinesePunctuation : "成对标点";
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    if (!state->session_) return false;
    const char *key = mode_ == Mode::Smart ? "smart_punctuation" : "smart_punctuation_repeat";
    return state->preferences_.value(key, true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic) return msime::linux_host::menu_label::kTheme;
    const auto *state = ic->propertyFor(factory_);
    const auto choices = state->themeChoices();
    const auto *current = msime::linux_host::find_theme_choice(choices, state->currentThemeChoice());
    // 写法与 macOS 的「主题（水杉）」相同。
    return msime::linux_host::menu_heading(msime::linux_host::menu_label::kTheme, current ? current->title : std::string());
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
      if (msime::linux_host::strict_json_value(state->view_, "page_size", uint8_t{}) == size_)
        return std::to_string(size_) + " 个候选 ✓";
    }
    return std::to_string(size_) + " 个候选";
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("learning", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    return state->session_ && state->preferences_.value("candidate_translations", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("cloud_candidates", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("ai_assistant", Json::object())
        .value("enabled", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    setLongText("重置" MSIME_EDITION_DISPLAY_NAME "：关闭所有输入会话并重新读取运行配置");
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (!state || state->restricted() ||
          (std::strcmp(panel_, "voice") == 0 && !state->voice_enabled_)) return;
      launchDesktopPanel(panel_);
    } catch (...) {}
  }
private:
  const char *panel_;
  fcitx::FactoryFor<FcitxState> *factory_;
};

// 「背单词（今日待复习 N）…」：文字取这个上下文获得焦点时读到的计数，点下去打开共享应用的背单词面板。只在有卡在等、没有在组字时列进「桌面工具」子菜单（FcitxEngine::syncVocabularyEntry），不放状态区顶层。
class FcitxVocabularyAction : public fcitx::Action {
public:
  explicit FcitxVocabularyAction(fcitx::FactoryFor<FcitxState> *factory) : factory_(factory) {}
  std::string shortText(fcitx::InputContext *ic) const override {
    if (!ic) return {};
    const auto &label = ic->propertyFor(factory_)->vocabulary_due_;
    return label ? *label : std::string{};
  }
  std::string icon(fcitx::InputContext *) const override { return ""; }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
    try {
      if (ic->propertyFor(factory_)->restricted()) return;
      launchDesktopPanel(msime::vocabulary_due::kPanelRoute);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

// 工具栏子菜单里 macOS 齿轮菜单（FloatingToolbarPanel.mm）的两项：在光标处插入产品介绍，在浏览器里打开官网。
class FcitxUtilityAction : public fcitx::SimpleAction {
public:
  enum class Kind { Promotion, Website };
  FcitxUtilityAction(fcitx::FactoryFor<FcitxState> *factory, Kind kind) : factory_(factory), kind_(kind) {
    setShortText(kind == Kind::Promotion ? msime::linux_host::menu_label::kPromotion
                                         : msime::linux_host::menu_label::kWebsite);
  }
  void activate(fcitx::InputContext *ic) override {
    if (kind_ == Kind::Website) {
      fcitx::startProcess({"xdg-open", msime::linux_host::kWebsiteUrl});
      return;
    }
    // 介绍要写进有焦点的输入框，密码框里不写。
    if (!ic || !ic->hasFocus()) return;
    auto *state = ic->propertyFor(factory_);
    if (state->restricted()) return;
    try { state->commitPromotion(); } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
  Kind kind_;
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
  std::string shortText(fcitx::InputContext *) const override { return msime::linux_host::menu_label::kToolbarEnabled; }
  std::string icon(fcitx::InputContext *) const override { return "view-restore"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("floating_toolbar", Json::object())
        .value("enabled", true);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->voice_enabled_;
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
  std::string icon(fcitx::InputContext *) const override { return ""; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->preferences_.value("clipboard_history", false);
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
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
    setShortText(msime::linux_host::menu_label::kVoice);
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
  std::string shortText(fcitx::InputContext *) const override { return msime::linux_host::menu_label::kTraditionalOutput; }
  std::string icon(fcitx::InputContext *) const override { return "input-keyboard"; }
  bool isChecked(fcitx::InputContext *ic) const override {
    if (!ic) return false;
    const auto *state = ic->propertyFor(factory_);
    return state->session_ && state->traditionalApplies();
  }
  void activate(fcitx::InputContext *ic) override {
    if (!ic || !ic->propertyFor(factory_)->menuTarget()) return;
    try {
      auto *state = ic->propertyFor(factory_);
      if (state->restricted() || state->privateInput()) return;
      if (state->toggleTraditional()) update(ic);
    } catch (...) {}
  }
private:
  fcitx::FactoryFor<FcitxState> *factory_;
};

// Fcitx5 自带候选主题的恢复值，接管记录与退出接管共用；与 `msime-linux-setup --unregister` 一致。
const Json kClassicuiStockThemes{{"Theme", "default"}, {"DarkTheme", "default-dark"}};

// classicui's options are shared by every input method, so before one changes, the value it replaces is recorded for msime-linux-setup --unregister to put back (see PanelRestoreRecord.h). A failed record does not hold the change back.
void record_classicui_takeover(const fcitx::RawConfig &current, const fcitx::RawConfig &written) {
  const auto file = msime::linux_host::panel_restore_file(std::getenv("XDG_STATE_HOME"), std::getenv("HOME"));
  if (!file) return;
  for (const auto &key : written.subItems()) {
    const auto *value = written.valueByPath(key);
    if (!value) continue;
    const auto *prior = current.valueByPath(key);
    const auto replaced = prior ? Json(*prior) : Json(nullptr);
    // 水杉只从 Fcitx5 自带的主题手里接管，卸载时会删掉自己的主题，所以一项已经指向水杉时，记下它替代的那个自带主题。
    auto restore = replaced;
    if (prior && *prior == msime::linux_host::kFcitxCandidateTheme && kClassicuiStockThemes.contains(key))
      restore = kClassicuiStockThemes.at(key);
    msime::linux_host::record_panel_takeover(*file, "fcitx5", key, replaced, *value, restore);
  }
}
// `current` 是调用方刚从 `getConfig()` 存下的现值：经典界面的 `getConfig()` 每次都要扫描并解析所有已装主题，已经取过一次的调用方不该为了记录再取一次。
void set_classicui_config(fcitx::AddonInstance &classicui, const fcitx::RawConfig &current, const fcitx::RawConfig &config) {
  record_classicui_takeover(current, config);
  classicui.setConfig(config);
}
void set_classicui_config(fcitx::AddonInstance &classicui, const fcitx::RawConfig &config) {
  fcitx::RawConfig current;
  if (const auto *existing = classicui.getConfig()) existing->save(current);
  set_classicui_config(classicui, current, config);
}

// 经典界面当前选中的 `Theme` 与 `DarkTheme`，取自它落盘的 `conf/classicui.conf`。经典界面启动时从这个文件读配置，`setConfig`（fcitx5-configtool 保存、水杉接管主题都走它）写完立即 `safeSaveAsIni` 回同一个文件，所以文件就是现值。不用 `getConfig()`：它每调一次都扫描 PkgData 下所有 themes 目录、逐个打开解析 theme.conf，还要在 PATH 里找 Plasma 主题工具，开销随已装主题数线性增长，而且跑在处理按键的事件循环上（#5988）。读不到的键保持缺省：`Theme` 为空串，`DarkTheme` 为空，与 `fcitx_theme_replaceable` / `fcitx_candidate_theme_drawn` 对缺省的处理一致（经典界面的缺省值 default、default-dark 也都可替换）。
struct ClassicUiThemeSelection {
  std::string theme;
  std::optional<std::string> dark_theme;
};
ClassicUiThemeSelection read_classicui_theme_selection() {
  fcitx::RawConfig config;
  fcitx::readAsIni(config, "conf/classicui.conf");
  ClassicUiThemeSelection selection;
  if (const auto *theme = config.valueByPath("Theme")) selection.theme = *theme;
  if (const auto *dark = config.valueByPath("DarkTheme")) selection.dark_theme = *dark;
  return selection;
}
// 经典界面落盘的 `WheelForPaging`，原生画布的滚轮翻页照它，X11 上换成原生画布后滚轮与原先 classicui 画的候选窗一样：水杉的「鼠标滚轮」开关动过之后会写进这一项（applyCandidateWheelPaging），没动过时就是桌面原有的值。等于缺省值的项 fcitx 写成注释，解析后缺失即缺省的开，只有写成 `False` 才是关。
bool read_classicui_wheel_for_paging() {
  fcitx::RawConfig config;
  fcitx::readAsIni(config, "conf/classicui.conf");
  const auto *value = config.valueByPath("WheelForPaging");
  return !value || *value != "False";
}
// 无候选覆盖时，只恢复仍由水杉持有的 classicui 主题项：有记录用原值，否则用自带主题；直接写 addon，不经接管记录接口，避免污染原值。
void restore_classicui_theme(fcitx::AddonInstance &classicui) {
  // 没有候选覆盖是每一拍都会走到的状态（全局主题为「系统」是默认值），所以先按落盘的 classicui.conf 看有没有仍是水杉写的项：
  // 没有就一次 `getConfig()` 都不调，后者每次都扫描并解析全部已装主题（#5988）。
  const auto selection = read_classicui_theme_selection();
  const bool holds_theme = selection.theme == msime::linux_host::kFcitxCandidateTheme;
  const bool holds_dark = selection.dark_theme && *selection.dark_theme == msime::linux_host::kFcitxCandidateTheme;
  if (!holds_theme && !holds_dark) return;
  fcitx::RawConfig current;
  if (const auto *existing = classicui.getConfig()) existing->save(current);
  Json held = Json::object();
  for (const auto &item : kClassicuiStockThemes.items()) {
    const auto *value = current.valueByPath(item.key());
    if (value && *value == msime::linux_host::kFcitxCandidateTheme) held[item.key()] = *value;
  }
  if (held.empty()) return;
  Json record = Json::object();
  if (const auto file = msime::linux_host::panel_restore_file(std::getenv("XDG_STATE_HOME"), std::getenv("HOME")))
    if (const auto saved = msime::linux_host::read_panel_restore(*file)) record = *saved;
  const auto restore = msime::linux_host::panel_restore_values(record, "fcitx5", held, kClassicuiStockThemes);
  fcitx::RawConfig config;
  for (const auto &item : restore.items()) config.setValueByPath(item.key(), item.value().get<std::string>());
  classicui.setConfig(config);
}

#ifdef MSIME_FCITX5_HINT_FONT
// classicui 的经典界面按自己的字体与 Pango 分辨率排版模式提示；这里用同一个分辨率测量，
// 才能把「中」「英」补到装饰图完整显示所需的逻辑宽度。分辨率只在 Wayland 且设置了
// ForceWaylandDPI 时改变——X11 的 Xft.dpi 与每屏 DPI 只改设备缩放（cairo device scale），
// 不改变 yoga 布局用的逻辑尺寸，所以提示宽度不必跟着它们走。
double fcitx_default_font_resolution() {
  static const double resolution = [] {
    auto *font_map = pango_cairo_font_map_new();
    const double value = pango_cairo_font_map_get_resolution(PANGO_CAIRO_FONT_MAP(font_map));
    g_object_unref(font_map);
    return value;
  }();
  return resolution;
}

// 一段文字在同一字体与分辨率下的逻辑宽度（像素），与 classicui 的 pango_layout_get_pixel_size 一致。
int fcitx_measure_text(const std::string &font, double resolution, const std::string &text) {
  auto *font_map = pango_cairo_font_map_new();
  pango_cairo_font_map_set_resolution(PANGO_CAIRO_FONT_MAP(font_map), resolution);
  auto *context = pango_font_map_create_context(PANGO_FONT_MAP(font_map));
  auto *layout = pango_layout_new(context);
  auto *description = pango_font_description_from_string(font.c_str());
  pango_layout_set_font_description(layout, description);
  pango_layout_set_text(layout, text.c_str(), -1);
  int width = 0;
  pango_layout_get_pixel_size(layout, &width, nullptr);
  pango_font_description_free(description);
  g_object_unref(layout);
  g_object_unref(context);
  g_object_unref(font_map);
  return width;
}
#endif

// classicui 正在绘制的是不是水杉主题：UseDarkTheme 打开且桌面为深色时画 DarkTheme，
// 否则画 Theme（与 classicui reloadTheme 的规则一致）。
inline bool fcitx_draws_candidate_theme(const std::string &theme, const std::string &dark_theme,
                                        bool use_dark_theme, bool system_dark) {
  return (use_dark_theme && system_dark ? dark_theme : theme) == msime::linux_host::kFcitxCandidateTheme;
}

// 装饰图完整落在 OverlayClipMargin 之内所需的面板宽度（逻辑单位），与 classicui Theme::paint 的定位
// 一致：居中时两边各留一份 clip 边距，靠边时从对齐边量 OverlayOffsetX，再留对面的 clip 边距。
inline int fcitx_overlay_panel_width(int overlay_width, const std::string &gravity, int offset_x,
                                     int clip_left, int clip_right) {
  if (overlay_width <= 0) return 0;
  if (gravity == "Top Center") return overlay_width + 2 * std::max(clip_left, clip_right);
  return overlay_width + offset_x + (gravity == "Top Left" ? clip_right : clip_left);
}

// 提示文本要占的宽度：面板宽度减去主题在文字两侧留的边距，与 classicui 的 yoga 布局一致。
inline int fcitx_hint_text_width(int panel_width, int content_left, int content_right, int text_left, int text_right) {
  return panel_width - content_left - content_right - text_left - text_right;
}

// 读回刚写好的主题，得出提示文本至少要多宽才能让装饰完整显示。没有装饰、装饰宽度读不出来
// （非 PNG、声明尺寸超限）时返回 0，提示保持原样。只在写主题时调用一次，热路径不读文件。
inline int fcitx_hint_width_from_theme(const std::filesystem::path &theme_file) {
  namespace host = msime::linux_host;
  fcitx::RawConfig theme;
  fcitx::readAsIni(theme, theme_file.string());
  const auto *overlay = theme.valueByPath("InputPanel/Background/Overlay");
  if (!overlay || overlay->empty()) return 0;
  std::string header(24, '\0');
  std::ifstream image(theme_file.parent_path() / *overlay, std::ios::binary);
  if (!image.read(header.data(), static_cast<std::streamsize>(header.size()))) return 0;
  const auto width = host::fcitx_png_width(header);
  if (!width) return 0;
  const auto number = [&theme](const char *path) {
    const auto *value = theme.valueByPath(path);
    if (!value) return 0;
    try {
      return std::stoi(*value);
    } catch (...) {
      return 0;
    }
  };
  const auto *gravity = theme.valueByPath("InputPanel/Background/Gravity");
  const int panel = fcitx_overlay_panel_width(*width, gravity ? *gravity : std::string(),
                                              number("InputPanel/Background/OverlayOffsetX"),
                                              number("InputPanel/Background/OverlayClipMargin/Left"),
                                              number("InputPanel/Background/OverlayClipMargin/Right"));
  const int text = fcitx_hint_text_width(panel, number("InputPanel/ContentMargin/Left"),
                                         number("InputPanel/ContentMargin/Right"),
                                         number("InputPanel/TextMargin/Left"),
                                         number("InputPanel/TextMargin/Right"));
  return text > 0 ? text : 0;
}

// 把提示补到至少 target 宽：按全角空格的数量一次算出，不逐字符排版。
inline std::string fcitx_pad_hint_label(const std::string &label, int target, int natural, int space) {
  if (target <= natural || space <= 0) return label;
  const int count = (target - natural + space - 1) / space;
  std::string message = label;
  for (int index = 0; index < count; ++index) message += "\u3000";
  return message;
}

// classicui 的主题选择与面板字体，在每次主题/字体同步时读一次；模式提示的中/英切换只用这份缓存，
// 不再调 getConfig()——它会扫描主题目录并逐个解析 theme.conf，不能放在按键热路径上。
struct FcitxHintInputs {
  bool active = false;
  std::string font;
  int force_wayland_dpi = 0;
};

inline FcitxHintInputs fcitx_hint_inputs(const fcitx::RawConfig &config, bool system_dark) {
  const auto value = [&config](const char *path) {
    const auto *found = config.valueByPath(path);
    return found ? *found : std::string();
  };
  int dpi = 0;
  try {
    dpi = std::stoi(value("ForceWaylandDPI"));
  } catch (...) {
    dpi = 0;
  }
  return FcitxHintInputs{
      fcitx_draws_candidate_theme(value("Theme"), value("DarkTheme"), value("UseDarkTheme") == "True", system_dark),
      value("Font"), dpi};
}

// 活动主题、跟随深色、字体与 Wayland 字体 DPI 任一变化都要刷新提示缓存。
// 只读落盘的 `conf/classicui.conf`，不逐拍调会扫描全部已装主题的 `getConfig()`（#5988）；
// 等于默认值的项 fcitx 写成注释，解析后缺失即默认，而提示本身在真正重读时取自经典界面的现值（见 FcitxHintInputs）。
std::string read_classicui_hint_stamp() {
  fcitx::RawConfig config;
  fcitx::readAsIni(config, "conf/classicui.conf");
  std::string stamp;
  for (const auto *key : {"Theme", "DarkTheme", "UseDarkTheme", "Font", "ForceWaylandDPI"}) {
    const auto *value = config.valueByPath(key);
    stamp += key;
    stamp += '=';
    stamp += value ? *value : std::string();
    stamp += '\n';
  }
  return stamp;
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
    auto *classicui = instance_->addonManager().addon("classicui", true);
    // 面板里现有的描述只在第一次同步时用得上（见 CandidateFontSync::next）。
    std::optional<std::string> current;
    if (!candidate_font_sync_.primed() && classicui && classicui->getConfig()) {
      fcitx::RawConfig existing;
      classicui->getConfig()->save(existing);
      if (const auto *font = existing.valueByPath("Font")) current = *font;
    }
    const auto description = candidate_font_sync_.next(msime::linux_host::read_candidate_font(preferences), current);
    if (!description || !classicui) return;
    fcitx::RawConfig config;
    config.setValueByPath("Font", *description);
    set_classicui_config(*classicui, config);
    candidate_font_sync_.mark_applied(*description);
    // 写字体不经 applyCandidatePanelTheme，提示的字体缓存要在这里跟上。
    hint_inputs_.font = *description;
  }
  bool candidatePanelFontPending() const { return candidate_font_sync_.pending(); }
  // 配色写入用户目录的 `msime` 主题：后台同步只替换自带或水杉主题，主动选择可接管第三方；无覆盖时恢复仍持有的项，写配置让 classicui 重载。
  // 返回 true 表示面板主题已套用或退出，调用方需要刷新真实候选窗口。
  bool applyCandidatePanelTheme(const Json &preferences, bool system_dark, const Json &catalog, bool chosen) {
    return applyCandidatePanelTheme(instance_->addonManager().addon("classicui", true), preferences, system_dark, catalog, chosen);
  }
  // 每个有焦点的上下文每 250 ms 都会走到这里（refreshProviderSockets），所以只比廉价的输入：主题请求本身（全局主题、自定义主题、明暗与所画皮肤包的目录条目，颜色完全由它决定）、圆角和装饰图的戳。这些输入已经接管写好过一次就什么也不做，和原先按整份主题文本比较的语义相同：之后用户自己改了 classicui.conf，或卸载时 `msime-linux-setup --unregister` 把主题还原，都不会被这里再改回去。`chosen` 是唯一的例外：用户在主题菜单里重新选择水杉主题时，即使输入没变也要重新接管。
  //
  // 还没接管成功时（用户选了第三方主题、没有经典界面、写主题失败），原先每一拍都重新栅格化并调 `getConfig()`，后者每次都扫描并解析全部已装主题（#5988）。现在只在可能让结果不同的东西变了时才重试：上面的输入、经典界面是否存在，以及它落盘的 Theme/DarkTheme 选择（fcitx5-configtool 改回默认主题会写这个文件，接管随之恢复）。写主题失败另外每 10 秒重试一次，登录时数据目录暂时不可写之类的情况能自己恢复。「重启输入法服务」（resetSessions）清掉两份记录，下一拍无条件重做。
  bool applyCandidatePanelTheme(fcitx::AddonInstance *classicui, const Json &preferences, bool system_dark,
                                const Json &catalog, bool chosen) {
    namespace host = msime::linux_host;
    // 用户维护 classicui 主题时完全退出生成与接管，不写文件、不清旧图片，也不以原生画布盖住人工主题。
    if (!host::fcitx_theme_autogenerate_enabled(std::getenv("MSIME_FCITX5_THEME_AUTOGENERATE"))) {
      const bool was_applied = !candidate_theme_applied_.empty();
      candidate_theme_applied_.clear();
      candidate_theme_attempt_.clear();
      candidate_canvas_source_.clear();
#ifdef MSIME_FCITX5_NATIVE_CANVAS
      native_canvas_assets_.reset();
      native_canvas_sources_.clear();
      native_canvas_logo_.reset();
      native_canvas_owned_ = false;
#endif
      candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point::max();
      hint_text_width_ = 0;
      hint_inputs_ = FcitxHintInputs{};
      return was_applied;
    }
    const auto resolved = resolveCandidateTheme(preferences, system_dark, catalog);
    // 无覆盖自定义主题或解析失败才退出；有效系统主题仍绘制水杉样式，只跟随原生配色。
    // 只恢复仍是水杉自己写的项，用户在 fcitx5-configtool 选的主题不动。
    if (!resolved.covers_candidates) {
      const bool was_applied = !candidate_theme_applied_.empty();
      if (classicui) restore_classicui_theme(*classicui);
      candidate_theme_applied_.clear();
      candidate_canvas_source_.clear();
#ifdef MSIME_FCITX5_NATIVE_CANVAS
      native_canvas_assets_.reset();
      native_canvas_sources_.clear();
      native_canvas_logo_.reset();
      native_canvas_owned_ = false;
#endif
      candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point::max();
      hint_text_width_ = 0;
      hint_inputs_ = FcitxHintInputs{};
      return was_applied;
    }
    auto colors = resolved.colors;
    const auto decoration = host::candidate_skin_decoration(catalog, resolved.candidate_skin);
    const auto corner_radius = host::candidate_corner_radius(preferences, catalog, resolved.candidate_skin);
    // Only the user's own radius pulls the highlight's corners in with the card.
    const bool user_radius = host::candidate_corner_radius_preference(preferences).has_value();
    // 用户的候选窗样式（缩放、内边距、不透明度、翻页箭头）与皮肤包的行圆角，照 macOS 的 StyledCandidateSkin 生成进主题。
    auto style = host::read_fcitx_candidate_style(preferences);
    style.item_radius = host::candidate_skin_item_radius(catalog, resolved.candidate_skin);
    // logo 只在 show_app_logo 打开时画，与 macOS、Windows 相同；已存的偏好文档缺这个键时按共享层的读法当作打开。
    const bool show_logo = [&] {
      if (!preferences.is_object()) return true;
      const auto value = preferences.find("show_app_logo");
      return value == preferences.end() || !value->is_boolean() || value->get<bool>();
    }();
    // 皮肤包的背景图；它的戳和装饰图一样代替图片本身进输入，图片换了才重画。
    const auto background = host::candidate_skin_background(catalog, resolved.candidate_skin);
    // The decoration's stamp stands in for its image, so an unchanged skin costs a stat per refresh, not a copy.
    const auto canvas_directory = host::fcitx_canvas_directory(catalog);
    const auto overlay_stamp = host::fcitx_overlay_stamp(decoration);
    std::map<std::filesystem::path, std::string> source_stamps;
    const auto source_stamp = [&](const std::filesystem::path &path) {
      auto at = source_stamps.find(path);
      if (at == source_stamps.end()) at = source_stamps.emplace(path, host::fcitx_canvas_file_stamp(path)).first;
      return at->second;
    };
    const auto theme_inputs = [&] {
      Json canvas_stamps = Json::array();
      if (canvas_directory && host::safe_skin_id(resolved.candidate_skin)) {
        canvas_stamps.push_back(source_stamp(*canvas_directory / resolved.candidate_skin / "skin.toml"));
        if (!candidate_canvas_source_.empty()) canvas_stamps.push_back(source_stamp(candidate_canvas_source_));
#ifdef MSIME_FCITX5_NATIVE_CANVAS
        for (const auto &source : native_canvas_sources_) canvas_stamps.push_back(source_stamp(source));
#endif
      }
      return Json{
        {"request", host::candidate_theme_request(preferences, host::candidate_dark_theme(preferences, system_dark), catalog)},
        {"corner_radius", corner_radius ? Json(*corner_radius) : Json(nullptr)},
        {"user_radius", user_radius},
        {"style", Json{{"scale", style.scale}, {"padding", style.padding ? Json(*style.padding) : Json(nullptr)},
                       {"opacity", style.opacity}, {"page_arrows", style.page_arrows},
                       {"item_radius", style.item_radius ? Json(*style.item_radius) : Json(nullptr)}}},
        {"logo", show_logo},
        {"background", background ? host::fcitx_canvas_file_stamp(background->image) : std::string()},
        {"overlay", overlay_stamp},
        {"canvas", std::move(canvas_stamps)}}.dump();
    };
    auto inputs = theme_inputs();
    if (std::chrono::steady_clock::now() >= candidate_canvas_retry_at_) {
      candidate_theme_applied_.clear();
      candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point::max();
    }
    // 缓存只决定「同一份输入已经接管写好」这一件事：一拍不调 `getConfig()`，也不重写主题（#5988）；一次新的主动选择（chosen）仍然往下走，重新判断所有权。
    // classicui 落盘的活动主题、跟随深色、字体与 Wayland 字体 DPI 变了时刷新提示（见 read_classicui_hint_stamp）。
    const auto hint_stamp = read_classicui_hint_stamp();
    // 用户可能在 fcitx5-configtool 里改了滚轮翻页，原生画布每一拍跟上经典界面落盘的值。
    classicui_wheel_paging_ = read_classicui_wheel_for_paging();
    if (inputs == candidate_theme_applied_ && !chosen && hint_stamp == classicui_hint_stamp_) return false;
    auto attempt = Json{{"inputs", inputs}, {"chosen", chosen}, {"classicui", classicui != nullptr}, {"hint", hint_stamp}};
    if (classicui) {
      const auto selection = read_classicui_theme_selection();
      attempt["theme"] = selection.theme;
      attempt["dark_theme"] = selection.dark_theme ? Json(*selection.dark_theme) : Json(nullptr);
    }
    auto attempt_key = attempt.dump();
    const auto now = std::chrono::steady_clock::now();
    if (attempt_key == candidate_theme_attempt_ && now < candidate_theme_retry_at_) return false;
    candidate_theme_attempt_ = std::move(attempt_key);
    candidate_theme_retry_at_ = std::chrono::steady_clock::time_point::max();
    const auto *live = classicui ? classicui->getConfig() : nullptr;
    if (!live) return false;
    fcitx::RawConfig current;
    live->save(current);
    // 模式提示的热路径不读 classicui 配置（getConfig() 会扫描主题目录并逐个解析 theme.conf），
    // 每次同步在这里记下它画的主题、字体与 Wayland 字体 DPI。
    hint_inputs_ = fcitx_hint_inputs(current, system_dark);
    classicui_hint_stamp_ = hint_stamp;
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    const bool native_was_owned = native_canvas_owned_;
    const auto *native_theme = current.valueByPath("Theme");
    const auto *native_dark_theme = current.valueByPath("DarkTheme");
    native_canvas_owned_ = native_theme && *native_theme == host::kFcitxCandidateTheme &&
        host::fcitx_candidate_theme_drawn(*native_theme, native_dark_theme);
#endif
    // 同一份主题已接管过时，外部配置变化只刷新提示，不追回用户手改或卸载还原的主题。
    if (inputs == candidate_theme_applied_ && !chosen) {
#ifdef MSIME_FCITX5_NATIVE_CANVAS
      return native_was_owned != native_canvas_owned_;
#else
      return false;
#endif
    }
    const auto *selected = current.valueByPath("Theme");
    const auto *selected_dark = current.valueByPath("DarkTheme");
    // 用户刚在主题菜单里选了水杉主题时（chosen）即使当前是第三方主题也接管；焦点进入、偏好同步、系统明暗变化都只是重读同一份偏好，绝不把用户选的主题换回来。
    if (!chosen && !host::fcitx_theme_replaceable(selected ? *selected : std::string{})) return false;
    const bool replace_dark = selected_dark && (chosen || host::fcitx_theme_replaceable(*selected_dark));
    // 同一份输入已经写过主题文件，就不必为一次重新接管再栅格化一遍；所有权与 classicui 配置仍照常处理。
    if (inputs != candidate_theme_applied_) {
      std::optional<host::FcitxCanvasPanel> canvas_panel;
#ifdef MSIME_FCITX5_NATIVE_CANVAS
      native_canvas_assets_.reset();
      native_canvas_sources_.clear();
      native_canvas_logo_.reset();
      // 所画皮肤包的完整清单（msime_client_skin_package）：非 canvas 包也从这里取共享版式、`[animation]` 和装饰图待机动效。
      Json drawn_package = Json::object();
      // canvas 包（或读不出来、不知道是不是 canvas 的包）贴图坏了时照旧退回 classicui 的静态面板，不改用 macOS 结构的卡片：canvas 包的外观由它自己的贴图和清单版式决定。
      bool canvas_package = false;
#endif
      candidate_canvas_source_.clear();
      if (canvas_directory && host::safe_skin_id(resolved.candidate_skin)) {
        try {
          const auto encoded = Json{{"directory", canvas_directory->string()}, {"id", resolved.candidate_skin}}.dump();
          const auto package = response(msime_client_skin_package(reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
#ifdef MSIME_FCITX5_NATIVE_CANVAS
          drawn_package = package;
          canvas_package = package.value("canvas", false);
#endif
          candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point::max();
          if (const auto border = package.find("borderImage"); package.value("canvas", false) && border != package.end()) {
            const auto &image = border->at(!resolved.dark && border->contains("lightSource") ? "lightSource" : "source");
            if (const auto source = host::fcitx_canvas_resource(*canvas_directory, resolved.candidate_skin, image))
              candidate_canvas_source_ = *source;
          }
#ifdef MSIME_FCITX5_NATIVE_CANVAS
          for (const auto &item : package.value("decorations", Json::array())) {
            if (const auto source = host::fcitx_canvas_resource(*canvas_directory, resolved.candidate_skin, item.at("image")))
              native_canvas_sources_.push_back(*source);
          }
          if (const auto image = package.find("decorationImage"); image != package.end())
            if (const auto source = host::fcitx_canvas_resource(*canvas_directory, resolved.candidate_skin, *image))
              native_canvas_sources_.push_back(*source);
#endif
          // 新依赖也在解码前取戳；已有路径与清单沿用本次调用首次读到的戳。
          inputs = theme_inputs();
          canvas_panel = host::prepare_fcitx_canvas_panel(package, *canvas_directory, resolved.candidate_skin, resolved.dark,
                                                         &candidate_canvas_source_);
#ifdef MSIME_FCITX5_NATIVE_CANVAS
          if (canvas_package) native_canvas_assets_ = host::load_native_canvas(package, *canvas_directory, resolved.candidate_skin, resolved.dark);
#endif
          if (canvas_panel
#ifdef MSIME_FCITX5_NATIVE_CANVAS
              || native_canvas_assets_
#endif
          ) {
            auto request = host::candidate_theme_request(preferences, resolved.dark, catalog);
            request.erase("package");
            request["skins_directory"] = canvas_directory->string();
            request["canvas_panel_drawn"] = canvas_panel.has_value();
            const auto query = request.dump();
            const auto panel_document = response(msime_client_resolve_theme(reinterpret_cast<const uint8_t *>(query.data()), query.size()));
#ifdef MSIME_FCITX5_NATIVE_CANVAS
            native_canvas_palette_ = host::native_canvas_palette(panel_document, resolved.dark);
            if (native_canvas_assets_ && corner_radius) {
              native_canvas_assets_->card_radius = *corner_radius;
              native_canvas_assets_->user_radius = user_radius;
            }
            if (native_canvas_assets_) native_canvas_assets_->opacity = style.opacity;
#endif
            const auto panel_theme = host::candidate_theme_colors(panel_document, resolved.dark);
            if (panel_theme.candidate_skin == resolved.candidate_skin) colors = panel_theme.colors;
            else {
              canvas_panel.reset();
#ifdef MSIME_FCITX5_NATIVE_CANVAS
              native_canvas_assets_.reset();
#endif
            }
          }
        } catch (const std::exception &) {
#ifdef MSIME_FCITX5_NATIVE_CANVAS
          native_canvas_assets_.reset();
          canvas_package = true;
#endif
          canvas_panel.reset();
          // 无效包不能阻止之后的资源修复；失败扫描每十秒重试，不进入每拍热路径。
          candidate_canvas_retry_at_ = now + kCandidateThemeRetry;
        }
      }
#ifdef MSIME_FCITX5_NATIVE_CANVAS
      // X11 会话里原生画布是所有主题的默认画法：没有 canvas 包时按主题配色、用户样式和皮肤包的版式合成 macOS 结构的卡片。逃生口 MSIME_FCITX5_CANDIDATE_RENDERER=classicui 让整套退回 classicui；kimpanel 和 Wayland 原生客户端本来就不走这里（renderNativeCanvas 只认 X11 上的 classicui）。Wayland 会话里走 XWayland 的程序也不合成这张卡片，照旧由 classicui 画水杉主题，与 candidate-panel.json 报告的能力一致（publishCandidatePanelStatus 用同一个会话判断）；canvas 皮肤包在那里照旧由原生画布画。
      const bool native_renderer = host::native_canvas_renderer_enabled(std::getenv("MSIME_FCITX5_CANDIDATE_RENDERER"));
      const bool chrome_session =
          host::candidate_session_is_x11(std::getenv("XDG_SESSION_TYPE"), std::getenv("DISPLAY"), std::getenv("WAYLAND_DISPLAY"));
      if (!native_renderer) native_canvas_assets_.reset();
      if (native_renderer && chrome_session && !native_canvas_assets_ && !canvas_package) {
        try {
          const auto query = host::candidate_theme_request(preferences, resolved.dark, catalog).dump();
          const auto document = response(msime_client_resolve_theme(reinterpret_cast<const uint8_t *>(query.data()), query.size()));
          native_canvas_palette_ = host::native_canvas_theme_palette(document, resolved.dark);
          const auto metrics = host::fcitx_panel_metrics(style);
          host::NativeCanvasThemeStyle chrome;
          chrome.scale = metrics.scale;
          chrome.opacity = style.opacity;
          // 描边画在内边距里，内边距取两者中较大的（macOS 的 CandidateInset）；描边本身不缩放。
          chrome.border_width = colors.border ? std::max(0, colors.border_width) : 0;
          chrome.padding = std::max<double>(chrome.border_width, metrics.padding);
          chrome.card_radius = (corner_radius ? std::clamp(*corner_radius, 0.0, 32.0) : 10.0) * metrics.scale;
          chrome.user_radius = user_radius;
          // 行圆角照 macOS 的 StyledCandidateSkin：用户设了卡片圆角时与卡片同心，只调了内边距时至少大到高亮的角不伸出卡片，都没调时取皮肤包的行圆角或 6pt。
          double row_radius = (style.item_radius ? std::clamp(*style.item_radius, 0.0, 16.0) : 6.0) * metrics.scale;
          const double half_border = chrome.border_width / 2;
          if (user_radius) row_radius = std::max(0.0, chrome.card_radius - chrome.padding);
          else if (style.padding)
            row_radius = std::max(row_radius, chrome.card_radius - half_border - (chrome.padding - half_border) * (2.0 + std::sqrt(2.0)));
          chrome.item_radius = row_radius;
          chrome.show_selected_bar = native_canvas_palette_.value("show_selected_bar", false);
          chrome.min_width = drawn_package.value("minWidthDip", 0.0) * metrics.scale;
          chrome.decoration = decoration;
          chrome.background = background;
          native_canvas_assets_ = host::native_canvas_theme_assets(chrome, drawn_package);
          if (decoration) native_canvas_sources_.push_back(decoration->image);
          if (show_logo) native_canvas_logo_ = host::load_native_canvas_logo(MSIME_ICON_DIR);
        } catch (const std::exception &) {
          native_canvas_assets_.reset();
        }
      }
#endif
      const auto file = host::fcitx_theme_file(std::getenv("XDG_DATA_HOME"), std::getenv("HOME"));
      // 图标只随安装包变化，这里只在主题输入变了、要重画主题时才按当前缩放读一次。
      const auto logo = show_logo ? host::load_fcitx_theme_logo(MSIME_ICON_DIR, style.scale) : std::nullopt;
      // 皮肤包的 [toolbar] 配色画到 classicui 的菜单上：Linux 的工具栏功能都在输入法菜单里。
      colors.menu = host::overlay_menu_slots(colors.menu,
                                             host::candidate_skin_toolbar_menu(catalog, resolved.candidate_skin, resolved.dark));
      if (!file || !host::write_fcitx_candidate_theme(*file, colors, resolved.dark, decoration, corner_radius, logo, user_radius,
                                                      host::scale_fcitx_overlay_png, canvas_panel, style, background,
                                                      host::render_fcitx_background)) {
        candidate_theme_retry_at_ = now + kCandidateThemeRetry;
        return false;
      }
#ifdef MSIME_FCITX5_HINT_FONT
      // 装饰图的宽度只在写主题时读一次，模式提示切换时直接用结果。
      hint_text_width_ = fcitx_hint_width_from_theme(*file);
#endif
    }
    fcitx::RawConfig config;
    config.setValueByPath("Theme", std::string(host::kFcitxCandidateTheme));
    // Fcitx5 releases with a separate dark-mode theme would otherwise switch to their stock dark theme; MSIME already resolves "follow" against the system appearance itself.
    if (replace_dark) config.setValueByPath("DarkTheme", std::string(host::kFcitxCandidateTheme));
    set_classicui_config(*classicui, current, config);
    // 缓存的是写入后的活动主题，第一次模式提示不应继续沿用接管前的第三方主题。
    current.setValueByPath("Theme", std::string(host::kFcitxCandidateTheme));
    if (replace_dark) current.setValueByPath("DarkTheme", std::string(host::kFcitxCandidateTheme));
    hint_inputs_ = fcitx_hint_inputs(current, system_dark);
    classicui_hint_stamp_ = read_classicui_hint_stamp();
    // 只缓存解码前的完整依赖快照，不能把加载期间的新文件误记为已解码。
    candidate_theme_applied_ = std::move(inputs);
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    native_canvas_owned_ = host::fcitx_candidate_theme_drawn(
        *current.valueByPath("Theme"), current.valueByPath("DarkTheme"));
#endif
    candidate_theme_attempt_.clear();
    return true;
  }
#ifdef MSIME_FCITX5_MODE_BADGE
  // 整个插件只有一块切换徽章，与 macOS 只有一个提示面板相同：新的提示替换屏幕上那块，而不是在另一个输入框的光标下再叠一块；Fcitx5 也只为它占一条显示服务器连接，而不是每个输入上下文一条。第一次真要画时才建，连不上合成器或 X 服务器时记下来不再重试，之后只用面板的文字提示。
  msime::linux_host::ModeBadgeSurface *modeBadge() {
    if (!mode_badge_ && !mode_badge_unavailable_) {
      mode_badge_ = msime::linux_host::ModeBadgeSurface::create();
      mode_badge_unavailable_ = !mode_badge_;
    }
    return mode_badge_.get();
  }
  // 徽章显示 0.6 秒，再分几帧在 0.18 秒里淡出，与 macOS 相同；后端做不到半透明时直接隐藏。新的提示重新计时。用事件循环的定时器而不是线程：绘制和销毁都必须回到这条线程上，Wayland 连接不是线程安全的。`owner` 是这次提示所属的输入上下文，见 dismissModeBadge。
  void scheduleModeBadgeHide(fcitx::InputContext &owner) {
    using Timing = msime::linux_host::ModeBadgeTiming;
    mode_badge_owner_ = owner.watch();
    mode_badge_fade_step_ = 0;
    mode_badge_timer_ = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + Timing::visible_ms * 1000ULL, 0,
        [this](fcitx::EventSourceTime *timer, uint64_t) {
          if (!mode_badge_) return true;
          ++mode_badge_fade_step_;
          const double alpha = msime::linux_host::mode_badge_fade_alpha(mode_badge_fade_step_);
          if (alpha > 0.0 && mode_badge_->set_opacity(alpha)) {
            timer->setNextInterval(Timing::fade_ms * 1000ULL / Timing::fade_steps);
            timer->setOneShot();
            return true;
          }
          mode_badge_->hide();
          return true;
        });
  }
  // 离开输入框或换到别的输入法时立刻收起徽章并停掉淡出，与 macOS 在 deactivateServer: 里 orderOut 提示面板相同。只收这个上下文自己的那条提示：Fcitx5 先让新输入框获得焦点、再让旧的失焦时，屏幕上那块已经属于新输入框。
  void dismissModeBadge(fcitx::InputContext *ic) {
    if (!mode_badge_ || mode_badge_owner_.get() != ic) return;
    mode_badge_owner_.unwatch();
    mode_badge_timer_.reset();
    mode_badge_->hide();
  }
#endif
#ifdef MSIME_LINUX_FLOATING_PANELS
  // fcitx::EventLoop 上的 OverlayLoop（见 OverlayLoop.h）。回调先把函数对象复制出来再调用：回调里换掉自己的令牌会连同事件源一起析构它原来的那份。
  msime::linux_host::OverlayLoop overlayLoop() {
    msime::linux_host::OverlayLoop loop;
    auto *events = &instance_->eventLoop();
    loop.watch = [events](int fd, std::function<void()> ready) -> msime::linux_host::OverlayLoop::Token {
      return std::shared_ptr<fcitx::EventSourceIO>(events->addIOEvent(
          fd, fcitx::IOEventFlag::In, [ready](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) {
            const auto callback = ready;
            callback();
            return true;
          }));
    };
    loop.timer = [events](int milliseconds, std::function<void()> fire) -> msime::linux_host::OverlayLoop::Token {
      return std::shared_ptr<fcitx::EventSourceTime>(events->addTimeEvent(
          CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + static_cast<uint64_t>(std::max(0, milliseconds)) * 1000ULL, 0,
          [fire](fcitx::EventSourceTime *, uint64_t) {
            const auto callback = fire;
            callback();
            return true;
          }));
    };
    return loop;
  }
  // 整个插件只有一条悬浮工具栏，与 macOS 的 sharedPanel 相同，跟着当前属主（获得焦点的输入上下文）走。只在开关开着、有上下文接手时才建；建不出来（没有 X 服务器）时记下来不再重试，之后只有状态区的「工具栏」子菜单。
  msime::linux_host::FloatingToolbar *floatingToolbar(bool create) {
    if (!floating_toolbar_ && create && !floating_toolbar_unavailable_) {
      msime::linux_host::FloatingToolbar::Options options;
      options.icon_path = MSIME_MODE_BADGE_ICON;
      options.display_name = MSIME_EDITION_DISPLAY_NAME;
      if (auto file = msime::linux_host::floating_toolbar_position_path(
              std::getenv("XDG_STATE_HOME"), std::getenv("HOME"), MSIME_EDITION_CLIENT_DIRECTORY))
        options.position_file = *file;
      const char *wayland = std::getenv("WAYLAND_DISPLAY");
      options.hide_on_focus_out = wayland && *wayland;
      auto loop = overlayLoop();
      floating_toolbar_ = msime::linux_host::FloatingToolbar::create(
          loop,
          [this](msime::linux_host::FloatingToolbar::Owner owner, msime::linux_host::FloatingToolbarAction action,
                 const std::string &argument) {
            floatingToolbarAction(static_cast<fcitx::InputContext *>(const_cast<void *>(owner)), action, argument);
          },
          std::move(options), msime::linux_host::FloatingToolbar::default_surfaces(loop));
      floating_toolbar_unavailable_ = !floating_toolbar_;
    }
    return floating_toolbar_.get();
  }
  // 工具栏上的按钮和菜单项就是状态区里的同一个动作，与「工具栏」子菜单一样不另写一份。属主在上下文销毁时已经放开（floating_toolbar_destroy_watch_），这里的指针总是活的。
  void floatingToolbarAction(fcitx::InputContext *ic, msime::linux_host::FloatingToolbarAction action,
                             const std::string &argument) {
    using Action = msime::linux_host::FloatingToolbarAction;
    fcitx::Action *target = nullptr;
    switch (action) {
    case Action::ToggleInputMode:
      // 「En」时回到中文：退出英文候选模式，而不是关掉中文输入，与 macOS 相同。
      target = msime::linux_host::floating_toolbar_input_mode_leaves_english_candidates(
                   ic->propertyFor(&factory_)->floatingToolbarState())
                   ? static_cast<fcitx::Action *>(&english_action_)
                   : &input_mode_action_;
      break;
    case Action::SelectScheme:
      for (const auto &[id, item] : std::initializer_list<std::pair<std::string_view, fcitx::Action *>>{
               {"quanpin", &scheme_quanpin_action_}, {"shuangpin", &scheme_shuangpin_action_},
               {"wubi", &scheme_wubi_action_}, {"japanese", &scheme_japanese_action_},
               {"korean", &scheme_korean_action_}, {"cantonese", &scheme_cantonese_action_},
               {"zhuyin", &scheme_zhuyin_action_}, {"vietnamese", &scheme_vietnamese_action_},
               {"tibetan", &scheme_tibetan_action_}, {"stroke", &scheme_stroke_action_}})
        if (id == argument) target = item;
      break;
    case Action::TogglePunctuation: target = &chinese_punctuation_action_; break;
    case Action::ToggleFullwidth: target = &width_action_; break;
    case Action::ToggleCharacterSet: target = &traditional_action_; break;
    case Action::OpenEmoji: target = &desktop_emoji_action_; break;
    case Action::OpenHandwriting:
      if (MSIME_EDITION_HANDWRITING != 0) target = &handwriting_action_;
      break;
    case Action::OpenScreenKeyboard: target = &keyboard_action_; break;
    case Action::ToggleVoice: target = &voice_action_; break;
    case Action::OpenSettings: target = &settings_action_; break;
    case Action::Promotion: target = &promotion_action_; break;
    case Action::Website: target = &website_action_; break;
    case Action::Help: target = &help_action_; break;
    case Action::About: target = &about_action_; break;
    case Action::Feedback: target = &feedback_action_; break;
    case Action::Hide: {
      // 「隐藏悬浮状态栏」写共享偏好 floating_toolbar.enabled，与状态区取消「显示工具栏」同一条保存路径。
      auto *state = ic->propertyFor(&factory_);
      if (state->preferences_.value("floating_toolbar", Json::object()).value("enabled", true)) {
        try { state->toggleFloatingToolbar(); } catch (...) {}
      }
      return;
    }
    }
    if (target) target->activate(ic);
  }
  msime::linux_host::ShuangpinKeymapX11Surface &shuangpinKeymap() {
    if (!shuangpin_keymap_) shuangpin_keymap_ = std::make_unique<msime::linux_host::ShuangpinKeymapX11Surface>();
    return *shuangpin_keymap_;
  }
#endif
#ifdef MSIME_FCITX5_HINT_FONT
  // classicui 的面板只在画水杉主题时才有装饰图；此时把「中」「英」补到主题自己预留的宽度，
  // 装饰就不会被 OverlayClipMargin 从两侧切掉。第三方主题、没有装饰、以及 kimpanel（不画
  // classicui 主题）都保持原来的窄提示。宽度在写主题时算好，这里不读 theme.conf，也不逐字符
  // 重排，只在字体或 DPI 变化时重测一次。
  std::string modeHintLabel(const std::string &display, const std::string &label) {
    if (instance_->currentUI() != "classicui" || !hint_inputs_.active || hint_text_width_ <= 0) return label;
    const auto &font = hint_inputs_.font;
    if (font.empty()) return label;
    // Wayland 下 classicui 用 ForceWaylandDPI 排版；X11 与未设置时用 Pango 默认分辨率。
    double resolution = 0;
    if (display.rfind("wayland:", 0) == 0 && hint_inputs_.force_wayland_dpi > 0)
      resolution = hint_inputs_.force_wayland_dpi;
    if (resolution <= 0) resolution = fcitx_default_font_resolution();
    if (font != hint_measured_font_ || resolution != hint_measured_resolution_ ||
        hint_text_width_ != hint_measured_width_) {
      const int space = fcitx_measure_text(font, resolution, "\u3000");
      const std::array<std::string, 2> labels{{"中", "英"}};
      for (std::size_t index = 0; index < labels.size(); ++index) {
        const auto &text = labels[index];
        auto message = fcitx_pad_hint_label(text, hint_text_width_, fcitx_measure_text(font, resolution, text), space);
        // 一次最终校验：字体整形不保证空格严格线性，差一点就再补一个；结果也缓存。
        if (message.size() != text.size() && fcitx_measure_text(font, resolution, message) < hint_text_width_)
          message += "\u3000";
        hint_labels_[index] = std::move(message);
      }
      hint_measured_font_ = font;
      hint_measured_resolution_ = resolution;
      hint_measured_width_ = hint_text_width_;
    }
    return label == "中" ? hint_labels_[0] : label == "英" ? hint_labels_[1] : label;
  }
#endif
  // 告诉设置页经典界面画不画候选字体、配色和皮肤（见 candidates/CandidatePanelStatus.h）。每次主题同步都问一遍，因为用户随时可能在 fcitx5-configtool 里换界面或主题；答案变了才重写文件。主题选择取自落盘的 classicui.conf（read_classicui_theme_selection），不调 `getConfig()`，后者每次都扫描全部已装主题（#5988）。
  void publishCandidatePanelStatus() {
    namespace host = msime::linux_host;
    const auto file = host::candidate_panel_status_file(std::getenv("XDG_RUNTIME_DIR"));
    if (!file) return;
    const auto &ui = instance_->currentUI();
    // 只有经典界面在画时主题才有意义（fcitx_candidate_panel_limit）；kimpanel 等界面下不读它的配置。关闭自动生成时经典界面画的是用户维护的主题，配色和皮肤同样不生效，即使 Theme 仍指向水杉。
    bool replaceable = true;
    if (ui == "classicui" && !host::fcitx_theme_autogenerate_enabled(std::getenv("MSIME_FCITX5_THEME_AUTOGENERATE"))) {
      replaceable = false;
    } else if (ui == "classicui") {
      const auto selection = read_classicui_theme_selection();
      replaceable = host::fcitx_candidate_theme_drawn(selection.theme,
                                                      selection.dark_theme ? &*selection.dark_theme : nullptr);
    }
    const auto limit = host::fcitx_candidate_panel_limit(ui, replaceable);
    bool native = false;
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    // X11 的头文件把 None 定义成宏，这里按「没有限制名」判断。
    native = !host::candidate_panel_limit_name(limit) && ui == "classicui" && native_canvas_assets_ &&
        native_canvas_assets_->chrome &&
        host::candidate_session_is_x11(std::getenv("XDG_SESSION_TYPE"), std::getenv("DISPLAY"), std::getenv("WAYLAND_DISPLAY"));
#endif
    host::write_candidate_panel_status(*file, host::candidate_panel_status_document("fcitx5", limit, native));
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
      if (state->menuTarget()) focused.push_back(ic);
      state->close();
      state->clearPanel();
      return true;
    });
    refreshOptions();
    refreshTypingStatistics();
    // 「重启输入法服务」让下一次主题同步无条件重做，不论上次是接管了、不可接管还是写失败（见 applyCandidatePanelTheme）。
    candidate_theme_applied_.clear();
    candidate_theme_attempt_.clear();
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
    candidate_wheel_paging_sync_.mark_applied(*enabled);
    classicui_wheel_paging_ = *enabled;
  }
  bool candidateWheelPagingPending() const { return candidate_wheel_paging_sync_.pending(); }
  explicit FcitxEngine(fcitx::Instance *instance) : instance_(instance) {
    // 脚本插件在 fcitx5 守护进程里运行（本 addon 是它加载的共享库），在第一个会话之前打开；开发构建和 release 构建一样（msime_client.h）。
    msime_client_scripts_enable(true);
    // A library Fcitx5 kept loaded across an earlier engine's teardown keeps its globals; this engine writes off the loop again.
    fcitx_key_presses_shutting_down = false;
    refreshOptions();
    refreshTypingStatistics();
    try {
      startDictionarySync(readOptions());
    } catch (...) {
    }
    // 输入上下文属性名在整个 fcitx5 进程里唯一：两个版本的插件同时加载时，后注册的同名属性会失败、factory_ 拿不到槽位，第一次 propertyFor 就越界。名字随插件名，full 仍是 msimeState。
    if (!instance->inputContextManager().registerProperty(MSIME_EDITION_FCITX5_ADDON "State", &factory_))
      throw std::runtime_error("Fcitx5 input context property " MSIME_EDITION_FCITX5_ADDON "State is already registered");
    english_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-english-candidates", &instance->userInterfaceManager());
    input_mode_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-input-mode", &instance->userInterfaceManager());
    scheme_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-scheme", &instance->userInterfaceManager());
    shuangpin_profile_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-shuangpin-profile", &instance->userInterfaceManager());
    width_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-fullwidth", &instance->userInterfaceManager());
    nine_key_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-nine-key", &instance->userInterfaceManager());
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
    helpcode_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-helpcode", &instance->userInterfaceManager());
    mixed_english_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-mixed-english", &instance->userInterfaceManager());
    mixed_emoji_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-mixed-emoji", &instance->userInterfaceManager());
    mixed_kaomoji_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-mixed-kaomoji", &instance->userInterfaceManager());
    mixed_symbol_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-mixed-symbols", &instance->userInterfaceManager());
    local_unicode_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-unicode", &instance->userInterfaceManager());
    local_date_time_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-date-time", &instance->userInterfaceManager());
    local_quick_phrase_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-quick-phrase", &instance->userInterfaceManager());
    local_emoji_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-emoji", &instance->userInterfaceManager());
    local_kaomoji_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-kaomoji", &instance->userInterfaceManager());
    local_super_jianpin_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-super-jianpin", &instance->userInterfaceManager());
    local_temporary_english_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-temporary-english", &instance->userInterfaceManager());
    local_temporary_japanese_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-temporary-japanese", &instance->userInterfaceManager());
    local_chaizi_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-chaizi", &instance->userInterfaceManager());
    local_expression_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-expression", &instance->userInterfaceManager());
    local_command_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-command", &instance->userInterfaceManager());
    local_mention_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-local-mention", &instance->userInterfaceManager());
    english_gloss_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-english-gloss", &instance->userInterfaceManager());
    word_character_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-word-character", &instance->userInterfaceManager());
    number_row_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-number-row", &instance->userInterfaceManager());
    shuangpin_preedit_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-shuangpin-preedit", &instance->userInterfaceManager());
    wubi_code_hint_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-wubi-code-hint", &instance->userInterfaceManager());
    wubi_auto_commit_unique_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-wubi-auto-commit-unique", &instance->userInterfaceManager());
    helpcode_schema_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-helpcode-schema", &instance->userInterfaceManager());
    // 辅助码方案 lists the built-in schemas rather than stepping through them on each click, as 输入方案 does and as the IBus menu always has.
    helpcode_schema_action_.setMenu(&helpcode_schema_menu_);
    for (const auto &[value, label] : msime::linux_host::kHelpcodeSchemaNames) {
      helpcode_schema_items_.push_back(std::make_unique<FcitxHelpcodeSchemaItemAction>(&factory_, value, label));
      helpcode_schema_items_.back()->registerAction(MSIME_EDITION_FCITX5_ADDON "-helpcode-schema-" + std::string(value), &instance->userInterfaceManager());
      helpcode_schema_menu_.addAction(helpcode_schema_items_.back().get());
    }
    maintenance_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-candidate-tools", &instance->userInterfaceManager());
    clipboard_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-clipboard", &instance->userInterfaceManager());
    clipboard_history_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-clipboard-history", &instance->userInterfaceManager());
    cloud_clipboard_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-cloud-clipboard", &instance->userInterfaceManager());
    emoji_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-emoji", &instance->userInterfaceManager());
    emoji_search_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-emoji-search", &instance->userInterfaceManager());
    emoji_category_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-emoji-category", &instance->userInterfaceManager());
    emoji_group_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-emoji-group", &instance->userInterfaceManager());
    voice_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-voice", &instance->userInterfaceManager());
    voice_cancel_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-voice-cancel", &instance->userInterfaceManager());
    desktop_tools_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-desktop-tools", &instance->userInterfaceManager());
    toolbar_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-toolbar", &instance->userInterfaceManager());
    traditional_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-traditional", &instance->userInterfaceManager());
    chinese_punctuation_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-chinese-punctuation", &instance->userInterfaceManager());
    paired_punctuation_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-paired-punctuation", &instance->userInterfaceManager());
    smart_punctuation_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-smart-punctuation", &instance->userInterfaceManager());
    smart_punctuation_repeat_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-smart-punctuation-repeat", &instance->userInterfaceManager());
    candidate_layout_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-candidate-layout", &instance->userInterfaceManager());
    candidate_theme_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-candidate-theme", &instance->userInterfaceManager());
    global_theme_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-global-theme", &instance->userInterfaceManager());
    global_theme_action_.setMenu(&global_theme_menu_);
    if (const auto themes = themeCatalog().find("themes"); themes != themeCatalog().end() && themes->is_array())
      for (const auto &theme : *themes) {
        if (!theme.is_object() || !theme.value("id", Json()).is_string() || !theme.value("title", Json()).is_string())
          continue;
        const auto id = theme.at("id").get<std::string>();
        global_theme_items_.push_back(
            std::make_unique<FcitxGlobalThemeItemAction>(&factory_, id, theme.at("title").get<std::string>()));
        // Registered so the D-Bus menus (StatusNotifierItem, kimpanel), which address items by their registered id, can trigger them too.
        global_theme_items_.back()->registerAction(MSIME_EDITION_FCITX5_ADDON "-global-theme-" + id, &instance->userInterfaceManager());
        global_theme_menu_.addAction(global_theme_items_.back().get());
      }
    candidate_page_size_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-candidate-page-size", &instance->userInterfaceManager());
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
    candidate_page_size_menu_.addAction(&candidate_page_size10_);
    learning_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-learning", &instance->userInterfaceManager());
    frequency_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-frequency", &instance->userInterfaceManager());
    frequency_trigger_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-frequency-trigger", &instance->userInterfaceManager());
    frequency_step_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-frequency-step", &instance->userInterfaceManager());
    mode_scope_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-mode-scope", &instance->userInterfaceManager());
    candidate_translation_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-candidate-translations", &instance->userInterfaceManager());
    sentence_translation_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-translate-sentence", &instance->userInterfaceManager());
    punctuation_lock_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-punctuation-lock", &instance->userInterfaceManager());
    translation_language_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-translation-language", &instance->userInterfaceManager());
    cloud_candidates_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-cloud-candidates", &instance->userInterfaceManager());
    ai_candidates_action_.registerAction(MSIME_EDITION_FCITX5_ADDON "-ai-candidates", &instance->userInterfaceManager());
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
    // 手写、屏幕键盘、表情面板、云词库、云剪贴板和「显示工具栏」与 macOS 一样挂在状态区顶层（arrangeStatusArea）；这里只剩 macOS 菜单里没有的入口。手写识别板只认汉字，不提供手写的版本（日文、越南文和藏文版，LinuxEdition.h 的 MSIME_EDITION_HANDWRITING 为 0）不列它，见 arrangeStatusArea。
    desktop_tools_menu_.addAction(&desktop_clipboard_action_);
    desktop_tools_menu_.addAction(&desktop_voice_action_);
    desktop_tools_menu_.addAction(&help_action_);
    desktop_tools_menu_.addAction(&feedback_action_);
    desktop_tools_menu_.addAction(&reload_service_action_);
    desktop_tools_menu_.addAction(&voice_enabled_action_);
    desktop_tools_menu_.addAction(&preference_save_retry_action_);
    // The D-Bus menus (StatusNotifierItem, kimpanel) address entries by their registered name and skip an unregistered one, so every entry of the menus below is registered, separators included.
    for (auto [action, name] : std::initializer_list<std::pair<fcitx::Action *, const char *>>{
             {&handwriting_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-handwriting"},
             {&keyboard_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-keyboard"},
             {&desktop_emoji_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-emoji"},
             {&desktop_clipboard_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-clipboard"},
             {&desktop_voice_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-voice"},
             {&cloud_dictionary_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-cloud-dictionary"},
             {&vocabulary_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-vocabulary"},
             {&desktop_cloud_clipboard_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-cloud-clipboard"},
             {&help_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-help"},
             {&feedback_action_, MSIME_EDITION_FCITX5_ADDON "-desktop-feedback"},
             {&reload_service_action_, MSIME_EDITION_FCITX5_ADDON "-reload-service"},
             {&toolbar_enabled_action_, MSIME_EDITION_FCITX5_ADDON "-toolbar-enabled"},
             {&voice_enabled_action_, MSIME_EDITION_FCITX5_ADDON "-voice-enabled"},
             {&preference_save_retry_action_, MSIME_EDITION_FCITX5_ADDON "-preference-save-retry"},
             {&promotion_action_, MSIME_EDITION_FCITX5_ADDON "-promotion"},
             {&website_action_, MSIME_EDITION_FCITX5_ADDON "-website"},
             {&toolbar_separator_, MSIME_EDITION_FCITX5_ADDON "-toolbar-separator"},
             {&dictionary_action_, MSIME_EDITION_FCITX5_ADDON "-dictionary"},
             {&settings_action_, MSIME_EDITION_FCITX5_ADDON "-settings"},
             {&about_action_, MSIME_EDITION_FCITX5_ADDON "-about"},
             {&scheme_quanpin_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-quanpin"},
             {&scheme_shuangpin_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-shuangpin"},
             {&scheme_wubi_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-wubi"},
             {&scheme_japanese_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-japanese"},
             {&scheme_korean_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-korean"},
             {&scheme_cantonese_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-cantonese"},
             {&scheme_zhuyin_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-zhuyin"},
             {&scheme_stroke_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-stroke"},
             {&scheme_vietnamese_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-vietnamese"},
             {&scheme_tibetan_action_, MSIME_EDITION_FCITX5_ADDON "-scheme-tibetan"},
             {&input_group_action_, MSIME_EDITION_FCITX5_ADDON "-group-input"},
             {&input_group_separator_, MSIME_EDITION_FCITX5_ADDON "-group-input-separator"},
             {&punctuation_group_action_, MSIME_EDITION_FCITX5_ADDON "-group-punctuation"},
             {&punctuation_group_separator_, MSIME_EDITION_FCITX5_ADDON "-group-punctuation-separator"},
             {&candidate_group_action_, MSIME_EDITION_FCITX5_ADDON "-group-candidate"},
             {&candidate_group_separator_, MSIME_EDITION_FCITX5_ADDON "-group-candidate-separator"}})
      action->registerAction(name, &instance->userInterfaceManager());
    // 输入方案 lists the schemes rather than stepping through them on each click. Cantonese, Zhuyin and Stroke join it once a context has read runtime options naming their dictionaries (rebuildSchemeMenu).
    scheme_action_.setMenu(&scheme_menu_);
    rebuildSchemeMenu(nullptr, false, false, false);
    // 状态区顶层只放 macOS 菜单里的那些项（arrangeStatusArea），其余开关作为同一个动作收进三个子菜单。
    input_group_action_.setMenu(&input_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &shuangpin_profile_action_, &helpcode_action_, &helpcode_schema_action_,
             &mixed_english_action_, &mixed_symbol_action_, &mixed_emoji_action_, &mixed_kaomoji_action_, &english_gloss_action_,
             &cloud_candidates_action_, &ai_candidates_action_, &number_row_action_, &word_character_action_,
             &mode_scope_action_, &clipboard_history_action_, &input_group_separator_, &local_unicode_action_,
             &local_date_time_action_, &local_quick_phrase_action_, &local_emoji_action_, &local_kaomoji_action_,
             &local_super_jianpin_action_, &local_temporary_english_action_, &local_temporary_japanese_action_,
             &local_chaizi_action_, &local_expression_action_, &local_command_action_, &local_mention_action_}) {
      // 不带临时日文的版本（五笔版）不列这个本地模式：宿主库在这些版本里总是把它关掉，列出来也打不开。
      if (!MSIME_EDITION_TEMPORARY_JAPANESE && action == &local_temporary_japanese_action_) continue;
      // 不带双拼的版本（五笔版）不列双拼键位方案。
      if (action == &shuangpin_profile_action_ && !msime::linux_host::edition_offers_scheme("shuangpin")) continue;
      input_group_menu_.addAction(action);
    }
    punctuation_group_action_.setMenu(&punctuation_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &paired_punctuation_action_, &smart_punctuation_action_, &smart_punctuation_repeat_action_,
             &punctuation_lock_action_, &punctuation_group_separator_, &sentence_translation_action_,
             &translation_language_action_})
      punctuation_group_menu_.addAction(action);
    candidate_group_action_.setMenu(&candidate_group_menu_);
    for (auto *action : std::initializer_list<fcitx::Action *>{
             &candidate_layout_action_, &candidate_page_size_action_, &candidate_theme_action_,
             &shuangpin_preedit_action_, &wubi_code_hint_action_, &wubi_auto_commit_unique_action_,
             &candidate_group_separator_, &learning_action_,
             &frequency_action_, &frequency_trigger_action_, &frequency_step_action_}) {
      // 双拼原始预编辑只在双拼下生效、五笔剩余编码和四码唯一自动上屏只在五笔下生效，本版本没有那个方案就不列。
      if (action == &shuangpin_preedit_action_ && !msime::linux_host::edition_offers_scheme("shuangpin")) continue;
      if ((action == &wubi_code_hint_action_ || action == &wubi_auto_commit_unique_action_) &&
          !msime::linux_host::edition_offers_scheme("wubi")) continue;
      candidate_group_menu_.addAction(action);
    }
    emoji_action_.setMenu(&emoji_menu_);
    emoji_menu_.addAction(&emoji_item1_);
    emoji_menu_.addAction(&emoji_item2_);
    emoji_menu_.addAction(&emoji_item3_);
    emoji_menu_.addAction(&emoji_item4_);
    emoji_menu_.addAction(&emoji_item5_);
    emoji_menu_.addAction(&emoji_previous_action_);
    emoji_menu_.addAction(&emoji_next_action_);
    maintenance_action_.setMenu(&maintenance_menu_);
    // 与候选右键操作同一顺序：置顶、固定排位、取消固定、删除。
    maintenance_menu_.addAction(&pin_action_);
    maintenance_menu_.addAction(&fix1_action_);
    maintenance_menu_.addAction(&fix2_action_);
    maintenance_menu_.addAction(&fix3_action_);
    maintenance_menu_.addAction(&fix4_action_);
    maintenance_menu_.addAction(&fix5_action_);
    maintenance_menu_.addAction(&clear_action_);
    maintenance_menu_.addAction(&remove_action_);
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    native_canvas_cursor_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextCursorRectChanged, fcitx::EventWatcherPhase::PostInputMethod,
        [this](fcitx::Event &event) {
          auto *ic = static_cast<fcitx::InputContextEvent &>(event).inputContext();
          auto *state = ic->propertyFor(&factory_);
          if (!state->native_canvas_) return;
          const auto rect = ic->cursorRect();
          if (!state->native_canvas_->move(rect.left(), rect.top(), rect.height())) {
            try { state->render(); } catch (...) { unavailable(*state); }
          }
        });
    native_canvas_ui_watch_ = instance->watchEvent(
        fcitx::EventType::UIChanged, fcitx::EventWatcherPhase::PostInputMethod,
        [this](fcitx::Event &) {
          instance_->inputContextManager().foreach([this](fcitx::InputContext *ic) {
            auto *state = ic->propertyFor(&factory_);
            if (state->session_ && ic->hasFocus()) {
              try { state->render(); } catch (...) { unavailable(*state); }
            }
            return true;
          });
        });
#endif
#ifdef MSIME_FCITX5_MODE_BADGE
    // 获得焦点后在等光标矩形的徽章，等到输入框报来新矩形就放（见 FcitxState::settleFocusInputModeHud）。报来的若仍是全 0 的矩形（还没有光标，见 mode_badge_caret_usable），接着等下一次报告或等满时间。
    mode_hud_cursor_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextCursorRectChanged, fcitx::EventWatcherPhase::PostInputMethod,
        [this](fcitx::Event &event) {
          auto *ic = static_cast<fcitx::InputContextEvent &>(event).inputContext();
          const auto &rect = ic->cursorRect();
          if (!msime::linux_host::mode_badge_caret_usable({rect.left(), rect.top(), rect.width(), rect.height()}))
            return;
          ic->propertyFor(&factory_)->settleFocusInputModeHud();
        });
#endif
#ifdef MSIME_LINUX_FLOATING_PANELS
    floating_toolbar_destroy_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextDestroyed, fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
          if (floating_toolbar_) floating_toolbar_->deactivate(static_cast<fcitx::InputContextEvent &>(event).inputContext());
        });
#endif
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
            // 会话照旧，但 Sensitive 和 NoSpellCheck 之间的切换也会改变字段属性；现在就报告，音乐不等下一个键。
            state->syncFieldTraits();
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
          // 离开客户端时上屏打开的韩文音节、注音转换、越南文单词或藏文音节串。失焦时 Fcitx5 会自己上屏客户端预编辑（或由客户端用 ClientUnfocusCommit 上屏），所以这里只上屏画在面板里的组字，即不支持预编辑的客户端的组字。
          if (state->commitsOnBlur() && !state->view_.value("editing_text", std::string{}).empty()) {
            if (!ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) {
              try { state->apply(state->focusSession(false)); } catch (...) {}
            } else {
              // The composition reaches the document through the preedit rather than through commitText, so it is counted here as typed text.
              state->recordTypingStatistics(state->view_.value("preedit", std::string{}), state->typingSource());
            }
          }
          state->rememberInputMode();
          state->ime_mode_chosen_ = false;
          state->mode_restore_pending_ = true;
          // 会话不在这里关：随后的 deactivate 把它留给托盘菜单，到下一次有上下文获得焦点再关（见 holdMenuContext）。
          try { state->leaveFocus(); } catch (...) { state->close(); }
          state->clearPanel();
        });
    // Any focused context counts, whichever input method it uses: the panels type through Fcitx5 itself once this addon is loaded.
    panel_focus_watch_ = instance->watchEvent(
        fcitx::EventType::InputContextFocusIn,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
          auto *focused = static_cast<fcitx::InputContextEvent &>(event).inputContext();
          // 任何上下文获得焦点都可能是换了程序，不论它用哪个输入法：离开有规则的程序再回来，规则要重新生效。
          enterAppRuleStay(focused->program());
          // 任何上下文获得焦点，之前留给托盘菜单的那个就交出去，不论新上下文用哪个输入法。
          releaseMenuContext(focused);
#ifdef MSIME_LINUX_FLOATING_PANELS
          // Fcitx5 默认每个上下文各记各的输入法：焦点移到用别的输入法的上下文时本插件只收到旧上下文的失焦，收不到切换输入法。这时收起悬浮工具栏并放开属主，与 macOS 的 deactivateForInputSourceSwitch 相同；不收起的话它会显示旧上下文的状态，而旧上下文刚刚交出了菜单，按钮点了也没有反应。用本输入法的上下文随后由 activate 接手。
          if (floating_toolbar_ && floating_toolbar_->owner() && instance_->inputMethodEngine(focused) != this)
            floating_toolbar_->deactivate(floating_toolbar_->owner());
#endif
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
  // Every detached worker runs addon code. Wait for the complete tracker before Fcitx5 can unload
  // the shared object; otherwise a provider reply that outlives this object can jump into unmapped
  // plugin code. Any provider shutdown delay is paid here, after input contexts
  // have stopped accepting events, instead of risking execution after dlclose.
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
    stopDictionarySync();
    fcitx_detached_jobs.wait_idle();
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
    auto root = msime::telemetry::default_directory();
    if (root.empty()) return;
    // 与 IBus 宿主相同，使用统计目录按版本分开（LinuxEdition.h）：full 仍是 $XDG_STATE_HOME/msime，其他版本是同级的 msime-<id>。
    root = root.parent_path() / MSIME_EDITION_TELEMETRY_DIRECTORY;
    msime::telemetry::install_crash_handlers();
    telemetry_job_ = detachedJob([this, host = msime::telemetry::Host{"linux", MSIME_LINUX_VERSION, root / "fcitx5", std::nullopt, preferences}] {
      {
        // 开始与停止在线程间线性化；迟到的启动不能在插件停止后重新武装。
        std::lock_guard guard(telemetry_lifecycle_);
        if (telemetry_stopped_) return Json();
        msime::telemetry::begin(host);
        telemetry_observer_.emplace();
      }
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
    {
      std::lock_guard guard(telemetry_lifecycle_);
      telemetry_stopped_ = true;
      telemetry_observer_.reset();
      msime::telemetry::end();
    }
    if (telemetry_job_.valid()) telemetry_job_.wait_for(std::chrono::seconds(2));
    msime::telemetry::remove_crash_handlers();
  }
  static constexpr uint64_t kTelemetryIntervalUs = 30ull * 60 * 1000000;
  std::mutex telemetry_lifecycle_;
  bool telemetry_stopped_ = false;
  std::optional<msime::telemetry::ConsentObserver> telemetry_observer_;
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
      if (!request.text.empty())
        ic->propertyFor(&factory_)->rememberSmartPunctuationCommit(request.text);
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
    // 获得焦点时 panel_focus_watch_ 已经交出；换到本输入法不经过它。
    releaseMenuContext(event.inputContext());
    auto *state = event.inputContext()->propertyFor(&factory_);
    refreshVocabularyDue(event.inputContext());
    arrangeStatusArea(event.inputContext());
    try {
      if (state->ensure()) {
        state->render();
        state->syncMusic();
        // 聚焦提示由独立偏好控制（#2589、#9154）；换到本输入法时已有 Fcitx5 自己的提示，密码和隐私输入框仍不提示。
        if (event.type() == fcitx::EventType::InputContextFocusIn)
          state->showInputModeHud(FcitxState::ModeHudTrigger::FocusEntered);
      }
    } catch (const OptionsNotConfigured &) { notConfigured(*state, true); }
    catch (...) { unavailable(*state); }
    // 工具栏与语音两项要看会话，在建会话之后才定。
    state->refreshToolbar();
    state->syncVoiceAction();
    // 悬浮工具栏的属主交给这个上下文（获得焦点或切换到本输入法），与 macOS 的 activateServer: 相同。
    state->syncFloatingToolbar(true);
    state->publishInputStatus(true);
    noticeReplacedAddon(*state);
  }
  void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
#ifdef MSIME_FCITX5_MODE_BADGE
    state->cancelFocusInputModeHud();
    dismissModeBadge(event.inputContext());
#endif
    // 切换到别的输入法时，把打开的韩文音节、注音转换、越南文单词或藏文音节串作为文字结束，因为它已经是用户写下的内容。失焦不需要在这里处理：那时 Fcitx5 会自己上屏客户端预编辑（见 focus_watch_）。
    if (event.type() == fcitx::EventType::InputContextSwitchInputMethod && state->session_ &&
        state->commitsOnBlur() && !state->view_.value("editing_text", std::string{}).empty()) {
      try { state->command(MSIME_FINISH_COMPOSITION); } catch (...) {}
    }
    // 失焦时会话和状态动作留给托盘菜单（见 holdMenuContext 与 README「状态区」一段）；换输入法等其余情况照旧关掉、清空。
    if (event.type() == fcitx::EventType::InputContextFocusOut && !state->restricted()) {
      holdMenuContext(event.inputContext());
    } else if (menu_context_ic_.get() == event.inputContext()) {
      releaseMenuContext(nullptr);
    } else {
      event.inputContext()->statusArea().clearGroup(fcitx::StatusGroup::InputMethod);
      state->close(); state->clearPanel();
    }
#ifdef MSIME_LINUX_FLOATING_PANELS
    // 失焦照 macOS 留着悬浮工具栏；换到别的输入法才收起并放开属主（deactivateForInputSourceSwitch）。放在上屏和关会话之后：上屏打开的组字要经 render 刷新工具栏，那时会话还在、焦点也还在，先放开的话它会把工具栏又接回来，换到别的输入法后工具栏还留在屏幕上。
    if (floating_toolbar_) {
      if (event.type() == fcitx::EventType::InputContextFocusOut) floating_toolbar_->focus_out(event.inputContext());
      else floating_toolbar_->deactivate(event.inputContext());
    }
#endif
    state->publishInputStatus(false);
  }
  // Fcitx5 的托盘（SNI/dbusmenu）列的是最近聚焦的上下文的状态动作，点下去也作用于它；在 Hyprland 上的 noctalia、waybar 等桌面，点托盘本身就让应用失焦。所以失焦的上下文留着会话，托盘读到的勾选是它真实的状态，点下的开关作用于它的会话并照常保存。同一时间只留一个，到有上下文获得焦点时交出（releaseMenuContext）。只往有焦点的输入框里写的入口——语音、候选维护、剪贴板、云剪贴板、表情——这时点了也没有地方落，先从状态区拿掉，获得焦点时 activate 再放回。
  void holdMenuContext(fcitx::InputContext *ic) {
    if (menu_context_ic_.get() != ic) releaseMenuContext(nullptr);
    menu_context_ic_ = ic->watch();
    refreshVocabularyDue(ic);
    // 失焦了，只往有焦点的输入框里写的那几项（focusOnlyActions 与语音项，以及「工具栏」子菜单里的语音和宣传）随之拿掉，顺带刷新状态区。
    rebuildToolbarMenu(ic);
    arrangeStatusArea(ic);
  }
  // 读一次今日待复习，记在这个上下文上，syncVocabularyEntry 和菜单里的文字都用它。只在获得焦点和失焦留给托盘时调（Hyprland 这类桌面上点托盘本身就让应用失焦，这时读到的才是用户点开托盘时的数）：要取进度文件锁、解析选中的一本书，在 Fcitx5 的主线程上同步进行，所以不放在按键或刷新状态区的路径上；从没开过背单词时不取锁也不读书。那两个时刻都没有在组字，受限（密码框）的上下文不列这一行。
  void refreshVocabularyDue(fcitx::InputContext *ic) {
    auto *state = ic->propertyFor(&factory_);
    state->vocabulary_due_.reset();
    if (state->restricted()) return;
    try {
      const bool composing = state->view_.is_object() && state->composingOrCandidates();
      state->vocabulary_due_ = msime::linux_host::vocabulary_due_label(readOptions(), composing);
    } catch (...) {}
  }
  // 「背单词」一行在「桌面工具」子菜单里的有无：这个上下文今天有卡在等、没有在组字时才列。不放状态区顶层，因为 kimpanel 把顶层动作画成面板上一直在的按钮，那就成了常驻的角标，而这一行只该在用户点开菜单时看得到。子菜单对象是各上下文共用的，跟着最近调用的那个上下文走：arrangeStatusArea 在获得焦点和失焦留给托盘时调，render 在开始或结束组字时调。返回子菜单是否变了。
  bool syncVocabularyEntry(fcitx::InputContext *ic) {
    const auto *state = ic->propertyFor(&factory_);
    const bool listed = state->vocabulary_due_.has_value() &&
                        !(state->view_.is_object() && state->composingOrCandidates());
    if (listed == vocabulary_listed_) return false;
    if (listed) desktop_tools_menu_.insertAction(&desktop_clipboard_action_, &vocabulary_action_);
    else desktop_tools_menu_.removeAction(&vocabulary_action_);
    vocabulary_listed_ = listed;
    return true;
  }
  // 只往有焦点的输入框里写的那几项；托盘留着失焦的上下文时不列它们。语音项另按会话和语音开关决定，见 arrangeStatusArea。
  std::array<fcitx::Action *, 8> focusOnlyActions() {
    return {&voice_cancel_action_, &maintenance_action_, &clipboard_action_, &cloud_clipboard_action_,
            &emoji_action_, &emoji_search_action_, &emoji_category_action_, &emoji_group_action_};
  }
  // 按 macOS 输入法菜单的顺序排状态区（InputController.mm 的 `menu`）：中文输入、英文候选模式；繁体输出、全角字符、中文标点、显示译文；输入方案、主题；显示工具栏（与紧跟的「工具栏」子菜单）、表情面板、云剪贴板、云词库、屏幕键盘、手写、语音；设置、关于。macOS 的「中文输入」「英文输入」是一对单选，这里仍是一个开关（勾选即中文），它就是 Shift 切换的那个状态。macOS 菜单之后才是 Linux 独有的入口：词库…、随会话变化的候选操作、九键拼音、剪贴板和表情选择，再是三个选项子菜单和「桌面工具」。kimpanel 把状态动作原样列出，分隔线在那里只会是空白项，所以这里不放分隔线。
  //
  // 每一项都按顺序重新 addAction 一遍：已在状态区里的动作会被移到末尾，所以按这个顺序加完，状态区就是这个顺序；不该出现的移除。工具栏和语音两项要看会话，建好会话后 refreshToolbar、syncVoiceAction 会再排一次。
  void arrangeStatusArea(fcitx::InputContext *ic) {
    auto *state = ic->propertyFor(&factory_);
    auto &status = ic->statusArea();
    const bool focused = ic->hasFocus();
    const std::pair<fcitx::Action *, bool> order[] = {
        {&input_mode_action_, true}, {&english_action_, true},
        {&traditional_action_, true}, {&width_action_, true}, {&chinese_punctuation_action_, true},
        {&candidate_translation_action_, true},
        {&scheme_action_, true}, {&global_theme_action_, true},
        {&toolbar_enabled_action_, true}, {&toolbar_action_, toolbarEnabled(ic)},
        {&desktop_emoji_action_, true}, {&desktop_cloud_clipboard_action_, true}, {&cloud_dictionary_action_, true},
        {&keyboard_action_, true}, {&handwriting_action_, MSIME_EDITION_HANDWRITING != 0},
        // 语音要往有焦点的输入框里写，失焦后托盘上不列它。
        {&voice_action_, state->session_ && state->voice_enabled_ && focused}, {&voice_cancel_action_, focused},
        {&settings_action_, true}, {&about_action_, true},
        {&dictionary_action_, true}, {&maintenance_action_, focused}, {&nine_key_action_, true},
        {&clipboard_action_, focused}, {&cloud_clipboard_action_, focused}, {&emoji_action_, focused},
        {&emoji_search_action_, focused}, {&emoji_category_action_, focused}, {&emoji_group_action_, focused},
        {&input_group_action_, true}, {&punctuation_group_action_, true}, {&candidate_group_action_, true},
        {&desktop_tools_action_, true}};
    for (const auto &[action, present] : order) {
      if (present) status.addAction(fcitx::StatusGroup::InputMethod, action);
      else status.removeAction(action);
    }
    syncVocabularyEntry(ic);
    ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
  }
  // 留给托盘的上下文真正关掉会话。获得焦点的不是它时，连它的状态动作一起拿掉，托盘从此列新聚焦的那个。
  void releaseMenuContext(fcitx::InputContext *focused) {
    auto *held = menu_context_ic_.get();
    menu_context_ic_.unwatch();
    if (!held) return;
    auto *state = held->propertyFor(&factory_);
    state->close();
    state->clearPanel();
    if (held != focused) {
      held->statusArea().clearGroup(fcitx::StatusGroup::InputMethod);
      held->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
    }
  }
  void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
    auto *state = event.inputContext()->propertyFor(&factory_);
    state->cancelModifierTaps();
    state->backspace_hold_.reset();
    state->caps_lock_tap_.reset();
    state->caps_lock_down_ = false;
    state->toggle_chord_held_ = FcitxKey_None;
    state->pending_caret_.clear();
    state->caret_forward_event_.reset();
    // 为不支持预编辑的客户端画在面板里的韩文音节、注音转换、越南文单词或藏文音节串只存在于这里，所以重置时把它写出去，而不是丢掉用户已经打的字（失焦时的同一规则见 focus_watch_）。
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
#ifdef MSIME_LINUX_FLOATING_PANELS
    // 每一次按下都让空闲隐藏的工具栏回来并重新计时，与 macOS 的 wakeForInputDelegate: 相同。
    if (floating_toolbar_ && !event.isRelease()) floating_toolbar_->note_input(event.inputContext());
#endif
    state->noteCapsLock(event.rawKey().states().test(fcitx::KeyState::CapsLock));
    try {
      if (state->ensure()) {
#ifdef MSIME_FCITX5_MODE_BADGE
        if (!event.isRelease()) state->cancelFocusInputModeHud();
#endif
        state->syncSessionCapsLock();
        state->countKeyPress(event);
        const auto comboBefore = state->typing_combo_;
        state->keyDown(event);
        if (state->key(event)) event.filterAndAccept();
        else {
          if (!event.isRelease()) state->forgetSmartPunctuationShadow();
          state->countPassthroughKey(event);
        }
        state->keySettled(comboBefore);
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
    // KWin 启动的 Fcitx5 没有重启命令（见 ReplacedProgram.h 的 kwin_launched），只能注销或在系统设置里重新选一次虚拟键盘。
    const auto restart = msime::linux_host::fcitx5_restart_command();
    const std::string how = restart ? "执行 " + *restart + " 或注销后重新登录" : "注销后重新登录";
    state.ic_.inputPanel().setAuxUp(fcitx::Text(current == ProgramFileState::Replaced
        ? MSIME_EDITION_DISPLAY_NAME "已升级：" + how + "即可使用新版本"
        : MSIME_EDITION_DISPLAY_NAME "已卸载：" + how + "即可完成卸载"));
    state.ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    // 升级时另发一条桌面通知，有重启命令时带「立即重启 Fcitx5」按钮，用户不必自己去敲命令（见 msime-linux-first-run-guide 的 upgraded）。卸载时脚本可能已经不在，只留辅助行。
    if (current == ProgramFileState::Replaced) {
      const auto guide = std::string(MSIME_BINDIR "/") + std::string(msime::linux_host::kFirstRunGuideProgram);
      if (access(guide.c_str(), X_OK) == 0) {
        if (restart)
          fcitx::startProcess({guide, "--host", "fcitx5", "--reason", "upgraded", "--restart-unit",
                               msime::linux_host::current_user_service_unit().value_or(std::string())});
        else
          fcitx::startProcess({guide, "--host", "fcitx5", "--reason", "upgraded", "--launched-by", "kwin"});
      }
    }
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
  // 使用运行库版本，旧开发库构建的包在新版 classicui 上也要写磅，避免序号被放大。
  msime::linux_host::CandidateFontSync candidate_font_sync_{
      msime::linux_host::fcitx_candidate_font_unit(fcitx::Instance::version())};
  // classicui 属于整个插件，已见的主题选择也共用；不随某个上下文关闭而清空。
  Json candidate_theme_selection_;
  // applyCandidatePanelTheme 上一次接管写好时的主题输入。
  std::string candidate_theme_applied_;
  std::filesystem::path candidate_canvas_source_;
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  std::shared_ptr<msime::linux_host::NativeCanvasAssets> native_canvas_assets_;
  std::vector<std::filesystem::path> native_canvas_sources_;
  Json native_canvas_palette_ = Json::object();
  bool native_canvas_owned_ = false;
  // 首行 logo（show_app_logo 打开时），随主题输入一起重读。
  msime::linux_host::NativeCanvasImage native_canvas_logo_;
  // 桌面的「减弱动态效果」，每次画原生画布时重读（GSettings 走内存映射，KDE 的文件按戳重读，都很便宜）。
  msime::linux_host::ReduceMotionReader reduce_motion_reader_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> native_canvas_cursor_watch_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> native_canvas_ui_watch_;
#endif
  std::chrono::steady_clock::time_point candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point::max();
  // 还没接管成功时，上一次尝试所见的主题输入与经典界面落盘的选择；写主题失败时到 candidate_theme_retry_at_ 再试一次，其他情况要等它们变化。
  std::string candidate_theme_attempt_;
  std::chrono::steady_clock::time_point candidate_theme_retry_at_;
  static constexpr auto kCandidateThemeRetry = std::chrono::seconds(10);
  // 提示要撑到的文本宽度（逻辑单位），写主题时算好；0 表示不加宽。
  int hint_text_width_ = 0;
  FcitxHintInputs hint_inputs_;
  // 上一次重读经典界面现值时落盘配置的样子（见 read_classicui_hint_stamp）：没变就不必再读一次完整配置。
  std::string classicui_hint_stamp_;
#ifdef MSIME_FCITX5_HINT_FONT
  // 字体、分辨率或装饰宽度变化时测量并缓存完整提示，重复切换不再分配 Pango 对象。
  std::string hint_measured_font_;
  double hint_measured_resolution_ = 0;
  int hint_measured_width_ = 0;
  std::array<std::string, 2> hint_labels_;
#endif
  msime::linux_host::CandidateWheelPagingSync candidate_wheel_paging_sync_;
  // 经典界面现在的 WheelForPaging（见 read_classicui_wheel_for_paging），原生画布按它决定滚轮翻不翻页；Fcitx5 的缺省是开。
  bool classicui_wheel_paging_ = true;
  // Last appearance the addon-wide probe reported; see stepSystemTheme.
  bool system_dark_ = false;
  std::shared_future<Json> system_theme_job_;
#ifdef MSIME_LINUX_FLOATING_PANELS
  // 声明在 factory_ 之前，所以比各输入上下文的 FcitxState 活得久：上下文在插件卸载时析构，那时它们可能还要收起键位图。
  std::unique_ptr<msime::linux_host::FloatingToolbar> floating_toolbar_;
  bool floating_toolbar_unavailable_ = false;
  std::unique_ptr<msime::linux_host::ShuangpinKeymapX11Surface> shuangpin_keymap_;
#endif
  fcitx::FactoryFor<FcitxState> factory_{[this](fcitx::InputContext &ic) {
    return new FcitxState(ic, this, instance_->eventLoop(), system_dark_);
  }};
  std::unique_ptr<fcitx::EventSourceTime> system_theme_timer_;
#ifdef MSIME_FCITX5_MODE_BADGE
  std::unique_ptr<msime::linux_host::ModeBadgeSurface> mode_badge_;
  bool mode_badge_unavailable_ = false;
  std::unique_ptr<fcitx::EventSourceTime> mode_badge_timer_;
  int mode_badge_fade_step_ = 0;
  // 屏幕上那块徽章是为哪个输入上下文显示的，见 dismissModeBadge。
  fcitx::TrackableObjectReference<fcitx::InputContext> mode_badge_owner_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> mode_hud_cursor_watch_;
#endif
#ifdef MSIME_LINUX_FLOATING_PANELS
  // 工具栏的属主是输入上下文本身，上下文销毁时放开它。
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> floating_toolbar_destroy_watch_;
#endif
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> capability_watch_;
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> focus_watch_;
  // 失焦后留给托盘菜单的那个上下文，见 holdMenuContext；上下文销毁时自动失效。
  fcitx::TrackableObjectReference<fcitx::InputContext> menu_context_ic_;
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
  FcitxSchemeItemAction scheme_japanese_action_{&factory_, 3, "日语"};
  FcitxSchemeItemAction scheme_korean_action_{&factory_, 4, "韩语"};
  FcitxSchemeItemAction scheme_cantonese_action_{&factory_, 5, "粤拼"};
  FcitxSchemeItemAction scheme_zhuyin_action_{&factory_, 6, "注音"};
  FcitxSchemeItemAction scheme_vietnamese_action_{&factory_, 7, "越南语"};
  FcitxSchemeItemAction scheme_tibetan_action_{&factory_, 8, "藏文"};
  FcitxSchemeItemAction scheme_stroke_action_{&factory_, 9, "笔画"};
  // The entries scheme_menu_ holds, in menu order, and whether Cantonese, Zhuyin and Stroke were among them when it was last built.
  std::vector<fcitx::Action *> scheme_menu_entries_;
  std::optional<std::tuple<bool, bool, bool>> scheme_menu_languages_;
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
  FcitxMixedCandidateAction mixed_symbol_action_{&factory_, "symbols", "混合符号"};
  FcitxLocalModeAction local_unicode_action_{&factory_, "unicode", "Unicode（U 模式）"};
  FcitxLocalModeAction local_date_time_action_{&factory_, "date_time", "日期时间（T 模式）"};
  FcitxLocalModeAction local_quick_phrase_action_{&factory_, "quick_phrase", "快捷短语（K 模式）"};
  FcitxLocalModeAction local_emoji_action_{&factory_, "emoji", "Emoji（E 模式）"};
  FcitxLocalModeAction local_kaomoji_action_{&factory_, "kaomoji", "颜文字（M 模式）"};
  FcitxLocalModeAction local_super_jianpin_action_{&factory_, "super_jianpin", "超级简拼（J 模式）"};
  FcitxLocalModeAction local_temporary_english_action_{&factory_, "temporary_english", "临时英文（Y 模式）"};
  FcitxLocalModeAction local_temporary_japanese_action_{&factory_, "temporary_japanese", "临时日文（R 模式）"};
  FcitxLocalModeAction local_chaizi_action_{&factory_, "chaizi", "拆字（C 模式）"};
  FcitxLocalModeAction local_expression_action_{&factory_, "expression", "计算与数字（V 模式）"};
  FcitxLocalModeAction local_command_action_{&factory_, "command", "指令（/ 模式）"};
  FcitxLocalModeAction local_mention_action_{&factory_, "mention", "名单（@ 模式）"};
  FcitxEnglishGlossAction english_gloss_action_{&factory_};
  FcitxWordCharacterAction word_character_action_{&factory_};
  FcitxNumberRowAction number_row_action_{&factory_};
  FcitxSchemeBooleanAction shuangpin_preedit_action_{&factory_, FcitxSchemeBooleanAction::Kind::ShuangpinPreedit};
  FcitxSchemeBooleanAction wubi_code_hint_action_{&factory_, FcitxSchemeBooleanAction::Kind::WubiCodeHint};
  FcitxSchemeBooleanAction wubi_auto_commit_unique_action_{&factory_, FcitxSchemeBooleanAction::Kind::WubiAutoCommitUnique};
  FcitxHelpcodeSchemaAction helpcode_schema_action_{&factory_};
  fcitx::Menu helpcode_schema_menu_;
  std::vector<std::unique_ptr<FcitxHelpcodeSchemaItemAction>> helpcode_schema_items_;
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
  void rebuildSchemeMenu(fcitx::InputContext *ic, bool cantonese, bool zhuyin, bool stroke);
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
  FcitxCandidatePageSizeItemAction candidate_page_size10_{&factory_, 10};
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
  FcitxMaintenanceAction remove_action_{&factory_, 2, msime::linux_host::candidate_remove_label};
  FcitxMaintenanceAction fix1_action_{&factory_, 11, msime::linux_host::candidate_fix_label(1).c_str()};
  FcitxMaintenanceAction fix2_action_{&factory_, 12, msime::linux_host::candidate_fix_label(2).c_str()};
  FcitxMaintenanceAction fix3_action_{&factory_, 13, msime::linux_host::candidate_fix_label(3).c_str()};
  FcitxMaintenanceAction fix4_action_{&factory_, 14, msime::linux_host::candidate_fix_label(4).c_str()};
  FcitxMaintenanceAction fix5_action_{&factory_, 15, msime::linux_host::candidate_fix_label(5).c_str()};
  FcitxMaintenanceAction clear_action_{&factory_, 20, msime::linux_host::candidate_clear_fix_label};
  fcitx::Menu desktop_tools_menu_;
  FcitxDesktopPanelAction handwriting_action_{&factory_, "handwriting", msime::linux_host::menu_label::kHandwriting};
  FcitxDesktopPanelAction keyboard_action_{&factory_, "keyboard", msime::linux_host::menu_label::kScreenKeyboard};
  FcitxDesktopPanelAction desktop_emoji_action_{&factory_, "emoji", msime::linux_host::menu_label::kEmojiPanel};
  FcitxDesktopPanelAction desktop_clipboard_action_{&factory_, "clipboard", msime::linux_host::menu_label::kLocalClipboard};
  FcitxDesktopPanelAction desktop_voice_action_{&factory_, "voice", msime::linux_host::menu_label::kVoicePanel};
  FcitxDesktopPanelAction cloud_dictionary_action_{&factory_, "cloud-dictionary", msime::linux_host::menu_label::kCloudDictionary};
  FcitxVocabularyAction vocabulary_action_{&factory_};
  // vocabulary_action_ 此刻是否在 desktop_tools_menu_ 里（syncVocabularyEntry）。
  bool vocabulary_listed_ = false;
  FcitxDesktopPanelAction desktop_cloud_clipboard_action_{&factory_, "cloud-clipboard", msime::linux_host::menu_label::kCloudClipboard};
  FcitxDesktopPanelAction dictionary_action_{&factory_, "dictionary", msime::linux_host::menu_label::kDictionary};
  FcitxDesktopPanelAction settings_action_{&factory_, "settings", msime::linux_host::menu_label::kSettings};
  FcitxToolbarAction toolbar_action_;
  fcitx::Menu toolbar_menu_;
  // What is currently in the submenu, so a rebuild removes exactly what it added.
  std::vector<fcitx::Action *> toolbar_entries_;
  bool toolbarEnabled(fcitx::InputContext *ic);
  void rebuildToolbarMenu(fcitx::InputContext *ic);
  FcitxDesktopPanelAction about_action_{&factory_, "about", msime::linux_host::menu_label::kAbout};
  FcitxDesktopPanelAction help_action_{&factory_, "help", msime::linux_host::menu_label::kHelp};
  FcitxDesktopPanelAction feedback_action_{&factory_, "feedback", msime::linux_host::menu_label::kFeedback};
  FcitxToolbarEnabledAction toolbar_enabled_action_{&factory_};
  FcitxUtilityAction promotion_action_{&factory_, FcitxUtilityAction::Kind::Promotion};
  FcitxUtilityAction website_action_{&factory_, FcitxUtilityAction::Kind::Website};
  FcitxMenuSeparatorAction toolbar_separator_;
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

bool FcitxState::menuTarget() const {
  return ic_.hasFocus() || (engine_ && engine_->menu_context_ic_.get() == &ic_);
}

// Defined here rather than in the class body because FcitxEngine is only forward-declared there, and the voice action it owns cannot be named until the definition above.
void FcitxState::syncVoiceAction() {
  if (!engine_) return;
  // 语音项的有无与位置都由 arrangeStatusArea 决定。
  engine_->arrangeStatusArea(&ic_);
}

// 快捷键和状态区动作都在进程内重置，见 FcitxEngine::resetSessions。只有菜单作用得到的（有焦点，或失焦后托盘仍指向的）、非受限、非隐私的上下文能发起。
bool FcitxState::reloadService() {
  if (!engine_ || !menuTarget() || restricted() || privateInput()) return false;
  engine_->resetSessions();
  return true;
}

void FcitxState::syncCandidatePanelFont() {
  if (!engine_) return;
  engine_->applyCandidatePanelFont(preferences_);
  engine_->applyCandidateWheelPaging(preferences_);
}

void FcitxState::retryCandidatePanelSettings() {
  if (!engine_ || !session_ || !ic_.hasFocus() || restricted()) return;
  if (engine_->candidatePanelFontPending()) engine_->applyCandidatePanelFont(preferences_);
  if (engine_->candidateWheelPagingPending()) engine_->applyCandidateWheelPaging(preferences_);
}

bool FcitxState::candidateThemeSelectionChanged(const Json &preferences) {
  if (!engine_) return false;
  const auto selection = msime::linux_host::candidate_theme_selection(preferences);
  const bool changed = !engine_->candidate_theme_selection_.is_null() &&
                       selection != engine_->candidate_theme_selection_;
  engine_->candidate_theme_selection_ = selection;
  return changed;
}

void FcitxState::syncCandidatePanelTheme(bool chosen) {
  if (engine_ && session_ && engine_->applyCandidatePanelTheme(preferences_, system_dark_, candidate_skin_document_, chosen)) {
    // 资源戳变化可能没有偏好或候选变更；原生窗口从当前视图重建，不沿用清空给桌面 UI 的数据。
#ifdef MSIME_FCITX5_NATIVE_CANVAS
    if (native_canvas_ && ic_.hasFocus()) render();
    else
#endif
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    // GTK 的客户端面板异步监视主题文件；当前这次更新可能先于重载。下一轮偏好轮询再刷新一次，
    // 只重画当前面板，不保存候选快照；失焦、关闭与之后的换肤都会取消或替换这次刷新。
    candidate_theme_refresh_at_.reset();
    if (ic_.capabilityFlags().test(fcitx::CapabilityFlag::ClientSideInputPanel))
      candidate_theme_refresh_at_ = std::chrono::steady_clock::now() + std::chrono::milliseconds(250);
  } else if (candidate_theme_refresh_at_ && std::chrono::steady_clock::now() >= *candidate_theme_refresh_at_) {
    candidate_theme_refresh_at_.reset();
    if (session_ && ic_.hasFocus() && !restricted())
      ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
  }
  if (engine_) engine_->publishCandidatePanelStatus();
}

void FcitxState::refreshThemeMenu() {
  if (engine_) engine_->rebuildThemeMenu(&ic_);
}

void FcitxState::refreshSchemeMenu() {
  if (engine_) engine_->rebuildSchemeMenu(&ic_, schemeAvailable("cantonese"), schemeAvailable("zhuyin"),
                                          schemeAvailable("stroke"));
}

void FcitxState::refreshToolbar() {
  if (!engine_) return;
  engine_->rebuildToolbarMenu(&ic_);
  // 「工具栏」子菜单紧跟在「显示工具栏」之后，有无与位置由 arrangeStatusArea 决定。
  engine_->arrangeStatusArea(&ic_);
}

// 候选页下方的辅助行：页码、本地模式、候选预编辑显示拼音时的拼音、光标后编辑生效时的「候选编辑：光标后」，最后是打字连击（有的时候）。提示排在连击前面，连击数随按键变宽变窄时提示不跟着挪（CaretEditSuffix.h）。
std::string FcitxState::candidateAux() const {
  const auto page = msime::linux_host::strict_json_required_integer<int>(view_.at("page"));
  const auto page_count =
      msime::linux_host::strict_json_required_integer<int>(view_.at("page_count"));
  std::string aux = msime::linux_host::candidate_page_indicator(page, page_count);
  if (!preferences_.value("show_candidate_page_number", true)) aux.clear();
  const auto mode = view_.value("local_mode", std::string("none"));
  if (const char *modeLabel = msime::linux_host::candidate_local_mode_label(mode))
    aux += (aux.empty() ? "" : " · ") + std::string(modeLabel);
  if (preferences_.value("candidate_preedit_style", std::string("pinyin")) == "pinyin") {
    const auto candidatePreedit = view_.value("preedit", std::string{});
    if (!candidatePreedit.empty()) {
      const auto editing = view_.value("editing_text", std::string());
      const auto caret = std::min(editing.size(), msime::linux_host::strict_json_value(view_, "caret_position", editing.size()));
      const auto displayed = msime::linux_host::candidate_preedit_with_caret(
          candidatePreedit, editing, caret);
      if (!displayed.empty()) aux += (aux.empty() ? "" : " · ") + displayed;
    }
  }
  msime::linux_host::append_caret_edit_suffix_hint(aux, msime::linux_host::caret_edit_suffix(view_), " · ");
  const auto combo = msime::linux_host::typing_combo_label(typing_combo_);
  if (!combo.empty()) aux += (aux.empty() ? "" : " · ") + combo;
  return aux;
}

void FcitxState::render() {
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  // 展开的候选先对照当前视图：换代时重载完整候选，all_candidates 可能放出缓存尾部并推进代次、重读当前页（expandedEntries），必须在下面构建这一页的候选列表之前做完。
  if (!expanded_.empty() && !expandedStillApplies()) discardExpanded();
#endif
  if (engine_) {
    engine_->english_action_.update(&ic_);
    engine_->width_action_.update(&ic_);
    // 开始或结束组字时「背单词」一行随之从「桌面工具」子菜单里拿掉或放回；其余时候不动菜单。
    if (engine_->syncVocabularyEntry(&ic_)) ic_.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
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
  } else if (const auto conversion = msime::linux_host::conversion_preedit(view_); conversion.active) {
    // 整句改字：不管预编辑样式，内嵌显示已选的词加改好的整句，光标在焦点字前；焦点那一段高亮，其余加下划线。fcitx5 的光标和分段都按字节计。
    fcitx::Text preedit;
    const auto &text = conversion.text;
    if (conversion.caret_bytes > 0)
      preedit.append(text.substr(0, conversion.caret_bytes), fcitx::TextFormatFlag::Underline);
    if (conversion.focus_end_bytes > conversion.caret_bytes)
      preedit.append(text.substr(conversion.caret_bytes, conversion.focus_end_bytes - conversion.caret_bytes),
                     fcitx::TextFormatFlags{fcitx::TextFormatFlag::Underline, fcitx::TextFormatFlag::HighLight});
    if (text.size() > conversion.focus_end_bytes)
      preedit.append(text.substr(conversion.focus_end_bytes), fcitx::TextFormatFlag::Underline);
    preedit.setCursor(static_cast<int>(conversion.caret_bytes));
    if (ic_.capabilityFlags().test(fcitx::CapabilityFlag::Preedit))
      ic_.inputPanel().setClientPreedit(preedit);
    else
      ic_.inputPanel().setPreedit(preedit);
  } else if (style != "empty" || alwaysInline) {
    // 韩文音节、注音转换、越南文单词或藏文音节串是用户正在写的文字，所以不论预编辑样式如何都内嵌显示：列表打开之前没有候选窗可以显示它（core/InputSchemeTraits.h 的 AlwaysInlinePreedit）。
    auto reading = style == "pinyin" || alwaysInline ? view_.value("preedit", editing) : editing;
    // A Japanese composition is かな, not the letters that produced it; see
    // ../src/core/PhrasePreedit.h for the one case that keeps the letters.
    const auto kana = view_.value("reading", std::string{});
    if (msime::linux_host::composition_shows_reading(
            kana, msime::linux_host::strict_json_value(view_, "caret_position", size_t{}), editing.size()))
      reading = kana;
    // The piece already picked for the phrase leads the reading, the way the reference draws
    // `word_for_creating_word`. fcitx5 takes the cursor as a byte offset into the string it is
    // given, which is why the offset comes from the same place the text does.
    const auto composed = msime::linux_host::compose_phrase_preedit(
        view_.value("phrase_prefix", std::string{}), reading,
        msime::linux_host::strict_json_value(view_, "caret_position", size_t{}));
    // 正在组词时，已选的那一段和还在输入的读音分成两个子句：已选的单下划线，读音加下划线并高亮，和上面整句改字的焦点子句同一种画法（phrase_preedit_has_clauses）。Fcitx5 没有粗下划线这种格式，`Bold` 在前端里是字形加粗，会把读音的字母画粗画宽，不是 macOS 那条加粗的下划线。没有已选段时整段照旧单下划线。
    fcitx::Text preedit;
    if (msime::linux_host::phrase_preedit_has_clauses(composed)) {
      preedit.append(composed.text.substr(0, composed.prefix_bytes), fcitx::TextFormatFlag::Underline);
      preedit.append(composed.text.substr(composed.prefix_bytes),
                     fcitx::TextFormatFlags{fcitx::TextFormatFlag::Underline, fcitx::TextFormatFlag::HighLight});
    } else {
      preedit.append(composed.text, fcitx::TextFormatFlag::Underline);
    }
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
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  renderNativeCanvas();
#endif
  renderShuangpinKeymap();
  ic_.updatePreedit();
  ic_.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

#ifdef MSIME_FCITX5_NATIVE_CANVAS
bool FcitxState::nativeCanvasDrawable() const {
  return engine_ && engine_->native_canvas_owned_ && engine_->native_canvas_assets_ &&
      engine_->instance_->currentUI() == "classicui" && ic_.hasFocus() && ic_.display().rfind("x11:", 0) == 0 &&
      !ic_.capabilityFlags().test(fcitx::CapabilityFlag::ClientSideInputPanel);
}

bool FcitxState::nativeChromeActive() const {
  return nativeCanvasDrawable() && engine_->native_canvas_assets_->chrome;
}

// chrome 按「展开候选」偏好给网格；canvas 包按清单的 expandPanel 给卷轴，与 Windows 一样不看那个偏好。
FcitxState::ExpandStyle FcitxState::expandStyle() const {
  if (!nativeCanvasDrawable()) return ExpandStyle::none;
  const auto &assets = *engine_->native_canvas_assets_;
  if (assets.chrome) return preferences_.value("expand_candidates", false) ? ExpandStyle::grid : ExpandStyle::none;
  return assets.expand_panel ? ExpandStyle::scroll : ExpandStyle::none;
}

// 这时候还能不能展开着：有展开样式、横排、有焦点、不是受限输入，没有打开候选列表（韩文汉字、注音）或释义页。
bool FcitxState::expandAllowed() const {
  return session_ && expandStyle() != ExpandStyle::none && !restricted() &&
      preferences_.value("candidate_layout", std::string("vertical")) == "horizontal" &&
      !translationCandidatesActive() && !msime::linux_host::opened_candidate_list(view_) &&
      !msime::linux_host::korean_hanja_list_open(view_) && !view_.value("candidates", Json::array()).empty();
}

bool FcitxState::expandOffered() const {
  return expandAllowed() && expandStyle() == ExpandStyle::grid &&
      msime::linux_host::strict_json_value(view_, "page_count", 0u) > 1;
}

// 这个会话这一代的完整候选。all_candidates 会放出引擎扣着的尾部并推进代次（input-runtime 的 Runtime::all_candidates），所以复制之后重读当前页，按重读后的这一代整份核对（NativeCanvasSnapshot.h：会话、代次和严格递增的全局下标）；混了代、身份不合法或重复时一个也不认，不拿数组位置猜全局下标。即便完整候选不能用，也要换上重读的当前页，旧页的身份可能已经属于上一代。
std::vector<Json> FcitxState::expandedEntries() {
  std::vector<Json> entries;
  try {
    auto snapshot = response(msime_client_all_candidates(session_));
    view_ = response(msime_client_view(session_));
    const auto generation = msime::linux_host::strict_json_value(view_, "generation", uint64_t{});
    if (!msime::linux_host::native_canvas_snapshot_current(snapshot, session_, generation)) return entries;
    for (auto &entry : snapshot.at("candidates")) entries.push_back(std::move(entry));
  } catch (const std::exception &) {
    entries.clear();
  }
  return entries;
}

size_t FcitxState::expandedVisibleRows(size_t rows) const {
  const auto &assets = engine_ ? engine_->native_canvas_assets_ : nullptr;
  const size_t limit = assets && assets->expand_rows ? 1 + *assets->expand_rows : msime::candidate::kExpandedGridVisibleRows;
  return std::min(rows, limit);
}

// 展开。网格：高亮停在普通候选窗正高亮的那一个上，没有更多候选时返回 false，调用方照旧处理这个键。卷轴：只有一页时也展开（展开键翻成收起，下面没有网格）。
bool FcitxState::openExpanded() {
  const auto style = expandStyle();
  if (!expanded_.empty() || !(style == ExpandStyle::grid ? expandOffered() : expandAllowed())) return false;
  const auto before = msime::linux_host::strict_json_value(view_, "generation", uint64_t{});
  auto entries = expandedEntries();
  const auto page = view_.value("candidates", Json::array());
  if (entries.empty() || (style == ExpandStyle::grid && entries.size() <= page.size())) {
    // 复制时推进了代次：当前页已经换成重读的那一页，重画让候选列表跟上新的身份。
    if (msime::linux_host::strict_json_value(view_, "generation", uint64_t{}) != before) render();
    return false;
  }
  const auto columns = expandedColumns();
  size_t cursor = std::min(msime::linux_host::strict_json_value(view_, "page", size_t{}) * columns, entries.size() - 1);
  for (const auto &candidate : page) {
    if (!candidate.is_object() || !candidate.value("highlighted", false)) continue;
    for (size_t index = 0; index < entries.size(); ++index)
      if (entries[index].at("id") == candidate.value("id", Json())) { cursor = index; break; }
    break;
  }
  expanded_ = std::move(entries);
  expanded_style_ = style;
  expanded_assets_ = engine_->native_canvas_assets_;
  expanded_session_ = session_;
  expanded_generation_ = msime::linux_host::strict_json_value(view_, "generation", uint64_t{});
  expanded_composition_ = view_.value("editing_text", std::string{});
  expanded_cursor_ = cursor;
  // 当前高亮所在页是网格的第一行。
  expanded_first_row_ = cursor / columns;
  render();
  return !expanded_.empty();
}

// 每次重画和每个键之前对照当前视图；返回 false 表示该收起。
bool FcitxState::expandedStillApplies() {
  if (expanded_.empty() || !expandAllowed() || expandStyle() != expanded_style_ ||
      expanded_assets_.lock() != engine_->native_canvas_assets_ || session_ != expanded_session_)
    return false;
  // 网格跟着一段组字，组字变了就收起；卷轴是一个开关，继续输入时留着。
  if (expanded_style_ == ExpandStyle::grid && view_.value("editing_text", std::string{}) != expanded_composition_) return false;
  if (msime::linux_host::strict_json_value(view_, "generation", uint64_t{}) == expanded_generation_) return true;
  // 候选换了一代（云候选、AI 回答、整句重排到达，卷轴还有继续输入）：按新的一代重载，网格的光标留在原来那个词上，它不在新的一代里时夹在范围内。重载可能再推进一代，记重读后的那一代。
  auto entries = expandedEntries();
  if (entries.empty()) return false;
  const auto text = expanded_cursor_ < expanded_.size() ? expanded_[expanded_cursor_].value("text", std::string{}) : std::string{};
  size_t cursor = std::min(expanded_cursor_, entries.size() - 1);
  for (size_t index = 0; index < entries.size(); ++index)
    if (entries[index].value("text", std::string{}) == text) { cursor = index; break; }
  expanded_ = std::move(entries);
  expanded_generation_ = msime::linux_host::strict_json_value(view_, "generation", uint64_t{});
  expanded_composition_ = view_.value("editing_text", std::string{});
  expanded_cursor_ = cursor;
  return true;
}

// 选完整候选里的第 index 个（两种样式共用），按复制时的全局身份选，不翻页、不按文字找替代。复制时的一代已经不是当前这一代时什么也不选：网格收起，卷轴重画（重画时按新的一代重载）。网格选完就收起；卷轴留着，上屏后没了候选时随下一次重画收起。`pointer` 是鼠标点选，先开一轮。
void FcitxState::selectExpanded(size_t index, bool pointer) {
  if (expanded_.empty() || index >= expanded_.size()) return;
  const auto id = expanded_[index].at("id");
  const auto generation = msime::linux_host::strict_json_value(id, "generation", uint64_t{});
  const auto global = msime::linux_host::strict_json_value(id, "index", size_t{});
  const bool current = ic_.hasFocus() && session_ == expanded_session_ && generation == expanded_generation_ &&
      msime::linux_host::strict_json_value(view_, "generation", uint64_t{}) == generation && !restricted();
  if (expanded_style_ == ExpandStyle::grid) discardExpanded();
  if (!current) { render(); return; }
  if (pointer) pointerTurn();
  try {
    apply(msime_client_select_any_candidate(session_, generation, global));
  } catch (const std::exception &) {
    render();
  }
}

// 滚轮在网格上一格滚一行，高亮留在可见的几行里、列不变；键盘滚动仍按 StepExpandedGrid 走。
void FcitxState::scrollExpanded(bool backwards) {
  if (expanded_.empty() || !expandedStillApplies()) { closeExpanded(); return; }
  const auto columns = expandedColumns();
  const auto rows = msime::candidate::ExpandedGridRows(expanded_.size(), columns);
  const auto visible = expandedVisibleRows(rows);
  if (rows <= visible) return;
  if (backwards && expanded_first_row_ > 0) --expanded_first_row_;
  else if (!backwards && expanded_first_row_ + visible < rows) ++expanded_first_row_;
  else return;
  const auto row = expanded_cursor_ / columns, column = expanded_cursor_ % columns;
  const auto target = row < expanded_first_row_ ? expanded_first_row_
                      : row >= expanded_first_row_ + visible ? expanded_first_row_ + visible - 1 : row;
  expanded_cursor_ = std::min(target * columns + column, expanded_.size() - 1);
  render();
}

// 网格开着时的每一个键，规则见声明处。返回 false 时网格已经收起，这个键照常往下走。
bool FcitxState::expandedKey(fcitx::KeyEvent &event) {
  if (!expandedStillApplies()) {
    closeExpanded();
    return false;
  }
  const auto sym = event.key().sym();
  const auto states = event.key().states();
  const bool shift = event.rawKey().states().test(fcitx::KeyState::Shift);
  const bool modified = states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                                        fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                                        fcitx::KeyState::Mod5});
  const auto count = expanded_.size();
  const auto columns = expandedColumns();
  std::optional<msime::candidate::ExpandedGridMove> move;
  using Move = msime::candidate::ExpandedGridMove;
  if (!modified && !shift) {
    // 数字键选高亮那一行里的第几个；数字是正在拼写的输入（Unicode、表达式模式）或九键组字时不是选词键。
    int digit = -1;
    if (sym >= FcitxKey_0 && sym <= FcitxKey_9) digit = sym - FcitxKey_0;
    else if (sym >= FcitxKey_KP_0 && sym <= FcitxKey_KP_9) digit = sym - FcitxKey_KP_0;
    if (digit >= 0 && !view_.value("nine_key", false) &&
        !msime::linux_host::engine_spelling(view_, static_cast<char32_t>(U'0' + digit))) {
      const size_t slot = digit == 0 ? 9 : static_cast<size_t>(digit - 1);
      const size_t index = (expanded_cursor_ / columns) * columns + slot;
      // 这一行没有的序号（最后一行不满）留在网格里，不当作输入。
      if (slot < columns && index < count) selectExpanded(index, false);
      return true;
    }
    switch (sym) {
    case FcitxKey_Left: case FcitxKey_KP_Left: move = Move::Left; break;
    case FcitxKey_Right: case FcitxKey_KP_Right: move = Move::Right; break;
    case FcitxKey_Down: case FcitxKey_KP_Down: move = Move::Down; break;
    case FcitxKey_Up: case FcitxKey_KP_Up: move = Move::Up; break;
    case FcitxKey_Page_Up: case FcitxKey_KP_Page_Up: move = Move::PageUp; break;
    case FcitxKey_Page_Down: case FcitxKey_KP_Page_Down: move = Move::PageDown; break;
    case FcitxKey_Tab: case FcitxKey_KP_Tab:
      if (navigation_.value("tab", true)) move = Move::PageDown;
      break;
    case FcitxKey_space:
      // 注音等拿空格拼写的方案里空格是输入，收起网格后照常处理。
      if (msime::linux_host::spelling_space(view_)) break;
      selectExpanded(expanded_cursor_, false);
      return true;
    case FcitxKey_Return: case FcitxKey_KP_Enter:
      selectExpanded(expanded_cursor_, false);
      return true;
    case FcitxKey_Escape:
      closeExpanded();
      return true;
    default: {
      // 字符翻页键（- =、[ ]、, .）按「翻页方式」里打开的那几组，缺省与普通候选窗的翻页相同（- = 和 , . 开、[ ] 关）。它们是拼写输入时不算翻页；日语（方案或临时日文）的 - = 是长音和等号，以词定字占着的那一组也不翻页（macOS 的 IsJapaneseMinusEqualKey、wordCharacterClaimsEvent）。
      const bool minus_equal = sym == FcitxKey_minus || sym == FcitxKey_equal;
      const bool brackets = sym == FcitxKey_bracketleft || sym == FcitxKey_bracketright;
      const char *binding = minus_equal ? "minus_equal" : brackets ? "brackets"
                          : sym == FcitxKey_comma || sym == FcitxKey_period ? "comma_period" : nullptr;
      const bool japanese = msime::linux_host::strict_json_value(view_, "scheme", 0u) == 3 ||
          view_.value("local_mode", std::string("none")) == "temporary_japanese";
      const bool word_character = word_character_enabled_ && msime::linux_host::strict_json_value(view_, "scheme", 0u) != 3 &&
          (word_character_minus_equal_ ? minus_equal : brackets);
      const auto spelled = static_cast<char32_t>(fcitx::Key::keySymToUnicode(sym));
      if (binding && navigation_.value(binding, !brackets) && !(minus_equal && japanese) && !word_character &&
          !msime::linux_host::engine_spelling(view_, spelled))
        move = sym == FcitxKey_minus || sym == FcitxKey_bracketleft || sym == FcitxKey_comma ? Move::PageUp : Move::PageDown;
      break;
    }
    }
  } else if (!modified && shift && navigation_.value("tab", true) &&
             (sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab ||
              event.rawKey().sym() == FcitxKey_ISO_Left_Tab)) {
    move = Move::PageUp;
  }
  if (!move) {
    closeExpanded();
    return false;
  }
  const auto rows = msime::candidate::ExpandedGridRows(count, columns);
  const auto visible = std::max<size_t>(1, expandedVisibleRows(rows));
  const auto step = msime::candidate::StepExpandedGrid(expanded_cursor_, count, columns, visible, *move);
  if (step.close) {
    closeExpanded();
    return true;
  }
  expanded_cursor_ = step.cursor;
  render();
  return true;
}

bool FcitxState::renderNativeCanvas() {
  namespace host = msime::linux_host;
  const auto list = ic_.inputPanel().candidateList();
  // 客户端自己画候选面板（ClientSideInputPanel，例如 fcitx5-gtk 的客户端面板）时 classicui 也不画，原生画布同样让给它。
  const bool drawable = nativeCanvasDrawable();
  if (!drawable || !list || list->size() == 0) {
    // 只有普通收起（上屏、取消后没了候选）播退场；失焦、换界面、换到 Wayland 一律立即收起，不在屏幕上留旧候选文字。同一个输入上下文换进密码、数字或敏感字段（Chromium、Electron 复用一个 IC）时也立即收起，不让上一个字段的候选在退场里多留一会儿；只带 NoSpellCheck 的普通字段照常播退场。
    const bool animate = drawable && !restricted() && !ic_.capabilityFlags().test(fcitx::CapabilityFlag::Sensitive);
    if (native_canvas_) native_canvas_->hide(animate);
    discardExpanded();
    return false;
  }
  const auto &assets = *engine_->native_canvas_assets_;
  const bool chrome = assets.chrome;
  host::NativeCanvasContent content;
  content.preedit = ic_.inputPanel().preedit().toString();
  if (!content.preedit.empty()) content.preedit_cursor = ic_.inputPanel().preedit().cursor();
  // 拼音行属于画布，客户端内嵌预编辑仍单独保留。
  if (content.preedit.empty() && preferences_.value("candidate_preedit_style", std::string("pinyin")) == "pinyin")
    content.preedit = view_.value("preedit", std::string{});
  if (!content.preedit.empty() && content.preedit == view_.value("editing_text", std::string{}))
    content.preedit_cursor = static_cast<int>(std::min(content.preedit.size(),
        host::strict_json_value(view_, "caret_position", content.preedit.size())));
  if (!ic_.inputPanel().auxUp().empty()) {
    content.preedit = ic_.inputPanel().auxUp().toString() + (content.preedit.empty() ? "" : " · " + content.preedit);
    content.preedit_cursor = -1;
  }
  content.font = engine_->hint_inputs_.font.empty() ? "sans 16" : engine_->hint_inputs_.font;
  content.horizontal = list->layoutHint() == fcitx::CandidateLayoutHint::Horizontal;
  content.selected = list->cursorIndex();
  // 与 classicui 画候选窗时相同：水杉的开关经 WheelForPaging 生效，从没动过时沿用桌面原有设置（Fcitx5 缺省为开）。
  content.wheel_paging = engine_->classicui_wheel_paging_;
  content.page_arrows = preferences_.value("show_candidate_page_number", true);
  if (chrome) {
    // macOS 结构：页码进首行（「1 / 3」），拼音在首行左边；底部一行只剩本地模式和打字连击，macOS 候选窗没有这一行，Linux 留着它们。光标后编辑的提示画在这一行的右端（auxiliary_hint），本地模式和连击照旧从左边排。
    std::string footer;
    if (const char *label = host::candidate_local_mode_label(view_.value("local_mode", std::string("none")))) footer = label;
    const auto combo = host::typing_combo_label(typing_combo_);
    if (!combo.empty()) footer += (footer.empty() ? "" : " · ") + combo;
    content.auxiliary = std::move(footer);
    if (host::caret_edit_suffix(view_).active) content.auxiliary_hint = std::string(host::kCaretEditSuffixHint);
    content.logo = engine_->native_canvas_logo_;
    const auto scale = host::read_fcitx_candidate_style(preferences_).scale;
    const auto preedit_size = preferences_.find("candidate_preedit_font_size");
    content.preedit_size = (preedit_size != preferences_.end() && preedit_size->is_number_integer() &&
                            *preedit_size >= 12 && *preedit_size <= 32 ? preedit_size->get<int>() : 15) * scale;
    // 横排为还没到的释义预留高度（translationReservedLines），韩文汉字列表再加训音一行。
    content.reserved_gloss_lines = translationReservedLines() + (host::korean_hanja_list_open(view_) ? 1 : 0);
    content.selection_context = view_.value("editing_text", std::string{}) + "\n" +
        std::to_string(host::strict_json_value(view_, "page", size_t{}));
  } else {
    content.auxiliary = ic_.inputPanel().auxDown().toString();
  }
  const auto label = [chrome](std::string text) {
    // macOS 的序号不带空格；classicui 用空格代替序号与候选之间的间距，原生画布自己留 1.5pt。
    if (chrome) while (!text.empty() && text.back() == ' ') text.pop_back();
    return text;
  };
  const auto make_item = [&](const FcitxCandidateDisplay &display, std::string number) {
    host::NativeCanvasItem result{std::move(number), chrome ? display.word : display.body, display.translation,
        display.suffix, display.translation_runs, display.fixed};
    result.decoration_text = display.decoration_text;
    result.decoration_badge = display.decoration_badge;
    result.dim_prefix_bytes = display.dim_prefix_bytes;
    result.pronunciation = display.pronunciation;
    // 「仅当前高亮」的高亮行一直留出释义的行数；chrome 的释义行先画训音，有训音时多留一行。
    if (display.reserved_gloss_lines > 0)
      result.reserved_gloss_lines = display.reserved_gloss_lines + (chrome && !display.reading.empty() ? 1 : 0);
    if (chrome) {
      result.reading = display.reading;
      result.script_annotation = display.script_annotation;
      result.pending = display.pending;
    }
    return result;
  };
  // 装饰词始终取普通当前页；展开网格只改变候选的显示与选择，不改变装饰词索引。
  content.decoration_items.emplace();
  content.decoration_selected = list->cursorIndex();
  for (int at = 0; at < list->size(); ++at) {
    if (const auto *candidate = dynamic_cast<const FcitxCandidate *>(&list->candidate(at)))
      content.decoration_items->push_back(make_item(candidate->display(), {}));
    else content.decoration_items->push_back({{}, {}});
  }
  // 展开状态已在 render() 开头对照过当前视图；两种样式都只认展开时的那种画法。
  const bool grid = chrome && !expanded_.empty() && expanded_style_ == ExpandStyle::grid;
  const bool scroll = !chrome && !expanded_.empty() && expanded_style_ == ExpandStyle::scroll;
  // 展开／收起键：网格和卷轴共用，开的时候收起、收着的时候展开。
  const auto toggle = [this](bool open) {
    if (open) openExpanded();
    else closeExpanded();
  };
  if (grid) {
    // 网格：可见的几行，只有高亮所在的那一行有序号；首行是「行 / 总行数」和收起键。
    const auto columns = expandedColumns();
    const auto count = expanded_.size();
    const auto rows = msime::candidate::ExpandedGridRows(count, columns);
    const auto visible = expandedVisibleRows(rows);
    expanded_cursor_ = std::min(expanded_cursor_, count - 1);
    const auto cursor_row = expanded_cursor_ / columns;
    expanded_first_row_ = msime::candidate::ExpandedGridFirstRow(expanded_first_row_, cursor_row, rows, visible);
    const auto first = expanded_first_row_ * columns;
    const auto last = std::min(count, (expanded_first_row_ + visible) * columns);
    const auto caretEdit = host::caret_edit_suffix(view_);
    for (auto at = first; at < last; ++at) {
      const auto &entry = expanded_[at];
      const auto display = candidateDisplay(entry, traditionalApplies(), showCandidateAnnotations(),
          host::korean_hanja_gloss(view_, entry), traditional_variant_, host::caret_edit_dim_scalars(caretEdit, entry),
          0, candidatePronunciation(entry));
      content.items.push_back(make_item(display, at / columns == cursor_row ? host::candidate_index_label(at % columns) : std::string{}));
    }
    content.selected = static_cast<int>(expanded_cursor_ - first);
    content.grid_columns = columns;
    content.grid_row = cursor_row;
    content.grid_rows = rows;
    content.selection_context.clear();
    content.expanded = true;
    // 画出来的这一代已经被重载替换时不选（下标会落到新的一代上）。
    content.select = [this, first, generation = expanded_generation_](int at) {
      if (ic_.hasFocus() && !restricted() && at >= 0 && expanded_generation_ == generation)
        selectExpanded(first + static_cast<size_t>(at), true);
    };
    content.grid_scroll = [this](bool backwards) { scrollExpanded(backwards); };
    content.expand = toggle;
  } else {
    for (int at = 0; at < list->size(); ++at) {
      if (const auto *candidate = dynamic_cast<const FcitxCandidate *>(&list->candidate(at))) {
        content.items.push_back(make_item(candidate->display(), label(list->label(at).toString())));
      } else {
        content.items.push_back({label(list->label(at).toString()), list->candidate(at).text().toString()});
      }
    }
    // 卷轴：首行仍是当前页，下面是完整候选里当前页之后的那些（跳过联网占位和空文字），不画序号和释义；命中下标接在当前页后面，按完整候选里的位置选。
    std::vector<size_t> grid_entries;
    if (scroll) {
      const auto &page = view_.at("candidates");
      const auto last = page.empty() ? size_t{} :
          host::strict_json_required_integer<size_t>(page.back().at("id").at("index"));
      for (size_t at = 0; at < expanded_.size(); ++at) {
        const auto &entry = expanded_[at];
        if (host::strict_json_required_integer<size_t>(entry.at("id").at("index")) <= last ||
            host::strict_json_value(entry, "source", uint64_t{}) == 12 ||
            entry.at("text").get_ref<const std::string &>().empty()) continue;
        const auto display = candidateDisplay(entry, traditionalApplies(), showCandidateAnnotations(),
            host::korean_hanja_gloss(view_, entry), traditional_variant_);
        content.grid_items.push_back({{}, display.body, {}, display.suffix});
        grid_entries.push_back(at);
      }
      content.expanded = true;
      // 同一段组字、同一页、同样的简繁投影重画时画布保留滚动位置；后台换代（云候选、释义到达）不算，继续输入、翻页才回到顶部。
      content.revision = std::to_string(session_) + "\n" + view_.value("editing_text", std::string{}) + "\n" +
          std::to_string(host::strict_json_value(view_, "page", size_t{})) + "\n" +
          std::to_string(traditionalApplies()) + ":" + std::to_string(traditional_variant_);
      content.page_size = host::strict_json_value(view_, "page_size", size_t{9});
    }
    content.select = [this, list, grid_entries = std::move(grid_entries), generation = expanded_generation_](int at) {
      if (!ic_.hasFocus() || restricted() || at < 0) return;
      if (at < list->size()) { list->candidate(at).select(&ic_); return; }
      const auto cell = static_cast<size_t>(at - list->size());
      if (cell < grid_entries.size() && !expanded_.empty() && expanded_generation_ == generation)
        selectExpanded(grid_entries[cell], true);
    };
    if (auto *page = list->toPageable()) {
      content.previous = page->hasPrev(); content.next = page->hasNext();
      content.page_index = page->currentPage();
      content.page_count = page->totalPages();
      content.page = [this, list](bool next) {
        if (!ic_.hasFocus() || restricted()) return;
        auto *page = list->toPageable();
        if (next) page->next(); else page->prev();
      };
    }
    content.actions = [this, list](int at) {
      std::vector<host::NativeCanvasAction> result;
      if (!ic_.hasFocus() || restricted() || at < 0 || at >= list->size()) return result;
      const auto *item = dynamic_cast<const FcitxCandidate *>(&list->candidate(at));
      if (!item) return result;
      for (const auto &[id, text] : candidateMenuActions(*this, *item)) {
        result.push_back({text, [this, list, at, id = id] {
          const auto &candidate = static_cast<const FcitxCandidate &>(list->candidate(at));
          triggerCandidateMenuAction(*this, candidate, id);
        }});
      }
      return result;
    };
    // 展开键。网格：「展开候选」打开、显示展开按钮、横排而且不止一页。卷轴：清单打开 expandPanel、横排，没在释义页或候选列表里；展开着时它是收起键。
    if (chrome ? expandOffered() && preferences_.value("show_candidate_expand_button", true)
               : scroll || (expandStyle() == ExpandStyle::scroll && expandAllowed())) {
      content.expandable = true;
      content.expand = toggle;
    }
  }
  if (!native_canvas_) native_canvas_ = std::make_unique<host::NativeCanvasX11>(*loop_);
  native_canvas_->setReduceMotion(engine_->reduce_motion_reader_.read());
  const auto cursor = ic_.cursorRect();
  if (!native_canvas_->show(ic_.display(), engine_->native_canvas_assets_, std::move(content),
      engine_->native_canvas_palette_, cursor.left(), cursor.top(), cursor.height())) {
    // 展开后画不下（工作区太小）时收起，接着由原生画布画普通的一页，不把这一帧交给 classicui；普通的一页也画不出来（工作区太小、X 连接失败）时才由 classicui 画。
    if (expanded_.empty()) return false;
    discardExpanded();
    return renderNativeCanvas();
  }
  const auto client_preedit = ic_.inputPanel().clientPreedit();
  ic_.inputPanel().reset();
  ic_.inputPanel().setClientPreedit(client_preedit);
  return true;
}
#endif

void FcitxState::maintenance(int operation) {
  if (!ensure() || view_.value("candidates", Json::array()).empty()) return;
  for (const auto &candidate : view_.at("candidates")) {
    if (!candidate.value("highlighted", false)) continue;
    const auto &id = candidate.at("id");
    const auto generation =
        msime::linux_host::strict_json_required_integer<uint64_t>(id.at("generation"));
    const auto index = msime::linux_host::strict_json_required_integer<size_t>(id.at("index"));
    char *raw = nullptr;
    if (operation == 1) raw = msime_client_pin_candidate(session_, generation, index);
    else if (operation == 2 && msime::linux_host::candidate_dictionary_removal_available(
                 msime::linux_host::strict_json_value(view_, "scheme", 0u), msime::linux_host::strict_json_value(candidate, "source", uint64_t{}),
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

// 模式角标的主题：与 macOS 的角标一样跟随颜色模式，"system"（跟随系统，也是默认值）看桌面明暗，存下的 toolbar_theme 不再读。固定明暗的全局主题（水杉是深色，纸白是浅色）照样决定明暗，颜色取那个主题的调色板，与悬浮工具栏取法相同。
msime::linux_host::CandidateTheme fcitx_mode_badge_theme(const Json &preferences, bool system_dark, const Json &catalog) {
  const bool dark = msime::linux_host::surface_dark_theme(preferences, system_dark);
  return resolveThemeInMode(preferences, dark, catalog);
}

void FcitxState::showInputModeHud([[maybe_unused]] ModeHudTrigger trigger) {
#ifdef MSIME_FCITX5_CUSTOM_IM_INFORMATION
#ifdef MSIME_FCITX5_MODE_BADGE
  // 新的提示取代还在等光标的那条获得焦点提示。
  if (trigger != ModeHudTrigger::FocusSettled) cancelFocusInputModeHud();
#endif
  // 切换提示默认开，聚焦提示默认关；光标等待完成后仍遵循聚焦偏好。
  if (!(trigger == ModeHudTrigger::Switch ? preferences_.value("input_mode_hud", true)
                                         : preferences_.value("input_mode_focus_hud", false))) return;
  if (restricted() || privateInput() || !ic_.hasFocus()) return;
  const std::string label = input_enabled_ ? "中" : "英";
  auto hint = msime::linux_host::ModeBadgeHint::PanelText;
#ifdef MSIME_FCITX5_MODE_BADGE
  // 标志跟随「显示水杉图标」（show_app_logo，默认关），与 macOS 一致。只有 X11 客户端报的光标矩形是根窗口坐标；Wayland 客户端的是它自己表面里的局部坐标，拿来放徽章会放错地方。
  const bool show_logo = preferences_.value("show_app_logo", false);
  const bool allow_corner = preferences_.value("input_mode_corner_hud", true);
  const bool x11_client = ic_.display().rfind("x11:", 0) == 0;
  auto *badge =
      msime::linux_host::mode_badge_surface_wanted(x11_client, show_logo, allow_corner) ? engine_->modeBadge() : nullptr;
  hint = msime::linux_host::mode_badge_hint(badge != nullptr, badge && badge->follows_caret(), x11_client,
                                          show_logo, allow_corner);
  if (hint == msime::linux_host::ModeBadgeHint::CaretBadge && trigger == ModeHudTrigger::FocusEntered && loop_) {
    mode_hud_after_focus_ = true;
    mode_hud_focus_timer_ = loop_->addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + msime::linux_host::ModeBadgeTiming::focus_caret_wait_ms * 1000ULL,
        0, [this](fcitx::EventSourceTime *, uint64_t) {
          settleFocusInputModeHud();
          return true;
        });
    return;
  }
  const auto &rect = ic_.cursorRect();
  const msime::linux_host::ModeBadgeCaret caret_rect{rect.left(), rect.top(), rect.width(), rect.height()};
  if (trigger == ModeHudTrigger::FocusSettled) hint = msime::linux_host::mode_badge_focus_hint(hint, caret_rect);
  if (hint != msime::linux_host::ModeBadgeHint::PanelText) {
    const msime::linux_host::ModeBadgeStyle style{
        msime::linux_host::ModeBadgeMetrics{},
        msime::linux_host::floating_surface_colors(
            fcitx_mode_badge_theme(preferences_, system_dark_, candidate_skin_document_))};
    std::optional<msime::linux_host::ModeBadgeCaret> caret;
    if (hint == msime::linux_host::ModeBadgeHint::CaretBadge) caret = caret_rect;
    if (badge->show(label, show_logo ? MSIME_MODE_BADGE_ICON : "", style, caret)) {
      engine_->scheduleModeBadgeHide(ic_);
    } else {
      hint = msime::linux_host::ModeBadgeHint::PanelText;
    }
  }
#endif
  // 徽章已经放在光标旁时不再发面板提示：macOS 只显示一处。
  if (hint == msime::linux_host::ModeBadgeHint::CaretBadge) return;
  if (auto *instance = engine_->instance()) {
#ifdef MSIME_FCITX5_HINT_FONT
    instance->showCustomInputMethodInformation(&ic_, engine_->modeHintLabel(ic_.display(), label));
#else
    instance->showCustomInputMethodInformation(&ic_, label);
#endif
  }
#endif
}

// 悬浮工具栏的状态，与状态区那几个动作显示的勾选一致：中英文、英文候选模式、大写锁定、方案、标点、全半角、简繁，以及方案菜单（只列本版本提供、这台机器上也能运行的方案，与 rebuildSchemeMenu 相同）。
msime::linux_host::FloatingToolbarState FcitxState::floatingToolbarState() const {
  msime::linux_host::FloatingToolbarState toolbar;
  toolbar.input_enabled = input_enabled_;
  toolbar.english_candidates = view_.is_object() && view_.value("dedicated_english", false);
  toolbar.caps_lock = caps_lock_;
  toolbar.scheme = effectiveScheme();
  toolbar.scheme_title = schemeMenuTitle(toolbar.scheme);
  toolbar.chinese_punctuation = chinese_punctuation_;
  toolbar.fullwidth = fullwidthOutput();
  toolbar.traditional = traditional_;
  for (const char *scheme : msime::linux_host::kSchemeMenuOrder) {
    const std::string_view id(scheme);
    const bool offered = id == "cantonese" || id == "zhuyin" || id == "stroke" ? schemeAvailable(scheme)
                                                                              : msime::linux_host::edition_offers_scheme(id);
    if (offered) toolbar.schemes.push_back({scheme, schemeMenuTitle(id)});
  }
  return toolbar;
}

void FcitxState::syncFloatingToolbar([[maybe_unused]] bool focus_entered) {
#ifdef MSIME_LINUX_FLOATING_PANELS
  // 没有会话的上下文什么也不碰：close() 把偏好清成了空对象，按它读出的缺省配置（开关开着、缺省按钮和大小）会把用户关掉的工具栏重新打开、把属主的外观冲掉。这包括密码框和数字框这类受限输入框（ensure 关掉会话，activate 照样走到这里）、交出托盘菜单时关掉的上下文，以及换到别的输入法时已经关掉会话的上下文。
  if (!engine_ || !session_ || !preferences_.is_object() || preferences_.empty()) return;
  // 能接手的只有有焦点的上下文：activate 时（focus_entered）总能接手，之后在工具栏还没有属主时接手。
  const bool can_own = ic_.hasFocus();
  const auto config = msime::linux_host::floating_toolbar_config(preferences_, MSIME_EDITION_HANDWRITING != 0, voice_enabled_);
  // 只在开关开着、这个上下文有焦点时才建：失焦的上下文刷新状态区时不为它连 X 服务器。
  auto *toolbar = engine_->floatingToolbar(config.enabled && can_own);
  if (!toolbar) return;
  // 主题要经 host-api 解析，只在偏好、系统明暗或皮肤目录变了时重算，不在每次按键时解析。
  if (floating_colors_.stale(preferences_, system_dark_, candidate_skin_document_))
    floating_colors_.toolbar = msime::linux_host::floating_toolbar_colors(
        fcitx_mode_badge_theme(preferences_, system_dark_, candidate_skin_document_));
  toolbar->configure(config, floating_colors_.toolbar);
  if (can_own && (focus_entered || !toolbar->owner()))
    toolbar->activate(&ic_, floatingToolbarState());
  else
    toolbar->update(&ic_, floatingToolbarState());
#endif
}

// 双拼键位图，条件与 macOS 的 updateKeymapPanel 相同：有焦点、中文输入开着、偏好打开、双拼组字中（shuangpin_keymap_hint）。只给 X11 客户端画：只有它们报的光标矩形是根窗口坐标；Wayland 原生客户端的光标是它自己表面里的局部坐标，那里不画键位图。光标矩形不可用时不画。
void FcitxState::renderShuangpinKeymap() {
#ifdef MSIME_LINUX_FLOATING_PANELS
  std::optional<msime::linux_host::ShuangpinKeymapHint> hint;
  if (engine_ && session_ && ic_.hasFocus() && !restricted() && input_enabled_ && voice_preedit_.empty() &&
      ic_.display().rfind("x11:", 0) == 0 && msime::linux_host::shuangpin_keymap_preference(preferences_))
    hint = msime::linux_host::shuangpin_keymap_hint(view_);
  if (!hint) {
    hideShuangpinKeymap();
    return;
  }
  if (keymap_colors_.stale(preferences_, system_dark_, candidate_skin_document_))
    keymap_colors_.keymap = msime::linux_host::shuangpin_keymap_colors(
        resolveCandidateTheme(preferences_, system_dark_, candidate_skin_document_));
  const auto rect = ic_.cursorRect();
  const auto candidates = view_.value("candidates", Json::array());
  const bool preedit_row = preferences_.value("candidate_preedit_style", std::string("pinyin")) == "pinyin" &&
                           !view_.value("preedit", std::string{}).empty();
  msime::linux_host::ShuangpinKeymapAnchor anchor{
      rect.left(), rect.top(), rect.width(), rect.height(),
      msime::linux_host::shuangpin_keymap_candidate_clearance(
          msime::linux_host::scaled_candidate_font_size(msime::linux_host::read_candidate_font(preferences_)), 1.0,
          preferences_.value("candidate_layout", std::string("vertical")) == "horizontal",
          candidates.is_array() ? candidates.size() : 0, preedit_row),
      std::nullopt};
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  // 候选窗是插件自己的原生画布时（render 先画它再画键位图）用它真实的矩形，与 macOS 按候选窗几何让开相同；经典界面画的候选窗只能按字号估计。
  if (native_canvas_)
    if (const auto frame = native_canvas_->frame())
      anchor.measured_clearance =
          msime::linux_host::shuangpin_keymap_measured_clearance(rect.top(), rect.height(), frame->y, frame->height);
#endif
  engine_->shuangpinKeymap().show(ic_.display().substr(4), *hint, keymap_colors_.keymap, anchor);
#endif
}

void FcitxState::hideShuangpinKeymap() {
#ifdef MSIME_LINUX_FLOATING_PANELS
  if (engine_ && engine_->shuangpin_keymap_) engine_->shuangpin_keymap_->hide();
#endif
}

std::optional<bool> FcitxState::caretShiftHeld() const {
#ifdef MSIME_FCITX5_XKB_STATE_MASK
  if (const auto mask = engine_->instance()->xkbStateMask(ic_.display())) {
    const auto [depressed, latched, locked] = *mask;
    // XKB 核心修饰位的 Shift 与 Fcitx 的 ShiftMask 均为最低位；锁定及粘滞 Shift 也须等待。
    return ((depressed | latched | locked) & static_cast<uint32_t>(fcitx::KeyState::Shift)) != 0;
  }
#endif
  // debt: 老版 Fcitx5 无法回读修饰状态，等下一次无 Shift 的按键再回移；升级至 5.1.22 后可即时回移。
  return std::nullopt;
}

bool FcitxState::key(fcitx::KeyEvent &event) {
  const auto &key = event.key();
  const auto sym = key.sym();
  const auto states = key.states();
  if (!event.isRelease() &&
      (sym == FcitxKey_BackSpace || sym == FcitxKey_Delete || sym == FcitxKey_Left ||
       sym == FcitxKey_Right || sym == FcitxKey_Up || sym == FcitxKey_Down ||
       sym == FcitxKey_Home || sym == FcitxKey_End || sym == FcitxKey_Page_Up ||
       sym == FcitxKey_Page_Down))
    forgetSmartPunctuationShadow();
  // Esc 收起语音失败提示，与 macOS 一样，这个键照常交给编辑器或当前组字。
  if (!event.isRelease() && sym == FcitxKey_Escape && !voice_loading_) dismissVoiceFailure();
  // 和裸 Shift 一样，切换之后松开事件照常交给应用；布防了的按下也只交给应用。
  if (observeCapsLockTap(event)) return false;
  // 规范化后的符号可能已丢掉 Shift 位，光标转发必须按原始事件判断。
  caret_shift_ = event.rawKey().states().test(fcitx::KeyState::Shift);
  if (!caret_shift_ && !isShiftKey(event)) {
    // 下一按键可能与松开事件同批到达；先回移，再处理输入，避免文字落在闭标点外。
    caret_forward_event_.reset();
    flushPendingCaret();
  }
  if (sym == FcitxKey_BackSpace && event.isRelease()) {
    const bool owned = backspace_hold_.armed();
    backspace_hold_.release();
    if (owned) return true;
  } else if (!event.isRelease() && sym != FcitxKey_BackSpace) {
    backspace_hold_.reset();
  }
  // An accepted stroke owns its repeats and release, even if modifiers or
  // preferences change while held. Unmatched releases must not stop voice.
  const auto voiceStroke = event.rawKey().code() != 0
      ? static_cast<unsigned>(event.rawKey().code())
      : msime::linux_host::toggle_key_symbol(event.rawKey().sym()) | 0x80000000u;
  if (voice_toggle_held_ != 0 && voice_toggle_held_ == voiceStroke) {
    if (event.isRelease()) voice_toggle_held_ = 0;
    return true;
  }
  if (sym == FcitxKey_F9 && voice_f9_held_) {
    if (event.isRelease()) voice_f9_held_ = false;
    return true;
  }
  if ((sym == FcitxKey_r || sym == FcitxKey_R) && maintenance_reload_held_) {
    if (event.isRelease()) maintenance_reload_held_ = false;
    return true;
  }
  // 切换类组合键一次按下只翻转一次，和 Windows 一样：按住时的自动重复被吃掉，不会来回翻转，松开结束这次按住。F 和 f 算同一个键，因为先松开 Shift 会改变松开事件的 keysym；J 和 j 同理。
  if (toggle_chord_held_ != FcitxKey_None &&
      (sym == FcitxKey_F ? FcitxKey_f : sym == FcitxKey_J ? FcitxKey_j : sym) == toggle_chord_held_) {
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
          !ic_.capabilityFlags().test(fcitx::CapabilityFlag::Sensitive) &&
          !states.testAny(fcitx::KeyStates{fcitx::KeyState::Alt, fcitx::KeyState::Super,
                                          fcitx::KeyState::Hyper}) &&
          !event.rawKey().states().test(shiftRelease ? fcitx::KeyState::Ctrl : fcitx::KeyState::Shift) &&
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
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  // 网格开着时先由它处理；横排候选窗的 ↓ 和 Tab 在「展开候选」打开、不止一页时展开网格（只有一页、竖排、候选列表开着时 openExpanded 返回 false，键照下面的规则走）。canvas 包的卷轴不接管键盘，也不由键盘打开。
  if (!expanded_.empty() && expanded_style_ == ExpandStyle::grid && expandedKey(event)) return true;
  if (expanded_.empty() && expandStyle() == ExpandStyle::grid && !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                                             fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                                             fcitx::KeyState::Mod5}) &&
      !event.rawKey().states().test(fcitx::KeyState::Shift) &&
      (sym == FcitxKey_Down || sym == FcitxKey_KP_Down || sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab) &&
      openExpanded())
    return true;
#endif
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
    if (!voice_space_locked_) {
      voice_space_locked_ = true;
      // 锁定后的样子（圆形操作按钮，或文本回退里的锁定提示）立即显示，不等下一次 250 ms 的轮询。
      updateVoiceOverlay();
    }
    return true;
  }
  const auto voiceModifiers = event.rawKey().states();
  const bool customVoiceToggle = !voiceModifiers.testAny(fcitx::KeyStates{
      fcitx::KeyState::Hyper, fcitx::KeyState::Meta, fcitx::KeyState::Mod5}) &&
      voice_toggle_chord_.matches(msime::linux_host::toggle_key_name(event.rawKey().sym(),
          event.rawKey().code() >= 8 ? event.rawKey().code() - 8 : 0),
          voiceModifiers.test(fcitx::KeyState::Ctrl), voiceModifiers.test(fcitx::KeyState::Alt),
          voiceModifiers.test(fcitx::KeyState::Shift), voiceModifiers.test(fcitx::KeyState::Super));
  if (customVoiceToggle && voice_enabled_ && !voice_socket_.empty() && !restricted() &&
      !privateInput() && ic_.hasFocus()) {
    if (voice_loading_) {
      if (!stopVoice()) return false;
    } else if (!requestVoice()) return false;
    voice_toggle_held_ = voiceStroke;
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
    if (!alt && !mode_ctrl_space_enabled_) return false;
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
        fullwidthOutput(), english_punctuation_, viewPunctuationMarks());
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
  // 英文模式在上面已经交给应用；Engine 自己的英文模式（Ctrl+Shift+E）里同样交给应用，和 Windows 一致。
  if (japanese_toggle_enabled_ && ctrl && shift && !alt && (sym == FcitxKey_j || sym == FcitxKey_J) &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper}) &&
      !view_.value("dedicated_english", false)) {
    if (!toggleJapanese()) return false;
    toggle_chord_held_ = FcitxKey_j;
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
        msime::linux_host::strict_json_value(translation_saved_view_, "page_size", size_t{9}), size_t{1}, size_t{9});
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
    // Tab pages the senses the way it pages an ordinary candidate list below; leaving the overlay first would page the Engine's hidden list instead. Use the raw key because normalization can drop Shift and turn ISO_Left_Tab into Tab.
    if (!ctrl && !alt && navigation_.value("tab", true) &&
        !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Mod5}) &&
        (sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab)) {
      translationPage(event.rawKey().states().test(fcitx::KeyState::Shift) ||
                              event.rawKey().sym() == FcitxKey_ISO_Left_Tab
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
      preferences_.value("candidate_translations", true)) {
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
            // 同 commitTranslationCandidate：宿主写入的译文，组字按 HostCommit 结束。
            command(MSIME_CANCEL_AFTER_HOST_COMMIT);
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
    // The XKB keycode (evdev + 8) of the number row is 10..19; Fcitx5 drops Shift from a normalised symbol, so ask the raw event.
    const auto rowCode = event.rawKey().code();
    const bool picks = msime::linux_host::number_row_picks(
        view_, spelled, event.rawKey().states().test(fcitx::KeyState::Shift), rowCode >= 10 && rowCode <= 19,
        preferences_.value("number_row_selection", true) && !view_.value("nine_key", false) &&
            !view_.value("candidates", Json::array()).empty());
    if ((!picks && msime::linux_host::engine_spelling(view_, spelled)) ||
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
  // 分段编辑与 IBus、Windows 和 macOS 保持一致：组字时 Shift+左右和 Ctrl+左右（只按这一个修饰键）在所有方案里都按音节移动拼音光标，全拼和双拼也一样；只按 Ctrl 的退格删除一段，Shift+退格不在此列。共享运行时在横排候选窗里会把字母光标命令换成移动候选，但从不换分段命令，所以高亮停在哪个候选上，这两个组合键都编辑拼音。实际的分段边界由共享运行时决定，局部模式下也能安全退回。韩文音节、注音转换、越南文单词和藏文音节串没有分段，所以这些组合键在下面照旧结束组字，并仍然作为应用的快捷键。没有组字时它们同样是应用的按键。
  if (composing && !commitsOnBlur()) {
    if (ctrl && !alt && !shift && sym == FcitxKey_BackSpace) return command(MSIME_BACKSPACE_SEGMENT);
    const bool segmentChord =
        ctrl != shift && !alt &&
        !states.testAny(fcitx::KeyStates{fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                         fcitx::KeyState::Meta, fcitx::KeyState::Mod5});
    if (segmentChord && (sym == FcitxKey_Left || sym == FcitxKey_KP_Left))
      return command(MSIME_MOVE_LEFT_SEGMENT);
    if (segmentChord && (sym == FcitxKey_Right || sym == FcitxKey_KP_Right))
      return command(MSIME_MOVE_RIGHT_SEGMENT);
  }
  if (states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                      fcitx::KeyState::Super, fcitx::KeyState::Hyper,
                                      fcitx::KeyState::Mod5})) {
    // 韩文音节、注音转换、越南文单词或藏文音节串已经是文字，所以快捷键结束它而不是丢弃它。
    if (composing) command(commitsOnBlur() ? MSIME_FINISH_COMPOSITION : MSIME_CANCEL);
    return false;
  }
  // 还没开始新组字时，CapsLock 打出的大写字母属于编辑器，与 Windows 和 IBus 宿主的按键路由一致。韩文按键不论 CapsLock 如何都是谚文字母，越南文单词可以以大写开头，藏文威利转写的大写字母是另一个字母，所以这三个方案仍然组字。
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
  // 与 Windows（korean_hanja_key）、macOS 和 IBus 一致：候选列表打开时组字里没有光标，四个方向键都移高亮（←↑ 上一个、→↓ 下一个），不看导航开关。
  if (composing && openedList &&
      !states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Shift,
                                       fcitx::KeyState::Super, fcitx::KeyState::Hyper, fcitx::KeyState::Meta,
                                       fcitx::KeyState::Mod5})) {
    switch (sym) {
    case FcitxKey_Left: case FcitxKey_KP_Left: case FcitxKey_Up: case FcitxKey_KP_Up:
      return command(MSIME_PREVIOUS_CANDIDATE);
    case FcitxKey_Right: case FcitxKey_KP_Right: case FcitxKey_Down: case FcitxKey_KP_Down:
      return command(MSIME_NEXT_CANDIDATE);
    default: break;
    }
  }
  // 除此之外韩文音节没有候选。结束它的按键把它作为上屏交给应用，再在应用里做自己的事（转换结果为未处理），和所有韩文输入法一样；Esc 丢弃它，Backspace 退回一个字母。其他按键继续往下走：字母组字，数字或标点经运行时结束音节，其余按键在本函数末尾结束它。列表未打开的注音转换和越南文单词也这样结束。藏文音节串同样走这里，只是 Engine 吞掉结束它的空格和回车（空格带音节点上屏，回车只上屏藏文），转换结果为已处理，按键不再交给应用。
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
    const bool japanese = msime::linux_host::strict_json_value(view_, "scheme", 0u) == 3;
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
        const int first_source = candidates.empty() ? -1 : msime::linux_host::strict_json_value(candidates[0], "source", -1);
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
            return apply(msime_client_select(
                session_, msime::linux_host::strict_json_value(id, "generation", uint64_t{0}),
                msime::linux_host::strict_json_value(id, "index", size_t{0})));
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
    // 全拼和双拼里左右键是整句改字的光标键，引擎进不了改字时自己按字母光标处理。
    case FcitxKey_Left: case FcitxKey_KP_Left:
      return command(msime::linux_host::edits_sentence(view_) ? MSIME_CONVERSION_LEFT : MSIME_MOVE_LEFT);
    case FcitxKey_Right: case FcitxKey_KP_Right:
      return command(msime::linux_host::edits_sentence(view_) ? MSIME_CONVERSION_RIGHT : MSIME_MOVE_RIGHT);
    // Home 分两段：高亮不在首选时回到首选，已在首选时把拼音光标移到开头（msime_client.h 的 MSIME_FIRST_CANDIDATE_OR_HOME）。End 仍只跳到末选。
    case FcitxKey_Home: case FcitxKey_KP_Home:
      return command(MSIME_FIRST_CANDIDATE_OR_HOME);
    case FcitxKey_End: case FcitxKey_KP_End:
      return command(MSIME_LAST_CANDIDATE);
    case FcitxKey_Tab: case FcitxKey_KP_Tab:
      // Normalization can drop Shift and turn ISO_Left_Tab into Tab; the raw key preserves both directions.
      if (navigation_.value("tab", true))
        return command(event.rawKey().states().test(fcitx::KeyState::Shift) ||
                               event.rawKey().sym() == FcitxKey_ISO_Left_Tab
                           ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE);
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
        return apply(msime_client_select(
            session_, msime::linux_host::strict_json_required_integer<uint64_t>(id.at("generation")),
            msime::linux_host::strict_json_required_integer<size_t>(id.at("index"))));
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
      const auto caret = std::min(editing.size(), msime::linux_host::strict_json_value(view_, "caret_position", editing.size()));
      const auto separator = caret ? editing.rfind('\'', caret - 1) : std::string::npos;
      const auto start = separator == std::string::npos ? 0 : separator + 1;
      if ((caret - start) % 2 == 1)
        return apply(msime_client_character(session_, ';', false));
    }
    const bool asciiPunctuation =
        std::ispunct(static_cast<unsigned char>(text[0])) != 0 &&
        // 正在拼写时，撇号在表情、颜文字和日文模式里是 Engine 的输入字符，与 IBus 路由一致。在韩文、注音和越南文里它和别的标点一样跟在打开的组字后面。藏文的撇号列在 spelling_symbols 里，在前面已经作为字符交给 Engine。
        !(text[0] == '\'' && composing && !commitsOnBlur());
    const bool japaneseLongVowel = msime::linux_host::strict_json_value(view_, "scheme", 0u) == 3 && !shift &&
                                   (text[0] == '-' || text[0] == '=');
    if (japaneseLongVowel)
      return apply(msime_client_character(session_, static_cast<uint8_t>(text[0]), false));
    if (asciiPunctuation) {
      if (repeatSmartPunctuationToChinese(text[0]))
        return true;
      // 韩文、越南文和藏文的标点是普通 ASCII，注音的标点只取自 Engine，所以都不补全成对标点。
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
    // Fcitx5 规范化 Shift+字母时会移除 Shift 位；快捷模式入口必须读取原始事件。
    return apply(msime_client_character(session_, static_cast<uint8_t>(text[0]),
                                        event.rawKey().states().test(fcitx::KeyState::Shift)));
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
    global_theme_package_items_.back()->registerAction(MSIME_EDITION_FCITX5_ADDON "-global-theme-package-" + id,
                                                       &instance_->userInterfaceManager());
    global_theme_menu_.addAction(global_theme_package_items_.back().get());
  }
  global_theme_packages_ = std::move(packages);
  ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

// The scheme menu is shared by every context, so it follows the runtime options the last context read, as the theme menu does. The Chinese schemes come first and the other input languages after them, as in the IBus menu; nothing is rebuilt while the languages stay the same, so an entry is never replaced under a menu that shows it.
void FcitxEngine::rebuildSchemeMenu(fcitx::InputContext *ic, bool cantonese, bool zhuyin, bool stroke) {
  const std::tuple languages{cantonese, zhuyin, stroke};
  if (scheme_menu_languages_ == languages) return;
  for (auto *entry : scheme_menu_entries_) scheme_menu_.removeAction(entry);
  scheme_menu_entries_.clear();
  scheme_menu_entries_.reserve(10);
  // 只列本版本提供的方案（版本表的 input_schemes）：不提供的方案选了也会被 selectScheme 拒绝。粤拼、注音与笔画另外要有词库，cantonese、zhuyin 与 stroke 已经算上了版本。
  // 顺序与 macOS 的子菜单和 Engine 的方案编号相同（MenuLabels.h 的 kSchemeMenuOrder）：全拼、双拼、五笔、日语、韩语、粤拼、注音、越南语、藏文、笔画。
  for (const auto &[id, action] : std::initializer_list<std::pair<std::string_view, fcitx::Action *>>{
           {"quanpin", &scheme_quanpin_action_}, {"shuangpin", &scheme_shuangpin_action_},
           {"wubi", &scheme_wubi_action_}, {"japanese", &scheme_japanese_action_},
           {"korean", &scheme_korean_action_}, {"cantonese", &scheme_cantonese_action_},
           {"zhuyin", &scheme_zhuyin_action_}, {"vietnamese", &scheme_vietnamese_action_},
           {"tibetan", &scheme_tibetan_action_}, {"stroke", &scheme_stroke_action_}}) {
    const bool offered = id == "cantonese" ? cantonese
                         : id == "zhuyin"  ? zhuyin
                         : id == "stroke"  ? stroke
                                           : msime::linux_host::edition_offers_scheme(id);
    if (offered) scheme_menu_entries_.push_back(action);
  }
  for (auto *entry : scheme_menu_entries_) scheme_menu_.addAction(entry);
  scheme_menu_languages_ = languages;
  if (ic) ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

void FcitxEngine::rebuildToolbarMenu(fcitx::InputContext *ic) {
  for (auto *action : toolbar_entries_)
    toolbar_menu_.removeAction(action);
  toolbar_entries_.clear();
  toolbar_entries_.reserve(16);
  if (!ic) return;
  const auto *state = ic->propertyFor(&factory_);
  if (!state->session_) return;
  const auto toolbar = state->preferences_.value("floating_toolbar", Json::object());
  if (!toolbar.value("enabled", true)) return;
  // 按钮的顺序和默认开关与 macOS 悬浮工具栏相同（FloatingToolbarPanel.mm；默认值见 preferences.rs 的 FloatingToolbarPreferences）：english_mode 管的是中英文那一项（macOS 的 _inputModeButton），macOS 工具栏没有「英文候选模式」，它只在状态区顶层；表情、手写、屏幕键盘和语音默认关。之后是 macOS 齿轮菜单里的宣传、官网、帮助、关于与反馈。条目就是状态区里的同一个动作，不是副本。托盘留着失焦的上下文时，语音和宣传这两项要往有焦点的输入框里写，点了没有地方落，不列（与 arrangeStatusArea 对 focusOnlyActions 的处理相同）。
  const bool focused = ic->hasFocus();
  const auto append = [&](bool present, fcitx::Action *action) {
    if (!present) return;
    toolbar_menu_.addAction(action);
    toolbar_entries_.push_back(action);
  };
  append(toolbar.value("english_mode", true), &input_mode_action_);
  append(toolbar.value("input_scheme", true), &scheme_action_);
  append(toolbar.value("punctuation", true), &chinese_punctuation_action_);
  append(toolbar.value("fullwidth", true), &width_action_);
  append(toolbar.value("character_set", true), &traditional_action_);
  append(toolbar.value("emoji", false), &desktop_emoji_action_);
  append(MSIME_EDITION_HANDWRITING != 0 && toolbar.value("handwriting", false), &handwriting_action_);
  append(toolbar.value("screen_keyboard", false), &keyboard_action_);
  append(toolbar.value("voice", false) && state->voice_enabled_ && focused, &voice_action_);
  append(toolbar.value("settings", true), &settings_action_);
  append(true, &toolbar_separator_);
  append(focused, &promotion_action_);
  append(true, &website_action_);
  append(true, &help_action_);
  append(true, &about_action_);
  append(true, &feedback_action_);
}

class FcitxFactory : public fcitx::AddonFactory {
public:
  fcitx::AddonInstance *create(fcitx::AddonManager *manager) override { return new FcitxEngine(manager->instance()); }
};
} // namespace msime::fcitx_host

FCITX_ADDON_FACTORY(msime::fcitx_host::FcitxFactory)
