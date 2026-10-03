#include "ClientEngine.h"
#include "LinuxEdition.h"
#include "KeyRouterAdapter.h"
#include "BackspaceHoldPolicy.h"
#include "../clipboard/ClipboardText.h"
#include "../clipboard/ClipboardAtomicWrite.h"
#include "../system/ChineseTextConversion.h"
#include "HelpcodeDefaults.h"
#include "HelpcodeSchemaNames.h"
#include "NavigationBindings.h"
#include "NativeCompose.h"
#include "PhrasePreedit.h"
#include "JapaneseConversion.h"
#include "KoreanHanja.h"
#include "InputSchemes.h"
#include "CandidateSkinCatalog.h"
#include "GlobalTheme.h"
#include "DictionaryQuiesceLease.h"
#include "InputModeIndicator.h"
#include "ReplacedProgram.h"
#include "SmartPunctuationSpace.h"
#include "SpellingSymbols.h"
#include "PreparePaths.h"
#include "LocalModeSwitches.h"
#include "WordCharacterBinding.h"
#include "SurroundingCharacters.h"
#include "../voice/VoiceAction.h"
#include "../voice/VoiceHotwords.h"
#include "../voice/VoiceProviderOptions.h"
#include "../voice/VoiceWorker.h"
#include "../overlay/WaveOverlayModel.h"
#include "../overlay/WaveOverlayIbusSurface.h"
#include "../overlay/WaveOverlaySurfaceFactory.h"
#include "../candidates/CandidateColors.h"
#include "../candidates/CandidatePalette.h"
#include "../candidates/CandidateFontPolicy.h"
#include "../candidates/PanelRestoreRecord.h"
#include "../candidates/CandidateActionPolicy.h"
#include "../candidates/CandidateLocalModeLabels.h"
#include "../candidates/CandidatePanelStatus.h"
#include "../candidates/CandidateTranslationPolicy.h"
#include "../candidates/PairedPunctuation.h"
#include "../candidates/ShuangpinProfileNames.h"
#include "../candidates/WubiProfileNames.h"
#include "ClientInputModeMemory.h"
#include "../system/DiagnosticLog.h"
#include "../system/KeySound.h"
#include "../system/PanelInputChannel.h"
#include "../system/TypingStatistics.h"
#include "msime_client.h"
#include <algorithm>
#include <atomic>
#include <array>
#include <cctype>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <fcntl.h>
#include <glib-unix.h>
#include <initializer_list>
#include <memory>
#include <nlohmann/json.hpp>
#include <optional>
#include <set>
#include <tuple>
#include <cstdlib>
#include <stdexcept>
#include <string_view>
#include <sys/file.h>
#include <unistd.h>
#include <vector>

using Json = nlohmann::json;
struct MsimeIbusEngine;
namespace {
// 主题目录由共享层发布，这个宿主不存 id 或标题的副本；菜单是这份目录加上运行配置里列出的外部皮肤。
std::vector<msime::linux_host::ThemeChoice> theme_choices();
Json configured;
// Which language dictionaries the configured options name, worked out when they are loaded (msime_ibus_configure) so the key path never looks at the disk.
msime::linux_host::LanguageDictionaryAvailability configured_dictionaries;
uint64_t configuration_generation = 0;
std::atomic<uint64_t> next_client_token{1};
// Store acceptance is shared by all contexts and survives session recreation.
// Effective runtime revisions also include local overrides and are independent.
std::string accepted_preferences_directory;
Json accepted_preferences_snapshot;
bool menu_save_pending = false;
uint64_t menu_status_generation = 0;

// GNOME Shell starts ibus-daemon with its panel disabled and draws the candidate popup itself from the shell theme, so neither the panel font nor the colour attributes reach it. The desktop name says which session this is, and the shell's bus name confirms the shell is the one running; the answer holds for the life of the process.
bool candidate_panel_is_gnome_shell() {
  static const bool gnome_shell = [] {
    if (!msime::linux_host::candidate_desktop_is_gnome_shell(g_getenv("XDG_CURRENT_DESKTOP")))
      return false;
    GError *error = nullptr;
    auto *connection = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
    if (!connection) {
      g_clear_error(&error);
      return true;
    }
    auto *reply = g_dbus_connection_call_sync(
        connection, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
        "NameHasOwner", g_variant_new("(s)", "org.gnome.Shell"), G_VARIANT_TYPE("(b)"),
        G_DBUS_CALL_FLAGS_NONE, 1000, nullptr, &error);
    g_object_unref(connection);
    if (!reply) {
      g_clear_error(&error);
      return true;
    }
    gboolean owned = FALSE;
    g_variant_get(reply, "(b)", &owned);
    g_variant_unref(reply);
    return owned == TRUE;
  }();
  return gnome_shell;
}

// Tell the settings page whether the desktop panel honours the candidate font, colours and skin (see candidates/CandidatePanelStatus.h). The write is skipped when the file already says the same.
void publish_candidate_panel_status() {
  const auto file = msime::linux_host::candidate_panel_status_file(g_get_user_runtime_dir());
  if (!file) return;
  msime::linux_host::write_candidate_panel_status(
      *file, msime::linux_host::candidate_panel_status_document(
                 "ibus", candidate_panel_is_gnome_shell() ? msime::linux_host::CandidatePanelLimit::GnomeShell
                                                          : msime::linux_host::CandidatePanelLimit::None));
}

// The panel keys are the desktop's, so before one changes, what it held is recorded for msime-linux-setup --unregister (see PanelRestoreRecord.h): the user's own value, or null for a key left at the schema default, which uninstall resets. A failed record does not hold the change back.
void record_ibus_panel_takeover(GSettings *settings, const char *key, const Json &written) {
  const auto file = msime::linux_host::panel_restore_file(std::getenv("XDG_STATE_HOME"), std::getenv("HOME"));
  if (!file) return;
  Json current(nullptr);
  if (auto *value = g_settings_get_user_value(settings, key)) {
    if (g_variant_is_of_type(value, G_VARIANT_TYPE_STRING))
      current = g_variant_get_string(value, nullptr);
    else if (g_variant_is_of_type(value, G_VARIANT_TYPE_BOOLEAN))
      current = static_cast<bool>(g_variant_get_boolean(value));
    g_variant_unref(value);
  }
  msime::linux_host::record_panel_takeover(*file, "ibus", key, current, written);
}

// The IBus panel draws the candidate list from one font description the whole desktop shares, the same pair of keys ibus-setup writes. A desktop without the schema has nothing to write and is left alone, and so is GNOME Shell: its popup follows the shell theme, and turning on use-custom-font there would only change ibus-setup's own panel for a panel that never shows.
void apply_candidate_panel_font(const Json &preferences) {
  publish_candidate_panel_status();
  if (candidate_panel_is_gnome_shell()) return;
  static msime::linux_host::CandidateFontSync sync;
  const auto description = sync.next(msime::linux_host::read_candidate_font(preferences));
  if (!description) return;
  auto *source = g_settings_schema_source_get_default();
  auto *schema = source ? g_settings_schema_source_lookup(source, "org.freedesktop.ibus.panel", TRUE)
                        : nullptr;
  if (!schema) return;
  const bool writable = g_settings_schema_has_key(schema, "custom-font") &&
                        g_settings_schema_has_key(schema, "use-custom-font");
  g_settings_schema_unref(schema);
  if (!writable) return;
  auto *settings = g_settings_new("org.freedesktop.ibus.panel");
  record_ibus_panel_takeover(settings, "custom-font", Json(*description));
  record_ibus_panel_takeover(settings, "use-custom-font", Json(true));
  g_settings_set_string(settings, "custom-font", description->c_str());
  g_settings_set_boolean(settings, "use-custom-font", TRUE);
  g_object_unref(settings);
}

bool system_dark = false;
msime::linux_host::CandidateTheme candidate_theme(const Json &preferences);
std::optional<bool> global_input_enabled;
// The shared preference defaults, read once from the Host API rather than
// restated here. A nested preference object is optional as a whole but requires
// every one of its members, so patching a single key into an object the
// runtime-options document happens to omit produces a partial object the API
// rejects - and the host then cannot create a session at all.
const Json &shared_preference_defaults() {
  static const Json defaults = [] {
    std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
        msime_client_default_preferences(), msime_client_string_free);
    if (!owned)
      return Json::object();
    auto document = Json::parse(owned.get(), nullptr, false);
    if (document.is_discarded() || !document.is_object() ||
        !document.value("ok", false))
      return Json::object();
    auto value = document.at("value");
    return value.is_object() ? value : Json::object();
  }();
  return defaults;
}
// Patch `overrides` into the named nested preference object, completing any
// member the document omits from the shared defaults.
void patch_preference_object(Json &preferences, const char *name,
                             const Json &overrides) {
  auto &target = preferences[name];
  if (!target.is_object())
    target = Json::object();
  const auto &defaults = shared_preference_defaults().value(name, Json::object());
  if (defaults.is_object())
    for (const auto &[key, value] : defaults.items())
      if (!target.contains(key))
        target[key] = value;
  for (const auto &[key, value] : overrides.items())
    target[key] = value;
}
void register_properties(IBusEngine *engine);
void page(IBusEngine *engine, uint32_t command);
Json response(char *raw) {
  std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
      raw, msime_client_string_free);
  if (!raw)
    throw std::runtime_error("Missing host response");
  auto document = Json::parse(raw);
  if (!document.at("ok").get<bool>())
    throw std::runtime_error("Host operation failed");
  return document.at("value");
}
std::vector<msime::linux_host::ThemeChoice> theme_choices() {
  static const Json catalog = [] {
    try {
      return response(msime_client_theme_catalog());
    } catch (const std::exception &) {
      return Json::object();
    }
  }();
  return msime::linux_host::theme_choices(catalog, msime::linux_host::parse_configured_skins(configured));
}
// The candidate colours for one preferences document, resolved by the shared layer (msime_client_resolve_theme) in the given mode. The package is a catalogue entry the shared layer reads strictly, and one it refuses fails the whole call, so that costs only the package: the theme is resolved again without it. A call that still fails draws the native tokens.
msime::linux_host::CandidateTheme theme_in_mode(const Json &preferences, bool dark) {
  const auto catalog =
      configured.is_object() ? configured.value("candidate_skin_catalog", Json(nullptr)) : Json(nullptr);
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
msime::linux_host::CandidateTheme candidate_theme(const Json &preferences) {
  return theme_in_mode(preferences, msime::linux_host::candidate_dark_theme(preferences, system_dark));
}
IBusOrientation candidate_orientation(const Json &preferences);
std::string preedit_style(const Json &preferences);
bool launch_desktop_panel(const char *panel);
enum class MenuPreference { Toolbar, CloudCandidates, CandidateTranslations, TranslationLanguage, CandidateTheme, PreeditStyle, CandidateLayout, GlobalTheme, CandidatePageSize, FrequencyMode, FrequencyTriggerCount, FrequencyLinearStep, Learning, ShuangpinPreedit, WubiCodeHint, SmartPunctuation, SmartPunctuationRepeat, PairedPunctuation, PunctuationLock, AutocorrectTransposition, AutocorrectNeighbor, EnglishCandidates, EmojiCandidates, KaomojiCandidates, QuanpinHelpcode, ShuangpinHelpcode, QuanpinHelpcodeSchema, ShuangpinHelpcodeSchema, ShuangpinProfile, InputScheme, NineKey, LocalMode, NumberRowSelection, WordCharacter, TraditionalOutput, ChinesePunctuation, ClipboardHistoryEnabled, CharacterWidth, VoiceEnabled };
void save_menu_preference(IBusEngine *engine, MenuPreference preference, Json value);
struct FailedMenuSave {
  MenuPreference preference;
  Json value;
  std::string directory;
  uint64_t configuration;
};
std::optional<FailedMenuSave> failed_menu_save;

void voice_cancel(IBusEngine *engine);
void voice_stop(IBusEngine *engine);
std::string configured_clipboard_path(const Json &options) {
  const auto explicit_path = options.value("clipboard_history_path", std::string{});
  if (!explicit_path.empty())
    return explicit_path;
  const auto directory = options.value("preferences_directory", std::string{});
  if (directory.empty() || directory.front() != '/')
    return {};
  return (std::filesystem::path(directory) / "clipboard_history.json").string();
}
std::string provider_socket_fallback(const Json &options, const char *option,
                                      const char *environment, const char *filename) {
  auto value = options.value(option, std::string{});
  if (!value.empty())
    return value;
  if (const auto *socket = g_getenv(environment); socket && *socket)
    return socket;
  const auto *runtime = g_get_user_runtime_dir();
  if (!runtime || !*runtime)
    return {};
  const auto candidate = std::filesystem::path(runtime) / MSIME_EDITION_CLIENT_DIRECTORY / filename;
  std::error_code error;
  return std::filesystem::is_socket(candidate, error) ? candidate.string() : std::string{};
}
struct State;
bool script_conversion_applies(const Json &context);
std::string traditional_display(const State &s, const Json &context,
                                std::string text);
struct State {
  msime::linux_host::NativeCompose native_compose;
  MsimeVoiceWorker voice_worker;
  uint64_t session = 0;
  uint64_t client_token = 0;
  uint64_t focus_epoch = 0;
  msime::linux_host::KeyRouterAdapter key_router;
  Json view;
  Json rendered_view;
  // IBus reports only the row index for a candidate click. Keep the exact
  // candidate page that was most recently handed to the panel so an
  // asynchronous Engine refresh cannot make that index resolve against a
  // different page.
  Json rendered_candidates = Json::array();
  // Ctrl+Enter can expose multiple senses as a short-lived Linux-native
  // candidate page. Keep the Engine view immutable while that page is shown.
  bool translation_candidates_active = false;
  Json translation_saved_view;
  std::vector<std::string> translation_options;
  size_t translation_page = 0;
  size_t translation_cursor = 0;
  int rendered_scheme = 255;
  uint64_t rendered_session = 0;
  bool focused = false;
  std::string focused_context;
  std::string focused_client;
  bool blocked = false;
  bool private_input = false;
  // What the process's sound player was last told about background music; see sync_music.
  msime::linux_host::MusicActivity music;
  // The combo count the session last answered msime_client_typing_effect with, shown at the end of the candidate aux line, and the key held down, so an auto-repeat is drawn but not counted.
  uint32_t typing_combo = 0;
  msime::linux_host::KeyRepeat key_repeat;
  // Per-key press counts for the key heatmap, written in batches; see KeyPressCounter.
  msime::linux_host::KeyPressCounter key_presses;
  guint preferences_timer = 0;
  bool preferences_loading = false;
  // IBus hide notifications can trail the next confirmed candidate update;
  // retain the last lookup table for one short grace window while invalidating
  // its action snapshot immediately.
  guint candidate_hide_source = 0;
  // GNOME Shell rebuilds the whole nested property menu on every update.
  // Candidate identity changes on each letter, so publish the menu only after
  // the user pauses instead of making the desktop rebuild it per keystroke.
  guint candidate_properties_source = 0;
  uint64_t candidate_hide_serial = 0;
  uint64_t seen_menu_status_generation = 0;
  uint64_t seen_menu_configuration = 0;
  bool input_enabled = true;
  bool mode_scope_global = false;
  msime::linux_host::ClientInputModeMemory app_input_modes;
  bool chinese_punctuation = true;
  bool properties_registered = false;
  std::optional<bool> english_override;
  std::optional<bool> dedicated_english_override;
  std::optional<bool> cloud_candidates_override;
  std::optional<bool> candidate_translations_override;
  std::optional<bool> traditional_output_override;
  std::optional<bool> emoji_override;
  std::optional<bool> kaomoji_override;
  std::optional<bool> punctuation_override, helpcode_override;
  // What the session was last told. The runtime keeps the host's punctuation
  // toggle as an override that outranks the preferences it is handed, so a
  // preference change only reaches the session when the host re-states it.
  std::optional<bool> session_chinese_punctuation;
  // The width the session was last told. update_preferences never touches the runtime's width, so a preference or menu change reaches the session only when the host re-states it.
  std::optional<bool> session_fullwidth;
  std::optional<bool> autocorrect_transposition_override, autocorrect_neighbor_override;
  bool show_helpcode_in_candidate_window = true;
  std::optional<bool> word_character_override;
  std::optional<bool> smart_punctuation_override, smart_repeat_override, paired_punctuation_override;
  std::optional<std::string> punctuation_lock_override;
  std::optional<uint8_t> candidate_page_size_override;
  std::optional<std::string> frequency_mode_override, helpcode_schema_override;
  std::optional<uint8_t> frequency_trigger_count_override, frequency_linear_step_override;
  std::optional<bool> learning_override;
  std::optional<bool> shuangpin_preedit_override;
  std::optional<bool> wubi_code_hint_override;
  std::optional<std::string> layout_override, preedit_override, theme_override;
  std::optional<std::string> scheme_override, shuangpin_profile_override;
  // A 主题 menu choice being saved, as theme_choice_change writes it.
  std::optional<Json> theme_choice_override;
  std::optional<std::string> translation_target_language_override;
  std::optional<bool> nine_key_override;
  Json local_mode_overrides = Json::object();
  bool fullwidth = false;
  bool english_mode = false;
  bool traditional_output = false;
  std::string candidate_preedit_style = "pinyin";
  bool show_candidate_page_number = true;
  bool learning = true;
  uint8_t frequency_trigger_count = 1;
  uint8_t frequency_linear_step = 1;
  bool shuangpin_preedit_uses_raw = true;
  bool wubi_code_hint = true;
  bool smart_punctuation = true;
  bool smart_punctuation_repeat = true;
  bool smart_punctuation_space_convert = false;
  // Which preceding characters keep a smart mark ASCII: the settings page's "direct digit" and "direct letter" switches, which the shared route (and so the Fcitx5 host) already honours.
  bool smart_punctuation_direct_digit = true;
  bool smart_punctuation_direct_letter = true;
  bool paired_punctuation = true;
  bool pure_shift_candidate = false;
  bool pure_ctrl_candidate = false;
  bool shift_down = false;
  bool ctrl_down = false;
  bool right_ctrl_down = false;
  bool left_ctrl_down = false;
  bool mode_chord_held = false;
  // Ctrl+Shift+F owns its stroke the way mode_chord_held owns Space: repeats while it is held toggle nothing, even while the first toggle's save is still pending.
  bool character_set_chord_held = false;
  std::set<guint> host_shortcut_strokes;
  gint64 modifier_toggle_deadline = 0;
  void reset_mode_modifiers() {
    pure_shift_candidate = false;
    pure_ctrl_candidate = false;
    shift_down = false;
    ctrl_down = false;
    right_ctrl_down = false;
    left_ctrl_down = false;
    modifier_toggle_deadline = 0;
    mode_chord_held = false;
    character_set_chord_held = false;
    host_shortcut_strokes.clear();
  }
  bool mode_shift_enabled = true;
  bool mode_ctrl_enabled = false;
  bool mode_ctrl_space_enabled = true;
  bool mode_ctrl_alt_space_enabled = true;
  bool character_set_shortcut_enabled = true;
  bool number_row_selection = true;
  std::optional<bool> number_row_override;
  char last_smart_punctuation = 0;
  gint64 last_smart_punctuation_time = 0;
  // A deleted ASCII smart mark keeps this caret position on the Chinese path
  // when the same key is immediately retyped, matching the Windows behavior.
  char smart_punctuation_rejected = 0;
  // The Chinese mark Engine just committed with nothing composing. A bare
  // Space arriving next takes that mark back to ASCII; any other key disarms
  // it. `space_convert_preceding` is the character that stood
  // in front of the mark when it was committed - the document usually holds the
  // same mark in several places, and moving the caret inside one window is not
  // a focus change, so the rewrite re-reads both characters before touching
  // anything. An empty value means the mark was committed at the start of the
  // document and there is nothing to fingerprint.
  std::string space_convert_mark;
  std::string space_convert_preceding;
  std::string punctuation_lock = "follow";
  std::string preedit_style = "raw";
  std::optional<guint> candidate_text_color, candidate_background_color;
  std::optional<guint> candidate_number_color, candidate_accent_color;
  std::optional<guint> candidate_selected_color;
  std::optional<guint> candidate_selected_text_color;
  std::optional<guint> candidate_selected_number_color;
  std::optional<guint> candidate_translation_color;
  IBusOrientation candidate_orientation = IBUS_ORIENTATION_VERTICAL;
  msime::linux_host::NavigationBindings navigation;
  msime::linux_host::WordCharacterBinding word_character;
  msime::linux_host::PairedPunctuationTracker paired_tracker;
  // Quote and book-title state for Chinese marks committed in English mode under the Chinese punctuation lock or after Ctrl+.; reset on every mode switch.
  msime::linux_host::EnglishPunctuationState english_punctuation;
  // Ctrl+. pressed in English mode under the "follow" lock: English mode types Chinese punctuation until the next Chinese/English switch, as Windows does with its punctuation compartment on and the IME closed. Kept apart from chinese_punctuation, which a focus or preference refresh re-derives from the saved preference.
  bool english_chinese_punctuation = false;
  // Japanese converts with Space and commits with Enter; see core/JapaneseConversion.h.
  msime::linux_host::JapaneseConversion japanese_conversion;
  msime::linux_host::BackspaceHoldPolicy backspace_hold;
  std::string ai_context;
  void remember_commit(const std::string &text) {
    if (!focused || blocked || private_input) {
      ai_context.clear();
      return;
    }
    ai_context += text;
    if (ai_context.size() > 1024) {
      size_t cut = ai_context.size() - 1024;
      while (cut < ai_context.size() &&
             (static_cast<unsigned char>(ai_context[cut]) & 0xc0) == 0x80)
        ++cut;
      ai_context.erase(0, cut);
    }
  }
  std::string clipboard_history_path, online_provider_socket,
      translation_provider_socket;
  std::string voice_provider_socket, voice_language = "zh-cn";
  bool voice_enabled = true;
  bool voice_hotkey_ralt = true;
  bool voice_hotkey_ctrl_win = false;
  bool voice_hotkey_rctrl_ralt = false;
  bool voice_hotkey_hold_space_lock = true;
  bool voice_hotkey_ctrl_f9 = true;
  // The lock state as the last key event reported it; IBus sends no event of its own when it changes.
  bool caps_lock = false;
  // Key ownership lasts until release, independently of provider completion.
  std::set<guint> voice_consumed_keys;
  guint voice_hold_key = 0;
  bool voice_space_consumed = false;
  bool voice_space_locked = false;
  bool voice_active = false;
  bool voice_stopping = false;
  bool voice_requires_control = false;
  uint64_t voice_generation = 0;
  uint64_t voice_failure_id = 0;
  // 中英文切换提示：辅助区域短暂显示「中」或「英」。代次用于丢弃过期的隐藏回调，
  // 与语音失败提示同一套做法。
  uint64_t mode_hint_id = 0;
  // 右键候选提示：辅助区域短暂指向「候选操作」菜单。代次用于丢弃过期的恢复回调。
  uint64_t candidate_menu_hint_id = 0;
  std::string voice_preedit;
  std::string voice_transcript;
  std::string voice_phase = "正在录音…";
  std::optional<unsigned> voice_level;
  msime::linux_host::WaveOverlayModel wave_overlay;
  std::unique_ptr<msime::linux_host::WaveOverlaySurface> wave_overlay_surface;
  bool wave_overlay_visible = false;
  std::shared_ptr<std::atomic_bool> alive =
      std::make_shared<std::atomic_bool>(true);
  std::vector<std::string> clipboard_items_cache;
  uint64_t clipboard_generation = 0;
  bool clipboard_loading = false, clipboard_loaded = false;
  bool clipboard_enabled = true;
  uint64_t applied_preferences_revision = 0;
  uint64_t applied_display_generation = 0;
  Json applied_preferences_snapshot;
  GFileMonitor *clipboard_monitor = nullptr;
  GFile *clipboard_watch_file = nullptr;
  void stop_clipboard_monitor() {
    if (clipboard_monitor) {
      g_file_monitor_cancel(clipboard_monitor);
      g_clear_object(&clipboard_monitor);
    }
    g_clear_object(&clipboard_watch_file);
  }
  void remember_app_input_mode() {
    if (!mode_scope_global)
      app_input_modes.remember(focused_client, input_enabled);
  }
  // A client identity that arrives after an anonymous focus describes the focus
  // already on screen, not a new one. Carry the mode the user is looking at
  // into that identity, unless we already remember one for it.
  void adopt_app_input_mode(std::string_view client) {
    if (mode_scope_global || client.empty() || app_input_modes.knows(client))
      return;
    app_input_modes.remember(client, input_enabled);
  }
  void restore_app_input_mode() {
    if (mode_scope_global)
      return;
    const auto fallback = configured.value("preferences", Json::object())
                              .value("default_ime_mode", "chinese") != "english";
    input_enabled = app_input_modes.restore(focused_client, fallback);
  }
  void configure_clipboard(std::string path, bool enabled) {
    if (!path.empty() && path.front() != '/')
      path.clear();
    if (path == clipboard_history_path && enabled == clipboard_enabled)
      return;
    stop_clipboard_monitor();
    ++clipboard_generation;
    clipboard_loaded = false;
    clipboard_items_cache.clear();
    clipboard_history_path = std::move(path);
    clipboard_enabled = enabled;
  }
  std::array<bool, 2> online_loading{};
  bool translation_loading = false;
  guint online_delay_source = 0;
  guint ai_delay_source = 0;
  guint translation_delay_source = 0;
  guint settled_rerank_source = 0;
  bool cloud_candidates = true;
  bool candidate_translations = true;
  bool candidate_english_gloss = false;
  bool translation_reset_pending = false;
  uint64_t sentence_translation_generation = 0;
  std::string translation_target_language = "en";
  uint64_t provider_epoch = 0;
  std::string translation_dispatched_query;
  std::array<std::string, 2> online_dispatched_query;
  void invalidate_providers() {
    if (online_delay_source) {
      const auto source = online_delay_source;
      online_delay_source = 0;
      g_source_remove(source);
    }
    if (ai_delay_source) {
      const auto source = ai_delay_source;
      ai_delay_source = 0;
      g_source_remove(source);
    }
    if (translation_delay_source) {
      const auto source = translation_delay_source;
      translation_delay_source = 0;
      g_source_remove(source);
    }
    if (settled_rerank_source) {
      const auto source = settled_rerank_source;
      settled_rerank_source = 0;
      g_source_remove(source);
    }
    ++provider_epoch;
    online_loading.fill(false);
    translation_loading = false;
    sentence_translation_generation = 0;
    translation_dispatched_query.clear();
    for (auto &query : online_dispatched_query) query.clear();
  }
  std::string surrounding_text;
  bool surrounding_valid = false;
  bool surrounding_utf16 = false;
  // Preserve client units until use: focus identity may arrive after text.
  guint surrounding_cursor = 0;
  guint surrounding_anchor = 0;
  ~State() {
    alive->store(false);
    close();
  }
  void close() {
    session_chinese_punctuation.reset();
    session_fullwidth.reset();
    if (candidate_hide_source) {
      const auto source = candidate_hide_source;
      candidate_hide_source = 0;
      g_source_remove(source);
    }
    if (candidate_properties_source) {
      const auto source = candidate_properties_source;
      candidate_properties_source = 0;
      g_source_remove(source);
    }
    ++candidate_hide_serial;
    translation_reset_pending = false;
    applied_preferences_revision = 0;
    applied_preferences_snapshot = nullptr;
    stop_clipboard_monitor();
    ai_context.clear();
    if (voice_active && !voice_provider_socket.empty())
      msime_client_string_free(msime_client_voice_provider_cancel(
          reinterpret_cast<const uint8_t *>(voice_provider_socket.data()),
          voice_provider_socket.size(), voice_generation));
    if (voice_active && session)
      msime_client_string_free(msime_client_voice_cancel(session));
    music.release(session, msime_client_music_set_active);
    // The combo lives in the session; the next one starts from none.
    typing_combo = 0;
    key_repeat.reset();
    voice_active = false;
    voice_generation = 0;
    voice_preedit.clear();
    voice_transcript.clear();
    wave_overlay.reset();
    voice_consumed_keys.clear();
    voice_hold_key = 0;
    voice_space_consumed = false;
    voice_space_locked = false;
    reset_mode_modifiers();
    voice_worker.cancel_async();
    invalidate_providers();
    ++clipboard_generation;
    clipboard_loading = false;
    clipboard_loaded = false;
    clipboard_items_cache.clear();
    if (session)
      msime_client_string_free(msime_client_destroy(session));
    session = 0;
    if (focused && client_token != 0)
      key_router.set_lease(
          {client_token, focus_epoch,
           msime::linux_host::KeyRouterAdapter::lease_token(client_token,
                                                             session)});
    view = nullptr;
    rendered_view = nullptr;
    rendered_candidates = Json::array();
    rendered_scheme = 255;
    rendered_session = 0;
    surrounding_text.clear();
    surrounding_valid = false;
    surrounding_cursor = 0;
    surrounding_anchor = 0;
    last_smart_punctuation = 0;
    last_smart_punctuation_time = 0;
    smart_punctuation_rejected = 0;
    space_convert_mark.clear();
    space_convert_preceding.clear();
    paired_tracker.clear();
    japanese_conversion.reset();
    backspace_hold.reset();
  }
  void open() {
    auto options = configured;
    auto &base_preferences = options["preferences"];
    mode_scope_global =
        base_preferences.value("ime_mode_scope", "app") == "global";
    if (mode_scope_global) {
      if (!global_input_enabled)
        global_input_enabled =
            base_preferences.value("default_ime_mode", "chinese") != "english";
      if (!session)
        input_enabled = *global_input_enabled;
    }
    if (session || blocked || !focused || !input_enabled)
      return;
    // Dictionary maintenance is running from the settings window; keys go to the application until it is done.
    if (msime::linux_host::dictionary_quiesced(options.value("user_data", std::string{})))
      return;
    number_row_selection = number_row_override.value_or(
        options.value("preferences", Json::object()).value("number_row_selection", true));
    auto &preferences = options["preferences"];
    if (punctuation_override) preferences["chinese_punctuation"] = *punctuation_override;
    if (paired_punctuation_override) preferences["paired_punctuation"] = *paired_punctuation_override;
    if (punctuation_lock_override) preferences["punctuation_lock"] = *punctuation_lock_override;
    if (scheme_override) preferences["scheme"] = *scheme_override;
    if (shuangpin_profile_override) preferences["shuangpin_profile"] = *shuangpin_profile_override;
    if (candidate_page_size_override) preferences["candidate_page_size"] = *candidate_page_size_override;
    fullwidth = preferences.value("character_width", "halfwidth") == "fullwidth";
    if (layout_override) preferences["candidate_layout"] = *layout_override;
    if (preedit_override) preferences["tsf_preedit_style"] = *preedit_override;
    if (theme_override) preferences["candidate_theme"] = *theme_override;
    if (theme_choice_override) msime::linux_host::apply_theme_choice(preferences, *theme_choice_override);
    // Default snapshots omit the empty quanpin override object.
    if (!preferences.contains("quanpin"))
      preferences["quanpin"] = Json::object();
    auto &quanpin = preferences["quanpin"];
    if (autocorrect_transposition_override)
      quanpin["autocorrect_transposition"] = *autocorrect_transposition_override;
    if (autocorrect_neighbor_override)
      quanpin["autocorrect_neighbor"] = *autocorrect_neighbor_override;
    if (frequency_mode_override) preferences["frequency"]["mode"] = *frequency_mode_override;
    if (frequency_trigger_count_override)
      preferences["frequency"]["trigger_count"] = *frequency_trigger_count_override;
    if (frequency_linear_step_override)
      preferences["frequency"]["linear_step"] = *frequency_linear_step_override;
    if (learning_override) preferences["learning"] = *learning_override;
    if (shuangpin_preedit_override)
      preferences["shuangpin_preedit_uses_raw"] = *shuangpin_preedit_override;
    if (wubi_code_hint_override)
      preferences["wubi_code_hint"] = *wubi_code_hint_override;
    const auto active_scheme = preferences.value("scheme", "quanpin");
    if (active_scheme == "quanpin" || active_scheme == "shuangpin") {
      if (helpcode_override) preferences[active_scheme + "_helpcode"]["enabled"] = *helpcode_override;
      if (helpcode_schema_override) preferences[active_scheme + "_helpcode"]["schema"] = *helpcode_schema_override;
      show_helpcode_in_candidate_window = preferences.value(
          active_scheme + "_helpcode", Json::object())
          .value("show_in_candidate_window",
                 msime::linux_host::default_show_helpcode(active_scheme));
    } else {
      show_helpcode_in_candidate_window = true;
    }
    configure_clipboard(configured_clipboard_path(options),
                        preferences.value("clipboard_history", false));
    online_provider_socket = provider_socket_fallback(
        options, "online_provider_socket", "MSIME_ONLINE_PROVIDER_SOCKET", "online.sock");
    translation_provider_socket = provider_socket_fallback(
        options, "translation_provider_socket", "MSIME_TRANSLATION_PROVIDER_SOCKET",
        "translation.sock");
    if (translation_provider_socket.empty())
      translation_provider_socket = online_provider_socket;
    voice_provider_socket = provider_socket_fallback(
        options, "voice_provider_socket", "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
    const auto voice_preferences = preferences.value("voice_input", Json::object());
    voice_enabled = voice_preferences.value("enabled", true);
    voice_language = voice_preferences.value("language", std::string("zh-cn"));
    voice_hotkey_ralt = voice_preferences.value("hotkey_ralt", true);
    voice_hotkey_ctrl_win = voice_preferences.value("hotkey_ctrl_win", false);
    voice_hotkey_rctrl_ralt = voice_preferences.value("hotkey_rctrl_ralt", false);
    voice_hotkey_hold_space_lock =
        voice_preferences.value("hotkey_hold_space_lock", true);
    voice_hotkey_ctrl_f9 = voice_preferences.value("hotkey_ctrl_f9", true);
    const auto keybindings = preferences.value("keybindings", Json::object());
    mode_ctrl_space_enabled = keybindings.value("switch_language_ctrl_space", true);
    mode_shift_enabled = keybindings.value("switch_language_shift", true);
    mode_ctrl_enabled = keybindings.value("switch_language_ctrl", false);
    mode_ctrl_alt_space_enabled =
        keybindings.value("switch_language_ctrl_alt_space", true);
    character_set_shortcut_enabled =
        keybindings.value("toggle_character_set_ctrl_shift_f", true);
    traditional_output = traditional_output_override.value_or(
        preferences.value("traditional_chinese_output", false));
    cloud_candidates = cloud_candidates_override.value_or(
        preferences.value("cloud_candidates", true));
    candidate_translations = candidate_translations_override.value_or(
        preferences.value("candidate_translations", true));
    candidate_english_gloss = preferences.value("candidate_english_gloss", false);
    translation_target_language = translation_target_language_override.value_or(
        preferences.value("translation_target_language", "en"));
    if (english_override || emoji_override || kaomoji_override) {
      Json mixed = Json::object();
      if (english_override) mixed["english"] = *english_override;
      if (emoji_override) mixed["emoji"] = *emoji_override;
      if (kaomoji_override) mixed["kaomoji"] = *kaomoji_override;
      patch_preference_object(options["preferences"], "mixed_input", mixed);
    }
    if (!local_mode_overrides.empty())
      patch_preference_object(options["preferences"], "local_modes",
                              local_mode_overrides);
    if (private_input)
      options["preferences"]["learning"] = false;
    options.erase("candidate_skin_catalog");
    // The built-in sound packs of this installation, unless the runtime options name others; resolved once, from the executable, as the resource bundle is.
    if (!options.contains("sound_packs")) {
      static const std::string sound_packs = [] {
        std::error_code error;
        const auto executable = std::filesystem::read_symlink("/proc/self/exe", error);
        return error ? std::string{} : msime_linux::installed_sound_pack_directory(executable);
      }();
      if (!sound_packs.empty())
        options["sound_packs"] = sound_packs;
    }
    // This host draws view.phrase_prefix ahead of the reading, so a phrase assembled out of
    // several selections stays in the composition instead of reaching the document one piece at a
    // time. Requesting it and drawing it are one decision; see PhrasePreedit.h.
    options["phrase_preedit"] = true;
    auto encoded = options.dump();
    auto bindings =
        msime::linux_host::NavigationBindings::read(options.at("preferences"));
    auto edge_binding = msime::linux_host::WordCharacterBinding::read(
        options.at("preferences"));
    view = response(msime_client_create(
        reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
    session = view.at("session").get<uint64_t>();
    translation_reset_pending = false;
    key_router.set_lease(
        {client_token, focus_epoch,
         msime::linux_host::KeyRouterAdapter::lease_token(client_token,
                                                           session)});
    view = response(msime_client_set_character_width(session, fullwidth));
    session_fullwidth = fullwidth;
    // CN/EN passthrough defaults are independent of the English candidate mode.
    english_mode = dedicated_english_override.value_or(false);
    view = response(msime_client_set_english_mode(session, english_mode));
    if (active_scheme == "quanpin" && nine_key_override)
      view = response(msime_client_set_nine_key_mode(session, *nine_key_override));
    chinese_punctuation = punctuation_override.value_or(
        options.at("preferences").value("chinese_punctuation", true));
    session_chinese_punctuation = chinese_punctuation;
    smart_punctuation = smart_punctuation_override.value_or(preferences.value("smart_punctuation", true));
    smart_punctuation_repeat = smart_repeat_override.value_or(preferences.value("smart_punctuation_repeat", true));
    smart_punctuation_space_convert =
        preferences.value("smart_punctuation_space_convert", false);
    smart_punctuation_direct_digit = preferences.value("smart_punctuation_direct_digit", true);
    smart_punctuation_direct_letter = preferences.value("smart_punctuation_direct_letter", true);
    paired_punctuation = options.at("preferences").value("paired_punctuation", true);
    punctuation_lock = preferences.value("punctuation_lock", "follow");
    if (punctuation_lock == "chinese")
      chinese_punctuation = true;
    else if (punctuation_lock == "english")
      chinese_punctuation = false;
    const auto colors = candidate_theme(options.at("preferences")).colors;
    candidate_text_color = colors.text;
    candidate_number_color = colors.number;
    candidate_translation_color = colors.translation;
    candidate_accent_color = colors.accent;
    candidate_background_color = colors.background;
    candidate_selected_color = colors.selected;
    candidate_selected_text_color = colors.selected_text;
    candidate_selected_number_color = colors.selected_number;
    candidate_orientation = ::candidate_orientation(options.at("preferences"));
    preedit_style = ::preedit_style(options.at("preferences"));
    candidate_preedit_style =
        preferences.value("candidate_preedit_style", "pinyin");
    show_candidate_page_number = preferences.value("show_candidate_page_number", true);
    if (candidate_preedit_style != "empty")
      candidate_preedit_style = "pinyin";
    learning = options.at("preferences").value("learning", true);
    const auto frequency_preferences = options.at("preferences").value("frequency", Json::object());
    frequency_trigger_count = static_cast<uint8_t>(std::clamp(
        frequency_preferences.value("trigger_count", 1), 1, 10));
    frequency_linear_step = static_cast<uint8_t>(std::clamp(
        frequency_preferences.value("linear_step", 1), 1, 10));
    shuangpin_preedit_uses_raw = options.at("preferences").value(
        "shuangpin_preedit_uses_raw", true);
    wubi_code_hint = options.at("preferences").value("wubi_code_hint", true);
    view = response(
        msime_client_set_chinese_punctuation(session, chinese_punctuation));
    navigation = bindings;
    word_character = edge_binding;
    if (word_character_override)
      word_character.enabled = *word_character_override;
    Json initial_snapshot{{"preferences", configured.at("preferences")}};
    apply_session_overrides(initial_snapshot);
    applied_preferences_snapshot =
        std::move(initial_snapshot.at("preferences"));
  }
  void refresh_host_preferences(const Json &preferences) {
    apply_candidate_panel_font(preferences);
    const auto diagnostic = preferences.value("diagnostic_log", Json::object());
    msime_linux_diagnostic_configure(
        configured.value("preferences_directory", std::string{}),
        diagnostic.is_object() && diagnostic.value("server", false));
    const bool previous_paired_punctuation = paired_punctuation;
    const bool previous_chinese_punctuation = chinese_punctuation;
    const bool previous_fullwidth = fullwidth;
    configure_clipboard(configured_clipboard_path(configured),
                        preferences.value("clipboard_history", false));
    mode_scope_global = preferences.value("ime_mode_scope", "app") == "global";
    if (mode_scope_global) {
      if (!global_input_enabled)
        global_input_enabled =
            preferences.value("default_ime_mode", "chinese") != "english";
      if (!session)
        input_enabled = *global_input_enabled;
    }
    navigation = msime::linux_host::NavigationBindings::read(preferences);
    word_character = msime::linux_host::WordCharacterBinding::read(preferences);
    if (word_character_override)
      word_character.enabled = *word_character_override;
    number_row_selection = number_row_override.value_or(
        preferences.value("number_row_selection", true));
    smart_punctuation = smart_punctuation_override.value_or(
        preferences.value("smart_punctuation", true));
    smart_punctuation_repeat = smart_repeat_override.value_or(
        preferences.value("smart_punctuation_repeat", true));
    smart_punctuation_space_convert =
        preferences.value("smart_punctuation_space_convert", false);
    smart_punctuation_direct_digit = preferences.value("smart_punctuation_direct_digit", true);
    smart_punctuation_direct_letter = preferences.value("smart_punctuation_direct_letter", true);
    paired_punctuation = paired_punctuation_override.value_or(
        preferences.value("paired_punctuation", true));
    punctuation_lock = punctuation_lock_override.value_or(
        preferences.value("punctuation_lock", "follow"));
    chinese_punctuation = punctuation_override.value_or(
        preferences.value("chinese_punctuation", true));
    fullwidth = preferences.value("character_width", "halfwidth") == "fullwidth";
    if (punctuation_lock == "chinese")
      chinese_punctuation = true;
    else if (punctuation_lock == "english")
      chinese_punctuation = false;
    if (paired_punctuation != previous_paired_punctuation ||
        chinese_punctuation != previous_chinese_punctuation ||
        fullwidth != previous_fullwidth)
      paired_tracker.clear();
    if (!smart_punctuation || !smart_punctuation_repeat) {
      last_smart_punctuation = 0;
      last_smart_punctuation_time = 0;
      smart_punctuation_rejected = 0;
    }
    if (!smart_punctuation || !smart_punctuation_space_convert ||
        !chinese_punctuation) {
      space_convert_mark.clear();
      space_convert_preceding.clear();
    }
    traditional_output = traditional_output_override.value_or(
        preferences.value("traditional_chinese_output", false));
    const bool next_cloud_candidates = cloud_candidates_override.value_or(
        preferences.value("cloud_candidates", true));
    if (next_cloud_candidates != cloud_candidates)
      invalidate_providers();
    cloud_candidates = next_cloud_candidates;
    const bool next_candidate_translations = candidate_translations_override.value_or(
        preferences.value("candidate_translations", true));
    const bool next_candidate_english_gloss =
        preferences.value("candidate_english_gloss", false);
    if (next_candidate_translations != candidate_translations ||
        next_candidate_english_gloss != candidate_english_gloss) {
      translation_reset_pending = true;
      invalidate_providers();
    }
    candidate_translations = next_candidate_translations;
    candidate_english_gloss = next_candidate_english_gloss;
    const auto next_translation_target_language = translation_target_language_override.value_or(
        preferences.value("translation_target_language", "en"));
    if (next_translation_target_language != translation_target_language) {
      translation_reset_pending = true;
      invalidate_providers();
    }
    translation_target_language = next_translation_target_language;
    auto display_preferences = preferences;
    if (layout_override)
      display_preferences["candidate_layout"] = *layout_override;
    if (theme_override)
      display_preferences["candidate_theme"] = *theme_override;
    if (theme_choice_override)
      msime::linux_host::apply_theme_choice(display_preferences, *theme_choice_override);
    const auto colors = candidate_theme(display_preferences).colors;
    candidate_text_color = colors.text;
    candidate_number_color = colors.number;
    candidate_translation_color = colors.translation;
    candidate_accent_color = colors.accent;
    candidate_background_color = colors.background;
    candidate_selected_color = colors.selected;
    candidate_selected_text_color = colors.selected_text;
    candidate_selected_number_color = colors.selected_number;
    candidate_orientation = ::candidate_orientation(display_preferences);
    preedit_style = preedit_override.value_or(::preedit_style(display_preferences));
    candidate_preedit_style = preferences.value("candidate_preedit_style", "pinyin");
    show_candidate_page_number = preferences.value("show_candidate_page_number", true);
    if (candidate_preedit_style != "empty")
      candidate_preedit_style = "pinyin";
    const auto active_scheme = scheme_override.value_or(
        preferences.value("scheme", "quanpin"));
    if (active_scheme == "quanpin" || active_scheme == "shuangpin")
      show_helpcode_in_candidate_window = preferences.value(
          active_scheme + "_helpcode", Json::object())
          .value("show_in_candidate_window",
                 msime::linux_host::default_show_helpcode(active_scheme));
    else
      show_helpcode_in_candidate_window = true;
    learning = learning_override.value_or(preferences.value("learning", true));
    const auto frequency_preferences = preferences.value("frequency", Json::object());
    frequency_trigger_count = frequency_trigger_count_override.value_or(static_cast<uint8_t>(std::clamp(
        frequency_preferences.value("trigger_count", 1), 1, 10)));
    frequency_linear_step = frequency_linear_step_override.value_or(static_cast<uint8_t>(std::clamp(
        frequency_preferences.value("linear_step", 1), 1, 10)));
    shuangpin_preedit_uses_raw = shuangpin_preedit_override.value_or(
        preferences.value("shuangpin_preedit_uses_raw", true));
    wubi_code_hint = wubi_code_hint_override.value_or(
        preferences.value("wubi_code_hint", true));
    const auto voice = preferences.value("voice_input", Json::object());
    // The voice overlay's mode from voice_theme by its own rule, its colours from the theme the candidate window resolves (as the floating toolbar takes them), so the bar matches the panel's theme; a fixed-appearance theme overrides the mode.
    const auto voice_theme = theme_in_mode(
        display_preferences, !msime_voice_overlay_light_theme(preferences.value("voice_theme", "follow"),
                                                              preferences.value("theme", "dark"), system_dark));
    wave_overlay.light_theme = !voice_theme.dark;
    wave_overlay.palette = msime::linux_host::floating_surface_colors(voice_theme);
    voice_enabled = voice.value("enabled", true);
    voice_language = voice.value("language", std::string("zh-cn"));
    voice_hotkey_ralt = voice.value("hotkey_ralt", true);
    voice_hotkey_ctrl_win = voice.value("hotkey_ctrl_win", false);
    voice_hotkey_rctrl_ralt = voice.value("hotkey_rctrl_ralt", false);
    voice_hotkey_hold_space_lock = voice.value("hotkey_hold_space_lock", true);
    voice_hotkey_ctrl_f9 = voice.value("hotkey_ctrl_f9", true);
    const auto keybindings = preferences.value("keybindings", Json::object());
    mode_ctrl_space_enabled = keybindings.value("switch_language_ctrl_space", true);
    mode_shift_enabled = keybindings.value("switch_language_shift", true);
    mode_ctrl_enabled = keybindings.value("switch_language_ctrl", false);
    mode_ctrl_alt_space_enabled =
        keybindings.value("switch_language_ctrl_alt_space", true);
    character_set_shortcut_enabled =
        keybindings.value("toggle_character_set_ctrl_shift_f", true);
  }
  bool refresh_provider_sockets(IBusEngine *engine) {
    const auto online = provider_socket_fallback(
        configured, "online_provider_socket", "MSIME_ONLINE_PROVIDER_SOCKET", "online.sock");
    const auto translation = [&] {
      auto socket = provider_socket_fallback(
          configured, "translation_provider_socket", "MSIME_TRANSLATION_PROVIDER_SOCKET",
          "translation.sock");
      return socket.empty() ? online : socket;
    }();
    const auto voice = provider_socket_fallback(
        configured, "voice_provider_socket", "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
    const bool voice_changed = voice != voice_provider_socket;
    const bool online_changed = online != online_provider_socket ||
                                translation != translation_provider_socket;
    if (translation != translation_provider_socket)
      translation_reset_pending = true;
    // Cancel against the old endpoint before replacing it, so the previous
    // provider does not keep recording after an environment/config change.
    if (voice_changed && voice_active)
      voice_cancel(engine);
    if (online_changed)
      invalidate_providers();
    online_provider_socket = online;
    translation_provider_socket = translation;
    voice_provider_socket = voice;
    return voice_changed || online_changed;
  }
  void apply_session_overrides(Json &options) const {
    auto &preferences = options["preferences"];
    if (cloud_candidates_override)
      preferences["cloud_candidates"] = *cloud_candidates_override;
    if (candidate_translations_override)
      preferences["candidate_translations"] = *candidate_translations_override;
    if (translation_target_language_override)
      preferences["translation_target_language"] = *translation_target_language_override;
    if (punctuation_override)
      preferences["chinese_punctuation"] = *punctuation_override;
    if (paired_punctuation_override)
      preferences["paired_punctuation"] = *paired_punctuation_override;
    if (punctuation_lock_override)
      preferences["punctuation_lock"] = *punctuation_lock_override;
    if (scheme_override)
      preferences["scheme"] = *scheme_override;
    if (shuangpin_profile_override)
      preferences["shuangpin_profile"] = *shuangpin_profile_override;
    if (candidate_page_size_override)
      preferences["candidate_page_size"] = *candidate_page_size_override;
    if (layout_override)
      preferences["candidate_layout"] = *layout_override;
    if (preedit_override)
      preferences["tsf_preedit_style"] = *preedit_override;
    if (theme_override)
      preferences["candidate_theme"] = *theme_override;
    if (theme_choice_override)
      msime::linux_host::apply_theme_choice(preferences, *theme_choice_override);
    if (!preferences.contains("quanpin"))
      preferences["quanpin"] = Json::object();
    auto &quanpin = preferences["quanpin"];
    if (autocorrect_transposition_override)
      quanpin["autocorrect_transposition"] = *autocorrect_transposition_override;
    if (autocorrect_neighbor_override)
      quanpin["autocorrect_neighbor"] = *autocorrect_neighbor_override;
    if (frequency_mode_override)
      preferences["frequency"]["mode"] = *frequency_mode_override;
    if (frequency_trigger_count_override)
      preferences["frequency"]["trigger_count"] = *frequency_trigger_count_override;
    if (frequency_linear_step_override)
      preferences["frequency"]["linear_step"] = *frequency_linear_step_override;
    if (learning_override)
      preferences["learning"] = *learning_override;
    if (shuangpin_preedit_override)
      preferences["shuangpin_preedit_uses_raw"] = *shuangpin_preedit_override;
    if (wubi_code_hint_override)
      preferences["wubi_code_hint"] = *wubi_code_hint_override;
    const auto active_scheme = preferences.value("scheme", "quanpin");
    if (active_scheme == "quanpin" || active_scheme == "shuangpin") {
      if (helpcode_override)
        preferences[active_scheme + "_helpcode"]["enabled"] = *helpcode_override;
      if (helpcode_schema_override)
        preferences[active_scheme + "_helpcode"]["schema"] = *helpcode_schema_override;
    }
    if (english_override)
      preferences["mixed_input"]["english"] = *english_override;
    if (emoji_override)
      preferences["mixed_input"]["emoji"] = *emoji_override;
    if (kaomoji_override)
      preferences["mixed_input"]["kaomoji"] = *kaomoji_override;
    auto &local_modes = preferences["local_modes"];
    for (const auto &[key, value] : local_mode_overrides.items())
      local_modes[key] = value;
    if (private_input)
      preferences["learning"] = false;
  }
};
// The scheme the Engine runs for this context: the menu's choice or the configured one, given way to the last Chinese scheme as host-api does when Cantonese or Zhuyin has no dictionary installed here (InputSchemes.h). The menus, the indicator and the key rules follow this one, so none of them claims a scheme the user is not typing in.
std::string effective_scheme(const State &s) {
  const auto &preferences = configured.at("preferences");
  return msime::linux_host::effective_input_scheme(
      s.scheme_override.value_or(preferences.value("scheme", std::string("quanpin"))),
      preferences.value("last_chinese_scheme", std::string("quanpin")), configured_dictionaries);
}
// 只转换基础中文方案：假名、谚文、越南文和藏文不是中文，粤拼和注音本来就写繁体字（`script_conversion_applies`）。
bool script_conversion_applies(const Json &context) {
  return context.is_object() &&
         msime::linux_host::scheme::ScriptConversionApplies(context.value("scheme", 255)) &&
         context.value("local_mode", "none") != "unicode";
}
std::string traditional_display(const State &s, const Json &context,
                                std::string text) {
  if (s.traditional_output && script_conversion_applies(context))
    text = msime_linux_simplified_to_traditional(text);
  return text;
}
constexpr size_t kClipboardStoreBytes = 1024 * 1024;
constexpr size_t kMaxClipboardItems = 50;

std::optional<Json> read_clipboard_store(const std::filesystem::path &path) {
  const auto payload = msime::linux_host::read_clipboard_file(path, kClipboardStoreBytes);
  if (!payload) return std::nullopt;
  try {
    return Json::parse(*payload);
  } catch (...) {
    return std::nullopt;
  }
}

std::vector<std::string> clipboard_items(const std::string &path) {
  std::vector<std::string> items;
  items.reserve(kMaxClipboardItems);
  if (path.empty() || path.size() > 4096) return items;
  const auto value = read_clipboard_store(std::filesystem::path(path));
  if (!value || !value->is_array()) return items;
  for (const auto &entry : *value) {
    if (items.size() == kMaxClipboardItems) break;
    if (!entry.is_string()) continue;
    auto text = entry.get<std::string>();
    if (text.size() > 12000) continue;
    if (!text.empty()) items.push_back(std::move(text));
  }
  return items;
}
bool clipboard_delete(const std::string &path, const std::optional<std::string> &text) {
  if (path.empty() || path.size() > 4096)
    return false;
  const int lock = msime::linux_host::open_clipboard_lock(std::filesystem::path(path));
  if (lock < 0 || flock(lock, LOCK_EX) != 0) {
    if (lock >= 0)
      close(lock);
    return false;
  }
  bool removed = false;
  try {
    if (!text) {
      removed = msime::linux_host::remove_clipboard_file(std::filesystem::path(path));
    } else {
      auto value = read_clipboard_store(std::filesystem::path(path));
      if (value && value->is_array()) {
        const auto entry = std::find(value->begin(), value->end(), Json(*text));
        if (entry == value->end()) {
          flock(lock, LOCK_UN);
          close(lock);
          return true;
        }
        value->erase(entry);
        removed = msime::linux_host::write_clipboard_file_atomically(
            std::filesystem::path(path), value->dump());
      }
    }
  } catch (...) {
  }
  flock(lock, LOCK_UN);
  close(lock);
  return removed;
}
State &state(IBusEngine *engine);
// Shared by every engine in this process: the statistics store is one per preferences directory, not one per input context.
msime::linux_host::TypingStatisticsSwitch typing_statistics_switch{msime_client_typing_statistics_enabled};
struct TypingStatisticsTask {
  std::string directory;
  std::string text;
  std::string source;
  std::string day;
  int hour = 0;
};

void record_typing_statistics(IBusEngine *engine, std::string text,
                              msime::linux_host::TypingSource source) {
  // With statistics off nothing below runs: no date, no request, no worker, no store lock.
  if (!typing_statistics_switch.enabled())
    return;
  // Private fields (passwords, no-spellcheck) never reach the statistics store, matching the Fcitx5 host.
  if (text.empty() || state(engine).private_input)
    return;
  const auto directory = configured.value("preferences_directory", std::string{});
  if (directory.empty() || directory.front() != '/')
    return;
  GDateTime *now = g_date_time_new_now_local();
  if (!now)
    return;
  gchar *formatted_day = g_date_time_format(now, "%Y-%m-%d");
  // Read the hour from the same instant as the day, before it is released: two
  // calls either side of midnight would file the commit under one day and the
  // other day's hour.
  const int hour = g_date_time_get_hour(now);
  g_date_time_unref(now);
  if (!formatted_day)
    return;
  TypingStatisticsTask request{
      directory, std::move(text),
      std::string(msime::linux_host::typing_source_id(source)), formatted_day,
      hour};
  g_free(formatted_day);
  auto task = g_task_new(G_OBJECT(engine), nullptr, nullptr, nullptr);
  g_task_set_task_data(task, new TypingStatisticsTask(std::move(request)),
                       [](gpointer value) {
                         delete static_cast<TypingStatisticsTask *>(value);
                       });
  g_task_run_in_thread(task, [](GTask *task, gpointer, gpointer data,
                                GCancellable *) {
    const auto &request = *static_cast<TypingStatisticsTask *>(data);
    try {
      const auto encoded = Json{
          {"directory", request.directory},
          {"action", Json{{"operation", "record"},
                            {"text", request.text},
                            {"source", request.source},
                            {"day", request.day},
                            {"hour", request.hour}}}}
                                .dump();
      auto *raw = msime_client_typing_statistics(
          reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size());
      if (raw)
        msime_client_string_free(raw);
    } catch (...) {
      // Statistics are best effort and must never affect text commitment.
    }
    g_task_return_boolean(task, TRUE);
  });
  g_object_unref(task);
}

// Sends one batch to the store's record_keys operation on the calling thread.
void write_key_presses(const msime::linux_host::KeyPressBatch &request) {
  try {
    const auto encoded = Json{
        {"directory", request.directory},
        {"action", Json{{"operation", "record_keys"},
                          {"day", request.day},
                          {"keys", request.keys}}}}
                              .dump();
    auto *raw = msime_client_typing_statistics(
        reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size());
    if (raw)
      msime_client_string_free(raw);
  } catch (...) {
    // Statistics are best effort and must never affect typing.
  }
}
// Key press writes still running on worker threads, which msime_ibus_shutdown_key_presses waits for.
msime::linux_host::PendingWrites key_press_writes;
// Set once the IBus main loop has quit: the process is about to exit, and a write handed to a worker thread now would die with it.
bool key_presses_shutting_down = false;
// Every live engine's state, so the last batches can be written at shutdown whether or not the factory destroys its engines first.
std::set<State *> key_press_states;
// Writes a batch of key press counts on a worker thread, or on this one once the process is shutting down. The task has no source object because destroy() flushes too, while the engine is being disposed.
void flush_key_presses(std::optional<msime::linux_host::KeyPressBatch> batch) {
  // Presses counted before statistics were turned off are dropped rather than sent; the store would not write them either.
  if (!batch || !typing_statistics_switch.enabled())
    return;
  if (key_presses_shutting_down) {
    write_key_presses(*batch);
    return;
  }
  auto task = g_task_new(nullptr, nullptr, nullptr, nullptr);
  g_task_set_task_data(task, new msime::linux_host::KeyPressBatch(std::move(*batch)),
                       [](gpointer value) {
                         delete static_cast<msime::linux_host::KeyPressBatch *>(value);
                       });
  key_press_writes.begin();
  g_task_run_in_thread(task, [](GTask *task, gpointer, gpointer data,
                                GCancellable *) {
    write_key_presses(*static_cast<msime::linux_host::KeyPressBatch *>(data));
    key_press_writes.end();
    g_task_return_boolean(task, TRUE);
  });
  g_object_unref(task);
}

// Every key event the engine receives passes through here before the Engine sees it, so the heatmap counts keys the IME consumes for a composition as well as keys it hands back to the application. Only key downs count, once per physical press; keycode 0 is a synthetic event with no physical key behind it. IBus passes no event time, so a repeat that arrives as a release and press pair is told apart by when the events reach the engine.
void count_key_press(IBusEngine *engine, guint keycode, guint flags) {
  auto &s = state(engine);
  if (flags & IBUS_RELEASE_MASK) {
    s.key_presses.up(keycode, g_get_monotonic_time());
    return;
  }
  // With statistics off nothing is buffered; password, PIN, number and private fields are never counted, the same contexts commits are not recorded in.
  if (keycode == 0 || !typing_statistics_switch.enabled() || !s.focused || s.blocked ||
      s.private_input)
    return;
  const auto now = g_get_monotonic_time();
  const auto id = s.key_presses.down(
      keycode, now, msime::linux_host::KeyPressCounter::kArrivalRepeatGapMicroseconds);
  if (id.empty())
    return;
  const auto directory = configured.value("preferences_directory", std::string{});
  if (directory.empty() || directory.front() != '/')
    return;
  const auto day = msime::linux_host::local_day(std::time(nullptr));
  if (day.empty())
    return;
  flush_key_presses(s.key_presses.add(id, directory, day, now));
}

msime::linux_host::TypingSource typing_source(const State &s) {
  const auto preferences = configured.value("preferences", Json::object());
  const auto profile = s.shuangpin_profile_override.value_or(
      preferences.value("shuangpin_profile", "xiaohe"));
  return msime::linux_host::resolve_typing_source(
      s.view.value("scheme", -1), s.view.value("nine_key", false),
      s.english_mode, s.view.value("local_mode", "none"), profile);
}

// Record the exact text sent to IBus after each route's output conversion. `typing_statistics` is false for text the Engine generated rather than the user typed out (the expression, command and mention modes), which the statistics leave out.
void commit_text(
    IBusEngine *engine, const std::string &text,
    std::optional<msime::linux_host::TypingSource> source_override = std::nullopt,
    bool typing_statistics = true) {
  if (text.empty())
    return;
  auto &s = state(engine);
  ibus_engine_commit_text(engine, ibus_text_new_from_string(text.c_str()));
  // Only derive the source from the Engine view when the caller did not name one: English mode and sessionless voice commits have no view, and value_or would evaluate typing_source eagerly and throw on it.
  if (typing_statistics)
    record_typing_statistics(engine, text,
                             source_override ? *source_override : typing_source(s));
  s.remember_commit(text);
}
// Background music may play while this input method is focused in a field that is not a secure one (a password, PIN or no-spellcheck field), and not while a recording would pick it up. The player serves the whole process, so this only tells it about a change (KeySound.h); State::close tells it the music is over before the session goes away.
void sync_music(IBusEngine *engine) {
  auto &s = state(engine);
  s.music.sync(s.session, s.focused && !s.blocked && !s.private_input && !s.voice_active,
               msime_client_music_set_active);
}
void publish_mode(IBusEngine *engine, bool registration = false);
void sync_global_input_mode(IBusEngine *engine);
void show_input_mode_hint(IBusEngine *engine);
void voice_cancel(IBusEngine *engine);
bool launch_desktop_panel(const char *panel) {
  const auto *command = g_getenv("MSIME_CLIENT_SETTINGS_COMMAND");
  if (!command || !*command)
    command = MSIME_EDITION_SETTINGS_PROGRAM;
  const std::string requested = panel ? panel : "";
  // About, help, feedback and the local dictionary are settings sections rather than desktop surfaces, so each travels as "settings:<category>"; the bare section name is not a route head and would be rejected by the shared parser.
  const bool settings_page = requested == "about" || requested == "help" ||
                             requested == "feedback" || requested == "dictionary";
  std::string route_argument =
      "--route=" + (settings_page ? std::string("settings:") + requested : requested);
  gchar *argv[] = {const_cast<gchar *>(command), route_argument.data(), nullptr};
  GError *error = nullptr;
  const auto started = g_spawn_async(
      nullptr, argv, nullptr, G_SPAWN_SEARCH_PATH, nullptr, nullptr,
      nullptr, &error);
  if (error)
    g_error_free(error);
  return started != FALSE;
}
// Set by the maintenance stop shortcut so main() can tell the launcher's supervisor not to restart this process.
bool maintenance_stop_requested = false;
bool restart_ibus_service() {
  gchar *argv[] = {const_cast<gchar *>("ibus"),
                   const_cast<gchar *>("restart"), nullptr};
  GError *error = nullptr;
  const auto started = g_spawn_async(nullptr, argv, nullptr,
                                     G_SPAWN_SEARCH_PATH, nullptr, nullptr,
                                     nullptr, &error);
  if (error)
    g_error_free(error);
  return started != FALSE;
}

struct DesktopPanelAction {
  const char *property;
  const char *panel;
  const char *label;
  // Drawn at the top level of the design menu (主题 / 词库… / 设置… / 关于) instead of inside 桌面工具; the key is the same either way, so activation does not depend on where the entry sits.
  bool design_menu;
};
constexpr DesktopPanelAction desktop_panel_actions[] = {
    {"DesktopTools/Handwriting", "handwriting", "手写识别板", false},
    {"DesktopTools/Keyboard", "keyboard", "屏幕键盘", false},
    {"DesktopTools/Emoji", "emoji", "表情与符号", false},
    {"DesktopTools/Clipboard", "clipboard", "本地剪贴板", false},
    {"DesktopTools/Voice", "voice", "语音面板", false},
    {"DesktopTools/CloudDictionary", "cloud-dictionary", "云词库", false},
    {"DesktopTools/CloudClipboard", "cloud-clipboard", "云剪贴板", false},
    {"DesktopTools/Dictionary", "dictionary", "词库…", true},
    {"DesktopTools/Settings", "settings", "设置…", true},
    {"DesktopTools/About", "about", "关于" MSIME_EDITION_DISPLAY_NAME, true},
    {"DesktopTools/Help", "help", "帮助", false},
    {"DesktopTools/Feedback", "feedback", "反馈", false},
};

// A menu separator. ibus-ui-gtk3 draws it as a rule and ends the radio group before it; keys must stay unique because panels find properties by key.
IBusProperty *menu_separator(const char *key) {
  return ibus_property_new(key, PROP_TYPE_SEPARATOR, ibus_text_new_from_static_string(""), "",
                           ibus_text_new_from_static_string(""), FALSE, TRUE, PROP_STATE_UNCHECKED, nullptr);
}

// A sub-menu that only holds other properties. It is registered once and never updated as a container: every leaf inside keeps its own key and is updated by that key, which panels resolve through the nesting.
IBusProperty *menu_group(const char *key, const char *label, const char *tooltip,
                         std::initializer_list<IBusProperty *> items) {
  auto list = ibus_prop_list_new();
  for (auto *item : items) ibus_prop_list_append(list, item);
  return ibus_property_new(key, PROP_TYPE_MENU, ibus_text_new_from_static_string(label), "",
                           ibus_text_new_from_static_string(tooltip), TRUE, TRUE, PROP_STATE_UNCHECKED, list);
}

// Update a menu whose children are fixed, children first: IBus panels apply an update to the property with the same key and never to its sub-properties, so a radio chosen from outside the menu would otherwise stay unmarked until the next registration.
void update_menu_property(IBusEngine *engine, IBusProperty *menu) {
  if (auto *items = ibus_property_get_sub_props(menu))
    for (guint index = 0; auto *item = ibus_prop_list_get(items, index); ++index)
      if (ibus_property_get_prop_type(item) != PROP_TYPE_SEPARATOR) ibus_engine_update_property(engine, item);
  ibus_engine_update_property(engine, menu);
}

IBusProperty *desktop_panel_property(IBusEngine *engine, const DesktopPanelAction &action) {
  const auto &s = state(engine);
  return ibus_property_new(
      action.property, PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string(action.label), "",
      ibus_text_new_from_static_string(action.label),
      s.focused && !s.blocked &&
          (std::string(action.property) != "DesktopTools/Voice" || s.voice_enabled),
      TRUE, PROP_STATE_UNCHECKED, nullptr);
}

IBusProperty *desktop_tools_property(IBusEngine *engine) {
  const auto &s = state(engine);
  auto items = ibus_prop_list_new();
  const auto directory = configured.value("preferences_directory", std::string{});
  const bool can_retry = failed_menu_save &&
      failed_menu_save->directory == directory &&
      failed_menu_save->configuration == configuration_generation;
  ibus_prop_list_append(items, ibus_property_new(
      "DesktopTools/RetrySave", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string(menu_save_pending ? "正在保存设置…" : "设置未保存，点击重试"), "",
      ibus_text_new_from_static_string("重新读取最新设置并重试上次菜单修改"),
      can_retry && !menu_save_pending && s.focused && !s.blocked,
      menu_save_pending || can_retry, PROP_STATE_UNCHECKED, nullptr));
  const bool toolbar_enabled = configured.at("preferences")
      .value("floating_toolbar", Json::object()).value("enabled", true);
  ibus_prop_list_append(items, ibus_property_new(
      "DesktopTools/ToolbarEnabled", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("工具栏"), "",
      ibus_text_new_from_static_string("保存工具栏显示开关"),
      s.focused && !s.blocked && !menu_save_pending &&
          !directory.empty() && directory.front() == '/',
      TRUE, toolbar_enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr));
  for (const auto &action : desktop_panel_actions)
    if (!action.design_menu) ibus_prop_list_append(items, desktop_panel_property(engine, action));
  ibus_prop_list_append(items, ibus_property_new(
      "DesktopTools/VoiceEnabled", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("启用语音输入"), "",
      ibus_text_new_from_static_string("启用或停用语音快捷键和录音入口"),
      s.focused && !s.blocked && !menu_save_pending && !directory.empty() &&
          directory.front() == '/',
      TRUE, s.voice_enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr));
  return ibus_property_new(
      "DesktopTools", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("桌面工具"), "",
      ibus_text_new_from_static_string("打开水杉桌面面板"),
      s.focused && !s.blocked, TRUE, PROP_STATE_UNCHECKED, items);
}

IBusProperty *toolbar_property(IBusEngine *engine) {
  const auto &s = state(engine);
  const auto toolbar = configured.at("preferences").value(
      "floating_toolbar", Json::object());
  const bool available = toolbar.value("enabled", true) && s.focused && !s.blocked;
  auto items = ibus_prop_list_new();
  const auto append_toggle = [&](const char *name, const char *label,
                                 const char *hint, bool available, bool checked) {
    if (!available)
      return;
    ibus_prop_list_append(
        items, ibus_property_new(
                   name, PROP_TYPE_TOGGLE, ibus_text_new_from_static_string(label),
                   "", ibus_text_new_from_static_string(hint), TRUE, TRUE,
                   checked ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr));
  };
  const auto append_action = [&](const char *name, const char *label,
                                 const char *hint, bool available) {
    if (!available)
      return;
    ibus_prop_list_append(
        items, ibus_property_new(
                   name, PROP_TYPE_NORMAL, ibus_text_new_from_static_string(label),
                   "", ibus_text_new_from_static_string(hint), TRUE, TRUE,
                   PROP_STATE_UNCHECKED, nullptr));
  };
  append_toggle("Toolbar/InputMode", "中英文模式", "切换中文输入与直接输入",
                available, s.input_enabled);
  append_toggle("Toolbar/EnglishMode", "英文输入模式",
                "切换 Engine 的独立英文输入模式",
                available && toolbar.value("english_mode", true) &&
                    s.input_enabled && s.session,
                s.english_mode);
  append_toggle("Toolbar/Fullwidth", "全角字符", "切换 ASCII 全角或半角输出",
                available && toolbar.value("fullwidth", true), s.fullwidth);
  append_toggle("Toolbar/Punctuation", "中文标点", "切换中文或英文标点",
                available && toolbar.value("punctuation", true), s.chinese_punctuation);
  append_toggle("Toolbar/CharacterSet", "繁体输出", "切换简体或繁体输出",
                available && toolbar.value("character_set", true), s.traditional_output);
  append_action("Toolbar/Emoji", "表情与符号", "打开 Emoji、颜文字和符号面板",
                available && toolbar.value("emoji", true));
  append_action("Toolbar/ScreenKeyboard", "屏幕键盘", "打开屏幕键盘面板",
                available && toolbar.value("screen_keyboard", false));
  append_action("Toolbar/Settings", "设置", "打开" MSIME_EDITION_DISPLAY_NAME "设置",
                available && toolbar.value("settings", true));
  return ibus_property_new(
      "LinuxToolbar", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("工具栏"), "",
      ibus_text_new_from_static_string("Linux 原生输入法工具栏"), available,
      toolbar.value("enabled", true),
      PROP_STATE_UNCHECKED, items);
}

struct ClipboardTask {
  std::string path;
  uint64_t generation;
};
void clipboard_schedule(IBusEngine *engine);
void watch_clipboard_history(IBusEngine *engine) {
  auto &s = state(engine);
  const auto previous_path = s.clipboard_history_path;
  s.configure_clipboard(configured_clipboard_path(configured),
                        s.clipboard_enabled);
  if (previous_path != s.clipboard_history_path && s.focused && !s.blocked)
    publish_mode(engine);
  if (!s.clipboard_enabled || !s.focused || s.blocked || !s.input_enabled ||
      s.clipboard_history_path.empty()) {
    s.stop_clipboard_monitor();
    return;
  }
  auto target = g_file_new_for_path(s.clipboard_history_path.c_str());
  if (s.clipboard_monitor && !g_file_monitor_is_cancelled(s.clipboard_monitor) &&
      s.clipboard_watch_file && g_file_equal(target, s.clipboard_watch_file)) {
    g_object_unref(target);
    return;
  }
  s.stop_clipboard_monitor();
  auto parent = g_file_get_parent(target);
  if (!parent) {
    g_object_unref(target);
    return;
  }
  // Watch the directory so atomic replace and delete/recreate keep working.
  s.clipboard_monitor = g_file_monitor_directory(
      parent, G_FILE_MONITOR_WATCH_MOVES, nullptr, nullptr);
  g_object_unref(parent);
  if (!s.clipboard_monitor) {
    g_object_unref(target);
    return;
  }
  s.clipboard_watch_file = target;
  g_file_monitor_set_rate_limit(s.clipboard_monitor, 100);
  g_signal_connect(s.clipboard_monitor, "changed",
      G_CALLBACK(+[](GFileMonitor *, GFile *file, GFile *other,
                     GFileMonitorEvent, gpointer data) {
        auto engine = IBUS_ENGINE(data);
        auto &s = state(engine);
        if (!s.clipboard_enabled || !s.clipboard_watch_file || !s.focused || s.blocked || !s.input_enabled)
          return;
        if ((!file || !g_file_equal(file, s.clipboard_watch_file)) &&
            (!other || !g_file_equal(other, s.clipboard_watch_file)))
          return;
        ++s.clipboard_generation;
        s.clipboard_loaded = false;
        s.clipboard_items_cache.clear();
        publish_mode(engine);
      }), engine);
  // A monitor may have been unavailable when the directory did not exist.
  // Reload after attaching to close that gap.
  ++s.clipboard_generation;
  s.clipboard_loaded = false;
  clipboard_schedule(engine);
}
void clipboard_complete(GObject *source, GAsyncResult *result, gpointer);
void clipboard_schedule(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.clipboard_enabled || s.clipboard_history_path.empty() || s.clipboard_loading || !s.focused ||
      s.blocked || !s.input_enabled || s.clipboard_loaded)
    return;
  s.clipboard_loading = true;
  auto task = g_task_new(G_OBJECT(engine), nullptr, clipboard_complete, nullptr);
  g_task_set_task_data(task,
                       new ClipboardTask{s.clipboard_history_path,
                                         s.clipboard_generation},
                       [](gpointer value) { delete static_cast<ClipboardTask *>(value); });
  g_task_run_in_thread(task, [](GTask *task, gpointer, gpointer data, GCancellable *) {
    const auto &request = *static_cast<ClipboardTask *>(data);
    g_task_return_pointer(task, new std::vector<std::string>(clipboard_items(request.path)),
                          [](gpointer value) {
                            delete static_cast<std::vector<std::string> *>(value);
                          });
  });
  g_object_unref(task);
}
std::string fullwidth_text(const std::string &text) {
  std::string result;
  for (unsigned char c : text) {
    if (c >= 0x21 && c <= 0x7e) {
      const uint32_t code = c + 0xfee0;
      result.push_back(static_cast<char>(0xe0 | (code >> 12)));
      result.push_back(static_cast<char>(0x80 | ((code >> 6) & 0x3f)));
      result.push_back(static_cast<char>(0x80 | (code & 0x3f)));
    } else if (c == ' ') {
      result.append("\xe3\x80\x80");
    } else {
      // Preserve complete UTF-8 sequences byte-for-byte.
      result.push_back(static_cast<char>(c));
    }
  }
  return result;
}
IBusOrientation candidate_orientation(const Json &preferences) {
  return preferences.value("candidate_layout", "vertical") == "horizontal"
             ? IBUS_ORIENTATION_HORIZONTAL
             : IBUS_ORIENTATION_VERTICAL;
}
std::string preedit_style(const Json &preferences) {
  const auto style = preferences.value("tsf_preedit_style", "raw");
  return style == "pinyin" || style == "empty" ? style : "raw";
}
// The mapping itself is shared with the Fcitx5 host; this keeps the pointer
// shape its existing callers use. A string_view over a literal is NUL
// terminated, so data() is a valid C string here.
const char *smart_punctuation_pair(char value) {
  const auto mark = msime::linux_host::chinese_punctuation_mark(value);
  return mark.empty() ? nullptr : mark.data();
}
using msime::linux_host::normalize_punctuation_pair;
using msime::linux_host::paired_closing_from_text;
using msime::linux_host::PunctuationPairMode;
bool is_smart_punctuation_key(guint key) {
  return key <= 0x7f &&
         msime::linux_host::is_smart_punctuation_key(static_cast<char>(key));
}
bool is_ascii_alphanumeric(unsigned char value) {
  return (value >= '0' && value <= '9') ||
         (value >= 'A' && value <= 'Z') ||
         (value >= 'a' && value <= 'z');
}
std::size_t surrounding_byte_offset(const State &s, guint offset) {
  const auto *utf8 = s.surrounding_text.c_str();
  const auto *position = utf8;
  // IBus uses code points; QIBusInputContext forwards QString UTF-16 units.
  while (*position) {
    const guint units = s.surrounding_utf16 &&
                        g_utf8_get_char(position) > 0xffff ? 2 : 1;
    if (offset < units)
      break;
    offset -= units;
    position = g_utf8_next_char(position);
  }
  return static_cast<std::size_t>(position - utf8);
}
bool smart_punctuation_keeps_ascii_after(const State &s, unsigned char value) {
  if (value >= 0x80) return false;
  if (value >= '0' && value <= '9') return s.smart_punctuation_direct_digit;
  return is_ascii_alphanumeric(value) && s.smart_punctuation_direct_letter;
}
bool smart_punctuation_preceded_by_ascii_alphanumeric(const State &s) {
  // A highlighted candidate is the preceding text for punctuation finishing
  // an active composition. This mirrors the Windows TSF path, while IBus
  // surrounding text supplies the document character for a pure punctuation
  // input.
  if (s.view.is_object()) {
    const auto candidates = s.view.value("candidates", Json::array());
    if (candidates.is_array()) {
      for (const auto &candidate : candidates) {
        if (!candidate.is_object() || !candidate.value("highlighted", false))
          continue;
        const auto text = candidate.value("text", std::string{});
        if (text.empty())
          break;
        const auto last = static_cast<unsigned char>(text.back());
        return smart_punctuation_keeps_ascii_after(s, last);
      }
    }
  }
  const auto &surrounding = s.surrounding_text;
  // A commit replaces the selection, so inspect the character before its start.
  const auto cursor = surrounding_byte_offset(
      s, std::min(s.surrounding_cursor, s.surrounding_anchor));
  if (cursor == 0)
    return false;
  const auto value = static_cast<unsigned char>(surrounding[cursor - 1]);
  // A UTF-8 continuation byte means the preceding code point is non-ASCII.
  return smart_punctuation_keeps_ascii_after(s, value);
}
bool smart_punctuation_repeat_matches_document(const State &s) {
  if (s.surrounding_cursor != s.surrounding_anchor)
    return false;
  auto previous = msime::linux_host::ascii_mark_text(s.last_smart_punctuation,
                                                     s.fullwidth);
  const auto cursor = surrounding_byte_offset(s, s.surrounding_cursor);
  return cursor >= previous.size() &&
         s.surrounding_text.compare(cursor - previous.size(), previous.size(),
                                    previous) == 0;
}
// The `count` code points standing in front of the caret, oldest first. A
// shorter result means the document starts there; std::nullopt means the caret
// is not a plain insertion point or the host published no usable text, which is
// not the same thing as "nothing precedes it".
std::optional<std::vector<std::string>> surrounding_preceding_characters(
    const State &s, std::size_t count) {
  if (!s.surrounding_valid || s.surrounding_cursor != s.surrounding_anchor)
    return std::nullopt;
  return msime::linux_host::preceding_characters_from_byte_offset(
      s.surrounding_text, surrounding_byte_offset(s, s.surrounding_cursor), count);
}
std::optional<std::string> surrounding_following_character(const State &s) {
  if (!s.surrounding_valid || s.surrounding_cursor != s.surrounding_anchor)
    return std::nullopt;
  const auto offset = surrounding_byte_offset(s, s.surrounding_cursor);
  if (offset >= s.surrounding_text.size())
    return std::string{};
  const auto *start = s.surrounding_text.c_str() + offset;
  const auto codepoint = g_utf8_get_char_validated(start, -1);
  if (codepoint == static_cast<gunichar>(-1) ||
      codepoint == static_cast<gunichar>(-2))
    return std::nullopt;
  const auto *end = g_utf8_next_char(start);
  return std::string(start, static_cast<std::size_t>(end - start));
}
// A Space right after a Chinese mark the user did not want takes that mark back
// to ASCII. It re-reads the document first for the reason the source's own fix
// records: the same mark usually stands in more than one place, and moving the
// caret inside a window is not a focus change, so the mark in front of the
// caret has to still be the one that was committed and has to still follow the
// character it followed. A successful rewrite consumes the Space.
bool convert_smart_punctuation_space(IBusEngine *engine) {
  auto &s = state(engine);
  const auto mark = std::move(s.space_convert_mark);
  const auto expected_preceding = std::move(s.space_convert_preceding);
  s.space_convert_mark.clear();
  s.space_convert_preceding.clear();
  if (mark.empty() || !s.smart_punctuation ||
      !s.smart_punctuation_space_convert || !s.chinese_punctuation)
    return false;
  if (!s.view.value("editing_text", std::string{}).empty() ||
      !s.view.value("candidates", Json::array()).empty())
    return false;
  const auto replacement = msime::linux_host::space_conversion_ascii_text(mark);
  if (replacement.empty())
    return false;
  const auto characters = surrounding_preceding_characters(s, 2);
  if (!characters || !msime::linux_host::space_conversion_matches_document(
                         mark, expected_preceding, *characters))
    return false;
  ibus_engine_delete_surrounding_text(engine, -1, 1);
  // The mark was replaced in the editor, not appended.
  if (!s.ai_context.empty()) {
    std::size_t last = s.ai_context.size() - 1;
    while (last > 0 &&
           (static_cast<unsigned char>(s.ai_context[last]) & 0xc0) == 0x80)
      --last;
    s.ai_context.erase(last);
  }
  commit_text(engine, replacement);
  return true;
}
bool try_skip_paired_closing(IBusEngine *engine, guint key, guint flags) {
  auto &s = state(engine);
  if (msime::linux_host::paired_punctuation_excluded_client(s.focused_client))
    return false;
  if (key > 0x7f) return false;
  std::uint32_t paired_modifiers = 0;
  if (flags & IBUS_CONTROL_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Control);
  if (flags & IBUS_MOD1_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Alt);
  if (flags & IBUS_MOD4_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Super);
  if (flags & IBUS_SUPER_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Super);
  if (flags & IBUS_META_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Meta);
  if (flags & IBUS_HYPER_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Hyper);
  if (flags & IBUS_MOD5_MASK)
    paired_modifiers |= static_cast<std::uint32_t>(
        msime::linux_host::PairedPunctuationModifier::Mod5);
  const auto closing = msime::linux_host::paired_closing_for_key(
      static_cast<char>(key), s.fullwidth);
  if (!closing) return false;
  const auto following = surrounding_following_character(s);
  const bool modifiers_allowed =
      msime::linux_host::paired_closing_modifiers_allowed(paired_modifiers);
  if (!s.paired_tracker.consume(*closing, following.value_or(""),
                                following.has_value(), modifiers_allowed))
    return false;
  s.last_smart_punctuation = 0;
  s.last_smart_punctuation_time = 0;
  s.smart_punctuation_rejected = 0;
  // The paired mark is already in the document. Move over it without sending
  // the user's closing key through Engine, which would insert a duplicate.
  ibus_engine_forward_key_event(engine, IBUS_Right, 0, 0);
  return true;
}
constexpr gint64 kSmartPunctuationRepeatIntervalUs = 2 * G_USEC_PER_SEC;

struct OnlineTask {
  uint64_t session;
  uint64_t epoch;
  std::string query;
  std::string socket;
  std::string provider_query;
  uint8_t source;
  bool ai_cache_only = false;
};
struct TranslationTask {
  uint64_t session;
  uint64_t epoch;
  std::string query;
  std::string socket;
  std::string resources;
  std::string gloss_query;
  bool offline = false;
  Json local_translations = Json::array();
  // The offline glosses come from a non-English dictionary, which the user's own translator outranks: it is asked about every candidate and its answers replace the dictionary's (prefer_online_glosses).
  bool prefer_online = false;
};
// prefer_online_glosses over the JSON translation lists the host API exchanges.
Json prefer_online_translations(const Json &glosses, const Json &online) {
  std::vector<std::pair<std::string, std::string>> merged, answers;
  const auto read = [](const Json &values, auto &into) {
    if (!values.is_array()) return;
    into.reserve(into.size() + values.size());
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
bool apply(IBusEngine *engine, char *raw,
           PunctuationPairMode pair_mode = PunctuationPairMode::Unpaired,
           std::optional<std::string> space_convert_preceding = std::nullopt);
void render(IBusEngine *engine, const Json &view);
void exit_translation_candidates(IBusEngine *engine);
void render_translation_candidates(IBusEngine *engine);
void apply_live_preferences(IBusEngine *engine, Json snapshot);
void sync_translation_preferences(IBusEngine *engine) {
  auto &s = state(engine);
  apply_live_preferences(engine, Json{
      {"format_version", 1},
      {"preferences", s.applied_preferences_snapshot.is_object()
                          ? s.applied_preferences_snapshot
                          : configured.at("preferences")}});
}
void clear_candidate_translations(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.session || !s.view.is_object())
    return;
  constexpr uint8_t empty[] = {'[', ']'};
  auto result = response(msime_client_apply_translations(
      s.session, s.view.value("generation", uint64_t{0}), empty, sizeof(empty)));
  s.view = result.at("view");
  s.translation_reset_pending = false;
  render(engine, s.view);
}
void translation_complete(GObject *source, GAsyncResult *result, gpointer);
bool translation_request_is_stale(IBusEngine *engine, const std::string &encoded) {
  try {
    const auto request = Json::parse(encoded);
    const auto generation = request.at("generation").get<uint64_t>();
    auto &s = state(engine);
    if (!s.session)
      return false;
    const auto current = response(msime_client_translation_query(s.session));
    return current.is_object() &&
           current.value("generation", generation) != generation;
  } catch (...) {
    return false;
  }
}
void start_translation_task(IBusEngine *engine, TranslationTask request) {
  auto *task_data = new TranslationTask(std::move(request));
  state(engine).translation_loading = true;
  auto task = g_task_new(G_OBJECT(engine), nullptr, translation_complete, nullptr);
  g_task_set_task_data(task, task_data, [](gpointer value) { delete static_cast<TranslationTask *>(value); });
  g_task_run_in_thread(task, [](GTask *task, gpointer, gpointer data, GCancellable *) {
    auto &request = *static_cast<TranslationTask *>(data);
    auto *raw = request.offline
        ? msime_client_candidate_gloss_request(
              reinterpret_cast<const uint8_t *>(request.gloss_query.data()), request.gloss_query.size(),
              reinterpret_cast<const uint8_t *>(request.resources.data()), request.resources.size())
        : msime_client_translation_provider_request(
              reinterpret_cast<const uint8_t *>(request.query.data()), request.query.size(),
              reinterpret_cast<const uint8_t *>(request.socket.data()), request.socket.size());
    // Persistence is display-data I/O and stays on this worker. View updates
    // still pass the session/epoch/generation checks in translation_complete.
    if (!request.offline && raw) {
      try {
        const auto query = Json::parse(request.query);
        const auto result = Json::parse(raw);
        const auto user_data = Json::parse(request.gloss_query).value("user_data", std::string{});
        if (query.value("target_language", std::string{}) == "en" &&
            result.value("ok", false) && !user_data.empty()) {
          const auto save = Json{{"target_language", "en"},
                                 {"translations", result.at("value").at("translations")}}.dump();
          msime_client_string_free(msime_client_translation_gloss_save(
              reinterpret_cast<const uint8_t *>(save.data()), save.size(),
              reinterpret_cast<const uint8_t *>(user_data.data()), user_data.size()));
        }
      } catch (...) {}
    }
    g_task_return_pointer(task, raw, [](gpointer value) { msime_client_string_free(static_cast<char *>(value)); });
  });
  g_object_unref(task);
}
void translation_dispatch(IBusEngine *engine) {
  auto &s = state(engine);
  const bool online_translation =
      s.candidate_translations && !s.translation_provider_socket.empty();
  const auto local_mode = s.view.value("local_mode", std::string("none"));
  // Either switch can reach an offline gloss: English, or an installed non-English dictionary, which only the query below can name. The command mode may hold /fy, whose query is asked for whatever they say.
  if (!(s.candidate_english_gloss || s.candidate_translations || local_mode == "command") ||
      s.translation_loading || !s.session ||
      !s.focused || s.blocked || !s.input_enabled || s.private_input ||
      !s.view.value("candidates", Json::array()).size())
    return;
  try {
    auto query = response(msime_client_translation_query(s.session));
    if (query.is_null() || !query.is_object()) return;
    // /fy asks the selected service alone, in the query's own target language, and answers with a row rather than a gloss (CandidateTranslationPolicy.h): no offline dictionary, no other service, no gloss cache.
    if (msime::linux_host::command_translation_query(local_mode,
                                                     query.value("sentence", false))) {
      if (s.translation_provider_socket.empty()) return;
      auto texts = Json::array();
      for (const auto &candidate : query.at("candidates"))
        texts.push_back(candidate.at("text"));
      query["candidates"] = std::move(texts);
      const auto encoded = query.dump();
      if (encoded == s.translation_dispatched_query) return;
      s.translation_dispatched_query = encoded;
      start_translation_task(engine, TranslationTask{
          s.session, s.provider_epoch, encoded, s.translation_provider_socket,
          configured.value("resources", std::string{}),
          Json{{"generation", query.at("generation")}, {"candidates", Json::array()}}.dump(),
          false, Json::array(), false});
      return;
    }
    if (!(s.candidate_english_gloss || s.candidate_translations)) return;
    const auto &target = s.translation_target_language;
    const auto installed = query.value("offline_gloss_languages", Json::array());
    const bool dictionary = target != "en" && installed.is_array() &&
        std::find(installed.begin(), installed.end(), target) != installed.end();
    const bool offline_gloss = target == "en" || dictionary;
    if (!offline_gloss && !online_translation) return;
    query["target_language"] = target;
    // The shared query carries candidate objects; the socket protocol takes
    // the candidate texts, matching TranslationQuery in the host API.
    auto texts = Json::array();
    for (const auto &candidate : query.at("candidates"))
      texts.push_back(candidate.at("text"));
    query["candidates"] = std::move(texts);
    const auto encoded = query.dump();
    // Applying a gloss redraws the same generation. Do not start another
    // provider request until the query or provider configuration changes.
    if (encoded == s.translation_dispatched_query) return;
    s.translation_dispatched_query = encoded;
    auto candidates = Json::array();
    for (const auto &candidate : s.view.at("candidates"))
      candidates.push_back({{"text", candidate.at("text")}, {"source", candidate.at("source")}});
    auto gloss_query = Json{{"generation", query.at("generation")},
                            {"user_data", configured.value("user_data", std::string{})},
                            {"candidates", candidates}};
    if (dictionary) gloss_query["target_language"] = target;
    const auto provider_socket =
        s.candidate_translations ? s.translation_provider_socket : std::string{};
    start_translation_task(engine, TranslationTask{
        s.session, s.provider_epoch, encoded, provider_socket,
        configured.value("resources", std::string{}), gloss_query.dump(),
        offline_gloss,
        Json::array(), dictionary});
  } catch (...) { s.translation_loading = false; }
}

// Translate only after an explicit menu action. Candidate glosses retain
// their normal 40-character bound; this separate single-item request keeps a
// long composition off the network path while the user is typing.
void translate_sentence(IBusEngine *engine) {
  constexpr size_t kMaxSentenceChars = 512;
  auto &s = state(engine);
  if (s.translation_loading || !s.session || !s.focused || s.blocked ||
      !s.input_enabled || s.private_input || s.translation_provider_socket.empty())
    return;
  try {
    auto query = response(msime_client_translation_query(s.session));
    const auto candidates = s.view.value("candidates", Json::array());
    if (!query.is_object() || !candidates.is_array() || candidates.empty())
      return;
    const Json *selected = &candidates.front();
    for (const auto &candidate : candidates)
      if (candidate.value("highlighted", false)) { selected = &candidate; break; }
    const auto text = selected->value("text", std::string{});
    if (text.empty() || msime::linux_host::utf8_scalar_count(text) > kMaxSentenceChars)
      return;
    query["sentence"] = true;
    query["target_language"] = s.translation_target_language;
    query["candidates"] = Json::array({text});
    const auto encoded = query.dump();
    s.sentence_translation_generation = query.at("generation").get<uint64_t>();
    const auto source = selected->value("source", uint8_t{0});
    const auto gloss_query = Json{
        {"generation", query.at("generation")},
        {"user_data", configured.value("user_data", std::string{})},
        {"candidates", Json::array({Json{{"text", text}, {"source", source}}})}}
                                 .dump();
    start_translation_task(engine, TranslationTask{
        s.session, s.provider_epoch, encoded, s.translation_provider_socket,
        configured.value("resources", std::string{}), gloss_query,
        false, Json::array(), false});
  } catch (...) { s.translation_loading = false; }
}
// Match the Windows translation worker's 500ms idle window. Only copy
// the current Engine query when dispatching, never at the first keystroke.
/// How long the composition has to stand still before the larger model ranks it.
///
/// Shorter than the 500ms the cloud and translation paths wait, because this one is local and its
/// result changes what the user is looking at rather than annotating it. Long enough that it does
/// not fire between the keystrokes of ordinary typing, which is what keeps the 24M model off the
/// keystroke path where it measures p95 153ms against a 16ms frame.
constexpr guint kSettledRerankDelayMs = 150;

/// Re-rank the visible candidates with the settled model once typing stops.
///
/// Every keystroke reschedules, so the pass only runs when the user actually pauses. The runtime
/// reports whether the order moved and the window is only redrawn when it did: repainting an
/// identical candidate list on every pause is a flicker with no explanation behind it.
///
/// Inert unless a settled model was installed — `msime_client_rerank_settled` answers `moved:
/// false` immediately when none is attached, which is every installation that ships one model.
void settled_rerank_schedule(IBusEngine *engine) {
  auto &s = state(engine);
  if (s.settled_rerank_source) {
    const auto source = s.settled_rerank_source;
    s.settled_rerank_source = 0;
    g_source_remove(source);
  }
  if (!s.session || !s.focused || s.blocked || !s.input_enabled ||
      s.view.value("candidates", Json::array()).empty())
    return;
  s.settled_rerank_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, kSettledRerankDelayMs,
      [](gpointer data) -> gboolean {
        auto *engine = IBUS_ENGINE(data);
        auto &s = state(engine);
        s.settled_rerank_source = 0;
        if (!s.session) return G_SOURCE_REMOVE;
        try {
          const auto applied = response(msime_client_rerank_settled(s.session));
          if (applied.value("moved", false) && applied.contains("view")) {
            s.view = applied.at("view");
            render(engine, s.view);
          }
        } catch (...) {
        }
        return G_SOURCE_REMOVE;
      },
      engine, nullptr);
}

void translation_schedule(IBusEngine *engine) {
  auto &s = state(engine);
  if (s.translation_delay_source) {
    const auto source = s.translation_delay_source;
    s.translation_delay_source = 0;
    g_source_remove(source);
  }
  // The same switches reach every gloss, offline or online; translation_dispatch decides which applies once it has the query. The command mode may hold /fy, which does not depend on them.
  if (!(s.candidate_english_gloss || s.candidate_translations ||
        s.view.value("local_mode", std::string("none")) == "command") ||
      !s.session || !s.focused || s.blocked || !s.input_enabled || s.private_input ||
      s.view.value("candidates", Json::array()).empty())
    return;
  s.translation_delay_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, 500,
      [](gpointer data) -> gboolean {
        auto *engine = IBUS_ENGINE(data);
        state(engine).translation_delay_source = 0;
        translation_dispatch(engine);
        return G_SOURCE_REMOVE;
      },
      g_object_ref(engine), [](gpointer data) { g_object_unref(data); });
}
void online_complete(GObject *source, GAsyncResult *result, gpointer);
bool online_request_is_stale(IBusEngine *engine, const std::string &encoded) {
  try {
    const auto request = Json::parse(encoded);
    const auto generation = request.at("generation").get<uint64_t>();
    auto &s = state(engine);
    if (!s.session)
      return false;
    const auto current = response(msime_client_online_query(s.session));
    return current.is_object() &&
           current.value("generation", generation) != generation;
  } catch (...) {
    return false;
  }
}
// Send one source's provider request. The AI source is sent twice per input change: a cache-only probe at once, then the network request after its own idle delay.
void online_dispatch(IBusEngine *engine, uint8_t only_source, bool ai_cache_only = false) {
  auto &s = state(engine);
  // Private and no-spellcheck fields stay offline, matching Fcitx5 privateInput().
  if (s.online_provider_socket.empty() || !s.session ||
      !s.focused || s.blocked || !s.input_enabled || s.private_input)
    return;
  try {
    auto query = response(msime_client_online_query(s.session));
    if (query.is_object()) {
      query["cloud_candidates"] = s.cloud_candidates;
      if (s.applied_preferences_snapshot.is_object()) {
        const auto desired = s.applied_preferences_snapshot.value("ai_assistant", Json::object());
        const auto current = query.value("ai_assistant", Json::object());
        bool matches = desired.value("enabled", false) && current.value("enabled", false);
        // Engine preference application may be deferred until composition
        // ends. Never launch another request with the superseded AI settings.
        for (const auto *key : {"provider", "model", "endpoint", "candidate_limit",
                                "prompt_id", "prompt_custom_1",
                                "prompt_custom_2", "prompt_custom_3"}) {
          if (desired.contains(key) &&
              (!current.contains(key) || desired.at(key) != current.at(key)))
            matches = false;
        }
        if (!matches) {
          // Eligibility is part of Engine's response identity. Remove the
          // provider configuration rather than changing that identity.
          query.erase("ai_assistant");
        }
      }
    }
    if (!query.is_object()) return;
    const auto ai = query.value("ai_assistant", Json::object());
    const bool ai_requested = query.value("ai_eligible", false) &&
                              ai.is_object() && ai.value("enabled", false);
    if (!(s.cloud_candidates && query.value("cloud_eligible", false)) && !ai_requested)
      return;
    // The runtime puts its own commit history in every query; never let it leave a private field.
    if (s.private_input)
      query.erase("ai_context");
    else if (ai_requested)
      query["ai_context"] = s.ai_context;
    const auto encoded = query.dump();
    // Keep the original Engine identity for application, while each transport
    // request enables only one source. Fast cloud results need not wait for AI.
    std::vector<std::unique_ptr<OnlineTask>> requests;
    requests.reserve(2);
    for (uint8_t source = 0; source < 2; ++source) {
      if (source != only_source) continue;
      // Each source has one in-flight request and its own duplicate guard. A pending AI result must not delay cloud for a newer composition.
      if (s.online_loading[source] || encoded == s.online_dispatched_query[source]) continue;
      if (source == 0 && !(s.cloud_candidates && query.value("cloud_eligible", false))) continue;
      if (source == 1 && !ai_requested) continue;
      auto provider_query = query;
      if (source == 0) {
        provider_query.erase("ai_assistant");
        provider_query.erase("ai_context");
      } else {
        provider_query["cloud_candidates"] = false;
        if (ai_cache_only) provider_query["ai_cache_only"] = true;
      }
      requests.push_back(std::make_unique<OnlineTask>(OnlineTask{
          s.session, s.provider_epoch, encoded, s.online_provider_socket,
          provider_query.dump(), source, ai_cache_only}));
    }
    for (auto &request : requests) {
      s.online_dispatched_query[request->source] = encoded;
      s.online_loading[request->source] = true;
      auto task = g_task_new(G_OBJECT(engine), nullptr, online_complete, nullptr);
      g_task_set_task_data(task, request.release(), [](gpointer value) {
        delete static_cast<OnlineTask *>(value);
      });
      g_task_run_in_thread(task, [](GTask *task, gpointer, gpointer data, GCancellable *) {
        auto &request = *static_cast<OnlineTask *>(data);
        auto *raw = msime_client_online_provider_request(
            reinterpret_cast<const uint8_t *>(request.provider_query.data()), request.provider_query.size(),
            reinterpret_cast<const uint8_t *>(request.socket.data()), request.socket.size());
        g_task_return_pointer(task, raw, [](gpointer value) {
          msime_client_string_free(static_cast<char *>(value));
        });
      });
      g_object_unref(task);
    }
  } catch (...) {}
}
// Match Windows cloud_ime's 500ms and ai_assistant's 650ms idle delays without sleeping on the IBus input thread. Read the latest Engine query only when a timer fires.
constexpr guint kCloudIdleDelayMs = 500;
constexpr guint kAiIdleDelayMs = 650;
void online_schedule(IBusEngine *engine) {
  auto &s = state(engine);
  for (auto *pending : {&s.online_delay_source, &s.ai_delay_source}) {
    if (*pending) {
      const auto source = *pending;
      *pending = 0;
      g_source_remove(source);
    }
  }
  if (s.online_provider_socket.empty() || !s.session || !s.focused ||
      s.blocked || !s.input_enabled || s.private_input)
    return;
  s.online_delay_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, kCloudIdleDelayMs,
      [](gpointer data) -> gboolean {
        auto *engine = IBUS_ENGINE(data);
        state(engine).online_delay_source = 0;
        online_dispatch(engine, 0);
        return G_SOURCE_REMOVE;
      },
      g_object_ref(engine), [](gpointer data) { g_object_unref(data); });
  s.ai_delay_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, kAiIdleDelayMs,
      [](gpointer data) -> gboolean {
        auto *engine = IBUS_ENGINE(data);
        state(engine).ai_delay_source = 0;
        online_dispatch(engine, 1);
        return G_SOURCE_REMOVE;
      },
      g_object_ref(engine), [](gpointer data) { g_object_unref(data); });
  // Windows AiAssistant shows a cached answer without waiting for the idle delay. The probe never reaches the network, so it needs no debounce.
  online_dispatch(engine, 1, true);
}
void translation_complete(GObject *source, GAsyncResult *result, gpointer) {
  auto engine = IBUS_ENGINE(source);
  auto &s = state(engine);
  std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
      static_cast<char *>(g_task_propagate_pointer(G_TASK(result), nullptr)),
      msime_client_string_free);
  const auto *request = static_cast<const TranslationTask *>(
      g_task_get_task_data(G_TASK(result)));
  if (!request || request->session != s.session || request->epoch != s.provider_epoch)
    return;
  s.translation_loading = false;
  if (!s.session || !s.focused || s.blocked || !s.input_enabled)
    return;
  if (translation_request_is_stale(engine, request->query)) {
    translation_schedule(engine);
    settled_rerank_schedule(engine);
    return;
  }
  auto translations = request->local_translations;
  bool provider_response_valid = false;
  bool provider_answered = false;
  if (raw) {
    try {
      const auto document = Json::parse(raw.get());
      if (document.value("ok", false)) {
        const auto &value = document.at("value");
        if (value.is_object() && value.at("translations").is_array()) {
          const auto &remote = value.at("translations");
          provider_response_valid = true;
          provider_answered = !remote.empty();
          if (request->prefer_online && !request->offline)
            translations = prefer_online_translations(translations, remote);
          else
            for (const auto &item : remote)
              translations.push_back(item);
        }
      }
    } catch (...) {}
  }
  try {
    auto query = Json::parse(request->query);
    if (msime::linux_host::should_retry_translation_after_provider(
            !request->offline, provider_response_valid, provider_answered) &&
        s.translation_dispatched_query == request->query)
      s.translation_dispatched_query.clear();
    const auto generation = query.at("generation").get<uint64_t>();
    const auto encoded = translations.dump();
    auto applied = response(msime_client_apply_translations(
        s.session, generation, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
    s.view = applied.at("view");
    render(engine, s.view);
    // Publish local hits before starting network work, and only send misses; a non-English dictionary's hits are asked about again, since the provider outranks it.
    if (request->offline && !request->socket.empty()) {
      auto missing = Json::array();
      for (const auto &text : query.at("candidates")) {
        const bool found = !request->prefer_online &&
            std::any_of(translations.begin(), translations.end(),
                        [&](const Json &item) { return item.at("text") == text; });
        if (!found) missing.push_back(text);
      }
      if (!missing.empty()) {
        query["candidates"] = std::move(missing);
        auto next = *request;
        next.query = query.dump();
        next.offline = false;
        next.local_translations = std::move(translations);
        start_translation_task(engine, std::move(next));
      }
    }
  } catch (...) {}
}
void online_complete(GObject *source, GAsyncResult *result, gpointer) {
  auto engine = IBUS_ENGINE(source);
  auto &s = state(engine);
  std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
      static_cast<char *>(g_task_propagate_pointer(G_TASK(result), nullptr)),
      msime_client_string_free);
  const auto *request = static_cast<const OnlineTask *>(
      g_task_get_task_data(G_TASK(result)));
  if (!request || request->source >= s.online_loading.size() ||
      request->session != s.session || request->epoch != s.provider_epoch)
    return;
  s.online_loading[request->source] = false;
  if (!s.session || !s.focused || s.blocked || !s.input_enabled)
    return;
  const auto retry_empty_ai = [&] {
    if (request->source == 1 &&
        s.online_dispatched_query[1] == request->query) {
      s.online_dispatched_query[1].clear();
      // A missed cache probe hands over to the network request. If the AI idle timer fired while the probe was still in flight, that request was skipped, so send it now.
      if (request->ai_cache_only && !s.ai_delay_source)
        online_dispatch(engine, 1);
    }
  };
  if (online_request_is_stale(engine, request->query)) {
    online_schedule(engine);
    return;
  }
  if (!raw) {
    retry_empty_ai();
    return;
  }
  try {
    const auto document = Json::parse(raw.get());
    if (!document.value("ok", false)) {
      retry_empty_ai();
      return;
    }
    const auto value = document.at("value");
    if (!value.is_object()) {
      retry_empty_ai();
      return;
    }
    const auto candidates = value.value("candidates", Json());
    if (!candidates.is_array() || candidates.size() > 11) {
      retry_empty_ai();
      return;
    }
    Json groups[2] = {Json::array(), Json::array()};
    for (const auto &item : candidates) {
      const auto candidate = item.value("text", std::string{});
      const auto source = item.value("source", 255);
      if (candidate.empty() || source != request->source ||
          (!s.cloud_candidates && source == 0)) continue;
      groups[source].push_back(candidate);
    }
    if (groups[request->source].empty()) {
      retry_empty_ai();
      return;
    }
    bool ai_applied = false;
    for (uint8_t source = 0; source < 2; ++source) {
      if (groups[source].empty()) continue;
      const auto encoded = groups[source].dump();
      auto applied = response(msime_client_apply_online_candidates(
          s.session, reinterpret_cast<const uint8_t *>(request->query.data()), request->query.size(),
          reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(), source));
      s.view = applied.at("view");
      if (source == 1) {
        if (applied.value("applied", false))
          ai_applied = true;
        else
          retry_empty_ai();
      }
    }
    render(engine, s.view);
    // AI insertion changes the visible candidate generation just like cloud
    // insertion. Refresh translations immediately for the AI path so the new
    // row and its neighbours are not left unannotated during the idle delay.
    // Cloud-only updates retain the normal 500ms debounce.
    if (ai_applied)
      translation_dispatch(engine);
    else
      translation_schedule(engine);
    settled_rerank_schedule(engine);
  } catch (...) {}
}
} // namespace

struct MsimeIbusEngine {
  IBusEngine parent;
  State *state;
};
struct MsimeIbusEngineClass {
  IBusEngineClass parent;
};
G_DEFINE_TYPE(MsimeIbusEngine, msime_ibus_engine, IBUS_TYPE_ENGINE)

namespace {
State &state(IBusEngine *engine) {
  return *reinterpret_cast<MsimeIbusEngine *>(engine)->state;
}
void clipboard_complete(GObject *source, GAsyncResult *result, gpointer) {
  auto self = reinterpret_cast<MsimeIbusEngine *>(source);
  if (!self->state)
    return;
  auto &s = *self->state;
  s.clipboard_loading = false;
  auto request = static_cast<ClipboardTask *>(
      g_task_get_task_data(G_TASK(result)));
  // Always propagate the task result, including stale completions, so its
  // destroy notifier releases the worker allocation.
  auto *items = static_cast<std::vector<std::string> *>(
      g_task_propagate_pointer(G_TASK(result), nullptr));
  if (!s.clipboard_enabled || request->generation != s.clipboard_generation || !s.focused || s.blocked ||
      request->path != s.clipboard_history_path) {
    delete items;
    clipboard_schedule(IBUS_ENGINE(source));
    return;
  }
  if (!items)
    return;
  ++s.clipboard_generation;
  s.clipboard_items_cache = std::move(*items);
  s.clipboard_loaded = true;
  delete items;
  publish_mode(IBUS_ENGINE(source));
}
std::string candidate_action_name(const char *action, const Json &id) {
  return std::string(action) + "/" + std::to_string(id.at("session").get<uint64_t>()) +
         "/" + std::to_string(id.at("generation").get<uint64_t>()) +
         "/" + std::to_string(id.at("index").get<size_t>());
}
std::string nine_key_spelling_action_name(uint64_t session, uint64_t generation,
                                          size_t index) {
  return "NineKeySpelling/" + std::to_string(session) + "/" +
         std::to_string(generation) + "/" + std::to_string(index);
}
IBusProperty *nine_key_spellings(IBusEngine *engine) {
  const auto &s = state(engine);
  auto menu = ibus_prop_list_new();
  const auto spellings = s.rendered_view.is_object()
                             ? s.rendered_view.value("nine_key_spellings", Json::array())
                             : Json::array();
  const bool available = s.session && s.focused && !s.blocked && s.input_enabled &&
                         s.rendered_session == s.session && s.rendered_view.is_object() &&
                         s.rendered_view.value("nine_key", false) && spellings.is_array();
  bool has_items = false;
  if (available) {
    const auto generation = s.rendered_view.value("generation", uint64_t{0});
    for (size_t index = 0; index < spellings.size(); ++index) {
      if (!spellings.at(index).is_string())
        continue;
      auto spelling = spellings.at(index).get<std::string>();
      if (spelling.empty() || spelling.size() > 64)
        continue;
      const auto name = nine_key_spelling_action_name(s.session, generation, index);
      const auto label = std::to_string(index + 1) + ". " + spelling;
      ibus_prop_list_append(menu, ibus_property_new(
          name.c_str(), PROP_TYPE_NORMAL, ibus_text_new_from_string(label.c_str()),
          "", ibus_text_new_from_static_string("选择九键拼音"), TRUE, TRUE,
          PROP_STATE_UNCHECKED, nullptr));
      has_items = true;
    }
  }
  const bool enabled = available && has_items;
  return ibus_property_new(
      "NineKeySpellings", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("九键拼音"), "",
      ibus_text_new_from_static_string("选择九键数字对应的拼音"), enabled, TRUE,
      PROP_STATE_UNCHECKED, menu);
}
IBusProperty *candidate_actions(IBusEngine *engine) {
  const auto &s = state(engine);
  auto items = ibus_prop_list_new();
  // Candidate actions must describe the same page that was last handed to
  // IBus. The live Engine view can advance before the panel redraws (and is
  // retained while voice owns the preedit), so reading it here can bind a
  // menu action to an invisible or newer candidate.
  const auto candidates = s.rendered_candidates.is_array()
                              ? s.rendered_candidates
                              : Json::array();
  const auto scheme = s.rendered_scheme;
  const bool actions_available = s.focused && !s.blocked && s.input_enabled &&
                                 s.session && s.rendered_session == s.session;
  bool editable_candidates = false;
  for (size_t index = 0; index < candidates.size(); ++index) {
    const auto &candidate = candidates.at(index);
    if (!candidate.is_object() || !candidate.contains("id") ||
        !candidate.at("id").is_object())
      continue;
    const auto &id = candidate.at("id");
    if (!id.contains("session") || !id.contains("generation") || !id.contains("index") ||
        !id.at("session").is_number_unsigned() || !id.at("generation").is_number_unsigned() ||
        !id.at("index").is_number_unsigned())
      continue;
    // Engine only persists operations for local dictionary and English dictionary entries. Dynamic and local-mode candidates, and those of every scheme outside the main Chinese lexicon, have no user-dictionary identity to mutate.
    const auto source = candidate.value("source", 0);
    if (!msime::linux_host::candidate_dictionary_actions_available(scheme, source))
      continue;
    editable_candidates = true;
    const auto slot = index + 1;
    auto actions = ibus_prop_list_new();
    const auto candidate_text = candidate.value("text", std::string{});
    auto preview = candidate_text;
    msime_clipboard_truncate(preview, 48);
    const auto entry_label = std::to_string(slot) + ". " + preview;
    const auto entry_name = candidate_action_name("CandidateEntry", id);
    auto entry = ibus_property_new(
        entry_name.c_str(), PROP_TYPE_MENU,
        ibus_text_new_from_string(entry_label.c_str()), "",
        ibus_text_new_from_static_string("选择此候选的操作"), actions_available, TRUE,
        PROP_STATE_UNCHECKED, actions);
    ibus_prop_list_append(items, entry);
    const auto fixed_position = candidate.value("fixed_position", 0);
    // The parent entry already names the slot ("N. preview"), so the items carry only the action, worded like the Windows candidate menu.
    std::vector<std::pair<const char *, std::string>> candidate_commands;
    candidate_commands.reserve(7);
    candidate_commands.emplace_back("CandidatePin", msime::linux_host::candidate_pin_label);
    if (msime::linux_host::candidate_dictionary_removal_available(
            scheme, source, candidate_text))
      candidate_commands.emplace_back("CandidateRemove", "删除候选");
    for (const char *fix : {"CandidateFix1", "CandidateFix2", "CandidateFix3",
                            "CandidateFix4", "CandidateFix5"})
      candidate_commands.emplace_back(
          fix, msime::linux_host::candidate_fix_label(fix[12] - '0'));
    for (const auto &[action, title] : candidate_commands) {
      const auto name = candidate_action_name(action, candidate.at("id"));
      const auto state = g_str_has_prefix(action, "CandidateFix") &&
                                 fixed_position ==
                                     std::stoi(std::string(action).substr(12))
                             ? PROP_STATE_CHECKED
                             : PROP_STATE_UNCHECKED;
      ibus_prop_list_append(actions, ibus_property_new(
          name.c_str(), PROP_TYPE_NORMAL, ibus_text_new_from_string(title.c_str()), "",
          ibus_text_new_from_static_string("操作当前页候选"), actions_available, TRUE,
          state, nullptr));
    }
    const auto clear_name = candidate_action_name("CandidateClear", candidate.at("id"));
    ibus_prop_list_append(actions, ibus_property_new(
        clear_name.c_str(), PROP_TYPE_NORMAL,
        ibus_text_new_from_static_string("取消固定"), "",
        ibus_text_new_from_static_string("取消当前候选的位置固定"),
        actions_available && fixed_position > 0, TRUE,
        PROP_STATE_UNCHECKED, nullptr));
  }
  const auto page = s.rendered_view.is_object()
                        ? s.rendered_view.value("page", size_t{0})
                        : size_t{0};
  const auto page_count = s.rendered_view.is_object()
                              ? s.rendered_view.value("page_count", size_t{0})
                              : size_t{0};
  const bool paging_available = actions_available && !candidates.empty() &&
                                page_count > 1;
  auto paging = ibus_prop_list_new();
  ibus_prop_list_append(
      paging, ibus_property_new(
                  "CandidatePreviousPage", PROP_TYPE_NORMAL,
                  ibus_text_new_from_static_string("上一页"), "",
                  ibus_text_new_from_static_string("显示上一页候选"),
                  paging_available && page > 0, TRUE, PROP_STATE_UNCHECKED, nullptr));
  ibus_prop_list_append(
      paging, ibus_property_new(
                  "CandidateNextPage", PROP_TYPE_NORMAL,
                  ibus_text_new_from_static_string("下一页"), "",
                  ibus_text_new_from_static_string("显示下一页候选"),
                  paging_available && page + 1 < page_count, TRUE,
                  PROP_STATE_UNCHECKED, nullptr));
  ibus_prop_list_append(
      items, ibus_property_new(
                 "CandidatePaging", PROP_TYPE_MENU,
                 ibus_text_new_from_static_string("候选翻页"), "",
                 ibus_text_new_from_static_string("切换候选页"), paging_available,
                 TRUE, PROP_STATE_UNCHECKED, paging));
  return ibus_property_new("CandidateActions", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("候选操作"), "",
      ibus_text_new_from_static_string("固定或删除当前页候选"),
      s.session && s.focused && !s.blocked && s.input_enabled && editable_candidates,
      TRUE, PROP_STATE_UNCHECKED, items);
}
void cancel_candidate_properties(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.candidate_properties_source) return;
  const auto source = s.candidate_properties_source;
  s.candidate_properties_source = 0;
  g_source_remove(source);
}
void publish_candidate_properties(IBusEngine *engine) {
  cancel_candidate_properties(engine);
  if (candidate_panel_is_gnome_shell()) return;
  ibus_engine_update_property(engine, candidate_actions(engine));
  ibus_engine_update_property(engine, nine_key_spellings(engine));
}
void schedule_candidate_properties(IBusEngine *engine) {
  cancel_candidate_properties(engine);
  if (candidate_panel_is_gnome_shell()) return;
  state(engine).candidate_properties_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, 400,
      [](gpointer data) -> gboolean {
        auto *engine = IBUS_ENGINE(data);
        state(engine).candidate_properties_source = 0;
        ibus_engine_update_property(engine, candidate_actions(engine));
        ibus_engine_update_property(engine, nine_key_spellings(engine));
        return G_SOURCE_REMOVE;
      },
      g_object_ref(engine), [](gpointer data) { g_object_unref(data); });
}
IBusProperty *input_mode_property(IBusEngine *engine) {
  const auto &s = state(engine);
  const auto scheme = effective_scheme(s);
  auto *property = ibus_property_new(
      "InputMode", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("中文"), "",
      ibus_text_new_from_static_string(s.input_enabled ? "使用当前输入方案"
                                                       : "直接输入（不转换）"),
      s.focused && !s.blocked, TRUE,
      s.input_enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  const char *symbol = "文";
  switch (msime::linux_host::input_mode_indicator(s.input_enabled, scheme, s.caps_lock)) {
  case msime::linux_host::InputModeIndicator::Chinese: symbol = "文"; break;
  case msime::linux_host::InputModeIndicator::Japanese: symbol = "日"; break;
  case msime::linux_host::InputModeIndicator::Korean: symbol = "한"; break;
  case msime::linux_host::InputModeIndicator::Cantonese: symbol = "粤"; break;
  case msime::linux_host::InputModeIndicator::Zhuyin: symbol = "注"; break;
  case msime::linux_host::InputModeIndicator::Vietnamese: symbol = "越"; break;
  case msime::linux_host::InputModeIndicator::Tibetan: symbol = "藏"; break;
  case msime::linux_host::InputModeIndicator::English: symbol = "A"; break;
  case msime::linux_host::InputModeIndicator::CapsLock: symbol = "⇪"; break;
  }
  ibus_property_set_symbol(property, ibus_text_new_from_static_string(symbol));
  return property;
}
IBusProperty *gnome_settings_property(IBusEngine *engine) {
  const auto &s = state(engine);
  // Keep one direct action in GNOME Shell's input-source menu without
  // reintroducing the nested DesktopTools property tree that caused Shell
  // actor and GC churn. The existing activation path launches the native
  // Linux settings launcher, which hosts the shared settings surface.
  return ibus_property_new(
      "DesktopTools/Settings", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string("设置"), "",
      ibus_text_new_from_static_string("打开" MSIME_EDITION_DISPLAY_NAME "设置"),
      s.focused && !s.blocked, TRUE, PROP_STATE_UNCHECKED, nullptr);
}
void publish_mode(IBusEngine *engine, bool registration) {
  auto &s = state(engine);
  // GNOME Shell renders IBus properties inside its own input-source menu.
  // Repeatedly replacing this host's large nested property tree made Shell
  // rebuild actors and collect them until the whole desktop froze.
  if (candidate_panel_is_gnome_shell()) {
    auto *mode = input_mode_property(engine);
    auto *settings = gnome_settings_property(engine);
    if (registration) {
      auto *properties = ibus_prop_list_new();
      ibus_prop_list_append(properties, mode);
      ibus_prop_list_append(properties, settings);
      ibus_engine_register_properties(engine, properties);
    } else {
      ibus_engine_update_property(engine, mode);
      ibus_engine_update_property(engine, settings);
    }
    return;
  }
  const auto themes = theme_choices();
  // A choice made while no store was writable lives only here; drop it once the package it draws is no longer listed.
  if (s.theme_choice_override) {
    const auto custom = s.theme_choice_override->value("custom_theme", Json::object());
    const auto package = custom.value("candidate_skin", Json(nullptr));
    if (package.is_string() && !msime::linux_host::find_theme_choice(themes, package.get<std::string>()))
      s.theme_choice_override.reset();
  }
  clipboard_schedule(engine);
  auto toolbar = toolbar_property(engine);
  // The scheme the Engine runs: a Cantonese or Zhuyin preference without its dictionary falls back, and the menu checks the fallback.
  const auto active_scheme = effective_scheme(s);
  const int active_scheme_number = msime::linux_host::scheme_number(active_scheme);
  const bool japanese_scheme = active_scheme_number == msime::linux_host::scheme::Japanese;
  const bool korean_scheme = active_scheme_number == msime::linux_host::scheme::Korean;
  const bool vietnamese_scheme = active_scheme_number == msime::linux_host::scheme::Vietnamese;
  const bool tibetan_scheme = active_scheme_number == msime::linux_host::scheme::Tibetan;
  const bool chinese_scheme = msime::linux_host::scheme::IsChinese(active_scheme_number);
  const auto mixed_input = configured.at("preferences").value(
      "mixed_input", Json::object());
  const auto mixed_input_value = [&](const char *key, bool fallback) {
    if (!mixed_input.is_object()) return fallback;
    const auto value = mixed_input.find(key);
    return value != mixed_input.end() && value->is_boolean()
               ? value->get<bool>()
               : fallback;
  };
  const bool english_candidates = s.english_override.value_or(
      mixed_input_value("english", true));
  const bool emoji_candidates = s.emoji_override.value_or(
      mixed_input_value("emoji", false));
  const bool kaomoji_candidates = s.kaomoji_override.value_or(
      mixed_input_value("kaomoji", false));
  const bool nine_key = active_scheme == "quanpin" && s.view.is_object() &&
                        s.view.value("nine_key", false);
  const bool helpcode = (active_scheme == "quanpin" || active_scheme == "shuangpin") &&
      s.helpcode_override.value_or(configured.at("preferences")
          .value(active_scheme + "_helpcode", Json::object()).value("enabled", true));
  const auto layout = s.layout_override.value_or(
      configured.at("preferences").value("candidate_layout", "vertical"));
  const auto preedit = s.preedit_override.value_or(
      configured.at("preferences").value("tsf_preedit_style", "raw"));
  const auto theme = s.theme_override.value_or(
      configured.at("preferences").value("candidate_theme", "follow"));
  auto theme_preferences = configured.at("preferences");
  if (s.theme_choice_override) msime::linux_host::apply_theme_choice(theme_preferences, *s.theme_choice_override);
  const auto global_theme = msime::linux_host::current_theme_choice(theme_preferences, themes);
  auto property = input_mode_property(engine);
  const auto voice_label = s.voice_active
      ? (s.voice_space_locked && !s.voice_stopping
             ? std::string("录音已锁定") : s.voice_phase)
      : std::string("语音输入");
  auto voice = ibus_property_new(
      "VoiceInput", PROP_TYPE_TOGGLE,
      ibus_text_new_from_string(voice_label.c_str()), "",
      ibus_text_new_from_static_string("点击开始录音，再次点击结束录音并提交识别结果；Esc 取消"),
      s.focused && !s.blocked && s.voice_enabled &&
          !s.voice_provider_socket.empty() && !(s.voice_active && s.voice_stopping),
      TRUE, s.voice_active ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto voice_cancel_property = ibus_property_new(
      "VoiceCancel", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string("取消语音输入"), "",
      ibus_text_new_from_static_string("取消当前录音、识别或润色，不提交语音结果"),
      s.focused && !s.blocked && s.voice_active,
      s.voice_active, PROP_STATE_UNCHECKED, nullptr);
  auto cloud = ibus_property_new(
      "CloudCandidates", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("云联想"), "",
      ibus_text_new_from_static_string("通过用户管理的 provider 请求云候选"),
      s.focused && !s.blocked && s.input_enabled && s.session &&
          !s.online_provider_socket.empty() && !menu_save_pending,
      TRUE, s.cloud_candidates ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto translations = ibus_property_new(
      "CandidateTranslations", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("显示译文"), "",
      ibus_text_new_from_static_string("通过用户管理的 provider 请求候选翻译"),
      s.focused && !s.blocked && s.input_enabled && s.session &&
          !s.translation_provider_socket.empty() && !menu_save_pending,
      TRUE, s.candidate_translations ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto sentence_translation = ibus_property_new(
      "TranslateSentence", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string("翻译当前句子"), "",
      ibus_text_new_from_static_string("手动翻译当前首选候选句子，结果显示在候选区"),
      s.focused && !s.blocked && s.input_enabled && s.session &&
          !s.translation_provider_socket.empty() && !s.translation_loading,
      TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto translation_language = ibus_property_new(
      "TranslationLanguage", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("翻译目标语言"), "",
      ibus_text_new_from_static_string("选择候选翻译的目标语言"),
      s.focused && !s.blocked && s.input_enabled && s.session &&
          !s.translation_provider_socket.empty() && !menu_save_pending,
      TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto translation_language_menu = ibus_prop_list_new();
  const bool translation_available = s.focused && !s.blocked && s.input_enabled &&
      s.session && !s.translation_provider_socket.empty() && !menu_save_pending;
  for (const auto &[value, label] : {std::pair{"en", "英语"},
                                     std::pair{"fr", "法语"},
                                     std::pair{"ja", "日语"},
                                     std::pair{"es", "西班牙语"},
                                     std::pair{"ru", "俄语"},
                                     std::pair{"de", "德语"},
                                     std::pair{"ko", "韩语"}}) {
    auto item = ibus_property_new(
        (std::string("TranslationLanguage/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("切换候选翻译目标语言"), translation_available, TRUE,
        s.translation_target_language == value ? PROP_STATE_CHECKED
                                                : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_prop_list_append(translation_language_menu, item);
  }
  ibus_property_set_sub_props(translation_language, translation_language_menu);
  auto punctuation = ibus_property_new(
      "Punctuation", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("中文标点"), "",
      ibus_text_new_from_static_string("切换中文或英文标点"),
      s.focused && !s.blocked && s.input_enabled && s.session && !menu_save_pending, TRUE,
      s.chinese_punctuation ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto smart_punctuation = ibus_property_new(
      "SmartPunctuation", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("智能标点"), "",
      ibus_text_new_from_static_string("按上下文选择标点形式"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      s.smart_punctuation ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto smart_repeat = ibus_property_new(
      "SmartPunctuationRepeat", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("重复标点转中文"), "",
      ibus_text_new_from_static_string("短时间重复输入 ASCII 标点时替换为中文标点"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      s.smart_punctuation_repeat ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto paired = ibus_property_new(
      "PairedPunctuation", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("成对标点"), "",
      ibus_text_new_from_static_string("输入成对引号和括号"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      s.paired_punctuation ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto punctuation_lock = ibus_property_new(
      "PunctuationLock", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("标点锁定"), "",
      ibus_text_new_from_static_string("跟随输入模式或固定中文/英文标点"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED,
      nullptr);
  auto punctuation_lock_menu = ibus_prop_list_new();
  const bool punctuation_available = s.focused && !s.blocked && s.input_enabled &&
      s.session && !menu_save_pending;
  for (const auto &[value, label] : {std::pair{"follow", "跟随"},
                                     std::pair{"chinese", "固定中文"},
                                     std::pair{"english", "固定英文"}}) {
    auto item = ibus_property_new(
        (std::string("PunctuationLock/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_string(label), "",
        ibus_text_new_from_static_string("选择标点锁定策略"), punctuation_available, TRUE,
        s.punctuation_lock == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_prop_list_append(punctuation_lock_menu, item);
  }
  ibus_property_set_sub_props(punctuation_lock, punctuation_lock_menu);
  auto character_mode = ibus_property_new(
      "CharacterMode", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("全角字符"), "",
      ibus_text_new_from_static_string("切换 ASCII 全角或半角输出"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      s.fullwidth ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto traditional = ibus_property_new(
      "TraditionalOutput", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("繁体输出"), "",
      ibus_text_new_from_static_string("将中文候选和上屏文本转换为繁体"),
      s.focused && !s.blocked && s.input_enabled && s.session &&
          msime::linux_host::scheme::ScriptConversionApplies(active_scheme_number) && !menu_save_pending,
      TRUE, s.traditional_output ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto english = ibus_property_new(
      "EnglishCandidates", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("英文候选"), "",
      ibus_text_new_from_static_string("在中文方案中补充英文候选"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      english_candidates ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto english_mode = ibus_property_new(
      "EnglishMode", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("英文输入模式"), "",
      ibus_text_new_from_static_string("切换 Engine 的独立英文输入模式（Ctrl+Shift+E）"),
      s.focused && !s.blocked && s.input_enabled && s.session, TRUE,
      s.english_mode ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto helpcode_property = ibus_property_new(
      "Helpcode", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("辅助码"), "",
      ibus_text_new_from_static_string("启用候选辅助码提示"),
      s.focused && !s.blocked && s.input_enabled &&
          (active_scheme == "quanpin" || active_scheme == "shuangpin") && !menu_save_pending, TRUE,
      helpcode ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto helpcode_schema = ibus_property_new(
      "HelpcodeSchema", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("辅助码方案"), "",
      ibus_text_new_from_static_string("选择辅助码编码方案"),
      s.focused && !s.blocked && s.input_enabled &&
          (active_scheme == "quanpin" || active_scheme == "shuangpin") && !menu_save_pending, TRUE,
      PROP_STATE_UNCHECKED, nullptr);
  auto helpcode_schema_menu = ibus_prop_list_new();
  const auto schema = s.helpcode_schema_override.value_or(
      configured.at("preferences").value(active_scheme + "_helpcode", Json::object())
          .value("schema", std::string(msime::linux_host::default_helpcode_schema(
                               active_scheme))));
  for (const auto &[value, label] : msime::linux_host::kHelpcodeSchemaNames) {
    auto item = ibus_property_new(
        (std::string("HelpcodeSchema/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("切换辅助码方案"),
        s.focused && !s.blocked && s.input_enabled &&
            (active_scheme == "quanpin" || active_scheme == "shuangpin") &&
            !menu_save_pending,
        TRUE,
        schema == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(helpcode_schema_menu, item);
  }
  ibus_property_set_sub_props(helpcode_schema, helpcode_schema_menu);
  auto emoji = ibus_property_new(
      "EmojiCandidates", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("Emoji 候选"), "",
      ibus_text_new_from_static_string("在中文方案中补充 Emoji 候选"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      emoji_candidates ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto kaomoji = ibus_property_new(
      "KaomojiCandidates", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("颜文字候选"), "",
      ibus_text_new_from_static_string("在中文方案中补充颜文字候选"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      kaomoji_candidates ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  const bool clipboard_available = s.clipboard_enabled && s.input_enabled &&
                                   !s.clipboard_history_path.empty();
  const bool clipboard_menu_available = s.focused && !s.blocked &&
                                       clipboard_available;
  auto clipboard = ibus_property_new(
      "ClipboardHistory", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("剪贴板历史"), "",
      ibus_text_new_from_static_string("浏览、提交和管理最近 50 条历史文本"),
      s.focused && !s.blocked,
      TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto clipboard_menu = ibus_prop_list_new();
  const auto preferences_directory =
      configured.value("preferences_directory", std::string{});
  auto clipboard_toggle = ibus_property_new(
      "ClipboardHistory/Enabled", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("启用历史采集"), "",
      ibus_text_new_from_static_string("启用或停用本地剪贴板历史记录"),
      s.focused && !s.blocked && !menu_save_pending &&
          !preferences_directory.empty() &&
          preferences_directory.front() == '/',
      TRUE,
      s.clipboard_enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  ibus_prop_list_append(clipboard_menu, clipboard_toggle);
  auto open_clipboard = ibus_property_new(
      "ClipboardHistory/OpenPanel", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string("打开历史面板"), "",
      ibus_text_new_from_static_string("搜索、管理或主动开启本地剪贴板历史"),
      s.focused && !s.blocked, TRUE, PROP_STATE_UNCHECKED, nullptr);
  ibus_prop_list_append(clipboard_menu, open_clipboard);
  const auto &items = s.clipboard_items_cache;
  auto refresh_clipboard = ibus_property_new(
      "ClipboardHistory/Refresh", PROP_TYPE_NORMAL,
      ibus_text_new_from_static_string("刷新历史"), "",
      ibus_text_new_from_static_string("重新加载本地历史列表"),
      clipboard_menu_available && !s.clipboard_loading, TRUE, PROP_STATE_UNCHECKED, nullptr);
  ibus_prop_list_append(clipboard_menu, refresh_clipboard);
  IBusPropList *page = nullptr;
  for (size_t index = 0; clipboard_available && index < items.size(); ++index) {
    if (index % 10 == 0) {
      page = ibus_prop_list_new();
      const auto label = std::to_string(index + 1) + "–" +
                         std::to_string(std::min(index + 10, items.size()));
      auto group = ibus_property_new(
          (std::string("ClipboardHistoryPage/") + std::to_string(index / 10)).c_str(),
          PROP_TYPE_MENU, ibus_text_new_from_string(label.c_str()), "",
          ibus_text_new_from_static_string("浏览这一组历史"), clipboard_menu_available, TRUE,
          PROP_STATE_UNCHECKED, page);
      ibus_prop_list_append(clipboard_menu, group);
    }
    auto preview = items[index];
    for (char &character : preview) {
      if (character == '\r' || character == '\n' || character == '\t')
        character = ' ';
    }
    msime_clipboard_truncate(preview, 48);
    const auto label = std::to_string(index + 1) + ". " + preview;
    const auto identity = std::to_string(s.clipboard_generation) + "/" + std::to_string(index);
    auto item = ibus_property_new(
        (std::string("ClipboardHistory/") + identity).c_str(),
        PROP_TYPE_NORMAL, ibus_text_new_from_string(label.c_str()), "",
        ibus_text_new_from_static_string("提交历史文本"), clipboard_menu_available, TRUE,
        PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(page, item);
    auto remove = ibus_property_new(
        (std::string("ClipboardHistory/Remove/") + identity).c_str(),
        PROP_TYPE_NORMAL,
        ibus_text_new_from_string((std::string("删除 ") + std::to_string(index + 1)).c_str()),
        "", ibus_text_new_from_static_string("删除这一条历史文本"), clipboard_menu_available, TRUE,
        PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(page, remove);
  }
  auto clear_clipboard = ibus_property_new(
      (std::string("ClipboardHistory/Clear/") +
       std::to_string(s.clipboard_generation))
          .c_str(),
      PROP_TYPE_NORMAL, ibus_text_new_from_static_string("清空历史"), "",
      ibus_text_new_from_static_string("删除本地剪贴板历史文件"),
      clipboard_menu_available && !items.empty(), TRUE, PROP_STATE_UNCHECKED,
      nullptr);
  ibus_prop_list_append(clipboard_menu, clear_clipboard);
  ibus_property_set_sub_props(clipboard, clipboard_menu);
  auto layout_property = ibus_property_new(
      "CandidateLayout", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("候选布局"), "",
      ibus_text_new_from_static_string("选择候选排列方向"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto layout_menu = ibus_prop_list_new();
  const bool policy_available = s.focused && !s.blocked && s.input_enabled &&
      s.session && !menu_save_pending;
  auto vertical = ibus_property_new(
      "CandidateLayout/Vertical", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("竖排"), "",
      ibus_text_new_from_static_string("竖直排列候选"), policy_available, TRUE,
      layout == "vertical" ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto horizontal = ibus_property_new(
      "CandidateLayout/Horizontal", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("横排"), "",
      ibus_text_new_from_static_string("水平排列候选"), policy_available, TRUE,
      layout == "horizontal" ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  ibus_prop_list_append(layout_menu, vertical);
  ibus_prop_list_append(layout_menu, horizontal);
  ibus_property_set_sub_props(layout_property, layout_menu);
  auto page_size_property = ibus_property_new(
      "CandidatePageSize", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("候选数量"), "",
      ibus_text_new_from_static_string("选择每页显示的候选数量"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED,
      nullptr);
  auto page_size_menu = ibus_prop_list_new();
  const auto page_size = s.candidate_page_size_override.value_or(
      configured.at("preferences").value("candidate_page_size", 6));
  for (uint8_t value = 1; value <= 9; ++value) {
    auto item = ibus_property_new(
        (std::string("CandidatePageSize/") + std::to_string(value)).c_str(),
        PROP_TYPE_RADIO, ibus_text_new_from_string(std::to_string(value).c_str()),
        "", ibus_text_new_from_static_string("设置候选页大小"), policy_available, TRUE,
        page_size == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(page_size_menu, item);
  }
  ibus_property_set_sub_props(page_size_property, page_size_menu);
  auto frequency_property = ibus_property_new(
      "FrequencyMode", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("词频调节"), "",
      ibus_text_new_from_static_string("选择学习词频调节策略"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED,
      nullptr);
  auto frequency_menu = ibus_prop_list_new();
  const auto frequency = s.frequency_mode_override.value_or(
      configured.at("preferences").value("frequency", Json::object())
          .value("mode", "promote"));
  for (const auto &[value, label] : {std::pair{"disabled", "禁用"},
                                     std::pair{"pin", "固定"},
                                     std::pair{"halve", "减半"},
                                     std::pair{"linear", "线性"},
                                     std::pair{"promote", "提升"}}) {
    auto item = ibus_property_new(
        (std::string("FrequencyMode/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("设置词频调节模式"), policy_available, TRUE,
        frequency == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(frequency_menu, item);
  }
  ibus_property_set_sub_props(frequency_property, frequency_menu);
  auto frequency_trigger_property = ibus_property_new(
      "FrequencyTriggerCount", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("词频触发次数"), "",
      ibus_text_new_from_static_string("选择候选学习触发次数"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      PROP_STATE_UNCHECKED, nullptr);
  auto frequency_trigger_menu = ibus_prop_list_new();
  for (uint8_t value = 1; value <= 10; ++value) {
    auto item = ibus_property_new(
        (std::string("FrequencyTriggerCount/") + std::to_string(value)).c_str(),
        PROP_TYPE_RADIO, ibus_text_new_from_string((std::to_string(value) + " 次").c_str()), "",
        ibus_text_new_from_static_string("设置候选学习触发次数"), policy_available, TRUE,
        s.frequency_trigger_count == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(frequency_trigger_menu, item);
  }
  ibus_property_set_sub_props(frequency_trigger_property, frequency_trigger_menu);
  auto frequency_step_property = ibus_property_new(
      "FrequencyLinearStep", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("线性调整步长"), "",
      ibus_text_new_from_static_string("选择线性词频调整步长"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      PROP_STATE_UNCHECKED, nullptr);
  auto frequency_step_menu = ibus_prop_list_new();
  for (uint8_t value = 1; value <= 10; ++value) {
    auto item = ibus_property_new(
        (std::string("FrequencyLinearStep/") + std::to_string(value)).c_str(),
        PROP_TYPE_RADIO, ibus_text_new_from_string((std::to_string(value) + " 次").c_str()), "",
        ibus_text_new_from_static_string("设置线性词频调整步长"), policy_available, TRUE,
        s.frequency_linear_step == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(frequency_step_menu, item);
  }
  ibus_property_set_sub_props(frequency_step_property, frequency_step_menu);
  auto learning_property = ibus_property_new(
      "Learning", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("学习候选词频"), "",
      ibus_text_new_from_static_string("记录候选选择并调整词频"),
      policy_available && !s.private_input, TRUE,
      s.learning ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto number_row_property = ibus_property_new(
      "NumberRowSelection", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("数字选词"), "",
      ibus_text_new_from_static_string("使用数字键选择候选词"),
      s.focused && !s.blocked && s.input_enabled && !nine_key && !menu_save_pending, TRUE,
      s.number_row_selection && !nine_key ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto nine_key_property = ibus_property_new(
      "NineKey", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("九键输入"), "",
      ibus_text_new_from_static_string("使用数字键输入全拼并选择拼音候选"),
      s.focused && !s.blocked && s.input_enabled && active_scheme == "quanpin" && !menu_save_pending,
      TRUE, nine_key ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto nine_key_spellings_property = nine_key_spellings(engine);
  auto local_modes_property = ibus_property_new(
      "LocalModes", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("快捷模式"), "",
      ibus_text_new_from_static_string("启用或停用快捷模式"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED,
      nullptr);
  auto local_modes_menu = ibus_prop_list_new();
  const auto configured_local_modes = configured.at("preferences").value(
      "local_modes", Json::object());
  const std::pair<const char *, const char *> local_mode_options[] = {
      {"unicode", "Unicode（U 模式）"},
      {"date_time", "日期时间（T 模式）"},
      {"quick_phrase", "快捷短语（K 模式）"},
      {"emoji", "Emoji（E 模式）"},
      {"kaomoji", "颜文字（M 模式）"},
      {"super_jianpin", "超级简拼（J 模式）"},
      {"temporary_english", "临时英文（Y 模式）"},
      {"temporary_japanese", "临时日文（R 模式）"},
      {"expression", "计算与数字（V 模式）"},
      {"command", "指令（/ 模式）"},
      {"mention", "名单（@ 模式）"}};
  for (const auto &[key, label] : local_mode_options) {
    // 不带临时日文的版本（五笔版）不列这个本地模式：宿主库在这些版本里总是把它关掉，列出来也打不开。
    if (!MSIME_EDITION_TEMPORARY_JAPANESE && std::string_view(key) == "temporary_japanese") continue;
    const bool enabled = s.local_mode_overrides.contains(key)
                             ? s.local_mode_overrides.at(key).get<bool>()
                             : configured_local_modes.value(
                                   key, msime::linux_host::local_mode_enabled_by_default(key));
    auto item = ibus_property_new(
        (std::string("LocalModes/") + key).c_str(), PROP_TYPE_TOGGLE,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("快捷模式"),
        s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
        enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(local_modes_menu, item);
  }
  ibus_property_set_sub_props(local_modes_property, local_modes_menu);
  auto word_character_property = ibus_property_new(
      "WordCharacter", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("以词定字"), "",
      ibus_text_new_from_static_string("使用减号/等号或方括号选择词语首末汉字"),
      s.focused && !s.blocked && s.input_enabled && !menu_save_pending, TRUE,
      s.word_character.enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto preedit_property = ibus_property_new(
      "PreeditStyle", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("预编辑显示"), "",
      ibus_text_new_from_static_string("选择预编辑显示方式"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto preedit_menu = ibus_prop_list_new();
  const std::pair<const char *, const char *> preedit_options[] = {
      {"raw", "编码"}, {"pinyin", "拼音"}, {"empty", "隐藏"}};
  for (const auto &[value, label] : preedit_options) {
    auto item = ibus_property_new(
        (std::string("PreeditStyle/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_string(label), "",
        ibus_text_new_from_static_string("选择预编辑显示方式"), !menu_save_pending, TRUE,
        preedit == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_prop_list_append(preedit_menu, item);
  }
  ibus_property_set_sub_props(preedit_property, preedit_menu);
  auto shuangpin_preedit_property = ibus_property_new(
      "ShuangpinPreedit", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("双拼原始预编辑"), "",
      ibus_text_new_from_static_string("双拼输入时显示原始双拼编码"),
      s.focused && !s.blocked && s.input_enabled && active_scheme == "shuangpin" &&
          !menu_save_pending,
      TRUE, s.shuangpin_preedit_uses_raw ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
      nullptr);
  auto wubi_code_hint_property = ibus_property_new(
      "WubiCodeHint", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("五笔剩余编码"), "",
      ibus_text_new_from_static_string("在五笔候选后显示尚未输入的编码"),
      s.focused && !s.blocked && s.input_enabled && active_scheme == "wubi" &&
          !menu_save_pending,
      TRUE, s.wubi_code_hint ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto theme_property = ibus_property_new(
      "CandidateTheme", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("候选明暗"), "",
      ibus_text_new_from_static_string("选择候选窗口明暗"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto theme_menu = ibus_prop_list_new();
  const std::pair<const char *, const char *> theme_options[] = {
      {"follow", "跟随颜色模式"}, {"light", "浅色"}, {"dark", "深色"}};
  for (const auto &[value, label] : theme_options) {
    auto item = ibus_property_new(
        (std::string("CandidateTheme/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_string(label), "",
        ibus_text_new_from_static_string("选择候选明暗"), !menu_save_pending, TRUE,
        theme == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
    ibus_prop_list_append(theme_menu, item);
  }
  ibus_property_set_sub_props(theme_property, theme_menu);
  // Named after the entry drawn, as the Fcitx5 主题 action is and as the design's 主题 shows the current theme beside it.
  const auto *current_theme = msime::linux_host::find_theme_choice(themes, global_theme);
  auto global_theme_property = ibus_property_new(
      "GlobalTheme", PROP_TYPE_MENU,
      ibus_text_new_from_string(current_theme ? ("主题：" + current_theme->title).c_str() : "主题"), "",
      ibus_text_new_from_static_string("选择候选窗口、菜单与工具栏的主题"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto global_theme_menu = ibus_prop_list_new();
  for (const auto &entry : themes)
    ibus_prop_list_append(global_theme_menu, ibus_property_new(
        (std::string("GlobalTheme/") + entry.id).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_string(entry.title.c_str()), "",
        ibus_text_new_from_static_string(entry.package_base ? "外部候选皮肤，使用自定义主题" : "选择主题"),
        !menu_save_pending, TRUE,
        global_theme == entry.id ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr));
  ibus_property_set_sub_props(global_theme_property, global_theme_menu);
  auto scheme = ibus_property_new(
      "Scheme", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("输入方案"), "",
      ibus_text_new_from_static_string("选择中文、日文、韩文、越南文或藏文输入方案"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto scheme_menu = ibus_prop_list_new();
  auto chinese = ibus_property_new(
      "Scheme/Chinese", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("中文"), "",
      ibus_text_new_from_static_string("使用当前中文方案"), !menu_save_pending, TRUE,
      chinese_scheme ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto japanese = ibus_property_new(
      "Scheme/Japanese", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("日文"), "",
      ibus_text_new_from_static_string("使用日语罗马字方案"), !menu_save_pending, TRUE,
      japanese_scheme ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto korean = ibus_property_new(
      "Scheme/Korean", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("韩文"), "",
      ibus_text_new_from_static_string("使用韩语两套式方案"), !menu_save_pending, TRUE,
      korean_scheme ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto vietnamese = ibus_property_new(
      "Scheme/Vietnamese", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("越南文"), "",
      ibus_text_new_from_static_string("使用越南语输入方案"), !menu_save_pending, TRUE,
      vietnamese_scheme ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  auto tibetan = ibus_property_new(
      "Scheme/Tibetan", PROP_TYPE_RADIO,
      ibus_text_new_from_static_string("藏文"), "",
      ibus_text_new_from_static_string("使用藏文威利转写方案"), !menu_save_pending, TRUE,
      tibetan_scheme ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  ibus_prop_list_append(scheme_menu, chinese);
  ibus_prop_list_append(scheme_menu, japanese);
  ibus_prop_list_append(scheme_menu, korean);
  ibus_prop_list_append(scheme_menu, vietnamese);
  ibus_prop_list_append(scheme_menu, tibetan);
  // The input languages and the Chinese schemes are two radio groups; without the rule ibus-ui-gtk3 joins them and marks only one of the two checked entries.
  ibus_prop_list_append(scheme_menu, menu_separator("Scheme/Separator"));
  // Cantonese and Zhuyin are offered only when their dictionary is installed: host-api would fall back from either without it.
  // 五笔一项跟随存储的码表版本显示「86 五笔」或「98 五笔」。
  const char *wubi_label = msime::linux_host::wubi_scheme_label(
      configured.at("preferences").value("wubi_profile", std::string("wubi86")));
  for (const auto &[value, name, label] : {std::tuple{"quanpin", "Scheme/Quanpin", "全拼"},
                                           std::tuple{"shuangpin", "Scheme/Shuangpin", "双拼"},
                                           std::tuple{"wubi", "Scheme/Wubi", wubi_label},
                                           std::tuple{"cantonese", "Scheme/Cantonese", "粤拼"},
                                           std::tuple{"zhuyin", "Scheme/Zhuyin", "注音"}}) {
    if (!msime::linux_host::input_scheme_available(value, configured_dictionaries)) continue;
    auto item = ibus_property_new(
        name, PROP_TYPE_RADIO,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("直接选择中文输入方案"), !menu_save_pending, TRUE,
        chinese_scheme && active_scheme == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_prop_list_append(scheme_menu, item);
  }
  ibus_property_set_sub_props(scheme, scheme_menu);
  auto profile = ibus_property_new(
      "ShuangpinProfile", PROP_TYPE_MENU,
      ibus_text_new_from_static_string("双拼方案"), "",
      ibus_text_new_from_static_string("选择双拼键位方案"),
      s.focused && !s.blocked && !menu_save_pending, TRUE, PROP_STATE_UNCHECKED, nullptr);
  auto profile_menu = ibus_prop_list_new();
  const auto configured_profile = s.shuangpin_profile_override.value_or(
      configured.at("preferences").value("shuangpin_profile", "xiaohe"));
  for (const auto &[value, label] : msime::linux_host::kShuangpinProfileNames) {
    auto item = ibus_property_new(
        (std::string("ShuangpinProfile/") + value).c_str(), PROP_TYPE_RADIO,
        ibus_text_new_from_static_string(label), "",
        ibus_text_new_from_static_string("切换双拼键位方案"), !menu_save_pending, TRUE,
        configured_profile == value ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_prop_list_append(profile_menu, item);
  }
  ibus_property_set_sub_props(profile, profile_menu);
  // The design menu: 中文/英文; 全角/标点/译文; 输入方案; 主题/词库…/设置…/关于, then the tools that depend on the moment (voice, candidate actions, nine-key spellings, clipboard history) and the three option groups holding every other switch. 中文/英文 is the one InputMode toggle Shift flips, labelled 中文 and checked while letters compose; the Engine's dedicated English mode (EnglishMode, Ctrl+Shift+E) is a different feature and sits in 输入选项 beside 英文候选. Nesting keeps each key, so activation and the by-key updates below do not change.
  std::vector<IBusProperty *> design_panel_actions;
  design_panel_actions.reserve(3);
  for (const auto &action : desktop_panel_actions)
    if (action.design_menu) design_panel_actions.push_back(desktop_panel_property(engine, action));
  if (registration) {
    auto properties = ibus_prop_list_new();
    ibus_prop_list_append(properties, property);
    ibus_prop_list_append(properties, menu_separator("Separator/Mode"));
    ibus_prop_list_append(properties, character_mode);
    ibus_prop_list_append(properties, punctuation);
    ibus_prop_list_append(properties, translations);
    ibus_prop_list_append(properties, menu_separator("Separator/Switches"));
    ibus_prop_list_append(properties, scheme);
    ibus_prop_list_append(properties, menu_separator("Separator/Scheme"));
    ibus_prop_list_append(properties, global_theme_property);
    for (auto *action : design_panel_actions) ibus_prop_list_append(properties, action);
    ibus_prop_list_append(properties, menu_separator("Separator/Design"));
    ibus_prop_list_append(properties, voice);
    ibus_prop_list_append(properties, voice_cancel_property);
    ibus_prop_list_append(properties, candidate_actions(engine));
    ibus_prop_list_append(properties, nine_key_spellings_property);
    ibus_prop_list_append(properties, clipboard);
    ibus_prop_list_append(properties, menu_group(
        "Group/Input", "输入选项", "方案细节、混合候选与按键选项",
        {profile, helpcode_property, helpcode_schema, traditional, english, english_mode, emoji, kaomoji, cloud,
         nine_key_property, number_row_property, word_character_property, local_modes_property}));
    ibus_prop_list_append(properties, menu_group(
        "Group/Punctuation", "标点与翻译", "标点细节与候选翻译",
        {smart_punctuation, smart_repeat, paired, punctuation_lock, menu_separator("Group/Punctuation/Separator"),
         sentence_translation, translation_language}));
    ibus_prop_list_append(properties, menu_group(
        "Group/Candidate", "候选与词频", "候选窗口、编码显示与词频学习",
        {layout_property, page_size_property, theme_property, preedit_property, shuangpin_preedit_property,
         wubi_code_hint_property, menu_separator("Group/Candidate/Separator"), learning_property,
         frequency_property, frequency_trigger_property, frequency_step_property}));
    ibus_prop_list_append(properties, toolbar);
    ibus_prop_list_append(properties, desktop_tools_property(engine));
    ibus_engine_register_properties(engine, properties);
  } else {
    ibus_engine_update_property(engine, property);
    ibus_engine_update_property(engine, english_mode);
    ibus_engine_update_property(engine, character_mode);
    ibus_engine_update_property(engine, punctuation);
    ibus_engine_update_property(engine, translations);
    update_menu_property(engine, scheme);
    update_menu_property(engine, global_theme_property);
    for (auto *action : design_panel_actions) ibus_engine_update_property(engine, action);
    ibus_engine_update_property(engine, voice);
    ibus_engine_update_property(engine, voice_cancel_property);
    ibus_engine_update_property(engine, candidate_actions(engine));
    ibus_engine_update_property(engine, nine_key_spellings_property);
    ibus_engine_update_property(engine, clipboard);
    ibus_engine_update_property(engine, profile);
    ibus_engine_update_property(engine, helpcode_property);
    ibus_engine_update_property(engine, helpcode_schema);
    ibus_engine_update_property(engine, traditional);
    ibus_engine_update_property(engine, english);
    ibus_engine_update_property(engine, emoji);
    ibus_engine_update_property(engine, kaomoji);
    ibus_engine_update_property(engine, cloud);
    ibus_engine_update_property(engine, nine_key_property);
    ibus_engine_update_property(engine, number_row_property);
    ibus_engine_update_property(engine, word_character_property);
    ibus_engine_update_property(engine, local_modes_property);
    ibus_engine_update_property(engine, smart_punctuation);
    ibus_engine_update_property(engine, smart_repeat);
    ibus_engine_update_property(engine, paired);
    ibus_engine_update_property(engine, punctuation_lock);
    ibus_engine_update_property(engine, sentence_translation);
    ibus_engine_update_property(engine, translation_language);
    ibus_engine_update_property(engine, layout_property);
    ibus_engine_update_property(engine, page_size_property);
    ibus_engine_update_property(engine, theme_property);
    ibus_engine_update_property(engine, preedit_property);
    ibus_engine_update_property(engine, shuangpin_preedit_property);
    ibus_engine_update_property(engine, wubi_code_hint_property);
    ibus_engine_update_property(engine, learning_property);
    ibus_engine_update_property(engine, frequency_property);
    ibus_engine_update_property(engine, frequency_trigger_property);
    ibus_engine_update_property(engine, frequency_step_property);
    ibus_engine_update_property(engine, toolbar);
    ibus_engine_update_property(engine, desktop_tools_property(engine));
  }
}
struct CandidateHideRequest {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
  uint64_t serial;
};

gboolean apply_candidate_hide(gpointer data) {
  auto *request = static_cast<CandidateHideRequest *>(data);
  if (!request->alive->load())
    return G_SOURCE_REMOVE;
  auto &s = state(request->engine);
  if (s.candidate_hide_serial != request->serial)
    return G_SOURCE_REMOVE;
  s.candidate_hide_source = 0;
  ibus_engine_hide_lookup_table(request->engine);
  ibus_engine_hide_auxiliary_text(request->engine);
  s.rendered_view = nullptr;
  s.rendered_candidates = Json::array();
  s.rendered_scheme = 255;
  s.rendered_session = 0;
  publish_candidate_properties(request->engine);
  return G_SOURCE_REMOVE;
}

void destroy_candidate_hide_request(gpointer data) {
  auto *request = static_cast<CandidateHideRequest *>(data);
  g_object_unref(request->engine);
  delete request;
}

void cancel_candidate_hide(IBusEngine *engine) {
  auto &s = state(engine);
  ++s.candidate_hide_serial;
  if (s.candidate_hide_source) {
    const auto source = s.candidate_hide_source;
    s.candidate_hide_source = 0;
    g_source_remove(source);
  }
}

void schedule_candidate_hide(IBusEngine *engine) {
  auto &s = state(engine);
  cancel_candidate_hide(engine);
  const auto serial = s.candidate_hide_serial;
  auto *request = new CandidateHideRequest{
      IBUS_ENGINE(g_object_ref(engine)), s.alive, serial};
  s.candidate_hide_source = g_timeout_add_full(
      G_PRIORITY_DEFAULT, 24, apply_candidate_hide, request,
      destroy_candidate_hide_request);
}

void clear(IBusEngine *engine) {
  cancel_candidate_hide(engine);
  auto &s = state(engine);
  if (s.translation_candidates_active && s.translation_saved_view.is_object())
    s.view = std::move(s.translation_saved_view);
  s.translation_candidates_active = false;
  s.translation_saved_view = nullptr;
  s.translation_options.clear();
  s.translation_page = 0;
  s.translation_cursor = 0;
  ibus_engine_update_preedit_text_with_mode(
      engine, ibus_text_new_from_static_string(""), 0, FALSE,
      IBUS_ENGINE_PREEDIT_CLEAR);
  ibus_engine_hide_lookup_table(engine);
  ibus_engine_hide_auxiliary_text(engine);
  s.rendered_view = nullptr;
  s.rendered_candidates = Json::array();
  s.rendered_scheme = 255;
  s.rendered_session = 0;
  publish_candidate_properties(engine);
}
// Windows re-resolves the punctuation state on every Chinese/English switch: under the "follow" lock it tracks the mode (Chinese punctuation in Chinese mode, ASCII in English), and a pinned lock keeps its value. The switch supersedes a Ctrl+. choice, so the session override is dropped and the saved preference is the authority again on the next focus or refresh; the preference file itself is not written. Call after open(), because opening a session re-derives chinese_punctuation from the preferences.
void resync_punctuation_for_mode(IBusEngine *engine) {
  auto &s = state(engine);
  s.english_punctuation = {};
  s.english_chinese_punctuation = false;
  if (s.punctuation_lock != "follow")
    return;
  s.punctuation_override.reset();
  s.chinese_punctuation = s.input_enabled;
  if (s.session && s.session_chinese_punctuation != s.chinese_punctuation) {
    s.view = response(
        msime_client_set_chinese_punctuation(s.session, s.chinese_punctuation));
    s.session_chinese_punctuation = s.chinese_punctuation;
  }
}
void sync_global_input_mode(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.mode_scope_global || !global_input_enabled ||
      s.input_enabled == *global_input_enabled)
    return;
  if (!*global_input_enabled && s.voice_active)
    voice_cancel(engine);
  s.invalidate_providers();
  if (!*global_input_enabled && s.session)
    apply(engine, msime_client_command(s.session, MSIME_COMMIT_RAW));
  s.input_enabled = *global_input_enabled;
  s.open();
  resync_punctuation_for_mode(engine);
  if (s.session)
    apply(engine, msime_client_focus(s.session, s.input_enabled));
  clear(engine);
  publish_mode(engine);
}
[[maybe_unused]] void publish_input_enabled(IBusEngine *engine, bool enabled) {
  auto property = ibus_property_new(
      "InputEnabled", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("输入启用"), "",
      ibus_text_new_from_static_string("启用或停用当前 Linux 输入会话"), TRUE,
      TRUE, enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  ibus_engine_update_property(engine, property);
}
[[maybe_unused]] void publish_punctuation(IBusEngine *engine, bool enabled) {
  auto property = ibus_property_new(
      "ChinesePunctuation", PROP_TYPE_TOGGLE,
      ibus_text_new_from_static_string("中文标点"), "",
      ibus_text_new_from_static_string("启用中文标点转换"), TRUE, TRUE,
      enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  ibus_engine_update_property(engine, property);
}
[[maybe_unused]] void publish_character_width(IBusEngine *engine, bool fullwidth) {
  auto property = ibus_property_new(
      "CharacterWidth", PROP_TYPE_TOGGLE, ibus_text_new_from_static_string("全角字符"), "",
      ibus_text_new_from_static_string("切换 ASCII 字符的全角/半角输出"), TRUE, TRUE,
      fullwidth ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED, nullptr);
  ibus_engine_update_property(engine, property);
}
[[maybe_unused]] void publish_expressive(IBusEngine *engine, const State &s) {
  const auto preferences = configured.at("preferences").value("mixed_input", Json::object());
  const auto value = [&](const std::optional<bool> &override_value,
                         const char *key, bool fallback) {
    return override_value.value_or(preferences.value(key, fallback));
  };
  for (const auto &[name, label, key, fallback] : {
           std::tuple<const char *, const char *, const char *, bool>{
               "EnglishCandidates", "英文候选", "english", true},
           {"EmojiCandidates", "Emoji候选", "emoji", true},
           {"KaomojiCandidates", "颜文字候选", "kaomoji", false}}) {
    const auto &override_value = std::string(key) == "english"
                                     ? s.english_override
                                     : std::string(key) == "emoji"
                                           ? s.emoji_override
                                           : s.kaomoji_override;
    auto property = ibus_property_new(
        name, PROP_TYPE_TOGGLE, ibus_text_new_from_string(label), "",
        ibus_text_new_from_static_string("在中文方案中补充表达候选"), !menu_save_pending, TRUE,
        value(override_value, key, fallback) ? PROP_STATE_CHECKED
                                              : PROP_STATE_UNCHECKED,
        nullptr);
    ibus_engine_update_property(engine, property);
    }
}
// A composition is drawn single-underlined, the IBus convention and what the Fcitx5 host does with TextFormatFlag::Underline; it is this platform's form of the dotted TF_LS_DOT attribute Windows gives its composition. Empty text and the clear paths carry no attribute.
void underline_preedit(IBusText *text, guint length) {
  if (length == 0)
    return;
  ibus_text_append_attribute(text, IBUS_ATTR_TYPE_UNDERLINE,
                             IBUS_ATTR_UNDERLINE_SINGLE, 0,
                             static_cast<gint>(length));
}
// The aux line above a candidate page: the page number, the reading when the candidate preedit shows it, the local mode, and the typing combo while there is one.
std::string candidate_aux_text(IBusEngine *engine, const Json &view) {
  auto paging = std::to_string(view.at("page").get<size_t>() + 1) + "/" +
                std::to_string(view.at("page_count").get<size_t>());
  if (!state(engine).show_candidate_page_number) paging.clear();
  if (state(engine).candidate_preedit_style == "pinyin") {
    const auto candidate_preedit = view.value("preedit", std::string{});
    if (!candidate_preedit.empty()) {
      if (!paging.empty()) paging += "  · ";
      const auto editing_text = view.value("editing_text", std::string{});
      const auto caret = view.value("caret_position", editing_text.size());
      paging += msime::linux_host::candidate_preedit_with_caret(
          candidate_preedit, editing_text, caret);
    }
  }
  const auto mode = view.at("local_mode").get<std::string>();
  if (const char *label = msime::linux_host::candidate_local_mode_label(mode)) {
    if (!paging.empty()) paging += "  · ";
    paging += label;
  }
  const auto combo = msime::linux_host::typing_combo_label(state(engine).typing_combo);
  if (!combo.empty()) {
    if (!paging.empty()) paging += "  · ";
    paging += combo;
  }
  return paging;
}
void render(IBusEngine *engine, const Json &view) {
  cancel_candidate_hide(engine);
  // Engine caret offsets refer to ASCII editing_text, never the display
  // preedit.
  const auto style = state(engine).preedit_style;
  if (state(engine).voice_active) {
    auto &s = state(engine);
    const auto voice_length =
        static_cast<guint>(g_utf8_strlen(s.voice_preedit.c_str(), -1));
    auto voice_text = ibus_text_new_from_string(s.voice_preedit.c_str());
    underline_preedit(voice_text, voice_length);
    ibus_engine_update_preedit_text_with_mode(
        engine, voice_text, voice_length, !s.voice_preedit.empty(),
        IBUS_ENGINE_PREEDIT_CLEAR);
    ibus_engine_hide_lookup_table(engine);
    s.rendered_candidates = Json::array();
    s.rendered_scheme = 255;
    s.rendered_session = 0;
    s.rendered_view = nullptr;
    publish_candidate_properties(engine);
    s.wave_overlay.status = s.voice_phase;
    s.wave_overlay.locked = s.voice_space_locked && !s.voice_stopping;
    s.wave_overlay.listening = !s.voice_stopping && s.voice_level.has_value();
    if (s.wave_overlay_surface) {
      if (s.wave_overlay_visible)
        s.wave_overlay_surface->update(s.wave_overlay);
      else
        s.wave_overlay_visible = s.wave_overlay_surface->show(s.wave_overlay);
    }
    return;
  }
  if (state(engine).wave_overlay_surface && state(engine).wave_overlay_visible) {
    state(engine).wave_overlay_surface->hide();
    state(engine).wave_overlay_visible = false;
  }
  // 韩文、注音、越南文或藏文的组字是用户已经写下的文字，所以不论预编辑样式如何，都按拼音样式内嵌显示，光标在末尾：在候选列表打开之前没有候选窗可以显示它。客户端失焦时 IBus 会自己上屏 COMMIT 模式的预编辑，打开的组字就是这样到达被离开的客户端的（见 focus_out）。
  const int rules_scheme = msime::linux_host::scheme_rules(view);
  const bool always_inline = rules_scheme >= 0 && msime::linux_host::scheme::AlwaysInlinePreedit(rules_scheme);
  auto text = style == "pinyin" || always_inline ? view.at("preedit").get<std::string>()
                                                 : view.at("editing_text").get<std::string>();
  auto caret = view.at("caret_position").get<size_t>();
  // The check is about the letters the Engine produced, so it runs before the kana replace them
  // below; run after, it rejected every Japanese composition in the raw style. An inline composition is not the raw letters (a Vietnamese word is not ASCII).
  if (style == "raw" && !always_inline && (caret > text.size() ||
      std::any_of(text.begin(), text.end(),
                  [](unsigned char c) { return c < 0x20 || c > 0x7e; })))
    throw std::runtime_error("Invalid editing text");
  // A Japanese composition is かな, not the letters that produced it; see PhrasePreedit.h for the
  // one case that keeps the letters.
  const auto reading = view.value("reading", std::string{});
  if (msime::linux_host::composition_shows_reading(
          reading, caret, view.value("editing_text", std::string{}).size())) {
    text = reading;
    caret = reading.size();
  }
  // A phrase being assembled leads the reading, exactly as the reference draws
  // `word_for_creating_word`, so the piece the user has already picked is on screen instead of
  // being committed into the document a fragment at a time. It is prepended after the check above,
  // which is about the reading the Engine produced: the piece is Han text and asking it to be
  // printable ASCII would reject every phrase.
  const auto composed = msime::linux_host::compose_phrase_preedit(
      view.value("phrase_prefix", std::string{}), text, caret);
  text = composed.text;
  // IBus counts the cursor in Unicode scalars, and the piece is not ASCII.
  auto preedit_text = ibus_text_new_from_string(text.c_str());
  if (style != "empty" || always_inline)
    underline_preedit(preedit_text, static_cast<guint>(
                                        msime::linux_host::utf8_scalar_count(text)));
  ibus_engine_update_preedit_text_with_mode(
      engine, preedit_text,
      static_cast<guint>(style == "raw" && !always_inline
                             ? composed.caret_scalars
                             : msime::linux_host::utf8_scalar_count(text)),
      (style != "empty" || always_inline) && !text.empty(),
      always_inline ? IBUS_ENGINE_PREEDIT_COMMIT : IBUS_ENGINE_PREEDIT_CLEAR);
  const auto &candidates = view.at("candidates");
  if (candidates.empty()) {
    auto &s = state(engine);
    const bool had_candidates = s.rendered_candidates.is_array() &&
                                !s.rendered_candidates.empty();
    ibus_engine_hide_auxiliary_text(engine);
    s.rendered_candidates = Json::array();
    s.rendered_scheme = 255;
    s.rendered_session = 0;
    s.rendered_view = nullptr;
    publish_candidate_properties(engine);
    if (had_candidates)
      schedule_candidate_hide(engine);
    else
      ibus_engine_hide_lookup_table(engine);
    return;
  }
  const auto auxiliary = candidate_aux_text(engine, view);
  ibus_engine_update_auxiliary_text(
      engine, ibus_text_new_from_string(auxiliary.c_str()), !auxiliary.empty());
  auto table = ibus_lookup_table_new(static_cast<guint>(candidates.size()), 0,
                                     TRUE, FALSE);
  ibus_lookup_table_set_orientation(table, state(engine).candidate_orientation);
  for (size_t index = 0; index < candidates.size(); ++index) {
    const auto &candidate = candidates.at(index);
    auto value = candidate.at("text").get<std::string>();
    if (candidate.value("corrected", false))
      value += "*";
    // The gloss is kept apart until the row is converted, so its character range is known for a skin's translation colour.
    std::string gloss;
    const auto &engine_state = state(engine);
    const bool show_translations =
        engine_state.candidate_translations ||
        (engine_state.candidate_english_gloss &&
         engine_state.translation_target_language == "en") ||
        candidate.value("id", Json::object()).value("generation", uint64_t{0}) ==
            engine_state.sentence_translation_generation;
    // A Hanja row's 훈음 opens the gloss whatever the translation settings say, so it reads as the row's secondary line rather than as part of the candidate; a translation follows it. IBus has no second line, and the gloss is display text only: the row is chosen by index.
    const auto hanja_gloss = msime::linux_host::korean_hanja_gloss(view, candidate);
    if (!hanja_gloss.empty() && hanja_gloss.size() <= 4096 && value.size() <= 4096)
      gloss = " · " + hanja_gloss;
    if (show_translations && !engine_state.translation_reset_pending &&
        candidate.contains("translation") && !candidate.at("translation").is_null()) {
      auto translation = candidate.at("translation").get<std::string>();
      // IBus lookup rows are plain text; preserve the candidate and expose the optional gloss without allowing an oversized provider result to destabilize the panel.
      if (!translation.empty() && translation.size() <= 4096 &&
          value.size() <= 4096)
        gloss += " · " + translation;
    }
    std::string tail;
    switch (candidate.value("source", 0)) {
    case 2: tail += "  云"; break;
    case 3: tail += "  AI"; break;
    default: break;
    }
    const auto fixed_position = candidate.value("fixed_position", 0);
    if (fixed_position >= 1 && fixed_position <= 5)
      tail += "  固定" + std::to_string(fixed_position);
    const auto annotation = candidate.value("annotation", std::string{});
    const bool wubi_annotation = view.value("scheme", 255) != 2 ||
                                 state(engine).wubi_code_hint;
    // A Hanja row's annotation is its 훈음, already drawn in the gloss above.
    if (!annotation.empty() && hanja_gloss.empty() &&
        state(engine).show_helpcode_in_candidate_window && wubi_annotation) {
      tail += "  ";
      tail += annotation;
    }
    // Converted piece by piece: the separators already split them, and the gloss's range then stays exact whatever the conversion does to lengths.
    value = traditional_display(state(engine), view, std::move(value));
    const auto gloss_start = static_cast<guint>(g_utf8_strlen(value.c_str(), -1));
    if (!gloss.empty())
      value += traditional_display(state(engine), view, std::move(gloss));
    const auto gloss_end = static_cast<guint>(g_utf8_strlen(value.c_str(), -1));
    if (!tail.empty())
      value += traditional_display(state(engine), view, std::move(tail));
    auto text = ibus_text_new_from_string(value.c_str());
    const bool highlighted = candidate.at("highlighted").get<bool>();
    const auto row_text_color =
        highlighted && state(engine).candidate_selected_text_color
            ? state(engine).candidate_selected_text_color
            : state(engine).candidate_text_color;
    // Windows lets the selected-row text colour win for every candidate;
    // fixed entries use the accent only while they are not highlighted.
    if (highlighted && row_text_color)
      ibus_text_append_attribute(text, IBUS_ATTR_TYPE_FOREGROUND,
                                 *row_text_color, 0, G_MAXUINT);
    else if (fixed_position > 0 && state(engine).candidate_accent_color)
      ibus_text_append_attribute(
          text, IBUS_ATTR_TYPE_FOREGROUND,
          *state(engine).candidate_accent_color, 0, G_MAXUINT);
    else if (row_text_color)
      ibus_text_append_attribute(text, IBUS_ATTR_TYPE_FOREGROUND,
                                 *row_text_color, 0, G_MAXUINT);
    // A skin's translation colour over the gloss, appended after the row colour so it wins where they overlap. Whether the desktop panel honours a ranged attribute is up to the panel.
    if (gloss_end > gloss_start && state(engine).candidate_translation_color)
      ibus_text_append_attribute(text, IBUS_ATTR_TYPE_FOREGROUND,
                                 *state(engine).candidate_translation_color,
                                 gloss_start, gloss_end);
    const auto row_background =
        highlighted && state(engine).candidate_selected_color
            ? state(engine).candidate_selected_color
            : state(engine).candidate_background_color;
    if (row_background)
      ibus_text_append_attribute(
          text, IBUS_ATTR_TYPE_BACKGROUND, *row_background, 0, G_MAXUINT);
    ibus_lookup_table_append_candidate(table, text);
    auto label = std::to_string(index + 1);
    auto label_text = ibus_text_new_from_string(label.c_str());
    const auto row_number_color =
        highlighted && state(engine).candidate_selected_number_color
            ? state(engine).candidate_selected_number_color
            : state(engine).candidate_number_color;
    if (row_number_color)
      ibus_text_append_attribute(label_text, IBUS_ATTR_TYPE_FOREGROUND,
                                 *row_number_color, 0, G_MAXUINT);
    if (row_background)
      ibus_text_append_attribute(label_text, IBUS_ATTR_TYPE_BACKGROUND,
                                 *row_background, 0, G_MAXUINT);
    ibus_lookup_table_append_label(table, label_text);
    if (highlighted)
      ibus_lookup_table_set_cursor_pos(table, static_cast<guint>(index));
  }
  ibus_engine_update_lookup_table(engine, table, TRUE);
  auto &s = state(engine);
  s.rendered_view = view;
  s.rendered_candidates = candidates;
  s.rendered_scheme = view.value("scheme", 255);
  s.rendered_session = s.session;
  schedule_candidate_properties(engine);
}
void render_translation_candidates(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.translation_candidates_active || !s.translation_saved_view.is_object() ||
      s.translation_options.empty())
    return;
  const auto saved_page_size = s.translation_saved_view.value("page_size", size_t{9});
  const auto page_size = std::clamp(saved_page_size, size_t{1}, size_t{9});
  const auto page_count = (s.translation_options.size() + page_size - 1) / page_size;
  s.translation_page = std::min(s.translation_page, page_count - 1);
  const auto start = s.translation_page * page_size;
  s.translation_cursor = std::min(s.translation_cursor,
                                  s.translation_options.size() - start - 1);
  auto overlay = s.translation_saved_view;
  overlay["page"] = s.translation_page;
  overlay["page_size"] = page_size;
  overlay["page_count"] = page_count;
  overlay["candidates"] = Json::array();
  const auto end = std::min(start + page_size, s.translation_options.size());
  for (size_t index = start; index < end; ++index) {
    Json candidate = Json::object();
    candidate["text"] = s.translation_options.at(index);
    candidate["highlighted"] = index - start == s.translation_cursor;
    candidate["source"] = 5;
    candidate["fixed_position"] = 0;
    candidate["annotation"] = "";
    candidate["id"] = {{"session", s.session},
                        {"generation", overlay.value("generation", uint64_t{0})},
                        {"index", index}};
    overlay["candidates"].push_back(std::move(candidate));
  }
  s.view = std::move(overlay);
  render(engine, s.view);
}
void exit_translation_candidates(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.translation_candidates_active)
    return;
  s.translation_candidates_active = false;
  if (s.translation_saved_view.is_object())
    s.view = std::move(s.translation_saved_view);
  s.translation_options.clear();
  s.translation_page = 0;
  s.translation_cursor = 0;
  render(engine, s.view);
}
bool commit_translation_candidate(IBusEngine *engine, size_t index) {
  auto &s = state(engine);
  if (!s.translation_candidates_active || index >= s.translation_options.size())
    return false;
  const auto text = s.translation_options.at(index);
  exit_translation_candidates(engine);
  commit_text(engine, text);
  (void)apply(engine, msime_client_command(s.session, MSIME_CANCEL));
  return true;
}
bool apply(IBusEngine *engine, char *raw, PunctuationPairMode pair_mode,
           std::optional<std::string> space_convert_preceding) {
  auto result = response(raw);
  const auto &commit = result.at("commit");
  if (commit.is_string()) {
    auto text = commit.get<std::string>();
    auto &s = state(engine);
    const auto context = result.value("commit_context", Json(nullptr));
    text = traditional_display(s, context, std::move(text));
    const auto space_convert_ascii =
        space_convert_preceding
            ? msime::linux_host::smart_punctuation_ascii_mark(text)
            : 0;
    const auto space_convert_mark =
        space_convert_ascii == 0 ? std::string{} : text;
    const bool inserted_pair = normalize_punctuation_pair(text, pair_mode);
    if (s.smart_punctuation && text.size() == 1 &&
        smart_punctuation_pair(text.front())) {
      s.last_smart_punctuation = text.front();
      s.last_smart_punctuation_time = g_get_monotonic_time();
    } else if (text.size() != 1 || !smart_punctuation_pair(text.front())) {
      s.last_smart_punctuation = 0;
      s.last_smart_punctuation_time = 0;
    }
    // 韩文、越南文和藏文上屏的是谚文、拉丁字母或藏文旁边的半角 ASCII 标点，运行时已经不转换它们，宿主也不能把它们变成全角。专用英文模式在每个方案里都保持自己的规则，所以它的上屏照常变成全角。
    const auto &commit_context = result.contains("commit_context") ? result.at("commit_context") : Json(nullptr);
    const auto narrow_scheme = [](int scheme) {
      return scheme >= 0 && scheme < static_cast<int>(msime::linux_host::kInputSchemeIds.size()) &&
             !msime::linux_host::scheme::WidensFullWidth(scheme);
    };
    const bool narrow_commit =
        ((commit_context.is_object() && narrow_scheme(commit_context.value("scheme", 0))) ||
         narrow_scheme(result.at("view").value("scheme", 0))) &&
        !result.at("view").value("dedicated_english", false);
    if (state(engine).fullwidth && !narrow_commit)
      text = fullwidth_text(text);
    if (!text.empty()) {
      commit_text(engine, text, std::nullopt,
                  !context.is_object() || context.value("typing_statistics", true));
      // The key sound played when the key went down; this is the commit's own sound, or the next note of a melody that advances on commits. A transition only commits for a key or click in this focused, non-secure field.
      if (!s.private_input) {
        msime_client_commit_sound(s.session);
        // The commit counts nothing; it reports the combo as it stands, which is how one that lapsed while the user paused leaves the aux line drawn just below.
        s.typing_combo = msime::linux_host::typing_effect_combo(
            msime_client_typing_effect(s.session, msime::linux_host::kTypingEffectCommit));
      }
      if (space_convert_ascii != 0 && !inserted_pair) {
        // Preserve Engine's actual half (notably opening/closing quotes).
        s.space_convert_mark = space_convert_mark;
        s.space_convert_preceding = std::move(*space_convert_preceding);
      }
      if (inserted_pair) {
        if (const auto closing = paired_closing_from_text(text))
          s.paired_tracker.push(*closing);
      }
    }
  }
  state(engine).view = result.at("view");
  render(engine, state(engine).view);
  online_schedule(engine);
  translation_schedule(engine);
  settled_rerank_schedule(engine);
  return result.at("handled").get<bool>();
}
template <class F> void guarded(IBusEngine *engine, const char *operation, F action) noexcept {
  try {
    action();
  } catch (...) {
    // Never log the raw error or response: either can include input or paths.
    g_warning("MSIME preview host operation failed: %s", operation);
    msime_linux_diagnostic_write(
        std::string("operation_failed operation=") + operation);
    state(engine).close();
    clear(engine);
    publish_mode(engine);
  }
}
struct VoiceResult {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
  uint64_t generation;
  uint64_t focus_epoch;
  uint64_t session;
  std::string text;
  bool final = true;
  unsigned level = 0;
  bool provider_failed = false;
  bool inline_preedit = false;
  std::string provider_error{};
};
struct VoiceFailureNotice {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
  uint64_t id;
};
struct VoiceStreamContext {
  MsimeVoiceWorker::Progress progress;
  std::function<void(uint8_t)> status;
  std::function<void(float)> level;
};
extern "C" void voice_provider_level_update(float level, void *context) {
  auto *stream = static_cast<VoiceStreamContext *>(context);
  try {
    if (stream && stream->level && level >= 0.0f && level <= 1.0f) stream->level(level);
  } catch (...) {}
}
extern "C" void voice_provider_status_update(uint8_t phase, void *context) {
  auto *stream = static_cast<VoiceStreamContext *>(context);
  try {
    if (stream && stream->status && phase <= 2) stream->status(phase);
  } catch (...) {}
}
extern "C" void voice_provider_stream_update(const uint8_t *text,
                                               size_t length, bool final,
                                               void *context) {
  auto *stream = static_cast<VoiceStreamContext *>(context);
  if (!stream || !stream->progress || !text || length == 0 || length > 4096)
    return;
  try {
    stream->progress(msime_voice_bound_result(
                         std::string(reinterpret_cast<const char *>(text), length)),
                     final);
  } catch (...) {
    // Provider callbacks cross a C ABI; allocation or queue failures must not
    // escape into the provider worker and terminate the input method.
  }
}
// English mode has no Engine session to issue a voice generation, so the host numbers those recordings itself. The top bit keeps them apart from Engine generations, which count up from 1 in every session, so a late callback of one kind can never match a recording of the other.
uint64_t next_sessionless_voice_generation() {
  static uint64_t counter = 0;
  return (uint64_t{1} << 63) | (++counter & ~(uint64_t{1} << 63));
}
// A callback still belongs to the recording on screen. A recording bound to an Engine session also ends with that session; one started in English mode has no session, and its host-issued generation is enough. The input mode is deliberately not checked: switching between Chinese and English does not end a recording, as on Windows.
bool voice_result_current(const State &s, uint64_t generation, uint64_t session,
                          uint64_t focus_epoch) {
  return s.voice_active && s.voice_generation == generation &&
         (session == 0 || s.session == session) &&
         s.focus_epoch == focus_epoch && s.focused && !s.blocked;
}
// A recording can end in English mode with no Engine view to draw; then the voice preedit and the wave overlay have to come down here, since clear() does not touch the overlay the way render() does.
void render_after_voice(IBusEngine *engine) {
  auto &s = state(engine);
  if (s.session) {
    render(engine, s.view);
    return;
  }
  if (s.wave_overlay_surface && s.wave_overlay_visible) {
    s.wave_overlay_surface->hide();
    s.wave_overlay_visible = false;
  }
  s.wave_overlay.reset();
  clear(engine);
}
// 由方案决定繁体输出转换是否适用。没有 Engine 视图时（英文模式）取 Engine 将要运行的方案，所以日文、韩文、粤拼、注音、越南文和藏文的文字仍然不被改动。
Json voice_commit_context(const State &s) {
  if (s.view.is_object())
    return Json{{"scheme", s.view.value("scheme", 0)}, {"local_mode", "none"}};
  return Json{{"scheme", std::max(0, msime::linux_host::scheme_number(effective_scheme(s)))},
              {"local_mode", "none"}};
}
void voice_cancel(IBusEngine *engine) {
  auto &s = state(engine);
  const bool was_active = s.voice_active;
  if (s.voice_active && !s.voice_provider_socket.empty())
    msime_client_string_free(msime_client_voice_provider_cancel(
        reinterpret_cast<const uint8_t *>(s.voice_provider_socket.data()),
        s.voice_provider_socket.size(), s.voice_generation));
  if (s.voice_active && s.session)
    msime_client_string_free(msime_client_voice_cancel(s.session));
  s.voice_active = false;
  s.voice_stopping = false;
  s.voice_phase = "正在录音…";
  s.voice_level.reset();
  s.wave_overlay.reset();
  s.wave_overlay.actions_visible = false;
  if (s.wave_overlay_surface && s.wave_overlay_visible)
    s.wave_overlay_surface->hide();
  s.wave_overlay_visible = false;
  s.voice_generation = 0;

  s.voice_preedit.clear();
  s.voice_transcript.clear();
  s.wave_overlay.transcript.clear();
  s.voice_space_locked = false;
  s.voice_worker.cancel_async();
  if (s.session || was_active)
    render_after_voice(engine);
  publish_mode(engine);
}
void show_voice_failure(IBusEngine *engine, const char *message) {
  auto &s = state(engine);
  ++s.voice_failure_id;
  if (s.voice_failure_id == 0)
    ++s.voice_failure_id;
  s.wave_overlay.reset();
  s.wave_overlay.status = message;
  s.wave_overlay.show_transcript = false;
  s.wave_overlay.actions_visible = false;
  s.wave_overlay.listening = false;
  if (s.wave_overlay_surface) {
    if (s.wave_overlay_visible)
      s.wave_overlay_surface->update(s.wave_overlay);
    else
      s.wave_overlay_visible = s.wave_overlay_surface->show(s.wave_overlay);
  }
  ibus_engine_update_auxiliary_text(
      engine, ibus_text_new_from_static_string(message), TRUE);
  auto *notice = new VoiceFailureNotice{engine, s.alive, s.voice_failure_id};
  g_timeout_add_full(
      G_PRIORITY_DEFAULT, 1200,
      +[](gpointer data) -> gboolean {
        std::unique_ptr<VoiceFailureNotice> notice(
            static_cast<VoiceFailureNotice *>(data));
        if (!notice->alive->load())
          return G_SOURCE_REMOVE;
        auto &s = state(notice->engine);
        if (!s.voice_active &&
            s.voice_failure_id == notice->id) {
          if (s.wave_overlay_visible && s.wave_overlay_surface) {
            s.wave_overlay_surface->hide();
            s.wave_overlay_visible = false;
            s.wave_overlay.reset();
          }
        }
        return G_SOURCE_REMOVE;
      },
      notice, nullptr);
}
void voice_stop(IBusEngine *engine) {
  auto &s = state(engine);
  if (!s.voice_active || s.voice_stopping || s.voice_provider_socket.empty())
    return;
  std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
      msime_client_voice_provider_stop(
          reinterpret_cast<const uint8_t *>(s.voice_provider_socket.data()),
          s.voice_provider_socket.size(), s.voice_generation),
      msime_client_string_free);
  bool stopped = false;
  if (owned) {
    try {
      const auto result = Json::parse(owned.get());
      stopped = result.at("ok").get<bool>() && result.at("value").get<bool>();
    } catch (...) {
      stopped = false;
    }
  }
  if (!stopped) {
    voice_cancel(engine);
    show_voice_failure(
        engine, "结束录音失败，本次语音已取消，请检查语音服务后重试");
  } else {
    s.voice_stopping = true;
    s.voice_phase = "正在识别…";
    render(engine, s.view);
    s.voice_space_locked = false;
    publish_mode(engine);
  }
}
void voice_start_impl(IBusEngine *engine) {
  auto &s = state(engine);
  // Voice input stays available in English mode, as on Windows, where it is not part of the IME's open state.
  if (!s.voice_enabled || s.voice_provider_socket.empty() ||
      !s.focused || s.blocked || s.voice_active)
    return;
  const auto provider_options = msime::linux_host::voice_provider_options(
      configured.value("preferences", Json::object()));
  // On-device recognition takes the user's dictionary words as hotwords; the worker reads them with the options sessions are opened with, which carry the dictionary paths.
  Json hotword_options;
  if (msime::linux_host::voice_wants_hotwords(provider_options) && configured.is_object()) {
    hotword_options = configured;
    hotword_options.erase("candidate_skin_catalog");
  }
  uint64_t generation = 0;
  if (s.session) {
    const auto editing_text =
        s.view.value("editing_text", std::string{});
    const auto candidates = s.view.value("candidates", Json::array());
    if (!editing_text.empty() ||
        (candidates.is_array() && !candidates.empty()))
      apply(engine, msime_client_command(s.session, MSIME_CANCEL));
    generation = response(msime_client_voice_start(s.session)).get<uint64_t>();
  } else {
    // English mode has no composition to cancel and no Engine to hand the result to; the result is committed as recognised.
    generation = next_sessionless_voice_generation();
  }
  const auto session_id = s.session;
  const auto focus_epoch = s.focus_epoch;
  s.voice_active = true;
  sync_music(engine);
  s.voice_phase = "正在录音…";
  s.voice_level.reset();
  s.wave_overlay.reset();
  s.wave_overlay.listening = true;
  s.wave_overlay.actions_visible = true;
  s.wave_overlay_visible = false;
  s.voice_stopping = false;
  s.voice_generation = generation;
  s.voice_space_locked = false;
  const auto socket = s.voice_provider_socket;
  const auto language = s.voice_language;
  // IBus commit is the only voice commit path on Linux and the settings page offers no strategy, so a stored commit_mode must not turn the inline preedit off.
  const bool stream_inline_preedit = msime_voice_stream_inline_enabled(
      provider_options.value("stream_inline_preedit", false),
      provider_options.value("asr_provider", std::string{"doubao"}), "tsf");
  const auto alive = s.alive;
  const auto provider_succeeded = std::make_shared<std::atomic_bool>(false);
  // Written by the stream task and read by the result callback, which the worker runs afterwards on the same thread.
  const auto provider_error = std::make_shared<std::string>();
  s.voice_worker.run_stream(
      [socket, language, generation, session_id, focus_epoch, engine, alive, provider_succeeded,
       provider_error, provider_options, hotword_options](const std::atomic_bool &cancelled,
                         const MsimeVoiceWorker::Progress &progress) {
        if (cancelled.load())
          return std::string{};
        const auto query = msime::linux_host::voice_query(language, generation, provider_options, hotword_options).dump();
        VoiceStreamContext stream{progress, [engine, alive, generation, session_id, focus_epoch, &cancelled](uint8_t phase) {
          if (cancelled.load()) return;
          const char *labels[] = {"正在录音…", "正在识别…", "正在润色…"};
          auto *result = new VoiceResult{engine, alive, generation, focus_epoch, session_id, labels[phase], false};
          g_idle_add_full(G_PRIORITY_DEFAULT, +[](gpointer data) -> gboolean {
            std::unique_ptr<VoiceResult> result(static_cast<VoiceResult *>(data));
            if (!result->alive->load()) return G_SOURCE_REMOVE;
            auto &s = state(result->engine);
            if (!voice_result_current(s, result->generation, result->session,
                                      result->focus_epoch))
              return G_SOURCE_REMOVE;
            if (s.voice_stopping && result->text == "正在录音…") return G_SOURCE_REMOVE;
            s.voice_phase = std::move(result->text);
            if (s.voice_phase == "正在录音…")
              s.wave_overlay.compact_status = msime::linux_host::WaveOverlayModel::CompactStatus::None;
            else if (s.voice_phase.find("识别") != std::string::npos)
              s.wave_overlay.compact_status = msime::linux_host::WaveOverlayModel::CompactStatus::Recognizing;
            else
              s.wave_overlay.compact_status = msime::linux_host::WaveOverlayModel::CompactStatus::Processing;
            if (s.voice_phase != "正在录音…") s.voice_stopping = true;
            render(result->engine, s.view);
            publish_mode(result->engine);
            return G_SOURCE_REMOVE;
          }, result, nullptr);
        }, [engine, alive, generation, session_id, focus_epoch, &cancelled](float level) {
          if (cancelled.load()) return;
          auto *result = new VoiceResult{engine, alive, generation, focus_epoch, session_id, {}, false,
              static_cast<unsigned>(level * 10.0f + 0.5f)};
          g_idle_add_full(G_PRIORITY_DEFAULT, +[](gpointer data) -> gboolean {
            std::unique_ptr<VoiceResult> result(static_cast<VoiceResult *>(data));
            if (!result->alive->load()) return G_SOURCE_REMOVE;
            auto &s = state(result->engine);
            if (s.voice_stopping ||
                !voice_result_current(s, result->generation, result->session,
                                      result->focus_epoch))
              return G_SOURCE_REMOVE;
            if (s.voice_level != result->level) {
              s.voice_level = result->level;
              s.wave_overlay.listening = true;
              s.wave_overlay.set_input_level(result->level / 10.0f);
              render(result->engine, s.view);
            }
            return G_SOURCE_REMOVE;
          }, result, nullptr);
        }};
        auto *raw = msime_client_voice_provider_stream_feedback(
            reinterpret_cast<const uint8_t *>(query.data()), query.size(),
            reinterpret_cast<const uint8_t *>(socket.data()), socket.size(),
            voice_provider_stream_update, voice_provider_status_update, voice_provider_level_update, &stream);
        std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
            raw, msime_client_string_free);
        if (cancelled.load() || !raw)
          return std::string{};
        try {
          const auto document = Json::parse(raw);
          if (!document.value("ok", false)) {
            *provider_error = document.value("error", std::string{});
            return std::string{};
          }
          const auto value = document.at("value");
          if (!value.is_object())
            return std::string{};
          auto text = msime_voice_bound_result(value.value("text", std::string{}));
          provider_succeeded->store(true);
          return text;
        } catch (...) {
          return std::string{};
        }
      },
      [engine, alive, generation, session_id, focus_epoch,
       stream_inline_preedit](std::string text, bool final) {
        if (final || text.empty())
          return;
        auto *result = new VoiceResult{engine, alive, generation, focus_epoch, session_id,
                                       std::move(text), false};
        result->inline_preedit = stream_inline_preedit;
        g_idle_add_full(
            G_PRIORITY_DEFAULT,
            +[](gpointer data) -> gboolean {
              std::unique_ptr<VoiceResult> result(static_cast<VoiceResult *>(data));
              if (!result->alive->load())
                return G_SOURCE_REMOVE;
              auto &s = state(result->engine);
              if (!voice_result_current(s, result->generation, result->session,
                                        result->focus_epoch))
                return G_SOURCE_REMOVE;
              auto text = msime_voice_bound_result(std::move(result->text));
              if (result->inline_preedit) {
                s.voice_preedit = std::move(text);
                s.voice_transcript.clear();
                s.wave_overlay.transcript.clear();
              } else {
                s.voice_transcript = std::move(text);
                s.wave_overlay.set_transcript(s.voice_transcript);
                s.voice_preedit.clear();
              }
              render(result->engine, s.view);
              return G_SOURCE_REMOVE;
            },
            result, nullptr);
      },
      [engine, alive, generation, session_id, focus_epoch, provider_succeeded, provider_error](std::string text) {
        auto *result = new VoiceResult{engine, alive, generation, focus_epoch, session_id, std::move(text),
                                       true, 0, !provider_succeeded->load()};
        result->provider_error = *provider_error;
        g_idle_add_full(
            G_PRIORITY_DEFAULT,
            +[](gpointer data) -> gboolean {
              std::unique_ptr<VoiceResult> result(static_cast<VoiceResult *>(data));
              if (!result->alive->load())
                return G_SOURCE_REMOVE;
              auto &s = state(result->engine);
              if (!voice_result_current(s, result->generation, result->session,
                                        result->focus_epoch))
                return G_SOURCE_REMOVE;
              auto text = msime_voice_result_or_transcript(
                  std::move(result->text), s.voice_transcript, s.voice_preedit);
              try {
                if (text.empty()) {
                  msime_client_string_free(msime_client_voice_cancel(s.session));
                  s.voice_active = false;
                  s.voice_generation = 0;
                  s.voice_preedit.clear();
                  s.voice_transcript.clear();
                  s.wave_overlay.transcript.clear();
                  s.voice_space_locked = false;
                  render_after_voice(result->engine);
                  publish_mode(result->engine);
                  show_voice_failure(result->engine,
                                     result->provider_failed
                                         ? msime_voice_provider_failure_notice(result->provider_error)
                                         : "未识别到文字，请重新录音");
                  return G_SOURCE_REMOVE;
                }
                // A recording started in English mode has no Engine session to confirm its generation, so the provider text is committed as recognised; the currency check above already stands in for the Engine's.
                auto recognised = text;
                if (result->session != 0) {
                  auto applied = response(msime_client_voice_apply(
                      s.session, result->generation,
                      reinterpret_cast<const uint8_t *>(text.data()), text.size()));
                  if (!applied.is_string())
                    throw std::runtime_error("Voice result was rejected");
                  recognised = applied.get<std::string>();
                }
                s.voice_active = false;
                s.voice_generation = 0;
                s.voice_preedit.clear();
                s.voice_transcript.clear();
                s.wave_overlay.transcript.clear();
                s.voice_space_locked = false;
                auto committed = traditional_display(
                    s, voice_commit_context(s), std::move(recognised));
                if (s.fullwidth)
                  committed = fullwidth_text(std::move(committed));
                commit_text(result->engine, committed,
                            msime::linux_host::TypingSource::Voice);
                render_after_voice(result->engine);
                publish_mode(result->engine);
              } catch (...) {
                // The shared Engine route can be refused after recognition
                // (for example while focus is being torn down). Preserve the
                // transcript rather than losing the completed recording.
                try {
                  auto fallback = traditional_display(
                      s, voice_commit_context(s), std::move(text));
                  if (s.fullwidth)
                    fallback = fullwidth_text(std::move(fallback));
                  commit_text(result->engine, fallback,
                              msime::linux_host::TypingSource::Voice);
                  s.voice_active = false;
                  s.voice_generation = 0;
                  s.voice_preedit.clear();
                  s.voice_transcript.clear();
                  s.wave_overlay.transcript.clear();
                  s.voice_space_locked = false;
                  msime_client_string_free(msime_client_voice_cancel(s.session));
                  render_after_voice(result->engine);
                  publish_mode(result->engine);
                } catch (...) {
                  s.voice_active = false;
                  s.voice_generation = 0;
                  s.voice_preedit.clear();
                  s.voice_transcript.clear();
                  s.wave_overlay.transcript.clear();
                  s.voice_space_locked = false;
                  msime_client_string_free(msime_client_voice_cancel(s.session));
                  render_after_voice(result->engine);
                  publish_mode(result->engine);
                  show_voice_failure(result->engine,
                                     "语音结果处理失败，请重新录音");
                }
              }
              return G_SOURCE_REMOVE;
            },
            result, nullptr);
      });
  render(engine, s.view);
  publish_mode(engine);
}
void voice_start(IBusEngine *engine) {
  try {
    voice_start_impl(engine);
  } catch (...) {
    // Configuration and Host API errors may contain private values. Only
    // show a fixed message after dropping any partially started generation.
    voice_cancel(engine);
    show_voice_failure(
        engine, "无法启动语音输入，请检查语音设置后重试");
  }
}
// Super reaches IBus as MOD4, as the virtual SUPER bit, or as both, depending on the client (GTK3 adds SUPER next to MOD4, some clients send only one). Fold them into MOD4 so every exact modifier match sees one Super.
guint canonical_modifiers(guint flags) {
  const guint m = flags & (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK |
                           IBUS_MOD1_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK |
                           IBUS_META_MASK | IBUS_HYPER_MASK | IBUS_MOD5_MASK);
  if (m & (IBUS_MOD4_MASK | IBUS_SUPER_MASK))
    return (m & ~IBUS_SUPER_MASK) | IBUS_MOD4_MASK;
  return m;
}
// Hold shortcuts match the physical key, as the Windows hook does: X11, GDK and mutter report the modifier state from before the key, so a modifier's own bit may or may not be set on its own press and is ignored here.
bool voice_hotkey(const State &s, guint key, guint modifiers) {
  if (key == IBUS_F9 && modifiers == IBUS_CONTROL_MASK)
    return s.voice_hotkey_ctrl_f9;
  if (key == IBUS_Alt_R) {
    const guint m = modifiers & ~IBUS_MOD1_MASK;
    if (m == IBUS_CONTROL_MASK)
      return s.voice_hotkey_rctrl_ralt && s.right_ctrl_down;
    if (m == 0)
      return s.voice_hotkey_ralt;
    return false;
  }
  if (key == IBUS_Super_L || key == IBUS_Super_R) {
    const guint m = modifiers & ~(IBUS_MOD4_MASK | IBUS_SUPER_MASK);
    if (m == IBUS_CONTROL_MASK)
      return s.voice_hotkey_ctrl_win;
  }
  return false;
}
void set_surrounding(IBusEngine *engine, IBusText *text, guint cursor, guint anchor) {
  // Keep platform context available without feeding it into Engine composition.
  auto &s = state(engine);
  s.surrounding_valid = text != nullptr;
  s.surrounding_text = text && ibus_text_get_text(text) ? ibus_text_get_text(text) : "";
  s.surrounding_cursor = cursor;
  s.surrounding_anchor = anchor;
}
// The desktop panels type through this engine when it holds the focus; see PanelInputChannel.h. IBus gives every input context its own engine object, so the focused one is tracked here rather than in State.
IBusEngine *panel_input_engine = nullptr;
uint64_t panel_input_generation = 0;
msime::linux_host::PanelInputSocket panel_input_socket;
msime::linux_host::PanelInputBroker panel_input_broker;
guint panel_input_timer = 0;
gboolean process_key(IBusEngine *engine, guint key, guint keycode, guint flags);

msime::linux_host::PanelInputDelivery panel_input_deliver(
    const msime::linux_host::PanelInputRequest &request) {
  using msime::linux_host::PanelInputDelivery;
  using msime::linux_host::PanelInputRequest;
  auto *engine = panel_input_engine;
  if (!engine || !state(engine).focused) return PanelInputDelivery::NoFocus;
  if (state(engine).blocked) return PanelInputDelivery::Restricted;
  if (request.kind == PanelInputRequest::Kind::Text) {
    // Committed directly rather than through commit_text: the panel records its own typing statistics, as it does for every other route.
    ibus_engine_commit_text(engine, ibus_text_new_from_string(request.text.c_str()));
    return PanelInputDelivery::Delivered;
  }
  auto keyval = ibus_keyval_from_name(request.key.c_str());
  if (keyval == IBUS_VoidSymbol) return PanelInputDelivery::Invalid;
  guint modifiers = 0;
  if (request.shift) {
    modifiers |= IBUS_SHIFT_MASK;
    keyval = ibus_keyval_to_upper(keyval);
  }
  if (request.control) modifiers |= IBUS_CONTROL_MASK;
  if (request.alt) modifiers |= IBUS_MOD1_MASK;
  if (request.super) modifiers |= IBUS_SUPER_MASK | IBUS_MOD4_MASK;
  // The panel knows nothing of the lock; carry the one the last real key reported so this stroke does not flip the CapsLock indicator.
  if (state(engine).caps_lock) {
    modifiers |= IBUS_LOCK_MASK;
    // The panel sends letters lowercase; apply the lock the way xkb does for a physical key, so the stroke meets the CapsLock passthrough as an uppercase letter (and Shift under the lock gives lowercase).
    if (keyval < 0x80 && g_ascii_isalpha(static_cast<gchar>(keyval)))
      keyval = request.shift ? ibus_keyval_to_lower(keyval) : ibus_keyval_to_upper(keyval);
  }
  // Through this engine first, the way SendInput passes through the IME on Windows: letters compose, and digits, Space and BackSpace act on an open composition.
  msime::linux_host::deliver_panel_key_stroke(
      [&](bool release) {
        // A screen-keyboard key is a key press like a physical one, and the Fcitx5 host counts it because it arrives through the same keyEvent.
        count_key_press(engine, request.keycode, modifiers | (release ? IBUS_RELEASE_MASK : 0));
        return process_key(engine, keyval, request.keycode,
                           modifiers | (release ? IBUS_RELEASE_MASK : 0)) != FALSE;
      },
      [&](bool release) {
        ibus_engine_forward_key_event(engine, keyval, request.keycode,
                                      modifiers | (release ? IBUS_RELEASE_MASK : 0));
      });
  return PanelInputDelivery::Delivered;
}

void panel_input_pump() {
  panel_input_broker.pump(
      msime::linux_host::panel_input_monotonic_us(),
      [] {
        return msime::linux_host::PanelInputFocus{
            panel_input_engine && state(panel_input_engine).focused, panel_input_generation};
      },
      panel_input_deliver, msime::linux_host::PanelInputSocket::reply_and_close);
  if (!panel_input_broker.empty() && !panel_input_timer)
    panel_input_timer = g_timeout_add(50, [](gpointer) -> gboolean {
      panel_input_pump();
      if (!panel_input_broker.empty()) return G_SOURCE_CONTINUE;
      panel_input_timer = 0;
      return G_SOURCE_REMOVE;
    }, nullptr);
}

void panel_input_listen() {
  if (panel_input_socket.listening() ||
      !panel_input_socket.open(msime::linux_host::panel_input_socket_path()))
    return;
  g_unix_fd_add(panel_input_socket.fd(), G_IO_IN, [](gint, GIOCondition, gpointer) -> gboolean {
    if (auto accepted = panel_input_socket.accept_request()) {
      if (auto request = msime::linux_host::parse_panel_input_request(accepted->second)) {
        if (!panel_input_broker.submit(accepted->first, std::move(*request),
                                       msime::linux_host::panel_input_monotonic_us()))
          msime::linux_host::PanelInputSocket::reply_and_close(
              accepted->first, msime::linux_host::panel_input_error_reply("no_focus"));
      } else
        msime::linux_host::PanelInputSocket::reply_and_close(
            accepted->first, msime::linux_host::panel_input_error_reply("invalid"));
      panel_input_pump();
    }
    return G_SOURCE_CONTINUE;
  }, nullptr);
}

// Set once a package upgrade replaced msime-linux-ibus under this process and the host quit for it; main() turns it into msime_ibus_upgraded_exit so the launcher starts the new build at once.
bool upgrade_restart_requested = false;
bool upgrade_restart_scheduled = false;
struct UpgradeRestart {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
};
// The Windows installer stops the IME before it replaces the files and starts the new one afterwards; here the host notices on a focus change that its program was replaced and quits so the launcher starts the new build. It runs from an idle source so the focus-in that noticed it is answered first, and only while nothing is being composed or recorded, since the new process starts empty; otherwise a later focus change tries again. A removed program (the package was uninstalled) is left running: there is nothing to restart into, so quitting would only take the input method away early (the launcher does not restart a host whose program is gone).
void schedule_upgrade_restart(IBusEngine *engine) {
  if (upgrade_restart_requested || upgrade_restart_scheduled ||
      msime::linux_host::running_executable_state() !=
          msime::linux_host::ProgramFileState::Replaced)
    return;
  upgrade_restart_scheduled = true;
  g_idle_add_full(
      G_PRIORITY_DEFAULT_IDLE,
      +[](gpointer data) -> gboolean {
        upgrade_restart_scheduled = false;
        const auto *restart = static_cast<UpgradeRestart *>(data);
        if (upgrade_restart_requested || !restart->alive->load())
          return G_SOURCE_REMOVE;
        const auto &s = state(restart->engine);
        bool busy = s.translation_candidates_active || s.voice_active || s.voice_stopping;
        if (s.view.is_object()) {
          const auto editing = s.view.find("editing_text");
          const auto candidates = s.view.find("candidates");
          busy = busy ||
                 (editing != s.view.end() && editing->is_string() &&
                  !editing->get_ref<const std::string &>().empty()) ||
                 (candidates != s.view.end() && candidates->is_array() && !candidates->empty());
        }
        if (!s.focused || busy)
          return G_SOURCE_REMOVE;
        msime_linux_diagnostic_write("upgrade_restart");
        upgrade_restart_requested = true;
        ibus_quit();
        return G_SOURCE_REMOVE;
      },
      new UpgradeRestart{engine, state(engine).alive},
      +[](gpointer data) { delete static_cast<UpgradeRestart *>(data); });
}
void focus_in(IBusEngine *engine) {
  guarded(engine, "focus_in", [&] {
    auto &s = state(engine);
    const bool already_focused = s.focused;
    const auto previous_session = s.session;
    s.focused = true;
    panel_input_engine = engine;
    ++panel_input_generation;
    panel_input_listen();
    msime_linux_diagnostic_write("focus_in");
    ++s.focus_epoch;
    ibus_engine_get_surrounding_text(engine, nullptr, nullptr, nullptr);
    // Host shortcuts and presentation also apply before a runtime is needed.
    s.refresh_host_preferences(configured.at("preferences"));
    // open() only resolves provider sockets when it creates a session, so an English-mode context would otherwise wait for the reload timer before voice input is reachable.
    if (!s.session)
      s.refresh_provider_sockets(engine);
    const bool mode_before_restore = s.input_enabled;
    s.restore_app_input_mode();
    s.open();
    // Windows re-resolves punctuation on every OPENCLOSE change, including one the focus brings, so a Ctrl+. choice or override from the previous app does not outlive the switch.
    if (s.input_enabled != mode_before_restore)
      resync_punctuation_for_mode(engine);
    s.key_router.set_lease(
        {s.client_token, s.focus_epoch,
         msime::linux_host::KeyRouterAdapter::lease_token(s.client_token,
                                                           s.session)});
    watch_clipboard_history(engine);
    sync_global_input_mode(engine);
    // IBus may replay focus after negotiating client identity. Re-focusing
    // the same runtime would cancel input already typed during negotiation.
    if (s.session && (!already_focused || s.session != previous_session))
      apply(engine, msime_client_focus(s.session, s.input_enabled));
    if (!s.properties_registered &&
        g_getenv("MSIME_DISABLE_IBUS_PROPERTIES") == nullptr) {
      register_properties(engine);
      s.properties_registered = true;
    } else if (s.properties_registered &&
               (!already_focused || s.session != previous_session)) {
      // FocusOut disabled the existing menu. Refresh its state on activation
      // without re-registering it or disturbing repeated focus negotiation.
      publish_mode(engine);
    }
    // Moving into another text field shows the current 中/英 as a switch does (#2589). Only a new focus: the replay IBus sends while it negotiates the client's identity is the same focus, and showing it again would put the hint back over input the user has started. show_input_mode_hint keeps to the input_mode_hud preference and stays quiet in blocked fields.
    if (!already_focused)
      show_input_mode_hint(engine);
    sync_music(engine);
    schedule_upgrade_restart(engine);
  });
}
void focus_out(IBusEngine *engine) {
  guarded(engine, "focus_out", [&] {
    auto &s = state(engine);
    flush_key_presses(s.key_presses.take());
    s.key_presses.forget_held();
    s.remember_app_input_mode();
    msime_linux_diagnostic_write("focus_out");
    voice_cancel(engine);
    s.key_router.cancel(
        {s.client_token, s.focus_epoch,
         msime::linux_host::KeyRouterAdapter::lease_token(s.client_token,
                                                           s.session)});
    s.voice_consumed_keys.clear();
    s.voice_hold_key = 0;
    s.voice_space_consumed = false;
    s.focused = false;
    if (panel_input_engine == engine) panel_input_engine = nullptr;
    ++s.focus_epoch;
    s.focused_context.clear();
    s.focused_client.clear();
    s.surrounding_utf16 = false;
    s.stop_clipboard_monitor();
    s.native_compose.reset();
    s.reset_mode_modifiers();
    s.backspace_hold.reset();
    s.key_repeat.reset();
    s.ai_context.clear();
    s.invalidate_providers();
    s.surrounding_text.clear();
    s.surrounding_valid = false;
    s.surrounding_cursor = 0;
    s.surrounding_anchor = 0;
    s.last_smart_punctuation = 0;
    s.last_smart_punctuation_time = 0;
    s.smart_punctuation_rejected = 0;
    s.paired_tracker.clear();
    sync_music(engine);
    // 离开客户端时上屏打开的韩文音节、注音转换、越南文单词或藏文音节串（`commits_on_blur`）。render() 用 IBUS_ENGINE_PREEDIT_COMMIT 模式画它，所以 IBus 已经把这段预编辑交给了被离开的客户端；这里再上屏运行时的副本会打两遍，或者打进下一个获得焦点的客户端。会话仍然结束它，不留下任何正在组字的内容。
    const int blur_scheme = msime::linux_host::scheme_rules(s.view);
    const bool blur_composition =
        s.session && blur_scheme >= 0 && msime::linux_host::scheme::CommitsOnBlur(blur_scheme) &&
        !s.view.value("editing_text", std::string{}).empty();
    if (blur_composition) {
      // The source is read from the view the composition was typed under, before the focus result replaces it.
      const auto source = typing_source(s);
      const auto left = response(msime_client_focus(s.session, false));
      // IBus writes the syllable as the preedit, not through commit_text, so it is counted here as typed text.
      if (left.contains("commit") && left.at("commit").is_string())
        record_typing_statistics(engine, left.at("commit").get<std::string>(), source);
      s.view = left.at("view");
    } else if (s.session)
      apply(engine, msime_client_focus(s.session, false));
    clear(engine);
    publish_mode(engine);
  });
}
void property_activate(IBusEngine *engine, const gchar *name, guint value) {
  const std::string candidate_name = name ? name : "";
  if (candidate_name == "InputEnabled" || candidate_name == "ChinesePunctuation" ||
      candidate_name == "CharacterWidth") {
    const char *target = candidate_name == "InputEnabled" ? "InputMode"
        : candidate_name == "ChinesePunctuation" ? "Punctuation" : "CharacterMode";
    property_activate(engine, target, value);
    return;
  }
  if (candidate_name == "CandidatePreviousPage" ||
      candidate_name == "CandidateNextPage") {
    if (value != PROP_STATE_UNCHECKED && value != PROP_STATE_CHECKED)
      return;
    page(engine, candidate_name == "CandidatePreviousPage"
                   ? MSIME_PREVIOUS_PAGE
                   : MSIME_NEXT_PAGE);
    return;
  }
  if (candidate_name.rfind("CandidatePin", 0) == 0 ||
      candidate_name.rfind("CandidateRemove", 0) == 0 ||
      candidate_name.rfind("CandidateFix", 0) == 0 ||
      candidate_name.rfind("CandidateClear", 0) == 0) {
    guarded(engine, "candidate_property", [&] {
      auto &s = state(engine);
      if (!s.session || !s.focused || s.blocked || !s.input_enabled)
        return;
      if (s.rendered_session != s.session || !s.rendered_candidates.is_array() ||
          !s.rendered_view.is_object() ||
          s.rendered_view.value("generation", uint64_t{0}) !=
              s.view.value("generation", uint64_t{0}))
        return;
      for (const auto &candidate : s.rendered_candidates) {
        const auto &id = candidate.at("id");
        const bool pin = candidate_name == candidate_action_name("CandidatePin", id);
        const bool remove = candidate_name == candidate_action_name("CandidateRemove", id);
        const bool clear = candidate_name == candidate_action_name("CandidateClear", id);
        uint8_t position = 0;
        for (uint8_t slot = 1; slot <= 5; ++slot) {
          if (candidate_name == candidate_action_name(
                  (std::string("CandidateFix") + std::to_string(slot)).c_str(), id)) {
            position = slot;
            break;
          }
        }
        if (!pin && !remove && !clear && position == 0)
          continue;
        if (id.at("session").get<uint64_t>() != s.session)
          return;
        const auto source = candidate.value("source", 0);
        if (!msime::linux_host::candidate_dictionary_actions_available(s.rendered_scheme, source))
          return;
        const auto generation = id.at("generation").get<uint64_t>();
        const auto index = id.at("index").get<size_t>();
        if (remove && !msime::linux_host::candidate_dictionary_removal_available(
                          s.rendered_scheme, source,
                          candidate.value("text", std::string{})))
          return;
        if (pin)
          apply(engine, msime_client_pin_candidate(s.session, generation, index));
        else if (remove)
          apply(engine, msime_client_remove_candidate(s.session, generation, index));
        else if (clear)
          apply(engine, msime_client_clear_candidate_position(s.session, generation, index));
        else
          apply(engine, msime_client_fix_candidate_position(s.session, generation, index, position));
        return;
      }
    });
    return;
  }
  if (candidate_name.rfind("NineKeySpelling/", 0) == 0) {
    guarded(engine, "nine_key_spelling", [&] {
      auto &s = state(engine);
      if (!s.session || !s.focused || s.blocked || !s.input_enabled ||
          s.rendered_session != s.session || !s.rendered_view.is_object() ||
          !s.rendered_view.value("nine_key", false) ||
          s.rendered_view.value("generation", uint64_t{0}) !=
              s.view.value("generation", uint64_t{0}))
        return;
      const auto spellings = s.rendered_view.value("nine_key_spellings", Json::array());
      if (!spellings.is_array())
        return;
      const auto generation = s.rendered_view.value("generation", uint64_t{0});
      for (size_t index = 0; index < spellings.size(); ++index) {
        if (!spellings.at(index).is_string() ||
            candidate_name != nine_key_spelling_action_name(s.session, generation, index))
          continue;
        apply(engine, msime_client_choose_nine_key_spelling(
                         s.session, generation, index));
        return;
      }
    });
    return;
  }
  auto &s = state(engine);
  const std::string property_name = name ? name : "";
  if (property_name == "VoiceCancel") {
    if (s.focused && !s.blocked && s.voice_active)
      guarded(engine, "voice_menu_cancel", [&] { voice_cancel(engine); });
    return;
  }
  if (property_name == "ClipboardHistory/OpenPanel") {
    if (s.focused && !s.blocked && !launch_desktop_panel("clipboard"))
      g_warning("Cannot start MSIME clipboard panel launcher");
    return;
  }
  if (property_name == "TranslateSentence") {
    if (s.focused && !s.blocked && s.input_enabled && s.session)
      guarded(engine, "translate_sentence", [&] { translate_sentence(engine); });
    return;
  }
  if (property_name.rfind("DesktopTools/", 0) == 0) {
    if (!s.focused || s.blocked)
      return;
    if (property_name == "DesktopTools/VoiceEnabled") {
      if (menu_save_pending || !s.focused || s.blocked ||
          (value != PROP_STATE_CHECKED && value != PROP_STATE_UNCHECKED))
        return;
      save_menu_preference(engine, MenuPreference::VoiceEnabled, value == PROP_STATE_CHECKED);
      return;
    }
    if (property_name == "DesktopTools/RetrySave") {
      if (!menu_save_pending && failed_menu_save &&
          failed_menu_save->configuration == configuration_generation &&
          failed_menu_save->directory == configured.value("preferences_directory", std::string{})) {
        const auto retry = *failed_menu_save;
        save_menu_preference(engine, retry.preference, retry.value);
      }
      return;
    }
    if (property_name == "DesktopTools/ToolbarEnabled") {
      if (value == PROP_STATE_CHECKED || value == PROP_STATE_UNCHECKED)
        save_menu_preference(engine, MenuPreference::Toolbar, value == PROP_STATE_CHECKED);
      return;
    }
    for (const auto &action : desktop_panel_actions) {
      if (property_name == action.property) {
        if (!launch_desktop_panel(action.panel))
          g_warning("Cannot start MSIME desktop panel launcher");
        return;
      }
    }
    return;
  }
  if (property_name.rfind("Toolbar/", 0) == 0) {
    if (!s.focused || s.blocked)
      return;
    if (property_name == "Toolbar/Emoji") {
      launch_desktop_panel("emoji");
      return;
    }
    if (property_name == "Toolbar/ScreenKeyboard") {
      launch_desktop_panel("keyboard");
      return;
    }
    if (property_name == "Toolbar/Settings") {
      launch_desktop_panel("settings");
      return;
    }
    const char *target =
        property_name == "Toolbar/InputMode" ? "InputMode"
        : property_name == "Toolbar/EnglishMode" ? "EnglishMode"
        : property_name == "Toolbar/Fullwidth" ? "CharacterMode"
        : property_name == "Toolbar/Punctuation" ? "Punctuation"
        : property_name == "Toolbar/CharacterSet" ? "TraditionalOutput"
        : nullptr;
    if (target && (value == PROP_STATE_CHECKED || value == PROP_STATE_UNCHECKED))
      property_activate(engine, target, value);
    return;
  }
  const bool clipboard_item =
      property_name.rfind("ClipboardHistory/", 0) == 0 &&
      property_name != "ClipboardHistory/Refresh" &&
      property_name != "ClipboardHistory/Latest" &&
      property_name.rfind("ClipboardHistory/Clear/", 0) != 0 &&
      property_name.rfind("ClipboardHistory/Remove/", 0) != 0;
  const bool clipboard_remove = property_name.rfind("ClipboardHistory/Remove/", 0) == 0;
  const bool clipboard_clear =
      property_name.rfind("ClipboardHistory/Clear/", 0) == 0;
  if (!name ||
      (!(clipboard_item || clipboard_remove || clipboard_clear) &&
       property_name != "ClipboardHistory/Refresh" &&
       std::string(name) != "InputMode" &&
       std::string(name) != "ClipboardHistory/Enabled" &&
       std::string(name) != "VoiceInput" &&
       std::string(name) != "CloudCandidates" &&
       std::string(name) != "CandidateTranslations" &&
       property_name.rfind("TranslationLanguage/", 0) != 0 &&
       std::string(name) != "Punctuation" &&
       std::string(name) != "SmartPunctuation" &&
       std::string(name) != "SmartPunctuationRepeat" &&
       std::string(name) != "PairedPunctuation" &&
       std::string(name) != "PunctuationLock/follow" &&
       std::string(name) != "PunctuationLock/chinese" &&
       std::string(name) != "PunctuationLock/english" &&
       std::string(name) != "CharacterMode" &&
       std::string(name) != "TraditionalOutput" &&
       std::string(name) != "EnglishCandidates" &&
       std::string(name) != "EnglishMode" &&
       std::string(name) != "Helpcode" &&
       property_name.rfind("HelpcodeSchema/", 0) != 0 &&
       std::string(name) != "EmojiCandidates" &&
       std::string(name) != "KaomojiCandidates" &&
       std::string(name) != "CandidateLayout/Vertical" &&
       std::string(name) != "CandidateLayout/Horizontal" &&
       property_name.rfind("GlobalTheme/", 0) != 0 &&
       property_name.rfind("CandidatePageSize/", 0) != 0 &&
       property_name.rfind("FrequencyMode/", 0) != 0 &&
       std::string(name) != "NumberRowSelection" &&
       std::string(name) != "NineKey" &&
       property_name.rfind("LocalModes/", 0) != 0 &&
       std::string(name) != "WordCharacter" &&
       std::string(name) != "PreeditStyle/raw" &&
       std::string(name) != "PreeditStyle/pinyin" &&
       std::string(name) != "PreeditStyle/empty" &&
       std::string(name) != "CandidateTheme/follow" &&
       std::string(name) != "CandidateTheme/light" &&
       std::string(name) != "CandidateTheme/dark" &&
       std::string(name) != "Scheme/Chinese" &&
       std::string(name) != "Scheme/Japanese" &&
       std::string(name) != "Scheme/Korean" &&
       property_name != "Scheme/Quanpin" &&
       property_name != "Scheme/Shuangpin" && property_name != "Scheme/Wubi" &&
       property_name != "Scheme/Cantonese" && property_name != "Scheme/Zhuyin" &&
       property_name != "Scheme/Vietnamese" && property_name != "Scheme/Tibetan" &&
       property_name.rfind("ShuangpinProfile/", 0) != 0) ||
      !s.focused || s.blocked ||
      (value != PROP_STATE_CHECKED && value != PROP_STATE_UNCHECKED))
    return;
  guarded(engine, "property_activate", [&] {
    const auto mixed_input = configured.at("preferences").value(
        "mixed_input", Json::object());
    const auto mixed_input_value = [&](const char *key, bool fallback) {
      if (!mixed_input.is_object()) return fallback;
      const auto setting = mixed_input.find(key);
      return setting != mixed_input.end() && setting->is_boolean()
                 ? setting->get<bool>()
                 : fallback;
    };
    if (property_name == "ClipboardHistory/Enabled") {
      if (menu_save_pending || !s.focused || s.blocked) return;
      save_menu_preference(engine, MenuPreference::ClipboardHistoryEnabled, value == PROP_STATE_CHECKED);
      publish_mode(engine);
      return;
    }
    if (property_name.rfind("ClipboardHistory/", 0) == 0 &&
        (!s.clipboard_enabled || !s.input_enabled))
      return;
    if (property_name == "VoiceInput") {
      if (!s.voice_enabled || s.voice_provider_socket.empty())
        return;
      if (value == PROP_STATE_CHECKED)
        voice_start(engine);
      else if (s.voice_active)
        voice_stop(engine);
      return;
    }
    if (property_name == "CloudCandidates") {
      const bool enabled = value == PROP_STATE_CHECKED;
      if (menu_save_pending || enabled == s.cloud_candidates)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::CloudCandidates, enabled);
        return;
      }
      s.cloud_candidates_override = enabled;
      s.cloud_candidates = enabled;
      s.invalidate_providers();
      publish_mode(engine);
      if (enabled) online_schedule(engine);
      return;
    }
    if (property_name == "CandidateTranslations") {
      const bool enabled = value == PROP_STATE_CHECKED;
      if (menu_save_pending || enabled == s.candidate_translations)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::CandidateTranslations, enabled);
        return;
      }
      s.candidate_translations_override = enabled;
      s.candidate_translations = enabled;
      s.invalidate_providers();
      sync_translation_preferences(engine);
      clear_candidate_translations(engine);
      publish_mode(engine);
      if (enabled)
        translation_schedule(engine);
    settled_rerank_schedule(engine);
      return;
    }
    if (property_name.rfind("TranslationLanguage/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending)
        return;
      const auto selected = property_name.substr(
          std::string("TranslationLanguage/").size());
      if (selected != "en" && selected != "fr" && selected != "ja" &&
          selected != "es" && selected != "ru" && selected != "de" &&
          selected != "ko")
        return;
      if (selected == s.translation_target_language)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::TranslationLanguage, selected);
        return;
      }
      s.translation_target_language_override = selected;
      s.translation_target_language = selected;
      s.invalidate_providers();
      sync_translation_preferences(engine);
      clear_candidate_translations(engine);
      publish_mode(engine);
      if (s.candidate_translations)
        translation_schedule(engine);
    settled_rerank_schedule(engine);
      return;
    }
    if (property_name == "NumberRowSelection") {
      if (s.view.value("nine_key", false) || menu_save_pending ||
          s.number_row_selection == (value == PROP_STATE_CHECKED))
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::NumberRowSelection, value == PROP_STATE_CHECKED);
        return;
      }
      s.number_row_selection = value == PROP_STATE_CHECKED;
      s.number_row_override = s.number_row_selection;
      publish_mode(engine);
      return;
    }
    if (property_name == "NineKey") {
      const bool enabled = value == PROP_STATE_CHECKED;
      const auto active_scheme = effective_scheme(s);
      if (menu_save_pending || active_scheme != "quanpin" ||
          s.view.value("nine_key", false) == enabled)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::NineKey, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.nine_key_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name.rfind("LocalModes/", 0) == 0) {
      const auto key = property_name.substr(std::string("LocalModes/").size());
      const auto allowed = [](const std::string &value) {
        return value == "unicode" || value == "date_time" ||
               value == "quick_phrase" || value == "emoji" ||
               value == "kaomoji" || value == "super_jianpin" ||
               value == "temporary_english" || value == "temporary_japanese" ||
               value == "expression" || value == "command" || value == "mention";
      };
      if (!allowed(key))
        return;
      const auto configured_modes = configured.at("preferences").value(
          "local_modes", Json::object());
      const bool current = s.local_mode_overrides.contains(key)
                               ? s.local_mode_overrides.at(key).get<bool>()
                               : configured_modes.value(
                                     key, msime::linux_host::local_mode_enabled_by_default(key));
      const bool enabled = value == PROP_STATE_CHECKED;
      if (current == enabled)
        return;
      if (menu_save_pending) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        // Persist first. The save callback applies the accepted snapshot and
        // recreates the runtime through apply_live_preferences; a failed or
        // conflicting write must not leave a session-only value visible.
        save_menu_preference(engine, MenuPreference::LocalMode,
                             Json{{"key", key}, {"enabled", enabled}});
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.local_mode_overrides[key] = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name == "WordCharacter") {
      const bool enabled = value == PROP_STATE_CHECKED;
      if (menu_save_pending || s.word_character_override.value_or(s.word_character.enabled) == enabled)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::WordCharacter, enabled);
        return;
      }
      s.word_character_override = enabled;
      s.word_character.enabled = enabled;
      publish_mode(engine);
      return;
    }
    if (property_name.rfind("HelpcodeSchema/", 0) == 0) {
      const auto selected = property_name.substr(std::string("HelpcodeSchema/").size());
      if (selected != "lantian" && selected != "ziranma" && selected != "shouyou2_0" &&
          selected != "shouyouplus" && selected != "xiaohe" && selected != "jiajia")
        return;
      const auto active_scheme = effective_scheme(s);
      if (active_scheme != "quanpin" && active_scheme != "shuangpin")
        return;
      if (s.helpcode_schema_override.value_or(
              configured.at("preferences").value(active_scheme + "_helpcode", Json::object())
                  .value("schema", std::string(msime::linux_host::default_helpcode_schema(
                                       active_scheme)))) == selected)
        return;
      if (menu_save_pending || value != PROP_STATE_CHECKED) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, active_scheme == "quanpin" ? MenuPreference::QuanpinHelpcodeSchema
                                                                : MenuPreference::ShuangpinHelpcodeSchema, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.helpcode_schema_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name.rfind("FrequencyMode/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending) return;
      const auto selected = property_name.substr(std::string("FrequencyMode/").size());
      if (selected != "disabled" && selected != "pin" && selected != "halve" &&
          selected != "linear" && selected != "promote")
        return;
      if (s.frequency_mode_override.value_or(
              configured.at("preferences").value("frequency", Json::object())
                  .value("mode", "promote")) == selected)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::FrequencyMode, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.frequency_mode_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    const auto restart_with_frequency_override = [&](std::optional<uint8_t> trigger,
                                                      std::optional<uint8_t> step) {
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      if (trigger) s.frequency_trigger_count_override = *trigger;
      if (step) s.frequency_linear_step_override = *step;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
    };
    if (property_name.rfind("FrequencyTriggerCount/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending) return;
      const auto suffix = property_name.substr(std::string("FrequencyTriggerCount/").size());
      if (suffix.size() != 1 || suffix.front() < '1' || suffix.front() > '9') {
        if (suffix != "10") return;
      }
      const auto selected = static_cast<uint8_t>(std::stoi(suffix));
      if (s.frequency_trigger_count == selected) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::FrequencyTriggerCount, selected);
        return;
      }
      restart_with_frequency_override(selected, std::nullopt);
      return;
    }
    if (property_name.rfind("FrequencyLinearStep/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending) return;
      const auto suffix = property_name.substr(std::string("FrequencyLinearStep/").size());
      if (suffix.size() != 1 || suffix.front() < '1' || suffix.front() > '9') {
        if (suffix != "10") return;
      }
      const auto selected = static_cast<uint8_t>(std::stoi(suffix));
      if (s.frequency_linear_step == selected) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::FrequencyLinearStep, selected);
        return;
      }
      restart_with_frequency_override(std::nullopt, selected);
      return;
    }
    if (property_name == "Learning") {
      if (menu_save_pending || s.private_input) return;
      const bool enabled = value == PROP_STATE_CHECKED;
      if (enabled == s.learning) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::Learning, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.learning_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name == "ShuangpinPreedit") {
      const auto active_scheme = effective_scheme(s);
      if (active_scheme != "shuangpin" || menu_save_pending)
        return;
      const bool enabled = value == PROP_STATE_CHECKED;
      if (enabled == s.shuangpin_preedit_uses_raw)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::ShuangpinPreedit, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.shuangpin_preedit_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name == "WubiCodeHint") {
      const auto active_scheme = effective_scheme(s);
      if (active_scheme != "wubi" || menu_save_pending)
        return;
      const bool enabled = value == PROP_STATE_CHECKED;
      if (enabled == s.wubi_code_hint)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::WubiCodeHint, enabled);
        return;
      }
      s.wubi_code_hint_override = enabled;
      s.wubi_code_hint = enabled;
      render(engine, s.view);
      publish_mode(engine);
      return;
    }
    if (property_name.rfind("CandidatePageSize/", 0) == 0) {
      try {
        if (value != PROP_STATE_CHECKED || menu_save_pending) return;
        const auto suffix = property_name.substr(std::string("CandidatePageSize/").size());
        if (suffix.size() != 1 || suffix.front() < '1' || suffix.front() > '9') return;
        const auto selected = static_cast<uint8_t>(suffix.front() - '0');
        if (s.candidate_page_size_override.value_or(
                configured.at("preferences").value("candidate_page_size", 6)) == selected)
          return;
        const auto directory = configured.value("preferences_directory", std::string{});
        if (!directory.empty() && directory.front() == '/') {
          save_menu_preference(engine, MenuPreference::CandidatePageSize, selected);
          return;
        }
        if (!s.session) return;
        s.candidate_page_size_override = static_cast<uint8_t>(selected);
        s.view = response(msime_client_set_candidate_page_size(
                         s.session, static_cast<uint8_t>(selected))).at("view");
        render(engine, s.view);
        publish_mode(engine);
      } catch (...) {
      }
      return;
    }
    if (property_name.rfind("ShuangpinProfile/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending) return;
      const auto selected = property_name.substr(std::string("ShuangpinProfile/").size());
      if (selected != "xiaohe" && selected != "ziranma" && selected != "shoudao" &&
          selected != "microsoft")
        return;
      if (s.shuangpin_profile_override.value_or(
              configured.at("preferences").value("shuangpin_profile", "xiaohe")) == selected)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::ShuangpinProfile, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.shuangpin_profile_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (property_name == "ClipboardHistory/Refresh") {
      if (!s.input_enabled || s.clipboard_history_path.empty())
        return;
      ++s.clipboard_generation;
      s.clipboard_items_cache.clear();
      s.clipboard_loaded = false;
      publish_mode(engine);
      return;
    }
    if (clipboard_remove || clipboard_item) {
      for (size_t index = 0; index < s.clipboard_items_cache.size(); ++index) {
        const auto expected = std::string(clipboard_remove
            ? "ClipboardHistory/Remove/" : "ClipboardHistory/") +
            std::to_string(s.clipboard_generation) + "/" + std::to_string(index);
        if (property_name != expected)
          continue;
        const auto text = s.clipboard_items_cache[index];
        if (clipboard_remove) {
          if (clipboard_delete(s.clipboard_history_path, text)) {
            s.clipboard_items_cache.clear();
            s.clipboard_loaded = false;
            ++s.clipboard_generation;
            publish_mode(engine);
          }
        } else {
          commit_text(engine, text);
        }
        return;
      }
      return;
    }
    if (clipboard_clear) {
      const auto expected = std::string("ClipboardHistory/Clear/") +
                            std::to_string(s.clipboard_generation);
      if (property_name != expected)
        return;
      if (!clipboard_delete(s.clipboard_history_path, std::nullopt))
        return;
      s.clipboard_items_cache.clear();
      s.clipboard_loaded = false;
      ++s.clipboard_generation;
      publish_mode(engine);
      return;
    }
    if (property_name == "PairedPunctuation") {
      if (menu_save_pending || (value == PROP_STATE_CHECKED) == s.paired_punctuation) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::PairedPunctuation, value == PROP_STATE_CHECKED);
        return;
      }
      const bool enabled = value == PROP_STATE_CHECKED;
      if (s.session) {
        s.view = response(msime_client_set_paired_punctuation(s.session, enabled));
        render(engine, s.view);
      }
      s.paired_punctuation_override = enabled;
      s.paired_punctuation = enabled;
      s.paired_tracker.clear();
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "SmartPunctuation") {
      if (menu_save_pending || (value == PROP_STATE_CHECKED) == s.smart_punctuation) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::SmartPunctuation, value == PROP_STATE_CHECKED);
        return;
      }
      s.smart_punctuation = value == PROP_STATE_CHECKED;
      s.smart_punctuation_override = s.smart_punctuation;
      if (!s.smart_punctuation) {
        s.last_smart_punctuation = 0;
        s.smart_punctuation_rejected = 0;
      }
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "SmartPunctuationRepeat") {
      if (menu_save_pending || (value == PROP_STATE_CHECKED) == s.smart_punctuation_repeat) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::SmartPunctuationRepeat, value == PROP_STATE_CHECKED);
        return;
      }
      s.smart_punctuation_repeat = value == PROP_STATE_CHECKED;
      s.smart_repeat_override = s.smart_punctuation_repeat;
      if (!s.smart_punctuation_repeat) {
        s.last_smart_punctuation = 0;
        s.smart_punctuation_rejected = 0;
      }
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "CharacterMode") {
      if (menu_save_pending || s.fullwidth == (value == PROP_STATE_CHECKED)) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::CharacterWidth,
                             value == PROP_STATE_CHECKED);
        return;
      }
      s.fullwidth = value == PROP_STATE_CHECKED;
      s.paired_tracker.clear();
      if (s.session) {
        s.view = response(msime_client_set_character_width(s.session, s.fullwidth));
        s.session_fullwidth = s.fullwidth;
        render(engine, s.view);
      }
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "TraditionalOutput") {
      if (!msime::linux_host::scheme::ScriptConversionApplies(
              msime::linux_host::scheme_number(effective_scheme(s))))
        return;
      if (menu_save_pending || s.traditional_output == (value == PROP_STATE_CHECKED)) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::TraditionalOutput, value == PROP_STATE_CHECKED);
        return;
      }
      s.traditional_output = value == PROP_STATE_CHECKED;
      s.traditional_output_override = s.traditional_output;
      render(engine, s.view);
      publish_mode(engine);
      return;
    }
    if (std::string(name).rfind("CandidateTheme/", 0) == 0) {
      const auto selected = std::string(name).substr(std::string("CandidateTheme/").size());
      if (value != PROP_STATE_CHECKED || menu_save_pending)
        return;
      if (s.theme_override.value_or(
              configured.at("preferences").value("candidate_theme", "follow")) == selected)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::CandidateTheme, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.theme_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name).rfind("GlobalTheme/", 0) == 0) {
      const auto selected = std::string(name).substr(std::string("GlobalTheme/").size());
      if (value != PROP_STATE_CHECKED || menu_save_pending)
        return;
      auto preferences = configured.at("preferences");
      if (s.theme_choice_override) msime::linux_host::apply_theme_choice(preferences, *s.theme_choice_override);
      // Only an entry the menu lists can be chosen, and choosing the one already shown changes nothing.
      const auto themes = theme_choices();
      if (msime::linux_host::current_theme_choice(preferences, themes) == selected) return;
      auto change = msime::linux_host::theme_choice_change(themes, selected);
      if (!change) return;
      // 自定义 while the custom theme is drawn over a listed package changes no preference; republish so the panel checks the package entry again rather than the radio just clicked.
      auto chosen = preferences;
      msime::linux_host::apply_theme_choice(chosen, *change);
      if (chosen == preferences) {
        publish_mode(engine);
        return;
      }
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::GlobalTheme, std::move(*change));
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      // Later choices stack on an unsaved one; a removal stays a null so it still removes the stored key.
      if (s.theme_choice_override) {
        (*s.theme_choice_override)["global_theme"] = change->at("global_theme");
        // Held in a local: items() only refers to the JSON it iterates, and the temporary value() returns would be gone before the loop body runs.
        const Json custom_theme = change->value("custom_theme", Json::object());
        for (const auto &[key, item] : custom_theme.items())
          (*s.theme_choice_override)["custom_theme"][key] = item;
      } else {
        s.theme_choice_override = std::move(*change);
      }
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name).rfind("PreeditStyle/", 0) == 0) {
      const auto selected = std::string(name).substr(std::string("PreeditStyle/").size());
      if (value != PROP_STATE_CHECKED || menu_save_pending)
        return;
      if (s.preedit_override.value_or(
              configured.at("preferences").value("tsf_preedit_style", "raw")) == selected)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::PreeditStyle, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.preedit_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "CandidateLayout/Vertical" ||
        std::string(name) == "CandidateLayout/Horizontal") {
      const auto selected = std::string(name) == "CandidateLayout/Horizontal"
                                ? "horizontal" : "vertical";
      if (value != PROP_STATE_CHECKED || menu_save_pending)
        return;
      if (s.layout_override.value_or(
              configured.at("preferences").value("candidate_layout", "vertical")) == selected)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::CandidateLayout, selected);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.layout_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "EmojiCandidates" ||
        std::string(name) == "KaomojiCandidates") {
      const bool enabled = value == PROP_STATE_CHECKED;
      auto &setting_override = std::string(name) == "EmojiCandidates"
                           ? s.emoji_override : s.kaomoji_override;
      const auto key = std::string(name) == "EmojiCandidates" ? "emoji" : "kaomoji";
      if (setting_override.value_or(mixed_input_value(key, false)) == enabled)
        return;
      if (menu_save_pending) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, std::string(name) == "EmojiCandidates" ? MenuPreference::EmojiCandidates : MenuPreference::KaomojiCandidates, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      setting_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "EnglishCandidates") {
      const bool enabled = value == PROP_STATE_CHECKED;
      if (s.english_override.value_or(mixed_input_value("english", true)) == enabled)
        return;
      if (menu_save_pending) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::EnglishCandidates, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.english_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "EnglishMode") {
      const bool enabled = value == PROP_STATE_CHECKED;
      if (!s.input_enabled || !s.session || s.english_mode == enabled)
        return;
      s.view = response(msime_client_set_english_mode(s.session, enabled));
      s.english_mode = enabled;
      s.dedicated_english_override = enabled;
      render(engine, s.view);
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "Helpcode") {
      const bool enabled = value == PROP_STATE_CHECKED;
      const auto active_scheme = effective_scheme(s);
      if (active_scheme != "quanpin" && active_scheme != "shuangpin")
        return;
      const bool current = s.helpcode_override.value_or(
          configured.at("preferences").value(active_scheme + "_helpcode", Json::object())
              .value("enabled", true));
      if (current == enabled)
        return;
      if (menu_save_pending) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, active_scheme == "quanpin" ? MenuPreference::QuanpinHelpcode
                                                                : MenuPreference::ShuangpinHelpcode, enabled);
        return;
      }
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.helpcode_override = enabled;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    if (std::string(name).rfind("Scheme/", 0) == 0) {
      if (value != PROP_STATE_CHECKED || menu_save_pending) return;
      auto selected = property_name == "Scheme/Japanese" ? std::string("japanese")
          : property_name == "Scheme/Korean" ? std::string("korean")
          : property_name == "Scheme/Quanpin" ? std::string("quanpin")
          : property_name == "Scheme/Shuangpin" ? std::string("shuangpin")
          : property_name == "Scheme/Wubi" ? std::string("wubi")
          : property_name == "Scheme/Cantonese" ? std::string("cantonese")
          : property_name == "Scheme/Zhuyin" ? std::string("zhuyin")
          : property_name == "Scheme/Vietnamese" ? std::string("vietnamese")
          : property_name == "Scheme/Tibetan" ? std::string("tibetan")
          : configured.at("preferences").value("last_chinese_scheme", std::string("quanpin"));
      // A scheme this host does not know, or Cantonese and Zhuyin once their dictionary is gone, is saved as the scheme host-api would run instead.
      selected = msime::linux_host::effective_input_scheme(
          selected, configured.at("preferences").value("last_chinese_scheme", std::string("quanpin")), configured_dictionaries);
      if (s.scheme_override.value_or(
              configured.at("preferences").value("scheme", "quanpin")) == selected) return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::InputScheme, selected);
        return;
      }
      // Scheme-specific session overrides must not leak into the newly
      // selected scheme. Shared preferences remain intact and are reloaded
      // by the recreated Engine session.
      s.helpcode_override.reset();
      s.helpcode_schema_override.reset();
      s.autocorrect_transposition_override.reset();
      s.autocorrect_neighbor_override.reset();
      s.nine_key_override.reset();
      s.shuangpin_profile_override.reset();
      if (s.session)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      s.scheme_override = selected;
      s.open();
      if (s.session)
        apply(engine, msime_client_focus(s.session, true));
      publish_mode(engine);
      return;
    }
    const bool enabled = value == PROP_STATE_CHECKED;
    if (std::string(name).rfind("PunctuationLock/", 0) == 0) {
      const auto selected = std::string(name).substr(std::string("PunctuationLock/").size());
      if (value != PROP_STATE_CHECKED || menu_save_pending || selected == s.punctuation_lock)
        return;
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::PunctuationLock, selected);
        return;
      }
      if (s.session) {
        s.view = response(msime_client_set_punctuation_lock(
            s.session, selected == "chinese" ? 1 : selected == "english" ? 2 : 0));
        render(engine, s.view);
      }
      s.punctuation_lock_override = selected;
      s.punctuation_lock = selected;
      s.paired_tracker.clear();
      {
        const bool chinese = selected == "follow"
                                  ? s.punctuation_override.value_or(configured.at("preferences").value("chinese_punctuation", true))
                                  : selected == "chinese";
        if (s.session) {
          s.view = response(msime_client_set_chinese_punctuation(s.session, chinese));
          s.session_chinese_punctuation = chinese;
          render(engine, s.view);
        }
        s.chinese_punctuation = chinese;
      }
      publish_mode(engine);
      return;
    }
    if (std::string(name) == "Punctuation") {
      if (!s.input_enabled || !s.session || menu_save_pending)
        return;
      // A pinned lock holds, as Windows routes the toolbar switch through ResolvePunctuationOpen: nothing changes and nothing is saved. Republishing puts the toggle back.
      if (s.punctuation_lock != "follow") {
        publish_mode(engine);
        return;
      }
      const auto directory = configured.value("preferences_directory", std::string{});
      if (!directory.empty() && directory.front() == '/') {
        save_menu_preference(engine, MenuPreference::ChinesePunctuation, enabled);
        return;
      }
      s.view =
          response(msime_client_set_chinese_punctuation(s.session, enabled));
      s.session_chinese_punctuation = enabled;
      s.chinese_punctuation = enabled;
      s.punctuation_override = enabled;
      s.paired_tracker.clear();
      publish_mode(engine);
      return;
    }
    if (enabled != s.input_enabled) {
      // A recording survives the switch, as with the mode shortcuts in toggle_input_mode.
      s.invalidate_providers();
      if (!enabled && s.session)
        apply(engine,
              msime_client_command(s.session, MSIME_COMMIT_RAW));
      if (s.mode_scope_global)
        global_input_enabled = enabled;
      s.input_enabled = enabled;
      s.open();
      resync_punctuation_for_mode(engine);
      if (s.session)
        apply(engine, msime_client_focus(s.session, enabled));
      clear(engine);
      if (s.voice_active)
        render(engine, s.view);
    }
    publish_mode(engine);
  });
}
void reset(IBusEngine *engine) {
  guarded(engine, "reset", [&] {
    state(engine).host_shortcut_strokes.clear();
    // A chord release that never arrives must not swallow the next stroke of the same key.
    state(engine).mode_chord_held = false;
    state(engine).character_set_chord_held = false;
    state(engine).ai_context.clear();
    state(engine).native_compose.reset();
    state(engine).backspace_hold.reset();
    if (state(engine).voice_active)
      voice_cancel(engine);
    state(engine).voice_consumed_keys.clear();
    state(engine).voice_hold_key = 0;
    state(engine).voice_space_consumed = false;
    state(engine).last_smart_punctuation = 0;
    state(engine).last_smart_punctuation_time = 0;
    state(engine).smart_punctuation_rejected = 0;
    state(engine).paired_tracker.clear();
    if (state(engine).session)
      apply(engine, msime_client_command(state(engine).session, MSIME_CANCEL));
    clear(engine);
  });
}
void content_type(IBusEngine *engine, guint purpose, guint hints) {
  guarded(engine, "content_type", [&] {
    auto &s = state(engine);
    bool blocked = purpose == IBUS_INPUT_PURPOSE_PASSWORD ||
                   purpose == IBUS_INPUT_PURPOSE_PIN ||
                   purpose == IBUS_INPUT_PURPOSE_NUMBER ||
                   purpose == IBUS_INPUT_PURPOSE_DIGITS ||
                   purpose == IBUS_INPUT_PURPOSE_PHONE;
    bool private_input =
        (hints & (IBUS_INPUT_HINT_PRIVATE | IBUS_INPUT_HINT_NO_SPELLCHECK)) !=
        0;
    if (blocked == s.blocked && private_input == s.private_input)
      return;
    s.close();
    clear(engine);
    s.blocked = blocked;
    s.private_input = private_input;
    s.open();
    if (s.session)
      apply(engine, msime_client_focus(s.session, true));
    sync_music(engine);
    publish_mode(engine);
  });
}
bool modifier(guint key) {
  return (key >= IBUS_Shift_L && key <= IBUS_Hyper_R) || key == IBUS_Num_Lock ||
         key == IBUS_Scroll_Lock || key == IBUS_Mode_switch ||
         key == IBUS_ISO_Level3_Shift || key == IBUS_ISO_Level5_Shift;
}
std::optional<size_t> candidate_digit_slot(guint key, guint keycode,
                                           guint flags, const Json &view) {
  if (!view.is_object() ||
      view.value("local_mode", std::string("none")) == "unknown")
    return std::nullopt;
  const auto modifiers = flags &
      (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK |
       IBUS_SUPER_MASK | IBUS_META_MASK | IBUS_HYPER_MASK | IBUS_MOD5_MASK);
  // Digits are input in the modes that spell with them (unicode, expression): the Engine lists them in spelling_symbols.
  const bool spelling_digits = msime::linux_host::spelling_digits(view);
  const bool shifted = (modifiers & IBUS_SHIFT_MASK) != 0;
  // Windows uses Shift+the physical number row for Unicode candidates, while
  // ordinary modes use the unmodified row. IBus exposes the shifted symbols
  // as key values, so map those symbols back to their physical slots.
  if (modifiers != (spelling_digits ? IBUS_SHIFT_MASK : 0))
    return std::nullopt;
  if (!spelling_digits) {
    // IBus clients send evdev codes (GTK subtracts 8 from XKB hardware
    // codes). The number row is 2..11 (1..9,0); this preserves physical-key
    // selection when the active layout produces symbols such as '&' or 'é'.
    if (keycode >= 2 && keycode <= 11)
      return keycode == 11 ? 9 : static_cast<size_t>(keycode - 2);
    if (key >= IBUS_1 && key <= IBUS_9)
      return static_cast<size_t>(key - IBUS_1);
    if (key == IBUS_0)
      return 9;
    if (key >= IBUS_KP_1 && key <= IBUS_KP_9)
      return static_cast<size_t>(key - IBUS_KP_1);
    if (key == IBUS_KP_0)
      return 9;
    return std::nullopt;
  }
  if (!shifted)
    return std::nullopt;
  // A shifted number-row symbol the mode spells with (the expression mode's % ^ * ( )) is input, not the slot under it. A digit still picks: that is what Shift gives on AZERTY, and on the keypad of some X11 layouts.
  const gunichar shifted_character = ibus_keyval_to_unicode(key);
  if ((shifted_character < '0' || shifted_character > '9') &&
      msime::linux_host::spelling_symbol(view, shifted_character))
    return std::nullopt;
  if (keycode >= 2 && keycode <= 11)
    return keycode == 11 ? 9 : static_cast<size_t>(keycode - 2);
  switch (key) {
  case '!': return 0;
  case '@': return 1;
  case '#': return 2;
  case '$': return 3;
  case '%': return 4;
  case '^': return 5;
  case '&': return 6;
  case '*': return 7;
  case '(': return 8;
  case ')': return 9;
  // Some X11 layouts keep keypad keysyms unchanged with Shift.
  case IBUS_KP_1: return 0;
  case IBUS_KP_2: return 1;
  case IBUS_KP_3: return 2;
  case IBUS_KP_4: return 3;
  case IBUS_KP_5: return 4;
  case IBUS_KP_6: return 5;
  case IBUS_KP_7: return 6;
  case IBUS_KP_8: return 7;
  case IBUS_KP_9: return 8;
  case IBUS_KP_0: return 9;
  default: return std::nullopt;
  }
}
std::optional<char> keypad_punctuation(guint key) {
  switch (key) {
  case IBUS_KP_Decimal:
    return '.';
  case IBUS_KP_Separator:
    return ',';
  case IBUS_KP_Subtract:
    return '-';
  case IBUS_KP_Add:
    return '+';
  case IBUS_KP_Divide:
    return '/';
  case IBUS_KP_Multiply:
    return '*';
#ifdef IBUS_KP_Equal
  case IBUS_KP_Equal:
    return '=';
#endif
  default:
    return std::nullopt;
  }
}
bool microsoft_shuangpin_ing_key(const Json &view, guint key, guint modifiers) {
  if (key != IBUS_semicolon || modifiers != 0 ||
      !view.value("microsoft_shuangpin", false))
    return false;
  const auto editing_text = view.value("editing_text", std::string{});
  const auto caret = std::min<std::size_t>(
      view.value("caret_position", editing_text.size()), editing_text.size());
  const auto separator = caret == 0
                             ? std::string::npos
                             : editing_text.rfind('\'', caret - 1);
  const auto chunk_start = separator == std::string::npos ? 0 : separator + 1;
  return (caret - chunk_start) % 2 == 1;
}
bool unicode_plus_key(const Json &view, guint key, guint modifiers) {
  return key == '+' && modifiers == IBUS_SHIFT_MASK &&
         view.value("local_mode", std::string("none")) == "unicode" &&
         view.value("editing_text", std::string{}) == "U";
}
struct ModeHintNotice {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
  uint64_t id;
};
// 中英文切换后在辅助区域短暂显示「中」或「英」，对应共享偏好 input_mode_hud。
//
// IBus 没有 Fcitx5 那种由面板绘制的信息弹窗，辅助文本是这个宿主唯一能表达的位置：它由
// panel 按当前输入上下文摆放，因此也跟着输入点走。不自己画窗口——那条边界在这个宿主上
// 仍然成立（Fcitx5 那侧的徽章是所有者要求的例外，且带 logo 是它存在的理由）。
//
// 隐藏时先确认辅助区域还属于这条提示：用户可能在这 1.2 秒内已经开始打字，那时辅助文本
// 是候选页码，收掉它等于替用户关掉正在看的东西。代次和组合状态两道都查。
void show_input_mode_hint(IBusEngine *engine) {
  auto &s = state(engine);
  if (!configured.contains("preferences") ||
      !configured.at("preferences").value("input_mode_hud", true))
    return;
  if (!s.focused || s.blocked)
    return;
  ++s.mode_hint_id;
  if (s.mode_hint_id == 0)
    ++s.mode_hint_id;
  ibus_engine_update_auxiliary_text(
      engine, ibus_text_new_from_string(s.input_enabled ? "中" : "英"), TRUE);
  auto *notice = new ModeHintNotice{engine, s.alive, s.mode_hint_id};
  g_timeout_add_full(
      G_PRIORITY_DEFAULT, 1200,
      +[](gpointer data) -> gboolean {
        std::unique_ptr<ModeHintNotice> notice(static_cast<ModeHintNotice *>(data));
        if (!notice->alive->load())
          return G_SOURCE_REMOVE;
        auto &s = state(notice->engine);
        const bool composing =
            !s.view.value("editing_text", std::string{}).empty() ||
            !s.view.value("candidates", Json::array()).empty();
        if (s.mode_hint_id == notice->id && !composing)
          ibus_engine_hide_auxiliary_text(notice->engine);
        return G_SOURCE_REMOVE;
      },
      notice, nullptr);
}
void toggle_input_mode(IBusEngine *engine) {
  auto &s = state(engine);
  s.native_compose.reset();
  s.paired_tracker.clear();
  // A recording survives the switch, as on Windows: voice input does not depend on the input mode.
  s.invalidate_providers();
  // Windows mode switching commits the reading string, not the candidate.
  if (s.input_enabled && s.session)
    apply(engine, msime_client_command(s.session, MSIME_COMMIT_RAW));
  s.input_enabled = !s.input_enabled;
  s.remember_app_input_mode();
  if (s.mode_scope_global)
    global_input_enabled = s.input_enabled;
  s.open();
  resync_punctuation_for_mode(engine);
  if (s.session)
    apply(engine, msime_client_focus(s.session, s.input_enabled));
  clear(engine);
  if (s.voice_active)
    render(engine, s.view);
  publish_mode(engine);
  show_input_mode_hint(engine);
}
gboolean process_key(IBusEngine *engine, guint key, guint keycode, guint flags) {
  auto &s = state(engine);
  // A router lease cannot be issued before engine construction completes.
  if (s.client_token == 0)
    return FALSE;
  const msime_client_key_event routed_event = {
      {s.client_token, s.focus_epoch,
       msime::linux_host::KeyRouterAdapter::lease_token(s.client_token,
                                                         s.session)},
      msime::linux_host::KeyRouterAdapter::virtual_key(key), keycode,
      msime::linux_host::KeyRouterAdapter::modifiers(
          (flags & IBUS_SHIFT_MASK) != 0, (flags & IBUS_CONTROL_MASK) != 0,
          (flags & IBUS_MOD1_MASK) != 0,
          (flags & (IBUS_MOD4_MASK | IBUS_SUPER_MASK)) != 0),
      key <= 0xffffu ? key : 0u, false};
  const auto dispatch_result = s.key_router.check(routed_event);
  if (dispatch_result != MSIME_CLIENT_KEY_SENT)
    return FALSE;
  const bool shift_key = key == IBUS_Shift_L || key == IBUS_Shift_R;
  const bool ctrl_key = key == IBUS_Control_L || key == IBUS_Control_R;
  const bool release = (flags & IBUS_RELEASE_MASK) != 0;
  // Pressing CapsLock reports the old lock state and releasing it the new one, so every event is checked.
  if (const bool caps_lock = (flags & IBUS_LOCK_MASK) != 0; caps_lock != s.caps_lock) {
    s.caps_lock = caps_lock;
    publish_mode(engine);
  }
  if (key == IBUS_BackSpace && release) {
    const bool owned = s.backspace_hold.armed();
    s.backspace_hold.release();
    if (owned) return TRUE;
  } else if (!release && key != IBUS_BackSpace) {
    s.backspace_hold.reset();
  }
  // Physical key identity survives releasing Shift before the letter. Use a
  // normalized keysym only for synthetic events without a hardware keycode.
  const guint host_stroke = keycode != 0 ? keycode
      : ((key >= 'A' && key <= 'Z' ? key - 'A' + 'a' : key) | 0x80000000u);
  if (s.host_shortcut_strokes.count(host_stroke) != 0) {
    if (release) s.host_shortcut_strokes.erase(host_stroke);
    return TRUE;
  }
  // A held key can repeat after stop, cancellation or a fast final result.
  // Keep consuming its stroke even if modifiers or voice settings changed.
  if (!release &&
      (s.voice_consumed_keys.count(key) != 0 ||
       (key == IBUS_space && s.voice_space_consumed)))
    return TRUE;
  const bool repeated_modifier = !release &&
      ((shift_key && s.shift_down) || (ctrl_key && s.ctrl_down));
  if (shift_key) s.shift_down = !release;
  if (key == IBUS_Control_R) s.right_ctrl_down = !release;
  if (key == IBUS_Control_L) s.left_ctrl_down = !release;
  if (ctrl_key) s.ctrl_down = s.right_ctrl_down || s.left_ctrl_down;

  const guint chord_modifiers = flags &
      (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK |
       IBUS_META_MASK | IBUS_HYPER_MASK | IBUS_MOD5_MASK);
  if (shift_key && (flags & IBUS_RELEASE_MASK)) {
    if (!s.mode_shift_enabled || !s.pure_shift_candidate || chord_modifiers ||
        g_get_monotonic_time() >= s.modifier_toggle_deadline) {
      s.pure_shift_candidate = false;
      return FALSE;
    }
    s.pure_shift_candidate = false;
    if (!s.focused || s.blocked)
      return FALSE;
    guarded(engine, "process_key", [&] {
      toggle_input_mode(engine);
    });
    // Windows toggles on the bare modifier release but still lets the application see it, so a program tracking Shift state does not keep it latched.
    return FALSE;
  }
  if (shift_key && !(flags & IBUS_RELEASE_MASK)) {
    if (repeated_modifier) return FALSE;
    s.modifier_toggle_deadline = g_get_monotonic_time() + 500000;
    s.pure_ctrl_candidate = false;
    // A modifier already held when Shift arrives makes this a chord.
    // Ignore Caps/Num Lock; IBus includes Shift in the modifier mask for
    // the Shift key event itself.
    s.pure_shift_candidate =
        s.mode_shift_enabled && s.focused && !s.blocked && chord_modifiers == 0;
    return FALSE;
  }
  if (ctrl_key && (flags & IBUS_RELEASE_MASK)) {
    if (s.voice_requires_control && s.voice_hold_key &&
        (s.voice_hold_key == IBUS_Alt_R ? !s.right_ctrl_down : !s.ctrl_down)) {
      s.voice_hold_key = 0;
      s.voice_requires_control = false;
      s.pure_ctrl_candidate = false;
      if (s.voice_active && !s.voice_space_locked)
        guarded(engine, "voice_control_release", [&] { voice_stop(engine); });
      return FALSE;
    }
    if (!s.mode_ctrl_enabled || !s.pure_ctrl_candidate ||
        (chord_modifiers & ~IBUS_CONTROL_MASK) ||
        (flags & IBUS_SHIFT_MASK) ||
        g_get_monotonic_time() >= s.modifier_toggle_deadline) {
      s.pure_ctrl_candidate = false;
      return FALSE;
    }
    s.pure_ctrl_candidate = false;
    if (!s.focused || s.blocked)
      return FALSE;
    guarded(engine, "process_key", [&] { toggle_input_mode(engine); });
    // As with Shift: toggle, then let the application see the release.
    return FALSE;
  }
  if (ctrl_key && !(flags & IBUS_RELEASE_MASK)) {
    if (repeated_modifier) return FALSE;
    s.modifier_toggle_deadline = g_get_monotonic_time() + 500000;
    s.pure_shift_candidate = false;
    s.pure_ctrl_candidate =
        s.mode_ctrl_enabled && s.focused && !s.blocked &&
        (chord_modifiers & ~IBUS_CONTROL_MASK) == 0 &&
        (flags & IBUS_SHIFT_MASK) == 0;
    return FALSE;
  }
  // Own the complete consumed Space stroke. Modifier release order and
  // preference changes must not reinterpret repeats as another shortcut.
  if (key == IBUS_space && s.mode_chord_held) {
    if (release) s.mode_chord_held = false;
    return TRUE;
  }
  if ((key == IBUS_f || key == IBUS_F) && s.character_set_chord_held) {
    if (release) s.character_set_chord_held = false;
    return TRUE;
  }
  if (flags & IBUS_RELEASE_MASK) {
    if (key == IBUS_space && s.voice_space_consumed) {
      s.voice_space_consumed = false;
      return TRUE;
    }
    if (s.voice_consumed_keys.erase(key) != 0) {
      if (s.voice_hold_key == key) {
        s.voice_hold_key = 0;
        s.voice_requires_control = false;
        if (s.voice_active && !s.voice_space_locked)
          guarded(engine, "voice_hotkey_release", [&] { voice_stop(engine); });
      }
      return TRUE;
    }
    return FALSE;
  }
  s.pure_shift_candidate = false;
  s.pure_ctrl_candidate = false;
  const guint modifiers = canonical_modifiers(flags);
  const bool screen_keyboard_key =
      (key == IBUS_k || key == IBUS_K) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD4_MASK);
  if (!release && screen_keyboard_key && s.focused && !s.blocked) {
    if (!launch_desktop_panel("keyboard")) return FALSE;
    s.host_shortcut_strokes.insert(host_stroke);
    return TRUE;
  }
  const bool maintenance_restart_key =
      (key == IBUS_r || key == IBUS_R) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK);
  if (!release && maintenance_restart_key && s.focused && !s.blocked) {
    if (!restart_ibus_service()) return FALSE;
    s.host_shortcut_strokes.insert(host_stroke);
    return TRUE;
  }
  const bool maintenance_clear_cache_key =
      (key == IBUS_c || key == IBUS_C) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK);
  if (!release && maintenance_clear_cache_key && s.focused && !s.blocked) {
    s.host_shortcut_strokes.insert(host_stroke);
    guarded(engine, "reset_engine_cache", [&] {
      s.open();
      if (s.session)
        apply(engine, msime_client_reset_cache(s.session));
    });
    return TRUE;
  }
  const bool maintenance_exit_key =
      (key == IBUS_t || key == IBUS_T) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK);
  if (!release && maintenance_exit_key && s.focused && !s.blocked) {
    s.host_shortcut_strokes.insert(host_stroke);
    // Match the Windows maintenance shortcut: stop this user-owned IBus preview process without touching another IBus daemon or input source. main() turns the flag into msime_ibus_maintenance_stop_exit so the launcher's crash supervisor lets it stay stopped.
    maintenance_stop_requested = true;
    ibus_quit();
    return TRUE;
  }
  const bool dedicated_english_toggle =
      (key == IBUS_e || key == IBUS_E) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK);
  const bool ctrl_alt_space =
      key == IBUS_space &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_MOD1_MASK);
  const bool ctrl_space = key == IBUS_space && modifiers == IBUS_CONTROL_MASK;
  if ((ctrl_space && !s.mode_ctrl_space_enabled) ||
      (ctrl_alt_space && !s.mode_ctrl_alt_space_enabled))
    return FALSE;
  const bool mode_toggle = ctrl_space || ctrl_alt_space;
  const bool fullwidth_toggle = key == IBUS_space &&
                                modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK);
  const bool punctuation_toggle = modifiers == IBUS_CONTROL_MASK && key == IBUS_period;
  const bool character_set_chord =
      (key == IBUS_f || key == IBUS_F) &&
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK);
  if (character_set_chord && !s.character_set_shortcut_enabled)
    return FALSE;
  const bool character_set_toggle = character_set_chord;
  // Disabling IME spelling must not disable the system layout's Compose table.
  // GTK's asynchronous IBus passthrough does not perform dead-key composition.
  // A voice hold key must reach the voice path even with a dead key pending: X11/GDK/mutter send Alt_R with no modifier bits, and xkb_compose ignores modifier keysyms, so feeding it would swallow the press. Windows starts voice regardless of dead-key state.
  const bool voice_hotkey_match =
      s.voice_enabled && !s.voice_provider_socket.empty() && voice_hotkey(s, key, modifiers);
  if (s.focused && !s.blocked && !s.input_enabled && !release) {
    if (!voice_hotkey_match && (modifiers & ~(IBUS_SHIFT_MASK | IBUS_MOD5_MASK)) == 0) {
      if (const auto text = s.native_compose.feed(key)) {
        if (!text->empty()) commit_text(engine, *text);
        return TRUE;
      }
    } else {
      s.native_compose.reset();
    }
  }
  // English mode still honours fullwidth output, the "always Chinese punctuation" lock and a Ctrl+. choice made in English mode, as Windows does with the IME closed; everything else passes through.
  if (s.focused && !s.blocked && !s.input_enabled && !release &&
      (modifiers & ~(IBUS_SHIFT_MASK | IBUS_MOD5_MASK)) == 0) {
    const bool keypad = key >= IBUS_KP_Space && key <= IBUS_KP_9;
    const auto text = msime::linux_host::english_mode_output(
        ibus_keyval_to_unicode(key), keypad,
        s.punctuation_lock == "chinese" ||
            (s.punctuation_lock == "follow" && s.english_chinese_punctuation),
        s.fullwidth, s.english_punctuation);
    if (!text.empty()) {
      commit_text(engine, text, msime::linux_host::TypingSource::English);
      return TRUE;
    }
  }
  // Voice shortcuts, Esc during a recording and the hold-to-record Space lock stay live in English mode, as on Windows.
  const bool voice_key =
      (s.voice_enabled && !s.voice_provider_socket.empty() && voice_hotkey(s, key, modifiers)) ||
      (s.voice_active && (key == IBUS_Escape || (key == IBUS_space && s.voice_hold_key != 0)));
  if (!s.focused || s.blocked ||
      (!s.input_enabled && !mode_toggle && !fullwidth_toggle && !punctuation_toggle && !voice_key) ||
      (flags & IBUS_RELEASE_MASK))
    return FALSE;
  // Windows locks an active hold-to-record shortcut when Space is pressed.
  // IBus exposes the same interaction as key events; consume both halves of
  // the Space stroke so it cannot leak into the focused editor while voice
  // recognition is active. With the option disabled, Space follows the
  // regular editor/Engine path. Menu and Ctrl+F9 recordings have no held
  // shortcut, and recognition/polishing can no longer be locked. Use the
  // active hold captured on key-down: extra modifiers pressed afterwards
  // must not invalidate it. Releasing a required key clears voice_hold_key.
  if (s.voice_active && !s.voice_stopping && s.voice_hotkey_hold_space_lock &&
      key == IBUS_space && s.voice_hold_key != 0) {
    s.voice_space_consumed = true;
    if (!s.voice_space_locked) {
      guarded(engine, "voice_space_lock", [&] {
        s.voice_space_locked = true;
        render(engine, s.view);
        publish_mode(engine);
      });
    }
    return TRUE;
  }
  if (character_set_toggle) {
    if (!msime::linux_host::scheme::ScriptConversionApplies(
            msime::linux_host::scheme_number(effective_scheme(s))))
      return FALSE;
    if (menu_save_pending)
      return FALSE;
    s.character_set_chord_held = true;
    const bool next = !s.traditional_output;
    const auto directory = configured.value("preferences_directory", std::string{});
    if (!directory.empty() && directory.front() == '/') {
      save_menu_preference(engine, MenuPreference::TraditionalOutput, next);
      return TRUE;
    }
    guarded(engine, "toggle_character_set", [&] {
      s.open();
      if (!s.session)
        return;
      s.traditional_output = next;
      s.traditional_output_override = s.traditional_output;
      render(engine, s.view);
      publish_mode(engine);
    });
    return TRUE;
  }
  if (fullwidth_toggle) {
    s.mode_chord_held = true;
    s.fullwidth = !s.fullwidth;
    s.paired_tracker.clear();
    guarded(engine, "toggle_character_width", [&] {
      s.open();
      if (s.session) {
        s.view = response(msime_client_set_character_width(s.session, s.fullwidth));
        s.session_fullwidth = s.fullwidth;
        render(engine, s.view);
      }
      publish_mode(engine);
    });
    return TRUE;
  }
  const auto maintenance_candidate_slot =
      msime::linux_host::candidate_removal_slot(key, keycode);
  const bool maintenance_candidate_key =
      modifiers == (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK) &&
      maintenance_candidate_slot.has_value();
  if (maintenance_candidate_key) {
    const auto &candidates = s.rendered_candidates;
    const auto index = maintenance_candidate_slot;
    if (!index || s.rendered_session != s.session || !candidates.is_array() ||
        *index >= candidates.size() || !s.rendered_view.is_object() ||
        s.rendered_view.value("generation", uint64_t{0}) !=
            s.view.value("generation", uint64_t{0}))
      return FALSE;
    const auto &candidate = candidates.at(*index);
    if (!candidate.is_object() || !candidate.contains("id"))
      return FALSE;
    const auto &id = candidate.at("id");
    if (!id.is_object() || id.value("session", uint64_t{0}) != s.session)
      return FALSE;
    const auto source = candidate.value("source", 0);
    if (!msime::linux_host::candidate_dictionary_removal_available(
        s.rendered_scheme, source,
        candidate.value("text", std::string{})))
      return FALSE;
    guarded(engine, "remove_candidate_shortcut", [&] {
      apply(engine, msime_client_remove_candidate(
          s.session, id.at("generation").get<uint64_t>(),
          id.at("index").get<size_t>()));
    });
    return TRUE;
  }
  if (modifier(key) &&
      !(s.voice_enabled && !s.voice_provider_socket.empty() && voice_hotkey(s, key, modifiers)))
    return FALSE;
  if (key == IBUS_BackSpace) {
    if (s.last_smart_punctuation != 0) {
      s.smart_punctuation_rejected = s.last_smart_punctuation;
      s.last_smart_punctuation = 0;
      s.last_smart_punctuation_time = 0;
    }
  } else if (s.smart_punctuation_rejected != 0 &&
             s.smart_punctuation_rejected != static_cast<char>(key)) {
    s.smart_punctuation_rejected = 0;
  }
  // Only a Space arriving immediately after the mark, with nothing in between,
  // can take it back; anything else means the user moved on.
  if (!s.space_convert_mark.empty() && !(key == IBUS_space && modifiers == 0)) {
    s.space_convert_mark.clear();
    s.space_convert_preceding.clear();
  }
  bool handled = false;
  // 韩文输入半角 ASCII 标点，中文标点的各项辅助都不适用；它的字母是谚文字母，大小写只由 Shift 决定（见 msime_client.h 的 MsimeCommand 说明）。越南文和藏文在拉丁字母或藏文旁边输入同样的半角标点，注音则从 Engine 取中文标点，不用宿主的智能标点和成对标点辅助（`host_smart_punctuation`）。这四个方案在按键离开组字时都把组字写出去而不是丢弃（`commits_on_blur`），并让光标停在末尾（`locks_caret`）。专用英文模式在每个方案里都保持自己的规则。
  const auto active_input_scheme = effective_scheme(s);
  const int key_scheme = s.english_mode ? -1 : msime::linux_host::scheme_number(active_input_scheme);
  const bool korean_scheme = key_scheme == msime::linux_host::scheme::Korean;
  const bool zhuyin_scheme = key_scheme == msime::linux_host::scheme::Zhuyin;
  const bool vietnamese_scheme = key_scheme == msime::linux_host::scheme::Vietnamese;
  const bool tibetan_scheme = key_scheme == msime::linux_host::scheme::Tibetan;
  const bool commits_on_blur = korean_scheme || zhuyin_scheme || vietnamese_scheme || tibetan_scheme;
  const bool locks_caret = commits_on_blur;
  const bool narrow_scheme = korean_scheme || vietnamese_scheme || tibetan_scheme;
  const bool without_host_punctuation = korean_scheme || zhuyin_scheme || vietnamese_scheme || tibetan_scheme;
  const auto fullwidth_idle_commit = [&](guint value) {
    if (!s.fullwidth || narrow_scheme || value < 0x21 || value > 0x7e)
      return false;
    const auto editing_text = s.view.value("editing_text", std::string{});
    const auto candidates = s.view.value("candidates", Json::array());
    if (!editing_text.empty() || (candidates.is_array() && !candidates.empty()))
      return false;
    auto text = fullwidth_text(std::string(1, static_cast<char>(value)));
    commit_text(engine, text);
    return true;
  };
  guarded(engine, "process_key", [&] {
    s.open();
    const bool bare_backspace = key == IBUS_BackSpace && modifiers == 0;
    if (!bare_backspace) {
      s.backspace_hold.reset();
    } else {
      const bool composing =
          !s.view.value("editing_text", std::string{}).empty() ||
          !s.view.value("candidates", Json::array()).empty();
      if (s.backspace_hold.press(composing)) {
        handled = true;
        return;
      }
    }
    if (s.translation_candidates_active) {
      const auto page_size = std::clamp(
          s.translation_saved_view.value("page_size", size_t{9}), size_t{1},
          size_t{9});
      const auto page_count = (s.translation_options.size() + page_size - 1) /
                              page_size;
      const auto no_modifiers = modifiers == 0;
      if (modifiers == IBUS_CONTROL_MASK &&
          (key == IBUS_Return || key == IBUS_KP_Enter)) {
        handled = true;
        return;
      }
      if (no_modifiers && (key == IBUS_space ||
                          (key >= '1' && key <= '9') ||
                          (key >= IBUS_KP_1 && key <= IBUS_KP_9))) {
        const auto slot = key == IBUS_space
                              ? s.translation_cursor
                              : static_cast<size_t>(
                                    key >= IBUS_KP_1
                                        ? key - IBUS_KP_1
                                        : key - '1');
        const auto index = s.translation_page * page_size + slot;
        if (index < s.translation_options.size())
          handled = commit_translation_candidate(engine, index);
        else
          handled = true;
        return;
      }
      if (no_modifiers && (key == IBUS_Up || key == IBUS_KP_Up ||
                           key == IBUS_Down || key == IBUS_KP_Down)) {
        const auto page_start = s.translation_page * page_size;
        const auto page_end = std::min(page_start + page_size,
                                       s.translation_options.size());
        if (key == IBUS_Up || key == IBUS_KP_Up)
          s.translation_cursor = s.translation_cursor == 0
                                    ? s.translation_cursor
                                    : s.translation_cursor - 1;
        else if (s.translation_cursor + 1 < page_end - page_start)
          ++s.translation_cursor;
        render_translation_candidates(engine);
        handled = true;
        return;
      }
      // With the shared navigation.tab binding enabled, Tab and Shift+Tab page the senses exactly as they page an ordinary candidate list. With it disabled Tab keeps leaving the temporary page below.
      const bool tab_page =
          s.navigation.tab && (modifiers & ~IBUS_SHIFT_MASK) == 0 &&
          (key == IBUS_Tab || key == IBUS_KP_Tab || key == IBUS_ISO_Left_Tab);
      if (tab_page ||
          (no_modifiers && (key == IBUS_Page_Up || key == IBUS_KP_Page_Up ||
                            key == IBUS_Page_Down || key == IBUS_KP_Page_Down))) {
        const bool previous =
            tab_page ? (modifiers & IBUS_SHIFT_MASK) != 0 ||
                           key == IBUS_ISO_Left_Tab
                     : key == IBUS_Page_Up || key == IBUS_KP_Page_Up;
        if (previous)
          s.translation_page = s.translation_page == 0
                                   ? 0
                                   : s.translation_page - 1;
        else if (s.translation_page + 1 < page_count)
          ++s.translation_page;
        s.translation_cursor = 0;
        render_translation_candidates(engine);
        handled = true;
        return;
      }
      // Escape and every other key leave the temporary page first, then use
      // the normal Engine path so composition cancellation/editing semantics
      // stay identical to an ordinary candidate page.
      exit_translation_candidates(engine);
    }
    const auto &active_scheme = active_input_scheme;
    // Match the configured Windows Japanese mode, not temporary R mode: the
    // physical minus key extends romaji and -/= stop acting as page keys.
    const bool japanese_scheme = active_scheme == "japanese";
    const bool japanese_long_vowel =
        japanese_scheme && key == IBUS_minus && modifiers == 0;
    const bool japanese_minus_equal = japanese_scheme && modifiers == 0 &&
                                      (key == IBUS_minus || key == IBUS_equal);
    if (dedicated_english_toggle) {
      if (!s.session)
        return;
      const bool enabled = !s.english_mode;
      s.view = response(msime_client_set_english_mode(s.session, enabled));
      s.english_mode = enabled;
      s.dedicated_english_override = enabled;
      render(engine, s.view);
      publish_mode(engine);
      handled = true;
      return;
    }
    if (mode_toggle) {
      // Plain Ctrl+Space owns its stroke too: its auto-repeat used to flip the input mode on every repeat.
      s.mode_chord_held = true;
      toggle_input_mode(engine);
      handled = true;
      return;
    }
    if (!voice_hotkey_match && (modifiers & ~(IBUS_SHIFT_MASK | IBUS_MOD5_MASK)) == 0) {
      if (const auto text = s.native_compose.feed(key)) {
        if (!s.view.value("editing_text", std::string{}).empty() ||
            !s.view.value("candidates", Json::array()).empty())
          apply(engine, msime_client_command(s.session, MSIME_COMMIT_RAW));
        if (!text->empty()) commit_text(engine, *text);
        handled = true;
        return;
      }
    } else {
      s.native_compose.reset();
    }
    if (voice_hotkey(s, key, modifiers) && s.voice_enabled &&
        !s.voice_provider_socket.empty()) {
      // Windows keeps one active hold chord; another hold shortcut cannot
      // replace it. Ctrl+F9 has an independent consumed-key lifetime.
      if (key != IBUS_F9 && s.voice_hold_key != 0)
        return;
      s.voice_consumed_keys.insert(key);
      // Hold shortcuts start/continue recording; only a locked recording
      // turns their next press into Stop. Ctrl+F9 always toggles recording.
      if (s.voice_active) {
        if (key == IBUS_F9 || s.voice_space_locked)
          voice_stop(engine);
      } else {
        voice_start(engine);
      }
      // A hold chord may take over a recording started from the menu or
      // Ctrl+F9. Releasing its Ctrl must stop just like releasing Win/RAlt.
      if (key != IBUS_F9) {
        s.voice_hold_key = key;
        s.voice_requires_control = (modifiers & IBUS_CONTROL_MASK) != 0;
      }
      handled = true;
      return;
    }
    if (key == IBUS_Escape && s.voice_active) {
      voice_cancel(engine);
      handled = true;
      return;
    }
    // Windows eats Ctrl+. with the IME closed too and flips its punctuation compartment, which a pinned lock holds in place. The choice is session-only: it is never saved, and the next Chinese/English switch (resync_punctuation_for_mode) undoes it.
    if (punctuation_toggle && !s.input_enabled) {
      if (s.punctuation_lock == "follow") {
        s.english_chinese_punctuation = !s.english_chinese_punctuation;
        s.chinese_punctuation = s.english_chinese_punctuation;
        s.punctuation_override = s.chinese_punctuation;
        if (s.session) {
          s.view = response(msime_client_set_chinese_punctuation(
              s.session, s.chinese_punctuation));
          s.session_chinese_punctuation = s.chinese_punctuation;
        }
        publish_mode(engine);
      }
      handled = true;
      return;
    }
    if (!s.input_enabled)
      return;
    if (!s.view.at("focused").get<bool>())
      apply(engine, msime_client_focus(s.session, true));
    // A key the active local mode or scheme spells with is input before any binding below can claim it: a page key, a paired bracket, smart punctuation or a candidate digit (SpellingSymbols.h). Space is one of them only while a Zhuyin syllable composes, where it is the first tone.
    if ((modifiers & ~IBUS_SHIFT_MASK) == 0) {
      const gunichar spelled = ibus_keyval_to_unicode(key);
      if (msime::linux_host::engine_spelling(s.view, spelled) ||
          (modifiers == 0 && spelled == U' ' && msime::linux_host::spelling_space(s.view))) {
        handled = apply(engine, msime_client_character(
                                    s.session, static_cast<uint8_t>(spelled),
                                    (flags & IBUS_SHIFT_MASK) != 0));
        return;
      }
    }
    if (try_skip_paired_closing(engine, key, flags)) {
      handled = true;
      return;
    }
    // Hangul_Hanja, or a bare F9, converts the composing Korean syllable to Hanja, the keys of ibus-hangul and fcitx5-hangul; pressed again with the list open it closes it (msime_client.h, MSIME_OPEN_CANDIDATE_LIST). A composing Zhuyin conversion opens its candidate list with the same keys. Ctrl+F9 is the voice toggle above. While a composition is open the key stays the input method's whatever the Engine answers: a lone jamo has no Hanja, and the end of this function would write the composition out and hand the key to the application. With nothing composing it is the application's as before.
    if (modifiers == 0 && msime::linux_host::korean_hanja_key(key) &&
        msime::linux_host::candidate_list_composition(s.view)) {
      apply(engine, msime_client_command(s.session, MSIME_OPEN_CANDIDATE_LIST));
      handled = true;
      return;
    }
    // Down opens the list of a composing Zhuyin conversion, as in libchewing, before the navigation binding below can treat it as a key with no list to move in.
    if (modifiers == 0 && (key == IBUS_Down || key == IBUS_KP_Down) &&
        msime::linux_host::zhuyin_list_down_key(s.view)) {
      apply(engine, msime_client_command(s.session, MSIME_OPEN_CANDIDATE_LIST));
      handled = true;
      return;
    }
    // With its Hanja list open a Korean syllable has candidates, which the candidate keys below act on as for any list; so does a Zhuyin conversion with its list open.
    const bool korean_hanja_list = msime::linux_host::korean_hanja_list_open(s.view);
    const bool opened_list = msime::linux_host::opened_candidate_list(s.view);
    if (key == IBUS_BackSpace || key == IBUS_Delete || key == IBUS_KP_Delete ||
        key == IBUS_Return || key == IBUS_KP_Enter || key == IBUS_Escape ||
        key == IBUS_Left || key == IBUS_KP_Left || key == IBUS_Right ||
        key == IBUS_KP_Right || key == IBUS_Up || key == IBUS_KP_Up ||
        key == IBUS_Down || key == IBUS_KP_Down || key == IBUS_Home ||
        key == IBUS_KP_Home || key == IBUS_End || key == IBUS_KP_End ||
        key == IBUS_Page_Up || key == IBUS_KP_Page_Up ||
        key == IBUS_Page_Down || key == IBUS_KP_Page_Down || key == IBUS_Tab ||
        key == IBUS_KP_Tab || key == IBUS_ISO_Left_Tab)
      s.paired_tracker.clear();
    // 与 Windows TSF 一致：CapsLock 打开时，新组字开头的大写字母属于编辑器。IBus 在修饰键掩码里给出锁定状态并保留大写 keysym，所以这一击原样放过，不开始拼音组字。韩文按键不论 CapsLock 如何都是谚文字母，越南文单词可以以大写开头，藏文威利转写的大写字母是另一个字母，所以这三个方案仍然组字。
    if (!(key_scheme >= 0 && msime::linux_host::scheme::CapsLockBypassExempt(key_scheme)) && (flags & IBUS_LOCK_MASK) && key >= 'A' && key <= 'Z' &&
        s.view.at("editing_text").get<std::string>().empty() &&
        s.view.at("candidates").empty())
      return;
    // Apply configured candidate bindings before punctuation can consume them. The marks among them stay punctuation while a Korean Hanja list is open, as they are with no list (KoreanHanja.h): the Engine closes the list and writes the Hangul with the mark. Tab, Page Up/Down and the arrows still page and move.
    const bool korean_hanja_mark =
        korean_hanja_list && msime::linux_host::korean_hanja_punctuation_key(key);
    if ((modifiers & ~IBUS_SHIFT_MASK) == 0 &&
        !s.view.at("candidates").empty() && !korean_hanja_mark) {
      if (!japanese_long_vowel) {
        if (const auto edge =
                s.word_character.edge(key, (flags & IBUS_SHIFT_MASK) != 0)) {
          // Edge selection is an identity-bearing action. Resolve the
          // highlighted candidate from the page actually handed to IBus,
          // rather than a newer live view that may still be awaiting redraw.
          const bool rendered_current =
              s.rendered_session == s.session && s.rendered_view.is_object() &&
              s.rendered_candidates.is_array() &&
              !s.rendered_candidates.empty() &&
              s.rendered_view.value("generation", uint64_t{0}) ==
                  s.view.value("generation", uint64_t{0});
          if (rendered_current) {
            for (const auto &candidate : s.rendered_candidates) {
              if (!candidate.is_object() ||
                  !candidate.value("highlighted", false))
                continue;
              const auto &id = candidate.value("id", Json::object());
              if (!id.is_object() ||
                  id.value("session", uint64_t{0}) != s.session)
                return;
              handled = apply(engine,
                              msime_client_select_edge(
                                  s.session, id.value("generation", uint64_t{0}),
                                  id.value("index", size_t{0}), *edge));
              if (!handled)
                handled =
                    apply(engine, msime_client_punctuation(
                                      s.session, static_cast<uint8_t>(key)));
              return;
            }
          }
        }
      }
      if (!japanese_minus_equal) {
        if (const auto navigation =
                s.navigation.command(key, (flags & IBUS_SHIFT_MASK) != 0)) {
          handled = apply(engine, msime_client_command(s.session, *navigation));
          return;
        }
      }
    }
    if (!s.view.at("candidates").empty()) {
      if (const auto touch_navigation =
              msime::linux_host::touch_keyboard_command(key)) {
        // Touch-keyboard page events carry no generation. Keep them aligned
        // with the page handed to IBus, just like wheel and native page
        // callbacks, so a delayed event cannot page a newer live view.
        if (s.rendered_session != s.session || !s.rendered_view.is_object() ||
            !s.rendered_candidates.is_array() ||
            s.rendered_candidates.empty() ||
            s.rendered_view.value("generation", uint64_t{0}) !=
                s.view.value("generation", uint64_t{0}))
          return;
        handled = apply(engine, msime_client_command(
                                   s.session, *touch_navigation));
        return;
      }
    }
    // Match the Windows composition editor for the three Ctrl-only segment
    // edits. The shared runtime owns the segment boundaries (and falls back
    // to one raw character for local modes), so the Linux host only has to
    // preserve the editor's modifier boundary and route the action. With no
    // active composition these remain ordinary application shortcuts.
    const bool ctrl_only = modifiers == IBUS_CONTROL_MASK &&
                           !(flags & IBUS_RELEASE_MASK);
    const bool segment_edit_key = key == IBUS_BackSpace ||
                                  key == IBUS_Left || key == IBUS_KP_Left ||
                                  key == IBUS_Right || key == IBUS_KP_Right;
    const auto active_editing = s.view.value("editing_text", std::string{});
    const auto active_candidates = s.view.value("candidates", Json::array());
    // 韩文音节、注音转换、越南文单词和藏文音节串都没有分段：Ctrl 组合键在下面结束它，并仍然作为应用的快捷键。
    if (!locks_caret && ctrl_only && segment_edit_key &&
        (!active_editing.empty() ||
         (active_candidates.is_array() && !active_candidates.empty()))) {
      const uint32_t segment_command =
          key == IBUS_BackSpace
              ? MSIME_BACKSPACE_SEGMENT
              : (key == IBUS_Left || key == IBUS_KP_Left
                     ? MSIME_MOVE_LEFT_SEGMENT
                     : MSIME_MOVE_RIGHT_SEGMENT);
      handled = apply(engine,
                      msime_client_command(s.session, segment_command));
      return;
    }
    // Windows uses Ctrl+Enter to commit the highlighted candidate's
    // translation. A single gloss commits directly; multiple senses become a
    // temporary IBus-native candidate page backed by the saved Engine view.
    // The rendered identity fence prevents a delayed translation from being
    // committed for a newer page than the user saw.
    if (ctrl_only && (key == IBUS_Return || key == IBUS_KP_Enter) &&
        s.candidate_translations && s.rendered_session == s.session &&
        s.rendered_candidates.is_array() && !s.rendered_candidates.empty() &&
        s.rendered_view.is_object() &&
        s.rendered_view.value("generation", uint64_t{0}) ==
            s.view.value("generation", uint64_t{0})) {
      for (const auto &candidate : s.rendered_candidates) {
        if (!candidate.is_object() || !candidate.value("highlighted", false))
          continue;
        const auto translation = candidate.value("translation", std::string{});
        if (translation.empty() || translation.size() > 4096)
          break;
        const auto senses = msime::linux_host::split_translation_gloss(translation);
        if (senses.empty())
          break;
        if (senses.size() == 1) {
          commit_text(engine, senses.front());
          (void)apply(engine, msime_client_command(s.session, MSIME_CANCEL));
          handled = true;
          return;
        }
        s.translation_saved_view = s.view;
        s.translation_options = senses;
        s.translation_candidates_active = true;
        s.translation_page = 0;
        s.translation_cursor = 0;
        render_translation_candidates(engine);
        handled = true;
        return;
      }
    }
    // Disabled navigation keys belong to the application, including when a
    // composition is active. Finalize that composition first so the editor
    // never receives a navigation key while stale preedit is still owned by
    // the IBus engine.
    // Only bare or Shift-modified navigation keys take this path. A Ctrl/Alt/Super/AltGr chord (Ctrl+Tab, Ctrl+PageDown) is an application shortcut and falls through to the generic modifier branch below, which cancels the composition (or finishes a Korean syllable) before forwarding it, matching the Fcitx5 host.
    if ((modifiers & ~IBUS_SHIFT_MASK) == 0 &&
        msime::linux_host::navigation_key(key)) {
      const bool binding_enabled = s.navigation.command(
          key, (flags & IBUS_SHIFT_MASK) != 0).has_value();
      // 韩文音节除非打开了汉字列表，否则没有候选页可供绑定操作，所以这个键总是结束它并交给应用。列表打开时，已启用的绑定在上面已经处理；未启用的和任何列表一样属于应用，FINISH 写出谚文，而候选命令会写出高亮的汉字。注音转换、越南文单词和藏文音节串也这样结束：FINISH 写出组字内容，从不写候选；藏文若改发候选命令会多带一个音节点。
      if ((!binding_enabled || commits_on_blur) &&
          (!s.view.at("editing_text").get<std::string>().empty() ||
           !s.view.at("candidates").empty()))
        apply(engine, msime_client_command(s.session, korean_hanja_list || zhuyin_scheme || vietnamese_scheme || tibetan_scheme
                                                          ? MSIME_FINISH_COMPOSITION
                                                          : MSIME_COMMIT_CANDIDATE));
      return;
    }
    if (modifiers == IBUS_CONTROL_MASK && key == IBUS_period) {
      // A pinned lock holds here too, as Windows resolves Ctrl+. through ResolvePunctuationOpen: the chord is eaten and changes nothing.
      if (s.punctuation_lock != "follow") {
        handled = true;
        return;
      }
      s.chinese_punctuation = !s.chinese_punctuation;
      s.punctuation_override = s.chinese_punctuation;
      s.view = response(msime_client_set_chinese_punctuation(
          s.session, s.chinese_punctuation));
      s.session_chinese_punctuation = s.chinese_punctuation;
      render(engine, s.view);
      publish_mode(engine);
      handled = true;
      return;
    }
    // AltGr selects layout text, not an application shortcut. Finish spelling
    // before forwarding it, without applying candidate or punctuation bindings.
    if ((modifiers & ~IBUS_SHIFT_MASK) == IBUS_MOD5_MASK &&
        g_unichar_isprint(ibus_keyval_to_unicode(key))) {
      apply(engine, msime_client_command(s.session, MSIME_COMMIT_RAW));
      return;
    }
    if (flags &
        (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK |
         IBUS_META_MASK | IBUS_HYPER_MASK | IBUS_MOD5_MASK)) {
      // 韩文音节、注音转换、越南文单词或藏文音节串已经是文字，所以快捷键结束它而不是丢弃它。
      if (commits_on_blur && !s.view.at("editing_text").get<std::string>().empty())
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      else
        apply(engine, msime_client_command(s.session, MSIME_CANCEL));
      return;
    }
    if (s.number_row_selection && !s.view.value("nine_key", false) &&
        s.rendered_session == s.session && s.rendered_candidates.is_array() &&
        !s.rendered_candidates.empty()) {
      if (const auto index =
              candidate_digit_slot(key, keycode, flags, s.rendered_view)) {
        // A digit past the end of a Hanja or Zhuyin page picks nothing and is swallowed, as the runtime swallows it, rather than typed beside the open composition.
        if (*index >= s.rendered_candidates.size()) {
          handled = opened_list;
          return;
        }
        const auto &candidate = s.rendered_candidates.at(*index);
        const auto &id = candidate.at("id");
        if (id.at("session").get<uint64_t>() != s.session) return;
        handled = apply(engine, msime_client_select(
            s.session, id.at("generation").get<uint64_t>(),
            id.at("index").get<size_t>()));
        return;
      }
    }
    // Nothing rendered yet is not an ordinary candidate digit, and asking the
    // rendered view about it is what broke: `rendered_view` is null until the
    // first render, and reading it came before the identity fences that would
    // have stopped short of it. Every session rebuild - a Chinese/English
    // toggle, a menu override with no shared preferences directory to save
    // into, a scheme change - resets it, so the next key threw, was swallowed by
    // `guarded`, and was lost.
    const bool ordinary_candidate_digit =
        !s.english_mode && s.rendered_session == s.session &&
        s.rendered_candidates.is_array() && !s.rendered_candidates.empty() &&
        s.rendered_view.is_object() &&
        s.rendered_view.value("local_mode", "none") == "none" &&
        !s.rendered_view.value("nine_key", false) &&
        ((key >= IBUS_0 && key <= IBUS_9) ||
         (key >= IBUS_KP_0 && key <= IBUS_KP_9));
    if (ordinary_candidate_digit &&
        (!s.number_row_selection || modifiers != 0)) {
      // The shared runtime's character action has a legacy numeric fallback that selects candidates. Keep that fallback behind the Linux host toggle, and never turn shifted digits into candidate selection.
      // A digit released while a Korean Hanja list or a Zhuyin list is open ends the composition first, as a digit does with no Hanja list: the composition is written and the digit follows it, rather than the digit landing in the document before a composition and list left hanging.
      if (opened_list)
        apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      return;
    }
    if (const auto keypad = keypad_punctuation(key)) {
      const auto &editing_text = s.view.at("editing_text").get<std::string>();
      const auto &candidates = s.view.at("candidates");
      const bool has_composition = !editing_text.empty() ||
                                   (candidates.is_array() && !candidates.empty());
      if (*keypad == '.' || has_composition) {
        handled = apply(engine, msime_client_punctuation_ascii(
            s.session, static_cast<uint8_t>(*keypad)));
        if (!handled && *keypad == '.') {
          auto text = std::string(".");
          if (s.fullwidth && !narrow_scheme)
            text = fullwidth_text(text);
          commit_text(engine, text);
          handled = true;
        }
      } else {
        // Arithmetic keypad marks keep the normal Engine punctuation policy
        // while remaining outside the configurable minus/equal paging keys.
        handled = apply(engine, msime_client_punctuation(
            s.session, static_cast<uint8_t>(*keypad)));
      }
      if (!handled)
        handled = fullwidth_idle_commit(static_cast<guint>(*keypad));
      return;
    }
    if (microsoft_shuangpin_ing_key(s.view, key, modifiers)) {
      handled = apply(engine, msime_client_character(s.session, ';', false));
      return;
    }
    if (unicode_plus_key(s.view, key, modifiers)) {
      handled = apply(engine, msime_client_character(s.session, '+', true));
      return;
    }
    if (japanese_long_vowel) {
      handled = apply(engine, msime_client_character(s.session, '-', false));
      return;
    }
    const auto &editing_text = s.view.at("editing_text").get<std::string>();
    const bool paired_punctuation_enabled =
        s.paired_punctuation &&
        !msime::linux_host::paired_punctuation_excluded_client(
            s.focused_client);
    if (!without_host_punctuation && s.chinese_punctuation && paired_punctuation_enabled &&
        !(flags & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_SUPER_MASK)) &&
        (key == IBUS_quotedbl ||
         (key == IBUS_apostrophe && editing_text.empty()))) {
      const auto pair_mode = key == IBUS_quotedbl
                                 ? PunctuationPairMode::DoubleQuote
                                 : PunctuationPairMode::SingleQuote;
      handled = apply(engine, msime_client_punctuation(
                                   s.session, static_cast<uint8_t>(key)),
                               pair_mode);
      if (!handled)
        handled = fullwidth_idle_commit(key);
      if (handled)
        ibus_engine_forward_key_event(engine, IBUS_Left, 0, 0);
      return;
    }
    if (!without_host_punctuation && s.chinese_punctuation && paired_punctuation_enabled &&
        !(flags & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_SUPER_MASK)) &&
        (key == '(' || key == '[' || key == '<' || key == '{')) {
      const auto pair_mode = key == '{' ? PunctuationPairMode::Brace
                                        : PunctuationPairMode::Bracket;
      const bool engine_handled = apply(
          engine,
          key == '{'
              ? msime_client_punctuation_ascii(s.session, static_cast<uint8_t>(key))
              : msime_client_punctuation(s.session, static_cast<uint8_t>(key)),
          pair_mode);
      handled = engine_handled;
      if (engine_handled && key == '<')
        (void)response(msime_client_balance_paired_punctuation_after_auto_close(
            s.session, static_cast<uint8_t>(key)));
      if (!handled && key == '{') {
        auto text = std::string("{}");
        if (s.fullwidth)
          text = fullwidth_text(std::move(text));
        commit_text(engine, text);
        s.paired_tracker.push(s.fullwidth ? "｝" : "}");
        handled = true;
      }
      if (!handled)
        handled = fullwidth_idle_commit(key);
      if (handled)
        ibus_engine_forward_key_event(engine, IBUS_Left, 0, 0);
      return;
    }
    // 记下的是一个 ASCII 标点字符，键值是无符号的。char 在此平台有符号，直接比较既
    // 触发 -Werror=sign-compare（新编译器上整个 IBus 宿主因此编不出来），也会让任何
    // 高位为 1 的字节提升成一个巨大的无符号数去和键值比。按 unsigned char 取值。
    if (!without_host_punctuation && s.chinese_punctuation && s.smart_punctuation_repeat &&
        static_cast<guint>(static_cast<unsigned char>(s.last_smart_punctuation)) == key &&
        s.last_smart_punctuation_time != 0 &&
        g_get_monotonic_time() - s.last_smart_punctuation_time <=
            kSmartPunctuationRepeatIntervalUs &&
        smart_punctuation_repeat_matches_document(s) &&
        s.view.at("editing_text").get<std::string>().empty()) {
      if (const auto *replacement = smart_punctuation_pair(static_cast<char>(key))) {
        ibus_engine_delete_surrounding_text(engine, -1, 1);
        // The preceding mark was replaced in the editor, not appended.
        if (!s.ai_context.empty()) {
          size_t last = s.ai_context.size() - 1;
          while (last > 0 &&
                 (static_cast<unsigned char>(s.ai_context[last]) & 0xc0) == 0x80)
            --last;
          s.ai_context.erase(last);
        }
        commit_text(engine, replacement);
        s.last_smart_punctuation = 0;
        s.last_smart_punctuation_time = 0;
        handled = true;
        return;
        }
    }
    if (!without_host_punctuation && s.chinese_punctuation && s.smart_punctuation &&
        is_smart_punctuation_key(key) &&
        s.smart_punctuation_rejected != static_cast<char>(key) &&
        smart_punctuation_preceded_by_ascii_alphanumeric(s)) {
      const auto &editing_text = s.view.at("editing_text").get<std::string>();
      const auto &candidates = s.view.at("candidates");
      const bool has_composition = !editing_text.empty() ||
                                   (candidates.is_array() && !candidates.empty());
      if (has_composition) {
        handled = apply(engine, msime_client_punctuation_ascii(
                                  s.session, static_cast<uint8_t>(key)));
      } else {
        std::string text(1, static_cast<char>(key));
        if (s.fullwidth)
          text = fullwidth_text(text);
        commit_text(engine, text);
        s.last_smart_punctuation = static_cast<char>(key);
        s.last_smart_punctuation_time = g_get_monotonic_time();
        handled = true;
      }
      return;
    }
    const bool has_composition =
        !s.view.at("editing_text").get<std::string>().empty();
    const bool candidate_active =
        s.view.at("candidates").is_array() && !s.view.at("candidates").empty();
    const auto local_mode = s.view.value("local_mode", std::string("none"));
    // Japanese decides Space and Enter differently; the reading is what a conversion belongs to,
    // and what Enter commits when no conversion was started. See core/JapaneseConversion.h.
    const bool japanese_composition = s.view.value("scheme", 0) == 3;
    const auto japanese_reading = s.view.value("editing_text", std::string{});
    const bool lowercase_letter =
        (key >= 'a' && key <= 'z');
    const bool uppercase_letter =
        (key >= 'A' && key <= 'Z');
    const bool helpcode =
        (active_scheme == "quanpin" || active_scheme == "shuangpin") &&
        s.helpcode_override.value_or(
            configured.at("preferences")
                .value(active_scheme + "_helpcode", Json::object())
                .value("enabled", true));
    const bool accepted_letter =
        local_mode == "quick_phrase"
            ? lowercase_letter
            : local_mode == "unicode"
                  ? ((key >= 'a' && key <= 'f') ||
                     (key >= 'A' && key <= 'F'))
                  : local_mode == "date_time"
                        ? lowercase_letter
                        : local_mode != "none"
                              ? lowercase_letter || uppercase_letter
                              : lowercase_letter ||
                                    (uppercase_letter &&
                                     (helpcode || s.english_mode));
    const bool spelling_digits = msime::linux_host::spelling_digits(s.view);
    const bool nine_key_digit =
        !spelling_digits && s.view.value("nine_key", false) &&
        ((key >= IBUS_KP_2 && key <= IBUS_KP_9) ||
         (key >= '2' && key <= '9'));
    const bool spelling_digit =
        spelling_digits && key >= '0' && key <= '9' &&
        (modifiers & IBUS_SHIFT_MASK) == 0;
    const bool microsoft_ing =
        microsoft_shuangpin_ing_key(s.view, key, modifiers);
    const bool unicode_plus = unicode_plus_key(s.view, key, modifiers);
    const auto caret_position = s.view.value(
        "caret_position", s.view.value("editing_text", std::string{}).size());
    const bool accepted_apostrophe =
        key == IBUS_apostrophe && has_composition && caret_position != 0 &&
        ((local_mode == "none" && active_scheme != "wubi" && active_scheme != "zhuyin" &&
          active_scheme != "vietnamese" && active_scheme != "tibetan") ||
         local_mode == "emoji" || local_mode == "kaomoji" ||
         local_mode == "temporary_japanese");
    const bool candidate_input =
        candidate_active &&
        (accepted_letter || nine_key_digit || spelling_digit || microsoft_ing ||
         unicode_plus || accepted_apostrophe);
    if (candidate_input) {
      // Candidate visibility does not end composition. Engine owns how the
      // next spelling key extends the current input or local mode.
      if (microsoft_ing) {
        handled = apply(engine, msime_client_character(
                                   s.session, ';', false));
      } else if (unicode_plus) {
        handled = apply(engine, msime_client_character(
                                   s.session, '+', true));
      } else if (key >= IBUS_KP_0 && key <= IBUS_KP_9) {
        handled = apply(engine, msime_client_character(
                                   s.session,
                                   static_cast<uint8_t>('0' + key - IBUS_KP_0),
                                   false));
      } else {
        const auto input_value =
            (flags & IBUS_SHIFT_MASK) && key >= 'a' && key <= 'z'
                ? key - 'a' + 'A'
                : key;
        handled = apply(engine, msime_client_character(
            s.session, static_cast<uint8_t>(input_value),
            (flags & IBUS_SHIFT_MASK) != 0));
      }
      return;
    }
    const char ascii = static_cast<char>(key);
    // 拼写中的撇号是 Engine 的输入字符，但韩文、注音和越南文除外：在这些方案里它和别的标点一样跟在打开的组字后面。藏文的撇号列在 spelling_symbols 里，在前面已经作为字符交给 Engine。
    if (key >= 0x21 && key <= 0x7e &&
        std::ispunct(static_cast<unsigned char>(ascii)) != 0 &&
        (ascii != '\'' || !has_composition || commits_on_blur)) {
      // Engine is about to commit the Chinese mark for this key. Record what
      // the caret follows now, while the document still predates the commit;
      // a Space arriving next checks both characters before rewriting either.
      const bool arm_space_convert =
          !without_host_punctuation && s.smart_punctuation && s.smart_punctuation_space_convert &&
          s.chinese_punctuation &&
          msime::linux_host::is_space_conversion_key(ascii) &&
          !has_composition && !candidate_active;
      std::string armed_preceding;
      if (arm_space_convert) {
        const auto preceding = surrounding_preceding_characters(s, 1);
        if (preceding && !preceding->empty())
          armed_preceding = preceding->front();
      }
      std::optional<std::string> space_convert_preceding;
      if (arm_space_convert)
        space_convert_preceding = armed_preceding;
      handled = apply(
          engine,
          msime_client_punctuation(s.session, static_cast<uint8_t>(ascii)),
          PunctuationPairMode::Unpaired, std::move(space_convert_preceding));
      if (!handled)
        handled = fullwidth_idle_commit(key);
      if (is_smart_punctuation_key(key))
        s.smart_punctuation_rejected = 0;
      return;
    }
    if (!s.view.at("candidates").empty() &&
        (key == IBUS_Home || key == IBUS_KP_Home || key == IBUS_End ||
         key == IBUS_KP_End)) {
      const auto command = (key == IBUS_Home || key == IBUS_KP_Home)
                               ? MSIME_FIRST_CANDIDATE
                               : MSIME_LAST_CANDIDATE;
      handled = apply(engine, msime_client_command(s.session, command));
      return;
    }
    // Settle Space (and Return in a Korean Hanja or Zhuyin list) against the candidate page most recently handed to the IBus panel. Engine may have rebuilt or reordered its live view while the panel was still processing the previous update; selecting by the rendered candidate identity keeps the key aligned with what the user was shown, just like the Windows painted-page selection fence.
    const auto select_rendered_highlight = [&] {
      if (!candidate_active || s.rendered_session != s.session ||
          !s.rendered_candidates.is_array() || s.rendered_candidates.empty())
        return false;
      for (const auto &candidate : s.rendered_candidates) {
        if (!candidate.is_object() || !candidate.value("highlighted", false))
          continue;
        const auto &id = candidate.value("id", Json::object());
        if (!id.is_object() || id.value("session", uint64_t{0}) != s.session)
          return false;
        return apply(engine, msime_client_select(
            s.session, id.value("generation", uint64_t{0}),
            id.value("index", size_t{0})));
      }
      return false;
    };
    uint32_t command = UINT32_MAX;
    switch (key) {
    case IBUS_BackSpace:
      // Incremental candidates remain visible while Engine edits spelling.
      command = MSIME_BACKSPACE;
      break;
    case IBUS_Return:
    case IBUS_KP_Enter:
      // With a Korean Hanja list or a Zhuyin list open Return chooses the highlighted candidate, as Space does; otherwise it writes the composition out and breaks the line.
      if (opened_list) {
        if (select_rendered_highlight()) {
          handled = true;
          return;
        }
        command = MSIME_COMMIT_CANDIDATE;
        break;
      }
      // Japanese commits the kana, or the conversion the user stepped to with Space. Sending the
      // raw-input command here - which every scheme used to do - commits the romaji.
      if (japanese_composition && has_composition) {
        using Action = msime::linux_host::JapaneseConversion::Action;
        const auto action = s.japanese_conversion.enter(japanese_reading);
        if (action == Action::CommitCandidate) {
          // Select by the identity of the candidate that is on screen, the same fence Space uses
          // below: the live view may already be a generation ahead of what the user is looking at.
          const auto &candidates = s.view.at("candidates");
          const auto index = s.japanese_conversion.index();
          if (candidates.is_array() && index < candidates.size()) {
            const auto &id = candidates[index].value("id", Json::object());
            if (id.is_object() && id.value("session", uint64_t{0}) == s.session) {
              handled = apply(engine, msime_client_select(
                  s.session, id.value("generation", uint64_t{0}),
                  id.value("index", size_t{0})));
              if (handled) {
                s.japanese_conversion.reset();
                return;
              }
            }
          }
        }
        s.japanese_conversion.reset();
        if (action == Action::CommitReading) {
          handled = apply(engine, msime_client_command(s.session, MSIME_COMMIT_READING));
          if (handled)
            return;
        }
      }
      command = MSIME_COMMIT_RAW;
      break;
    case IBUS_Escape:
      command = MSIME_CANCEL;
      break;
    case IBUS_space:
      if (modifiers == 0 && convert_smart_punctuation_space(engine)) {
        handled = true;
        return;
      }
      if (s.fullwidth && !narrow_scheme && !has_composition && !candidate_active) {
        commit_text(engine, "\xe3\x80\x80");
        handled = true;
        return;
      }
      // Japanese converts rather than committing: the first press starts the conversion and
      // later ones step through it, which is the only way to reach the second candidate.
      if (japanese_composition && has_composition) {
        using Action = msime::linux_host::JapaneseConversion::Action;
        const auto &candidates = s.view.at("candidates");
        const int first_source = candidates.empty() ? -1 : candidates[0].value("source", -1);
        const auto action =
            s.japanese_conversion.space(japanese_reading, candidates.size(), first_source);
        if (action == Action::Start) {
          handled = true;
          return;
        }
        if (action == Action::StepNext || action == Action::StepFirst) {
          handled = apply(engine, msime_client_command(
              s.session, action == Action::StepFirst ? MSIME_FIRST_CANDIDATE
                                                      : MSIME_NEXT_CANDIDATE));
          if (handled)
            return;
        }
      }
      if (select_rendered_highlight()) {
        handled = true;
        return;
      }
      command = MSIME_COMMIT_CANDIDATE;
      break;
    case IBUS_Left:
    case IBUS_KP_Left:
      command = MSIME_MOVE_LEFT;
      break;
    case IBUS_Right:
    case IBUS_KP_Right:
      command = MSIME_MOVE_RIGHT;
      break;
    case IBUS_Home:
    case IBUS_KP_Home:
      command = MSIME_MOVE_HOME;
      break;
    case IBUS_End:
    case IBUS_KP_End:
      command = MSIME_MOVE_END;
      break;
    case IBUS_Delete:
    case IBUS_KP_Delete:
      command = MSIME_DELETE_FORWARD;
      break;
    }
    if (command != UINT32_MAX)
      handled = apply(engine, msime_client_command(s.session, command));
    else if (korean_scheme && ((key >= 'a' && key <= 'z') || (key >= 'A' && key <= 'Z')))
      // Shift decides the jamo (Shift+R is ㄲ, R alone ㄱ) and CapsLock does not, so the letter is sent in the case Shift gives it rather than the case of the keysym.
      handled = apply(engine, msime_client_character(
          s.session,
          static_cast<uint8_t>((flags & IBUS_SHIFT_MASK)
                                   ? g_ascii_toupper(static_cast<gchar>(key))
                                   : g_ascii_tolower(static_cast<gchar>(key))),
          (flags & IBUS_SHIFT_MASK) != 0));
    else if (key >= IBUS_KP_0 && key <= IBUS_KP_9)
      handled = apply(
          engine,
          msime_client_character(
              s.session, static_cast<uint8_t>('0' + key - IBUS_KP_0), false));
    else if (key >= 0x21 && key <= 0x7e)
      handled = apply(engine, msime_client_character(
          s.session,
          static_cast<uint8_t>((flags & IBUS_SHIFT_MASK) && key >= 'a' &&
                                       key <= 'z'
                                   ? key - 'a' + 'A'
                                   : key),
          (flags & IBUS_SHIFT_MASK) != 0));
    else if (g_unichar_isprint(ibus_keyval_to_unicode(key)) &&
             (!s.view.value("editing_text", std::string{}).empty() ||
              !s.view.value("candidates", Json::array()).empty())) {
      // IBus keysyms may encode Unicode with a 0x01000000 prefix.
      // Windows finalizes the active TSF composition before handing an
      // unsupported printable key back to the application. Preserve the
      // same text while allowing IBus to deliver the original keyval.
      apply(engine, msime_client_command(s.session, MSIME_COMMIT_RAW));
      handled = false;
    } else if (commits_on_blur && has_composition)
      // 其他按键像空格那样结束韩文音节，注音转换、越南文单词和藏文音节串也一样：组字内容保留下来，按键交给应用。
      apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
    else
      apply(engine, msime_client_command(s.session, MSIME_CANCEL));
    if (!handled) {
      guint fullwidth_value = key;
      if (key >= IBUS_KP_0 && key <= IBUS_KP_9)
        fullwidth_value = '0' + key - IBUS_KP_0;
      handled = fullwidth_idle_commit(fullwidth_value);
    }
  });
  return handled;
}
struct CandidateMenuHintNotice {
  IBusEngine *engine;
  std::shared_ptr<std::atomic_bool> alive;
  uint64_t id;
  uint64_t session;
  uint64_t generation;
};
// 右键候选：Windows 弹出候选右键菜单（固定、固定排位、删除），选定之前不改动词典。IBus 没有逐个候选的右键菜单接口，「候选操作」属性菜单就是这里的对应物，所以右键只在辅助区域提示去那里操作，约 1.5 秒后恢复页码。
//
// 恢复前确认辅助区域仍属于这条提示：期间任何重绘都已换上新的页码，只有同一会话、同一代次仍在显示时才重绘一次。
void show_candidate_menu_hint(IBusEngine *engine, uint64_t generation) {
  auto &s = state(engine);
  ++s.candidate_menu_hint_id;
  if (s.candidate_menu_hint_id == 0)
    ++s.candidate_menu_hint_id;
  ibus_engine_update_auxiliary_text(
      engine, ibus_text_new_from_static_string("请在「候选操作」菜单中固定、调整排位或删除候选"),
      TRUE);
  auto *notice = new CandidateMenuHintNotice{engine, s.alive, s.candidate_menu_hint_id,
                                             s.session, generation};
  g_timeout_add_full(
      G_PRIORITY_DEFAULT, 1500,
      +[](gpointer data) -> gboolean {
        std::unique_ptr<CandidateMenuHintNotice> notice(
            static_cast<CandidateMenuHintNotice *>(data));
        if (!notice->alive->load())
          return G_SOURCE_REMOVE;
        auto *engine = notice->engine;
        auto &s = state(engine);
        if (s.candidate_menu_hint_id != notice->id || !s.focused || s.blocked ||
            s.voice_active || s.translation_candidates_active ||
            !s.session || s.session != notice->session ||
            s.rendered_session != s.session || !s.rendered_view.is_object() ||
            s.rendered_view.value("generation", uint64_t{0}) != notice->generation ||
            s.view.value("generation", uint64_t{0}) != notice->generation)
          return G_SOURCE_REMOVE;
        guarded(engine, "candidate_menu_hint", [&] { render(engine, s.view); });
        return G_SOURCE_REMOVE;
      },
      notice, nullptr);
}
// Modifier and button masks in the state argument are ignored, as the Windows candidate window commits regardless of modifiers; NumLock (Mod2) alone would otherwise block every click.
void candidate_clicked(IBusEngine *engine, guint index, guint button,
                       G_GNUC_UNUSED guint flags) {
  if ((button < 1 || button > 5) || !state(engine).focused ||
      state(engine).blocked || !state(engine).input_enabled) return;
  guarded(engine, "candidate_clicked", [&] {
    auto &s = state(engine);
    if (s.translation_candidates_active) {
      const auto page_size = std::clamp(
          s.translation_saved_view.value("page_size", size_t{9}), size_t{1},
          size_t{9});
      const auto page_count = (s.translation_options.size() + page_size - 1) /
                              page_size;
      if (button >= 4) {
        // Same mapping and 鼠标滚轮 gate as the ordinary candidate page below: with the switch off the wheel does nothing, as on Windows.
        const auto wheel = s.navigation.wheel_command(button);
        if (!wheel)
          return;
        if (*wheel == MSIME_PREVIOUS_PAGE && s.translation_page > 0)
          --s.translation_page;
        else if (*wheel == MSIME_NEXT_PAGE && s.translation_page + 1 < page_count)
          ++s.translation_page;
        s.translation_cursor = 0;
        render_translation_candidates(engine);
      } else {
        const auto global = s.translation_page * page_size + index;
        if (button == 1 && global < s.translation_options.size())
          (void)commit_translation_candidate(engine, global);
      }
      return;
    }
    if (button >= 4) {
      // Mouse-wheel events can arrive after IBus has hidden the lookup table.
      // Do not let a late page command mutate a live session without the
      // candidate snapshot that was visible when the event was generated.
      if (!s.session || s.rendered_session != s.session ||
          !s.rendered_candidates.is_array() || s.rendered_candidates.empty() ||
          !s.rendered_view.is_object() ||
          s.rendered_view.value("generation", uint64_t{0}) !=
              s.view.value("generation", uint64_t{0}))
        return;
      if (const auto wheel = s.navigation.wheel_command(button))
        apply(engine, msime_client_command(s.session, *wheel));
      return;
    }
    const auto &candidates = s.rendered_candidates;
    if (!s.session || s.rendered_session != s.session ||
        !candidates.is_array() || index >= candidates.size()) return;
    const auto &entry = candidates.at(index);
    if (!entry.is_object() || !entry.contains("id")) return;
    const auto &id = entry.at("id");
    if (!id.is_object() || id.at("session").get<uint64_t>() != s.session) return;
    const auto generation = id.at("generation").get<uint64_t>();
    const auto global_index = id.at("index").get<size_t>();
    const auto source = entry.value("source", 0);
    const auto scheme = s.rendered_scheme;
    if (button == 3) {
      // The fences above establish the candidate, not the view it was rendered from, and
      // rendered_view is null until the first render and again after every session rebuild.
      // value() throws on null, guarded swallows the throw, and the whole click disappears into a
      // warning line. The hint has nothing to restore without a view either: its timeout only
      // re-renders while this generation is still the one on screen.
      if (msime::linux_host::candidate_dictionary_actions_available(scheme, source) && s.rendered_view.is_object())
        show_candidate_menu_hint(engine, s.rendered_view.value("generation", uint64_t{0}));
    } else
      apply(engine, msime_client_select(s.session, generation, global_index));
  });
}
void page(IBusEngine *engine, uint32_t command) {
  guarded(engine, "page", [&] {
    auto &s = state(engine);
    if (s.translation_candidates_active) {
      const auto page_size = std::clamp(
          s.translation_saved_view.value("page_size", size_t{9}), size_t{1},
          size_t{9});
      const auto page_count = (s.translation_options.size() + page_size - 1) /
                              page_size;
      if (command == MSIME_PREVIOUS_PAGE && s.translation_page > 0)
        --s.translation_page;
      else if (command == MSIME_NEXT_PAGE && s.translation_page + 1 < page_count)
        ++s.translation_page;
      else
        return;
      s.translation_cursor = 0;
      render_translation_candidates(engine);
      return;
    }
    // IBus page/cursor callbacks carry no generation. Fence them to the
    // candidate page currently owned by the panel so a delayed callback
    // cannot page a newer Engine view that has not been rendered yet.
    if (s.session && s.focused && !s.blocked && s.input_enabled &&
        s.rendered_session == s.session && s.rendered_view.is_object() &&
        s.rendered_candidates.is_array() && !s.rendered_candidates.empty() &&
        s.rendered_view.value("generation", uint64_t{0}) ==
            s.view.value("generation", uint64_t{0}))
      apply(engine, msime_client_command(s.session, command));
  });
}
struct PreferencesRead {
  std::string directory;
  uint64_t session;
  uint64_t configuration_generation;
};
void apply_live_preferences(IBusEngine *engine, Json snapshot) {
  auto &s = state(engine);
  if (!s.focused || s.blocked)
    return;
  s.apply_session_overrides(snapshot);
  const auto &preferences = snapshot.at("preferences");
  if (!s.session) {
    if (preferences != s.applied_preferences_snapshot ||
        s.applied_display_generation != configuration_generation) {
      s.refresh_host_preferences(preferences);
      s.applied_preferences_snapshot = preferences;
      s.applied_display_generation = configuration_generation;
      publish_mode(engine);
    }
    sync_global_input_mode(engine);
    // An English-mode recording has no session but still ends when voice input is turned off.
    if (s.voice_active && !s.voice_enabled)
      voice_cancel(engine);
    return;
  }
  if (preferences == s.applied_preferences_snapshot) {
    sync_global_input_mode(engine);
    const bool display_changed = s.applied_display_generation != configuration_generation;
    if (display_changed)
      s.refresh_host_preferences(preferences);
    // A menu save whose snapshot a concurrent read already applied has set only the host flag; the session learns the width here.
    const bool width_changed = s.session && s.session_fullwidth != s.fullwidth;
    if (width_changed) {
      s.view = response(msime_client_set_character_width(s.session, s.fullwidth));
      s.session_fullwidth = s.fullwidth;
    }
    if (display_changed || width_changed) {
      render(engine, s.view);
      publish_mode(engine);
    }
    s.applied_display_generation = configuration_generation;
    return;
  }
  // Store revisions belong to the store. The runtime needs an increasing
  // revision for each effective change, including local menu overrides.
  snapshot["revision"] = s.applied_preferences_revision + 1;
  const auto encoded = snapshot.dump();
  auto updated = response(msime_client_update_preferences(
      s.session, reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
  ++s.applied_preferences_revision;
  if (s.applied_preferences_snapshot.is_object() &&
      s.applied_preferences_snapshot.value("ai_assistant", Json(nullptr)) !=
          preferences.value("ai_assistant", Json(nullptr))) {
    s.invalidate_providers();
    s.ai_context.clear();
  }
  if (s.applied_preferences_snapshot.is_object()) {
    for (const auto *key : {"custom_translation", "niutrans", "tencent_tmt"}) {
      if (s.applied_preferences_snapshot.value(key, Json(nullptr)) ==
          preferences.value(key, Json(nullptr)))
        continue;
      s.invalidate_providers();
      s.translation_reset_pending = true;
      break;
    }
  }
  s.applied_preferences_snapshot = preferences;
  s.refresh_host_preferences(preferences);
  msime_linux_diagnostic_write("preferences_applied");
  s.applied_display_generation = configuration_generation;
  // Subsequent mode synchronization or voice cancellation may replace this
  // view. Do not restore the pre-transition snapshot after those actions.
  s.view = updated.at("view");
  // update_preferences does not disturb the runtime's punctuation override, so
  // a preference that moved the effective value has to be re-stated or the
  // session keeps converting with the value the last menu toggle left behind.
  if (s.session_chinese_punctuation != s.chinese_punctuation) {
    s.view = response(
        msime_client_set_chinese_punctuation(s.session, s.chinese_punctuation));
    s.session_chinese_punctuation = s.chinese_punctuation;
  }
  // The same holds for the width: without this the host maps idle ASCII at one width while the session commits its compositions at the other.
  if (s.session_fullwidth != s.fullwidth) {
    s.view = response(msime_client_set_character_width(s.session, s.fullwidth));
    s.session_fullwidth = s.fullwidth;
  }
  sync_global_input_mode(engine);
  if (s.voice_active && !s.voice_enabled)
    voice_cancel(engine);
  if (s.translation_reset_pending)
    clear_candidate_translations(engine);
  render(engine, s.view);
  publish_mode(engine);
  translation_schedule(engine);
  settled_rerank_schedule(engine);
  online_schedule(engine);
}
struct MenuPreferenceSave {
  std::string directory;
  uint64_t configuration;
  MenuPreference preference;
  Json value;
  std::string user_data;
};
void save_menu_preference(IBusEngine *engine, MenuPreference preference, Json value) {
  const auto directory = configured.value("preferences_directory", std::string{});
  if (menu_save_pending || directory.empty() || directory.front() != '/')
    return;
  menu_save_pending = true;
  ++menu_status_generation;
  failed_menu_save.reset();
  publish_mode(engine);
  auto task = g_task_new(G_OBJECT(engine), nullptr,
      +[](GObject *source, GAsyncResult *result, gpointer) {
        menu_save_pending = false;
        ++menu_status_generation;
        auto self = reinterpret_cast<MsimeIbusEngine *>(source);
        std::unique_ptr<Json> snapshot(static_cast<Json *>(
            g_task_propagate_pointer(G_TASK(result), nullptr)));
        if (!self->state) return;
        const auto &request = *static_cast<MenuPreferenceSave *>(
            g_task_get_task_data(G_TASK(result)));
        guarded(IBUS_ENGINE(source), "menu_preference_save", [&] {
          if (request.configuration != configuration_generation) return;
          if (!snapshot) {
            failed_menu_save = FailedMenuSave{request.preference, request.value,
                                             request.directory, request.configuration};
            g_warning("Cannot save MSIME menu preference");
            msime_linux_diagnostic_write("menu_save_failed");
            publish_mode(IBUS_ENGINE(source));
            return;
          }
          // A concurrent reader may already have accepted a newer store revision.
          if (accepted_preferences_directory == request.directory &&
              !accepted_preferences_snapshot.is_null() &&
              accepted_preferences_snapshot.at("revision").get<uint64_t>() >
                  snapshot->at("revision").get<uint64_t>()) {
            *snapshot = accepted_preferences_snapshot;
          }
          if (request.preference == MenuPreference::CloudCandidates)
            self->state->cloud_candidates_override.reset();
          if (request.preference == MenuPreference::CandidateTranslations)
            self->state->candidate_translations_override.reset();
          if (request.preference == MenuPreference::TranslationLanguage)
            self->state->translation_target_language_override.reset();
          if (request.preference == MenuPreference::CandidateTheme)
            self->state->theme_override.reset();
          if (request.preference == MenuPreference::PreeditStyle)
            self->state->preedit_override.reset();
          if (request.preference == MenuPreference::CandidateLayout)
            self->state->layout_override.reset();
          if (request.preference == MenuPreference::GlobalTheme)
            self->state->theme_choice_override.reset();
          if (request.preference == MenuPreference::CandidatePageSize)
            self->state->candidate_page_size_override.reset();
          if (request.preference == MenuPreference::FrequencyMode)
            self->state->frequency_mode_override.reset();
          if (request.preference == MenuPreference::FrequencyTriggerCount)
            self->state->frequency_trigger_count_override.reset();
          if (request.preference == MenuPreference::FrequencyLinearStep)
            self->state->frequency_linear_step_override.reset();
          if (request.preference == MenuPreference::Learning)
            self->state->learning_override.reset();
          if (request.preference == MenuPreference::ShuangpinPreedit)
            self->state->shuangpin_preedit_override.reset();
          if (request.preference == MenuPreference::WubiCodeHint)
            self->state->wubi_code_hint_override.reset();
          if (request.preference == MenuPreference::SmartPunctuation)
            self->state->smart_punctuation_override.reset();
          if (request.preference == MenuPreference::SmartPunctuationRepeat)
            self->state->smart_repeat_override.reset();
          if (request.preference == MenuPreference::PairedPunctuation)
            self->state->paired_punctuation_override.reset();
          if (request.preference == MenuPreference::PunctuationLock)
            self->state->punctuation_lock_override.reset();
          if (request.preference == MenuPreference::AutocorrectTransposition)
            self->state->autocorrect_transposition_override.reset();
          if (request.preference == MenuPreference::AutocorrectNeighbor)
            self->state->autocorrect_neighbor_override.reset();
          if (request.preference == MenuPreference::EnglishCandidates)
            self->state->english_override.reset();
          if (request.preference == MenuPreference::EmojiCandidates)
            self->state->emoji_override.reset();
          if (request.preference == MenuPreference::KaomojiCandidates)
            self->state->kaomoji_override.reset();
          if (request.preference == MenuPreference::QuanpinHelpcode &&
              self->state->scheme_override.value_or(configured.at("preferences").value("scheme", "quanpin")) == "quanpin")
            self->state->helpcode_override.reset();
          if (request.preference == MenuPreference::QuanpinHelpcodeSchema &&
              self->state->scheme_override.value_or(configured.at("preferences").value("scheme", "quanpin")) == "quanpin")
            self->state->helpcode_schema_override.reset();
          if (request.preference == MenuPreference::ShuangpinHelpcode &&
              self->state->scheme_override.value_or(configured.at("preferences").value("scheme", "quanpin")) == "shuangpin")
            self->state->helpcode_override.reset();
          if (request.preference == MenuPreference::ShuangpinHelpcodeSchema &&
              self->state->scheme_override.value_or(configured.at("preferences").value("scheme", "quanpin")) == "shuangpin")
            self->state->helpcode_schema_override.reset();
          if (request.preference == MenuPreference::ShuangpinProfile)
            self->state->shuangpin_profile_override.reset();
          if (request.preference == MenuPreference::InputScheme)
            self->state->scheme_override.reset();
          if (request.preference == MenuPreference::NineKey)
            self->state->nine_key_override.reset();
          if (request.preference == MenuPreference::LocalMode)
            self->state->local_mode_overrides.erase(request.value.at("key").get<std::string>());
          if (request.preference == MenuPreference::NumberRowSelection)
            self->state->number_row_override.reset();
          if (request.preference == MenuPreference::WordCharacter)
            self->state->word_character_override.reset();
          if (request.preference == MenuPreference::TraditionalOutput)
            self->state->traditional_output_override.reset();
          if (request.preference == MenuPreference::ChinesePunctuation)
            self->state->punctuation_override.reset();
          if (request.preference == MenuPreference::CharacterWidth)
            self->state->paired_tracker.clear();
          // Taken from the snapshot about to be applied, not the request: a newer revision another writer saved may have replaced it above, and apply_live_preferences re-states this flag to the session.
          if (request.preference == MenuPreference::CharacterWidth)
            self->state->fullwidth =
                snapshot->at("preferences").value("character_width", "halfwidth") == "fullwidth";
          if (request.preference == MenuPreference::VoiceEnabled)
            self->state->voice_enabled = request.value.get<bool>();
          accepted_preferences_directory = request.directory;
          accepted_preferences_snapshot = *snapshot;
          configured["preferences"] = snapshot->at("preferences");
          msime_linux_diagnostic_write("menu_save_succeeded");
          apply_live_preferences(IBUS_ENGINE(source), *snapshot);
          publish_mode(IBUS_ENGINE(source));
        });
      }, nullptr);
  g_task_set_task_data(task, new MenuPreferenceSave{directory, configuration_generation, preference, std::move(value),
                                                    configured.value("user_data", std::string{})},
      +[](gpointer value) { delete static_cast<MenuPreferenceSave *>(value); });
  g_task_run_in_thread(task,
      +[](GTask *task, gpointer, gpointer data, GCancellable *) {
        const auto &request = *static_cast<MenuPreferenceSave *>(data);
        Json *saved = nullptr;
        try {
          // The launcher refreshes `configured` only every few seconds, so a data directory move can be copying this root, or have taken it away, under a save; a held save fails like a conflict and can be retried (core/DictionaryQuiesceLease.h).
          if (msime::linux_host::preference_save_held(request.user_data))
            throw std::runtime_error("MSIME preferences held");
          const auto *path = reinterpret_cast<const uint8_t *>(request.directory.data());
          auto snapshot = response(msime_client_load_preferences(path, request.directory.size()));
          const auto revision = snapshot.at("revision").get<uint64_t>();
          switch (request.preference) {
          case MenuPreference::CandidateTheme:
            snapshot["preferences"]["candidate_theme"] = request.value;
            break;
          case MenuPreference::PreeditStyle:
            snapshot["preferences"]["tsf_preedit_style"] = request.value;
            break;
          case MenuPreference::CandidateLayout:
            snapshot["preferences"]["candidate_layout"] = request.value;
            break;
          case MenuPreference::GlobalTheme:
            msime::linux_host::apply_theme_choice(snapshot["preferences"], request.value);
            break;
          case MenuPreference::CandidatePageSize:
            snapshot["preferences"]["candidate_page_size"] = request.value;
            break;
          case MenuPreference::FrequencyMode:
            snapshot["preferences"]["frequency"]["mode"] = request.value;
            break;
          case MenuPreference::FrequencyTriggerCount:
            snapshot["preferences"]["frequency"]["trigger_count"] = request.value;
            break;
          case MenuPreference::FrequencyLinearStep:
            snapshot["preferences"]["frequency"]["linear_step"] = request.value;
            break;
          case MenuPreference::Learning:
            snapshot["preferences"]["learning"] = request.value;
            break;
          case MenuPreference::ShuangpinPreedit:
            snapshot["preferences"]["shuangpin_preedit_uses_raw"] = request.value;
            break;
          case MenuPreference::WubiCodeHint:
            snapshot["preferences"]["wubi_code_hint"] = request.value;
            break;
          case MenuPreference::SmartPunctuation:
            snapshot["preferences"]["smart_punctuation"] = request.value;
            break;
          case MenuPreference::SmartPunctuationRepeat:
            snapshot["preferences"]["smart_punctuation_repeat"] = request.value;
            break;
          case MenuPreference::PairedPunctuation:
            snapshot["preferences"]["paired_punctuation"] = request.value;
            break;
          case MenuPreference::PunctuationLock:
            snapshot["preferences"]["punctuation_lock"] = request.value;
            break;
          case MenuPreference::AutocorrectTransposition:
            snapshot["preferences"]["quanpin"]["autocorrect_transposition"] = request.value;
            break;
          case MenuPreference::AutocorrectNeighbor:
            snapshot["preferences"]["quanpin"]["autocorrect_neighbor"] = request.value;
            break;
          case MenuPreference::EnglishCandidates:
            snapshot["preferences"]["mixed_input"]["english"] = request.value;
            break;
          case MenuPreference::EmojiCandidates:
            snapshot["preferences"]["mixed_input"]["emoji"] = request.value;
            break;
          case MenuPreference::KaomojiCandidates:
            snapshot["preferences"]["mixed_input"]["kaomoji"] = request.value;
            break;
          case MenuPreference::QuanpinHelpcode:
            snapshot["preferences"]["quanpin_helpcode"]["enabled"] = request.value;
            break;
          case MenuPreference::QuanpinHelpcodeSchema:
            snapshot["preferences"]["quanpin_helpcode"]["schema"] = request.value;
            break;
          case MenuPreference::ShuangpinHelpcode:
            snapshot["preferences"]["shuangpin_helpcode"]["enabled"] = request.value;
            break;
          case MenuPreference::ShuangpinHelpcodeSchema:
            snapshot["preferences"]["shuangpin_helpcode"]["schema"] = request.value;
            break;
          case MenuPreference::ShuangpinProfile:
            snapshot["preferences"]["shuangpin_profile"] = request.value;
            break;
          case MenuPreference::InputScheme:
            snapshot["preferences"]["scheme"] = request.value;
            // 只有中文方案才是「中文」入口要回到的方案；日文、韩文、越南文和藏文是与它并列的输入语言。
            if (msime::linux_host::scheme::IsChinese(
                    msime::linux_host::scheme_number(request.value.get<std::string>())))
              snapshot["preferences"]["last_chinese_scheme"] = request.value;
            break;
          case MenuPreference::NineKey:
            snapshot["preferences"]["touch_keyboard_layout"] =
                request.value.get<bool>() ? "nine_key" : "twenty_six_key";
            break;
          case MenuPreference::LocalMode:
            snapshot["preferences"]["local_modes"][request.value.at("key").get<std::string>()] =
                request.value.at("enabled");
            break;
          case MenuPreference::NumberRowSelection:
            snapshot["preferences"]["number_row_selection"] = request.value;
            break;
          case MenuPreference::WordCharacter:
            snapshot["preferences"]["word_character"]["enabled"] = request.value;
            break;
          case MenuPreference::TraditionalOutput:
            snapshot["preferences"]["traditional_chinese_output"] = request.value;
            break;
          case MenuPreference::ChinesePunctuation:
            snapshot["preferences"]["chinese_punctuation"] = request.value;
            break;
          case MenuPreference::ClipboardHistoryEnabled:
            snapshot["preferences"]["clipboard_history"] = request.value;
            break;
          case MenuPreference::CharacterWidth:
            snapshot["preferences"]["character_width"] = request.value.get<bool>() ? "fullwidth" : "halfwidth";
            break;
          case MenuPreference::VoiceEnabled:
            snapshot["preferences"]["voice_input"]["enabled"] = request.value;
            break;
          case MenuPreference::Toolbar:
            snapshot["preferences"]["floating_toolbar"]["enabled"] = request.value;
            break;
          case MenuPreference::CloudCandidates:
            snapshot["preferences"]["cloud_candidates"] = request.value;
            break;
          case MenuPreference::CandidateTranslations:
            snapshot["preferences"]["candidate_translations"] = request.value;
            break;
          case MenuPreference::TranslationLanguage:
            snapshot["preferences"]["translation_target_language"] = request.value;
            break;
          }
          const auto encoded = snapshot.dump();
          saved = new Json(response(msime_client_save_preferences(
              path, request.directory.size(), revision,
              reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size())));
        } catch (...) {
          // Revision conflicts and storage errors leave the visible setting unchanged.
        }
        g_task_return_pointer(task, saved,
            +[](gpointer value) { delete static_cast<Json *>(value); });
      });
  g_object_unref(task);
}
gboolean reload_preferences(gpointer data) {
  auto engine = IBUS_ENGINE(data);
  auto &s = state(engine);
  flush_key_presses(s.key_presses.take_due(g_get_monotonic_time()));
  // Release the session, and with it the shared dictionary lock, when the settings window asks for maintenance. The composition is finished first, so nothing typed is lost; open() starts a new session once the lease is gone.
  if (s.session && msime::linux_host::dictionary_quiesced(configured.value("user_data", std::string{}))) {
    guarded(engine, "dictionary_quiesce", [&] {
      apply(engine, msime_client_command(s.session, MSIME_FINISH_COMPOSITION));
      s.close();
      clear(engine);
      msime_linux_diagnostic_write("dictionary_quiesce_released");
    });
  }
  // Saving is shared across contexts, but only the initiating context receives
  // the task callback. Refresh status even when preferences did not change or
  // a preference read is still in flight (including failed saves).
  if (s.focused && !s.blocked &&
      (s.seen_menu_status_generation != menu_status_generation ||
       s.seen_menu_configuration != configuration_generation)) {
    guarded(engine, "menu_status", [&] {
      publish_mode(engine);
      s.seen_menu_status_generation = menu_status_generation;
      s.seen_menu_configuration = configuration_generation;
    });
  }
  watch_clipboard_history(engine);
  guarded(engine, "provider_discovery", [&] {
    if (s.refresh_provider_sockets(engine) && s.focused && !s.blocked) {
      if (s.translation_reset_pending)
        clear_candidate_translations(engine);
      publish_mode(engine);
      online_schedule(engine);
      translation_schedule(engine);
    settled_rerank_schedule(engine);
    }
  });
  if (s.preferences_loading)
    return G_SOURCE_CONTINUE;
  const auto directory = configured.find("preferences_directory");
  if (directory == configured.end() || !directory->is_string() ||
      directory->get<std::string>().empty() ||
      directory->get<std::string>().front() != '/') {
    guarded(engine, "runtime_preferences", [&] {
      apply_live_preferences(engine, Json{{"format_version", 1}, {"revision", 0},
                                         {"preferences", configured.at("preferences")}});
    });
    return G_SOURCE_CONTINUE;
  }
  s.preferences_loading = true;
  auto task = g_task_new(G_OBJECT(engine), nullptr,
                         +[](GObject *source, GAsyncResult *result, gpointer) {
                           auto self = reinterpret_cast<MsimeIbusEngine *>(source);
                           std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
                               static_cast<char *>(g_task_propagate_pointer(
                                   G_TASK(result), nullptr)),
                               msime_client_string_free);
                           if (!self->state)
                             return;
                           auto &s = *self->state;
                           s.preferences_loading = false;
                           const auto *request = static_cast<const PreferencesRead *>(
                               g_task_get_task_data(G_TASK(result)));
                           if (!request || !raw ||
                               request->configuration_generation != configuration_generation)
                             return;
                           try {
                             auto snapshot = response(raw.release());
                             if (snapshot.is_null())
                               return;
                             const auto revision = snapshot.at("revision").get<uint64_t>();
                             if (accepted_preferences_directory == request->directory &&
                                 !accepted_preferences_snapshot.is_null()) {
                               const auto accepted_revision =
                                   accepted_preferences_snapshot.at("revision").get<uint64_t>();
                               if (revision < accepted_revision ||
                                   (revision == accepted_revision &&
                                    snapshot.at("preferences") !=
                                        accepted_preferences_snapshot.at("preferences")))
                                 return;
                             }
                             accepted_preferences_directory = request->directory;
                             accepted_preferences_snapshot = snapshot;
                             configured["preferences"] = snapshot.at("preferences");
                             if (s.session != request->session || !s.focused ||
                                 s.blocked)
                               return;
                             apply_live_preferences(IBUS_ENGINE(source), std::move(snapshot));
                           } catch (...) {
                             // Retry on the next tick without logging paths or input.
                           }
                         },
                         nullptr);
  g_task_set_task_data(
      task,
      new PreferencesRead{directory->get<std::string>(), s.session, configuration_generation},
      +[](gpointer value) { delete static_cast<PreferencesRead *>(value); });
  g_task_run_in_thread(
      task,
      +[](GTask *task, gpointer, gpointer data, GCancellable *) {
        const auto &path = static_cast<PreferencesRead *>(data)->directory;
        typing_statistics_switch.refresh(path);
        g_task_return_pointer(
            task,
            msime_client_try_load_preferences(
                reinterpret_cast<const uint8_t *>(path.data()), path.size()),
            +[](gpointer value) {
              msime_client_string_free(static_cast<char *>(value));
            });
      });
  g_object_unref(task);
  return G_SOURCE_CONTINUE;
}
void register_properties(IBusEngine *engine) {
  publish_mode(engine, true);
}
void destroy(IBusObject *object) {
  auto self = reinterpret_cast<MsimeIbusEngine *>(object);
  if (panel_input_engine == IBUS_ENGINE(object)) panel_input_engine = nullptr;
  if (self->state && self->state->preferences_timer)
    g_source_remove(self->state->preferences_timer);
  if (self->state) {
    flush_key_presses(self->state->key_presses.take());
    key_press_states.erase(self->state);
  }
  delete self->state;
  self->state = nullptr;
  IBUS_OBJECT_CLASS(msime_ibus_engine_parent_class)->destroy(object);
}
// The key sound of one press: every typing key while Chinese input is on in a field that is not a secure one, whether the Engine or the application takes the key, and none while a recording is running. It is asked for after the key is handled, when the session that holds the sound settings exists and the field's state is settled; the call only posts a request (msime_client.h).
void play_key_sound(IBusEngine *engine, guint key, guint flags) {
  auto &s = state(engine);
  sync_music(engine);
  const bool release = (flags & IBUS_RELEASE_MASK) != 0;
  if (release)
    s.key_repeat.release(key);
  if (!s.session || !s.focused || s.blocked || s.private_input || !s.input_enabled ||
      s.voice_active)
    return;
  const bool shortcut =
      (flags & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK |
                IBUS_META_MASK | IBUS_HYPER_MASK)) != 0;
  if (!msime::linux_host::key_press_sounds(release, modifier(key), shortcut))
    return;
  const auto key_class = msime::linux_host::key_sound_class(key);
  msime_client_key_sound(s.session, key_class);
  // The typing effect of the same press, from the same session: Linux shows only the combo count, in the candidate aux line. The key was rendered before this point, so a count that moved redraws that line alone, from the page the panel holds.
  const auto combo = msime::linux_host::typing_effect_combo(msime_client_typing_effect(
      s.session, key_class | (s.key_repeat.press(key) ? msime::linux_host::kTypingEffectRepeat : 0)));
  if (combo == s.typing_combo)
    return;
  s.typing_combo = combo;
  if (s.rendered_session == s.session && s.rendered_view.is_object() &&
      s.rendered_candidates.is_array() && !s.rendered_candidates.empty()) {
    const auto auxiliary = candidate_aux_text(engine, s.rendered_view);
    ibus_engine_update_auxiliary_text(
        engine, ibus_text_new_from_string(auxiliary.c_str()), !auxiliary.empty());
  }
}
// Characters the IME hands back to the application are still typed text: Windows counts them in the statistics (ShouldCountPassthroughChar), so English-mode letters and Chinese-mode keys the Engine declines show up in the daily totals. Keys the IME consumed already recorded their committed text.
gboolean process_key_and_count(IBusEngine *engine, guint key, guint keycode,
                               guint flags) {
  count_key_press(engine, keycode, flags);
  const gboolean handled = process_key(engine, key, keycode, flags);
  play_key_sound(engine, key, flags);
  if (handled || (flags & IBUS_RELEASE_MASK) || !typing_statistics_switch.enabled())
    return handled;
  const auto &s = state(engine);
  if (!s.focused || s.blocked || s.private_input)
    return handled;
  msime::linux_host::PassthroughModifiers held;
  held.control = (flags & IBUS_CONTROL_MASK) != 0;
  held.alt = (flags & IBUS_MOD1_MASK) != 0;
  held.super = (flags & (IBUS_MOD4_MASK | IBUS_SUPER_MASK)) != 0;
  held.hyper = (flags & IBUS_HYPER_MASK) != 0;
  held.meta = (flags & IBUS_META_MASK) != 0;
  const gunichar character = ibus_keyval_to_unicode(key);
  if (!msime::linux_host::should_count_passthrough_character(character, held))
    return handled;
  gchar encoded[8] = {};
  const auto length = g_unichar_to_utf8(character, encoded);
  record_typing_statistics(
      engine, std::string(encoded, static_cast<std::size_t>(length)),
      s.input_enabled ? typing_source(s) : msime::linux_host::TypingSource::English);
  return handled;
}
} // namespace

static void msime_ibus_engine_init(MsimeIbusEngine *engine) {
  engine->state = new State();
  key_press_states.insert(engine->state);
  engine->state->wave_overlay_surface =
      msime::linux_host::create_wave_overlay_surface(
          IBUS_ENGINE(engine), [engine](msime::linux_host::WaveOverlayModel::Action action) {
            if (action == msime::linux_host::WaveOverlayModel::Action::Cancel)
              voice_cancel(IBUS_ENGINE(engine));
            else
              voice_stop(IBUS_ENGINE(engine));
          });
  engine->state->client_token = next_client_token.fetch_add(1, std::memory_order_relaxed);
  // Seed once per host instance; refocus or session recreation keeps user choice.
  if (configured.is_object())
    engine->state->input_enabled = configured.at("preferences").value(
        "default_ime_mode", "chinese") != "english";
  engine->state->preferences_timer =
      g_timeout_add(1000, reload_preferences, engine);
}
static void msime_ibus_engine_class_init(MsimeIbusEngineClass *klass) {
  auto engine = IBUS_ENGINE_CLASS(klass);
  engine->process_key_event = process_key_and_count;
  engine->property_activate = property_activate;
  engine->enable = [](IBusEngine *engine) {
    // Advertise surrounding-text use so native IM modules send document updates.
    ibus_engine_get_surrounding_text(engine, nullptr, nullptr, nullptr);
  };
  engine->focus_in = focus_in;
  engine->focus_out = focus_out;
#if IBUS_CHECK_VERSION(1, 5, 27)
  engine->focus_in_id = [](IBusEngine *engine, const gchar *context, const gchar *client) {
    auto &s = state(engine);
    const std::string next_client = client ? client : "";
    const std::string next_context = context ? context : "";
    // IBus negotiates identity: the daemon may focus the engine first and name
    // the context and client a moment later. That second call is the same focus
    // arriving with a name, so take the name in place. Tearing the runtime down
    // here would throw away whatever the user typed during the negotiation.
    const bool naming_current_focus =
        s.focused && s.focused_context.empty() && s.focused_client.empty();
    if (s.focused && !naming_current_focus && s.focused_client != next_client)
      s.remember_app_input_mode();
    if (s.focused && !naming_current_focus &&
        (s.focused_context != next_context || s.focused_client != next_client))
      focus_out(engine);
    if (naming_current_focus)
      s.adopt_app_input_mode(next_client);
    s.focused_context = next_context;
    s.focused_client = next_client;
    s.surrounding_utf16 = g_strcmp0(client, "QIBusInputContext") == 0;
    focus_in(engine);
  };
  engine->focus_out_id = [](IBusEngine *engine, const gchar *context) {
    const auto &current = state(engine).focused_context;
    if (!current.empty() && current != (context ? context : ""))
      return;
    focus_out(engine);
  };
#endif
  engine->disable = [](IBusEngine *engine) {
    focus_out(engine);
    guarded(engine, "disable", [&] {
      // IBus disables the old engine when changing input sources. Unlike a
      // focus transfer, reactivation must start from the configured CN/EN mode.
      global_input_enabled.reset();
      // The daemon clears properties on disable; register them on reactivation.
      state(engine).properties_registered = false;
      state(engine).app_input_modes.clear();
      state(engine).input_enabled = configured.at("preferences").value(
          "default_ime_mode", "chinese") != "english";
    });
  };
  engine->reset = reset;
  engine->set_content_type = content_type;
  engine->set_surrounding_text = set_surrounding;
  engine->candidate_clicked = candidate_clicked;
  engine->page_up = [](IBusEngine *e) { page(e, MSIME_PREVIOUS_PAGE); };
  engine->page_down = [](IBusEngine *e) { page(e, MSIME_NEXT_PAGE); };
  // The IBus GTK panel and GNOME Shell raise cursor_up/down for the wheel over the candidate window; keyboard arrows never come this way, they arrive through process_key_event and NavigationBindings. As on Windows, the wheel pages when 鼠标滚轮 is on and does nothing otherwise.
  engine->cursor_up = [](IBusEngine *e) {
    if (state(e).navigation.mouse_wheel) page(e, MSIME_PREVIOUS_PAGE);
  };
  engine->cursor_down = [](IBusEngine *e) {
    if (state(e).navigation.mouse_wheel) page(e, MSIME_NEXT_PAGE);
  };
  IBUS_OBJECT_CLASS(klass)->destroy = destroy;
}
void msime_ibus_configure(const std::string &options) {
  if (options.size() > 16384 || msime_client_abi_version() != 3)
    throw std::runtime_error("Invalid host configuration");
  auto next = Json::parse(options);
  if (!next.is_object() || !next.contains("preferences") ||
      !next.at("preferences").is_object())
    throw std::runtime_error("Invalid host preferences");
  // Every load looks again, unchanged options included: a dictionary installed since the last one is offered from the next reload.
  configured_dictionaries = msime::linux_host::language_dictionary_availability(next);
  if (next != configured) {
    configured = std::move(next);
    ++configuration_generation;
    // Startup, or runtime options that may name another store: no engine is ticking yet at startup, and the first commits should not wait a tick for the switch.
    const auto directory = configured.find("preferences_directory");
    typing_statistics_switch.refresh(directory != configured.end() && directory->is_string()
                                         ? directory->get<std::string>()
                                         : std::string{});
  }
}
void msime_ibus_shutdown_key_presses() {
  key_presses_shutting_down = true;
  for (auto *state : key_press_states)
    flush_key_presses(state->key_presses.take());
  // A store write takes milliseconds; the bound only keeps a wedged store from holding the exit.
  key_press_writes.wait_idle(std::chrono::seconds(2));
}
bool msime_ibus_maintenance_stop_requested() { return maintenance_stop_requested; }
bool msime_ibus_upgrade_restart_requested() { return upgrade_restart_requested; }
void msime_ibus_set_system_dark(bool dark) {
  if (system_dark != dark) {
    system_dark = dark;
    ++configuration_generation;
  }
}
