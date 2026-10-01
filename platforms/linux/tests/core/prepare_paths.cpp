#include "../src/core/PreparePaths.h"
#include "../src/core/PrepareState.h"

#include <cassert>
#include <filesystem>

int main() {
  const auto root = std::filesystem::temp_directory_path() / "msime-prepare-paths-test";
  std::error_code error;
  std::filesystem::remove_all(root, error);
  std::filesystem::create_directories(root / "share/msime-client/resources");

  const auto executable = root / "bin/msime-linux-prepare";
  const auto expected = std::filesystem::canonical(root / "share/msime-client/resources");
  assert(msime_linux::installed_resource_directory(executable) == expected.string());
  std::filesystem::create_directories(root / "libdata/msime-client/resources");
  const auto custom_expected =
      std::filesystem::canonical(root / "libdata/msime-client/resources");
  assert(msime_linux::installed_resource_directory(
             executable, "../libdata/msime-client/resources") ==
         custom_expected.string());
  assert(msime_linux::installed_resource_directory(executable, "/etc/passwd").empty());
  assert(msime_linux::installed_resource_directory("bin/msime-linux-prepare").empty());
  // The built-in sound packs sit beside the resource bundle and resolve the same way, only once installed.
  assert(msime_linux::installed_sound_pack_directory(executable).empty());
  std::filesystem::create_directories(root / "share/msime-client/sound-packs");
  assert(msime_linux::installed_sound_pack_directory(executable) ==
         std::filesystem::canonical(root / "share/msime-client/sound-packs").string());
  assert(msime_linux::installed_sound_pack_directory("bin/msime-linux-ibus").empty());

  const auto outside = root / "outside";
  std::filesystem::create_directories(outside);
  const auto linked = root / "linked";
  std::filesystem::create_directory_symlink(outside, linked);
  assert(!msime_linux::state_directory_path_is_safe(linked / "new-state"));
  assert(msime_linux::state_directory_path_is_safe(root / "fresh-state"));

  std::filesystem::remove_all(root, error);
  return 0;
}
