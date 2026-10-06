#pragma once

#include <cstddef>
#include <stdexcept>
#include <string>
#include <string_view>

#include <nlohmann/json.hpp>

namespace msime::linux_host {

// The voice preferences a host forwards to the voice provider with each request. Both Linux hosts send the same set: the Fcitx5 host once kept its own list and dropped the prompt text, so custom polishing schemes silently fell back to the cleanup prompt there.
//
// A built-in scheme is named by `polish_prompt_id` and the provider carries its text; a custom scheme also sends the selected slot's text, and the provider falls back to the built-in cleanup prompt when that slot is empty. A prompt over 8 KiB is refused rather than cut, since a truncated instruction would change what polishing does.
inline nlohmann::json voice_provider_options(const nlohmann::json &preferences) {
  const auto voice = preferences.value("voice_input", nlohmann::json::object());
  nlohmann::json options = nlohmann::json::object();
  constexpr const char *boolean_keys[] = {
      "sound_enabled", "start_sound", "end_sound", "mute_system_audio",
      "polish_enabled", "polish_text", "doubao_enable_itn",
      "doubao_enable_punc", "doubao_enable_ddc", "stream_inline_preedit",
      "hotkey_hold_space_lock"};
  for (const auto *key : boolean_keys) {
    if (voice.contains(key) && voice.at(key).is_boolean())
      options[key] = voice.at(key);
  }
  constexpr const char *string_keys[] = {
      "capture_backend", "capture_device", "commit_mode", "asr_provider", "asr_model",
      "asr_resource_id", "doubao_auth_mode", "polish_provider", "polish_model",
      "doubao_boosting_table_id", "polish_prompt_id"};
  for (const auto *key : string_keys) {
    if (!voice.contains(key) || !voice.at(key).is_string())
      continue;
    auto value = voice.at(key).get<std::string>();
    if (std::string_view(key) == "doubao_auth_mode" && value != "api_key" && value != "legacy")
      continue;
    if (value.size() > 512) {
      std::size_t end = 512;
      while (end && (static_cast<unsigned char>(value[end]) & 0xc0) == 0x80) --end;
      value.resize(end);
    }
    options[key] = std::move(value);
  }
  const auto prompt_text = [&](const char *key) {
    if (!voice.contains(key) || !voice.at(key).is_string()) return std::string{};
    auto prompt = voice.at(key).get<std::string>();
    if (prompt.size() > 8192) throw std::runtime_error("Voice prompt exceeds limit");
    return prompt;
  };
  // The installed model directory the `local` provider recognises with. A path is refused rather than cut: a truncated one names another directory, or none.
  if (voice.contains("asr_model_path") && voice.at("asr_model_path").is_string()) {
    auto path = voice.at("asr_model_path").get<std::string>();
    if (path.size() > 4096) throw std::runtime_error("Voice model path exceeds limit");
    if (!path.empty()) options["asr_model_path"] = std::move(path);
  }
  const auto preset = voice.value("polish_prompt_id", std::string{"cleanup"});
  const char *slot = nullptr;
  if (preset == "custom_1") slot = "polish_prompt_custom_1";
  else if (preset == "custom_2") slot = "polish_prompt_custom_2";
  else if (preset == "custom_3") slot = "polish_prompt_custom_3";
  if (slot) {
    if (auto prompt = prompt_text(slot); !prompt.empty()) options[slot] = std::move(prompt);
  }
  return options;
}

// Whether a recording should carry the user's dictionary words as hotwords: only on-device recognition takes them.
inline bool voice_wants_hotwords(const nlohmann::json &options) {
  return options.value("asr_provider", std::string{}) == "local";
}

// Pack the `hotwords` array msime_client_voice_hotwords returns into the `voice_hotwords` option, one `text<TAB>pinyin` line per word, heaviest first as given. The provider takes only boolean and string options, and the whole query the host sends must stay within `query_limit` bytes (the provider request is capped at 16 KiB), so words are added while `query` with the option still fits; the rest are dropped. The default leaves 512 bytes for the envelope the provider client wraps around the query. Words carrying a tab or a line break would break the packing and are skipped.
inline void add_voice_hotwords(nlohmann::json &query, const nlohmann::json &hotwords, std::size_t query_limit = 15872) {
  if (!hotwords.is_array() || hotwords.empty() || !query.contains("options") || !query.at("options").is_object())
    return;
  const auto base = query.dump().size();
  // The key and its quotes, the colon and the comma the option adds to the options object.
  std::size_t size = base + std::string_view("\"voice_hotwords\":\"\",").size();
  std::string packed;
  for (const auto &word : hotwords) {
    if (!word.is_object() || !word.contains("text") || !word.at("text").is_string()) continue;
    const auto text = word.at("text").get<std::string>();
    const auto pinyin = word.contains("pinyin") && word.at("pinyin").is_string() ? word.at("pinyin").get<std::string>() : std::string{};
    if (text.empty() || text.find_first_of("\t\r\n") != std::string::npos || pinyin.find_first_of("\t\r\n") != std::string::npos)
      continue;
    auto line = (packed.empty() ? std::string{} : std::string{"\n"}) + text + "\t" + pinyin;
    // The encoded size of the line inside a JSON string: nlohmann writes UTF-8 unescaped and the tab and newline as two-character escapes.
    const auto encoded = nlohmann::json(line).dump().size() - 2;
    if (size + encoded > query_limit) break;
    size += encoded;
    packed += line;
  }
  if (!packed.empty()) query["options"]["voice_hotwords"] = std::move(packed);
}

}  // namespace msime::linux_host
