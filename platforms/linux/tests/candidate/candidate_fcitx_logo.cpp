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

  // 候选窗缩放到 150% 时，logo 也按 24 和 48 像素从更大的图标缩放出来，仍不是空白；缩小到 50% 时是 8 和 16 像素。
  for (const auto &[scale, side] : {std::pair{1.5, 24}, std::pair{0.5, 8}}) {
    const auto scaled = host::load_fcitx_theme_logo(icons, scale);
    assert(scaled && scaled->one.width == side && scaled->one.height == side);
    assert(scaled->two.width == 2 * side && scaled->two.height == 2 * side);
    bool visible = false;
    for (std::size_t at = 3; at < scaled->two.rgba.size(); at += 4) visible = visible || scaled->two.rgba[at] > 0.5f;
    assert(visible);
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
