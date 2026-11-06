#include "VoicePunctuation.h"

#include <cassert>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>

namespace {
using msime::windows::voice_punctuated;

// 模拟共享层：记下收到的请求，返回给定的 value。
struct FakeHost {
  explicit FakeHost(std::optional<nlohmann::json> response) : value(std::move(response)) {}
  std::optional<nlohmann::json> value;
  int calls = 0;
  nlohmann::json last_request;
  std::optional<nlohmann::json> operator()(const std::string &request) {
    ++calls;
    last_request = nlohmann::json::parse(request);
    return value;
  }
};
} // namespace

int main() {
  // auto（出厂默认）、缺省和空文本都不调用共享层，原样上屏。
  {
    FakeHost host{nlohmann::json{{"text", "不该用到"}}};
    assert(voice_punctuated("天啊！他是谁？", "auto", host) == "天啊！他是谁？");
    assert(voice_punctuated("天啊！他是谁？", "", host) == "天啊！他是谁？");
    assert(voice_punctuated("", "omit_all", host).empty());
    assert(host.calls == 0);
  }
  // 其他方式把文本和方式交给共享层，用它返回的文本。
  {
    FakeHost host{nlohmann::json{{"text", "天啊！他是谁"}}};
    assert(voice_punctuated("天啊！他是谁？", "omit_trailing", host) == "天啊！他是谁");
    assert(host.calls == 1);
    assert(host.last_request == (nlohmann::json{{"text", "天啊！他是谁？"}, {"mode", "omit_trailing"}}));
  }
  // 整段只有标点时共享层返回空串，这里保持为空，交给调用方按没有识别到文字提示。
  {
    FakeHost host{nlohmann::json{{"text", ""}}};
    assert(voice_punctuated("。", "omit_trailing", host).empty());
  }
  // 调用失败或响应格式不对时保留原文。
  {
    FakeHost failed{std::nullopt};
    assert(voice_punctuated("你好。", "omit_all", failed) == "你好。");
    FakeHost not_object{nlohmann::json::array()};
    assert(voice_punctuated("你好。", "omit_all", not_object) == "你好。");
    FakeHost missing{nlohmann::json::object()};
    assert(voice_punctuated("你好。", "omit_all", missing) == "你好。");
    FakeHost wrong_type{nlohmann::json{{"text", 1}}};
    assert(voice_punctuated("你好。", "omit_all", wrong_type) == "你好。");
  }
}
