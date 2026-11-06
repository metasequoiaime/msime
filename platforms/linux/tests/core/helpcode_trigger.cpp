#include "../../src/core/HelpcodeTrigger.h"

#include <cassert>
#include <string>

using Json = nlohmann::json;
namespace host = msime::linux_host;

namespace {

std::string inserted(const Json &view, std::string editing, std::size_t &caret) {
  host::insert_helpcode_trigger(view, editing, caret);
  return editing;
}

} // namespace

int main() {
  // 前导键刚按下：插在末尾，光标跟到它后面。
  {
    std::size_t caret = 2;
    const auto view = Json::parse(R"({"editing_text":"ni","helpcode_trigger":{"key":";","position":2}})");
    assert(inserted(view, "ni", caret) == "ni;" && caret == 3);
  }

  // 已经输了一位码：插在码前面，光标仍在末尾。
  {
    std::size_t caret = 3;
    const auto view = Json{{"helpcode_trigger", Json{{"key", ";"}, {"position", 2}}}};
    assert(inserted(view, "niF", caret) == "ni;F" && caret == 4);
  }

  // 光标在前导键前面时不动（引擎不会这样给，但宿主不该把光标挪错）。
  {
    std::size_t caret = 1;
    const auto view = Json{{"helpcode_trigger", Json{{"key", "/"}, {"position", 2}}}};
    assert(inserted(view, "ni", caret) == "ni/" && caret == 1);
  }

  // 字段缺失、类型不对、位置越界或为负、视图不是对象：原样返回。
  for (const auto &view : {Json(), Json::array(), Json{{"editing_text", "ni"}}, Json{{"helpcode_trigger", nullptr}},
                           Json{{"helpcode_trigger", ";"}}, Json{{"helpcode_trigger", Json{{"key", ";"}}}},
                           Json{{"helpcode_trigger", Json{{"key", ";"}, {"position", "2"}}}},
                           Json{{"helpcode_trigger", Json{{"key", ";"}, {"position", 3}}}},
                           Json{{"helpcode_trigger", Json{{"key", ";"}, {"position", -1}}}},
                           Json{{"helpcode_trigger", Json{{"key", ""}, {"position", 2}}}},
                           Json{{"helpcode_trigger", Json{{"key", 59}, {"position", 2}}}}}) {
    std::size_t caret = 2;
    assert(inserted(view, "ni", caret) == "ni" && caret == 2);
  }
  return 0;
}
