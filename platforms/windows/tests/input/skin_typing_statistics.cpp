#include "SkinTypingStatistics.h"
#include <cassert>
#include <iostream>
#include <vector>

using namespace msime::windows;
using Json = nlohmann::json;

int main() {
  AppliedSkinStatistics observer;
  std::vector<Json> requests;
  const auto apply = [&](const char *theme, const char *package) {
    if (auto id = observer.observe(theme, package))
      requests.push_back(Json::parse(typing_statistics_record_skin_request(
          "/synthetic/statistics", *id)));
  };

  // 首次套用、原生设置或托盘换主题都记一次；窗口重绘和资源重载不重复记。
  apply("system", "");
  apply("system", "");
  apply("system", "");
  assert(requests.size() == 1);
  assert(requests.back() == (Json{{"directory", "/synthetic/statistics"},
                                 {"action", {{"operation", "record_skin"},
                                             {"id", "system"}}}}));
  apply("songlin", "");
  assert(requests.back().at("action").at("id") == "songlin");

  // 外部包以共享解析器实际选中的 ID 记录，不能把两个明暗槽都当作已使用。
  apply("custom", "synthetic-light");
  apply("custom", "synthetic-light");
  assert(requests.size() == 3);
  assert(requests.back().at("action").at("id") == "synthetic-light");
  apply("custom", "synthetic-dark");
  assert(requests.back().at("action").at("id") == "synthetic-dark");
  apply("custom", "");
  assert(requests.back().at("action").at("id") == "custom");

  // 回到以前的皮肤仍提交事件，由共享存储按 ID 去重；不在宿主复制成就规则。
  apply("system", "");
  assert(requests.size() == 6);
  assert(requests.back().at("action").at("id") == "system");
  assert(!observer.observe("", ""));
  assert(!observer.observe("system", ""));

  // 请求只带聚合所需的 ID，不包含截图、目录里的资源或输入内容。
  assert(typing_statistics_record_skin_request("", "system").empty());
  assert(typing_statistics_record_skin_request("/synthetic/statistics", "").empty());
  for (const auto &request : requests) {
    assert(request.size() == 2);
    assert(request.at("action").size() == 2);
    assert(request.at("action").at("operation") == "record_skin");
  }
  std::cout << "Applied skin typing statistics passed\n";
}
