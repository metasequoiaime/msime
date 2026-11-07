#pragma once
// 语音标点（voice_input.punctuation_mode）：宿主拿到最终文本（msime_voice_result_or_transcript 之后）、交给会话上屏之前按设置处理一次，规则只在共享层 msime_client_voice_punctuation。放在宿主而不是语音服务里：服务交回空结果时宿主会退回中间转写，只在服务里处理的话，只剩标点的结果会以带标点的中间转写上屏；源码检出时服务也加载不到 host 库。中间结果不经过这里。

#include "../../../common/HostApiString.h"

#include <nlohmann/json.hpp>

#include <optional>
#include <string>
#include <string_view>
#include <utility>

namespace msime::linux_host {

// 共享文档里的语音标点方式。出厂默认 auto 不写进文档，所以缺键、空串和非字符串都是 auto；不认识的取值原样交给共享层，由它按 auto 处理。
inline std::string voice_punctuation_mode(const nlohmann::json &preferences) {
  if (!preferences.is_object())
    return "auto";
  const auto voice = preferences.find("voice_input");
  if (voice == preferences.end() || !voice->is_object())
    return "auto";
  const auto mode = voice->find("punctuation_mode");
  if (mode == voice->end() || !mode->is_string() || mode->get_ref<const std::string &>().empty())
    return "auto";
  return mode->get<std::string>();
}

// `call` 把请求 JSON 交给 msime_client_voice_punctuation，返回响应里的 value，调用失败时返回 std::nullopt。
// auto 和空文本不调用共享层，行为与没有这项设置时相同。调用失败或响应格式不对时保留原文：设置故障不能丢掉已经识别出的文字。
// 共享层返回的空串原样返回：整段只有标点时处理后为空，由调用方按没有识别到文字提示。
template <typename Call>
std::string voice_punctuated(std::string text, std::string_view mode, Call &&call) {
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

// 经共享层处理，IBus 和 Fcitx5 两个宿主共用。
inline std::string voice_punctuated(std::string text, std::string_view mode) {
  return voice_punctuated(std::move(text), mode, [](const std::string &request) -> std::optional<nlohmann::json> {
    const auto owned = msime::host_api::own_string(msime_client_voice_punctuation(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    if (!owned)
      return std::nullopt;
    auto document = nlohmann::json::parse(owned.get(), nullptr, false);
    if (!document.is_object() || !document.value("ok", false) || !document.contains("value"))
      return std::nullopt;
    return std::move(document.at("value"));
  });
}

} // namespace msime::linux_host
