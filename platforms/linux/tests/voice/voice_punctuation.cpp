#include "voice/VoicePunctuation.h"

#include <cassert>
#include <optional>
#include <string>
#include <utility>

namespace {
using msime::linux_host::voice_punctuated;
using msime::linux_host::voice_punctuation_mode;
using Json = nlohmann::json;

// 模拟共享层：记下调用次数，返回给定的 value。
struct FakeHost {
  explicit FakeHost(std::optional<Json> response) : value(std::move(response)) {}
  std::optional<Json> value;
  int calls = 0;
  std::optional<Json> operator()(const std::string &request) {
    ++calls;
    const auto parsed = Json::parse(request);
    assert(parsed.is_object());
    return value;
  }
};
} // namespace

int main() {
  // 缺键、空串、非字符串都是出厂默认 auto；不认识的取值原样保留，交给共享层按 auto 处理。
  assert(voice_punctuation_mode(Json::object()) == "auto");
  assert(voice_punctuation_mode(Json::array()) == "auto");
  assert(voice_punctuation_mode(Json{{"voice_input", Json::object()}}) == "auto");
  assert(voice_punctuation_mode(Json{{"voice_input", {{"punctuation_mode", ""}}}}) == "auto");
  assert(voice_punctuation_mode(Json{{"voice_input", {{"punctuation_mode", 1}}}}) == "auto");
  assert(voice_punctuation_mode(Json{{"voice_input", {{"punctuation_mode", "omit_all"}}}}) == "omit_all");
  assert(voice_punctuation_mode(Json{{"voice_input", {{"punctuation_mode", "from_a_newer_version"}}}}) ==
         "from_a_newer_version");

  // auto 和空文本不调用共享层。
  {
    FakeHost host{Json{{"text", "不该用到"}}};
    assert(voice_punctuated("你好。", "auto", host) == "你好。");
    assert(voice_punctuated("", "omit_all", host).empty());
    assert(host.calls == 0);
  }
  // 共享层返回空串时保持为空；调用失败或响应格式不对时保留原文。
  {
    FakeHost empty{Json{{"text", ""}}};
    assert(voice_punctuated("。", "omit_trailing", empty).empty());
    FakeHost failed{std::nullopt};
    assert(voice_punctuated("你好。", "omit_all", failed) == "你好。");
    FakeHost wrong_type{Json{{"text", 1}}};
    assert(voice_punctuated("你好。", "omit_all", wrong_type) == "你好。");
  }

  // 经真实的共享层：四种方式与 Android 的结果一致，只剩标点时为空，超过上限时保留原文。
  const std::string text = "天啊！他是谁？";
  assert(voice_punctuated(text, "auto") == text);
  assert(voice_punctuated(text, "omit_trailing") == "天啊！他是谁");
  assert(voice_punctuated(text, "omit_all") == "天啊他是谁");
  assert(voice_punctuated(text, "space") == "天啊 他是谁");
  assert(voice_punctuated(text, "from_a_newer_version") == text);
  assert(voice_punctuated("。", "omit_trailing").empty());
  std::string too_long;
  for (int index = 0; index < 10001; ++index)
    too_long += "好";
  assert(voice_punctuated(too_long, "omit_all") == too_long);
  return 0;
}
