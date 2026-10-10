#include "../../src/candidates/FcitxThemeOverlayScale.h"

#include <cassert>
#include <cstdint>
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

// 只有指定尺寸的 PNG 签名和 IHDR，没有像素数据，宿主能读头但 cairo 必须拒绝解码。
std::string png_header(std::uint32_t width, std::uint32_t height) {
  std::string bytes("\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR", 16);
  for (const auto side : {width, height})
    for (int shift = 24; shift >= 0; shift -= 8) bytes.push_back(static_cast<char>((side >> shift) & 0xff));
  bytes.append(16, '\0');
  return bytes;
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

// 读取主题的 Overlay 键值。
std::string overlay_of(const std::string &theme) {
  const auto start = theme.find("\nOverlay=");
  assert(start != std::string::npos);
  const auto value = start + std::string_view("\nOverlay=").size();
  return theme.substr(value, theme.find('\n', value) - value);
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
  // 无边框卡片的内边距：1 px 发丝线加 6 px 内容边距，暂存与放置 overlay 用的是同一个值（见 fcitx_card_padding）。
  const auto pad = host::fcitx_card_padding({});
  // The resampler keeps the aspect ratio: a 340x448 character at the 85 dip a skin declares is 85x112, and 170x224 at 2x.
  const auto source = synthetic_png(340, 448);
  const auto one = host::scale_fcitx_overlay_png(source, 85, 0);
  const auto two = host::scale_fcitx_overlay_png(source, 170, 0);
  assert(one && host::fcitx_png_width(*one) == 85 && host::fcitx_png_height(*one) == 112);
  assert(two && host::fcitx_png_width(*two) == 170 && host::fcitx_png_height(*two) == 224);
  assert(!host::scale_fcitx_overlay_png("not a png", 85, 0));
  assert(!host::scale_fcitx_overlay_png(source, 0, 0));
  // 宽度留出的高度超过房间高度时以房间为准继续等比缩小（200 → 76 宽，正好 100 高）；房间不构成限制或为 0 时只按宽度缩放。
  const auto fitted = host::scale_fcitx_overlay_png(source, 200, 100);
  assert(fitted && host::fcitx_png_width(*fitted) == 76 && host::fcitx_png_height(*fitted) == 100);
  assert(host::scale_fcitx_overlay_png(source, 85, 119) == one);

  // 文件头里的尺寸是唯一不解码就能读到的部分：任一边超出共享层上限就拒绝，cairo 不会为它分配像素。
  assert(host::fcitx_png_width(source) == 340 && host::fcitx_png_height(source) == 448);
  assert(host::fcitx_png_size(source) && host::fcitx_png_size(source)->height == 448);
  const auto biggest = host::fcitx_png_size(png_header(2048, 2048));
  assert(biggest && biggest->width == 2048 && biggest->height == 2048);
  assert(!host::fcitx_png_size(png_header(0, 10)) && !host::fcitx_png_size(png_header(10, 0)));
  assert(!host::fcitx_png_width(png_header(2048, 4096)) && !host::fcitx_png_height(png_header(4096, 2048)));
  assert(!host::fcitx_png_size("GIF89a" + std::string(32, '\0')));

  const auto root = std::filesystem::temp_directory_path() / ("msime-fcitx-overlay-scale-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root / "skin");
  const auto image = root / "skin" / "character.png";
  write(image, source);

  // With the scaler, the copy is the resampled image and its @2x twin, and the theme places it by the resampled height: the image's top stays inside the band (at or below the clip margin, shadow_top), so none of it is cut.
  const auto theme_dir = root / "theme";
  std::filesystem::create_directories(theme_dir);
  const host::CandidateSkinDecoration declared{image.string(), 112, 85, host::CandidateSkinAlign::right};
  const auto staged = host::stage_fcitx_overlay(theme_dir, declared, pad,
                                                host::scale_fcitx_overlay_png);
  assert(staged && staged->height == 112 && staged->band == 112);
  const auto twin = host::fcitx_overlay_2x_name(staged->file);
  assert(host::fcitx_png_width(read(theme_dir / staged->file)) == 85);
  assert(host::fcitx_png_width(read(theme_dir / twin)) == 170);
  const auto conf = host::fcitx_candidate_theme_files({}, true, staged).conf;
  assert(offset_y(conf) >= host::FcitxPanelGeometry::shadow_top);
  // 缩放后的副本不高过房间（预留高度加卡片内边距），底边正好在卡片顶边之下一个内边距。
  assert(staged->height <= staged->band + pad);
  assert(offset_y(conf) + *staged->height ==
         host::FcitxPanelGeometry::shadow_top + staged->band + pad);

    // A skin resource replaced by a symlink must not redirect the host outside the package.
  const auto outside = root / "outside.png";
  write(outside, source);
  const auto linked = root / "skin" / "linked.png";
  std::filesystem::create_symlink(outside, linked);
  assert(!host::stage_fcitx_overlay(theme_dir,
                                    {linked.string(), 112, 85, host::CandidateSkinAlign::right}, pad));
  const auto hardlinked = root / "skin" / "hardlinked.png";
  std::filesystem::create_hard_link(outside, hardlinked);
  assert(!host::stage_fcitx_overlay(theme_dir,
                                    {hardlinked.string(), 112, 85, host::CandidateSkinAlign::right}, pad));
  assert(std::filesystem::exists(outside));
  std::filesystem::remove(hardlinked);

  // Without the scaler, or with no width declared, the image is copied as it is and placed by its own height, as before.
  const auto plain_dir = root / "plain";
  std::filesystem::create_directories(plain_dir);
  const auto plain = host::stage_fcitx_overlay(plain_dir, declared, pad);
  assert(plain && plain->height == 448 && read(plain_dir / plain->file) == source);
  assert(!std::filesystem::exists(plain_dir / host::fcitx_overlay_2x_name(plain->file)));
  auto widthless = declared;
  widthless.width_dip = 0;
  const auto unscaled = host::stage_fcitx_overlay(plain_dir, widthless, pad,
                                                  host::scale_fcitx_overlay_png);
  assert(unscaled && unscaled->height == 448 && unscaled->file == plain->file);

  // A scaler that refuses leaves the original copy; a format other than PNG is never handed to it.
  const auto refused = host::stage_fcitx_overlay(plain_dir, declared, pad,
                                                 [](const std::string &, int, int) { return std::optional<std::string>{}; });
  assert(refused && refused->height == 448 && refused->file == plain->file);
  // 1x 缩放成功但 @2x 失败时，两者都退回原图，主题不能引用缺少配对副本的缩放图。
  bool scaled_once = false;
  const auto half_scaled = host::stage_fcitx_overlay(
      plain_dir, declared, pad, [&](const std::string &png, int width, int room) {
        if (scaled_once) return std::optional<std::string>{};
        scaled_once = true;
        return host::scale_fcitx_overlay_png(png, width, room);
      });
  assert(half_scaled && half_scaled->height == 448 && half_scaled->file == plain->file);
  assert(!std::filesystem::exists(plain_dir / host::fcitx_overlay_2x_name(half_scaled->file)));
  const auto gif = root / "skin" / "character.gif";
  write(gif, "GIF89a synthetic");
  bool asked = false;
  const auto other = host::stage_fcitx_overlay(plain_dir, {gif.string(), 112, 85, host::CandidateSkinAlign::right},
                                               pad,
                                               [&](const std::string &, int, int) { asked = true; return std::optional<std::string>{}; });
  assert(other && !asked && other->file.ends_with(".gif"));

  // 声明尺寸超过上限的 PNG 根本不会交给缩放器，能读出文件头但 cairo 解不开的 PNG 缩放会失败：两种都退回原图，装饰不丢，只是按原尺寸放置。
  for (const auto &name : {std::string("oversized.png"), std::string("broken.png")}) {
    const auto file = root / "skin" / name;
    write(file, name == "oversized.png" ? png_header(4096, 10) : png_header(340, 448));
    const auto fallback_dir = root / ("fallback-" + name);
    std::filesystem::create_directories(fallback_dir);
    bool tried = false;
    const auto copied = host::stage_fcitx_overlay(
        fallback_dir, {file.string(), 121, 64, host::CandidateSkinAlign::right}, pad,
        [&](const std::string &, int, int) { tried = true; return std::optional<std::string>{}; });
    assert(copied && !std::filesystem::exists(fallback_dir / host::fcitx_overlay_2x_name(copied->file)));
    assert(read(fallback_dir / copied->file) == read(file));
    assert(name == "oversized.png" ? !tried : tried);
  }

  // 声明宽度会让图高超过顶带加内边距时继续缩小：顶边落在裁剪边距内，@2x 的两边尺寸恰好翻倍。
  {
    const auto banner_image = root / "skin" / "banner.png";
    write(banner_image, synthetic_png(64, 512));
    const auto banner_dir = root / "banner";
    std::filesystem::create_directories(banner_dir);
    const auto file = banner_dir / "theme.conf";
    // 121 的预留加 7 的内边距（无边框时 1 px 发丝线加 6 px）：房间高 128，所以 64 宽的 64x512 原图缩放成 16x128，@2x 正好 32x256。
    const host::CandidateSkinDecoration banner{banner_image.string(), 121, 64, host::CandidateSkinAlign::right};
    assert(host::write_fcitx_candidate_theme(file, {}, true, banner, std::nullopt, std::nullopt, false,
                                             host::scale_fcitx_overlay_png));
    const auto banner_theme = read(file);
    assert(offset_y(banner_theme) == host::FcitxPanelGeometry::shadow_top);
    const auto copy = overlay_of(banner_theme);
    const auto copy_size = host::fcitx_png_size(read(banner_dir / copy));
    const auto twin_size = host::fcitx_png_size(read(banner_dir / host::fcitx_overlay_2x_name(copy)));
    assert(copy_size && copy_size->width == 16 && copy_size->height == 128);
    assert(twin_size && twin_size->width == 32 && twin_size->height == 256);
    assert(decorations(banner_dir) == std::vector<std::string>({copy, host::fcitx_overlay_2x_name(copy)}));
  }

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
