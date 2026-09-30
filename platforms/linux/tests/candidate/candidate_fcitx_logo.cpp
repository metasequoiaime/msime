#include "../../src/candidates/FcitxThemeLogo.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <unistd.h>

namespace host = msime::linux_host;

int main(int argc, char **argv) {
  assert(argc == 2);
  const std::filesystem::path icons = argv[1];
  // The icons the package installs decode at 1x and 2x: square, the sizes the theme draws, premultiplied values in range, and not blank.
  const auto logo = host::load_fcitx_theme_logo(icons);
  assert(logo);
  for (const auto *pixels : {&logo->one, &logo->two}) {
    const auto side = pixels == &logo->one ? 16 : 32;
    assert(pixels->width == side && pixels->height == side);
    assert(pixels->rgba.size() == static_cast<std::size_t>(side * side * 4));
    bool opaque = false;
    for (std::size_t at = 0; at < pixels->rgba.size(); at += 4) {
      const auto alpha = pixels->rgba[at + 3];
      assert(alpha >= 0.0f && alpha <= 1.0f);
      for (int channel = 0; channel < 3; ++channel) assert(pixels->rgba[at + channel] >= 0.0f && pixels->rgba[at + channel] <= alpha + 1e-6f);
      opaque = opaque || alpha == 1.0f;
    }
    assert(opaque);
  }

  // A missing size, a file that is not a PNG, or an icon of the wrong size leaves the theme without a mark.
  const auto root = std::filesystem::temp_directory_path() / ("msime-fcitx-logo-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  assert(!host::load_fcitx_theme_logo(root));
  std::filesystem::create_directories(root / "16x16/apps");
  std::filesystem::create_directories(root / "32x32/apps");
  std::filesystem::copy_file(icons / "16x16/apps/msime-linux.png", root / "16x16/apps/msime-linux.png");
  assert(!host::load_fcitx_theme_logo(root));
  std::ofstream(root / "32x32/apps/msime-linux.png", std::ios::binary) << "not a png";
  assert(!host::load_fcitx_theme_logo(root));
  std::filesystem::copy_file(icons / "16x16/apps/msime-linux.png", root / "32x32/apps/msime-linux.png",
                             std::filesystem::copy_options::overwrite_existing);
  assert(!host::load_fcitx_theme_logo(root));
  std::filesystem::copy_file(icons / "32x32/apps/msime-linux.png", root / "32x32/apps/msime-linux.png",
                             std::filesystem::copy_options::overwrite_existing);
  assert(host::load_fcitx_theme_logo(root));
  std::filesystem::remove_all(root);
  return 0;
}
