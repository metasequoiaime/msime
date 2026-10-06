#include "../src/voice/VoiceProviderOptions.h"

#include <cassert>
#include <stdexcept>
#include <string>

using Json = nlohmann::json;
using msime::linux_host::voice_provider_options;

int main() {
  // A built-in scheme travels by id alone; the provider carries its text.
  auto options = voice_provider_options(Json{{"voice_input", {{"polish_prompt_id", "cleanup"}, {"polish_prompt_custom_1", "一号"}, {"polish_enabled", true}}}});
  assert(options.at("polish_prompt_id") == "cleanup");
  assert(options.at("polish_enabled") == true);
  assert(!options.contains("polish_prompt_custom_1"));

  // The selected custom slot is still forwarded; the other slots stay home.
  options = voice_provider_options(Json{{"voice_input", {{"polish_prompt_id", "custom_2"}, {"polish_prompt_custom_2", "二号"}, {"polish_prompt_custom_3", "三号"}}}});
  assert(options.at("polish_prompt_custom_2") == "二号");
  assert(!options.contains("polish_prompt_custom_3"));

  // An empty slot is not sent, and the provider falls back to the built-in prompt.
  options = voice_provider_options(Json{{"voice_input", {{"polish_prompt_id", "custom_1"}, {"polish_prompt_custom_1", ""}}}});
  assert(!options.contains("polish_prompt_custom_1"));
  // "custom" names no slot.
  options = voice_provider_options(Json{{"voice_input", {{"polish_prompt_id", "custom"}, {"polish_prompt_custom_1", "一号"}}}});
  assert(!options.contains("polish_prompt_custom_1"));

  // Short strings are cut on a UTF-8 boundary, never through a character.
  std::string long_device(170, 'a');
  for (int i = 0; i < 200; ++i) long_device += "字";
  options = voice_provider_options(Json{{"voice_input", {{"capture_device", long_device}}}});
  const auto device = options.at("capture_device").get<std::string>();
  assert(device.size() <= 512 && device.size() == 170 + 114 * 3);
  assert(Json(device).dump().size() > 0);

  // Unknown auth modes, non-boolean flags and the hold-to-lock gesture flag.
  options = voice_provider_options(Json{{"voice_input", {{"doubao_auth_mode", "other"}, {"sound_enabled", "yes"}, {"hotkey_hold_space_lock", true}}}});
  assert(!options.contains("doubao_auth_mode"));
  assert(!options.contains("sound_enabled"));
  assert(options.at("hotkey_hold_space_lock") == true);

  // An oversized prompt is refused rather than truncated.
  bool refused = false;
  try {
    voice_provider_options(Json{{"voice_input", {{"polish_prompt_id", "custom_2"}, {"polish_prompt_custom_2", std::string(8193, 'x')}}}});
  } catch (const std::runtime_error &) {
    refused = true;
  }
  assert(refused);
  assert(voice_provider_options(Json::object()).empty());

  // The on-device model path is forwarded whole, past the 512-byte cut of the short strings, and refused rather than truncated beyond 4 KiB.
  const std::string model_path = "/home/user/.local/share/msime-client/voice-models/" + std::string(600, 'm');
  options = voice_provider_options(Json{{"voice_input", {{"asr_provider", "local"}, {"asr_model_path", model_path}}}});
  assert(options.at("asr_model_path") == model_path);
  assert(msime::linux_host::voice_wants_hotwords(options));
  assert(!voice_provider_options(Json{{"voice_input", {{"asr_model_path", ""}}}}).contains("asr_model_path"));
  assert(!msime::linux_host::voice_wants_hotwords(voice_provider_options(Json{{"voice_input", {{"asr_provider", "doubao"}}}})));
  refused = false;
  try {
    voice_provider_options(Json{{"voice_input", {{"asr_model_path", "/" + std::string(4096, 'x')}}}});
  } catch (const std::runtime_error &) {
    refused = true;
  }
  assert(refused);

  // Hotwords travel as one string option, heaviest first, skipping words that would break the packing.
  auto query = Json{{"language", "zh-cn"}, {"generation", 7}, {"options", {{"asr_provider", "local"}}}};
  msime::linux_host::add_voice_hotwords(query, Json::array({{{"text", "水杉"}, {"pinyin", "shui shan"}},
                                                            {{"text", "坏\t词"}, {"pinyin", "huai ci"}},
                                                            {{"pinyin", "no text"}},
                                                            {{"text", "输入法"}, {"pinyin", "shu ru fa"}}}));
  assert(query.at("options").at("voice_hotwords") == "水杉\tshui shan\n输入法\tshu ru fa");
  query = Json{{"options", Json::object()}};
  msime::linux_host::add_voice_hotwords(query, Json::array());
  assert(!query.at("options").contains("voice_hotwords"));

  // The whole query stays within the limit: words that would push it over are dropped, never cut.
  Json many = Json::array();
  for (int i = 0; i < 2000; ++i) many.push_back({{"text", "词语" + std::to_string(i)}, {"pinyin", "ci yu"}});
  query = Json{{"language", "zh-cn"}, {"generation", 7}, {"options", {{"asr_provider", "local"}}}};
  msime::linux_host::add_voice_hotwords(query, many);
  const auto packed = query.at("options").at("voice_hotwords").get<std::string>();
  assert(query.dump().size() <= 15872);
  assert(query.dump().size() > 15872 - 64);
  assert(packed.rfind("词语0\tci yu\n", 0) == 0);
  assert(packed.back() == 'u');
  return 0;
}
