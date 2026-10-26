#pragma once

#include <nlohmann/json.hpp>

#include <optional>
#include <string>
#include <utility>

namespace msime::windows {

// 候选词翻译范围是「仅当前高亮」时，host-api 的 msime_client_translation_query 只把高亮的那一个候选放进查询的 candidates，「显示英文释义」等离线释义开关要查的整页释义另放在 gloss_query 里：一份完整的查询文档，同一代次，覆盖这一页全部候选（含高亮那个），不带任何翻译服务。翻译线程把两份分开、各自照普通查询算（gloss_query 不带服务，不会联网），再用 merge_gloss_answers 拼起来。

// 把 gloss_query 从查询里拿出来：返回那份子查询，查询里随之去掉这个键，剩下的就是只为高亮候选的主查询。没有这个键时返回空、查询不变（范围是「全部」，或高亮不在这一页、查询本身就是那份不带服务的文档）；有这个键但不是对象时同样返回空，键照样去掉。
inline std::optional<nlohmann::json> take_gloss_query(nlohmann::json &query) {
  if (!query.is_object())
    return std::nullopt;
  const auto found = query.find("gloss_query");
  if (found == query.end())
    return std::nullopt;
  auto gloss = std::move(*found);
  query.erase("gloss_query");
  if (!gloss.is_object())
    return std::nullopt;
  return gloss;
}

// 主查询和 gloss_query 各自算出的回答（[{"text","translation"}] 的 JSON，算不出时为空）拼成交给 msime_client_apply_translations 的一个数组：主查询的在前、原样不加标记，gloss_query 的每条标上 "gloss": true。共享运行时只凭这个标记区分两种回答：不带标记的是候选词翻译，只画在高亮行；带标记的是离线释义，每一行都画。主查询没有回答而离线释义有时照样交出离线释义。两边都没有回答时返回空：空数组表示清掉全部释义，不能拿它当「没有回答」交出去。不是数组的回答、数组里不是对象的条目都当作没有。
inline std::optional<std::string> merge_gloss_answers(const std::optional<std::string> &primary,
                                                      const std::optional<std::string> &glosses) {
  auto merged = nlohmann::json::array();
  const auto append = [&](const std::optional<std::string> &answers, bool gloss) {
    if (!answers)
      return;
    auto values = nlohmann::json::parse(*answers, nullptr, false);
    if (!values.is_array())
      return;
    for (auto &value : values) {
      if (!value.is_object())
        continue;
      if (gloss)
        value["gloss"] = true;
      merged.push_back(std::move(value));
    }
  };
  append(primary, false);
  append(glosses, true);
  if (merged.empty())
    return std::nullopt;
  return merged.dump();
}

// 这一页画在候选下面的那些候选：带 gloss_query 时是它覆盖的整页（含高亮那个），否则是查询自己的 candidates。读音按它算，免得「仅当前高亮」时只有高亮的那一行有读音。调用方已经校验过查询自己的 candidates。
inline const nlohmann::json &translation_page_candidates(const nlohmann::json &query) {
  const auto gloss = query.find("gloss_query");
  if (gloss != query.end() && gloss->is_object()) {
    const auto candidates = gloss->find("candidates");
    if (candidates != gloss->end() && candidates->is_array())
      return *candidates;
  }
  return query.at("candidates");
}

} // namespace msime::windows
