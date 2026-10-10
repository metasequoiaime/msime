#include "ServerSession.h"
#include "../../../common/HostApiString.h"
#include "CandidateCompletionPolicy.h"
#include "EditPolicy.h"
#include "InputSchemeTraits.h"
#include "input/CandidateTextPolicy.h"
#include "KeyEvent.h"
#include "PunctuationPolicy.h"
#include <algorithm>
#include <stdexcept>

namespace msime::windows {
namespace {
nlohmann::json response(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!raw)
    throw std::runtime_error("Missing shared host response");
  auto document = nlohmann::json::parse(raw);
  // Do not propagate input/path-bearing raw library diagnostics to production
  // logs.
  if (!document.at("ok").get<bool>())
    throw std::runtime_error("Shared host operation failed");
  return document.at("value");
}

bool translation_preferences_changed(const nlohmann::json &before,
                                     const nlohmann::json &after) {
  for (const auto *key : {"candidate_translations", "candidate_english_gloss",
                          "translation_target_language",
                          "translation_secondary_language", "translation_account",
                          "custom_translation", "niutrans", "tencent_tmt"}) {
    if (before.value(key, nlohmann::json(nullptr)) !=
        after.value(key, nlohmann::json(nullptr)))
      return true;
  }
  return false;
}
} // namespace
ServerSession::ServerSession(uint64_t client_id, const std::string &options)
    : client_(client_id) {
  if (!client_ || options.size() > 16384 || msime_client_abi_version() != 3)
    throw std::invalid_argument("Invalid Windows session configuration");
  const auto document = nlohmann::json::parse(options);
  preferences_ = document.value("preferences", nlohmann::json::object());
  traditional_output_ = document.value("preferences", nlohmann::json::object())
                            .value("traditional_chinese_output", false);
  auto created = response(msime_client_create(
      reinterpret_cast<const uint8_t *>(options.data()), options.size()));
  session_ = created.at("session").get<uint64_t>();
}
ServerSession::~ServerSession() {
  // Silently destroying on another thread would leak the thread-local Rust
  // session. Ownership cannot be transferred to a pipe I/O worker.
  if (std::this_thread::get_id() != thread_)
    std::terminate();
  // The player outlives every session, so music this session let play would otherwise go on with no input method in front of it.
  if (music_active_)
    (void)msime_client_music_set_active(session_, false);
  msime_client_string_free(msime_client_destroy(session_));
}
void ServerSession::check_thread() const {
  if (std::this_thread::get_id() != thread_)
    throw std::logic_error("Wrong Server session thread");
}
void ServerSession::check_active(uint64_t epoch) const {
  check_thread();
  if (!active_ || !epoch || epoch != epoch_)
    throw std::logic_error("Expired Windows focus route");
}
nlohmann::json ServerSession::activate(uint64_t epoch) {
  check_thread();
  if (!epoch || epoch < epoch_ || (epoch == epoch_ && !active_))
    throw std::logic_error("Expired Windows activation");
  if (active_ && epoch != epoch_)
    response(msime_client_focus(session_, false));
  auto result = response(msime_client_focus(session_, input_enabled_));
  epoch_ = epoch;
  active_ = true;
  refresh_typing_effect_settings();
  return result;
}
nlohmann::json ServerSession::deactivate(uint64_t epoch) {
  check_active(epoch);
  auto result = response(msime_client_focus(session_, false));
  active_ = false;
  return result;
}
void ServerSession::set_input_enabled(uint64_t epoch, bool enabled) {
  check_active(epoch);
  if (input_enabled_ != enabled) {
    response(msime_client_focus(session_, enabled));
    input_enabled_ = enabled;
  }
}
nlohmann::json ServerSession::cancel_again(nlohmann::json result) {
  // 韩文汉字列表或注音列表打开时，MSIME_CANCEL 只关闭列表、组字保留（msime_client.h）；越南文词和藏文音节串上的第一次只把原文重新显示出来；第二次才丢弃它。
  if (result.at("commit").is_null() &&
      scheme::AlwaysInlinePreedit(static_cast<int>(result.at("view").value("scheme", 0u))) &&
      !result.at("view").at("editing_text").get<std::string>().empty())
    return response(msime_client_command(session_, MSIME_CANCEL));
  return result;
}
void ServerSession::cancel_composition(uint64_t epoch) {
  check_active(epoch);
  auto result = response(msime_client_command(session_, MSIME_CANCEL));
  result = cancel_again(std::move(result));
  if (!result.at("commit").is_null() ||
      !result.at("view").at("editing_text").get<std::string>().empty() ||
      !result.at("view").at("candidates").empty())
    throw std::logic_error("Shared host did not cancel composition");
}
nlohmann::json ServerSession::finish_composition(uint64_t epoch) {
  check_active(epoch);
  return response(msime_client_command(session_, MSIME_FINISH_COMPOSITION));
}
nlohmann::json ServerSession::command(uint64_t epoch, uint32_t command) {
  check_active(epoch);
  if (!input_enabled_)
    throw std::logic_error("Session command while input disabled");
  return response(msime_client_command(session_, command));
}
void ServerSession::reset_cache() {
  check_thread();
  (void)response(msime_client_reset_cache(session_));
}
void ServerSession::set_chinese_punctuation(uint64_t epoch, bool enabled) {
  check_active(epoch);
  response(msime_client_set_chinese_punctuation(session_, enabled));
}
void ServerSession::balance_paired_punctuation(uint64_t epoch,
                                               uint8_t opening) {
  check_active(epoch);
  response(msime_client_balance_paired_punctuation_after_auto_close(session_,
                                                                   opening));
}
nlohmann::json ServerSession::toggle_traditional_output(uint64_t epoch) {
  check_active(epoch);
  traditional_output_ = !traditional_output_;
  return view();
}
nlohmann::json ServerSession::dedicated_english(uint64_t epoch, bool exit) {
  check_active(epoch);
  auto current = view();
  if (!exit || !current.at("dedicated_english").get<bool>())
    return current;
  cancel_composition(epoch);
  return response(msime_client_set_english_mode(session_, false));
}
nlohmann::json ServerSession::toggle_dedicated_english(uint64_t epoch) {
  check_active(epoch);
  const bool enabled = view().at("dedicated_english").get<bool>();
  cancel_composition(epoch);
  return response(msime_client_set_english_mode(session_, !enabled));
}
KeyResult ServerSession::key(const FanyImeNamedpipeData &packet,
                             uint64_t epoch) {
  check_active(epoch);
  const auto action = translate_key(packet);
  if (packet.client_id != client_ ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      packet.request_id == FANY_IME_NO_REQUEST_ID ||
      (!packet.request_id && action.kind != KeyKind::LocalReset &&
       action.kind != KeyKind::Ignore) ||
      packet.pinyin_length < 0 || packet.pinyin_length >= 128)
    throw std::invalid_argument("Invalid Windows key request");
  nlohmann::json result;
  if (!input_enabled_) {
    result = {{"handled", false},
              {"commit", nullptr},
              {"diagnostic", nullptr},
              {"view", view()}};
    return {client_, epoch_, packet.request_id,
            action.kind != KeyKind::LocalReset &&
                action.kind != KeyKind::Ignore,
            std::move(result)};
  }
  const auto modifiers = PipeMetadata::key_modifiers(packet.modifiers_down);
  const auto digit_key = normalize_digit_key(packet.keycode);
  nlohmann::json current = view();
  // Digits only. The branch selects a candidate by slot, and without this it
  // claimed every unmodified key: 'b' is VK 66, so digit_key - '1' is 17, and
  // an empty page sent it to the else below - reported handled, never given to
  // the Engine, so the character was silently dropped. A page of eighteen or
  // more candidates would have been worse, committing candidate 17 for a
  // letter press. Typing shuangpin through this path could not work at all,
  // and nothing noticed because these suites had never been run.
  // 韩文和注音只在列表打开时有候选，这时数字从中选择；越南文和藏文没有候选。其他情况下数字是文字，引擎作为字符收到时要么拼写它，要么让它结束组字。
  const bool selection_digit =
      digit_key >= '1' && digit_key <= '9' &&
      !(current.is_object() &&
        scheme::AlwaysInlinePreedit(static_cast<int>(current.value("scheme", 0u))) &&
        current.at("candidates").empty());
  if (selection_digit && !current.is_null() &&
      digit_selects_candidate(
          current.at("local_mode").get<std::string>(),
          current.value("spelling_symbols", std::string{}),
          static_cast<uint32_t>(packet.wch), modifiers)) {
    // TSF selects by VK digit. Unicode requires Shift, a digit the Engine spells (V) is input, and ordinary modes use unmodified digits; see digit_selects_candidate. Use current IDs.
    const auto slot = static_cast<size_t>(digit_key - '1');
    const auto &page = current.at("candidates");
    if (slot < page.size()) {
      const auto &id = page.at(slot).at("id");
      result = select(epoch, id.at("generation").get<uint64_t>(),
                      id.at("index").get<size_t>());
    } else {
      result = {
          {"handled", !current.at("editing_text").get<std::string>().empty()},
          {"commit", nullptr},
          {"diagnostic", nullptr},
          {"view", current}};
    }
  } else if (action.kind == KeyKind::Ignore) {
    result = {{"handled", false},
              {"commit", nullptr},
              {"diagnostic", nullptr},
              {"view", view()}};
  } else if (action.kind == KeyKind::Character) {
    result = response(msime_client_character(
        session_, static_cast<uint8_t>(action.value), action.shift));
  } else {
    if ((action.value == MSIME_BACKSPACE_SEGMENT || action.value == MSIME_MOVE_LEFT_SEGMENT ||
         action.value == MSIME_MOVE_RIGHT_SEGMENT) && current.at("editing_text").get<std::string>().empty()) {
      result = {{"handled", false}, {"commit", nullptr}, {"diagnostic", nullptr}, {"view", current}};
      return {client_, epoch_, packet.request_id, false, std::move(result)};
    }
    result = response(msime_client_command(session_, action.value));
    // A reset discards the composition, as the TIP discards it from its own host session. Escape on a word whose first cancel only shows its raw keys again stops there, as the TIP does (scheme::CancelRestoresRaw).
    if (action.kind == KeyKind::LocalReset &&
        !(packet.keycode == kVirtualKeyEscape &&
          scheme::CancelRestoresRaw(static_cast<int>(result.at("view").value("scheme", 0u)))))
      result = cancel_again(std::move(result));
    if (action.kind == KeyKind::CancelAndForward ||
        action.kind == KeyKind::LocalReset)
      result["handled"] = false;
  }
  return {client_, epoch_, packet.request_id,
          action.kind != KeyKind::LocalReset && action.kind != KeyKind::Ignore,
          std::move(result)};
}
KeyResult ServerSession::restore_raw(uint64_t epoch, uint64_t request,
                                     const std::string &raw) {
  check_active(epoch);
  if (!request || request == FANY_IME_NO_REQUEST_ID || raw.empty() ||
      raw.size() > 127 || raw.find('\0') != std::string::npos)
    throw std::invalid_argument("Invalid Windows segment restoration");
  nlohmann::json result;
  for (const unsigned char value : raw) {
    if (value < 0x21 || value > 0x7e)
      throw std::invalid_argument("Invalid Windows segment restoration text");
    result = response(msime_client_character(session_, value, false));
  }
  return {client_, epoch_, request, true, std::move(result)};
}
std::optional<NavigationResult>
ServerSession::navigate(const FanyImeNamedpipeData &packet, uint64_t epoch,
                        const NavigationBindings &bindings) {
  check_active(epoch);
  if (packet.client_id != client_ ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      !packet.request_id || packet.request_id == FANY_IME_NO_REQUEST_ID ||
      packet.pinyin_length < 0 || packet.pinyin_length >= 128)
    throw std::invalid_argument("Invalid Windows navigation request");
  if (!input_enabled_)
    return std::nullopt;
  // Do not fetch a full snapshot for keys that cannot use these bindings.
  auto action = navigation_action(packet, bindings, false, false);
  if (!action)
    return std::nullopt;
  const auto current = view();
  if (current.at("editing_text").get<std::string>().empty())
    return std::nullopt;
  // 日文、韩文、注音、越南文和藏文是方案，不是本地模式，没有本地模式以它们命名。它们把 '-' 和 '=' 当作文字而不是翻页键：注音列表用 Page Up/Down 和方向键翻页（KoreanHanjaKey.h），藏文的 '-' 是威利拼写符号。
  const int scheme = static_cast<int>(current.value("scheme", 0u));
  action = navigation_action(packet, bindings,
                             current.at("local_mode").get<std::string>() == "unicode",
                             scheme == scheme::Japanese || scheme::AlwaysInlinePreedit(scheme));
  if (!action)
    return std::nullopt;
  auto result = action->command
                    ? response(msime_client_command(session_, *action->command))
                    : nlohmann::json{{"handled", true},
                                     {"commit", nullptr},
                                     {"diagnostic", nullptr},
                                     {"view", current}};
  return NavigationResult{
      {client_, epoch_, packet.request_id, true, std::move(result)},
      action->reply};
}
nlohmann::json ServerSession::select(uint64_t epoch, uint64_t generation,
                                     size_t index) {
  check_active(epoch);
  if (!input_enabled_)
    throw std::logic_error("Candidate selection while input disabled");
  // Cloud suggestions are already complete results for their query. The
  // Windows server commits them as a whole and clears any shorter residual
  // pinyin instead of entering the ordinary partial-word creation path.
  bool completes_composition = false;
  const auto current = view();
  if (current.at("generation").get<uint64_t>() == generation) {
    for (const auto &candidate : current.at("candidates")) {
      const auto &id = candidate.at("id");
      if (id.at("generation").get<uint64_t>() == generation &&
          id.at("index").get<size_t>() == index) {
        completes_composition = candidate_finishes_composition(
            candidate.value("source", uint8_t{}));
        break;
      }
    }
  }
  auto result = response(msime_client_select(session_, generation, index));
  if (completes_composition && !result.at("commit").is_null()) {
    const auto cleared = response(msime_client_command(session_, MSIME_CANCEL));
    result["view"] = cleared.at("view");
  }
  return result;
}
nlohmann::json ServerSession::candidate_action(uint64_t epoch,
                                               uint64_t generation,
                                               size_t index,
                                               CandidateAction action,
                                               uint8_t position) {
  check_active(epoch);
  if (!input_enabled_)
    throw std::logic_error("Candidate action while input disabled");
  char *raw = nullptr;
  switch (action) {
  case CandidateAction::Pin:
    raw = msime_client_pin_candidate(session_, generation, index);
    break;
  case CandidateAction::Remove:
    raw = msime_client_remove_candidate(session_, generation, index);
    break;
  case CandidateAction::FixPosition:
    if (position < 1 || position > 5)
      throw std::invalid_argument("Candidate position outside 1-5");
    raw = msime_client_fix_candidate_position(session_, generation, index,
                                               position);
    break;
  case CandidateAction::ClearPosition:
    raw = msime_client_clear_candidate_position(session_, generation, index);
    break;
  case CandidateAction::Select:
    throw std::invalid_argument("Selection is not a candidate action");
  }
  auto result = response(raw);
  if (!result.at("commit").is_null())
    throw std::logic_error("Candidate action unexpectedly committed text");
  return result;
}
std::optional<std::string> ServerSession::online_query(uint64_t epoch) {
  check_active(epoch);
  try {
    const auto value = response(msime_client_online_query(session_));
    if (value.is_null() || !value.is_object())
      return std::nullopt;
    auto serialized = value.dump();
    if (serialized.empty() || serialized.size() > 16384)
      return std::nullopt;
    return serialized;
  } catch (...) {
    // Cloud candidates are optional. A provider/query serialization failure
    // must not fail the input queue after Engine input has already advanced.
    return std::nullopt;
  }
}
std::optional<std::string> ServerSession::ai_request(uint64_t epoch,
                                                     const std::string &query) {
  check_active(epoch);
  if (query.empty() || query.size() > 16384)
    return std::nullopt;
  try {
    const auto value = response(msime_client_ai_request_for_query(
        session_, reinterpret_cast<const uint8_t *>(query.data()),
        query.size()));
    if (value.is_null() || !value.is_object())
      return std::nullopt;
    const auto serialized = value.dump();
    if (serialized.empty() || serialized.size() > 131072)
      return std::nullopt;
    return serialized;
  } catch (...) {
    // AI is an optional provider. Invalid preferences or credentials must not
    // make an otherwise valid Engine reply fail.
    return std::nullopt;
  }
}
std::optional<nlohmann::json>
ServerSession::rerank_settled(uint64_t epoch) {
  check_active(epoch);
  try {
    const auto value = response(msime_client_rerank_settled(session_));
    // Nothing moved is the common answer and is not a result: redrawing an
    // identical candidate list on every pause is a flicker with no cause the
    // user can see. It is also what an installation with no settled model
    // installed always answers.
    if (!value.is_object() || !value.value("moved", false) ||
        !value.contains("view") || !value.at("view").is_object())
      return std::nullopt;
    return value.at("view");
  } catch (...) {
    // A reranking pass is an improvement, never a reason to stop input.
    return std::nullopt;
  }
}
std::optional<nlohmann::json>
ServerSession::apply_cloud_response(uint64_t epoch, const std::string &query,
                                     const std::string &body) {
  check_active(epoch);
  if (query.empty() || query.size() > 16384 || body.empty() ||
      body.size() > 256 * 1024)
    return std::nullopt;
  try {
    const auto value = response(msime_client_apply_cloud_response(
        session_, reinterpret_cast<const uint8_t *>(query.data()),
        query.size(), reinterpret_cast<const uint8_t *>(body.data()),
        body.size()));
    if (!value.is_object() || !value.value("applied", false) ||
        !value.contains("view") || !value.at("view").is_object())
      return std::nullopt;
    return value.at("view");
  } catch (...) {
    // Malformed/stale provider data is a no-op, never a reason to stop input.
    return std::nullopt;
  }
}
std::optional<nlohmann::json>
ServerSession::apply_ai_candidates(uint64_t epoch, const std::string &query,
                                   const std::string &candidates) {
  check_active(epoch);
  if (query.empty() || query.size() > 16384 || candidates.empty() ||
      candidates.size() > 16384)
    return std::nullopt;
  try {
    // Source 1 is AI, so the candidates land in their own slot rather than
    // displacing the cloud ones.
    const auto value = response(msime_client_apply_online_candidates(
        session_, reinterpret_cast<const uint8_t *>(query.data()), query.size(),
        reinterpret_cast<const uint8_t *>(candidates.data()),
        candidates.size(), 1));
    if (!value.is_object() || !value.value("applied", false) ||
        !value.contains("view") || !value.at("view").is_object())
      return std::nullopt;
    return value.at("view");
  } catch (...) {
    // Optional provider data is a no-op, never a reason to stop input.
    return std::nullopt;
  }
}
std::optional<std::string> ServerSession::translation_query(uint64_t epoch) {
  check_active(epoch);
  try {
    const auto value = response(msime_client_translation_query(session_));
    if (value.is_null() || !value.is_object())
      return std::nullopt;
    auto serialized = value.dump();
    if (serialized.empty() || serialized.size() > 65536)
      return std::nullopt;
    return serialized;
  } catch (...) {
    return std::nullopt;
  }
}
std::optional<nlohmann::json>
ServerSession::apply_translations(uint64_t epoch, uint64_t generation,
                                  const std::string &translations) {
  check_active(epoch);
  if (translations.empty() || translations.size() > 1024 * 1024)
    return std::nullopt;
  try {
    const auto value = response(msime_client_apply_translations(
        session_, generation, reinterpret_cast<const uint8_t *>(translations.data()),
        translations.size()));
    if (!value.is_object() || !value.value("applied", false) ||
        !value.contains("view") || !value.at("view").is_object())
      return std::nullopt;
    return value.at("view");
  } catch (...) {
    return std::nullopt;
  }
}
nlohmann::json ServerSession::update_preferences(uint64_t epoch,
                                                 const std::string &snapshot) {
  check_active(epoch);
  const auto document = nlohmann::json::parse(snapshot);
  const auto next_preferences = document.at("preferences");
  const bool cloud_changed = preferences_.value("cloud_candidates", true) !=
                             next_preferences.value("cloud_candidates", true);
  const auto previous_ai = preferences_.value("ai_assistant", nlohmann::json::object());
  const auto next_ai = next_preferences.value("ai_assistant", nlohmann::json::object());
  const bool ai_changed = previous_ai != next_ai;
  const bool translation_changed =
      translation_preferences_changed(preferences_, next_preferences);
  auto result = response(msime_client_update_preferences(
      session_, reinterpret_cast<const uint8_t *>(snapshot.data()),
      snapshot.size()));
  preferences_ = next_preferences;
  const auto clear_online = [&](uint8_t source) {
    auto cleared = response(msime_client_clear_online_candidates(session_, source));
    if (cleared.contains("view") && cleared.at("view").is_object())
      result["view"] = std::move(cleared.at("view"));
  };
  if (cloud_changed)
    clear_online(0);
  if (ai_changed)
    clear_online(1);
  traditional_output_ = next_preferences.value(
      "traditional_chinese_output", false);
  if (translation_changed && result.contains("view") &&
      result.at("view").is_object() &&
      result.at("view").at("generation").is_number_unsigned()) {
    const auto empty = std::string("[]");
    auto cleared = response(msime_client_apply_translations(
        session_, result.at("view").at("generation").get<uint64_t>(),
        reinterpret_cast<const uint8_t *>(empty.data()), empty.size()));
    if (cleared.contains("view") && cleared.at("view").is_object())
      result["view"] = std::move(cleared.at("view"));
  }
  // With nothing switched on the library starts no player, so it has not kept the earlier "active". Say it again, so music switched on while this client holds the focus starts now rather than at the next focus change.
  if (music_active_)
    (void)msime_client_music_set_active(session_, true);
  refresh_typing_effect_settings();
  return result;
}
nlohmann::json ServerSession::page_candidate(uint64_t epoch, uint64_t session,
                                             uint64_t generation, bool previous,
                                             unsigned steps) {
  check_active(epoch);
  if (!input_enabled_ || session != session_ || !generation || steps == 0 ||
      steps > 9)
    throw std::invalid_argument("Invalid Windows candidate paging request");
  auto current = view();
  if (current.at("session").get<uint64_t>() != session ||
      current.at("generation").get<uint64_t>() != generation ||
      current.at("editing_text").get<std::string>().empty())
    throw std::invalid_argument("Stale Windows candidate paging request");
  const auto command = previous ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE;
  nlohmann::json result = std::move(current);
  for (unsigned step = 0; step < steps; ++step) {
    result = response(msime_client_command(session_, command));
    if (!result.at("commit").is_null())
      throw std::logic_error("Candidate paging unexpectedly committed text");
  }
  return result;
}
nlohmann::json ServerSession::view() const {
  check_thread();
  return response(msime_client_view(session_));
}
bool ServerSession::key_sound(uint32_t key_class) {
  check_thread();
  return msime_client_key_sound(session_, key_class);
}
bool ServerSession::commit_sound() {
  check_thread();
  return msime_client_commit_sound(session_);
}
uint32_t ServerSession::typing_effect(uint32_t event) {
  check_thread();
  return msime_client_typing_effect(session_, event);
}
void ServerSession::refresh_typing_effect_settings() {
  // A settings answer that cannot be read leaves the effect as the preferences alone describe it rather than failing the focus change or the preference update it follows.
  TypingEffectSettings settings;
  try {
    const auto value = response(msime_client_typing_effect_settings(session_));
    const auto intensity = value.value("intensity", 50.0);
    std::optional<uint32_t> duration;
    if (value.contains("duration_ms") && value.at("duration_ms").is_number())
      duration = static_cast<uint32_t>((std::max)(0.0, value.at("duration_ms").get<double>()));
    std::optional<uint32_t> color;
    if (value.contains("colors") && value.at("colors").is_array() && !value.at("colors").empty() &&
        value.at("colors").front().is_string())
      color = typing_effect_rgb(value.at("colors").front().get<std::string>());
    settings = resolve_typing_effect_settings(static_cast<uint32_t>((std::max)(0.0, intensity)), duration, color);
  } catch (...) {
    settings = TypingEffectSettings{};
  }
  typing_effect_settings_ = settings;
}
void ServerSession::set_music_active(bool active) {
  check_thread();
  (void)msime_client_music_set_active(session_, active);
  music_active_ = active;
}
KeyResult ServerSession::punctuation(const FanyImeNamedpipeData &packet,
                                     uint64_t epoch) {
  check_active(epoch);
  const auto action = translate_key(packet);
  if (packet.client_id != client_ ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      !packet.request_id || packet.request_id == FANY_IME_NO_REQUEST_ID ||
      packet.pinyin_length < 0 || packet.pinyin_length >= 128 ||
      action.kind != KeyKind::Character)
    throw std::invalid_argument("Invalid Windows punctuation request");
  if (!input_enabled_)
    return key(packet, epoch);
  // Numpad arithmetic keys, the numpad decimal and '/' finish the highlighted candidate and append their ASCII mark untranslated. Without a composition there is no candidate to finish, and the key keeps its ordinary punctuation.
  const char literal = literal_candidate_punctuation(packet);
  if (literal && !view().at("editing_text").get<std::string>().empty())
    return {client_, epoch_, packet.request_id, true,
            response(msime_client_punctuation_ascii(
                session_, static_cast<uint8_t>(literal)))};
  auto result = response(
      msime_client_punctuation(session_, static_cast<uint8_t>(action.value)));
  return {client_, epoch_, packet.request_id, true, std::move(result)};
}
std::optional<WordCharacterResult>
ServerSession::word_character(const FanyImeNamedpipeData &packet,
                              uint64_t epoch, WordCharacterBinding binding) {
  check_active(epoch);
  const auto edge = word_character_edge(packet, binding);
  if (!edge)
    return std::nullopt;
  if (packet.client_id != client_ || !packet.request_id ||
      packet.request_id == FANY_IME_NO_REQUEST_ID || packet.pinyin_length < 0 ||
      packet.pinyin_length >= 128)
    throw std::invalid_argument("Invalid word-to-character request");
  if (!input_enabled_)
    return std::nullopt;
  const auto current = view();
  // 不论汉字列表是否打开，韩文的 '-'、'='、'[' 和 ']' 都是标点：引擎关闭列表，把韩文连同标点写出，和其他宿主一样，而不是取单个汉字的首尾字。注音、越南文和藏文同样在 TIP 自己的宿主会话里组字，所以也不取首尾字。
  // A key the Engine spells in its current state (V mode's '-') is input, as `edit_kind` routes it; taking it here first would commit the highlighted row instead.
  if (current.at("local_mode") == "unknown" ||
      current.at("editing_text").get<std::string>().empty() ||
      scheme::AlwaysInlinePreedit(static_cast<int>(current.value("scheme", 0u))) ||
      spelled_by_engine(current.value("spelling_symbols", std::string{}),
                        static_cast<uint32_t>(packet.wch)) ||
      !word_character_edge(packet, binding, current.value("scheme", 0u) == 3u))
    return std::nullopt;
  std::string fallback;
  for (const auto &candidate : current.at("candidates")) {
    if (!candidate.at("highlighted").get<bool>())
      continue;
    fallback = candidate.at("text").get<std::string>();
    const auto &id = candidate.at("id");
    auto selected = response(
        msime_client_select_edge(session_, id.at("generation").get<uint64_t>(),
                                 id.at("index").get<size_t>(), *edge));
    if (selected.at("handled").get<bool>())
      return WordCharacterResult{
          {client_, epoch_, packet.request_id, true, std::move(selected)},
          true};
    break;
  }
  if (const auto character = extract_han_character(
          fallback, *edge == MSIME_FIRST_HAN ? HanCharacterEdge::First
                                             : HanCharacterEdge::Last))
    fallback = *character;
  // Legacy Normal delegates smart punctuation to TSF. Do not finish remaining
  // segments or translate punctuation here; only the highlighted text is sent.
  auto cancelled = response(msime_client_command(session_, MSIME_CANCEL));
  if (!cancelled.at("commit").is_null() ||
      !cancelled.at("view").at("editing_text").get<std::string>().empty())
    throw std::logic_error("Engine did not cancel word-to-character fallback");
  cancelled["handled"] = true;
  cancelled["commit"] = fallback;
  return WordCharacterResult{
      {client_, epoch_, packet.request_id, true, std::move(cancelled)}, false};
}
} // namespace msime::windows
