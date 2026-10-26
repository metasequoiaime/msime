#pragma once

// 候选词翻译的「翻译范围」在 Linux 两个宿主（IBus、Fcitx5）请求侧共用的规则。共享层 msime_client_translation_query 已经按范围组好查询：「仅当前高亮」时主查询只有高亮的那一个候选，「显示英文释义」要的整页离线释义另成一份不带服务的 gloss_query（见 crates/host-api/include/msime_client.h）。宿主这里只拆开它、用同一条路径算两份、给 gloss_query 的回答打标记，再记住主查询的回答，高亮移回来时不再问 provider。

#include "CandidateTranslationPolicy.h"

#include <cstddef>
#include <cstdint>
#include <deque>
#include <nlohmann/json.hpp>
#include <string>
#include <utility>

namespace msime::linux_host {

// 候选词翻译的范围是不是「仅当前高亮」：只在候选词翻译开着时生效；偏好 candidate_translation_scope 缺键或不认识的值都按「仅当前高亮」读，只有 "all" 是全部，与共享层 CandidateTranslationScope 的读法相同。
inline bool translation_scope_highlighted(bool candidate_translations, const nlohmann::json &preferences) {
  if (!candidate_translations) return false;
  if (!preferences.is_object()) return true;
  const auto scope = preferences.find("candidate_translation_scope");
  return scope == preferences.end() || !scope->is_string() || scope->get_ref<const std::string &>() != "all";
}

// 一份翻译查询拆成的两部分：main 是候选词翻译的主查询（范围「全部」时就是原文档；「仅当前高亮」时只有高亮的那一个候选；高亮不在这一页、只剩离线释义时是那份不带服务的文档本身），gloss 是 gloss_query（整页离线释义，不带服务），没有时为 null。
struct TranslationQueryParts {
  nlohmann::json main;
  nlohmann::json gloss;
};

// 拆开共享查询：主查询里不再留着 gloss_query，免得它跟着主查询发给 provider。
inline TranslationQueryParts split_translation_query(nlohmann::json query) {
  TranslationQueryParts parts;
  if (!query.is_object()) return parts;
  if (const auto gloss = query.find("gloss_query"); gloss != query.end()) {
    if (gloss->is_object()) parts.gloss = *gloss;
    query.erase(gloss);
  }
  parts.main = std::move(query);
  return parts;
}

// 查询是否点名了一家翻译服务：provider 为 "none" 且不问账号时 provider 进程本来就什么也不发，宿主就不去连它。gloss_query 和只剩离线释义的那份文档都是这样，非高亮候选因此不会到 provider。缺 provider 字段或它不是字符串时照旧发，由 provider 按格式错误拒绝。
inline bool translation_query_reaches_service(const nlohmann::json &query) {
  if (!query.is_object()) return false;
  const auto provider = query.find("provider");
  if (provider == query.end() || !provider->is_string() ||
      provider->get_ref<const std::string &>() != "none")
    return true;
  const auto account = query.find("translation_account");
  return account != query.end() && account->is_boolean() && account->get<bool>();
}

// gloss_query 的回答每条标上 "gloss": true：范围生效时 Rust 把它们当作不受范围限制的离线释义画在每一行，不当作候选词翻译。
inline nlohmann::json mark_gloss_answers(nlohmann::json answers) {
  if (!answers.is_array()) return nlohmann::json::array();
  for (auto &answer : answers)
    if (answer.is_object()) answer["gloss"] = true;
  return answers;
}

// 交给 provider 的候选文字。共享查询里每个候选带着自己的来源，只展示原文的来源（candidate_translation_source_allowed）在只认文字的 provider 协议丢掉来源之前剔除；「仅当前高亮」时查询只有高亮那一个候选，不能再按视图的下标去对来源。没有来源或来源不是非负整数的候选照旧可以请求；已经是文字的候选（整句翻译）原样保留。
inline bool translation_candidate_reaches_provider(const nlohmann::json &candidate) {
  if (candidate.is_string()) return true;
  if (!candidate.is_object() || !candidate.contains("text")) return false;
  const auto source = candidate.find("source");
  return source == candidate.end() || !source->is_number_integer() || source->get<std::int64_t>() < 0 ||
         candidate_translation_source_allowed(source->get<std::uint64_t>());
}
inline nlohmann::json translation_provider_texts(const nlohmann::json &candidates) {
  auto texts = nlohmann::json::array();
  if (!candidates.is_array()) return texts;
  for (const auto &candidate : candidates)
    if (translation_candidate_reaches_provider(candidate))
      texts.push_back(candidate.is_string() ? candidate : candidate.at("text"));
  return texts;
}

// 离线词典查询（msime_client_candidate_gloss_request）用的候选：查询里的候选原样带上文字和来源。主查询「仅当前高亮」时只有高亮那一个，不能换成视图的整页，否则只为候选词翻译查的离线词典会替整页作答。
inline nlohmann::json translation_lookup_candidates(const nlohmann::json &candidates) {
  auto lookup = nlohmann::json::array();
  if (!candidates.is_array()) return lookup;
  for (const auto &candidate : candidates) {
    if (!candidate.is_object() || !candidate.contains("text")) continue;
    nlohmann::json row{{"text", candidate.at("text")}};
    if (const auto source = candidate.find("source"); source != candidate.end()) row["source"] = *source;
    lookup.push_back(std::move(row));
  }
  return lookup;
}

// 主查询回答的缓存键：去掉代次的查询文档。移动高亮会推进代次，高亮移开再移回来时其余部分（候选、目标语言、服务和凭据）不变，就是同一把键。
inline std::string translation_answer_key(nlohmann::json query) {
  if (query.is_object()) query.erase("generation");
  return query.dump();
}

// 「仅当前高亮」时主查询的回答，按 translation_answer_key 记。高亮移回一个问过的候选时宿主把记下的回答直接交给 msime_client_apply_translations，不再问 provider（AI 翻译在 provider 里没有缓存，每问一次都要联网）。只记有回答的，失败的下一次照常重试；有上限，最早记下的先丢；偏好、服务或 socket 变化时宿主整个清掉。
class TranslationAnswerCache {
public:
  static constexpr std::size_t kCapacity = 32;
  const nlohmann::json *find(const std::string &key) const {
    for (const auto &entry : entries_)
      if (entry.first == key) return &entry.second;
    return nullptr;
  }
  void store(std::string key, nlohmann::json answers) {
    if (!answers.is_array() || answers.empty()) return;
    for (auto at = entries_.begin(); at != entries_.end(); ++at)
      if (at->first == key) {
        entries_.erase(at);
        break;
      }
    if (entries_.size() >= kCapacity) entries_.pop_front();
    entries_.emplace_back(std::move(key), std::move(answers));
  }
  void clear() { entries_.clear(); }
  std::size_t size() const { return entries_.size(); }

private:
  std::deque<std::pair<std::string, nlohmann::json>> entries_;
};

} // namespace msime::linux_host
