#include "../src/core/ViewComposition.h"

#include <cassert>
#include <string>

using Json = nlohmann::json;
using msime::linux_host::view_is_composing;

int main() {
  // #6675：输入模式提示的 1.2 秒定时器原来直接对 `State::view` 调 value()。会话还没打开或刚被 close() 时视图是 null，value() 抛 type_error.306，而那个 glib 回调外面没有 guarded()，宿主进程因此 abort。这里先钉住这条前提，再确认新的判断在同样的输入上不抛。
  const Json closed;
  bool threw = false;
  try {
    (void)closed.value("editing_text", std::string{});
  } catch (const Json::type_error &error) {
    threw = error.id == 306;
  }
  assert(threw);
  assert(!view_is_composing(closed));
  assert(!view_is_composing(Json(nullptr)));

  // 其他不是对象的形状同样当作没有组字。
  assert(!view_is_composing(Json::array({"ni"})));
  assert(!view_is_composing(Json("ni")));

  // 空视图和空组字。
  assert(!view_is_composing(Json::object()));
  assert(!view_is_composing(Json{{"editing_text", ""}, {"candidates", Json::array()}}));

  // 编辑串或候选任一非空就是在组字，此时提示不能收掉辅助区域里的候选页码。
  assert(view_is_composing(Json{{"editing_text", "ni"}, {"candidates", Json::array()}}));
  assert(view_is_composing(Json{{"editing_text", ""}, {"candidates", Json::array({Json::object()})}}));
  assert(view_is_composing(Json{{"candidates", Json::array({Json::object()})}}));
  assert(view_is_composing(Json{{"editing_text", "ni"}}));

  // 字段类型不对时不抛，按没有该字段处理。
  assert(!view_is_composing(Json{{"editing_text", 3}, {"candidates", "x"}}));
  assert(view_is_composing(Json{{"editing_text", 3}, {"candidates", Json::array({1})}}));
  return 0;
}
