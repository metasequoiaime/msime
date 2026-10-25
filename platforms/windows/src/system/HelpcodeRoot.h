#pragma once
#include <filesystem>
#include <nlohmann/json.hpp>
#include <system_error>

namespace msime::windows {
// Name where the built-in helpcode tables are in the options sessions are created from. The shared library reads them from `helpcodes` under the resources directory, where the macOS bundle and the Linux install put them, and falls back to `helpcode_fallback_resources` only when the table is missing there. The Windows installer stages them in DataDir\helpcodes instead (installer/Prepare-PackageFiles.ps1), while the resources directory is the Server's under Program Files, and DataDir is the state root of both the Server and the TIP. Without this name every built-in table reads as empty: helpcodes neither annotate nor narrow the candidates. A state root without them, as in a development run, leaves the options alone, and a root already in the options wins.
inline void name_builtin_helpcodes(nlohmann::json &options,
                                   const std::filesystem::path &state_root) {
  if (!options.is_object() || options.contains("helpcode_fallback_resources") ||
      !state_root.is_absolute())
    return;
  std::error_code error;
  if (std::filesystem::is_directory(state_root / "helpcodes", error))
    options["helpcode_fallback_resources"] = state_root.u8string();
}
} // namespace msime::windows
