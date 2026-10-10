#include "../../src/candidate/SkinResourceRevision.h"
#include <cassert>
#include <chrono>
#include <fstream>

int main() {
  using namespace msime::windows;
  const auto root =
      std::filesystem::temp_directory_path() /
      ("msime-skin-revision-" +
       std::to_string(
           std::chrono::steady_clock::now().time_since_epoch().count()));
  const auto directory = root / "sample";
  std::filesystem::create_directories(directory / "images");
  std::ofstream(directory / "skin.toml") << "synthetic";
  SkinResourceRevision revision;
  assert(revision.changed(root, {"sample"}));
  assert(!revision.changed(root, {"sample"}));
  const auto image = directory / "images" / "preview.png";
  std::ofstream(image) << "first";
  assert(revision.changed(root, {"sample"}));
  assert(!revision.changed(root, {"sample"}));
  const auto old_time = std::filesystem::last_write_time(image);
  std::ofstream(image) << "other"; // same path and size, different revision
  std::filesystem::last_write_time(image, old_time + std::chrono::seconds(2));
  assert(revision.changed(root, {"sample"}));
  assert(!revision.changed(root, {"sample"}));
  std::filesystem::remove(image);
  assert(revision.changed(root, {"sample"}));
  std::ofstream(image) << "restored";
  assert(revision.changed(root, {"sample"}));
  std::ofstream(directory / "skin.toml") << "updated manifest";
  assert(revision.changed(root, {"sample"}));
  // 两个槽位的包合成一份指纹：任一个包被编辑都算变化，换成同一组包不算。
  const auto dark_directory = root / "dusk";
  std::filesystem::create_directories(dark_directory);
  std::ofstream(image) << "light";
  std::ofstream(dark_directory / "skin.toml") << "synthetic";
  assert(revision.changed(root, {"sample", "dusk"}));
  assert(!revision.changed(root, {"sample", "dusk"}));
  std::ofstream(dark_directory / "skin.toml") << "edited dark manifest";
  assert(revision.changed(root, {"sample", "dusk"}));
  assert(!revision.changed(root, {"sample", "dusk"}));
  std::ofstream(image) << "edited light image";
  assert(revision.changed(root, {"sample", "dusk"}));
  // 去掉深色槽位的包也让指纹变化一次。
  assert(revision.changed(root, {"sample"}));
  assert(!revision.changed(root, {"sample"}));
  std::filesystem::remove(dark_directory / "skin.toml");
  std::filesystem::remove(dark_directory);
  // Switching to a package that is not on disk, or to a theme that draws none, clears the fingerprint once.
  assert(revision.changed(root, {"absent"}));
  assert(!revision.changed(root, {"absent"}));
  assert(!revision.changed(root, {""}));
  assert(!revision.changed(root, {"../escape"}));
  std::filesystem::remove(image);
  std::filesystem::remove(directory / "images");
  std::filesystem::remove(directory / "skin.toml");
  std::filesystem::remove(directory);
  std::filesystem::remove(root);
  assert(!revision.changed(root, {"sample"}));
}
