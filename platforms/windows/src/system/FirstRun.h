#pragma once
#include "PrepareHost.h"
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
// 安装器写下的所有权标记，文件名按版本取（full 是 .metasequoiaime-data）。只认本版本的文件名：带着别的版本标记的目录不是本版本安装器准备的，Server 不在里面准备状态。
inline constexpr const wchar_t *kDataDirectoryMarker = MSIME_EDITION_DATA_DIR_MARKER;

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
