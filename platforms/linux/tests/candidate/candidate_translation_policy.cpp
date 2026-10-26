#include "../src/candidates/CandidateTranslationPolicy.h"
#include "../src/candidates/CandidateTranslationScope.h"

#include <cassert>

int main() {
  // 横排预留的译文行数：一种语言一行，两种不同的语言两行，不认识的语言不算，至少一行。
  using msime::linux_host::translation_reserved_lines;
  assert(translation_reserved_lines("en", "") == 1);
  assert(translation_reserved_lines("en", "ja") == 2);
  assert(translation_reserved_lines("en", "en") == 1);
  assert(translation_reserved_lines("xx", "ja") == 1);
  assert(translation_reserved_lines("xx", "yy") == 1);
  using msime::linux_host::split_translation_gloss;
  assert((split_translation_gloss("apple; fruit") ==
          std::vector<std::string>{"apple", "fruit"}));
  assert((split_translation_gloss("苹果；家伙") ==
          std::vector<std::string>{"苹果", "家伙"}));
  assert((split_translation_gloss(" first ; ; second ; ") ==
          std::vector<std::string>{"first", "second"}));
  assert((split_translation_gloss("cloud gloss") ==
          std::vector<std::string>{"cloud gloss"}));
  assert(split_translation_gloss(" ; \xEF\xBC\x9B").empty());

  // A non-English offline dictionary answers first and the user's own
  // translator then outranks it: its gloss replaces the dictionary's in place,
  // a text only it answered is appended, an empty answer changes nothing, and
  // the dictionary keeps what the provider left.
  using msime::linux_host::prefer_online_glosses;
  using Glosses = std::vector<std::pair<std::string, std::string>>;
  Glosses glosses{{"你好", "bonjour"}, {"世界", "monde"}};
  prefer_online_glosses(glosses, Glosses{{"你好", "salut"},
                                         {"再见", "au revoir"},
                                         {"世界", ""}});
  assert((glosses == Glosses{{"你好", "salut"},
                             {"世界", "monde"},
                             {"再见", "au revoir"}}));
  Glosses none;
  prefer_online_glosses(none, Glosses{});
  assert(none.empty());

  // Only the command mode's one-sentence query is /fy's; a gloss request, or the sentence the menu action translates outside that mode, keeps the gloss rules.
  using msime::linux_host::command_translation_query;
  assert(command_translation_query("command", true));
  assert(!command_translation_query("command", false));
  assert(!command_translation_query("none", true));
  assert(!command_translation_query("mention", true));

  // 表情、颜文字、日文混输、插件和连续多语言行只展示原文；
  // 转成只有文字的提供者 socket 协议时仍须保留来源身份。
  using msime::linux_host::candidate_translation_source_allowed;
  assert(candidate_translation_source_allowed(0));
  assert(candidate_translation_source_allowed(4));
  assert(!candidate_translation_source_allowed(6));
  assert(!candidate_translation_source_allowed(7));
  assert(!candidate_translation_source_allowed(13));
  assert(!candidate_translation_source_allowed(14));
  assert(!candidate_translation_source_allowed(18));
  assert(candidate_translation_source_allowed(255));
  assert(candidate_translation_source_allowed(256));

  using msime::linux_host::should_retry_translation_after_provider;
  assert(should_retry_translation_after_provider(true, false, false));
  assert(should_retry_translation_after_provider(true, true, false));
  assert(!should_retry_translation_after_provider(true, true, true));
  assert(!should_retry_translation_after_provider(false, false, false));

  // 「仅当前高亮」：缺键和不认识的值都按仅当前高亮读，只有 "all" 是全部；候选词翻译关着时范围不生效。
  using Json = nlohmann::json;
  using msime::linux_host::translation_scope_highlighted;
  assert(translation_scope_highlighted(true, Json::object()));
  assert(translation_scope_highlighted(true, Json{{"candidate_translation_scope", "highlighted"}}));
  assert(translation_scope_highlighted(true, Json{{"candidate_translation_scope", "unknown"}}));
  assert(translation_scope_highlighted(true, Json{{"candidate_translation_scope", 3}}));
  assert(!translation_scope_highlighted(true, Json{{"candidate_translation_scope", "all"}}));
  assert(!translation_scope_highlighted(false, Json::object()));

  // 查询拆成主查询（只有高亮那一个候选，带服务）和 gloss_query（整页离线释义，不带服务）；主查询里不再留着 gloss_query，免得它跟着发给 provider。
  using msime::linux_host::split_translation_query;
  const Json gloss_document{{"generation", 7}, {"provider", "none"}, {"translation_account", false},
                            {"candidates", Json::array({Json{{"text", "你好"}, {"source", 0}},
                                                        Json{{"text", "拟好"}, {"source", 0}}})}};
  const Json scoped{{"generation", 7}, {"provider", "tencent"}, {"translation_account", false},
                    {"candidates", Json::array({Json{{"text", "拟好"}, {"source", 0}}})},
                    {"gloss_query", gloss_document}};
  const auto parts = split_translation_query(scoped);
  assert(parts.main.is_object() && !parts.main.contains("gloss_query"));
  assert(parts.main.at("candidates").size() == 1 && parts.main.at("provider") == "tencent");
  assert(parts.gloss == gloss_document);
  // 范围是「全部」（没有 gloss_query），或高亮不在这一页、返回的就是那份不带服务的文档：照常当主查询处理。
  auto whole = scoped;
  whole.erase("gloss_query");
  const auto unscoped = split_translation_query(whole);
  assert(unscoped.main == whole && unscoped.gloss.is_null());
  assert(split_translation_query(gloss_document).main == gloss_document);
  assert(split_translation_query(Json()).main.is_null());

  // 只有点名了一家服务（或问账号）的查询才连 provider：gloss_query 和只剩离线释义的那份文档都不连，非高亮候选因此到不了 provider。缺 provider 字段时照旧发，由 provider 按格式错误拒绝。
  using msime::linux_host::translation_query_reaches_service;
  assert(translation_query_reaches_service(parts.main));
  assert(!translation_query_reaches_service(parts.gloss));
  assert(translation_query_reaches_service(Json{{"provider", "none"}, {"translation_account", true}}));
  assert(translation_query_reaches_service(Json{{"provider", "ai"}}));
  assert(translation_query_reaches_service(Json::object()));

  // gloss_query 的回答每条标 "gloss": true，范围生效时 Rust 把它们画在每一行，不当作候选词翻译。
  using msime::linux_host::mark_gloss_answers;
  assert((mark_gloss_answers(Json::array({Json{{"text", "你好"}, {"translation", "hello"}}})) ==
          Json::array({Json{{"text", "你好"}, {"translation", "hello"}, {"gloss", true}}})));
  assert(mark_gloss_answers(Json::array()).empty());

  // 交给 provider 的文字按候选自己带的来源剔除只展示原文的行（查询只有高亮那一个时，不能再按视图的下标去对来源）；离线词典查询带着查询里的候选，不换成视图的整页。
  using msime::linux_host::translation_provider_texts;
  using msime::linux_host::translation_lookup_candidates;
  const Json rows = Json::array({Json{{"text", "你好"}, {"source", 0}}, Json{{"text", "😀"}, {"source", 6}},
                                 Json{{"text", "拟好"}}, Json{{"text", "插件"}, {"source", 14}}});
  assert((translation_provider_texts(rows) == Json::array({"你好", "拟好"})));
  assert((translation_lookup_candidates(rows) ==
          Json::array({Json{{"text", "你好"}, {"source", 0}}, Json{{"text", "😀"}, {"source", 6}},
                       Json{{"text", "拟好"}}, Json{{"text", "插件"}, {"source", 14}}})));
  assert((translation_provider_texts(Json::array({"整句"})) == Json::array({"整句"})));
  using msime::linux_host::translation_candidate_reaches_provider;
  assert(translation_candidate_reaches_provider(rows[0]) && !translation_candidate_reaches_provider(rows[1]));
  assert(translation_candidate_reaches_provider(rows[2]) && !translation_candidate_reaches_provider(rows[3]));
  assert(translation_candidate_reaches_provider(Json{{"text", "未知"}, {"source", -1}}));
  assert(!translation_candidate_reaches_provider(Json::object()));
  assert(translation_lookup_candidates(Json()).empty());

  // 高亮移开再移回来，代次推进了、其余不变：去掉代次的查询就是同一把键，缓存的回答直接交给 Rust，不再问 provider。
  using msime::linux_host::translation_answer_key;
  auto moved_back = parts.main;
  moved_back["generation"] = 11;
  assert(translation_answer_key(moved_back) == translation_answer_key(parts.main));
  auto other = parts.main;
  other["candidates"] = Json::array({Json{{"text", "你好"}, {"source", 0}}});
  assert(translation_answer_key(other) != translation_answer_key(parts.main));
  auto other_service = parts.main;
  other_service["provider"] = "niutrans";
  assert(translation_answer_key(other_service) != translation_answer_key(parts.main));

  using msime::linux_host::TranslationAnswerCache;
  TranslationAnswerCache cache;
  const auto answers = Json::array({Json{{"text", "拟好"}, {"translation", "draft"}}});
  assert(cache.find(translation_answer_key(parts.main)) == nullptr);
  cache.store(translation_answer_key(parts.main), answers);
  assert(cache.find(translation_answer_key(moved_back)) && *cache.find(translation_answer_key(moved_back)) == answers);
  // 没有回答的不记：下一次照常去问（provider 失败时可以重试）。
  cache.store(translation_answer_key(other), Json::array());
  assert(cache.find(translation_answer_key(other)) == nullptr);
  // 有上限：最早记下的先丢。
  for (std::size_t at = 0; at < TranslationAnswerCache::kCapacity; ++at)
    cache.store("synthetic-" + std::to_string(at), answers);
  assert(cache.size() == TranslationAnswerCache::kCapacity);
  assert(cache.find(translation_answer_key(parts.main)) == nullptr);
  assert(cache.find("synthetic-0") && cache.find("synthetic-" + std::to_string(TranslationAnswerCache::kCapacity - 1)));
  cache.clear();
  assert(cache.size() == 0 && cache.find("synthetic-1") == nullptr);
}
