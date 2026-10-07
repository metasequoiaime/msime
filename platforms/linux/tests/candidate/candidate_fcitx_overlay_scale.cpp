#include "../../src/candidates/FcitxThemeOverlayScale.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <unistd.h>
#include <vector>

namespace host = msime::linux_host;

namespace {

// A synthetic opaque PNG of the given size, encoded by cairo.
std::string synthetic_png(int width, int height) {
  cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height);
  cairo_t *cr = cairo_create(surface);
  cairo_set_source_rgb(cr, 0.8, 0.2, 0.5);
  cairo_paint(cr);
  cairo_destroy(cr);
  std::string out;
  cairo_surface_write_to_png_stream(
      surface,
      [](void *closure, const unsigned char *data, unsigned int length) {
        static_cast<std::string *>(closure)->append(reinterpret_cast<const char *>(data), length);
        return CAIRO_STATUS_SUCCESS;
      },
      &out);
  cairo_surface_destroy(surface);
  return out;
}

std::optional<int> png_width(const std::string &bytes) {
  if (bytes.size() < 24) return std::nullopt;
  std::uint32_t width = 0;
  for (std::size_t index = 16; index < 20; ++index) width = (width << 8) | static_cast<unsigned char>(bytes[index]);
  return static_cast<int>(width);
}

std::string read(const std::filesystem::path &file) {
  std::ifstream in(file, std::ios::binary);
  return {std::istreambuf_iterator<char>(in), {}};
}

void write(const std::filesystem::path &file, const std::string &bytes) {
  std::ofstream(file, std::ios::binary) << bytes;
}

int offset_y(const std::string &theme) {
  const auto start = theme.find("\nOverlayOffsetY=");
  assert(start != std::string::npos);
  return std::stoi(theme.substr(start + std::string_view("\nOverlayOffsetY=").size()));
}

std::vector<std::string> decorations(const std::filesystem::path &directory) {
  std::vector<std::string> names;
  for (const auto &entry : std::filesystem::directory_iterator(directory)) {
    const auto name = entry.path().filename().string();
    if (name.rfind(host::kFcitxOverlayPrefix, 0) == 0) names.push_back(name);
  }
  std::sort(names.begin(), names.end());
  return names;
}

}  // namespace

int main() {
  // The resampler keeps the aspect ratio: a 340x448 character at the 85 dip a skin declares is 85x112, and 170x224 at 2x.
  const auto source = synthetic_png(340, 448);
  const auto one = host::scale_fcitx_overlay_png(source, 85);
  const auto two = host::scale_fcitx_overlay_png(source, 170);
  assert(one && png_width(*one) == 85 && host::fcitx_png_height(*one) == 112);
  assert(two && png_width(*two) == 170 && host::fcitx_png_height(*two) == 224);
  assert(!host::scale_fcitx_overlay_png("not a png", 85));
  assert(!host::scale_fcitx_overlay_png(source, 0));

  const auto root = std::filesystem::temp_directory_path() / ("msime-fcitx-overlay-scale-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root / "skin");
  const auto image = root / "skin" / "character.png";
  write(image, source);

  // With the scaler, the copy is the resampled image and its @2x twin, and the theme places it by the resampled height: the image's top stays inside the band (at or below the clip margin, shadow_top), so none of it is cut.
  const auto theme_dir = root / "theme";
  std::filesystem::create_directories(theme_dir);
  const host::CandidateSkinDecoration declared{image.string(), 112, 85, host::CandidateSkinAlign::right};
  const auto staged = host::stage_fcitx_overlay(theme_dir, declared, host::scale_fcitx_overlay_png);
  assert(staged && staged->height == 112 && staged->band == 112);
  const auto twin = host::fcitx_overlay_2x_name(staged->file);
  assert(png_width(read(theme_dir / staged->file)) == 85);
  assert(png_width(read(theme_dir / twin)) == 170);
  const auto conf = host::fcitx_candidate_theme_files({}, true, staged).conf;
  assert(offset_y(conf) >= host::FcitxPanelGeometry::shadow_top);

  // Without the scaler, or with no width declared, the image is copied as it is and placed by its own height, as before.
  const auto plain_dir = root / "plain";
  std::filesystem::create_directories(plain_dir);
  const auto plain = host::stage_fcitx_overlay(plain_dir, declared);
  assert(plain && plain->height == 448 && read(plain_dir / plain->file) == source);
  assert(!std::filesystem::exists(plain_dir / host::fcitx_overlay_2x_name(plain->file)));
  auto widthless = declared;
  widthless.width_dip = 0;
  const auto unscaled = host::stage_fcitx_overlay(plain_dir, widthless, host::scale_fcitx_overlay_png);
  assert(unscaled && unscaled->height == 448 && unscaled->file == plain->file);

  // A scaler that refuses leaves the original copy; a format other than PNG is never handed to it.
  const auto refused = host::stage_fcitx_overlay(plain_dir, declared, [](const std::string &, int) { return std::optional<std::string>{}; });
  assert(refused && refused->height == 448 && refused->file == plain->file);
  const auto gif = root / "skin" / "character.gif";
  write(gif, "GIF89a synthetic");
  bool asked = false;
  const auto other = host::stage_fcitx_overlay(plain_dir, {gif.string(), 112, 85, host::CandidateSkinAlign::right},
                                               [&](const std::string &, int) { asked = true; return std::optional<std::string>{}; });
  assert(other && !asked && other->file.ends_with(".gif"));

  // Writing the whole theme keeps both resampled copies and removes nothing else's.
  const auto file = theme_dir / "theme.conf";
  assert(host::write_fcitx_candidate_theme(file, {}, true, declared, std::nullopt, std::nullopt, false,
                                           host::scale_fcitx_overlay_png));
  auto expected = std::vector<std::string>{staged->file, twin};
  std::sort(expected.begin(), expected.end());
  assert(decorations(theme_dir) == expected);
  assert(offset_y(read(file)) >= host::FcitxPanelGeometry::shadow_top);
  // The stamp follows the declared width, so a skin that changes only its width is staged again.
  auto wider = declared;
  wider.width_dip = 100;
  assert(host::fcitx_overlay_stamp(wider) != host::fcitx_overlay_stamp(declared));

  std::filesystem::remove_all(root);
  return 0;
}
