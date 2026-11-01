#pragma once

#include "HostApiString.h"

#include <cstdint>
#include <cstring>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

// 候选菜单「朗读」的共同部分，Windows 和 Linux 的候选菜单都用它：菜单项的文字，以及经 msime_client_read_aloud_text 问共享层要读哪段英文。规则只在共享层，见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
namespace msime::read_aloud {

inline constexpr const char *kMenuLabel = "朗读";

// 候选 `candidate` 显示着释义 `gloss` 时要读的英文；没有能读的、请求不是有效的 UTF-8 或共享层出错时为空，菜单就不出「朗读」。不读写文件，在打开菜单时调用。
inline std::optional<std::string> text_for(const std::string &candidate, const std::string &gloss) {
  std::string body;
  try {
    body = nlohmann::json{{"candidate", candidate}, {"gloss", gloss}}.dump();
  } catch (const nlohmann::json::exception &) {
    return std::nullopt; // 不是有效的 UTF-8。
  }
  auto reply = msime::host_api::own_string(
      msime_client_read_aloud_text(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  if (!reply)
    return std::nullopt;
  const auto envelope = nlohmann::json::parse(reply.get(), reply.get() + std::strlen(reply.get()), nullptr, false);
  if (!envelope.is_object() || envelope.value("ok", false) != true || !envelope.contains("value") ||
      !envelope.at("value").is_string())
    return std::nullopt;
  auto text = envelope.at("value").get<std::string>();
  if (text.empty())
    return std::nullopt;
  return text;
}

} // namespace msime::read_aloud
