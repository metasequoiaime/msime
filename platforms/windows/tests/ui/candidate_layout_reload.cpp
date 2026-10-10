#include "../../src/candidate/CandidateCardSize.h"
#include "../../src/candidate/CandidateLayoutSettings.h"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

int main() {
  using namespace msime::windows;
  const auto defaults = candidate_layout_settings(nlohmann::json::object());
  assert(defaults && !defaults->horizontal && defaults->show_preedit);
  // The shared `wubi_code_hint` is on when absent or null, and must survive the atomic encoding.
  assert(defaults->wubi_code_hint);
  assert(candidate_layout_settings({{"wubi_code_hint", nullptr}})->wubi_code_hint);
  const auto hint_off = candidate_layout_settings({{"wubi_code_hint", false}});
  assert(hint_off && !hint_off->wubi_code_hint);
  assert(!CandidateLayoutSettings::decode(hint_off->encode()).wubi_code_hint);
  assert(CandidateLayoutSettings::decode(defaults->encode()).wubi_code_hint);
  assert(!candidate_layout_settings({{"wubi_code_hint", "false"}}));
  // 共享的 `show_app_logo` 缺值或为 null 时按新装处理，不画 logo；打开后要经得起原子量编码。
  assert(!defaults->show_app_logo);
  assert(!candidate_layout_settings({{"show_app_logo", nullptr}})->show_app_logo);
  const auto logo_on = candidate_layout_settings({{"show_app_logo", true}});
  assert(logo_on && logo_on->show_app_logo);
  assert(CandidateLayoutSettings::decode(logo_on->encode()).show_app_logo);
  assert(!CandidateLayoutSettings::decode(defaults->encode()).show_app_logo);
  assert(!candidate_layout_settings({{"show_app_logo", "yes"}}));
  // 释义预留行数：翻译和离线英文释义都关时为 0；打开任一个时按有释义来源的目标语言算，第二种语言为空或与第一种相同时只算一行。
  assert(defaults->reserved_gloss_lines == 0);
  assert(candidate_reserved_gloss_lines({{"candidate_english_gloss", true},
                                         {"translation_target_language", "en"}}) == 1);
  // 新装的默认偏好：候选翻译开着，腾讯翻译默认开着但没有密钥，离线英文释义关着。Windows 没有本机翻译补位，这一行永远等不到释义，所以不预留。
  const nlohmann::json tencent_without_keys{{"enabled", true}, {"secret_id", ""}, {"secret_key", ""}};
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"candidate_english_gloss", false},
                                         {"translation_target_language", "en"},
                                         {"tencent_tmt", tencent_without_keys}}) == 0);
  // 占位的、假的密钥也不算。
  assert(candidate_reserved_gloss_lines(
             {{"candidate_translations", true},
              {"tencent_tmt", {{"enabled", true}, {"secret_id", "<id>"}, {"secret_key", "FAKESECRET_x"}}}}) == 0);
  // 有能回答的在线服务时每种目标语言都有来源。
  const nlohmann::json tencent{{"enabled", true}, {"secret_id", "AKIDsynthetic"}, {"secret_key", "synthetic"}};
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_target_language", "en"},
                                         {"translation_secondary_language", nullptr},
                                         {"tencent_tmt", tencent}}) == 1);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_target_language", "en"},
                                         {"translation_secondary_language", "en"},
                                         {"tencent_tmt", tencent}}) == 1);
  const nlohmann::json two_targets{{"candidate_translations", true},
                                   {"translation_target_language", "en"},
                                   {"translation_secondary_language", "ja"},
                                   {"tencent_tmt", tencent}};
  const auto two = candidate_layout_settings(two_targets);
  assert(two && two->reserved_gloss_lines == 2);
  assert(CandidateLayoutSettings::decode(two->encode()).reserved_gloss_lines == 2);
  assert(CandidateLayoutSettings::decode(two->encode()).wubi_code_hint);
  // 选了水杉账号、小牛或自定义翻译也算；小牛开着但凭据不全时选的就是它，不会退回水杉账号。
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true}, {"translation_account", true}}) == 1);
  assert(candidate_reserved_gloss_lines(
             {{"candidate_translations", true},
              {"niutrans", {{"enabled", true}, {"app_id", "app"}, {"apikey", "key"}}}}) == 1);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_account", true},
                                         {"niutrans", {{"enabled", true}, {"app_id", ""}, {"apikey", ""}}}}) == 0);
  assert(candidate_reserved_gloss_lines(
             {{"candidate_translations", true},
              {"custom_translation", {{"enabled", true}, {"endpoint", "https://example.test"}}}}) == 1);
  assert(candidate_reserved_gloss_lines(
             {{"candidate_translations", true}, {"custom_translation", {{"enabled", true}, {"endpoint", ""}}}}) == 0);
  // 在线服务只在候选翻译开着时问。
  assert(candidate_reserved_gloss_lines({{"candidate_translations", false},
                                         {"candidate_english_gloss", true},
                                         {"translation_target_language", "ja"},
                                         {"tencent_tmt", tencent}}) == 0);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", false},
                                         {"translation_secondary_language", "ja"}}) == 0);
  // 第一种语言缺值按英语；不支持的第二种语言不占行，和 macOS 取目标语言的规则相同。
  assert(candidate_reserved_gloss_lines({{"candidate_english_gloss", true},
                                         {"translation_secondary_language", "en"}}) == 1);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_target_language", "en"},
                                         {"translation_secondary_language", "zh"},
                                         {"tencent_tmt", tencent}}) == 1);
  // 非英文目标要装了它的离线释义词典才算；第二种语言有来源而第一种没有时第一行留空，照样占两行。
  assert(candidate_reserved_gloss_lines({{"candidate_english_gloss", true},
                                         {"translation_secondary_language", "fr"}}) == 1);
  assert(candidate_reserved_gloss_lines({{"candidate_english_gloss", true},
                                         {"translation_secondary_language", "fr"}},
                                        {"fr"}) == 2);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_target_language", "ja"},
                                         {"translation_secondary_language", "fr"}},
                                        {"fr"}) == 2);
  assert(candidate_reserved_gloss_lines({{"candidate_translations", true},
                                         {"translation_target_language", "ja"}},
                                        {"fr"}) == 0);
  const auto offline = candidate_layout_settings({{"candidate_translations", true},
                                                  {"translation_target_language", "fr"}},
                                                 {"fr"});
  assert(offline && offline->reserved_gloss_lines == 1);
  // 离线释义词典装在 resources 的同级目录 offline-glosses 里，英文不算（它随包在 resources 里）。
  {
    const auto root = std::filesystem::temp_directory_path() / "msime-offline-gloss-languages-test";
    std::filesystem::remove_all(root);
    std::filesystem::create_directories(root / "resources");
    std::filesystem::create_directories(root / "offline-glosses" / "zh-de.db");
    assert(installed_offline_gloss_languages(root / "resources").empty());
    for (const char *name : {"zh-fr.db", "zh-ko.db", "zh-en.db"})
      std::ofstream(root / "offline-glosses" / name) << "x";
    assert(installed_offline_gloss_languages(root / "resources") == (std::vector<std::string>{"fr", "ko"}));
    std::filesystem::remove_all(root);
  }
  for (bool horizontal : {false, true}) {
    for (bool preedit : {false, true}) {
      auto settings = candidate_layout_settings(
          {{"candidate_layout", horizontal ? "horizontal" : "vertical"},
           {"candidate_preedit_style", preedit ? "pinyin" : "empty"}});
      assert(settings);
      auto decoded = CandidateLayoutSettings::decode(settings->encode());
      assert(decoded.horizontal == horizontal &&
             decoded.show_preedit == preedit);
    }
  }
  for (auto invalid : {nlohmann::json{{"candidate_layout", "diagonal"}},
                       nlohmann::json{{"candidate_layout", 2}},
                       nlohmann::json{{"candidate_preedit_style", "raw"}},
                       nlohmann::json{{"candidate_preedit_style", nullptr}},
                       nlohmann::json::array()})
    assert(!candidate_layout_settings(invalid));
  CandidateCardInput input;
  input.items = {{90}, {90}, {90}};
  input.max_width = 1000;
  input.max_height = 1000;
  input.preedit_visible = true;
  input.horizontal = false;
  const auto vertical = candidate_card_size(input);
  input.horizontal = true;
  const auto horizontal = candidate_card_size(input);
  assert(horizontal.width > vertical.width);
  assert(horizontal.height < vertical.height);
  input.preedit_visible = false;
  const auto hidden_preedit = candidate_card_size(input);
  assert(hidden_preedit.height < horizontal.height);
}
