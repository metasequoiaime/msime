// TIP 续接 Server 的部分选词（ContinueHostCandidate）：候选不在本会话的当前页上时（Server 卷轴展开网格里的候选），到本会话这一代的完整候选列表里按文字找唯一的一个，按全局下标选；页上有的照旧按页上的选。只模拟宿主边界，数据是合成的。
#include "../../tsf/HostCandidateContinuation.h"
#include <cstdio>
#include <cstdlib>

using namespace msime::windows;
namespace {
void require(bool value, const char *message) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", message);
    std::exit(EXIT_FAILURE);
  }
}
nlohmann::json view(const char *editing, uint64_t generation, const std::vector<std::string> &page) {
  nlohmann::json candidates = nlohmann::json::array();
  for (size_t index = 0; index < page.size(); ++index)
    candidates.push_back({{"id", {{"session", 5}, {"generation", generation}, {"index", index}}},
                          {"text", page[index]},
                          {"highlighted", index == 0}});
  return {{"session", 5},       {"generation", generation}, {"editing_text", editing},
          {"preedit", editing}, {"caret_position", 0},      {"scheme", 0},
          {"local_mode", "url"}, {"dedicated_english", false}, {"candidates", candidates}};
}
struct Host {
  nlohmann::json current, all, selected;
  bool all_ok = true, select_ok = true;
  uint64_t page_generation = 0, any_generation = 0;
  size_t page_index = 999, any_index = 999;
  bool valid() const { return true; }
  bool view(std::string *out, std::string *) {
    *out = nlohmann::json{{"ok", true}, {"value", current}}.dump();
    return true;
  }
  bool select(uint64_t generation, size_t index, std::string *out, std::string *) {
    page_generation = generation;
    page_index = index;
    *out = selected.dump();
    return select_ok;
  }
  bool all_candidates(std::string *out, std::string *) {
    *out = nlohmann::json{{"ok", true}, {"value", all}}.dump();
    return all_ok;
  }
  bool select_any(uint64_t generation, size_t index, std::string *out, std::string *) {
    any_generation = generation;
    any_index = index;
    *out = selected.dump();
    return select_ok;
  }
};
} // namespace

int main() {
  const auto before = view("ht", 40, {"https://", "http://"});
  const auto after = view("https://", 42, {});
  const auto payload = candidate_continuation_request(before, "https://www.", after);
  Host baseline;
  baseline.current = before;
  baseline.selected = {{"ok", true}, {"value", {{"handled", true}, {"commit", nullptr}, {"view", after}}}};
  // 取完整列表放出了尾部，代次从 40 前进到 41：按列表自己的代次和全局下标选。
  baseline.all = view("ht", 41, {"https://", "http://", "ftp://", "https://www."});
  {
    auto host = baseline;
    require(msime::tsf::ContinueHostCandidate(host, payload) && host.any_generation == 41 &&
                host.any_index == 3 && host.page_index == 999,
            "grid candidate selected from the full list");
  }
  {
    // 页上有的候选照旧按页上的选，不去取完整列表。
    auto host = baseline;
    host.all_ok = false;
    const auto on_page = candidate_continuation_request(before, "http://", after);
    require(msime::tsf::ContinueHostCandidate(host, on_page) && host.page_generation == 40 &&
                host.page_index == 1 && host.any_index == 999,
            "page candidate keeps page selection");
  }
  {
    auto host = baseline;
    host.all["candidates"].push_back(host.all["candidates"][3]);
    host.all["candidates"].back()["id"]["index"] = 4;
    require(!msime::tsf::ContinueHostCandidate(host, payload) && host.any_index == 999,
            "ambiguous full-list candidate rejected");
  }
  {
    auto host = baseline;
    host.all["candidates"].erase(3);
    require(!msime::tsf::ContinueHostCandidate(host, payload) && host.any_index == 999,
            "missing full-list candidate rejected");
  }
  {
    auto host = baseline;
    host.all_ok = false;
    require(!msime::tsf::ContinueHostCandidate(host, payload) && host.any_index == 999,
            "unreadable full list rejected");
  }
  {
    auto host = baseline;
    host.select_ok = false;
    require(!msime::tsf::ContinueHostCandidate(host, payload), "failed select-any rejected");
  }
  {
    // 前置状态对不上（两边的组字不同）时什么都不选。
    auto host = baseline;
    host.current["editing_text"] = "hx";
    require(!msime::tsf::ContinueHostCandidate(host, payload) && host.any_index == 999,
            "stale before rejected");
  }
  std::puts("host candidate continuation: passed");
}
