#pragma once
#include <nlohmann/json.hpp>
#include <algorithm>
#include <filesystem>
#include <optional>
#include <string>
#include <system_error>
#include <vector>

namespace msime::windows {
struct CandidateLayoutSettings {
  bool horizontal = false;
  bool show_preedit = true;
  // The shared `wubi_code_hint`, which is on when the document does not say.
  bool wubi_code_hint = true;
  // 共享偏好 `show_app_logo`：首行左端画不画水杉 logo。新装默认关，文档里没有这个键时也按关处理，和 macOS 一致。
  bool show_app_logo = false;
  // 横排候选为释义预留几行（0 到 2）：按有释义来源的目标语言预留，最多两行，见 candidate_reserved_gloss_lines；行高的算法和 macOS 的 reservedGlossHeightForFont 一样。
  unsigned reserved_gloss_lines = 0;
  // The switches travel in one atomic value between monitor and UI threads.
  constexpr unsigned encode() const {
    return (horizontal ? 1u : 0u) | (show_preedit ? 2u : 0u) | (wubi_code_hint ? 4u : 0u) |
           (show_app_logo ? 8u : 0u) | ((reserved_gloss_lines > 2u ? 2u : reserved_gloss_lines) << 4);
  }
  static constexpr CandidateLayoutSettings decode(unsigned value) {
    return {(value & 1u) != 0, (value & 2u) != 0, (value & 4u) != 0, (value & 8u) != 0,
            (value >> 4) & 3u};
  }
};

// 翻译服务的凭据能不能用，和 client-core 的 translation::usable_credential 一样：去掉首尾空白后不为空，不是「<...>」这样的占位，也不是 FAKESECRET_ 开头的假值。
inline bool candidate_usable_credential(const nlohmann::json &service, const char *key) {
  const auto found = service.find(key);
  if (found == service.end() || !found->is_string())
    return false;
  const auto &value = found->get_ref<const std::string &>();
  const auto first = value.find_first_not_of(" \t\r\n");
  if (first == std::string::npos)
    return false;
  const auto trimmed = value.substr(first, value.find_last_not_of(" \t\r\n") - first + 1);
  return !(trimmed.front() == '<' && trimmed.back() == '>') && trimmed.rfind("FAKESECRET_", 0) != 0;
}

// 打开候选翻译时有没有一个能回答的在线服务，和 host-api selected_translation_services 的判据一样：小牛翻译开着且两项凭据可用，或者自定义翻译开着且填了地址，或者（这两个都没开时）选了水杉账号，或者腾讯翻译开着且两项密钥可用（腾讯默认是开的，没有密钥不算）。
inline bool candidate_online_gloss_service(const nlohmann::json &preferences) {
  const auto flag = [&](const char *key) {
    const auto found = preferences.find(key);
    return found != preferences.end() && found->is_boolean() && found->get<bool>();
  };
  const auto service = [&](const char *key) {
    const auto found = preferences.find(key);
    return found != preferences.end() && found->is_object() ? *found : nlohmann::json::object();
  };
  const auto enabled = [](const nlohmann::json &value) {
    const auto found = value.find("enabled");
    return found != value.end() && found->is_boolean() && found->get<bool>();
  };
  // 小牛和自定义开着时选的就是它们，配置不全也不会退回水杉账号或腾讯。
  const auto niutrans = service("niutrans");
  if (enabled(niutrans))
    return candidate_usable_credential(niutrans, "app_id") && candidate_usable_credential(niutrans, "apikey");
  const auto custom = service("custom_translation");
  if (enabled(custom)) {
    const auto endpoint = custom.find("endpoint");
    return endpoint != custom.end() && endpoint->is_string() && !endpoint->get_ref<const std::string &>().empty();
  }
  if (flag("translation_account"))
    return true;
  const auto tencent = service("tencent_tmt");
  return enabled(tencent) && candidate_usable_credential(tencent, "secret_id") &&
         candidate_usable_credential(tencent, "secret_key");
}

// 装在 resources 旁边的非英文离线释义词典（offline-glosses/zh-<语言>.db）有哪些语言，语言表和 host-api 的 OFFLINE_GLOSS_LANGUAGES、offline_glosses_beside 相同。Server 启动时查一次；下载的资源包不在这里看，只装了资源包的语言不预留，释义到了卡片照旧变高。
inline std::vector<std::string> installed_offline_gloss_languages(const std::filesystem::path &resources) {
  std::vector<std::string> languages;
  std::error_code error;
  const auto directory = resources.parent_path() / "offline-glosses";
  for (const char *language : {"fr", "ja", "es", "ru", "de", "ko"})
    if (std::filesystem::is_regular_file(directory / (std::string("zh-") + language + ".db"), error))
      languages.emplace_back(language);
  return languages;
}

// 释义要占几行：每种目标语言一行，最多两行，但只算有释义来源的语言，算到最后一种有来源的为止（第一种没有来源、第二种有时第一行留空，和 join_translation_lines 一样）。目标语言的取法和 macOS 的 MSIMETranslationTargetsFromPreferences 一样：第一种缺值时按英语，只认支持的七种语言，第二种语言不支持、没有设置或与第一种相同时不算。macOS 打开候选翻译就预留，因为默认的本机翻译总能补上；Windows 没有本机翻译（docs/windows-parity.md），一种语言有来源指：打开候选翻译且有能回答的在线服务（candidate_online_gloss_service），或者英文目标打开了离线英文释义，或者候选翻译、离线英文释义打开其一且这种语言的离线释义词典装在 resources 旁边（offline_languages，Server 启动时查一次）。都没有时不预留，横排卡片不会在每个候选下面挂一行永远空着的释义。
inline unsigned candidate_reserved_gloss_lines(const nlohmann::json &preferences,
                                               const std::vector<std::string> &offline_languages = {}) {
  const auto flag = [&](const char *key) {
    const auto found = preferences.find(key);
    return found != preferences.end() && found->is_boolean() && found->get<bool>();
  };
  const bool translations = flag("candidate_translations");
  const bool english_gloss = flag("candidate_english_gloss");
  if (!translations && !english_gloss)
    return 0;
  const auto supported = [](const std::string &language) {
    for (const char *known : {"en", "fr", "ja", "es", "ru", "de", "ko"})
      if (language == known)
        return true;
    return false;
  };
  const auto primary = preferences.find("translation_target_language");
  const std::string first =
      primary != preferences.end() && primary->is_string() ? primary->get<std::string>() : "en";
  std::vector<std::string> targets;
  if (supported(first)) {
    targets.push_back(first);
    const auto secondary = preferences.find("translation_secondary_language");
    if (secondary != preferences.end() && secondary->is_string() &&
        supported(secondary->get<std::string>()) && secondary->get<std::string>() != first)
      targets.push_back(secondary->get<std::string>());
  }
  const bool online = translations && candidate_online_gloss_service(preferences);
  unsigned lines = 0;
  for (size_t index = 0; index < targets.size(); ++index) {
    const auto &language = targets[index];
    const bool offline =
        std::find(offline_languages.begin(), offline_languages.end(), language) != offline_languages.end();
    if (online || (language == "en" && english_gloss) || (language != "en" && offline))
      lines = static_cast<unsigned>(index + 1);
  }
  return lines;
}

// offline_languages：装在 resources 旁边的非英文离线释义词典的语言，见 candidate_reserved_gloss_lines。
inline std::optional<CandidateLayoutSettings>
candidate_layout_settings(const nlohmann::json &preferences,
                          const std::vector<std::string> &offline_languages = {}) {
  try {
    if (!preferences.is_object())
      return std::nullopt;
    const auto layout =
        preferences.value("candidate_layout", std::string("vertical"));
    const auto preedit =
        preferences.value("candidate_preedit_style", std::string("pinyin"));
    if ((layout != "horizontal" && layout != "vertical") ||
        (preedit != "pinyin" && preedit != "empty"))
      return std::nullopt;
    const auto hint = preferences.find("wubi_code_hint");
    if (hint != preferences.end() && !hint->is_boolean() && !hint->is_null())
      return std::nullopt;
    const auto logo = preferences.find("show_app_logo");
    if (logo != preferences.end() && !logo->is_boolean() && !logo->is_null())
      return std::nullopt;
    return CandidateLayoutSettings{layout == "horizontal", preedit != "empty",
                                   hint == preferences.end() || hint->is_null() || hint->get<bool>(),
                                   logo != preferences.end() && logo->is_boolean() && logo->get<bool>(),
                                   candidate_reserved_gloss_lines(preferences, offline_languages)};
  } catch (...) {
    return std::nullopt;
  }
}
} // namespace msime::windows
