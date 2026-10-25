#pragma once
#include <filesystem>
#include <nlohmann/json.hpp>

// 共享回归与原生管道测试使用隔离的中文会话，不依赖各平台不同的出厂中英文状态。
// 调用方负责临时目录及清理；不读取用户数据。
inline nlohmann::json test_host_options(const std::filesystem::path &root) {
  nlohmann::json result{{"api_version", 1},
                        {"preferences",
                         {{"scheme", "quanpin"},
                          {"default_ime_mode", "chinese"},
                          {"learning", false},
                          {"cloud_candidates", false},
                          {"candidate_page_size", 5},
                          {"chinese_punctuation", true}}}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    const auto directory = root / name;
    std::filesystem::create_directories(directory);
    result[name] = directory.u8string();
  }
  return result;
}
