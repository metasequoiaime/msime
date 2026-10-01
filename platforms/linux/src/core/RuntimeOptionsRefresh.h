#pragma once

#include "msime_client.h"

#include <filesystem>
#include <memory>
#include <nlohmann/json.hpp>
#include <stdexcept>
#include <string>

namespace msime::linux_host {

// The recorded resource directory does not hold the dictionaries this version pins: the user downloaded them with msime-linux-setup and a package upgrade raised the dictionary version without replacing them. Unlike every other refresh failure this one has a fix the user can run, so the hosts point at it.
struct DictionaryOutdated : std::runtime_error {
  DictionaryOutdated() : std::runtime_error("recorded dictionaries are older than this version") {}
};

// 把安装包升级前写下的运行时配置带到已安装的词库代次：Host API 准备新代次、把用户词库回放进去并改写文件。文件被改写时返回 true，已是当前代次时返回 false。准备失败时抛出异常，原因是记录的词库与本版本不符时抛出 DictionaryOutdated；此时 `resources` 与 `dictionaries` 保持原样（资源目录旁装有粤语或注音词库时，`language_dictionaries` 仍会更新），旧代次照常可用，调用方继续运行，下次启动再试。Host API 的错误除稳定前缀外不向外报告，因为其中可能有私人路径。
inline bool refresh_runtime_options(const std::filesystem::path &path) {
  const auto text = path.string();
  std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
      msime_client_refresh_host(reinterpret_cast<const uint8_t *>(text.data()), text.size()),
      msime_client_string_free);
  if (!raw) throw std::runtime_error("runtime options refresh failed");
  const auto result = nlohmann::json::parse(raw.get());
  if (!result.value("ok", false)) {
    const auto error = result.value("error", std::string{});
    if (error.rfind("dictionary_outdated:", 0) == 0) throw DictionaryOutdated();
    throw std::runtime_error("runtime options refresh failed");
  }
  if (!result.at("value").is_boolean()) throw std::runtime_error("runtime options refresh failed");
  return result.at("value").get<bool>();
}

} // namespace msime::linux_host
