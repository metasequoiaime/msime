#include "../src/candidates/CandidateSources.h"

#include <cassert>
#include <cstdint>
#include <nlohmann/json.hpp>

int main() {
  namespace host = msime::linux_host;
  using Json = nlohmann::json;
  // 云的占位席位（15）不画、不占序号；其余来源照旧，包括旧的 AI 占位行（12）。
  for (const std::uint64_t source : {0u, 1u, 2u, 3u, 4u, 12u})
    assert(host::candidate_source_is_visible(source));
  assert(!host::candidate_source_is_visible(host::kCandidateSourceCloudPending));
  // 引擎这一页是「本地、云的占位席位、本地」。用户看到两行：按 2 要上屏的是第二行（页内下标 2），按 3 什么也不选。
  const Json page = Json::array({
      {{"text", "测试"}, {"source", 0}},
      {{"text", "☁"}, {"source", host::kCandidateSourceCloudPending}},
      {{"text", "侧室"}, {"source", 0}, {"id", {{"index", 2}}}},
  });
  assert(host::visible_page_size(page) == 2);
  assert(host::visible_page_candidate(page, 0)->at("text") == "测试");
  assert(host::visible_page_candidate(page, 1)->at("text") == "侧室");
  assert(host::visible_page_candidate(page, 1)->at("id").at("index") == 2);
  assert(host::visible_page_candidate(page, 2) == nullptr);
  // 过滤后的页与映射看到的是同一组行，并保留原来的身份（点选按它，不按数组位置）。
  const auto visible = host::visible_page(page);
  assert(visible.size() == host::visible_page_size(page));
  assert(visible.at(1).at("id").at("index") == 2);
  assert(visible.at(1).value("source", 99u) == 0);
  // 只有占位席位的一页没有可画的候选，候选窗不该为它打开。
  const Json pending_only = Json::array({{
      {"text", "☁"}, {"source", host::kCandidateSourceCloudPending}}});
  assert(host::visible_page_size(pending_only) == 0);
  assert(host::visible_page_candidate(pending_only, 0) == nullptr);
  assert(host::visible_page(pending_only).empty());
  // 缺 source 的行按本地候选算。
  const Json no_source = Json::array({{{"text", "无来源"}}});
  assert(host::visible_page_size(no_source) == 1);
  assert(host::visible_page_candidate(no_source, 0)->at("text") == "无来源");
  return 0;
}
