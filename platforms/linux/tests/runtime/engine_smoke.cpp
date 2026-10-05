#include "../src/core/ClientEngine.h"
#include "msime_client.h"
#include <algorithm>
#include <cstdlib>
#include <fcntl.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <memory>
#include <nlohmann/json.hpp>
#include <poll.h>
#include <stdexcept>
#include <cstring>
#include <sys/file.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <vector>
#include "../voice/voice_provider_fixture.h"
#include "../dictionary/translation_provider_fixture.h"

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
struct ForwardedKey {
  guint keyval = 0;
  guint keycode = 0;
  guint state = 0;
};
struct PreeditAttribute {
  guint type = 0;
  guint value = 0;
  guint start = 0;
  guint end = 0;
};
struct Observation {
  std::string committed;
  std::vector<ForwardedKey> forwarded;
  std::string preedit;
  // Attributes carried by the last UpdatePreeditText, as the client receives them over D-Bus.
  std::vector<PreeditAttribute> preedit_attributes;
  std::string auxiliary;
  gboolean auxiliary_visible = FALSE;
  // HideLookupTable and HideAuxiliaryText in the order they arrived.
  std::vector<std::string> hides;
  std::vector<std::string> candidates;
  std::vector<std::string> labels;
  std::string forbidden_gloss;
  bool forbidden_gloss_seen = false;
  guint first_candidate_color = 0;
  guint first_candidate_background = 0;
  guint second_candidate_background = 0;
  // The second row is not highlighted, so it carries the picked text and number colours themselves.
  guint second_candidate_color = 0;
  guint second_candidate_number_color = 0;
  guint first_candidate_number_color = 0;
  std::string first_candidate_fix_name;
  std::string first_candidate_clear_name;
  // Page positions of the rows the host offers candidate actions for, which it does only for dictionary rows (see candidate_actions in ClientEngine.cpp). Generated sentences are absent.
  std::vector<guint> dictionary_slots;
  std::string clipboard_clear_name;
  bool desktop_help = false;
  bool desktop_feedback = false;
  bool desktop_dictionary = false;
  // Keys of the last RegisterProperties, top level only, in menu order.
  std::vector<std::string> registered_keys;
  // The 输入方案 menu's entries as it was last sent, each with whether it is checked. An update sends the entries before the menu, so the list is whole once the menu itself arrives.
  std::map<std::string, bool> scheme_entries;
  bool lookup_visible = false;
  bool preedit_visible = false;
  guint cursor = 0;
  guint preedit_cursor = 0;
  // IBUS_ENGINE_PREEDIT_CLEAR or IBUS_ENGINE_PREEDIT_COMMIT: what IBus does with the preedit when the client loses focus.
  guint preedit_mode = IBUS_ENGINE_PREEDIT_CLEAR;
  bool mode_registered = false;
  bool input_enabled = false;
  bool english_mode = false;
  bool emoji_candidates = false;
  std::string global_theme;
  bool traditional_output = false;
  // The CharacterMode menu item, which shows the host's width flag.
  bool character_width = false;
  bool mode_sensitive = false;
  bool smart_punctuation_sensitive = false;
  bool clipboard_toggle_sensitive = false;
  bool clipboard_clear_sensitive = false;
  bool punctuation_enabled = false;
  bool autocorrect_properties_registered = false;
  bool autocorrect_transposition = false;
  bool autocorrect_neighbor = false;
  bool learning_enabled = false;
  bool learning_sensitive = false;
  int delete_surrounding_calls = 0;
  gint delete_surrounding_offset = 0;
  guint delete_surrounding_count = 0;
};
// The whole composition carries one single underline, as the Fcitx5 host draws it.
bool preedit_underlined(const Observation &seen) {
  const auto length = static_cast<guint>(g_utf8_strlen(seen.preedit.c_str(), -1));
  if (length == 0 || seen.preedit_attributes.size() != 1)
    return false;
  const auto &attribute = seen.preedit_attributes.front();
  return attribute.type == IBUS_ATTR_TYPE_UNDERLINE &&
         attribute.value == IBUS_ATTR_UNDERLINE_SINGLE && attribute.start == 0 &&
         attribute.end == length;
}
void signal(GDBusConnection *, const gchar *, const gchar *, const gchar *,
            const gchar *name, GVariant *parameters, gpointer data) {
  auto &seen = *static_cast<Observation *>(data);
  if (std::string(name) == "HidePreeditText") {
    seen.preedit_visible = false;
    return;
  }
  if (std::string(name) == "HideLookupTable") {
    seen.hides.emplace_back(name);
    seen.lookup_visible = false;
    return;
  }
  if (std::string(name) == "HideAuxiliaryText") {
    seen.hides.emplace_back(name);
    seen.auxiliary.clear();
    seen.auxiliary_visible = FALSE;
    return;
  }
  if (std::string(name) == "ForwardKeyEvent") {
    ForwardedKey forwarded;
    g_variant_get(parameters, "(uuu)", &forwarded.keyval, &forwarded.keycode, &forwarded.state);
    seen.forwarded.push_back(forwarded);
    return;
  }
  if (std::string(name) == "DeleteSurroundingText") {
    g_variant_get(parameters, "(iu)", &seen.delete_surrounding_offset,
                  &seen.delete_surrounding_count);
    ++seen.delete_surrounding_calls;
    return;
  }
  if (std::string(name) != "CommitText" &&
      std::string(name) != "UpdatePreeditText" &&
      std::string(name) != "UpdateLookupTable" &&
      std::string(name) != "UpdateAuxiliaryText" &&
      std::string(name) != "RegisterProperties" &&
      std::string(name) != "UpdateProperty")
    return;
  GVariant *encoded = g_variant_get_child_value(parameters, 0);
  auto object = ibus_serializable_deserialize(encoded);
  g_variant_unref(encoded);
  if (!object)
    std::abort();
  g_object_ref_sink(object);
  if (std::string(name) == "UpdateAuxiliaryText") {
    seen.auxiliary = ibus_text_get_text(IBUS_TEXT(object));
    g_variant_get_child(parameters, 1, "b", &seen.auxiliary_visible);
  }
  auto observe_property = [&](auto &&self, IBusProperty *property) -> void {
    const std::string key = ibus_property_get_key(property);
    if (key == "CandidateActions") {
      seen.first_candidate_fix_name.clear();
      seen.first_candidate_clear_name.clear();
      seen.dictionary_slots.clear();
    }
    if (key.rfind("CandidateEntry/", 0) == 0) {
      // The entry label is "<slot>. <text>" with a 1-based page slot.
      const std::string label = ibus_text_get_text(ibus_property_get_label(property));
      const auto slot = std::strtoul(label.c_str(), nullptr, 10);
      if (slot > 0) seen.dictionary_slots.push_back(static_cast<guint>(slot - 1));
    }
    if (key == "ClipboardHistory") {
      seen.clipboard_clear_name.clear();
      seen.clipboard_clear_sensitive = false;
    }
    if (key == "Scheme") seen.scheme_entries.clear();
    if (key.rfind("Scheme/", 0) == 0 && ibus_property_get_prop_type(property) == PROP_TYPE_RADIO)
      seen.scheme_entries[key] = ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "DesktopTools/Help") seen.desktop_help = true;
    if (key == "DesktopTools/Feedback") seen.desktop_feedback = true;
    if (key == "DesktopTools/Dictionary") seen.desktop_dictionary = true;
    if (seen.first_candidate_fix_name.empty() &&
        (key == "CandidateFix1" || key.rfind("CandidateFix1/", 0) == 0))
      seen.first_candidate_fix_name = key;
    if (seen.first_candidate_clear_name.empty() &&
        (key == "CandidateClear" || key.rfind("CandidateClear/", 0) == 0))
      seen.first_candidate_clear_name = key;
    if (key == "InputMode") {
      seen.input_enabled =
          ibus_property_get_state(property) == PROP_STATE_CHECKED;
      seen.mode_sensitive = ibus_property_get_sensitive(property);
    }
    if (key == "Learning") {
      seen.learning_enabled = ibus_property_get_state(property) == PROP_STATE_CHECKED;
      seen.learning_sensitive = ibus_property_get_sensitive(property);
    }
    if (key == "SmartPunctuation")
      seen.smart_punctuation_sensitive = ibus_property_get_sensitive(property);
    if (key == "ClipboardHistory/Enabled")
      seen.clipboard_toggle_sensitive = ibus_property_get_sensitive(property);
    if (key.rfind("ClipboardHistory/Clear/", 0) == 0) {
      seen.clipboard_clear_name = key;
      seen.clipboard_clear_sensitive = ibus_property_get_sensitive(property);
    }
    if (key == "EnglishMode")
      seen.english_mode = ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "EmojiCandidates")
      seen.emoji_candidates = ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key.rfind("GlobalTheme/", 0) == 0 &&
        ibus_property_get_state(property) == PROP_STATE_CHECKED)
      seen.global_theme = key.substr(std::string("GlobalTheme/").size());
    if (key == "TraditionalOutput")
      seen.traditional_output = ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "CharacterMode")
      seen.character_width = ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "Punctuation")
      seen.punctuation_enabled =
          ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "AutocorrectTransposition")
      seen.autocorrect_properties_registered = true;
    if (key == "AutocorrectNeighbor")
      seen.autocorrect_properties_registered = true;
    if (key == "AutocorrectTransposition")
      seen.autocorrect_transposition =
          ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (key == "AutocorrectNeighbor")
      seen.autocorrect_neighbor =
          ibus_property_get_state(property) == PROP_STATE_CHECKED;
    if (auto sub_properties = ibus_property_get_sub_props(property))
      for (guint i = 0; auto child = ibus_prop_list_get(sub_properties, i); ++i)
        self(self, child);
  };
  if (std::string(name) == "RegisterProperties") {
    auto properties = IBUS_PROP_LIST(object);
    seen.registered_keys.clear();
    for (guint i = 0; auto property = ibus_prop_list_get(properties, i); ++i) {
      seen.registered_keys.push_back(ibus_property_get_key(property));
      observe_property(observe_property, property);
      if (std::string(ibus_property_get_key(property)) == "InputMode")
        seen.mode_registered = true;
    }
  }
  if (std::string(name) == "UpdateProperty")
    observe_property(observe_property, IBUS_PROPERTY(object));
  if (std::string(name) == "CommitText")
    seen.committed += ibus_text_get_text(IBUS_TEXT(object));
  if (std::string(name) == "UpdatePreeditText") {
    g_variant_get_child(parameters, 1, "u", &seen.preedit_cursor);
    seen.preedit = ibus_text_get_text(IBUS_TEXT(object));
    seen.preedit_attributes.clear();
    if (auto attributes = ibus_text_get_attributes(IBUS_TEXT(object)))
      for (guint i = 0; auto attribute = ibus_attr_list_get(attributes, i); ++i)
        seen.preedit_attributes.push_back(
            {ibus_attribute_get_attr_type(attribute), ibus_attribute_get_value(attribute),
             ibus_attribute_get_start_index(attribute), ibus_attribute_get_end_index(attribute)});
    gboolean visible;
    g_variant_get_child(parameters, 2, "b", &visible);
    seen.preedit_visible = visible;
    if (g_variant_n_children(parameters) > 3)
      g_variant_get_child(parameters, 3, "u", &seen.preedit_mode);
  }
  if (std::string(name) == "UpdateLookupTable") {
    seen.candidates.clear();
    seen.labels.clear();
    auto table = IBUS_LOOKUP_TABLE(object);
    seen.cursor = ibus_lookup_table_get_cursor_pos(table);
    for (guint i = 0; i < ibus_lookup_table_get_number_of_candidates(table);
         ++i) {
      seen.candidates.emplace_back(
          ibus_text_get_text(ibus_lookup_table_get_candidate(table, i)));
      if (!seen.forbidden_gloss.empty() &&
          seen.candidates.back().find(seen.forbidden_gloss) != std::string::npos)
        seen.forbidden_gloss_seen = true;
      seen.labels.emplace_back(
          ibus_text_get_text(ibus_lookup_table_get_label(table, i)));
    }
    if (ibus_lookup_table_get_number_of_candidates(table) != 0) {
      auto text = ibus_lookup_table_get_candidate(table, 0);
      auto attributes = ibus_text_get_attributes(text);
      if (auto attribute = ibus_attr_list_get(attributes, 0))
        seen.first_candidate_color = ibus_attribute_get_value(attribute);
      if (auto attribute = ibus_attr_list_get(attributes, 1))
        seen.first_candidate_background = ibus_attribute_get_value(attribute);
      if (ibus_lookup_table_get_number_of_candidates(table) > 1) {
        if (auto second = ibus_text_get_attributes(ibus_lookup_table_get_candidate(table, 1))) {
          if (auto attribute = ibus_attr_list_get(second, 0))
            seen.second_candidate_color = ibus_attribute_get_value(attribute);
          if (auto attribute = ibus_attr_list_get(second, 1))
            seen.second_candidate_background = ibus_attribute_get_value(attribute);
        }
        if (auto label_attributes = ibus_text_get_attributes(ibus_lookup_table_get_label(table, 1)))
          if (auto attribute = ibus_attr_list_get(label_attributes, 0))
            seen.second_candidate_number_color = ibus_attribute_get_value(attribute);
      }
      auto label = ibus_lookup_table_get_label(table, 0);
      if (auto label_attributes = ibus_text_get_attributes(label))
        if (auto attribute = ibus_attr_list_get(label_attributes, 0))
          seen.first_candidate_number_color = ibus_attribute_get_value(attribute);
    }
    gboolean visible;
    g_variant_get_child(parameters, 1, "b", &visible);
    seen.lookup_visible = visible;
  }
  g_object_unref(object);
}
struct Call {
  bool done = false;
  GVariant *result = nullptr;
  GError *error = nullptr;
};
GVariant *call(GDBusConnection *connection, const char *destination,
               const char *method, GVariant *parameters) {
  Call pending;
  const char *interface = std::string(method) == "Set"
                              ? "org.freedesktop.DBus.Properties"
                              : "org.freedesktop.IBus.Engine";
  g_dbus_connection_call(
      connection, destination, "/app/msime/test/engine", interface, method,
      parameters, nullptr, G_DBUS_CALL_FLAGS_NONE, 5000, nullptr,
      +[](GObject *source, GAsyncResult *result, gpointer data) {
        auto &p = *static_cast<Call *>(data);
        p.result = g_dbus_connection_call_finish(G_DBUS_CONNECTION(source),
                                                 result, &p.error);
        p.done = true;
      },
      &pending);
  while (!pending.done)
    g_main_context_iteration(nullptr, TRUE);
  if (pending.error) {
    // This connection carries only the isolated synthetic test fixture.
    std::string message = std::string(method) + ": " + pending.error->message;
    g_error_free(pending.error);
    throw std::runtime_error(message);
  }
  while (g_main_context_iteration(nullptr, FALSE)) {
  }
  return pending.result;
}
} // namespace

int main(int argc, char **argv) {
  if (argc != 2 && !(argc == 3 && (std::string(argv[2]) == "--page-number" ||
                                  std::string(argv[2]) == "--ctrl-space")))
    return 2;
  try {
    // This fixture asserts RegisterProperties and must exercise the real menu path.
    g_unsetenv("MSIME_DISABLE_IBUS_PROPERTIES");
    // Synthetic fixture only; the production host does not invoke this
    // bootstrap.
    gchar *temporary = g_dir_make_tmp("msime-ibus-test-XXXXXX", nullptr);
    require(temporary != nullptr, "Cannot create test directory");
    std::filesystem::path root(temporary);
    g_free(temporary);
    struct Cleanup {
      std::filesystem::path path;
      ~Cleanup() {
        std::error_code error;
        std::filesystem::remove_all(path, error);
      }
    } cleanup{root};
    // The panel input socket lives under the runtime directory. Give the fixture its own, so it never meets a live host's socket.
    const auto runtime = root / "runtime";
    std::filesystem::create_directory(runtime);
    std::filesystem::permissions(runtime, std::filesystem::perms::owner_all,
                                 std::filesystem::perm_options::replace);
    g_setenv("XDG_RUNTIME_DIR", runtime.c_str(), TRUE);
    auto bootstrap = nlohmann::json{
        {"resources", argv[1]},
        {"state_root",
         root.string()}}.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> prepared(
        msime_client_prepare_host(
            reinterpret_cast<const uint8_t *>(bootstrap.data()),
            bootstrap.size()),
        msime_client_string_free);
    auto result = nlohmann::json::parse(prepared.get());
    require(result.at("ok").get<bool>(), "Locked dictionary bootstrap failed");
    auto options = result.at("value");
    const auto voice_socket = (root / "voice.sock").string();
    VoiceProviderFixture voice_provider(voice_socket);
    options["voice_provider_socket"] = voice_socket;
    options["preferences"]["learning"] = false;
    options["preferences"]["candidate_translations"] = false;
    options["preferences"]["keybindings"]["switch_language_ctrl"] = true;
    options["preferences"]["voice_input"]["hotkey_ctrl_win"] = true;
    options["preferences"]["voice_input"]["stream_inline_preedit"] = true;
    // A stored commit strategy, which the Linux settings page does not offer and the hosts ignore: the streaming voice preedit asserted below must still appear.
    options["preferences"]["voice_input"]["commit_mode"] = "ctrl_v";
    options["preferences"]["voice_input"]["hotkey_rctrl_ralt"] = true;
    options["preferences"]["global_theme"] = "custom";
    options["preferences"]["custom_theme"]["candidate_colors"] = {
        {"text", "#123456"}, {"surface", "#654321"}, {"number", "#abcdef"}, {"selected", "#204060"}};
    options["preferences"]["candidate_page_size"] = 2;
    options["preferences"]["default_ime_mode"] = "chinese";
    options["preferences"]["smart_punctuation_space_convert"] = true;
    // Keep the mixed-Emoji path below explicit; the missing-field fallback is
    // checked separately before the main fixture starts.
    options["preferences"]["mixed_input"]["emoji"] = true;
    std::ofstream(root / "preferences.json") << nlohmann::json{
        {"format_version", 1},
        {"revision", 0},
        {"preferences", options.at("preferences")}}.dump();
    msime_ibus_configure(options.dump());
    ibus_init();
    auto bus = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus);
    // g_test_dbus_up() calls g_test_dbus_unset(), which clears XDG_RUNTIME_DIR so a test never reaches the user's bus. Set it again, or the host has no runtime directory and the panel input socket never opens.
    g_setenv("XDG_RUNTIME_DIR", runtime.c_str(), TRUE);
    auto connect = [&] {
      return g_dbus_connection_new_for_address_sync(
          g_test_dbus_get_bus_address(bus),
          static_cast<GDBusConnectionFlags>(
              G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
              G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION),
          nullptr, nullptr, nullptr);
    };
    auto server = connect();
    auto client = connect();
    require(server && client, "Private D-Bus unavailable");
    auto create_engine = [&] {
      auto created = IBUS_ENGINE(
          g_object_new(msime_ibus_engine_get_type(), "engine-name",
                     "msime-linux", "object-path",
                     "/app/msime/test/engine", "connection", server, nullptr));
      g_object_ref_sink(created);
      return created;
    };
    Observation seen;
    auto missing_emoji_options = options;
    missing_emoji_options["preferences"].erase("mixed_input");
    missing_emoji_options["preferences"].erase("global_theme");
    // What this case is about is the absent mixed_input object: the default it
    // falls back to, and the menu still being able to flip it. With a shared
    // preferences directory configured the menu flips it by saving a revision in
    // the background instead, so the assertions below would be racing a write
    // rather than reading a decision. Other cases in this fixture drop the
    // directory for the same reason.
    missing_emoji_options.erase("preferences_directory");
    msime_ibus_configure(missing_emoji_options.dump());
    auto engine = create_engine();
    const char *destination = g_dbus_connection_get_unique_name(server);
    guint subscription = g_dbus_connection_signal_subscribe(
        client, destination, "org.freedesktop.IBus.Engine", nullptr,
        "/app/msime/test/engine", nullptr, G_DBUS_SIGNAL_FLAGS_NONE, signal,
        &seen, nullptr);
    auto invoke = [&](const char *method, GVariant *params = nullptr) {
      auto value = call(client, destination, method, params);
      g_variant_unref(value);
    };
    auto key = [&](guint value, guint flags = 0) {
      auto reply = call(client, destination, "ProcessKeyEvent",
                        g_variant_new("(uuu)", value, 0, flags));
      gboolean handled;
      g_variant_get(reply, "(b)", &handled);
      g_variant_unref(reply);
      return handled != FALSE;
    };
    // Direct class calls make no D-Bus round trip, so the signals an observation
    // comes from are still queued when the call returns. Wait for the
    // observation itself rather than draining whatever happens to be pending.
    auto wait_until = [&](auto ready) {
      const auto deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
      while (!ready() && g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      return ready();
    };
    // The lookup table only ever carries the page the panel shows, so a
    // candidate the engine ranks past the first page has to be paged to before
    // it can be selected. Returns its index on the page it was found, or -1.
    auto page_to = [&](const std::string &wanted) {
      for (int page = 0; page < 24; ++page) {
        const auto found = std::find(seen.candidates.begin(), seen.candidates.end(), wanted);
        if (found != seen.candidates.end())
          return static_cast<int>(found - seen.candidates.begin());
        const auto before = seen.candidates;
        if (!key(IBUS_Page_Down) || seen.candidates == before)
          break;
      }
      return -1;
    };
    // The host hides the candidate window on a 24ms timer rather than in the
    // turn that empties it, so a composition that briefly has no candidates does
    // not flicker the panel. Anything asserting that the window is gone has to
    // wait for that timer instead of reading a value deliberately not there yet.
    auto settle_lookup = [&] {
      const auto deadline = g_get_monotonic_time() + 2 * G_USEC_PER_SEC;
      while (seen.lookup_visible && g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
    };
    // Fifty-six call sites share this, and "Phrase key not consumed" named none
    // of them. Number the calls and say which key: a failure here otherwise costs
    // a bisection through the whole fixture to find out where it happened.
    int phrase_calls = 0;
    auto phrase = [&] {
      const int call = ++phrase_calls;
      for (char c : std::string("nihao"))
        require(key(c), ("Phrase key '" + std::string(1, c) + "' not consumed in phrase() call " +
                         std::to_string(call))
                            .c_str());
    };
    invoke("FocusIn");
    // #2589: a new focus shows the current mode the way a switch does, then the hint goes away by itself. The replay IBus sends while it names the client is the same focus and must not show it again.
    require(wait_until([&] { return seen.auxiliary == "中" && seen.auxiliary_visible; }),
            "Focus did not show the input mode");
    require(wait_until([&] { return !seen.auxiliary_visible; }), "Focus mode hint did not hide");
    invoke("FocusIn");
    require(!wait_until([&] { return seen.auxiliary_visible; }) || seen.auxiliary != "中",
            "A repeated focus showed the input mode again");
    const auto finish = [&] {
      g_dbus_connection_signal_unsubscribe(client, subscription);
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      g_dbus_connection_close_sync(client, nullptr, nullptr);
      g_dbus_connection_close_sync(server, nullptr, nullptr);
      g_object_unref(client);
      g_object_unref(server);
      g_test_dbus_down(bus);
      g_object_unref(bus);
    };
    if (argc == 3 && std::string(argv[2]) == "--ctrl-space") {
      msime_ibus_configure(options.dump());
      invoke("FocusIn");
      const auto chord = [&] {
        const bool pressed = key(IBUS_space, IBUS_CONTROL_MASK);
        require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK) == pressed,
                "Ctrl+Space press and release have different ownership");
        return pressed;
      };
      require(chord() && !seen.input_enabled && chord() && seen.input_enabled,
              "Default Ctrl+Space does not switch in both directions");
      const auto set_binding = [&](bool enabled) {
        nlohmann::json snapshot;
        std::ifstream(root / "preferences.json") >> snapshot;
        snapshot["revision"] = snapshot.at("revision").get<uint64_t>() + 1;
        snapshot["preferences"]["keybindings"]["switch_language_ctrl_space"] = enabled;
        std::ofstream(root / "next.json") << snapshot.dump();
        std::filesystem::rename(root / "next.json", root / "preferences.json");
      };
      set_binding(false);
      require(wait_until([&] {
                if (!chord()) return true;
                chord();
                return false;
              }), "Disabled Ctrl+Space did not hot-reload");
      for (guint mode : {guint(PROP_STATE_CHECKED), guint(PROP_STATE_UNCHECKED)}) {
        invoke("PropertyActivate", g_variant_new("(su)", "InputMode", mode));
        const bool before = seen.input_enabled;
        for (int repeat = 0; repeat < 3; ++repeat)
          require(!key(IBUS_space, IBUS_CONTROL_MASK) && seen.input_enabled == before,
                  "Disabled Ctrl+Space press was intercepted or switched mode");
        require(!key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
                "Disabled Ctrl+Space release was intercepted");
      }
      invoke("PropertyActivate", g_variant_new("(su)", "InputMode", PROP_STATE_CHECKED));
      phrase();
      const auto committed = seen.committed;
      require(!chord() && seen.preedit == "nihao" && seen.committed == committed,
              "Disabled Ctrl+Space changed the active composition");
      invoke("Reset");
      set_binding(true);
      require(wait_until([&] { return chord(); }), "Enabled Ctrl+Space did not hot-reload");
      invoke("PropertyActivate", g_variant_new("(su)", "InputMode", PROP_STATE_CHECKED));
      for (int repeat = 0; repeat < 3; ++repeat)
        require(key(IBUS_space, IBUS_CONTROL_MASK) && !seen.input_enabled,
                "Held Ctrl+Space toggled more than once");
      require(key(IBUS_space, IBUS_RELEASE_MASK) && !seen.input_enabled,
              "Consumed Ctrl+Space release escaped after Ctrl was released");
      require(chord() && seen.input_enabled, "Ctrl+Space did not restore Chinese mode");
      invoke("Disable");
      finish();
      std::cout << "IBus Ctrl+Space defaults, passthrough, hot-reload and repeat passed\n";
      return 0;
    }
    if (argc == 3) {
      auto visibility = options;
      visibility.erase("preferences_directory");
      visibility["preferences"]["candidate_preedit_style"] = "empty";
      visibility["preferences"].erase("show_candidate_page_number");
      const auto compose = [&] {
        msime_ibus_configure(visibility.dump());
        invoke("FocusIn");
        invoke("Reset");
        phrase();
      };
      compose();
      require(seen.auxiliary.rfind("1/", 0) == 0 && seen.auxiliary_visible,
              "Legacy page indicator missing");
      visibility["preferences"]["show_candidate_page_number"] = false;
      msime_ibus_configure(visibility.dump());
      require(wait_until([&] { return seen.auxiliary.empty() && !seen.auxiliary_visible; }) &&
                  seen.lookup_visible && seen.preedit == "nihao",
              "Page visibility did not hot-reload or hid the candidates");
      const auto first = seen.candidates;
      require(key(IBUS_Page_Down) && seen.candidates != first && seen.auxiliary.empty(),
              "Hidden indicator broke forward paging");
      require(key(IBUS_Page_Up) && seen.candidates == first,
              "Hidden indicator broke backward paging");
      require(key(IBUS_space) && seen.committed == "你好",
              "Hidden indicator broke selection");
      visibility["preferences"]["show_candidate_page_number"] = true;
      compose();
      require(seen.auxiliary.rfind("1/", 0) == 0, "Re-enabled page indicator missing");
      visibility["preferences"]["show_candidate_page_number"] = false;
      visibility["preferences"]["candidate_preedit_style"] = "pinyin";
      compose();
      require(seen.auxiliary.find("ni") != std::string::npos &&
                  seen.auxiliary.find('/') == std::string::npos &&
                  seen.auxiliary.rfind("  · ", 0) != 0 && seen.auxiliary_visible,
              "Page visibility changed the independent reading or left a separator");
      invoke("Disable");
      finish();
      std::cout << "IBus page-number visibility, paging and selection passed\n";
      return 0;
    }
    // #3759: hiding the auxiliary line ahead of the list shrinks a window that is still showing, and GNOME then moves it from above the cursor to below it until the list hides.
    phrase();
    require(seen.lookup_visible, "Phrase did not show candidates");
    seen.hides.clear();
    require(key(IBUS_space) && seen.committed == "你好" &&
                wait_until([&] { return !seen.lookup_visible && !seen.auxiliary_visible; }) &&
                !seen.hides.empty() && seen.hides.front() == "HideLookupTable",
            "Selection hid the auxiliary line before the candidate list");
    seen.committed.clear();
    require(!seen.emoji_candidates,
            "Missing mixed Emoji preference did not default to disabled");
    require(seen.global_theme == "system",
            "Missing global theme preference did not use the shared default");
    IBUS_ENGINE_GET_CLASS(engine)->property_activate(
        IBUS_ENGINE(engine), "EmojiCandidates", PROP_STATE_CHECKED);
    // property_activate is a direct call, so unlike invoke() it makes no round
    // trip that would deliver the property signal this observation comes from.
    // Wait for the observation rather than for a duration: draining only what
    // happens to be pending reads a value that may not have arrived yet, which is
    // how this passed under one timing and failed under another.
    const auto emoji_deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
    while (!seen.emoji_candidates && g_get_monotonic_time() < emoji_deadline)
      g_main_context_iteration(nullptr, FALSE) || (g_usleep(1000), false);
    require(seen.emoji_candidates,
            "Missing mixed input object could not activate Emoji candidates");
    invoke("FocusOut");
    // Host shortcuts must load and reload while English passthrough has no
    // Engine session. Keep this store separate from the remaining fixtures.
    ibus_object_destroy(IBUS_OBJECT(engine));
    g_object_unref(engine);
    auto live_options = options;
    auto &live_preferences = live_options["preferences"];
    live_preferences["default_ime_mode"] = "english";
    live_preferences["keybindings"]["switch_language_shift"] = false;
    live_preferences["keybindings"]["switch_language_ctrl"] = false;
    const auto live_directory = root / "passthrough-preferences";
    std::filesystem::create_directory(live_directory);
    live_options["preferences_directory"] = live_directory.string();
    auto save_live_preferences = [&](unsigned revision) {
      std::ofstream(live_directory / "next.json") << nlohmann::json{
          {"format_version", 1}, {"revision", revision},
          {"preferences", live_preferences}}.dump();
      std::filesystem::rename(live_directory / "next.json", live_directory / "preferences.json");
    };
    save_live_preferences(1);
    msime_ibus_configure(live_options.dump());
    engine = create_engine();
    seen = Observation{};
    invoke("FocusIn");
    require(!key(IBUS_Shift_L, IBUS_SHIFT_MASK) &&
                !key(IBUS_Shift_L, IBUS_RELEASE_MASK) && !seen.input_enabled,
            "Passthrough ignored the disabled Shift mode shortcut");
    require(!key(IBUS_Control_L, IBUS_CONTROL_MASK) &&
                !key(IBUS_Control_L, IBUS_RELEASE_MASK) && !seen.input_enabled,
            "Passthrough ignored the disabled Ctrl mode shortcut");
    live_preferences["keybindings"]["switch_language_ctrl"] = true;
    save_live_preferences(2);
    // The bare modifier release toggles but is never consumed, so the toggle is read from the published mode rather than from the return value.
    const auto live_deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
    while (!seen.input_enabled && g_get_monotonic_time() < live_deadline) {
      require(!key(IBUS_Control_L, IBUS_CONTROL_MASK), "Ctrl press was intercepted");
      require(!key(IBUS_Control_L, IBUS_RELEASE_MASK), "Ctrl release was consumed");
      if (!seen.input_enabled) g_usleep(50000);
    }
    require(seen.input_enabled,
            "Host shortcuts did not reload without an Engine session");
    phrase();
    require(!seen.english_mode && seen.preedit == "nihao",
            "Reloaded mode shortcut did not open Chinese input");
    live_preferences["ime_mode_scope"] = "global";
    save_live_preferences(3);
    const auto scope_deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
    while (seen.input_enabled && g_get_monotonic_time() < scope_deadline) {
      while (g_main_context_iteration(nullptr, FALSE)) {}
      g_usleep(1000);
    }
    require(!seen.input_enabled && seen.committed == "nihao",
            "Global mode synchronization did not finish the reading string");
    require(!key('n') && !seen.preedit_visible && !seen.lookup_visible,
            "Preference refresh restored composition after switching to passthrough");
    seen.committed.clear();
    invoke("Reset");
    // English mode keeps fullwidth output and the "always Chinese punctuation" lock, as Windows does with the IME closed. The injected options are the authority here, so no preferences directory is read back.
    {
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      auto english = options;
      english.erase("preferences_directory");
      english["preferences"]["ime_mode_scope"] = "app";
      english["preferences"]["default_ime_mode"] = "english";
      english["preferences"]["character_width"] = "fullwidth";
      english["preferences"]["punctuation_lock"] = "chinese";
      msime_ibus_configure(english.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      if (seen.input_enabled) {
        key(IBUS_space, IBUS_CONTROL_MASK);
        key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      }
      require(!seen.input_enabled, "English output fixture did not start in English mode");
      require(key('a') && seen.committed == "ａ",
              "Fullwidth English mode did not widen a letter");
      seen.committed.clear();
      require(key(IBUS_space) && seen.committed == "\u3000",
              "Fullwidth English mode did not widen Space");
      seen.committed.clear();
      require(key(IBUS_comma) && seen.committed == "，",
              "Chinese punctuation lock did not convert a comma in English mode");
      seen.committed.clear();
      require(key(IBUS_quotedbl, IBUS_SHIFT_MASK) && key(IBUS_quotedbl, IBUS_SHIFT_MASK) &&
                  seen.committed == "“”",
              "Chinese punctuation lock did not alternate quotes in English mode");
      seen.committed.clear();
      // A pinned lock holds the punctuation in place, as Windows resolves Ctrl+. through the lock: the chord is eaten and changes nothing.
      require(key(IBUS_period, IBUS_CONTROL_MASK) && seen.committed.empty(),
              "English-mode Ctrl+. was not consumed under the Chinese punctuation lock");
      require(key(IBUS_comma) && seen.committed == "，",
              "English-mode Ctrl+. overrode the Chinese punctuation lock");
      seen.committed.clear();
      require(!key(IBUS_a, IBUS_CONTROL_MASK) && seen.committed.empty(),
              "English mode output swallowed a Ctrl shortcut");
      require(!key('a', IBUS_RELEASE_MASK) && seen.committed.empty(),
              "English mode output handled a key release");
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // Voice input is not part of the input mode, as on Windows: it records in English mode, where no Engine session exists yet, and switching modes mid-recording does not cancel it.
      auto english_voice = options;
      english_voice.erase("preferences_directory");
      english_voice["preferences"]["ime_mode_scope"] = "app";
      english_voice["preferences"]["default_ime_mode"] = "english";
      english_voice["preferences"]["keybindings"]["switch_language_shift"] = true;
      msime_ibus_configure(english_voice.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      require(!seen.input_enabled, "English voice fixture did not start in English mode");
      // The menu entry records too, and Esc still cancels, without an Engine session.
      auto voice_starts = voice_provider.started.load();
      auto voice_cancels = voice_provider.cancelled.load();
      auto voice_finals = voice_provider.finished.load();
      invoke("PropertyActivate", g_variant_new("(su)", "VoiceInput", PROP_STATE_CHECKED));
      require(wait_until([&] { return voice_provider.started.load() == voice_starts + 1; }),
              "Voice menu did not start recording in English mode");
      voice_provider.release_partial = true;
      require(wait_until([&] { return seen.preedit == "测试😀" && seen.preedit_visible; }),
              "English-mode recording did not show streaming preedit");
      require(preedit_underlined(seen), "Streaming voice preedit was not single-underlined");
      require(key(IBUS_Escape) && !seen.preedit_visible && seen.committed.empty(),
              "Esc did not cancel an English-mode recording");
      require(wait_until([&] { return voice_provider.cancelled.load() == voice_cancels + 1; }),
              "Esc did not cancel English-mode capture at the provider");
      voice_provider.release_final = true;
      require(wait_until([&] { return voice_provider.finished.load() == voice_finals + 1; }),
              "Cancelled English-mode provider did not finish");
      // Ctrl+F9 starts, stops and commits a recording without ever leaving English mode, so the result lands with no Engine session or view to render.
      voice_starts = voice_provider.started.load();
      voice_cancels = voice_provider.cancelled.load();
      auto voice_stops = voice_provider.stop_requests.load();
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 was not consumed in English mode");
      require(wait_until([&] { return voice_provider.started.load() == voice_starts + 1; }),
              "Ctrl+F9 did not start recording in English mode");
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 did not stop an English-mode recording");
      require(wait_until([&] { return voice_provider.stop_requests.load() == voice_stops + 1; }),
              "Ctrl+F9 stop did not reach the provider in English mode");
      voice_provider.release_final = true;
      require(wait_until([&] { return !seen.committed.empty(); }),
              "English-mode recording without a mode switch did not commit");
      require(seen.committed == "synthetic voice" && !seen.preedit_visible &&
                  !seen.input_enabled && voice_provider.cancelled.load() == voice_cancels,
              "English-mode recording did not commit the provider text once and stay in English mode");
      seen.committed.clear();
      // A pending dead key does not swallow Right Alt in English mode either; real clients send it with state 0.
      voice_starts = voice_provider.started.load();
      voice_stops = voice_provider.stop_requests.load();
      require(key(IBUS_dead_acute), "Dead key was not held by the system Compose table in English mode");
      key(IBUS_dead_acute, IBUS_RELEASE_MASK);
      require(key(IBUS_Alt_R), "Right Alt was swallowed by a pending dead key in English mode");
      require(wait_until([&] { return voice_provider.started.load() == voice_starts + 1; }),
              "Right Alt did not start recording with a dead key pending in English mode");
      require(key(IBUS_Alt_R, IBUS_MOD1_MASK | IBUS_RELEASE_MASK),
              "Right Alt release leaked after a dead-key voice hold in English mode");
      require(wait_until([&] { return voice_provider.stop_requests.load() == voice_stops + 1; }),
              "Right Alt release did not stop the English-mode recording");
      voice_provider.release_final = true;
      require(wait_until([&] { return seen.committed == "synthetic voice"; }),
              "English-mode dead-key voice hold did not commit");
      seen.committed.clear();
      // Ctrl+F9 starts a recording in English mode; a Shift switch to Chinese leaves it running, and the provider text is committed without an Engine session to confirm it.
      voice_starts = voice_provider.started.load();
      voice_cancels = voice_provider.cancelled.load();
      voice_stops = voice_provider.stop_requests.load();
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 was not consumed in English mode");
      require(wait_until([&] { return voice_provider.started.load() == voice_starts + 1; }),
              "Ctrl+F9 did not start recording in English mode");
      voice_provider.release_partial = true;
      require(wait_until([&] { return seen.preedit == "测试😀" && seen.preedit_visible; }),
              "Ctrl+F9 English-mode recording did not show streaming preedit");
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Shift did not switch to Chinese during an English-mode recording");
      require(seen.preedit == "测试😀" && seen.preedit_visible,
              "Switching modes took down the voice preedit");
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 did not stop the recording after the mode switch");
      require(wait_until([&] { return voice_provider.stop_requests.load() == voice_stops + 1; }),
              "Ctrl+F9 stop did not reach the provider after the mode switch");
      voice_provider.release_final = true;
      require(wait_until([&] { return seen.committed == "synthetic voice"; }),
              "English-mode recording did not commit the provider text");
      require(voice_provider.cancelled.load() == voice_cancels && !seen.preedit_visible,
              "Switching modes cancelled an English-mode recording");
      seen.committed.clear();
      // A recording bound to the Engine session in Chinese mode survives a switch to English and still goes through the Engine.
      voice_starts = voice_provider.started.load();
      voice_stops = voice_provider.stop_requests.load();
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 was not consumed in Chinese mode");
      require(wait_until([&] { return voice_provider.started.load() == voice_starts + 1; }),
              "Ctrl+F9 did not start recording in Chinese mode");
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) && !seen.input_enabled,
              "Shift did not switch to English during a Chinese-mode recording");
      require(key(IBUS_F9, IBUS_CONTROL_MASK) &&
                  key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "Ctrl+F9 did not stop the Chinese-mode recording in English mode");
      require(wait_until([&] { return voice_provider.stop_requests.load() == voice_stops + 1; }),
              "Ctrl+F9 stop did not reach the provider in English mode");
      voice_provider.release_final = true;
      require(wait_until([&] { return seen.committed == "synthetic voice"; }),
              "Chinese-mode recording did not commit after switching to English");
      require(voice_provider.cancelled.load() == voice_cancels,
              "Switching to English cancelled a Chinese-mode recording");
      seen.committed.clear();
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // Under the "follow" lock the mode switch re-resolves punctuation: a Ctrl+. choice made in Chinese mode does not survive a round trip through English mode, and English mode leaves ASCII marks alone.
      auto follow = options;
      follow.erase("preferences_directory");
      follow["preferences"]["ime_mode_scope"] = "app";
      follow["preferences"]["default_ime_mode"] = "chinese";
      follow["preferences"]["character_width"] = "halfwidth";
      follow["preferences"]["punctuation_lock"] = "follow";
      follow["preferences"]["chinese_punctuation"] = true;
      follow["preferences"]["keybindings"]["switch_language_shift"] = true;
      msime_ibus_configure(follow.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      if (!seen.input_enabled) {
        key(IBUS_space, IBUS_CONTROL_MASK);
        key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      }
      require(seen.input_enabled && seen.punctuation_enabled,
              "Follow fixture did not start in Chinese mode with Chinese punctuation");
      require(key(IBUS_period, IBUS_CONTROL_MASK) && !seen.punctuation_enabled,
              "Ctrl+. did not turn Chinese punctuation off in the follow fixture");
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) &&
                  !seen.input_enabled,
              "Bare Shift release was consumed or did not enter English mode");
      require(!key(IBUS_comma) && seen.committed.empty(),
              "Follow lock converted a comma in English mode");
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) &&
                  seen.input_enabled,
              "Bare Shift release was consumed or did not restore Chinese mode");
      require(seen.punctuation_enabled,
              "Switching back to Chinese mode under the follow lock did not restore Chinese punctuation");
      // Ctrl+. in English mode turns Chinese punctuation on for English mode too, as Windows does with the IME closed; a refocus keeps it and the next mode switch drops it.
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) &&
                  !seen.input_enabled && !seen.punctuation_enabled,
              "Follow fixture did not return to English mode with ASCII punctuation");
      require(key(IBUS_period, IBUS_CONTROL_MASK) && seen.committed.empty() &&
                  seen.punctuation_enabled,
              "English-mode Ctrl+. was not consumed or did not turn Chinese punctuation on");
      require(!key(IBUS_period, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
              "English-mode Ctrl+. release was consumed");
      require(key(IBUS_comma) && seen.committed == "，",
              "English mode did not convert a comma after Ctrl+.");
      seen.committed.clear();
      invoke("FocusOut");
      invoke("FocusIn");
      require(!seen.input_enabled && key(IBUS_comma) && seen.committed == "，",
              "Refocus dropped the English-mode Ctrl+. choice");
      seen.committed.clear();
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) &&
                  seen.input_enabled,
              "Bare Shift did not switch to Chinese after English-mode Ctrl+.");
      require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK) &&
                  !seen.input_enabled,
              "Bare Shift did not switch back to English after English-mode Ctrl+.");
      require(!key(IBUS_comma) && seen.committed.empty() && !seen.punctuation_enabled,
              "A mode round trip did not drop the English-mode Ctrl+. choice");
#if IBUS_CHECK_VERSION(1, 5, 27)
      // A mode the focus restores is a mode switch too: toggling Ctrl+. twice in an English app leaves nothing behind for a Chinese app focused next.
      invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/english-app", "msime-english-app"));
      require(!seen.input_enabled, "Naming the English-mode focus changed its mode");
      for (int press = 0; press < 2; ++press) {
        require(key(IBUS_period, IBUS_CONTROL_MASK) &&
                    !key(IBUS_period, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
                "English-mode Ctrl+. was not consumed in a named app");
        require(seen.punctuation_enabled == (press == 0),
                "English-mode Ctrl+. did not toggle punctuation in a named app");
      }
      require(!seen.punctuation_enabled,
              "Two English-mode Ctrl+. presses did not return to ASCII punctuation");
      invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/chinese-app", "msime-chinese-app"));
      require(seen.input_enabled && seen.punctuation_enabled,
              "An app restored to Chinese inherited the English-mode Ctrl+. choice of the previous app");
#endif
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // The "always English punctuation" lock keeps English mode ASCII whatever Ctrl+. says.
      auto english_lock = follow;
      english_lock["preferences"]["default_ime_mode"] = "english";
      english_lock["preferences"]["punctuation_lock"] = "english";
      msime_ibus_configure(english_lock.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      require(!seen.input_enabled, "English lock fixture did not start in English mode");
      require(key(IBUS_period, IBUS_CONTROL_MASK) && seen.committed.empty(),
              "English-mode Ctrl+. was not consumed under the English punctuation lock");
      require(!key(IBUS_comma) && seen.committed.empty(),
              "English punctuation lock converted a comma after Ctrl+. in English mode");
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // The same lock holds in Chinese mode, as Windows resolves Ctrl+. and the toolbar switch through ResolvePunctuationOpen: the chord is eaten and Chinese punctuation stays off.
      auto chinese_english_lock = english_lock;
      chinese_english_lock["preferences"]["default_ime_mode"] = "chinese";
      msime_ibus_configure(chinese_english_lock.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      if (!seen.input_enabled) {
        key(IBUS_space, IBUS_CONTROL_MASK);
        key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      }
      require(seen.input_enabled && !seen.punctuation_enabled,
              "Chinese-mode English lock fixture did not start in Chinese mode with ASCII punctuation");
      require(key(IBUS_period, IBUS_CONTROL_MASK) && seen.committed.empty() &&
                  !seen.punctuation_enabled,
              "Chinese-mode Ctrl+. was not consumed or overrode the English punctuation lock");
      key(IBUS_period, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      invoke("PropertyActivate", g_variant_new("(su)", "Punctuation", PROP_STATE_CHECKED));
      require(!seen.punctuation_enabled,
              "The Chinese punctuation menu overrode the English punctuation lock");
      key(IBUS_comma);
      require(seen.committed.find("，") == std::string::npos,
              "Chinese mode converted a comma under the English punctuation lock");
      seen.committed.clear();
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // With smart punctuation off, Chinese punctuation mode types the Chinese mark for every key, as Windows _ResolveSmartPunctuation returns ResolvePunctuation unchanged.
      auto smart_off = follow;
      smart_off["preferences"]["smart_punctuation"] = false;
      msime_ibus_configure(smart_off.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      if (!seen.input_enabled) {
        key(IBUS_space, IBUS_CONTROL_MASK);
        key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      }
      require(seen.input_enabled && seen.punctuation_enabled,
              "Smart-off fixture did not start in Chinese mode with Chinese punctuation");
      require(key(IBUS_comma) && seen.committed == "，",
              "Chinese punctuation with smart punctuation off did not convert a comma");
      seen.committed.clear();
      require(key(IBUS_period) && seen.committed == "。",
              "Chinese punctuation with smart punctuation off did not convert a period");
      seen.committed.clear();
      invoke("FocusOut");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      // 重复标点转中文 depends only on smart punctuation and its repeat switch, as Windows _CanInterceptSmartPunctuationRevert does; paired completion is a separate feature. The mock editor publishes what it holds before each key.
      auto repeat_unpaired = follow;
      repeat_unpaired["preferences"]["paired_punctuation"] = false;
      repeat_unpaired["preferences"]["smart_punctuation"] = true;
      repeat_unpaired["preferences"]["smart_punctuation_repeat"] = true;
      msime_ibus_configure(repeat_unpaired.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      if (!seen.input_enabled) {
        key(IBUS_space, IBUS_CONTROL_MASK);
        key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      }
      require(seen.input_enabled && seen.punctuation_enabled,
              "Unpaired repeat fixture did not start in Chinese mode with Chinese punctuation");
      auto editor_holds = [&](const char *value, guint cursor) {
        auto text = ibus_text_new_from_string(value);
        g_object_ref_sink(text);
        invoke("SetSurroundingText",
               g_variant_new("(vuu)", ibus_serializable_serialize(IBUS_SERIALIZABLE(text)),
                             cursor, cursor));
        g_object_unref(text);
      };
      editor_holds("a", 1);
      require(key(IBUS_comma) && seen.committed == ",",
              "Smart punctuation did not keep a comma after a letter ASCII");
      editor_holds("a,", 2);
      const auto deletes_before_repeat = seen.delete_surrounding_calls;
      require(key(IBUS_comma) && seen.committed == ",，" &&
                  seen.delete_surrounding_calls == deletes_before_repeat + 1 &&
                  seen.delete_surrounding_offset == -1 && seen.delete_surrounding_count == 1,
              "A repeated comma did not turn Chinese with paired completion off");
      seen.committed.clear();
      invoke("FocusOut");
    }
    for (const auto *scope : {"app", "global"}) {
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      auto initial = options;
      initial["preferences"].erase("default_ime_mode");
      initial["preferences"]["ime_mode_scope"] = scope;
      initial["preferences"]["keybindings"]["switch_language_shift"] = false;
      initial.erase("preferences_directory");
      msime_ibus_configure(initial.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      require(seen.mode_registered && seen.input_enabled &&
                  !seen.preedit_visible && !seen.lookup_visible,
              "Missing default mode did not use Windows Chinese input");
      invoke("FocusOut");
      invoke("FocusIn");
      require(seen.mode_sensitive && seen.input_enabled && seen.smart_punctuation_sensitive,
              "Chinese refocus did not restore the mode menu with a session");
      // The configured Ctrl shortcut switches the mode and the scope remembers the new one - that is what Windows does, whichever scope is configured, and what this host does. Like Windows, the release toggles but still reaches the application, so the switch is read from the published mode. Toggle twice, and check the mode each time.
      require(!key(IBUS_Control_L, IBUS_CONTROL_MASK) &&
                  !key(IBUS_Control_L, IBUS_RELEASE_MASK),
              "Configured Ctrl shortcut consumed a modifier event");
      require(!seen.input_enabled,
              "Configured Ctrl shortcut did not leave Chinese input");
      require(!key(IBUS_Control_L, IBUS_CONTROL_MASK) &&
                  !key(IBUS_Control_L, IBUS_RELEASE_MASK),
              "Configured Ctrl shortcut consumed a modifier event in passthrough");
      require(seen.input_enabled,
              "Configured Ctrl shortcut did not return to Chinese input");
      phrase();
      require(seen.input_enabled && !seen.english_mode && seen.preedit == "nihao" &&
                  !seen.candidates.empty() && seen.candidates.front() == "你好",
              "Chinese default did not restore Chinese candidates");
      require(preedit_underlined(seen), "Typed composition preedit was not single-underlined");
      invoke("Reset");
      invoke("FocusOut");
      invoke("FocusIn");
      require(seen.input_enabled && key('n') && !seen.english_mode,
              "Refocus failed to preserve the selected Chinese mode");
      invoke("Reset");
      invoke("PropertyActivate", g_variant_new("(su)", "ShuangpinProfile/ziranma", PROP_STATE_CHECKED));
      require(seen.input_enabled && key('n') && !seen.english_mode,
              "Session recreation failed to preserve the selected Chinese mode");
      invoke("Reset");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      require(seen.input_enabled && key('n') && !seen.english_mode,
              "New host did not apply the Chinese default after mode reset");
      invoke("Reset");
      // Switching input sources disables the engine; ordinary refocus above
      // must preserve mode, but reactivation starts from the configured default.
      invoke("PropertyActivate", g_variant_new("(su)", "InputMode", PROP_STATE_CHECKED));
      invoke("Disable");
      invoke("FocusIn");
      invoke("Enable");
      require(key('n'), "Input source reactivation did not restore the Chinese default");
      require(seen.input_enabled && seen.mode_sensitive && seen.preedit_visible,
              "Input source reactivation did not restore the Chinese presentation");
    }
    ibus_object_destroy(IBUS_OBJECT(engine));
    g_object_unref(engine);
    {
      const auto socket = (root / "online.sock").string();
      TranslationProviderFixture provider(socket);
      auto online = options;
      online.erase("preferences_directory");
      online["online_provider_socket"] = socket;
      online["preferences"]["cloud_candidates"] = true;
      online["preferences"]["candidate_page_size"] = 9;
      online["preferences"]["ai_assistant"]["enabled"] = false;
      msime_ibus_configure(online.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      auto settle_online = [&] {
        const auto deadline = g_get_monotonic_time() + 1700000;
        while (g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
      };
      // Online requests are debounced, so settling for a fixed duration before
      // reading an exact count is a race the loaded machine loses. Wait for the
      // request to arrive, then settle to prove no second one follows it.
      auto await_online = [&](unsigned expected) {
        const auto deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
        while (provider.online_requests.load() < expected &&
               g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
        settle_online();
        return provider.online_requests.load() == expected;
      };
      phrase();
      require(await_online(1),
              ("Synthetic online provider request count is not 1: " +
               std::to_string(provider.online_requests))
                  .c_str());
      invoke("Reset");
      phrase();
      require(await_online(2),
              ("New input did not request cloud candidates exactly once: " +
               std::to_string(provider.online_requests))
                  .c_str());
      invoke("PropertyActivate", g_variant_new("(su)", "CloudCandidates", PROP_STATE_UNCHECKED));
      invoke("Reset");
      phrase();
      settle_online();
      require(provider.online_requests == 2,
              "Disabled cloud and AI still dispatched an online request");
      provider.return_online_candidate = true;
      invoke("PropertyActivate", g_variant_new("(su)", "CloudCandidates", PROP_STATE_CHECKED));
      require(await_online(3),
              ("Re-enabled cloud candidates did not request input: " +
               std::to_string(provider.online_requests))
                  .c_str());
      require(std::any_of(seen.candidates.begin(), seen.candidates.end(),
                          [](const std::string &text) { return text == "云端测试  云"; }),
              "Cloud reply lost Engine identity when AI was disabled");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      provider.return_online_candidate = false;
      online["preferences"]["cloud_candidates"] = false;
      online["preferences"]["ai_assistant"]["enabled"] = true;
      // Candidate translations share the fixture socket through the online fallback, so the private-field block below can prove translation requests stop there too.
      online["preferences"]["candidate_translations"] = true;
      online["preferences"]["translation_target_language"] = "fr";
      msime_ibus_configure(online.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      for (char c : std::string("zaijian"))
        require(key(c), "AI-only fixture input was not consumed");
      settle_online();
      require(provider.online_requests == 4,
              ("Disabling cloud also disabled configured AI requests: " +
               std::to_string(provider.online_requests))
                  .c_str());
      // Windows AiAssistant shows a cached answer before the idle delay, so every input change also sends an immediate cache-only probe; those never reach the network and are not part of the debounced count above.
      require(provider.ai_cache_probes > 0, "AI input did not send the immediate cache probe");
      // Private and no-spellcheck fields keep their pinyin and committed text on the machine, as Fcitx5 does: no cloud, AI or translation request while typing there, and nothing committed there reaches the next field's AI context.
      require(provider.requests > 0, "Ordinary field did not request candidate translations");
      auto content_hints = [&](guint hints) {
        invoke("Set", g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                                    g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM, hints)));
      };
      content_hints(IBUS_INPUT_HINT_PRIVATE);
      const auto translations_before = provider.requests.load();
      const auto probes_before = provider.ai_cache_probes.load();
      const auto private_before = seen.committed;
      phrase();
      require(key(IBUS_space) && seen.committed == private_before + "你好",
              "Private field did not commit the phrase");
      for (char c : std::string("zaijian"))
        require(key(c), "Private field input was not consumed");
      settle_online();
      require(provider.online_requests == 4,
              ("Private field dispatched an online request: " +
               std::to_string(provider.online_requests))
                  .c_str());
      require(provider.requests == translations_before,
              "Private field dispatched a candidate translation request");
      require(provider.ai_cache_probes == probes_before,
              "Private field dispatched an AI cache probe");
      content_hints(0);
      for (char c : std::string("zaijian"))
        require(key(c), "Ordinary field input was not consumed after a private field");
      require(await_online(5),
              ("Ordinary field after a private field did not resume AI requests: " +
               std::to_string(provider.online_requests))
                  .c_str());
      require(provider.requests > translations_before,
              "Ordinary field after a private field did not resume candidate translations");
      const auto contexts = provider.online_ai_contexts();
      require(!contexts.empty() && contexts.back().find("你好") == std::string::npos,
              "Text committed in a private field reached the next AI context");
      content_hints(IBUS_INPUT_HINT_NO_SPELLCHECK);
      const auto no_spellcheck_translations = provider.requests.load();
      for (char c : std::string("zaijian"))
        require(key(c), "No-spellcheck field input was not consumed");
      settle_online();
      require(provider.online_requests == 5, "No-spellcheck field dispatched an online request");
      require(provider.requests == no_spellcheck_translations,
              "No-spellcheck field dispatched a candidate translation request");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      auto offline = options;
      offline.erase("preferences_directory");
      offline["preferences"]["candidate_translations"] = false;
      offline["preferences"]["candidate_english_gloss"] = true;
      msime_ibus_configure(offline.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      phrase();
      const auto deadline = g_get_monotonic_time() + 2000000;
      while ((seen.candidates.empty() || seen.candidates.front().find(" · ") == std::string::npos) &&
             g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(!seen.candidates.empty() && seen.candidates.front().find("你好 · ") == 0,
              "Packaged offline gloss did not render without a provider");
      require(seen.preedit == "nihao" && seen.committed.empty(),
              "Offline gloss changed the active composition");
      require(key(IBUS_space) && seen.committed == "你好",
              "Offline gloss leaked into committed candidate text");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      auto offline = options;
      offline.erase("preferences_directory");
      offline["preferences"]["candidate_translations"] = false;
      offline["preferences"]["candidate_english_gloss"] = false;
      msime_ibus_configure(offline.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      phrase();
      const auto deadline = g_get_monotonic_time() + 800000;
      while (g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(std::none_of(seen.candidates.begin(), seen.candidates.end(),
                           [](const std::string &text) {
                             return text.find(" · ") != std::string::npos;
                           }),
              "Disabled English gloss unexpectedly rendered");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      const auto socket = (root / "translation.sock").string();
      TranslationProviderFixture provider(socket);
      auto translated = options;
      translated.erase("preferences_directory");
      translated["translation_provider_socket"] = socket;
      translated["preferences"]["translation_target_language"] = "fr";
      translated["preferences"]["candidate_page_size"] = 9;
      translated["preferences"]["candidate_translations"] = false;
      msime_ibus_configure(translated.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      auto translated_page = [&] {
        return !seen.candidates.empty() &&
               seen.candidates.front().find("synthetic gloss") != std::string::npos;
      };
      auto wait_translation = [&] {
        const auto deadline = g_get_monotonic_time() + 2000000;
        while (!translated_page() && g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
        require(translated_page(), "Synthetic candidate translation did not render");
      };
      phrase();
      require(!translated_page(), "Initially disabled translations showed glosses");
      invoke("PropertyActivate", g_variant_new("(su)", "CandidateTranslations", PROP_STATE_CHECKED));
      require(seen.preedit == "nihao" && seen.committed.empty(),
              "Enabling translation disturbed the active composition");
      wait_translation();
      const auto settled = g_get_monotonic_time() + 1200000;
      while (g_get_monotonic_time() < settled) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(provider.requests == 1, "Unchanged translation page repeated provider requests");
      invoke("PropertyActivate", g_variant_new("(su)", "TranslationLanguage/de", PROP_STATE_CHECKED));
      wait_translation();
      require(provider.requests == 2, "Translation target change did not request a new page");
      invoke("Reset");
      phrase();
      wait_translation();
      require(provider.requests == 3, "New composition reused a stale translation request");
      invoke("PropertyActivate", g_variant_new("(su)", "CandidateTranslations", PROP_STATE_UNCHECKED));
      require(!translated_page(), "Disabling candidate translations left glosses visible");
      invoke("PropertyActivate", g_variant_new("(su)", "CandidateTranslations", PROP_STATE_CHECKED));
      wait_translation();
      require(provider.requests == 4, "Re-enabling translation did not refresh the current page");
      provider.hold_responses = true;
      invoke("PropertyActivate", g_variant_new("(su)", "TranslationLanguage/en", PROP_STATE_CHECKED));
      const auto local_deadline = g_get_monotonic_time() + 2000000;
      while ((provider.requests < 5 || seen.candidates.empty() ||
              seen.candidates.front().find("你好 · ") != 0 || translated_page()) &&
             g_get_monotonic_time() < local_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(provider.requests == 5 && !seen.candidates.empty() &&
                  seen.candidates.front().find("你好 · ") == 0 && !translated_page(),
              "Offline hits waited for the online fallback response");
      const auto local_hit = seen.candidates.front();
      require(provider.english_greeting_requests == 0,
              "Offline dictionary hit was also sent to the online provider");
      provider.hold_responses = false;
      // Releasing the provider is not the last step: the reply is read on a
      // worker thread, merged on an idle turn, and any re-query behind it waits
      // out the 500ms translation debounce and a 150ms settle tick. Two seconds
      // covered that only when the machine was idle. This asserts that the miss
      // eventually merges, not how fast.
      const auto remote_deadline = g_get_monotonic_time() + 8 * G_USEC_PER_SEC;
      auto has_remote_gloss = [&] {
        return std::any_of(seen.candidates.begin(), seen.candidates.end(),
            [](const std::string &text) { return text.find("synthetic gloss") != std::string::npos; });
      };
      while (!has_remote_gloss() && g_get_monotonic_time() < remote_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      {
        std::string observed;
        for (const auto &candidate : seen.candidates)
          observed += "[" + candidate + "]";
        require(has_remote_gloss() && seen.candidates.front() == local_hit,
                ("Online misses did not merge with the displayed offline hits: gloss=" +
                 std::to_string(has_remote_gloss()) + " local_hit=[" + local_hit +
                 "] candidates=" + observed)
                    .c_str());
      }
      // A fresh host with no online socket must reuse the persisted misses.
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      auto learned = translated;
      learned.erase("translation_provider_socket");
      learned["preferences"]["candidate_translations"] = true;
      learned["preferences"]["translation_target_language"] = "en";
      msime_ibus_configure(learned.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      phrase();
      const auto learned_deadline = g_get_monotonic_time() + 2000000;
      while (!has_remote_gloss() && g_get_monotonic_time() < learned_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(has_remote_gloss() && provider.requests == 5,
              "Learned translation did not survive host restart without an online provider");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
      learned["translation_provider_socket"] = socket;
      msime_ibus_configure(learned.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      provider.tag_responses = true;
      // Observe every publication, not only the final page: a stale gloss
      // must never flash after changing target, composition or focus.
      for (unsigned boundary : {0u, 1u, 2u}) {
        provider.hold_responses = true;
        const auto previous = provider.requests.load();
        invoke("PropertyActivate", g_variant_new("(su)", "TranslationLanguage/fr", PROP_STATE_CHECKED));
        invoke("Reset");
        phrase();
        const auto pending_deadline = g_get_monotonic_time() + 2000000;
        while (provider.requests == previous && g_get_monotonic_time() < pending_deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
        require(provider.requests == previous + 1, "Delayed translation request did not start");
        seen.forbidden_gloss = "synthetic gloss [" + std::to_string(previous + 1) + "]";
        seen.forbidden_gloss_seen = false;
        if (boundary == 0) {
          invoke("PropertyActivate", g_variant_new("(su)", "TranslationLanguage/de", PROP_STATE_CHECKED));
        } else {
          if (boundary == 2) {
            invoke("FocusOut");
            invoke("FocusIn");
          } else {
            invoke("Reset");
          }
          phrase();
        }
        provider.hold_responses = false;
        const auto expected = "synthetic gloss [" + std::to_string(previous + 2) + "]";
        const auto replacement_deadline = g_get_monotonic_time() + 2500000;
        auto replacement_visible = [&] {
          return !seen.candidates.empty() && seen.candidates.front().find(expected) != std::string::npos;
        };
        while (!replacement_visible() && g_get_monotonic_time() < replacement_deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
        require(replacement_visible() && provider.requests == previous + 2,
                "New translation state did not replace the delayed request");
        require(!seen.forbidden_gloss_seen,
                "Delayed translation was published across a state boundary");
        require(seen.preedit == "nihao" && seen.committed.empty(),
                "Delayed translation changed composition or committed text");
        seen.forbidden_gloss.clear();
      }
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      const auto socket = (root / "translation-preferences.sock").string();
      TranslationProviderFixture provider(socket);
      provider.tag_responses = true;
      const auto directory = root / "translation-preferences";
      std::filesystem::create_directory(directory);
      auto translated = options;
      translated["translation_provider_socket"] = socket;
      translated["preferences_directory"] = directory.string();
      translated["preferences"]["candidate_translations"] = true;
      translated["preferences"]["candidate_english_gloss"] = false;
      translated["preferences"]["translation_target_language"] = "fr";
      translated["preferences"]["custom_translation"]["enabled"] = false;
      translated["preferences"]["niutrans"]["enabled"] = false;
      auto save = [&](unsigned revision) {
        std::ofstream(directory / "next.json") << nlohmann::json{
            {"format_version", 1}, {"revision", revision},
            {"preferences", translated.at("preferences")}}.dump();
        std::filesystem::rename(directory / "next.json",
                                directory / "preferences.json");
      };
      save(1);
      msime_ibus_configure(translated.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      phrase();
      const auto first_deadline =
          g_get_monotonic_time() + 3 * G_USEC_PER_SEC;
      while ((seen.candidates.empty() ||
              seen.candidates.front().find("synthetic gloss [1]") ==
                  std::string::npos) &&
             g_get_monotonic_time() < first_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(provider.requests == 1 && !seen.candidates.empty() &&
                  seen.candidates.front().find("synthetic gloss [1]") !=
                      std::string::npos,
              "Initial translation provider result did not render");
      seen.committed.clear();
      auto translation_commit = call(
          client, destination, "ProcessKeyEvent",
          g_variant_new("(uuu)", IBUS_Return, 0, IBUS_CONTROL_MASK));
      gboolean translation_handled = FALSE;
      g_variant_get(translation_commit, "(b)", &translation_handled);
      g_variant_unref(translation_commit);
      // The candidate window is hidden on a 24ms timer, not in the same turn as
      // the commit: the host defers it so a composition that briefly empties its
      // candidate list does not flicker the panel. Wait for the condition rather
      // than reading a value that is deliberately not there yet.
      const auto translation_hidden = g_get_monotonic_time() + 2 * G_USEC_PER_SEC;
      while (seen.lookup_visible && g_get_monotonic_time() < translation_hidden) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(translation_handled &&
                  seen.committed.find("synthetic gloss [1]") != std::string::npos &&
                  !seen.preedit_visible && !seen.lookup_visible,
              ("Ctrl+Enter did not commit the rendered candidate translation: handled=" +
               std::to_string(static_cast<int>(translation_handled)) + " committed=[" +
               seen.committed + "] preedit=" + std::to_string(seen.preedit_visible) +
               " lookup=" + std::to_string(seen.lookup_visible))
                  .c_str());
      invoke("Reset");
      provider.hold_responses = true;
      translated["preferences"]["niutrans"] = {
          {"enabled", true}, {"app_id", "synthetic-app"},
          {"apikey", "synthetic-key"}};
      save(2);
      // Ctrl+Enter above committed the gloss and cancelled the composition, so
      // there are no candidates left. `translation_schedule` returns early when
      // the view has none - correctly, since there is nothing to translate - so
      // without composing again this wait could never end. What is checked here
      // is therefore that the next composition asks the provider again rather
      // than reusing the gloss from before the preference change; it does not
      // also prove the request was made for candidates that were already on
      // screen, which this fixture has no composition left to show.
      phrase();
      const auto changed_deadline =
          g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
      auto old_translation_visible = [&] {
        return std::any_of(
            seen.candidates.begin(), seen.candidates.end(),
            [](const std::string &text) {
              return text.find("synthetic gloss [1]") != std::string::npos;
            });
      };
      while ((provider.requests < 2 || old_translation_visible()) &&
             g_get_monotonic_time() < changed_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(provider.requests == 2,
              "NiuTrans preference change did not request a new translation");
      require(!old_translation_visible(),
              "NiuTrans preference change left the previous provider gloss visible");
      provider.hold_responses = false;
      const auto replacement_deadline =
          g_get_monotonic_time() + 3 * G_USEC_PER_SEC;
      while ((seen.candidates.empty() ||
              seen.candidates.front().find("synthetic gloss [2]") ==
                  std::string::npos) &&
             g_get_monotonic_time() < replacement_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(!seen.candidates.empty() &&
                  seen.candidates.front().find("synthetic gloss [2]") !=
                      std::string::npos,
              "Updated translation provider result did not replace the cleared gloss");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      const auto socket = (root / "translation-multi-sense.sock").string();
      TranslationProviderFixture provider(socket);
      provider.multi_sense = true;
      auto translated = options;
      translated.erase("preferences_directory");
      translated["translation_provider_socket"] = socket;
      translated["preferences"]["candidate_translations"] = true;
      translated["preferences"]["candidate_page_size"] = 2;
      // Not English. The packaged msime-english.db answers 你好 offline with the single
      // sense "hello", and an offline hit is shown without ever reaching the
      // provider - which is the documented behaviour and what Windows does. With
      // English as the target, the provider's multi-sense gloss therefore landed
      // on the second candidate only, the highlighted one carried "hello", and
      // Ctrl+Enter committed that single sense instead of opening the page this
      // case exists to check. Any other target language goes straight online.
      translated["preferences"]["translation_target_language"] = "ja";
      msime_ibus_configure(translated.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      phrase();
      const auto deadline = g_get_monotonic_time() + 3000000;
      while ((seen.candidates.empty() ||
              seen.candidates.front().find("first sense") == std::string::npos) &&
             g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(provider.requests == 1 && !seen.candidates.empty(),
              "Multi-sense translation did not render");
      seen.committed.clear();
      auto translation_commit = call(
          client, destination, "ProcessKeyEvent",
          g_variant_new("(uuu)", IBUS_Return, 0, IBUS_CONTROL_MASK));
      gboolean translation_handled = FALSE;
      g_variant_get(translation_commit, "(b)", &translation_handled);
      g_variant_unref(translation_commit);
      {
        std::string shown;
        for (const auto &candidate : seen.candidates)
          shown += (shown.empty() ? "" : "|") + candidate;
        require(translation_handled && seen.candidates.size() == 2 &&
                    seen.candidates[0] == "first sense" &&
                    seen.candidates[1] == "second sense" && seen.committed.empty(),
                ("Ctrl+Enter did not open the multi-sense translation page: handled=" +
                 std::to_string(static_cast<int>(translation_handled)) + " candidates=[" +
                 shown + "] committed=[" + seen.committed + "]")
                    .c_str());
      }
      invoke("CandidateClicked", g_variant_new("(uuu)", 1, 1, 0));
      // The candidate window closes on the host's 24ms anti-flicker timer, not in
      // the same turn as the commit; wait for it rather than for a duration.
      const auto sense_hidden = g_get_monotonic_time() + 2 * G_USEC_PER_SEC;
      while (seen.lookup_visible && g_get_monotonic_time() < sense_hidden) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      require(seen.committed == "second sense" && !seen.preedit_visible &&
                  !seen.lookup_visible,
              ("Selecting a translated sense did not commit and close the page: committed=[" +
               seen.committed + "] preedit=" + std::to_string(seen.preedit_visible) +
               " lookup=" + std::to_string(seen.lookup_visible))
                  .c_str());
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    // A panel that reports the wheel as CandidateClicked button 4/5 pages the translated senses only with 鼠标滚轮 on; with it off the wheel does nothing, as on Windows and on the ordinary candidate page. One sense per page so a page change is visible.
    {
      const auto socket = (root / "translation-wheel.sock").string();
      TranslationProviderFixture provider(socket);
      provider.multi_sense = true;
      for (const bool wheel : {false, true}) {
        auto translated = options;
        translated.erase("preferences_directory");
        translated["translation_provider_socket"] = socket;
        translated["preferences"]["candidate_translations"] = true;
        translated["preferences"]["candidate_page_size"] = 1;
        translated["preferences"]["translation_target_language"] = "ja";
        translated["preferences"]["navigation"]["mouse_wheel"] = wheel;
        msime_ibus_configure(translated.dump());
        engine = create_engine();
        seen = Observation{};
        invoke("FocusIn");
        phrase();
        const auto deadline = g_get_monotonic_time() + 3000000;
        while ((seen.candidates.empty() ||
                seen.candidates.front().find("first sense") == std::string::npos) &&
               g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {}
          g_usleep(1000);
        }
        require(!seen.candidates.empty() &&
                    seen.candidates.front().find("first sense") != std::string::npos,
                "Multi-sense translation did not render for the wheel check");
        seen.committed.clear();
        auto opened = call(client, destination, "ProcessKeyEvent",
                           g_variant_new("(uuu)", IBUS_Return, 0, IBUS_CONTROL_MASK));
        g_variant_unref(opened);
        require(seen.candidates == std::vector<std::string>{"first sense"},
                "Ctrl+Enter did not open the one-per-page translation list");
        invoke("CandidateClicked", g_variant_new("(uuu)", 0, 5, 0));
        const auto expected = wheel ? "second sense" : "first sense";
        require(seen.candidates == std::vector<std::string>{expected} &&
                    seen.committed.empty() && seen.lookup_visible,
                (std::string(wheel ? "Wheel with 鼠标滚轮 on did not page"
                                   : "Wheel with 鼠标滚轮 off paged") +
                 " the translation list: shown=[" +
                 (seen.candidates.empty() ? std::string{} : seen.candidates.front()) + "]")
                    .c_str());
        ibus_object_destroy(IBUS_OBJECT(engine));
        g_object_unref(engine);
      }
    }
    // Tab and Shift+Tab page the translated senses like an ordinary candidate page while the shared Tab binding is on. With it off Tab keeps leaving the temporary page: the ordinary candidate is committed and the key goes to the application. One sense per page so a page change is visible.
    {
      const auto socket = (root / "translation-tab.sock").string();
      TranslationProviderFixture provider(socket);
      provider.multi_sense = true;
      for (const bool tab : {true, false}) {
        auto translated = options;
        translated.erase("preferences_directory");
        translated["translation_provider_socket"] = socket;
        translated["preferences"]["candidate_translations"] = true;
        translated["preferences"]["candidate_page_size"] = 1;
        translated["preferences"]["translation_target_language"] = "ja";
        translated["preferences"]["navigation"]["tab"] = tab;
        msime_ibus_configure(translated.dump());
        engine = create_engine();
        seen = Observation{};
        invoke("FocusIn");
        phrase();
        require(wait_until([&] {
                  return !seen.candidates.empty() &&
                         seen.candidates.front().find("first sense") != std::string::npos;
                }),
                "Multi-sense translation did not render for the Tab check");
        seen.committed.clear();
        auto opened = call(client, destination, "ProcessKeyEvent",
                           g_variant_new("(uuu)", IBUS_Return, 0, IBUS_CONTROL_MASK));
        g_variant_unref(opened);
        require(seen.candidates == std::vector<std::string>{"first sense"},
                "Ctrl+Enter did not open the one-per-page translation list for the Tab check");
        auto shown = [&] {
          return seen.candidates.empty() ? std::string{} : seen.candidates.front();
        };
        if (tab) {
          const auto expect_page = [&](bool handled, const char *expected, const char *what) {
            require(handled && seen.candidates == std::vector<std::string>{expected} &&
                        seen.committed.empty() && seen.lookup_visible,
                    (std::string(what) + " did not page the translation list: handled=" +
                     std::to_string(handled) + " shown=[" + shown() + "] committed=[" +
                     seen.committed + "]")
                        .c_str());
          };
          expect_page(key(IBUS_Tab), "second sense", "Tab");
          // The last page stays put, as Page Down does on the translation list.
          expect_page(key(IBUS_Tab), "second sense", "Tab on the last page");
          expect_page(key(IBUS_ISO_Left_Tab, IBUS_SHIFT_MASK), "first sense", "Shift+ISO_Left_Tab");
          expect_page(key(IBUS_Tab), "second sense", "Tab after paging back");
          expect_page(key(IBUS_Tab, IBUS_SHIFT_MASK), "first sense", "Shift+Tab");
        } else {
          const bool tab_forwarded = !key(IBUS_Tab);
          settle_lookup();
          require(tab_forwarded && !seen.committed.empty() &&
                      seen.committed.find("sense") == std::string::npos &&
                      !seen.preedit_visible && !seen.lookup_visible,
                  ("Disabled Tab did not leave the translation list for the application: forwarded=" +
                   std::to_string(tab_forwarded) + " committed=[" + seen.committed +
                   "] preedit=" + std::to_string(seen.preedit_visible) + " lookup=" +
                   std::to_string(seen.lookup_visible))
                      .c_str());
        }
        ibus_object_destroy(IBUS_OBJECT(engine));
        g_object_unref(engine);
      }
    }
    {
      const auto oversized_history_path = root / "clipboard-oversized-history.json";
      std::ofstream(oversized_history_path)
          << nlohmann::json::array({"synthetic-old", std::string(1024 * 1024, 'x')}).dump();
      auto oversized_clipboard = options;
      oversized_clipboard["clipboard_history_path"] = oversized_history_path.string();
      oversized_clipboard["preferences"]["clipboard_history"] = true;
      msime_ibus_configure(oversized_clipboard.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      const auto oversized_deadline = g_get_monotonic_time() + 3000000;
      while (seen.clipboard_clear_name.empty() &&
             g_get_monotonic_time() < oversized_deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {
        }
        g_usleep(1000);
      }
      require(!seen.clipboard_clear_name.empty(),
              "Oversized clipboard history did not finish loading");
      require(!seen.clipboard_clear_sensitive,
              "Oversized clipboard history exposed entries to the menu");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    {
      const auto history_path = root / "clipboard-generation-history.json";
      std::ofstream(history_path)
          << nlohmann::json::array({"synthetic-old"}).dump();
      auto clipboard = options;
      clipboard["clipboard_history_path"] = history_path.string();
      clipboard["preferences"]["clipboard_history"] = true;
      msime_ibus_configure(clipboard.dump());
      engine = create_engine();
      seen = Observation{};
      invoke("FocusIn");
      auto wait_clipboard = [&](auto ready) {
        const auto deadline = g_get_monotonic_time() + 3000000;
        while (!ready() && g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {
          }
          g_usleep(1000);
        }
        return ready();
      };
      require(wait_clipboard([&] {
                return seen.clipboard_clear_sensitive &&
                       !seen.clipboard_clear_name.empty();
              }),
              "Synthetic clipboard history did not publish a clear action");
      const auto stale_clear = seen.clipboard_clear_name;
      const auto replacement = history_path.string() + ".next";
      std::ofstream(replacement)
          << nlohmann::json::array({"synthetic-new"}).dump();
      std::filesystem::rename(replacement, history_path);
      require(wait_clipboard([&] {
                return seen.clipboard_clear_sensitive &&
                       seen.clipboard_clear_name != stale_clear;
              }),
              "Clipboard replacement did not publish a fresh clear action");
      invoke("PropertyActivate",
             g_variant_new("(su)", stale_clear.c_str(), PROP_STATE_UNCHECKED));
      require(std::filesystem::exists(history_path) &&
                  nlohmann::json::parse(std::ifstream(history_path)) ==
                      nlohmann::json::array({"synthetic-new"}),
              "Stale clipboard clear action deleted refreshed history");
      invoke("PropertyActivate",
             g_variant_new("(su)", seen.clipboard_clear_name.c_str(),
                           PROP_STATE_UNCHECKED));
      require(!std::filesystem::exists(history_path),
              "Current clipboard clear action did not delete history");
      ibus_object_destroy(IBUS_OBJECT(engine));
      g_object_unref(engine);
    }
    msime_ibus_configure(options.dump());
    engine = create_engine();
    seen = Observation{};
    invoke("FocusIn");
    require(seen.mode_registered && seen.input_enabled && seen.mode_sensitive &&
                seen.clipboard_toggle_sensitive,
            "Initial input and clipboard properties were not available");
    require(seen.desktop_help && seen.desktop_feedback && seen.desktop_dictionary,
            "Linux desktop tools did not publish help, feedback and dictionary routes");
    {
      // The top of the menu follows the design order: 中文/英文; 全角/标点/译文; 输入方案; 主题/词库…/设置…/关于.
      const std::vector<std::string> design{
          "InputMode", "Separator/Mode", "CharacterMode", "Punctuation",
          "CandidateTranslations", "Separator/Switches", "Scheme", "Separator/Scheme", "GlobalTheme",
          "DesktopTools/Dictionary", "DesktopTools/Settings", "DesktopTools/About", "Separator/Design"};
      require(seen.registered_keys.size() > design.size() &&
                  std::equal(design.begin(), design.end(), seen.registered_keys.begin()),
              "IBus properties were not registered in the design menu order");
      // 中文/英文 is the Shift toggle alone; the Engine's dedicated English mode is nested in 输入选项.
      require(std::find(seen.registered_keys.begin(), seen.registered_keys.end(), "EnglishMode") ==
                  seen.registered_keys.end(),
              "Dedicated English mode was registered beside the design menu's 中文/英文");
    }
    {
      const auto panel_marker = root / "panel-launches.log";
      const auto panel_launcher = root / "panel-launcher";
      std::ofstream(panel_launcher)
          << "#!/bin/sh\ncase \"$1\" in --route=*) route=${1#--route=} ;; *) route=\"unexpected: $*\" ;; esac\nprintf '%s\\n' \"$route\" >> \""
          << panel_marker.string() << "\"\n";
      std::filesystem::permissions(panel_launcher,
                                   std::filesystem::perms::owner_read |
                                       std::filesystem::perms::owner_write |
                                       std::filesystem::perms::owner_exec,
                                   std::filesystem::perm_options::replace);
      auto panel_routes = [&] {
        std::vector<std::string> routes;
        std::ifstream input(panel_marker);
        for (std::string route; std::getline(input, route);)
          routes.push_back(std::move(route));
        return routes;
      };
      auto wait_panel = [&](auto ready) {
        const auto deadline = g_get_monotonic_time() + 2000000;
        while (!ready() && g_get_monotonic_time() < deadline) {
          while (g_main_context_iteration(nullptr, FALSE)) {
          }
          g_usleep(1000);
        }
        return ready();
      };
      g_setenv("MSIME_CLIENT_SETTINGS_COMMAND", panel_launcher.c_str(), TRUE);
      invoke("PropertyActivate",
             g_variant_new("(su)", "Toolbar/Emoji", PROP_STATE_UNCHECKED));
      require(wait_panel([&] { return panel_routes().size() == 1; }) &&
                  panel_routes().front() == "emoji",
              "Active toolbar panel action did not launch its route");
      invoke("FocusOut");
      invoke("PropertyActivate",
             g_variant_new("(su)", "Toolbar/Emoji", PROP_STATE_UNCHECKED));
      g_usleep(100000);
      while (g_main_context_iteration(nullptr, FALSE)) {
      }
      require(panel_routes().size() == 1,
              "Stale toolbar panel action launched after focus out");
      invoke("FocusIn");
      invoke("Set", g_variant_new(
                        "(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                        g_variant_new("(uu)", IBUS_INPUT_PURPOSE_PASSWORD, 0)));
      invoke("PropertyActivate",
             g_variant_new("(su)", "Toolbar/Emoji", PROP_STATE_UNCHECKED));
      g_usleep(100000);
      while (g_main_context_iteration(nullptr, FALSE)) {
      }
      require(panel_routes().size() == 1,
              "Toolbar panel action launched in a restricted field");
      invoke("Set",
             g_variant_new(
                 "(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                 g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM, 0)));
      invoke("PropertyActivate",
             g_variant_new("(su)", "DesktopTools/Help", PROP_STATE_UNCHECKED));
      invoke("PropertyActivate",
             g_variant_new("(su)", "DesktopTools/Feedback", PROP_STATE_UNCHECKED));
      invoke("PropertyActivate",
             g_variant_new("(su)", "DesktopTools/Dictionary", PROP_STATE_UNCHECKED));
      require(wait_panel([&] { return panel_routes().size() == 4; }),
              "Desktop help, feedback and dictionary actions did not launch settings routes");
      const auto routes = panel_routes();
      require(routes[1] == "settings:help" && routes[2] == "settings:feedback" &&
                  routes[3] == "settings:dictionary",
              "Desktop help, feedback and dictionary actions used incorrect settings routes");
      // Ctrl+Shift+Super+K opens the screen keyboard whether the client reports Super as MOD4, as the virtual SUPER bit, or as both (GTK3).
      for (guint super_bits : {guint(IBUS_MOD4_MASK), guint(IBUS_MOD4_MASK | IBUS_SUPER_MASK),
                               guint(IBUS_SUPER_MASK)}) {
        const auto launches = panel_routes().size();
        const guint chord = IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | super_bits;
        require(key(IBUS_K, chord) && key(IBUS_K, chord | IBUS_RELEASE_MASK),
                "Ctrl+Shift+Super+K was not consumed for every Super encoding");
        require(wait_panel([&] { return panel_routes().size() == launches + 1; }) &&
                    panel_routes().back() == "keyboard",
                "Ctrl+Shift+Super+K did not launch the screen keyboard");
      }
      g_unsetenv("MSIME_CLIENT_SETTINGS_COMMAND");
    }
    auto relative_preferences = options;
    relative_preferences["preferences_directory"] = "relative";
    invoke("FocusOut");
    msime_ibus_configure(relative_preferences.dump());
    invoke("FocusIn");
    require(!seen.clipboard_toggle_sensitive,
            "Relative preferences directory enabled the clipboard toggle");
    invoke("FocusOut");
    msime_ibus_configure(options.dump());
    invoke("FocusIn");
    require(seen.clipboard_toggle_sensitive,
            "Absolute preferences directory did not restore the clipboard toggle");
    for (const bool enabled : {false, true}) {
      invoke("PropertyActivate", g_variant_new("(su)", "InputMode",
          enabled ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED));
      invoke("FocusOut");
      require(!seen.mode_sensitive && !seen.smart_punctuation_sensitive,
              "Unfocused input menus remained available");
      invoke("FocusIn");
      require(seen.mode_sensitive && seen.input_enabled == enabled &&
                  seen.smart_punctuation_sensitive == enabled,
              ("Refocus did not restore mode-dependent menu availability: wanted=" +
               std::to_string(enabled) + " mode_sensitive=" +
               std::to_string(seen.mode_sensitive) + " input_enabled=" +
               std::to_string(seen.input_enabled) + " smart_punctuation_sensitive=" +
               std::to_string(seen.smart_punctuation_sensitive))
                  .c_str());
    }
    invoke("Set", g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                               g_variant_new("(uu)", IBUS_INPUT_PURPOSE_PASSWORD, 0)));
    invoke("FocusOut");
    invoke("FocusIn");
    require(!seen.mode_sensitive && !seen.smart_punctuation_sensitive && !key('n'),
            "Refocus enabled input menus in a blocked field");
    invoke("Set", g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                               g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM, 0)));
    invoke("PropertyActivate", g_variant_new("(su)", "InputMode", PROP_STATE_UNCHECKED));
    invoke("Disable");
    invoke("Enable");
    invoke("FocusIn");
    require(seen.input_enabled && key('n') && !seen.english_mode,
            "Input source reactivation did not restore default Chinese");
    invoke("Reset");
    phrase();
    invoke("FocusIn");
    require(seen.preedit_visible && seen.preedit == "nihao",
            "Repeated focus cancelled active composition");
#if IBUS_CHECK_VERSION(1, 5, 27)
    invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/context1", "msime-test"));
    require(seen.preedit_visible && seen.preedit == "nihao",
            "Delayed focus identity cancelled composition");
    invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/context1", "msime-test"));
    require(seen.preedit_visible && seen.preedit == "nihao",
            "Repeated focus identity cancelled composition");
    invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/context2", "msime-test"));
    require(!seen.preedit_visible && !seen.lookup_visible && !key(IBUS_Return),
            "Different context retained old composition");
    phrase();
    invoke("FocusOutId", g_variant_new("(s)", "/app/msime/test/context1"));
    require(seen.preedit_visible && seen.preedit == "nihao" && key('x'),
            "Old context focus loss cancelled the active context");
    invoke("FocusOutId", g_variant_new("(s)", "/app/msime/test/context2"));
    require(!seen.preedit_visible && !seen.lookup_visible && !key('n'),
            "Current context focus loss did not stop input");
    invoke("FocusIn");
    phrase();
    invoke("FocusOutId", g_variant_new("(s)", "/app/msime/test/legacy"));
    require(!seen.preedit_visible && !key('n'),
            "Focus loss without a known context identity did not stop input");
    invoke("FocusIn");
    // Client identity can arrive after surrounding text during negotiation.
    // Qt reports UTF-16 positions; other IBus clients report code points.
    for (const bool qt : {true, false}) {
      for (const int selection : {0, 1, -1}) {
        invoke("FocusOut");
        invoke("FocusIn");
        auto text = ibus_text_new_from_string("😀7水");
        g_object_ref_sink(text);
        const guint start = qt ? 3 : 2;
        const guint end = selection ? start + 1 : start;
        invoke("SetSurroundingText", g_variant_new("(vuu)",
            ibus_serializable_serialize(IBUS_SERIALIZABLE(text)),
            selection < 0 ? start : end, selection < 0 ? end : start));
        g_object_unref(text);
        invoke("FocusInId", g_variant_new("(ss)", "/app/msime/test/surrounding",
            qt ? "QIBusInputContext" : "msime-test"));
        const auto before = seen.committed;
        require(key(IBUS_period) && seen.committed == before + ".",
                "Delayed client identity lost the surrounding selection start");
      }
    }
    invoke("FocusOut");
    invoke("FocusIn");
    seen.committed.clear();
#endif
    const auto wait_saved_preferences = [&](const auto &ready) {
      const auto deadline = g_get_monotonic_time() + 2 * G_USEC_PER_SEC;
      gint64 ready_since = 0;
      while (g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        try {
          std::ifstream input(root / "preferences.json");
          nlohmann::json snapshot;
          input >> snapshot;
          if (ready(snapshot.at("preferences"))) {
            if (ready_since == 0)
              ready_since = g_get_monotonic_time();
            if (g_get_monotonic_time() - ready_since >= 50000)
              return true;
          } else {
            ready_since = 0;
          }
        } catch (...) {}
        g_usleep(1000);
      }
      return false;
    };
    invoke("PropertyActivate",
           g_variant_new("(su)", "DesktopTools/VoiceEnabled",
                         PROP_STATE_INCONSISTENT));
    invoke("PropertyActivate",
           g_variant_new("(su)", "NumberRowSelection", PROP_STATE_UNCHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return !preferences.value("number_row_selection", true) &&
                     preferences.value("voice_input", nlohmann::json::object())
                         .value("enabled", true);
            }),
            "Invalid desktop voice state occupied the menu save slot");
    phrase();
    const auto number_row_commit = seen.committed;
    require(!key(IBUS_1) && seen.committed == number_row_commit &&
                seen.preedit == "nihao" && seen.lookup_visible,
            "Disabled number-row selection did not release the digit");
    invoke("Reset");
    invoke("FocusOut");
    invoke("FocusIn");
    phrase();
    require(!key(IBUS_1) && seen.committed == number_row_commit,
            "Disabled number-row selection did not survive refocus");
    invoke("Reset");
    invoke("PropertyActivate",
           g_variant_new("(su)", "NumberRowSelection", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("number_row_selection", false);
            }),
            "Number-row selection restore was not persisted");
    phrase();
    const auto first_numbered_candidate = seen.candidates.front();
    require(key(IBUS_1) &&
                seen.committed == number_row_commit + first_numbered_candidate,
            "Restored number-row selection did not select the candidate");
    seen.committed.clear();
    invoke("Reset");
    require(!seen.autocorrect_properties_registered,
            "Autocorrect controls unexpectedly appeared in the IBus menu");
    invoke("PropertyActivate",
           g_variant_new("(su)", "CharacterMode", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("character_width", "halfwidth") ==
                     "fullwidth";
            }),
            "Fullwidth character mode was not persisted");
    // The saved width has to reach the session as well as the host flag: the raw spelling Enter commits is widened by the session, the idle digit after it by the host.
    const auto width_committed = seen.committed;
    const auto width_probe = [&] {
      seen.committed.clear();
      phrase();
      require(key(IBUS_Return), "Width probe spelling was not committed");
      settle_lookup();
      key('1');
      const auto committed = seen.committed;
      seen.committed.clear();
      return committed;
    };
    require(wait_until([&] { return seen.character_width; }),
            "Fullwidth menu save did not reach the host flag");
    require(width_probe() == "ｎｉｈａｏ１",
            "Fullwidth menu save did not reach the session");
    invoke("PropertyActivate",
           g_variant_new("(su)", "CharacterMode", PROP_STATE_UNCHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("character_width", "fullwidth") ==
                     "halfwidth";
            }),
            "Halfwidth character mode was not restored");
    require(wait_until([&] { return !seen.character_width; }) && width_probe() == "nihao",
            "Halfwidth menu save did not reach the session");
    // The settings page writes the store; the open session follows it on the next poll, with no focus change.
    const auto store_width = [&](const char *width) {
      nlohmann::json snapshot;
      {
        std::ifstream input(root / "preferences.json");
        input >> snapshot;
      }
      snapshot["revision"] = snapshot.at("revision").get<uint64_t>() + 1;
      snapshot["preferences"]["character_width"] = width;
      std::ofstream(root / "width-next.json") << snapshot.dump();
      std::filesystem::rename(root / "width-next.json", root / "preferences.json");
    };
    store_width("fullwidth");
    require(wait_until([&] { return seen.character_width; }) && width_probe() == "ｎｉｈａｏ１",
            "A fullwidth store did not reach the open session");
    store_width("halfwidth");
    require(wait_until([&] { return !seen.character_width; }) && width_probe() == "nihao",
            "A halfwidth store did not reach the open session");
    seen.committed = width_committed;
    require(seen.punctuation_enabled, "Chinese punctuation was not enabled");
    invoke("PropertyActivate",
           g_variant_new("(su)", "PunctuationLock/english", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("punctuation_lock", "follow") ==
                     "english";
            }),
            "English punctuation lock was not persisted");
    invoke("PropertyActivate",
           g_variant_new("(su)", "PunctuationLock/follow", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("punctuation_lock", "english") ==
                     "follow";
            }),
            "Follow punctuation lock was not restored");
    auto mode = [&](guint value) {
      invoke("PropertyActivate", g_variant_new("(su)", "InputMode", value));
    };
    require(key(IBUS_e, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
            "Ctrl+Shift+E was not consumed");
    require(seen.english_mode && seen.input_enabled, "Ctrl+Shift+E did not enter dedicated English mode");
    require(key(IBUS_e, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
            "Ctrl+Shift+E could not restore the input mode");
    require(!seen.english_mode && seen.input_enabled, "Ctrl+Shift+E did not restore Chinese mode");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            "Ctrl+Alt+Space was not consumed");
    require(!seen.input_enabled, "Ctrl+Alt+Space did not enter English mode");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_RELEASE_MASK),
            "Ctrl+Alt+Space release was not consumed");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            "Ctrl+Alt+Space could not restore input mode");
    require(seen.input_enabled, "Ctrl+Alt+Space did not restore input mode");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_RELEASE_MASK),
            "Ctrl+Alt+Space restore release was not consumed");
    require(key(IBUS_space, IBUS_CONTROL_MASK), "Ctrl+Space was not consumed");
    require(!seen.input_enabled, "Ctrl+Space did not enter English mode");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Ctrl+Space release was not consumed");
    require(key(IBUS_space, IBUS_CONTROL_MASK),
            "Ctrl+Space could not restore input mode");
    require(seen.input_enabled, "Ctrl+Space did not restore input mode");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Ctrl+Space restore release was not consumed");
    // Holding Ctrl+Space auto-repeats the press; the mode flips once for the whole stroke, as on Windows.
    for (int press = 0; press < 3; ++press)
      require(key(IBUS_space, IBUS_CONTROL_MASK) && !seen.input_enabled,
              "Held Ctrl+Space toggled the input mode more than once");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK) && !seen.input_enabled,
            "Held Ctrl+Space release was not consumed");
    require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK) && seen.input_enabled,
            "Ctrl+Space after a held stroke did not toggle again");
    // Once Ctrl+Alt+Space is consumed, the entire Space stroke belongs to
    // the shortcut even if its modifiers or configured binding change.
    for (guint remaining : {0u, guint(IBUS_CONTROL_MASK), guint(IBUS_MOD1_MASK),
                            guint(IBUS_CONTROL_MASK | IBUS_MOD1_MASK)}) {
      for (bool disable_binding : {false, true}) {
        require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK) &&
                    !seen.input_enabled,
                "Mode chord fixture did not disable input");
        if (disable_binding) {
          auto disabled = options;
          // Keep the store out of it: with a preferences directory the reload
          // tick re-reads the file and puts the binding back, so whether this
          // sees the injected preference depends on where the tick lands.
          disabled.erase("preferences_directory");
          disabled["preferences"]["keybindings"]["switch_language_ctrl_alt_space"] = false;
          msime_ibus_configure(disabled.dump());
          invoke("FocusIn");
        }
        require(key(IBUS_space, remaining) && !seen.input_enabled,
                "Consumed mode chord repeat escaped after modifiers or binding changed");
        require(key(IBUS_space, remaining | IBUS_RELEASE_MASK) && !seen.input_enabled,
                "Consumed mode chord release escaped after modifiers or binding changed");
        msime_ibus_configure(options.dump());
        invoke("FocusIn");
        require(!key(IBUS_space),
                "Completed mode chord consumed the next independent Space stroke");
        mode(PROP_STATE_CHECKED);
      }
    }
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK) && !seen.input_enabled,
            "Focus mode chord fixture did not disable input");
    invoke("FocusOut");
    invoke("FocusIn");
    require(key(IBUS_space, IBUS_CONTROL_MASK | IBUS_MOD1_MASK) && seen.input_enabled,
            "Consumed mode chord crossed a focus boundary");
    require(key(IBUS_space, IBUS_RELEASE_MASK),
            "Refocused mode chord release was not consumed");
    // Modifier chords must never be mistaken for a bare Ctrl/Shift release.
    for (bool ctrl_first : {true, false}) {
      for (bool ctrl_release_first : {true, false}) {
        const guint first = ctrl_first ? IBUS_Control_L : IBUS_Shift_L;
        const guint second = ctrl_first ? IBUS_Shift_L : IBUS_Control_L;
        const guint first_mask = ctrl_first ? IBUS_CONTROL_MASK : IBUS_SHIFT_MASK;
        require(!key(first, first_mask), "Modifier press was intercepted");
        require(!key(second, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
                "Modifier chord press was intercepted");
        const guint released = ctrl_release_first ? IBUS_Control_L : IBUS_Shift_L;
        const guint remaining = ctrl_release_first ? IBUS_Shift_L : IBUS_Control_L;
        const guint remaining_mask = ctrl_release_first ? IBUS_SHIFT_MASK : IBUS_CONTROL_MASK;
        require(!key(released, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
                "Modifier chord release toggled input");
        require(!key(remaining, IBUS_RELEASE_MASK | remaining_mask) && seen.input_enabled,
                "Modifier chord tail toggled input");
      }
    }
    for (guint modifier_key : {IBUS_Control_L, IBUS_Shift_L}) {
      require(!key(modifier_key), "Bare modifier press was intercepted");
      // Windows toggles on the bare release and still lets the application see it.
      require(!key(modifier_key, IBUS_RELEASE_MASK) && !seen.input_enabled,
              "Bare modifier release was consumed or no longer disabled input");
      require(!key(modifier_key), "Bare modifier restore press was intercepted");
      require(!key(modifier_key, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Bare modifier release was consumed or no longer restored input");
    }
    // Preferences can change while a modifier is held; release uses the
    // current binding without committing or clearing the active composition.
    for (guint modifier_key : {IBUS_Control_L, IBUS_Control_R,
                               IBUS_Shift_L, IBUS_Shift_R}) {
      phrase();
      require(!key(modifier_key), "Reconfigured modifier press was intercepted");
      auto disabled = options;
      // As above: the injected binding has to be the authority for this moment,
      // not a value the next reload tick can overwrite from the file.
      disabled.erase("preferences_directory");
      const bool ctrl = modifier_key == IBUS_Control_L || modifier_key == IBUS_Control_R;
      disabled["preferences"]["keybindings"][ctrl ? "switch_language_ctrl"
                                                   : "switch_language_shift"] = false;
      msime_ibus_configure(disabled.dump());
      invoke("FocusIn");
      require(!key(modifier_key, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Disabled modifier binding toggled input on release");
      require(seen.preedit_visible && seen.preedit == "nihao" && seen.committed.empty(),
              "Disabled modifier binding changed the active composition");
      msime_ibus_configure(options.dump());
      invoke("FocusIn");
      invoke("Reset");
    }
    for (guint modifier_key : {IBUS_Control_L, IBUS_Shift_L}) {
      require(!key(modifier_key), "Held modifier press was intercepted");
      g_usleep(350000);
      require(!key(modifier_key), "Repeated modifier press was intercepted");
      g_usleep(250000);
      require(!key(modifier_key, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Long modifier hold or repeat extended the mode-toggle deadline");
      require(!key(modifier_key), "Focus fixture modifier press was intercepted");
      invoke("FocusOut");
      invoke("FocusIn");
      require(!key(modifier_key, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Modifier release crossed a focus boundary");
    }
    require(seen.committed.empty(), "Mode setup unexpectedly committed text");
    auto wait_voice = [&](auto ready) {
      const auto deadline = g_get_monotonic_time() + 2000000;
      while (!ready() && g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
      return ready();
    };
    // The English-mode voice cases above already used the shared provider fixture, whose counters only grow.
    const auto base_voice_starts = voice_provider.started.load();
    const auto base_voice_cancels = voice_provider.cancelled.load();
    const auto base_voice_finals = voice_provider.finished.load();
    invoke("PropertyActivate", g_variant_new("(su)", "VoiceInput", PROP_STATE_CHECKED));
    require(wait_voice([&] { return voice_provider.started.load() == base_voice_starts + 1; }),
            "Synthetic voice capture did not start");
    require(seen.first_candidate_fix_name.empty() &&
                seen.first_candidate_clear_name.empty(),
            "Voice overlay retained stale candidate actions");
    voice_provider.release_partial = true;
    require(wait_voice([&] { return seen.preedit == "测试😀" && seen.preedit_visible; }),
            "Streaming voice did not publish synthetic preedit");
    require(seen.preedit_cursor == 3 && seen.committed.empty(),
            "Streaming voice cursor was not a Unicode character offset");
    invoke("PropertyActivate", g_variant_new("(su)", "CharacterMode", PROP_STATE_UNCHECKED));
    require(seen.preedit == "测试😀" && seen.preedit_cursor == 3 && seen.preedit_visible,
            "Voice preedit redraw used a UTF-8 byte offset");
    // Switching modes through the menu keeps the recording, like the mode shortcuts.
    mode(PROP_STATE_UNCHECKED);
    require(!seen.input_enabled && seen.preedit == "测试😀" && seen.preedit_visible,
            "Disabling input through the menu took down the voice recording");
    mode(PROP_STATE_CHECKED);
    require(seen.input_enabled && seen.preedit == "测试😀" && seen.preedit_visible,
            "Re-enabling input through the menu took down the voice recording");
    require(key(IBUS_Escape) && !seen.preedit_visible,
            "Voice cancellation left streaming preedit visible");
    require(wait_voice([&] { return voice_provider.cancelled.load() == base_voice_cancels + 1; }),
            "Esc did not cancel voice capture after the menu mode switches");
    voice_provider.release_final = true;
    require(wait_voice([&] { return voice_provider.finished.load() == base_voice_finals + 1; }),
            "Synthetic late voice result did not finish");
    const auto voice_settle = g_get_monotonic_time() + 100000;
    while (g_get_monotonic_time() < voice_settle) {
      while (g_main_context_iteration(nullptr, FALSE)) {}
      g_usleep(1000);
    }
    require(seen.committed.empty() && seen.input_enabled,
            "Cancelled voice result committed late");
    invoke("PropertyActivate", g_variant_new("(su)", "VoiceInput", PROP_STATE_CHECKED));
    require(wait_voice([&] { return voice_provider.started.load() == base_voice_starts + 2; }),
            "Voice capture could not restart after cancellation");
    voice_provider.release_final = true;
    const bool fresh_committed = wait_voice([&] { return seen.committed == "synthetic voice"; });
    require(fresh_committed,
            "Fresh voice result did not commit after cancellation");
    seen.committed.clear();
    // Recreating the Engine session on the same IBus object must invalidate
    // callbacks from the old session, even when the voice generation resets.
    const auto recreate_starts = voice_provider.started.load();
    const auto recreate_finals = voice_provider.finished.load();
    invoke("PropertyActivate", g_variant_new("(su)", "VoiceInput", PROP_STATE_CHECKED));
    require(wait_voice([&] { return voice_provider.started.load() == recreate_starts + 1; }),
            "Session-recreation voice fixture did not start");
    // Deliver the old result before rebuilding, but leave its idle callback
    // queued so the replacement session is active when it is dispatched.
    voice_provider.release_final = true;
    const auto old_result_deadline = g_get_monotonic_time() + 2000000;
    while (voice_provider.finished.load() < recreate_finals + 1 &&
           g_get_monotonic_time() < old_result_deadline)
      g_usleep(1000);
    require(voice_provider.finished.load() == recreate_finals + 1,
            "Session-recreation fixture did not produce the old result");
    g_usleep(50000);
    // The rebuild has to happen without running the main loop, so the queued
    // callback from the old session is still waiting when the replacement
    // opens. A content-type transition rebuilds the session in place; a menu
    // preference cannot be used here, because with a stored preferences
    // directory it only schedules a save and updates the session that exists.
    IBUS_ENGINE_GET_CLASS(engine)->set_content_type(
        engine, IBUS_INPUT_PURPOSE_FREE_FORM, IBUS_INPUT_HINT_PRIVATE);
    IBUS_ENGINE_GET_CLASS(engine)->property_activate(
        engine, "VoiceInput", PROP_STATE_CHECKED);
    require(wait_voice([&] { return voice_provider.started.load() == recreate_starts + 2; }),
            "Voice capture did not restart after Engine session recreation");
    const auto recreate_settle = g_get_monotonic_time() + 100000;
    while (g_get_monotonic_time() < recreate_settle) {
      while (g_main_context_iteration(nullptr, FALSE)) {}
      g_usleep(1000);
    }
    require(seen.committed.empty(),
            "Old-session voice result committed after Engine session recreation");
    voice_provider.release_final = true;
    require(wait_voice([&] { return seen.committed == "synthetic voice"; }),
            "Current-session voice result did not commit after recreation");
    seen.committed.clear();
    invoke("Set", g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                               g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM, 0)));
    const auto escape_starts = voice_provider.started.load();
    const auto escape_cancels = voice_provider.cancelled.load();
    const auto escape_finals = voice_provider.finished.load();
    invoke("PropertyActivate", g_variant_new("(su)", "VoiceInput", PROP_STATE_CHECKED));
    require(wait_voice([&] { return voice_provider.started.load() == escape_starts + 1; }),
            "Streaming Escape fixture did not start");
    voice_provider.release_partial = true;
    require(wait_voice([&] { return seen.preedit == "测试😀" && seen.preedit_visible; }),
            "Streaming Escape fixture did not show preedit");
    require(key(IBUS_Escape) && !seen.preedit_visible && seen.committed.empty(),
            "Escape did not clear voice preedit without committing");
    require(wait_voice([&] { return voice_provider.cancelled.load() == escape_cancels + 1; }),
            "Escape did not cancel streaming capture");
    voice_provider.release_partial = true;
    voice_provider.release_final = true;
    require(wait_voice([&] { return voice_provider.finished.load() == escape_finals + 1; }),
            "Cancelled streaming provider did not finish");

    for (guint released_modifiers : {guint(0), guint(IBUS_MOD1_MASK)}) {
      const auto starts = voice_provider.started.load();
      const auto stops = voice_provider.stop_requests.load();
      require(key(IBUS_Alt_R, IBUS_MOD1_MASK), "Right Alt did not start hold recording");
      require(wait_voice([&] { return voice_provider.started.load() == starts + 1; }),
              "Hold recording did not reach the provider");
      require(key(IBUS_Alt_R, IBUS_RELEASE_MASK | released_modifiers),
              "Hold recording release was not consumed");
      require(wait_voice([&] { return voice_provider.stop_requests.load() == stops + 1; }),
              "Hold recording release did not stop capture");
      voice_provider.release_final = true;
      require(wait_voice([&] { return seen.committed == "synthetic voice"; }),
              "Stopping hold recording discarded final recognition");
      seen.committed.clear();
    }
    const auto before_wrong_ctrl = voice_provider.started.load();
    require(!key(IBUS_Control_L, IBUS_CONTROL_MASK) &&
                !key(IBUS_Alt_R, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            "Left Ctrl incorrectly activated the right-Ctrl voice shortcut");
    key(IBUS_Alt_R, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK);
    key(IBUS_Control_L, IBUS_RELEASE_MASK);
    require(voice_provider.started.load() == before_wrong_ctrl && seen.input_enabled,
            "Left Ctrl voice chord changed capture or input mode");
    require(!key(IBUS_Control_R, IBUS_CONTROL_MASK), "Right Ctrl fixture press was intercepted");
    invoke("FocusOut");
    invoke("FocusIn");
    require(!key(IBUS_Alt_R, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            "Right Ctrl voice state crossed a focus boundary");
    key(IBUS_Alt_R, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK);
    key(IBUS_Control_R, IBUS_RELEASE_MASK);
    for (auto chord : {std::pair<guint, guint>{IBUS_Alt_R, IBUS_CONTROL_MASK | IBUS_MOD1_MASK},
                       {IBUS_Super_L, IBUS_CONTROL_MASK | IBUS_MOD4_MASK},
                       {IBUS_Super_R, IBUS_CONTROL_MASK | IBUS_MOD4_MASK}}) {
      const auto starts = voice_provider.started.load();
      const auto stops = voice_provider.stop_requests.load();
      require(!key(IBUS_Control_R, IBUS_CONTROL_MASK), "Voice chord Ctrl press was intercepted");
      require(key(chord.first, chord.second), "Modifier voice chord was filtered out");
      require(wait_voice([&] { return voice_provider.started.load() == starts + 1; }),
              "Modifier voice chord did not reach provider");
      require(key(chord.first, IBUS_RELEASE_MASK), "Modifier voice chord release was not consumed");
      // A bare Ctrl release is never consumed, even when it toggles, so the mode itself is what shows a toggle.
      const bool mode_before_release = seen.input_enabled;
      require(!key(IBUS_Control_R, IBUS_RELEASE_MASK) &&
                  seen.input_enabled == mode_before_release,
              "Voice chord Ctrl release toggled input");
      require(wait_voice([&] { return voice_provider.stop_requests.load() == stops + 1; }),
              "Modifier voice chord release did not stop capture");
      voice_provider.release_final = true;
      require(wait_voice([&] { return seen.committed == "synthetic voice"; }),
              "Modifier voice chord lost final recognition");
      seen.committed.clear();
    }
    require(!key(IBUS_Alt_R, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            "Released right Ctrl remained eligible for voice capture");
    key(IBUS_Alt_R, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK);
    for (auto chord : {std::pair<guint, guint>{IBUS_Alt_R, IBUS_MOD1_MASK},
                       {IBUS_Super_L, IBUS_MOD4_MASK}, {IBUS_Super_R, IBUS_MOD4_MASK}}) {
      const auto starts = voice_provider.started.load();
      const auto stops = voice_provider.stop_requests.load();
      const guint control = chord.first == IBUS_Alt_R ? IBUS_Control_R : IBUS_Control_L;
      require(!key(control, IBUS_CONTROL_MASK) && key(chord.first, IBUS_CONTROL_MASK | chord.second),
              "Ctrl-first release fixture did not start recording");
      require(wait_voice([&] { return voice_provider.started.load() == starts + 1; }),
              "Ctrl-first release fixture did not reach provider");
      require(!key(control, IBUS_RELEASE_MASK | chord.second),
              "Voice chord intercepted the Ctrl release delivered to the editor");
      require(wait_voice([&] { return voice_provider.stop_requests.load() == stops + 1; }),
              "Releasing Ctrl before the voice key did not stop recording");
      require(key(chord.first, IBUS_RELEASE_MASK), "Voice chord tail release was not consumed");
      voice_provider.release_final = true;
      require(wait_voice([&] { return seen.committed == "synthetic voice"; }),
              "Ctrl-first release discarded final recognition");
      require(voice_provider.stop_requests.load() == stops + 1,
              "Voice chord tail sent a duplicate stop request");
      seen.committed.clear();
    }
    for (guint held_control : {guint(0), guint(IBUS_CONTROL_MASK)}) {
      const auto locked_starts = voice_provider.started.load();
      const auto locked_stops = voice_provider.stop_requests.load();
      if (held_control) require(!key(IBUS_Control_R, IBUS_CONTROL_MASK), "Locked Ctrl press was intercepted");
      require(key(IBUS_Alt_R, IBUS_MOD1_MASK | held_control), "Locked recording did not start");
      require(wait_voice([&] { return voice_provider.started.load() == locked_starts + 1; }),
              "Locked recording did not reach provider");
      require(key(IBUS_space, IBUS_MOD1_MASK | held_control) &&
                  key(IBUS_space, IBUS_MOD1_MASK | held_control | IBUS_RELEASE_MASK),
              "Space did not lock recording while Alt was held");
      if (held_control) require(!key(IBUS_Control_R, IBUS_MOD1_MASK | IBUS_RELEASE_MASK), "Locked Ctrl release was intercepted");
      require(key(IBUS_Alt_R, IBUS_RELEASE_MASK), "Locked Alt release was not consumed");
      require(key(IBUS_F9, IBUS_CONTROL_MASK), "Ctrl+F9 did not stop locked recording");
      require(wait_voice([&] { return voice_provider.stop_requests.load() == locked_stops + 1; }),
              "Locked recording stopped on release or did not stop on Ctrl+F9");
      key(IBUS_F9, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK);
      voice_provider.release_final = true;
      require(wait_voice([&] { return seen.committed == "synthetic voice"; }),
              "Locked recording lost final recognition");
      require(voice_provider.stop_requests.load() == locked_stops + 1,
              "Locked recording sent duplicate stop requests");
      seen.committed.clear();
    }
    // Real IBus clients (X11, GDK, mutter) report the modifier state from before the key, so a hold key's own bit is missing on its press, and GTK3 adds the virtual SUPER bit next to MOD4. The shortcuts follow the physical key, as on Windows.
    auto physical_hold = [&](guint control, guint hold, guint state, const char *what) {
      const auto starts = voice_provider.started.load();
      const auto stops = voice_provider.stop_requests.load();
      if (control) require(!key(control, IBUS_CONTROL_MASK), what);
      require(key(hold, state), what);
      require(wait_voice([&] { return voice_provider.started.load() == starts + 1; }), what);
      require(key(hold, state | IBUS_RELEASE_MASK), what);
      require(wait_voice([&] { return voice_provider.stop_requests.load() == stops + 1; }), what);
      if (control) require(!key(control, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK), what);
      voice_provider.release_final = true;
      require(wait_voice([&] { return seen.committed == "synthetic voice"; }), what);
      seen.committed.clear();
    };
    physical_hold(0, IBUS_Alt_R, 0, "Right Alt without its own MOD1 bit did not hold-record");
    // A pending dead key must not swallow the hold key: xkb_compose ignores modifier keysyms, so a state-0 Alt_R fed to it would stay "composing".
    require(key(IBUS_dead_acute), "Dead key was not held by the system Compose table");
    key(IBUS_dead_acute, IBUS_RELEASE_MASK);
    physical_hold(0, IBUS_Alt_R, 0, "Right Alt did not hold-record with a dead key pending");
    physical_hold(IBUS_Control_R, IBUS_Alt_R, IBUS_CONTROL_MASK,
                  "RCtrl+RAlt without the MOD1 bit did not hold-record");
    for (guint super_key : {IBUS_Super_L, IBUS_Super_R})
      for (guint state : {guint(IBUS_CONTROL_MASK),
                          guint(IBUS_CONTROL_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK),
                          guint(IBUS_CONTROL_MASK | IBUS_SUPER_MASK)})
        physical_hold(IBUS_Control_L, super_key, state,
                      "Ctrl+Win did not hold-record for every Super encoding");
    {
      const auto starts = voice_provider.started.load();
      const bool mode_before = seen.input_enabled;
      require(!key(IBUS_Control_L, IBUS_CONTROL_MASK) && !key(IBUS_Alt_R, IBUS_CONTROL_MASK),
              "Left Ctrl with Right Alt matched the right-Ctrl voice shortcut");
      key(IBUS_Alt_R, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK | IBUS_MOD1_MASK);
      key(IBUS_Control_L, IBUS_RELEASE_MASK | IBUS_CONTROL_MASK);
      require(!key(IBUS_Alt_R, IBUS_SHIFT_MASK), "Shift+Right Alt matched a voice shortcut");
      key(IBUS_Alt_R, IBUS_RELEASE_MASK | IBUS_SHIFT_MASK | IBUS_MOD1_MASK);
      g_usleep(50000);
      while (g_main_context_iteration(nullptr, FALSE)) {}
      require(voice_provider.started.load() == starts && seen.input_enabled == mode_before,
              "Unmatched Right Alt chords changed capture or input mode");
    }
    {
      auto ralt_off = options;
      // Keep the store out of it, as with the mode chord fixture: the reload tick would otherwise restore the stored preference.
      ralt_off.erase("preferences_directory");
      ralt_off["preferences"]["voice_input"]["hotkey_ralt"] = false;
      msime_ibus_configure(ralt_off.dump());
      invoke("FocusIn");
      const auto starts = voice_provider.started.load();
      require(!key(IBUS_Alt_R) && !key(IBUS_Alt_R, IBUS_MOD1_MASK | IBUS_RELEASE_MASK),
              "Right Alt was consumed with its voice shortcut disabled");
      g_usleep(50000);
      while (g_main_context_iteration(nullptr, FALSE)) {}
      require(voice_provider.started.load() == starts,
              "Right Alt started voice with its shortcut disabled");
      msime_ibus_configure(options.dump());
      invoke("FocusIn");
    }



    for (guint modifier_key : {IBUS_Control_L, IBUS_Shift_L}) {
      phrase();
      require(!key(modifier_key), "Composing modifier press was intercepted");
      require(!key(modifier_key, IBUS_RELEASE_MASK) && !seen.input_enabled &&
                  seen.committed == "nihao" && !seen.preedit_visible && !seen.lookup_visible,
              "Bare modifier did not switch mode and commit original spelling");
      require(!key('a'), "Direct mode intercepted text after modifier toggle");
      require(!key(modifier_key, IBUS_RELEASE_MASK) && seen.committed == "nihao",
              "Repeated modifier release committed twice");
      require(!key(modifier_key) && !key(modifier_key, IBUS_RELEASE_MASK) && seen.input_enabled,
              "Modifier did not restore mode after composition");
      seen.committed.clear();
    }

    for (guint toggle_mask : {guint(IBUS_CONTROL_MASK),
                              guint(IBUS_CONTROL_MASK | IBUS_MOD1_MASK)}) {
      phrase();
      require(key(IBUS_space, toggle_mask) && !seen.input_enabled &&
                  seen.committed == "nihao" && !seen.preedit_visible && !seen.lookup_visible,
              "Space shortcut did not switch mode and commit original spelling");
      key(IBUS_space, toggle_mask | IBUS_RELEASE_MASK);
      require(key(IBUS_space, toggle_mask) && seen.input_enabled && seen.committed == "nihao",
              "Space shortcut restore committed twice");
      key(IBUS_space, toggle_mask | IBUS_RELEASE_MASK);
      seen.committed.clear();
    }
    phrase();
    require(seen.committed.empty(), "Phrase unexpectedly committed before selection");
    require(key(IBUS_period, IBUS_CONTROL_MASK),
            "Ctrl+. punctuation toggle was not consumed");
    require(!seen.punctuation_enabled,
            "Ctrl+. did not toggle punctuation state");
    require(key(IBUS_period, IBUS_CONTROL_MASK),
            "Ctrl+. punctuation restore was not consumed");
    require(seen.punctuation_enabled,
            "Ctrl+. did not restore punctuation state");
    require(seen.committed.empty(), "Punctuation toggle unexpectedly committed text");
    invoke("CursorDown");
    require(seen.committed.empty(), "CursorDown unexpectedly committed text");
    const std::string mode_commit = "nihao";
    mode(PROP_STATE_UNCHECKED);
    require(!seen.input_enabled, "Direct mode remained enabled");
    require(seen.committed == mode_commit, "Direct mode did not commit original spelling");
    require(!seen.preedit_visible, "Direct mode left preedit visible");
    require(!seen.lookup_visible, "Direct mode left candidates visible");
    mode(PROP_STATE_UNCHECKED);
    require(seen.committed == mode_commit,
            "Repeated mode request committed twice");
    for (guint direct :
         std::vector<guint>{'n', ',', '1', IBUS_space, IBUS_Tab, IBUS_Down})
      require(!key(direct), "Direct input mode consumed an editor key");
    invoke("CandidateClicked", g_variant_new("(uuu)", 0, 1, 0));
    invoke("PageDown");
    require(seen.committed == mode_commit && !seen.lookup_visible,
            "Stale panel action modified direct input");
    invoke("FocusOut");
    mode(PROP_STATE_CHECKED);
    require(!seen.input_enabled && !seen.mode_sensitive,
            "Unfocused mode activation was accepted");
    invoke("FocusIn");
    require(!seen.input_enabled && !key('n'), "Focus reset direct input mode");
    mode(PROP_STATE_INCONSISTENT);
    invoke("PropertyActivate",
           g_variant_new("(su)", "Unknown", PROP_STATE_CHECKED));
    require(!seen.input_enabled, "Invalid property activation changed mode");
    mode(PROP_STATE_CHECKED);
    require(seen.input_enabled, "Input mode did not recover");
    seen.committed.clear();
    // A visible incremental candidate list must not turn editing into cancel
    // or make Enter select a candidate instead of committing raw spelling.
    phrase();
    require(key(IBUS_BackSpace) && seen.preedit == "niha" &&
                seen.preedit_visible && seen.committed.empty(),
            "Backspace cancelled incremental composition instead of deleting one key");
    invoke("Reset");
    for (guint delete_key : {IBUS_Delete, IBUS_KP_Delete}) {
      phrase();
      require(key(IBUS_Left) && key(delete_key) && seen.preedit == "niha" &&
                  seen.preedit_visible && seen.committed.empty(),
              "Forward delete cancelled incremental composition instead of editing at caret");
      invoke("Reset");
    }
    for (guint enter_key : {IBUS_Return, IBUS_KP_Enter}) {
      phrase();
      const bool enter_handled = key(enter_key);
      settle_lookup();
      require(enter_handled && seen.committed == "nihao" &&
                  !seen.preedit_visible && !seen.lookup_visible,
              ("Enter selected an incremental candidate instead of raw spelling: handled=" +
               std::to_string(enter_handled) + " committed=[" + seen.committed +
               "] preedit=" + std::to_string(seen.preedit_visible) + " lookup=" +
               std::to_string(seen.lookup_visible))
                  .c_str());
      seen.committed.clear();
    }
    // Ctrl-only segment editing follows the Windows composition behavior:
    // arrows cross one pinyin unit and Backspace removes the unit to the left.
    phrase();
    require(key(IBUS_Left, IBUS_CONTROL_MASK) && seen.preedit_cursor == 2,
            "Ctrl+Left did not move to the preceding pinyin segment");
    require(key(IBUS_Right, IBUS_CONTROL_MASK) && seen.preedit_cursor == 5,
            "Ctrl+Right did not move to the following pinyin segment");
    require(key(IBUS_BackSpace, IBUS_CONTROL_MASK) && seen.preedit == "ni" &&
                seen.preedit_cursor == 2,
            "Ctrl+Backspace did not remove one pinyin segment");
    invoke("Reset");
    for (guint idle_key : {IBUS_BackSpace, IBUS_Delete, IBUS_KP_Delete,
                           IBUS_Return, IBUS_KP_Enter})
      require(!key(idle_key) && seen.committed.empty(),
              "Idle composition edit key was intercepted");
    seen.first_candidate_fix_name.clear();
    seen.first_candidate_clear_name.clear();
    phrase();
    require(seen.preedit_visible && seen.preedit == "nihao",
            "Preedit signal missing");
    require(seen.lookup_visible && seen.candidates.size() == 2 &&
                seen.candidates[0] == "你好",
            "Candidate signal mismatch");
    require(!seen.labels.empty() && seen.labels.front().rfind("1", 0) == 0,
            "Candidate numeric label missing");
    require(seen.second_candidate_color == 0x123456,
            "Candidate text color attribute missing");
    // A picked selection colour carries black or white text by its luminance (client-core skin/theme.rs), so the dark fixture fill gives white.
    require(seen.first_candidate_color == 0xffffff,
            "Highlighted candidate text is not readable on the picked selection colour");
    require(seen.first_candidate_background == 0x204060,
            "Selected candidate color attribute missing");
    require(seen.second_candidate_number_color == 0xabcdef,
            "Candidate number color attribute missing");
    // The host publishes the candidate menu on a 400ms timer after the page changes, so wait for it rather than reading it in the turn that drew the page.
    const auto candidate_actions_deadline = g_get_monotonic_time() + 2 * G_USEC_PER_SEC;
    while ((seen.first_candidate_fix_name.empty() || seen.first_candidate_clear_name.empty()) &&
           g_get_monotonic_time() < candidate_actions_deadline) {
      while (g_main_context_iteration(nullptr, FALSE)) {}
      g_usleep(1000);
    }
    require(!seen.first_candidate_fix_name.empty() &&
                !seen.first_candidate_clear_name.empty(),
            "Candidate position actions were not published");
    invoke("PropertyActivate",
           g_variant_new("(su)", seen.first_candidate_fix_name.c_str(),
                         PROP_STATE_UNCHECKED));
    require(seen.candidates.front().find("固定1") != std::string::npos,
            "Candidate position action did not fix the highlighted candidate");
    require(seen.first_candidate_color == 0xffffff,
            "Highlighted fixed candidate did not keep selected-row text color");
    invoke("PropertyActivate",
           g_variant_new("(su)", seen.first_candidate_clear_name.c_str(),
                         PROP_STATE_UNCHECKED));
    require(key(IBUS_Left) && seen.auxiliary.find("niha|o") != std::string::npos,
            "Candidate auxiliary text did not expose the preedit caret");
    const auto stale_candidate_action = seen.first_candidate_fix_name;
    invoke("Reset");
    const auto committed_before_stale_action = seen.committed;
    invoke("PropertyActivate",
           g_variant_new("(su)", stale_candidate_action.c_str(),
                         PROP_STATE_UNCHECKED));
    require(seen.committed == committed_before_stale_action,
            "Stale candidate action mutated a cleared snapshot");
    const auto stale_wheel_candidates = seen.candidates;
    invoke("CandidateClicked", g_variant_new("(uuu)", 0, 4, 0));
    require(seen.candidates == stale_wheel_candidates && !seen.lookup_visible,
            "Stale candidate wheel event mutated a cleared snapshot");
    phrase();
    require(!key(IBUS_Shift_L) && !key('n', IBUS_RELEASE_MASK),
            "Modifier/release was consumed");
    require(seen.preedit == "nihao", "Modifier/release canceled composition");
    require(key(IBUS_space), "Space not handled");
    require(!key(IBUS_Shift_L, IBUS_RELEASE_MASK) && seen.input_enabled,
            "Shift chord tail toggled input after Space");
    settle_lookup();
    require(seen.committed == "你好" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Commit/clear signal mismatch");
    require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK),
            "Pure Shift consumed a modifier event");
    require(!seen.input_enabled, "Pure Shift did not enter direct mode");
    require(!key(IBUS_Shift_L) && !key(IBUS_Shift_L, IBUS_RELEASE_MASK),
            "Pure Shift consumed a modifier event on restore");
    require(seen.input_enabled, "Pure Shift did not restore input mode");
    phrase();
    require(key(IBUS_End), "End did not move to the page edge");
    require(key(IBUS_Home), "Home did not move to the page edge");
    const auto property_first_page = seen.candidates;
    IBUS_ENGINE_GET_CLASS(engine)->property_activate(
        IBUS_ENGINE(engine), "CandidateNextPage", PROP_STATE_UNCHECKED);
    require(wait_until([&] { return seen.candidates != property_first_page; }) &&
                seen.lookup_visible && !seen.candidates.empty(),
            "Candidate panel next-page action did not use shared paging");
    IBUS_ENGINE_GET_CLASS(engine)->property_activate(
        IBUS_ENGINE(engine), "CandidatePreviousPage", PROP_STATE_UNCHECKED);
    require(wait_until([&] { return seen.candidates == property_first_page; }),
            "Candidate panel previous-page action did not restore the shared page");
    invoke("PageDown");
    require(seen.lookup_visible && !seen.candidates.empty(),
            "Shared next page missing");
    auto selected = seen.candidates.front();
    invoke("CandidateClicked", g_variant_new("(uuu)", 0, 1, 0));
    require(seen.committed == "你好" + selected,
            "Candidate click did not use shared global index");
    invoke("Reset");
    const auto committed_before_stale_click = seen.committed;
    invoke("CandidateClicked", g_variant_new("(uuu)", 0, 1, 0));
    settle_lookup();
    require(seen.committed == committed_before_stale_click &&
                !seen.lookup_visible,
            "Candidate click used a cleared rendered snapshot");
    require(key(','), "Punctuation not consumed");
    require(seen.committed == "你好" + selected + "，",
            "Chinese punctuation not applied");
    require(key(IBUS_quotedbl), "Paired quote was not consumed");
    require(seen.committed == "你好" + selected + "，“”",
            "Paired quote output mismatch");
    auto paired_book_titles = seen.committed;
    require(key('<') && key('<'), "Paired book title marks were not consumed");
    require(seen.committed == paired_book_titles + "《》《》",
            "Auto-closed book title marks left Engine nesting elevated");

    // A successful punctuation-space rewrite edits the preceding mark and
    // consumes Space. The mock editor publishes the post-commit surrounding
    // text explicitly, just as a real IBus client does before the next key.
    invoke("Reset");
    seen.committed.clear();
    auto publish_surrounding = [&](const char *value, guint cursor) {
      auto text = ibus_text_new_from_string(value);
      g_object_ref_sink(text);
      invoke("SetSurroundingText",
             g_variant_new("(vuu)",
                           ibus_serializable_serialize(IBUS_SERIALIZABLE(text)),
                           cursor, cursor));
      g_object_unref(text);
    };
    publish_surrounding("好", 1);
    require(key(';') && seen.committed == "；",
            "Space-convert fixture punctuation was not committed");
    publish_surrounding("好；", 2);
    const auto deletes_before_space = seen.delete_surrounding_calls;
    require(
        key(IBUS_space) && seen.committed == "；;" &&
            seen.delete_surrounding_calls == deletes_before_space + 1 &&
            seen.delete_surrounding_offset == -1 &&
            seen.delete_surrounding_count == 1,
        "Smart punctuation rewrite did not replace the mark and consume Space");

    // Auto-completed pairs are deliberately outside the gesture: rewriting
    // only the left half would orphan the closing half.
    publish_surrounding("好", 1);
    require(key(IBUS_quotedbl) && seen.committed == "；;“”",
            "Paired quote fixture was not committed");
    publish_surrounding("好“”", 2);
    const auto deletes_before_pair_space = seen.delete_surrounding_calls;
    require(!key(IBUS_space) &&
                seen.delete_surrounding_calls == deletes_before_pair_space,
            "Space conversion rewrote an auto-completed punctuation pair");

    // Mixed input keeps the ordinary Chinese session while inserting the
    // fixed-resource English/Emoji candidate into the same lookup table.
    invoke("Reset");
    seen.committed.clear();
    for (const char character : std::string("xiaolian"))
      require(key(static_cast<guint>(character)), "Mixed Emoji input was not consumed");
    const auto mixed_emoji = std::find(seen.candidates.begin(), seen.candidates.end(), "😀");
    require(mixed_emoji != seen.candidates.end(),
            "Mixed Emoji candidate was not exposed in the Chinese session");
    const auto mixed_emoji_index = static_cast<guint>(mixed_emoji - seen.candidates.begin());
    invoke("CandidateClicked", g_variant_new("(uuu)", mixed_emoji_index, 1, 0));
    settle_lookup();
    require(seen.committed == "😀" && !seen.preedit_visible && !seen.lookup_visible,
            "Mixed Emoji candidate was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    for (const char character : std::string("hello"))
      require(key(static_cast<guint>(character)), "Mixed English input was not consumed");
    const auto mixed_english = std::find(seen.candidates.begin(), seen.candidates.end(), "hello");
    require(mixed_english != seen.candidates.end(),
            "Mixed English candidate was not exposed at the configured prefix threshold");
    const auto mixed_english_index = static_cast<guint>(mixed_english - seen.candidates.begin());
    invoke("CandidateClicked", g_variant_new("(uuu)", mixed_english_index, 1, 0));
    settle_lookup();
    require(seen.committed == "hello" && !seen.preedit_visible && !seen.lookup_visible,
            "Mixed English candidate was not committed through IBus");

    invoke("Reset");
    require(key('u', IBUS_SHIFT_MASK), "Shift+U Unicode mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "U",
            "Shift+U did not enter Unicode mode");
    require(key(IBUS_Escape), "Unicode mode could not be canceled");
    require(key('t', IBUS_SHIFT_MASK), "Shift+T date-time mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "T",
            "Shift+T did not enter date-time mode");
    require(key(IBUS_Escape), "Date-time mode could not be canceled");
    require(key('k', IBUS_SHIFT_MASK), "Shift+K quick-phrase mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "K",
            "Shift+K did not enter quick-phrase mode");
    require(key(IBUS_Escape), "Quick-phrase mode could not be canceled");
    require(key('e', IBUS_SHIFT_MASK), "Shift+E emoji mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "E",
            "Shift+E did not enter emoji mode");
    require(key(IBUS_Escape), "Emoji mode could not be canceled");
    require(key('m', IBUS_SHIFT_MASK), "Shift+M kaomoji mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "M",
            "Shift+M did not enter kaomoji mode");
    require(key(IBUS_Escape), "Kaomoji mode could not be canceled");
    require(key('j', IBUS_SHIFT_MASK), "Shift+J super-jianpin mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "J",
            "Shift+J did not enter super-jianpin mode");
    require(key(IBUS_Escape), "Super-jianpin mode could not be canceled");
    require(key('y', IBUS_SHIFT_MASK), "Shift+Y temporary English mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "Y",
            "Shift+Y did not enter temporary English mode");
    require(key(IBUS_Escape), "Temporary English mode could not be canceled");
    require(key('r', IBUS_SHIFT_MASK), "Shift+R temporary Japanese mode was not consumed");
    require(seen.preedit_visible && seen.preedit == "R",
            "Shift+R did not enter temporary Japanese mode");
    require(key(IBUS_Escape), "Temporary Japanese mode could not be canceled");

    // Local-mode submissions must traverse the IBus bridge as commits, not
    // merely expose a display prefix. These inputs are fixed, public fixtures
    // and do not contain user-provided text.
    invoke("Reset");
    seen.committed.clear();
    require(key('u', IBUS_SHIFT_MASK), "Unicode mode could not restart");
    for (const char character : std::string("4e00"))
      require(key(static_cast<guint>(character)), "Unicode scalar input was not consumed");
    require(seen.preedit == "U4e00" && seen.candidates.size() == 1 &&
                seen.candidates.front() == "一",
            "Unicode mode did not expose the deterministic scalar candidate");
    const bool settled_commit_1 = key(IBUS_space);
    settle_lookup();
    require(settled_commit_1 && seen.committed == "一" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Unicode candidate was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    require(key('t', IBUS_SHIFT_MASK), "Date-time mode could not restart");
    for (const char character : std::string("rq"))
      require(key(static_cast<guint>(character)), "Date-time keyword input was not consumed");
    {
      // The lookup table carries one page, so the whole date list is only
      // observable by paging through it. Windows shows the same seventeen
      // formats behind its own pager.
      std::vector<std::string> paged;
      for (int page = 0; page < 24; ++page) {
        const auto before = seen.candidates;
        for (const auto &candidate : seen.candidates)
          if (std::find(paged.begin(), paged.end(), candidate) == paged.end())
            paged.push_back(candidate);
        if (!key(IBUS_Page_Down) || seen.candidates == before)
          break;
      }
      std::string observed;
      for (const auto &candidate : paged)
        observed += "[" + candidate + "]";
      require(paged.size() >= 13 && !paged.front().empty(),
              ("Date-time mode did not expose current date candidates: count=" +
               std::to_string(paged.size()) + " candidates=" + observed)
                  .c_str());
    }
    const bool settled_commit_2 = key(IBUS_space);
    settle_lookup();
    require(settled_commit_2 && !seen.committed.empty() && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Date-time candidate was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    require(key('k', IBUS_SHIFT_MASK), "Quick-phrase mode could not restart");
    for (const char character : std::string("yyds"))
      require(key(static_cast<guint>(character)), "Quick-phrase input was not consumed");
    require(std::any_of(seen.candidates.begin(), seen.candidates.end(),
                        [](const std::string &candidate) { return candidate == "永远滴神"; }),
            "Quick-phrase fixture candidate was not exposed");
    const bool settled_commit_3 = key(IBUS_space);
    settle_lookup();
    require(settled_commit_3 && seen.committed == "永远滴神" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Quick-phrase candidate was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    require(key('j', IBUS_SHIFT_MASK), "Super-jianpin mode could not restart");
    for (const char character : std::string("nh"))
      require(key(static_cast<guint>(character)), "Super-jianpin input was not consumed");
    // The engine ranks 女孩/你会 above 你好 for this code, so the fixture pages to
    // the candidate instead of assuming it leads the list.
    const int jianpin_index = page_to("你好");
    {
      std::string observed;
      for (const auto &candidate : seen.candidates)
        observed += "[" + candidate + "]";
      require(jianpin_index >= 0,
              ("Super-jianpin fixture candidate was not exposed: preedit=[" + seen.preedit +
               "] last page=" + observed)
                  .c_str());
    }
    invoke("CandidateClicked",
           g_variant_new("(uuu)", static_cast<guint>(jianpin_index), 1, 0));
    settle_lookup();
    require(seen.committed == "你好" && !seen.preedit_visible && !seen.lookup_visible,
            ("Super-jianpin candidate was not committed through IBus: committed=[" +
             seen.committed + "]")
                .c_str());

    invoke("Reset");
    seen.committed.clear();
    require(key('y', IBUS_SHIFT_MASK), "Temporary English mode could not restart");
    for (const char character : std::string("MSIME"))
      require(key(static_cast<guint>(character)), "Temporary English input was not consumed");
    require(seen.preedit == "YMSIME" && seen.candidates.size() >= 1 &&
                seen.candidates.front() == "MSIME",
            "Temporary English mode did not expose its raw candidate");
    const bool settled_commit_5 = key(IBUS_Return);
    settle_lookup();
    require(settled_commit_5 && seen.committed == "MSIME" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Temporary English raw text was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    require(key('e', IBUS_SHIFT_MASK), "Emoji mode could not restart");
    for (const char character : std::string("xiaolian"))
      require(key(static_cast<guint>(character)), "Emoji keyword input was not consumed");
    require(!seen.candidates.empty() && seen.candidates.front() == "😀",
            "Emoji mode did not expose the locked-resource fixture candidate");
    const bool settled_commit_6 = key(IBUS_space);
    settle_lookup();
    require(settled_commit_6 && seen.committed == "😀" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Emoji candidate was not committed through IBus");

    invoke("Reset");
    seen.committed.clear();
    require(key('m', IBUS_SHIFT_MASK), "Kaomoji mode could not restart");
    for (const char character : std::string("kiss"))
      require(key(static_cast<guint>(character)), "Kaomoji keyword input was not consumed");
    require(!seen.candidates.empty() && seen.candidates.front() == "!(*￣(￣　*)",
            "Kaomoji mode did not expose the locked-resource fixture candidate");
    const bool settled_commit_7 = key(IBUS_space);
    settle_lookup();
    require(settled_commit_7 && seen.committed == "!(*￣(￣　*)" && !seen.preedit_visible &&
                !seen.lookup_visible,
            "Kaomoji candidate was not committed through IBus");

    // R mode owns the visible prefix but never forwards it to the Japanese
    // Engine. A candidate commit must therefore restore the Chinese session,
    // and the next letter must start a normal Chinese composition again.
    invoke("Reset");
    seen.committed.clear();
    require(key('r', IBUS_SHIFT_MASK), "Temporary Japanese mode could not restart");
    require(key('k') && key('a'), "Temporary Japanese Romaji input was not consumed");
    require(std::any_of(seen.candidates.begin(), seen.candidates.end(),
                        [](const std::string &candidate) { return candidate.find("か") != std::string::npos; }),
            "Temporary Japanese Romaji candidates were not exposed");
    require(key(IBUS_space), "Temporary Japanese candidate was not committed");
    require(seen.committed.find("か") != std::string::npos,
            "Temporary Japanese candidate commit did not reach IBus");
    require(key('n') && seen.preedit == "n",
            "Temporary Japanese candidate commit did not restore Chinese input");

    // Enter commits the raw Romaji spelling, without the display-only R
    // prefix, and also returns to the original Chinese session.
    invoke("Reset");
    seen.committed.clear();
    require(key('r', IBUS_SHIFT_MASK) && key('k') && key('a') && key(IBUS_Return),
            "Temporary Japanese raw Enter path was not consumed");
    require(seen.committed == "ka",
            "Temporary Japanese raw Enter committed the display-only prefix");
    require(key('n') && seen.preedit == "n",
            "Temporary Japanese raw Enter did not restore Chinese input");

    // Backspace on the bare display prefix cancels R mode without emitting R.
    invoke("Reset");
    seen.committed.clear();
    require(key('r', IBUS_SHIFT_MASK) && seen.preedit == "R" && key(IBUS_BackSpace),
            "Backspace did not cancel a bare temporary Japanese prefix");
    require(seen.committed.empty() && !seen.preedit_visible,
            "Bare temporary Japanese prefix escaped after Backspace");

    // The Linux local-mode preference is the shared equivalent of Windows'
    // r_mode switch. Disabled modes must leave Shift+R to the application.
    invoke("PropertyActivate",
           g_variant_new("(su)", "LocalModes/temporary_japanese", PROP_STATE_UNCHECKED));
    seen.committed.clear();
    // With a stored preferences directory the menu toggle is a save, not an
    // immediate switch: it is written, read back and only then applied to the
    // session. Asserting the key in the same turn tests the old preference.
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return !preferences.at("local_modes")
                          .at("temporary_japanese").get<bool>();
            }),
            "Temporary Japanese disable was not persisted");
    bool japanese_released = false;
    for (int attempt = 0; attempt < 200 && !japanese_released; ++attempt) {
      japanese_released = !key('r', IBUS_SHIFT_MASK) && !seen.preedit_visible &&
                          seen.committed.empty();
      if (!japanese_released) {
        invoke("Reset");
        seen.committed.clear();
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(10000);
      }
    }
    require(japanese_released, "Disabled temporary Japanese mode swallowed Shift+R");
    invoke("PropertyActivate",
           g_variant_new("(su)", "LocalModes/temporary_japanese", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.at("local_modes")
                  .at("temporary_japanese").get<bool>();
            }),
            "Temporary Japanese restore was not persisted");
    invoke("Reset");
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Japanese", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "quanpin") == "japanese";
            }),
            "Japanese scheme was not persisted");
    require(key('a'), "Japanese scheme did not consume Romaji input");
    require(seen.lookup_visible && !seen.candidates.empty() &&
                seen.candidates.front().find("あ") != std::string::npos,
            "Japanese scheme menu did not switch the Engine");
    const auto japanese_commit = seen.committed;
    // The composition shows the kana the letters make, so the long-vowel mark joins it as ー.
    require(key(IBUS_minus) && seen.preedit == "あー" && seen.lookup_visible &&
                seen.committed == japanese_commit,
            "Japanese minus was intercepted by candidate paging");
    invoke("Reset");
    require(
        key(IBUS_minus) && seen.preedit == "ー" && seen.candidates.size() == 2 &&
            seen.candidates[0] == "ー" && seen.candidates[1] == "-",
        "Bare Japanese minus did not offer long-vowel and hyphen candidates");
    const bool settled_commit_8 = key(IBUS_equal);
    settle_lookup();
    require(settled_commit_8 && seen.committed == japanese_commit + "ー=" &&
                !seen.preedit_visible && !seen.lookup_visible,
            "Japanese equal key paged candidates instead of committing "
            "punctuation");
    invoke("Reset");
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Chinese", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "japanese") == "quanpin";
            }),
            "Chinese scheme was not restored");
    // Korean composes Dubeolsik jamo into a Hangul syllable drawn inline in COMMIT mode, so IBus hands it to the client being left on a focus change. Starting a new syllable commits the previous one, the keys that end a syllable commit it and still reach the application, and punctuation stays ASCII.
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Korean", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "quanpin") == "korean" &&
                     preferences.value("last_chinese_scheme", "quanpin") == "quanpin";
            }),
            "Korean scheme was not persisted, or it replaced the last Chinese scheme");
    {
      const auto before = seen.committed;
      require(key('d') && key('k') && key('s') && seen.preedit == "안" &&
                  seen.preedit_visible && seen.preedit_cursor == 1 &&
                  seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT &&
                  !seen.lookup_visible && seen.committed == before,
              "Korean syllable was not drawn inline in commit mode");
      require(key('s') && seen.committed == before + "안" && seen.preedit == "ㄴ",
              "A new Korean syllable did not commit the previous one");
      require(key('u') && key('d') && seen.preedit == "녕",
              "Korean vowel did not join the open syllable");
      require(key(IBUS_BackSpace) && seen.preedit == "녀",
              "Backspace did not remove one jamo");
      require(!key(IBUS_space) && seen.committed == before + "안녀" &&
                  !seen.preedit_visible,
              "Space did not commit the Korean syllable and reach the application");
      require(key('R', IBUS_SHIFT_MASK) && seen.preedit == "ㄲ",
              "Shift+R did not type ㄲ");
      require(key(IBUS_Escape) && !seen.preedit_visible &&
                  seen.committed == before + "안녀",
              "Escape did not discard the Korean syllable");
      require(key('R', IBUS_LOCK_MASK) && seen.preedit == "ㄱ",
              "CapsLock shifted a Korean jamo");
      require(key('k') && key('.') && seen.committed == before + "안녀가." &&
                  !seen.preedit_visible,
              "A mark did not follow the Korean syllable in one commit");
      require(!key('.') && !key('.') && seen.committed == before + "안녀가.",
              "An idle Korean mark was not left to the application as ASCII");
      require(key('r') && key('k') && !key('1') &&
                  seen.committed == before + "안녀가.가" && !seen.preedit_visible,
              "A digit did not end the Korean syllable");
      // Hangul_Hanja or a bare F9 converts the composing syllable to Hanja (msime_client.h, MSIME_CONVERT_HANJA). With the list open the candidate keys choose, Escape only closes it, a paging mark writes the Hangul with it, and a trigger is never passed on while a syllable composes.
      auto hanja_commit = seen.committed;
      require(!key(IBUS_F9) && !key(IBUS_Hangul_Hanja) && seen.committed == hanja_commit,
              "A Hanja key with nothing composing was not left to the application");
      require(key('g') && key('k') && key('s') && key(IBUS_Hangul_Hanja) && seen.lookup_visible &&
                  !seen.candidates.empty() &&
                  seen.candidates.front() == "韓 · 나라 이름 한, 한나라 한" &&
                  seen.preedit == "한" && seen.committed == hanja_commit,
              "Hangul_Hanja did not open the Hanja list with its 훈음 as the row's gloss");
      require(key(IBUS_F9) && seen.preedit == "한" && seen.committed == hanja_commit,
              "The trigger did not keep the syllable when closing the list");
      settle_lookup();
      require(!seen.lookup_visible, "The trigger did not close the Hanja list");
      require(key(IBUS_F9) && seen.lookup_visible && key(IBUS_Down) && key(IBUS_Return) &&
                  seen.committed == hanja_commit + "漢" && !seen.preedit_visible,
              "Return did not choose the highlighted Hanja");
      hanja_commit = seen.committed;
      require(key('g') && key('k') && key('s') && key(IBUS_F9) && key(IBUS_space) &&
                  seen.committed == hanja_commit + "韓",
              "Space did not choose the highlighted Hanja");
      hanja_commit = seen.committed;
      require(key('g') && key('k') && key('s') && key(IBUS_F9) && key(IBUS_2) &&
                  seen.committed == hanja_commit + "漢",
              "A digit did not choose from the Hanja page");
      hanja_commit = seen.committed;
      require(key('g') && key('k') && key('s') && key(IBUS_F9) && key(IBUS_Escape) &&
                  seen.preedit == "한" && seen.committed == hanja_commit,
              "Escape did not close the Hanja list and keep the syllable");
      require(key(IBUS_F9) && key(IBUS_period) && seen.committed == hanja_commit + "한." &&
                  !seen.preedit_visible,
              "A paging mark turned a page instead of writing the Hangul with it");
      hanja_commit = seen.committed;
      require(key('r') && key(IBUS_F9) && seen.preedit == "ㄱ" && seen.committed == hanja_commit,
              "The trigger of a lone jamo was passed on");
      require(key(IBUS_Escape) && !seen.preedit_visible, "Escape did not discard the lone jamo");
      // With number-row selection off a digit is the application's, and it must not land in the document before a syllable and list left hanging: the Hangul is written first.
      invoke("PropertyActivate",
             g_variant_new("(su)", "NumberRowSelection", PROP_STATE_UNCHECKED));
      require(wait_saved_preferences([](const nlohmann::json &preferences) {
                return !preferences.value("number_row_selection", true);
              }),
              "Korean number-row selection was not turned off");
      require(key('g') && key('k') && key('s') && key(IBUS_F9) && seen.lookup_visible &&
                  !key(IBUS_1) && seen.committed == hanja_commit + "한" && !seen.preedit_visible,
              "With number-row selection off a digit did not write the Hangul before reaching the application");
      invoke("PropertyActivate",
             g_variant_new("(su)", "NumberRowSelection", PROP_STATE_CHECKED));
      require(wait_saved_preferences([](const nlohmann::json &preferences) {
                return preferences.value("number_row_selection", false);
              }),
              "Korean number-row selection was not restored");
    }
    invoke("Reset");
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Chinese", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "korean") == "quanpin";
            }),
            "Chinese scheme was not restored from Korean");
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Wubi", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "quanpin") == "wubi";
            }),
            "Wubi scheme was not persisted");
    require(key('a'), "Explicit Wubi scheme did not switch the Engine");
    invoke("Reset");
    invoke("PropertyActivate",
           g_variant_new("(su)", "Scheme/Quanpin", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("scheme", "wubi") == "quanpin";
            }),
            "Quanpin scheme was not restored");
    auto committed = seen.committed;
    phrase();
    int pending_punctuation_lock =
        open((root / "preferences.lock").c_str(), O_CREAT | O_RDWR, 0600);
    require(pending_punctuation_lock >= 0 &&
                flock(pending_punctuation_lock, LOCK_EX | LOCK_NB) == 0,
            "Cannot lock pending punctuation preference");
    invoke("PropertyActivate",
           g_variant_new("(su)", "ChinesePunctuation", PROP_STATE_UNCHECKED));
    require(seen.punctuation_enabled && seen.preedit_visible &&
                seen.preedit == "nihao" && seen.lookup_visible &&
                seen.committed == committed,
            "Pending punctuation save changed the active session");
    flock(pending_punctuation_lock, LOCK_UN);
    close(pending_punctuation_lock);
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return !preferences.value("chinese_punctuation", true);
            }) && !seen.punctuation_enabled,
            "English punctuation mode was not persisted");
    require(seen.preedit_visible && seen.preedit == "nihao" &&
                seen.lookup_visible && seen.committed == committed,
            "Punctuation toggle lost composition or committed input");
    invoke("Reset");
    {
      const bool comma_handled = key(',');
      require(!comma_handled && seen.committed == committed,
              ("English punctuation should pass through when idle: handled=" +
               std::to_string(comma_handled) + " committed=[" + seen.committed +
               "] before=[" + committed + "] preedit=[" + seen.preedit + "]")
                  .c_str());
    }
    invoke("PropertyActivate",
           g_variant_new("(su)", "ChinesePunctuation", PROP_STATE_CHECKED));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("chinese_punctuation", false);
            }),
            "Chinese punctuation mode was not restored");
    require(key(','), "Restored Chinese punctuation was not consumed");
    require(seen.committed == committed + "，",
            "Restored punctuation mode did not reach the session");
    // Numpad arithmetic keys are not the configurable minus/equal paging
    // bindings. When a candidate is highlighted they must finish that
    // candidate and append their literal ASCII mark, matching Windows' VK
    // arithmetic punctuation path (including keypad decimal).
    for (const auto &[keypad, mark] : std::vector<std::pair<guint, char>>{
             {IBUS_KP_Decimal, '.'},
             {IBUS_KP_Subtract, '-'},
             {IBUS_KP_Add, '+'},
             {IBUS_KP_Divide, '/'},
             {IBUS_KP_Multiply, '*'}}) {
      invoke("Reset");
      phrase();
      const auto highlighted = seen.candidates.front();
      const auto prefix = seen.committed;
      require(key(keypad), "Keypad punctuation was not consumed");
      require(seen.committed == prefix + highlighted + mark,
              "Keypad punctuation did not finish the highlighted candidate literally");
      settle_lookup();
      require(!seen.preedit_visible && !seen.lookup_visible,
              "Keypad punctuation left the candidate view visible");
    }
    committed = seen.committed;
    phrase();
    require(key(IBUS_KP_Page_Down), "Keypad paging not consumed");
    auto numbered = seen.candidates.front();
    require(key(IBUS_KP_1) && seen.committed == committed + numbered,
            "Keypad digit did not select the shared page");
    invoke("Reset");
    committed = seen.committed;
    phrase();
    require(!key('c', IBUS_CONTROL_MASK) && !seen.preedit_visible,
            "Shortcut was intercepted or left stale composition");
    require(seen.committed == committed,
            "Shortcut unexpectedly committed input");
    require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Control-space toggle was not handled");
    require(!key('n'), "Disabled input consumed a character");
    invoke("PropertyActivate",
           g_variant_new("(su)", "InputEnabled", PROP_STATE_CHECKED));
    require(key('n'), "InputEnabled property did not re-enable input");
    invoke("Reset");
    require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Control-space disable was not handled");
    require(!key('n'), "Disabled input consumed a character after property toggle");
    require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Control-space re-enable was not handled");
    require(key('n'), "Re-enabled input did not consume a character");
    invoke("Reset");
    invoke("Reset");
    invoke("FocusOut");
    settle_lookup();
    require(!seen.preedit_visible && !seen.lookup_visible && !key('n'),
            "Focus loss did not clear and stop input");
    invoke("Set", g_variant_new(
                      "(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                      g_variant_new("(uu)", IBUS_INPUT_PURPOSE_PASSWORD, 0)));
    invoke("FocusIn");
    require(!seen.mode_sensitive, "Password field exposed a mode switch");
    mode(PROP_STATE_UNCHECKED);
    require(seen.input_enabled, "Password field accepted a mode change");
    require(!key('n') && !seen.preedit_visible && seen.committed == committed,
            "Password input reached engine");
    invoke("Set",
           g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                         g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM,
                                       IBUS_INPUT_HINT_PRIVATE)));
    phrase();
    require(key(IBUS_space) && seen.committed == committed + "你好",
            "Private text focus did not recover");
    invoke("FocusOut");
    invoke("FocusIn");
    int pending_width_lock =
        open((root / "preferences.lock").c_str(), O_CREAT | O_RDWR, 0600);
    require(pending_width_lock >= 0 &&
                flock(pending_width_lock, LOCK_EX | LOCK_NB) == 0,
            "Cannot lock pending character-width preference");
    invoke("PropertyActivate", g_variant_new("(su)", "CharacterWidth", 1));
    require(!key('1') && seen.committed == committed + "你好",
            "Pending fullwidth save changed idle character handling");
    flock(pending_width_lock, LOCK_UN);
    close(pending_width_lock);
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("character_width", "halfwidth") ==
                     "fullwidth";
            }),
            "Fullwidth idle mode was not persisted");
    require(key('1'), "Fullwidth idle digit was not handled");
    require(seen.committed == committed + "你好１", "Fullwidth ASCII commit mismatch");
    invoke("PropertyActivate", g_variant_new("(su)", "CharacterWidth", 0));
    require(wait_saved_preferences([](const nlohmann::json &preferences) {
              return preferences.value("character_width", "fullwidth") ==
                     "halfwidth";
            }),
            "Halfwidth idle mode was not restored");
    require(!key('2'), "Halfwidth idle digit was intercepted");
    require(seen.committed == committed + "你好１", "Halfwidth ASCII commit mismatch");
    auto settle = [&] {
      const auto deadline = g_get_monotonic_time() + 2200000;
      while (g_get_monotonic_time() < deadline) {
        while (g_main_context_iteration(nullptr, FALSE)) {}
        g_usleep(1000);
      }
    };
    require(!seen.traditional_output, "Traditional output did not start disabled");
    // Holding Ctrl+Shift+F auto-repeats the press. A repeat while the first toggle's save is pending belongs to the shortcut instead of reaching the editor, and one after the save landed does not flip the setting back.
    require(key(IBUS_f, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
            "Ctrl+Shift+F was not consumed");
    require(key(IBUS_f, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
            "Ctrl+Shift+F repeat during the pending save reached the editor");
    settle();
    require(seen.traditional_output, "First Ctrl+Shift+F did not apply traditional output");
    require(key(IBUS_f, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK),
            "Ctrl+Shift+F repeat after the save was not consumed");
    settle();
    require(seen.traditional_output, "Held Ctrl+Shift+F repeat toggled the character set back");
    require(key(IBUS_f, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK),
            "Ctrl+Shift+F release was not consumed");
    settle();
    std::ifstream saved_preferences(root / "preferences.json");
    nlohmann::json saved_snapshot;
    saved_preferences >> saved_snapshot;
    require(saved_snapshot.at("preferences").at("traditional_chinese_output").get<bool>(),
            "Ctrl+Shift+F did not persist traditional output");
    require(seen.traditional_output,
            "Persisted traditional output was not applied to the active session");
    invoke("PropertyActivate",
           g_variant_new("(su)", "TraditionalOutput", PROP_STATE_UNCHECKED));
    settle();
    require(!seen.traditional_output,
            "Traditional output preference did not restore after the shortcut test");
    std::ifstream current_preferences(root / "preferences.json");
    nlohmann::json current_snapshot;
    current_preferences >> current_snapshot;
    const auto first_external_revision =
        current_snapshot.at("revision").get<uint64_t>() + 1;
    auto save = [&](uint64_t revision, size_t page_size) {
      auto preferences = options.at("preferences");
      preferences["candidate_page_size"] = page_size;
      preferences["learning"] = true;
      preferences["frequency"]["mode"] = "pin";
      preferences["frequency"]["trigger_count"] = 1;
      preferences["mixed_input"]["emoji"] = false;
      preferences["custom_theme"]["candidate_colors"]["text"] = "#abcdef";
      auto snapshot = nlohmann::json{{"format_version", 1},
                                     {"revision", revision},
                                     {"preferences", preferences}};
      std::ofstream(root / "preferences.next") << snapshot.dump();
      std::filesystem::rename(root / "preferences.next", root / "preferences.json");
    };
    phrase();
    save(first_external_revision, 3);
    settle();
    require(seen.preedit == "nihao" && seen.candidates.size() == 2,
            "Preferences interrupted the active composition");
    invoke("Reset");
    phrase();
    require(seen.candidates.size() == 3,
            "Deferred preferences did not apply after reset");
    require(seen.second_candidate_color == 0xabcdef,
            "Reloaded candidate text color did not apply");
    std::ofstream(root / "preferences.json") << "invalid";
    settle();
    require(seen.preedit == "nihao" && seen.candidates.size() == 3,
            "Malformed preferences disturbed composition");
    invoke("Reset");
    save(first_external_revision - 1, 4);
    settle();
    phrase();
    require(seen.candidates.size() == 3,
            "Stale revision replaced live settings");
    invoke("Reset");
    save(first_external_revision, 4);
    settle();
    phrase();
    require(seen.candidates.size() == 3,
            "Conflicting revision replaced live settings");
    invoke("Reset");
    invoke("FocusOut");
    invoke("FocusIn");
    settle();
    phrase();
    require(seen.candidates.size() == 3,
            "Conflicting revision replaced settings after focus recovery");
    invoke("Reset");
    int lock =
        open((root / "preferences.lock").c_str(), O_CREAT | O_RDWR, 0600);
    require(lock >= 0 && flock(lock, LOCK_EX | LOCK_NB) == 0,
            "Cannot lock synthetic preferences");
    save(first_external_revision + 1, 4);
    settle();
    phrase();
    require(seen.candidates.size() == 3, "Reader ignored the writer lock");
    invoke("Reset");
    flock(lock, LOCK_UN);
    close(lock);
    settle();
    phrase();
    require(seen.candidates.size() == 4,
            "Settings did not recover after writer unlock");
    // Only a dictionary row has a weight for the configured frequency mode to move. The lattice puts its generated sentences for nihao (倪好, 你号, ...) straight after the exact dictionary hits at the top, so the first two-character rows after 你好 are usually generated. Selecting one of those stores it as a user phrase instead (the Engine's standalone sentence learning, ported from MSIME-Windows 01c5bca3), which ignores the frequency mode and gives the row a fixed starting weight; it is not expected to come first. Learn a two-character dictionary row - one that shares nihao's two segments - wherever it is paged to. Returns its page position, or -1 if none shows up.
    auto dictionary_two_segment_index = [&] {
      for (int page = 0; page < 24; ++page) {
        for (const auto slot : seen.dictionary_slots)
          if ((page > 0 || slot > 0) && slot < seen.candidates.size() &&
              g_utf8_strlen(seen.candidates[slot].c_str(), -1) == 2)
            return static_cast<int>(slot);
        const auto before = seen.candidates;
        if (!key(IBUS_Page_Down) || seen.candidates == before)
          break;
      }
      return -1;
    };
    auto private_candidates = seen.candidates;
    const int private_learn = dictionary_two_segment_index();
    require(private_learn >= 0,
            "Private session exposed no two-segment dictionary candidate to learn");
    invoke("CandidateClicked",
           g_variant_new("(uuu)", static_cast<guint>(private_learn), 1, 0));
    phrase();
    require(seen.candidates == private_candidates,
            "Reload enabled frequency learning in a private session");
    invoke("Reset");
    invoke("Set",
           g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                         g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM, 0)));
    settle();
    phrase();
    const int learn_index = dictionary_two_segment_index();
    require(learn_index >= 0,
            "Normal session exposed no two-segment dictionary candidate to learn");
    auto learned = seen.candidates.at(static_cast<std::size_t>(learn_index));
    const auto before_learning = seen.candidates;
    invoke("CandidateClicked",
           g_variant_new("(uuu)", static_cast<guint>(learn_index), 1, 0));
    const auto committed_learning = seen.committed;
    phrase();
    {
      std::string observed;
      for (const auto &candidate : seen.candidates)
        observed += "[" + candidate + "]";
      std::string before;
      for (const auto &candidate : before_learning)
        before += "[" + candidate + "]";
      std::string journal = "missing";
      {
        std::error_code error;
        for (const auto &entry :
             std::filesystem::recursive_directory_iterator(root, error)) {
          if (entry.path().filename() == "msime_user.db")
            journal = entry.path().string() + " size=" +
                      std::to_string(std::filesystem::file_size(entry.path(), error));
        }
      }
      require(seen.candidates.front() == learned,
              ("Normal session did not restore configured frequency learning: learned=[" +
               learned + "] committed=[" + committed_learning + "] before=" + before +
               " after=" + observed + " journal=" + journal +
               " learning=" + std::to_string(seen.learning_enabled) +
               " learning_sensitive=" + std::to_string(seen.learning_sensitive))
                  .c_str());
    }
    invoke("Reset");
    // Right-click is the Windows context menu, which changes nothing until an action is picked. IBus has no per-candidate menu, so the click leaves the user dictionary alone and points at the 候选操作 menu instead of pinning. The old pin raised the row's weight, so a dictionary row paged past the top would have come first on the retyped page.
    {
      phrase();
      const auto baseline = seen.candidates;
      const int target = dictionary_two_segment_index();
      require(target >= 0, "Fixture exposed no dictionary candidate past the top to right-click");
      const auto page_before = seen.candidates;
      const auto aux_before = seen.auxiliary;
      const auto committed_before = seen.committed;
      invoke("CandidateClicked", g_variant_new("(uuu)", static_cast<guint>(target), 3, 0));
      require(seen.candidates == page_before && seen.committed == committed_before &&
                  seen.preedit == "nihao" && seen.lookup_visible &&
                  seen.auxiliary.find("候选操作") != std::string::npos,
              ("Right-click changed the composition or showed no menu hint: aux=[" +
               seen.auxiliary + "]")
                  .c_str());
      require(wait_until([&] { return seen.auxiliary == aux_before; }),
              ("Right-click hint did not give the page number back: aux=[" + seen.auxiliary +
               "] expected=[" + aux_before + "]")
                  .c_str());
      invoke("Reset");
      phrase();
      require(seen.candidates == baseline, "Right-click changed the user dictionary order");
      invoke("Reset");
    }
    struct Binding {
      const char *name;
      guint next;
      guint previous;
      bool candidate;
    };
    const std::vector<Binding> bindings = {
        {"minus_equal", IBUS_equal, IBUS_minus, false},
        {"comma_period", IBUS_period, IBUS_comma, false},
        {"brackets", IBUS_bracketright, IBUS_bracketleft, false},
        {"tab", IBUS_Tab, IBUS_ISO_Left_Tab, false},
        {"page_up_down", IBUS_KP_Page_Down, IBUS_KP_Page_Up, false},
        {"arrows", IBUS_KP_Down, IBUS_KP_Up, true}};
    // Paging bindings are mutually exclusive with word-to-character. Disable
    // the latter explicitly now that it follows the enabled Windows default.
    options["preferences"]["word_character"]["enabled"] = false;
    uint64_t revision = first_external_revision + 2;
    for (const auto &binding : bindings) {
      for (const auto &item : bindings)
        options["preferences"]["navigation"][item.name] = false;
      options["preferences"]["navigation"][binding.name] = true;
      phrase();
      auto first_page = seen.candidates;
      auto before_commit = seen.committed;
      save(revision++, 4);
      settle();
      require(seen.preedit == "nihao", "Binding update canceled composition");
      require(key(binding.next), "Configured forward binding was not handled");
      require(binding.candidate ? seen.cursor == 1
                                : seen.candidates != first_page,
              "Configured forward binding did not move candidates");
      require(key(binding.previous,
                  binding.previous == IBUS_ISO_Left_Tab ? IBUS_SHIFT_MASK : 0),
              "Configured backward binding was not handled");
      require(
          seen.candidates == first_page && seen.cursor == 0 &&
              seen.committed == before_commit,
          "Navigation changed input or failed to return to first candidate");
      // A Ctrl chord on an enabled navigation key is the application's shortcut (Ctrl+Tab switches tabs, Ctrl+PageDown switches documents), not paging. Like Ctrl+a and the Fcitx5 host it cancels the composition and goes to the application, so no preedit is left behind in the editor.
      const std::string binding_name = binding.name;
      if (binding_name == "tab" || binding_name == "page_up_down") {
        const std::vector<std::pair<guint, guint>> chords =
            binding_name == "tab"
                ? std::vector<std::pair<guint, guint>>{
                      {IBUS_Tab, IBUS_CONTROL_MASK},
                      {IBUS_ISO_Left_Tab, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK}}
                : std::vector<std::pair<guint, guint>>{
                      {IBUS_Page_Down, IBUS_CONTROL_MASK},
                      {IBUS_Page_Up, IBUS_CONTROL_MASK}};
        for (const auto &chord : chords) {
          invoke("Reset");
          phrase();
          require(seen.preedit == "nihao" && !seen.candidates.empty(),
                  "Modified navigation fixture did not show candidates");
          const auto committed_before = seen.committed;
          const bool chord_forwarded = !key(chord.first, chord.second);
          settle_lookup();
          require(chord_forwarded && seen.committed == committed_before &&
                      !seen.preedit_visible && !seen.lookup_visible,
                  ("Modified navigation key paged or left the composition behind: key=" +
                   std::to_string(chord.first) + " state=" + std::to_string(chord.second) +
                   " forwarded=" + std::to_string(chord_forwarded) + " committed=[" +
                   seen.committed + "] expected=[" + committed_before + "] preedit=" +
                   std::to_string(seen.preedit_visible) + " lookup=" +
                   std::to_string(seen.lookup_visible))
                      .c_str());
        }
        // The bare binding still pages after a cancelled chord.
        phrase();
        const auto chord_first_page = seen.candidates;
        require(key(binding.next) && seen.candidates != chord_first_page,
                "Navigation binding stopped paging after a modified chord");
        require(key(binding.previous,
                    binding.previous == IBUS_ISO_Left_Tab ? IBUS_SHIFT_MASK : 0) &&
                    seen.candidates == chord_first_page,
                "Navigation binding did not page back after a modified chord");
      }
      invoke("Reset");
    }
    // The panel wheel arrives as cursor_up/down. As on Windows it pages only with 鼠标滚轮 on and otherwise does nothing; keyboard arrows stay on the key path above.
    for (const bool wheel : {false, true}) {
      options["preferences"]["navigation"]["mouse_wheel"] = wheel;
      save(revision++, 4);
      settle();
      phrase();
      const auto wheel_first_page = seen.candidates;
      const auto wheel_committed = seen.committed;
      invoke("CursorDown");
      if (wheel)
        require(wait_until([&] { return seen.candidates != wheel_first_page; }) &&
                    seen.cursor == 0 && seen.committed == wheel_committed,
                "Wheel with 鼠标滚轮 on did not page the candidates");
      else
        require(seen.candidates == wheel_first_page && seen.cursor == 0 &&
                    seen.committed == wheel_committed && seen.preedit == "nihao",
                "Wheel with 鼠标滚轮 off moved the page or the highlight");
      invoke("CursorUp");
      require(seen.candidates == wheel_first_page && seen.cursor == 0 &&
                  seen.committed == wheel_committed,
              "Wheel up did not return to the first page");
      invoke("Reset");
    }
    options["preferences"]["navigation"]["mouse_wheel"] = false;
    // NumLock (Mod2) or a button mask in the panel's state argument must not block a click, as the Windows candidate window commits regardless of modifiers.
    for (const guint click_state : {guint(IBUS_MOD2_MASK), guint(IBUS_BUTTON1_MASK)}) {
      phrase();
      const auto expected = seen.committed + seen.candidates.front();
      invoke("CandidateClicked", g_variant_new("(uuu)", 0, 1, click_state));
      settle_lookup();
      require(seen.committed == expected && !seen.preedit_visible && !seen.lookup_visible,
              ("Candidate click with state " + std::to_string(click_state) +
               " did not select: committed=[" + seen.committed + "] expected=[" + expected + "]")
                  .c_str());
    }
    for (const auto &binding : bindings)
      options["preferences"]["navigation"][binding.name] = false;
    save(revision++, 4);
    settle();
    for (guint native_key : {IBUS_Tab, IBUS_ISO_Left_Tab, IBUS_Page_Down,
                             IBUS_Page_Up, IBUS_Down, IBUS_Up}) {
      require(!key(native_key), "Idle native navigation was consumed");
      phrase();
      auto expected = seen.committed + seen.candidates.front();
      // The key is what finishes the composition here, so the panel can only be
      // gone after it has been pressed.
      const bool native_forwarded = !key(native_key);
      settle_lookup();
      require(native_forwarded && seen.committed == expected &&
                  !seen.preedit_visible && !seen.lookup_visible,
              ("Disabled navigation lost input or intercepted the editor key: forwarded=" +
               std::to_string(native_forwarded) + " committed=[" + seen.committed +
               "] expected=[" + expected + "] preedit=" +
               std::to_string(seen.preedit_visible) + " lookup=" +
               std::to_string(seen.lookup_visible))
                  .c_str());
    }
    phrase();
    auto expected_punctuation = seen.committed + seen.candidates.front() + "。";
    require(key(IBUS_period) && seen.committed == expected_punctuation,
            "Disabled period paging did not restore punctuation");
    options["preferences"]["navigation"]["minus_equal"] = true;
    save(revision++, 4);
    settle();
    require(key('U', IBUS_SHIFT_MASK), "Unicode entry failed");
    require(key('+', IBUS_SHIFT_MASK) && seen.preedit == "U+",
            "Equal-key paging intercepted Unicode plus");
    for (char c : std::string("4e2d"))
      require(key(c), "Unicode digit failed");
    require(seen.preedit == "U+4e2d", "Unicode sequence was not preserved");
    require(seen.auxiliary.find("U+") != std::string::npos,
            "Unicode candidate mode indicator missing");
    invoke("Reset");
    require(seen.auxiliary.empty(), "Reset left a stale mode indicator");
    auto edge_text = [](const std::string &text, bool last) {
      auto length = g_utf8_strlen(text.c_str(), -1);
      require(length >= 1, "Expected a nonempty fixture candidate");
      gchar *part = g_utf8_substring(text.c_str(), last ? length - 1 : 0,
                                     last ? length : 1);
      std::string result(part);
      g_free(part);
      return result;
    };
    for (bool minus : {false, true}) {
      const char *group = minus ? "minus_equal" : "brackets";
      for (const auto &binding : bindings)
        options["preferences"]["navigation"][binding.name] = false;
      options["preferences"]["word_character"] = {{"enabled", true},
                                                  {"keys", group}};
      phrase();
      save(revision++, 4);
      settle();
      require(seen.preedit == "nihao", "Edge binding update canceled input");
      auto first = seen.committed + edge_text(seen.candidates.front(), false);
      require(key(minus ? IBUS_minus : IBUS_bracketleft) &&
                  seen.committed == first && !seen.preedit_visible,
              "First Han binding failed");
      phrase();
      invoke("PageDown");
      auto last =
          seen.committed + edge_text(seen.candidates.at(seen.cursor), true);
      const bool settled_commit_9 = key(minus ? IBUS_equal : IBUS_bracketright);
      settle_lookup();
      require(settled_commit_9 &&
                  seen.committed == last && !seen.lookup_visible,
              "Last Han binding did not use the displayed global candidate");
      // An invalid simultaneous paging binding must not replace live settings.
      options["preferences"]["navigation"][group] = true;
      save(revision++, 4);
      settle();
      phrase();
      first = seen.committed + edge_text(seen.candidates.front(), false);
      require(key(minus ? IBUS_minus : IBUS_bracketleft) &&
                  seen.committed == first,
              "Conflicting settings replaced the live edge binding");
      options["preferences"]["navigation"][group] = false;
    }
    // A Unicode Latin candidate contains no Han: finish it, then insert the
    // requested punctuation through the shared runtime rather than dropping it.
    options["preferences"]["word_character"]["keys"] = "brackets";
    save(revision++, 4);
    settle();
    require(key('U', IBUS_SHIFT_MASK), "Non-Han fixture entry failed");
    for (char c : std::string("0041"))
      require(key(c), "Non-Han fixture digit failed");
    auto non_han = seen.committed + "A";
    require(key(IBUS_bracketright) && seen.committed.find(non_han) == 0 &&
                seen.committed.size() > non_han.size() && !seen.preedit_visible,
            "Non-Han edge fallback lost candidate or punctuation");
    options["preferences"]["word_character"]["enabled"] = false;
    save(revision++, 4);
    settle();
    phrase();
    auto disabled = seen.committed + seen.candidates.front();
    require(key(IBUS_bracketright) && seen.committed.find(disabled) == 0 &&
                seen.committed.size() > disabled.size() &&
                !seen.preedit_visible,
            "Disabled edge binding did not restore normal punctuation");
    options["preferences"]["word_character"]["enabled"] = true;
    options.erase("preferences_directory");
    msime_ibus_configure(options.dump());
    invoke("Set",
           g_variant_new("(ssv)", "org.freedesktop.IBus.Engine", "ContentType",
                         g_variant_new("(uu)", IBUS_INPUT_PURPOSE_FREE_FORM,
                                       IBUS_INPUT_HINT_PRIVATE)));
    phrase();
    auto startup = seen.committed + edge_text(seen.candidates.front(), false);
    require(key(IBUS_bracketleft) && seen.committed == startup,
            "Startup edge binding required a preferences reload");
    require(key('U', IBUS_SHIFT_MASK), "Non-BMP fixture entry failed");
    for (char c : std::string("20000"))
      require(key(c), "Non-BMP fixture digit failed");
    auto supplementary = seen.committed + "𠀀";
    require(key(IBUS_bracketright) && seen.committed == supplementary,
            "Supplementary Han selection split a Unicode character");
    phrase();
    auto shifted = seen.committed + seen.candidates.front();
    require(key(IBUS_braceright, IBUS_SHIFT_MASK) &&
                seen.committed.find(shifted) == 0 &&
                seen.committed.size() > shifted.size(),
            "Shifted symbol triggered word-to-character selection");
    ibus_object_destroy(IBUS_OBJECT(engine));
    g_object_unref(engine);
    auto external_skin = options;
    external_skin.erase("preferences_directory");
    external_skin["candidate_skin_catalog"] = {
        {"scanned", true},
        {"packages",
         {{{"id", "sakura"},
           {"title", "樱花"},
           {"base", "system"},
           {"layouts", {"horizontal", "vertical"}},
           {"candidate", {{"light", {{"surface", "#654321"}, {"selected", "#fedcba"}}}}}}}}};
    external_skin["preferences"]["candidate_theme"] = "light";
    external_skin["preferences"]["custom_theme"] = {
        {"candidate_skin", "sakura"}, {"candidate_colors", {{"surface", "#123456"}}}};
    msime_ibus_configure(external_skin.dump());
    engine = create_engine();
    seen = Observation{};
    invoke("FocusIn");
    phrase();
    require(
        seen.second_candidate_background == 0x123456,
        "Custom candidate surface color was overwritten by an external skin");
    require(seen.first_candidate_background == 0xfedcba,
            "External skin selected color was not drawn");
    require(seen.global_theme == "sakura",
            "Theme menu did not show the external skin drawn");
    // Screen keyboard keys arrive over panel-input.sock and go through the engine before the editor, the way SendInput passes through the IME on Windows. Text from handwriting, emoji and voice is still committed as it is.
    {
      require(key(IBUS_Escape), "Screen keyboard fixture could not cancel the composition");
      const auto panel_socket = runtime / "msime-client" / "panel-input.sock";
      require(std::filesystem::exists(panel_socket), "Panel input socket did not open on focus");
      // The host serves the socket from this thread's main loop, so keep iterating it until the reply arrives.
      auto panel = [&](const nlohmann::json &request) {
        std::string reply;
        const int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
        require(fd >= 0, "Panel input client socket");
        sockaddr_un address{};
        address.sun_family = AF_UNIX;
        std::strncpy(address.sun_path, panel_socket.c_str(), sizeof(address.sun_path) - 1);
        const auto line = request.dump() + "\n";
        if (::connect(fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) == 0 &&
            send(fd, line.data(), line.size(), MSG_NOSIGNAL) == static_cast<ssize_t>(line.size())) {
          const auto deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
          while (reply.find('\n') == std::string::npos && g_get_monotonic_time() < deadline) {
            while (g_main_context_iteration(nullptr, FALSE)) {}
            pollfd ready{fd, POLLIN, 0};
            if (poll(&ready, 1, 1) <= 0) continue;
            char buffer[256];
            const auto count = recv(fd, buffer, sizeof(buffer), 0);
            if (count <= 0) break;
            reply.append(buffer, static_cast<size_t>(count));
          }
        }
        close(fd);
        return reply == "{\"ok\":true}\n";
      };
      // Evdev codes, as the panel sends them.
      auto panel_key = [&](const char *name, guint keycode) {
        return panel({{"op", "key"}, {"key", name}, {"keycode", keycode}});
      };
      seen.forwarded.clear();
      auto before = seen.committed;
      require(panel_key("n", 49) && panel_key("i", 23) && panel_key("h", 35) &&
                  panel_key("a", 30) && panel_key("o", 24),
              "Screen keyboard letters were refused");
      require(wait_until([&] { return seen.preedit_visible && seen.preedit == "nihao"; }) &&
                  !seen.candidates.empty(),
              "Screen keyboard letters did not compose");
      // Earlier sections select and learn nihao candidates, so the first one is whatever the ranking now puts there, as every other phrase() check in this file assumes.
      const auto composed = before + seen.candidates.front();
      require(panel_key("space", 57), "Screen keyboard Space was refused");
      require(wait_until([&] { return seen.committed != before; }) &&
                  seen.committed == composed && seen.forwarded.empty(),
              "Screen keyboard typed raw letters instead of composing");
      // A composition started on the physical keyboard: the screen keyboard's digits select from it and its BackSpace edits it.
      phrase();
      require(seen.candidates.size() >= 2, "Screen keyboard selection fixture has one candidate");
      const auto second = seen.committed + seen.candidates.at(1);
      require(panel_key("2", 3), "Screen keyboard digit was refused");
      require(wait_until([&] { return seen.committed == second; }) && seen.forwarded.empty(),
              "Screen keyboard digit did not select the second candidate");
      // A candidate shorter than the reading leaves the rest composing.
      key(IBUS_Escape);
      phrase();
      require(panel_key("BackSpace", 14), "Screen keyboard BackSpace was refused");
      require(wait_until([&] { return seen.preedit == "niha"; }) && seen.forwarded.empty(),
              "Screen keyboard BackSpace did not edit the composition");
      require(key(IBUS_Escape), "Screen keyboard fixture could not cancel the composition");
      // With nothing to compose, the whole stroke goes on to the editor.
      require(panel_key("BackSpace", 14), "Idle screen keyboard BackSpace was refused");
      require(wait_until([&] { return seen.forwarded.size() == 2; }) &&
                  seen.forwarded[0].keyval == IBUS_BackSpace && seen.forwarded[0].keycode == 14 &&
                  (seen.forwarded[0].state & IBUS_RELEASE_MASK) == 0 &&
                  seen.forwarded[1].keyval == IBUS_BackSpace &&
                  (seen.forwarded[1].state & IBUS_RELEASE_MASK) != 0,
              "Idle screen keyboard BackSpace did not reach the editor as one stroke");
      before = seen.committed;
      require(panel({{"op", "text"}, {"text", "好"}}), "Panel text was refused");
      require(wait_until([&] { return seen.committed == before + "好"; }) &&
                  seen.forwarded.size() == 2,
              "Panel text did not commit as it is");
      // Under CapsLock a physical letter arrives uppercase with the lock and goes to the editor; the screen keyboard's lowercase letter has to do the same rather than start a composition.
      require(!key('N', IBUS_LOCK_MASK) && !key('N', IBUS_LOCK_MASK | IBUS_RELEASE_MASK) &&
                  !seen.preedit_visible,
              "CapsLock letter was composed");
      seen.forwarded.clear();
      require(panel_key("n", 49), "Screen keyboard letter under CapsLock was refused");
      require(wait_until([&] { return seen.forwarded.size() == 2; }) &&
                  seen.forwarded[0].keyval == 'N' && seen.forwarded[1].keyval == 'N' &&
                  (seen.forwarded[0].state & IBUS_RELEASE_MASK) == 0 &&
                  (seen.forwarded[1].state & IBUS_RELEASE_MASK) != 0 && !seen.preedit_visible,
              "Screen keyboard letter under CapsLock did not reach the editor uppercase");
      // A key without the lock reports it off again.
      key(IBUS_Escape);
      key(IBUS_Escape, IBUS_RELEASE_MASK);
      // In English mode the engine passes the letter on untouched, as one whole stroke.
      require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                  key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK) && !seen.input_enabled,
              "Ctrl+Space did not enter English mode for the screen keyboard");
      seen.forwarded.clear();
      require(panel_key("n", 49), "Screen keyboard letter in English mode was refused");
      require(wait_until([&] { return seen.forwarded.size() == 2; }) &&
                  seen.forwarded[0].keyval == 'n' && seen.forwarded[0].keycode == 49 &&
                  (seen.forwarded[0].state & IBUS_RELEASE_MASK) == 0 &&
                  seen.forwarded[1].keyval == 'n' &&
                  (seen.forwarded[1].state & IBUS_RELEASE_MASK) != 0 && !seen.preedit_visible,
              "Screen keyboard letter in English mode did not reach the editor as one stroke");
      require(key(IBUS_space, IBUS_CONTROL_MASK) &&
                  key(IBUS_space, IBUS_CONTROL_MASK | IBUS_RELEASE_MASK) && seen.input_enabled,
              "Ctrl+Space did not restore Chinese mode after the screen keyboard");
    }
    invoke("Disable");
    require(!key('n'), "Disabled engine consumed input");
    // 注音、越南文、藏文和笔画各自跑在单独的引擎上，上面的用例不受它们留下的状态影响。注入的选项是权威来源，所以从菜单选的方案立即生效，而不是经由偏好存储。
    {
      const auto dictionaries = root / "language-dictionaries";
      std::filesystem::create_directory(dictionaries);
      auto languages = options;
      languages.erase("preferences_directory");
      languages["language_dictionaries"] = dictionaries.string();
      languages["preferences"]["scheme"] = "zhuyin";
      languages["preferences"]["last_chinese_scheme"] = "quanpin";
      languages["preferences"]["character_width"] = "halfwidth";
      languages["preferences"]["vietnamese"]["input_method"] = "vni";
      const auto restart = [&] {
        ibus_object_destroy(IBUS_OBJECT(engine));
        g_object_unref(engine);
        msime_ibus_configure(languages.dump());
        engine = create_engine();
        seen = Observation{};
        invoke("FocusIn");
      };
      const auto offered = [&](const char *entry) { return seen.scheme_entries.count(entry) != 0; };
      const auto checked = [&](const char *entry) {
        const auto found = seen.scheme_entries.find(entry);
        return found != seen.scheme_entries.end() && found->second;
      };
      // Zhuyin saved as the scheme while its dictionary is missing: host-api runs the last Chinese scheme instead, and the menu neither offers Zhuyin nor claims it is in use.
      restart();
      require(offered("Scheme/Quanpin") && offered("Scheme/Vietnamese") && !offered("Scheme/Zhuyin") &&
                  !offered("Scheme/Cantonese") && !offered("Scheme/Stroke"),
              "The scheme menu offered Zhuyin or Stroke without its dictionary");
      require(checked("Scheme/Chinese") && checked("Scheme/Quanpin"),
              "The scheme menu did not mark the quanpin fallback of a missing Zhuyin dictionary");
      require(key('n') && seen.preedit == "n" && seen.lookup_visible && !seen.candidates.empty(),
              "A missing Zhuyin dictionary did not fall back to quanpin");
      invoke("Reset");
      invoke("PropertyActivate", g_variant_new("(su)", "Scheme/Zhuyin", PROP_STATE_CHECKED));
      require(key('n') && seen.preedit == "n" && seen.lookup_visible,
              "Zhuyin was selected without its dictionary");
      invoke("Reset");
      invoke("PropertyActivate", g_variant_new("(su)", "Scheme/Stroke", PROP_STATE_CHECKED));
      require(key('n') && seen.preedit == "n" && seen.lookup_visible && !checked("Scheme/Stroke"),
              "Stroke was selected without its dictionary");
      invoke("Reset");
      // With the dictionary installed the saved Zhuyin runs and is offered.
      const auto fixture = std::string("python3 '") + MSIME_ZHUYIN_DICTIONARY_FIXTURE + "' '" + dictionaries.string() + "'";
      require(std::system(fixture.c_str()) == 0, "Zhuyin dictionary fixture was not written");
      restart();
      require(offered("Scheme/Zhuyin") && checked("Scheme/Chinese") && checked("Scheme/Zhuyin") &&
                  !checked("Scheme/Quanpin") && !offered("Scheme/Cantonese"),
              "The scheme menu did not offer and mark Zhuyin with its dictionary installed");
      // 1 8 spells ㄅㄚ: the digit is a bopomofo key, not a candidate shortcut, and Space is the first tone, which converts without opening a list.
      auto before = seen.committed;
      require(key('1') && seen.preedit_visible && key('8') && seen.committed == before,
              "Zhuyin did not spell with the digit row");
      require(key(IBUS_space) && seen.preedit == "八" && seen.committed == before &&
                  seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT,
              "Space did not give the Zhuyin syllable its first tone");
      settle_lookup();
      require(!seen.lookup_visible, "The first tone opened the Zhuyin list");
      require(key(IBUS_Down) && seen.lookup_visible && seen.candidates.size() == 2 &&
                  seen.candidates[0].rfind("八", 0) == 0 && seen.candidates[1].rfind("巴", 0) == 0,
              "Down did not open the Zhuyin list");
      require(key('2') && seen.preedit == "巴" && seen.committed == before,
              "A digit did not pick from the open Zhuyin list without committing");
      settle_lookup();
      require(!seen.lookup_visible, "Picking a Zhuyin candidate left the list open");
      require(key(IBUS_Down) && seen.lookup_visible && key('1') && seen.preedit == "八" &&
                  seen.committed == before,
              "1 did not pick the first row of the Zhuyin list");
      require(key(IBUS_Return) && seen.committed == before + "八" && !seen.preedit_visible,
              "Return did not commit the Zhuyin conversion");
      // F9 opens the list too, Escape closes it and keeps the conversion, and a second Escape discards it.
      before = seen.committed;
      require(key('1') && key('8') && key(IBUS_space) && key(IBUS_F9) && seen.lookup_visible,
              "F9 did not open the Zhuyin list");
      require(key(IBUS_Escape) && seen.preedit == "八", "Escape did not close the Zhuyin list");
      settle_lookup();
      require(!seen.lookup_visible && key(IBUS_Escape) && !seen.preedit_visible && seen.committed == before,
              "Escape did not discard the Zhuyin conversion");
      // The comma is ㄝ on the Dachen keyboard, not Chinese punctuation.
      require(key(IBUS_comma) && key(IBUS_space) && seen.preedit == "欸" && seen.committed == before,
              "The comma did not spell ㄝ");
      require(key(IBUS_Return) && seen.committed == before + "欸", "Return did not commit 欸");
      // Vietnamese: VNI digits mark the word, drawn inline in COMMIT mode, and it never opens a list.
      invoke("PropertyActivate", g_variant_new("(su)", "Scheme/Vietnamese", PROP_STATE_CHECKED));
      require(wait_until([&] { return checked("Scheme/Vietnamese") && !checked("Scheme/Chinese"); }),
              "The scheme menu did not select Vietnamese");
      before = seen.committed;
      require(!key('6') && seen.committed == before, "An idle VNI digit was not left to the application");
      for (char c : std::string("viet65"))
        require(key(c), "A VNI key was not consumed");
      require(seen.preedit == "việt" && seen.preedit_visible && seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT &&
                  seen.committed == before,
              "VNI digits did not compose việt inline");
      settle_lookup();
      require(!seen.lookup_visible, "Vietnamese opened a candidate list");
      // A mark follows the word as ASCII, also with fullwidth output on, and an idle mark is the application's.
      invoke("PropertyActivate", g_variant_new("(su)", "CharacterMode", PROP_STATE_CHECKED));
      require(wait_until([&] { return seen.character_width; }), "Fullwidth output was not turned on");
      key(IBUS_comma);
      require(seen.committed == before + "việt," && !seen.preedit_visible,
              "The comma after a Vietnamese word was not committed as ASCII with it");
      require(!key(IBUS_comma) && seen.committed == before + "việt,",
              "An idle Vietnamese comma was widened or kept from the application");
      require(key('a') && seen.preedit == "a" && !key(IBUS_space) && seen.committed == before + "việt,a",
              "Space did not commit the Vietnamese word unwidened and reach the application");
      invoke("PropertyActivate", g_variant_new("(su)", "CharacterMode", PROP_STATE_UNCHECKED));
      require(wait_until([&] { return !seen.character_width; }), "Fullwidth output was not turned off");
      // Caps Lock types a capital, which starts a word rather than going to the application.
      before = seen.committed;
      require(key('A', IBUS_LOCK_MASK) && seen.preedit == "A" && seen.committed == before,
              "Caps Lock did not start a Vietnamese word with a capital");
      require(!key(IBUS_Return) && seen.committed == before + "A" && !seen.preedit_visible,
              "Return did not commit the Vietnamese word and reach the application");
      // Leaving the field hands the word to the client in COMMIT mode, so the host commits nothing of its own and the next field starts empty.
      before = seen.committed;
      require(key('v') && key('i') && seen.preedit == "vi" && seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT,
              "Vietnamese word for the focus change");
      invoke("FocusOut");
      require(wait_until([&] { return !seen.preedit_visible; }) && seen.committed == before,
              "Focus out did not leave the Vietnamese word to IBus");
      invoke("FocusIn");
      require(key('a') && seen.preedit == "a" && seen.committed == before,
              "The Vietnamese word survived the focus change");
      invoke("Reset");
      // 藏文：威利原文在 COMMIT 模式下内嵌显示为转换后的藏文，从不打开列表。空格带音节点上屏、斜杠带垂符上屏，两者都被吞掉；回车只上屏藏文，同样被吞掉。
      require(offered("Scheme/Tibetan"), "The scheme menu did not offer Tibetan");
      invoke("PropertyActivate", g_variant_new("(su)", "Scheme/Tibetan", PROP_STATE_CHECKED));
      require(wait_until([&] { return checked("Scheme/Tibetan") && !checked("Scheme/Vietnamese") &&
                                      !checked("Scheme/Chinese"); }),
              "The scheme menu did not select Tibetan");
      before = seen.committed;
      require(!key('1') && seen.committed == before, "An idle digit was not left to the application in Tibetan");
      for (char c : std::string("bkra"))
        require(key(c), "A Wylie letter was not consumed");
      require(seen.preedit == "བཀྲ" && seen.preedit_visible && seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT &&
                  seen.committed == before,
              "Wylie did not compose བཀྲ inline");
      settle_lookup();
      require(!seen.lookup_visible, "Tibetan opened a candidate list");
      require(key(IBUS_space) && seen.committed == before + "བཀྲ་" && !seen.preedit_visible,
              "Space did not commit the Tibetan syllable with a tsheg and keep the key");
      for (char c : std::string("shis"))
        require(key(c), "A Wylie letter was not consumed");
      require(key(IBUS_slash) && seen.committed == before + "བཀྲ་ཤིས།" && !seen.preedit_visible,
              "The slash did not commit the Tibetan syllable with a shad");
      require(key(IBUS_slash) && seen.committed == before + "བཀྲ་ཤིས།།", "An idle slash did not write a shad");
      // 撇号在空闲时也是拼写（achung 开头的音节），加号是叠写；带 Shift 或 CapsLock 的大写字母是另一个字母，不交给应用。
      before = seen.committed;
      require(key(IBUS_apostrophe) && key('o') && key('d') && seen.preedit == "འོད" && seen.committed == before,
              "An apostrophe did not start an achung syllable");
      require(key(IBUS_Return) && seen.committed == before + "འོད" && !seen.preedit_visible,
              "Return did not commit the Tibetan syllable without a tsheg and keep the key");
      before = seen.committed;
      require(key('p') && key('a') && key('d') && key(IBUS_plus, IBUS_SHIFT_MASK) && key('m') && key('a') &&
                  seen.preedit == "པདྨ" && seen.committed == before,
              "The plus did not stack the Wylie letters");
      require(key(IBUS_Return) && seen.committed == before + "པདྨ", "Return did not commit the stacked syllable");
      before = seen.committed;
      require(key('T', IBUS_SHIFT_MASK) && key('a') && seen.preedit == "ཊ" && seen.committed == before,
              "Shift did not type the uppercase Wylie letter");
      require(key(IBUS_Return) && seen.committed == before + "ཊ", "Return did not commit the retroflex letter");
      before = seen.committed;
      require(key('D', IBUS_LOCK_MASK) && key('a', IBUS_LOCK_MASK) && seen.preedit == "ཌ" && seen.committed == before,
              "Caps Lock did not start a Tibetan syllable with the uppercase Wylie letter");
      require(key(IBUS_Return) && seen.committed == before + "ཌ", "Return did not commit the Caps Lock syllable");
      // 第一次 Esc 把显示退回威利原文，Backspace 删一个原文按键，第二次 Esc 丢弃组字。
      before = seen.committed;
      require(key('k') && key('a') && seen.preedit == "ཀ" && key(IBUS_Escape) && seen.preedit == "ka" &&
                  seen.committed == before,
              "Escape did not restore the raw Wylie");
      require(key(IBUS_Escape) && !seen.preedit_visible && seen.committed == before,
              "A second Escape did not discard the Tibetan composition");
      require(key('k') && key('a') && key(IBUS_BackSpace) && seen.preedit == "ཀ" && seen.committed == before,
              "Backspace did not take back one Wylie key");
      require(key(IBUS_Escape) && key(IBUS_Escape) && !seen.preedit_visible, "The Tibetan composition was not discarded");
      // 其他标点跟在藏文后面写成 ASCII，全角输出打开时也一样；空闲的标点和空格都交给应用，不变全角。
      invoke("PropertyActivate", g_variant_new("(su)", "CharacterMode", PROP_STATE_CHECKED));
      require(wait_until([&] { return seen.character_width; }), "Fullwidth output was not turned on");
      before = seen.committed;
      require(key('k') && key('a') && seen.preedit == "ཀ", "Wylie did not compose before the comma");
      key(IBUS_comma);
      require(seen.committed == before + "ཀ," && !seen.preedit_visible,
              "The comma after a Tibetan syllable was not committed as ASCII with it");
      require(!key(IBUS_comma) && !key(IBUS_space) && seen.committed == before + "ཀ,",
              "An idle Tibetan comma or space was widened or kept from the application");
      invoke("PropertyActivate", g_variant_new("(su)", "CharacterMode", PROP_STATE_UNCHECKED));
      require(wait_until([&] { return !seen.character_width; }), "Fullwidth output was not turned off");
      // 导航键先把藏文按显示写出去（不带音节点），再交给应用。
      before = seen.committed;
      require(key('k') && key('a') && !key(IBUS_Left) && seen.committed == before + "ཀ" && !seen.preedit_visible,
              "Left did not write the Tibetan syllable out without a tsheg and reach the application");
      // 离开输入框时组字以 COMMIT 模式交给客户端，宿主自己不上屏，下一个输入框从空开始。
      before = seen.committed;
      require(key('k') && key('a') && seen.preedit == "ཀ" && seen.preedit_mode == IBUS_ENGINE_PREEDIT_COMMIT,
              "Tibetan syllable for the focus change");
      invoke("FocusOut");
      require(wait_until([&] { return !seen.preedit_visible; }) && seen.committed == before,
              "Focus out did not leave the Tibetan syllable to IBus");
      invoke("FocusIn");
      require(key('g') && key('a') && seen.preedit == "ག" && seen.committed == before,
              "The Tibetan syllable survived the focus change");
      invoke("Reset");
      // 笔画：装好 msime-stroke.db 并重新读取选项后出现在菜单里，并从菜单选中，这样能发现 PropertyActivate 白名单或 id 映射漏掉的 Scheme/Stroke 一项。
      const auto stroke_fixture =
          std::string("python3 '") + MSIME_STROKE_DICTIONARY_FIXTURE + "' '" + dictionaries.string() + "'";
      require(std::system(stroke_fixture.c_str()) == 0, "Stroke dictionary fixture was not written");
      languages["preferences"]["scheme"] = "quanpin";
      restart();
      require(offered("Scheme/Stroke") && offered("Scheme/Zhuyin") && !checked("Scheme/Stroke") &&
                  checked("Scheme/Quanpin"),
              "The scheme menu did not offer Stroke with its dictionary installed");
      invoke("PropertyActivate", g_variant_new("(su)", "Scheme/Stroke", PROP_STATE_CHECKED));
      require(wait_until([&] { return checked("Scheme/Stroke") && checked("Scheme/Chinese"); }),
              "The scheme menu did not select Stroke");
      // 空闲时只有五个笔画字母开始组合：通配符 x 和其他字母都交给应用。
      before = seen.committed;
      require(!key('x') && !seen.preedit_visible && !key('a') && !seen.preedit_visible && seen.committed == before,
              "An idle Stroke wildcard or other letter was not left to the application");
      // 原样预编辑样式画出 Engine 放在 `reading` 里的笔画字形，editing_text 仍是 ASCII 字母，所以原样样式的可打印 ASCII 检查不会拒绝它。
      require(key('h') && seen.preedit_visible && seen.preedit == "一" && seen.lookup_visible &&
                  !seen.candidates.empty() && seen.candidates[0].rfind("一", 0) == 0,
              "h did not compose the stroke 一");
      require(key('s') && seen.preedit == "一丨" && seen.candidates.size() >= 3 &&
                  seen.candidates[0].rfind("十", 0) == 0 && seen.candidates[1].rfind("木", 0) == 0 &&
                  seen.candidates[2].rfind("古", 0) == 0,
              "h s did not list 十 exactly and then its completions");
      // 组字中其他字母被吞掉，组合不变。
      require(key('q') && seen.preedit == "一丨" && seen.committed == before,
              "A non-stroke letter changed the Stroke composition");
      require(key(IBUS_space) && seen.committed == before + "十" && !seen.preedit_visible,
              "Space did not commit the highlighted Stroke candidate");
      // 组字中 x 是通配符：h x 先列出两笔的码，再列更长的。
      before = seen.committed;
      require(key('h') && key('x') && seen.preedit == "一＊" && seen.candidates.size() >= 2 &&
                  seen.candidates[0].rfind("十", 0) == 0 && seen.candidates[1].rfind("二", 0) == 0,
              "The Stroke wildcard did not match any one stroke");
      require(key('2') && seen.committed == before + "二" && !seen.preedit_visible,
              "A digit did not pick the Stroke candidate");
      // Backspace 删掉最后一笔，回车写出键入的字母，Esc 清空。
      before = seen.committed;
      require(key('h') && key('s') && key(IBUS_BackSpace) && seen.preedit == "一" && seen.committed == before,
              "Backspace did not remove the last stroke");
      require(key('s') && key(IBUS_Return) && seen.committed == before + "hs" && !seen.preedit_visible,
              "Return did not commit the typed stroke letters");
      before = seen.committed;
      require(key('p') && key('n') && seen.preedit == "丿丶" && key(IBUS_Escape) && !seen.preedit_visible &&
                  seen.committed == before,
              "Escape did not clear the Stroke composition");
      invoke("Reset");
    }
    finish();
    std::cout << "IBus D-Bus shared-runtime acceptance passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
