#pragma once
#include "PrepareHost.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <fstream>
#include <istream>
#include <optional>
#include <string>

namespace msime::windows {
// 安装器写下的所有权标记，文件名按版本取（full 是 .metasequoiaime-data.full）。只认本版本的文件名：带着别的版本或 msime-windows（.metasequoiaime-data）标记的目录不是本版本安装器准备的，Server 不在里面准备状态。
inline constexpr const wchar_t *kDataDirectoryMarker = MSIME_EDITION_DATA_DIR_MARKER;
// 安装器「联网功能」页的选择，全新安装时写在数据目录里，形如 {"cloud_candidates": true}。运行中的宿主只读共享偏好，不读 config.toml，所以这份选择要由 Server 在首次准备状态后写进共享偏好。
inline constexpr const wchar_t *kInstallerChoicesFile = L"installer-choices.json";

// 解析安装器写入的联网选择，并把流读取限制在契约大小内。路径的大小检查与打开文件之间存在竞态，不能只依赖 `file_size` 防止替换后的文件占用无界内存。
inline std::optional<bool> parse_installer_cloud_choice(std::istream &input) {
  constexpr std::streamsize max_bytes = 4096;
  std::string document(static_cast<std::size_t>(max_bytes) + 1, '\0');
  input.read(document.data(), static_cast<std::streamsize>(document.size()));
  const auto count = input.gcount();
  if (input.bad() || count > max_bytes)
    return std::nullopt;
  document.resize(static_cast<std::size_t>(count));
  const auto parsed = nlohmann::json::parse(document, nullptr, false);
  if (!parsed.is_object() || !parsed.contains("cloud_candidates") ||
      !parsed.at("cloud_candidates").is_boolean())
    return std::nullopt;
  return parsed.at("cloud_candidates").get<bool>();
}

// 读出安装器记下的云候选选择并删掉这个文件：它只对首次准备有意义，留着会在以后重新准备状态时被错误地再用一次。文件缺失、过大、不是 JSON 或没有布尔值时返回空，偏好保持共享默认值（关闭）。
inline std::optional<bool> take_installer_cloud_choice(const std::filesystem::path &state) {
  const auto path = state / kInstallerChoicesFile;
  std::error_code error;
  if (!std::filesystem::is_regular_file(path, error)) return std::nullopt;
  std::optional<bool> choice;
  if (std::filesystem::file_size(path, error) <= 4096 && !error) {
    std::ifstream input(path, std::ios::binary);
    if (input)
      choice = parse_installer_cloud_choice(input);
  }
  std::filesystem::remove(path, error);
  return choice;
}

// Called only after acquiring the production single-instance guard and before
// StateRootLease (which would itself create the directory). Installer-created
// roots already exist and carry an ownership marker, but do not contain
// runtime-options.json because the elevated installer must not prepare state.
inline bool prepare_first_run(
    const std::filesystem::path &executable,
    const std::filesystem::path &state,
    const std::function<std::string(const std::string &)> &prepare) {
  if (!state.is_absolute())
    throw std::runtime_error("Production state directory unavailable");
  if (!executable.is_absolute())
    throw std::runtime_error("Installed resource directory unavailable");
  const auto status = std::filesystem::symlink_status(state);
  if (status.type() == std::filesystem::file_type::not_found) {
    prepare_host_state(executable / "resources", state, prepare);
    return true;
  }
  if (status.type() != std::filesystem::file_type::directory ||
      std::filesystem::exists(state / L"runtime-options.json") ||
      !std::filesystem::is_regular_file(state / kDataDirectoryMarker))
    return false;
  prepare_host_state_in_directory(executable / "resources", state, prepare);
  return true;
}
} // namespace msime::windows
