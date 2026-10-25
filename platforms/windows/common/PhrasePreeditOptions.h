#pragma once
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {
// 两个进程都能绘制暂存词段，由同一个 Runtime 保存选词与退格历史。
inline std::string phrase_preedit_options(const std::string &options) {
  auto document = nlohmann::json::parse(options);
  document["phrase_preedit"] = true;
  document["korean_romaja"] = true;
  return document.dump();
}
} // namespace msime::windows
