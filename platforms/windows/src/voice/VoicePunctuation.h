#pragma once
// 语音标点（voice_input.punctuation_mode）：最终识别结果在润色之后、繁体转换和上屏之前按设置处理一次，规则只在共享层 msime_client_voice_punctuation。中间结果不经过这里。

#include <nlohmann/json.hpp>

#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {

// `call` 把请求 JSON 交给 msime_client_voice_punctuation，返回响应里的 value，调用失败时返回 std::nullopt。
// auto（出厂默认）和空文本不调用共享层，行为与没有这项设置时相同。调用失败或响应格式不对时保留原文：设置故障不能丢掉已经识别出的文字。
// 与 correct_with_hotwords 不同，共享层返回的空串原样返回：整段只有标点时处理后为空，由调用方按没有识别到文字提示。
template <typename Call>
std::string voice_punctuated(const std::string &text, std::string_view mode, Call &&call) {
  if (text.empty() || mode.empty() || mode == "auto")
    return text;
  const auto request = nlohmann::json{{"text", text}, {"mode", mode}}.dump(
      -1, ' ', false, nlohmann::json::error_handler_t::replace);
  const std::optional<nlohmann::json> value = call(request);
  if (!value || !value->is_object())
    return text;
  const auto processed = value->find("text");
  if (processed == value->end() || !processed->is_string())
    return text;
  return processed->get<std::string>();
}

} // namespace msime::windows
