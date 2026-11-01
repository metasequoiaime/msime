#pragma once

#include "../../../common/HostApiString.h"

#include <cstdint>
#include <cstring>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::linux_host {

// IBus「候选操作」菜单和 Fcitx5 候选右键菜单里的那一项，照 macOS 和 Windows 的候选菜单放在最后一组。
inline constexpr const char *candidate_personal_wordbook_label = "加入生词本";
// 交不出去或存储出错时显示的提示；其余提示都由共享层给出。
inline constexpr const char *personal_wordbook_failure_notice = "无法加入生词本";

// 把候选原文和它此刻显示着的释义交给共享层的 `msime_client_personal_wordbook_add`，返回它答的那句提示。`directory` 是偏好目录（与背单词、打字统计同一个）。会取词书目录的文件锁、写一个小文件；只在用户点了菜单项之后调用一次。Linux 不画读音，所以不带读音。
inline std::string add_to_personal_wordbook(const std::string &directory, const std::string &candidate,
                                            const std::string &gloss) {
  if (directory.empty() || directory.front() != '/')
    return personal_wordbook_failure_notice;
  std::string body;
  try {
    body = nlohmann::json{{"directory", directory}, {"candidate", candidate}, {"gloss", gloss}}.dump();
  } catch (const nlohmann::json::exception &) {
    return personal_wordbook_failure_notice; // 不是有效的 UTF-8。
  }
  auto reply = msime::host_api::own_string(msime_client_personal_wordbook_add(
      reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  if (!reply)
    return personal_wordbook_failure_notice;
  const auto envelope = nlohmann::json::parse(reply.get(), reply.get() + std::strlen(reply.get()), nullptr, false);
  if (!envelope.is_object() || envelope.value("ok", false) != true || !envelope.contains("value") ||
      !envelope.at("value").is_object() || !envelope.at("value").contains("notice") ||
      !envelope.at("value").at("notice").is_string())
    return personal_wordbook_failure_notice;
  return envelope.at("value").at("notice").get<std::string>();
}

} // namespace msime::linux_host
