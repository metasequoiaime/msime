// 「仅当前高亮」的翻译查询带着 gloss_query：主查询只有高亮的候选，整页的离线释义另成一份不带服务的子查询。翻译线程把两份分开算，再拼成交给 msime_client_apply_translations 的一个数组，子查询的每条回答标上 "gloss": true——共享运行时只认这个标记来区分「每一行都画的离线释义」和「只画在高亮行的候选词翻译」，标错或漏标，其他行的离线释义就会消失或者译文画到别的行上。
#include "TranslationGlossQuery.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("translation gloss query failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

nlohmann::json candidates(std::initializer_list<const char *> texts) {
  auto values = nlohmann::json::array();
  for (const auto *text : texts)
    values.push_back({{"text", text}, {"source", 0}, {"online_gloss", false}});
  return values;
}
} // namespace

int main() {
  try {
    // 子查询从主查询里拿出来：主查询不再带这个键（递归处理时不会再算一遍），子查询原样交回。
    {
      auto query = nlohmann::json{{"generation", 7},
                                  {"target_language", "en"},
                                  {"candidates", candidates({"你好"})},
                                  {"provider", "tencent"}};
      query["gloss_query"] = nlohmann::json{{"generation", 7},
                                            {"target_language", "en"},
                                            {"candidates", candidates({"你好", "拟好", "你号"})},
                                            {"provider", "none"},
                                            {"translation_account", false}};
      const auto gloss = take_gloss_query(query);
      REQUIRE(gloss.has_value());
      REQUIRE(!query.contains("gloss_query"));
      REQUIRE(query.at("candidates").size() == 1);
      REQUIRE(gloss->at("candidates").size() == 3);
      REQUIRE(gloss->at("provider") == "none");
      // 第二次拿不到：处理过的主查询照普通查询走。
      REQUIRE(!take_gloss_query(query).has_value());
    }
    // 没有 gloss_query（范围是「全部」，或高亮不在这一页时的那份不带服务的文档）：查询不变，照普通查询处理。
    {
      auto query = nlohmann::json{{"generation", 7}, {"candidates", candidates({"你好"})}};
      const auto before = query;
      REQUIRE(!take_gloss_query(query).has_value());
      REQUIRE(query == before);
    }
    // 不是对象的 gloss_query 不算数，也不留在主查询里。
    {
      auto query = nlohmann::json{{"generation", 7}, {"gloss_query", "bad"}};
      REQUIRE(!take_gloss_query(query).has_value());
      REQUIRE(!query.contains("gloss_query"));
    }

    // 两边都有回答：主查询的在前，不带标记；子查询的每条都标上 gloss。高亮那个候选两边都可能答，两条都交给共享层，由它决定高亮行先画哪条。
    {
      const auto merged = merge_gloss_answers(
          std::string(R"([{"text":"你好","translation":"hi"}])"),
          std::string(R"([{"text":"你好","translation":"hello"},{"text":"你号","translation":"your number"}])"));
      REQUIRE(merged.has_value());
      const auto values = nlohmann::json::parse(*merged);
      REQUIRE(values.is_array() && values.size() == 3);
      REQUIRE(values[0].at("text") == "你好" && values[0].at("translation") == "hi");
      REQUIRE(!values[0].value("gloss", false));
      REQUIRE(values[1].at("text") == "你好" && values[1].at("translation") == "hello");
      REQUIRE(values[1].at("gloss") == true);
      REQUIRE(values[2].at("text") == "你号" && values[2].at("gloss") == true);
    }
    // 主查询没有回答（服务失败、没配服务、高亮候选查不到）而离线释义有：照样交出离线释义，其他行不因为高亮那一行没译文而丢掉释义。
    {
      const auto merged = merge_gloss_answers(
          std::nullopt, std::string(R"([{"text":"拟好","translation":"draft"}])"));
      REQUIRE(merged.has_value());
      const auto values = nlohmann::json::parse(*merged);
      REQUIRE(values.size() == 1 && values[0].at("gloss") == true);
      // 主查询答了空数组也一样。
      const auto empty_primary = merge_gloss_answers(
          std::string("[]"), std::string(R"([{"text":"拟好","translation":"draft"}])"));
      REQUIRE(empty_primary && nlohmann::json::parse(*empty_primary).size() == 1);
    }
    // 只有主查询有回答：原样交出，不加标记。
    {
      const auto merged =
          merge_gloss_answers(std::string(R"([{"text":"你好","translation":"hi"}])"), std::nullopt);
      REQUIRE(merged.has_value());
      const auto values = nlohmann::json::parse(*merged);
      REQUIRE(values.size() == 1 && !values[0].contains("gloss"));
    }
    // 两边都没有回答：什么也不交。空数组表示清掉全部释义，不能拿它当「没有回答」交出去。
    REQUIRE(!merge_gloss_answers(std::nullopt, std::nullopt).has_value());
    REQUIRE(!merge_gloss_answers(std::string("[]"), std::string("[]")).has_value());
    // 坏掉的回答当作没有回答，不连累另一边。
    {
      const auto merged = merge_gloss_answers(std::string("not json"),
                                              std::string(R"([{"text":"拟好","translation":"draft"},7])"));
      REQUIRE(merged.has_value());
      REQUIRE(nlohmann::json::parse(*merged).size() == 1);
    }

    // 读音按整页候选算：带 gloss_query 时是它覆盖的整页（含高亮那个），不是只有高亮候选的主查询。
    {
      auto query = nlohmann::json{{"generation", 7}, {"candidates", candidates({"你好"})}};
      REQUIRE(translation_page_candidates(query).size() == 1);
      query["gloss_query"] = nlohmann::json{{"generation", 7}, {"candidates", candidates({"你好", "拟好"})}};
      const auto &page = translation_page_candidates(query);
      REQUIRE(page.size() == 2 && page[1].at("text") == "拟好");
      // gloss_query 坏掉时退回主查询自己的候选。
      query["gloss_query"] = nlohmann::json{{"generation", 7}};
      REQUIRE(translation_page_candidates(query).size() == 1);
    }
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
