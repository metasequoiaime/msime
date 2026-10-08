#include "../src/candidates/CandidateFcitxTheme.h"

#include <algorithm>
#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

namespace {

using Json = nlohmann::json;
namespace host = msime::linux_host;

bool contains(const std::string &text, const std::string &needle) {
  return text.find(needle) != std::string::npos;
}

// The colours a resolved theme draws, mapped as both frontends map them.
host::CandidateColors colors_of(const Json &candidate, const char *appearance) {
  return host::candidate_theme_colors({{"appearance", appearance}, {"candidate", candidate}}, false).colors;
}

std::string read(const std::filesystem::path &file) {
  std::ifstream in(file, std::ios::binary);
  return std::string(std::istreambuf_iterator<char>(in), {});
}

void write(const std::filesystem::path &file, const std::string &bytes) {
  std::ofstream out(file, std::ios::binary | std::ios::trunc);
  out << bytes;
}

// A PNG signature and IHDR chunk header, which is as much of the image as the host reads.
std::string png(std::uint32_t height, char fill = 0) {
  std::string bytes("\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x10", 20);
  for (int shift = 24; shift >= 0; shift -= 8) bytes.push_back(static_cast<char>((height >> shift) & 0xffu));
  bytes.append(16, fill);
  return bytes;
}

// The value of the Overlay key in a theme, empty when it has none.
std::string overlay_of(const std::string &theme) {
  const auto start = theme.find("\nOverlay=");
  if (start == std::string::npos) return {};
  const auto value = start + std::string_view("\nOverlay=").size();
  return theme.substr(value, theme.find('\n', value) - value);
}

std::vector<std::string> files_with(const std::filesystem::path &directory, const std::string &prefix) {
  std::vector<std::string> names;
  for (const auto &entry : std::filesystem::directory_iterator(directory)) {
    const auto name = entry.path().filename().string();
    if (name.rfind(prefix, 0) == 0) names.push_back(name);
  }
  std::sort(names.begin(), names.end());
  return names;
}

std::vector<std::string> staged(const std::filesystem::path &directory) { return files_with(directory, "decoration-"); }

struct ShapeNameProbe {
  std::size_t reserved = 0;
  std::vector<std::string> names;

  void reserve(std::size_t count) {
    reserved = count;
    names.reserve(count);
  }

  void push_back(std::string name) { names.push_back(std::move(name)); }
};

// Every Image= value in a theme with the @2x copy of each, sorted.
std::vector<std::string> shapes_named(const std::string &theme) {
  std::vector<std::string> names;
  for (std::size_t at = theme.find("\nImage="); at != std::string::npos; at = theme.find("\nImage=", at + 1)) {
    const auto value = at + std::string_view("\nImage=").size();
    const auto name = theme.substr(value, theme.find('\n', value) - value);
    names.push_back(name);
    names.push_back(name.substr(0, name.size() - 4) + "@2x.png");
  }
  std::sort(names.begin(), names.end());
  return names;
}

// The value of the Image key of one section.
std::string image_in(const std::string &theme, const std::string &section) {
  const auto header = "[" + section + "]\nImage=";
  const auto start = theme.find(header);
  assert(start != std::string::npos);
  const auto value = start + header.size();
  return theme.substr(value, theme.find('\n', value) - value);
}

std::uint32_t be32(const std::string &bytes, std::size_t at) {
  std::uint32_t value = 0;
  for (std::size_t index = at; index < at + 4; ++index) value = (value << 8) | static_cast<unsigned char>(bytes[index]);
  return value;
}

// A decoded generated PNG. Decoding checks the chunk CRCs, the stored deflate blocks and the Adler-32 sum.
struct Decoded {
  std::uint32_t width = 0;
  std::uint32_t height = 0;
  std::string rgba;
  unsigned alpha(std::uint32_t x, std::uint32_t y) const {
    return static_cast<unsigned char>(rgba[(static_cast<std::size_t>(y) * width + x) * 4 + 3]);
  }
  std::uint32_t rgb(std::uint32_t x, std::uint32_t y) const {
    const auto at = (static_cast<std::size_t>(y) * width + x) * 4;
    return (static_cast<std::uint32_t>(static_cast<unsigned char>(rgba[at])) << 16) |
           (static_cast<std::uint32_t>(static_cast<unsigned char>(rgba[at + 1])) << 8) |
           static_cast<unsigned char>(rgba[at + 2]);
  }
};

Decoded decode(const std::string &png) {
  assert(png.compare(0, 8, std::string("\x89PNG\r\n\x1a\n", 8)) == 0);
  Decoded image;
  std::string zlib;
  for (std::size_t at = 8; at < png.size();) {
    const auto length = be32(png, at);
    const auto type = png.substr(at + 4, 4);
    assert(host::fcitx_png_crc(png.substr(at + 4, 4 + length), 0) == be32(png, at + 8 + length));
    if (type == "IHDR") {
      image.width = be32(png, at + 8);
      image.height = be32(png, at + 12);
      assert(png.substr(at + 16, 5) == std::string("\x08\x06\x00\x00\x00", 5));
    }
    if (type == "IDAT") zlib += png.substr(at + 8, length);
    at += 12 + length;
  }
  assert(zlib.size() > 6 && static_cast<unsigned char>(zlib[0]) == 0x78);
  assert((static_cast<unsigned char>(zlib[0]) * 256u + static_cast<unsigned char>(zlib[1])) % 31u == 0);
  std::string raw;
  std::size_t at = 2;
  for (bool last = false; !last;) {
    last = (zlib[at] & 1) != 0;
    assert((zlib[at] & 6) == 0);
    const unsigned length = static_cast<unsigned char>(zlib[at + 1]) | (static_cast<unsigned char>(zlib[at + 2]) << 8);
    const unsigned complement = static_cast<unsigned char>(zlib[at + 3]) | (static_cast<unsigned char>(zlib[at + 4]) << 8);
    assert((length ^ 0xffffu) == complement);
    raw += zlib.substr(at + 5, length);
    at += 5 + length;
  }
  std::uint32_t a = 1;
  std::uint32_t b = 0;
  for (const char value : raw) {
    a = (a + static_cast<unsigned char>(value)) % 65521u;
    b = (b + a) % 65521u;
  }
  assert(be32(zlib, at) == ((b << 16) | a) && at + 4 == zlib.size());
  const std::size_t stride = image.width * 4u + 1u;
  assert(raw.size() == image.height * stride);
  for (std::uint32_t y = 0; y < image.height; ++y) {
    assert(raw[y * stride] == 0);
    image.rgba += raw.substr(y * stride + 1, stride - 1);
  }
  return image;
}

}  // namespace

int main() {
  // A dark theme with its own fill and outline, and a light one drawn without an outline.
  const auto wechat_dark = colors_of({{"surface", "#151515"}, {"text", "#B7B7B7"}, {"selected", "#07C160"},
                                      {"selected_text", "#FFFFFF"}, {"border", "#292929"}},
                                     "dark");
  assert(wechat_dark.background == 0x151515u && wechat_dark.text == 0xB7B7B7u);
  assert(wechat_dark.selected == 0x07C160u && wechat_dark.selected_text == 0xFFFFFFu);
  assert(wechat_dark.border == 0x292929u && wechat_dark.border_width == 1);
  const auto willow = colors_of({{"surface", "#F4F5F3"}, {"border", "#00000000"}}, "light");
  assert(!willow.border && willow.border_width == 0);
  // A theme without a selected fill: the highlight stays transparent and only the text colour changes.
  host::CandidateColors plain;
  plain.text = 0x586476u;
  plain.number = 0x586476u;
  plain.background = 0xFBFBFCu;
  plain.selected_text = 0x111827u;
  plain.selected_number = 0x111827u;
  plain.border = 0xE5E5E5u;
  plain.border_width = 1;

  assert(host::fcitx_theme_color(0x07C160u) == "#07c160");
  assert(host::fcitx_theme_color(0xFBFBFCu, true) == "#fbfbfc00");

  const auto theme = host::fcitx_candidate_theme(wechat_dark, true);
  assert(contains(theme, "[InputPanel]\nNormalColor=#b7b7b7\nHighlightCandidateColor=#ffffff\n"));
  assert(contains(theme, "HighlightBackgroundColor=#07c160\n"));
  // The shapes are drawn from nine-slice images, with the flat colours kept for a classic UI that cannot load them.
  assert(contains(theme, "[InputPanel/Background]\nImage=shape-"));
  assert(contains(theme, ".png\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n\n"));
  assert(contains(theme, "[InputPanel/Highlight]\nImage=shape-"));
  assert(contains(theme, ".png\nColor=#07c160\n"));
  // The corner slices hold the 10 px corners and the shadow, which the classic UI leaves out when it places the panel on X11; the content sits a 1 px outline plus 6 px padding inside the card, and the items take the design's padding.
  assert(contains(theme, "[InputPanel/Background/Margin]\nLeft=22\nRight=22\nTop=22\nBottom=26\n\n"
                         "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                         "[InputPanel/ContentMargin]\nLeft=19\nRight=19\nTop=15\nBottom=23\n\n"
                         "[InputPanel/TextMargin]\nLeft=10\nRight=12\nTop=6\nBottom=6\n\n"));
  assert(contains(theme, "[InputPanel/Highlight/Margin]\nLeft=10\nRight=12\nTop=6\nBottom=6\n"));
  // The design's ‹ › page buttons: the classic UI draws them only when both images load, in the header row where the release reads PageButtonAlignment, and the whole image is the click target.
  assert(contains(theme, "FullWidthHighlight=True\nPageButtonAlignment=Top\n\n[InputPanel/Background]\n"));
  assert(contains(theme, "[InputPanel/PrevPage]\nImage=shape-"));
  assert(contains(theme, ".png\n\n[InputPanel/PrevPage/ClickMargin]\nLeft=0\nRight=0\nTop=0\nBottom=0\n\n[InputPanel/NextPage]\nImage=shape-"));
  assert(contains(theme, ".png\n\n[InputPanel/NextPage/ClickMargin]\nLeft=0\nRight=0\nTop=0\nBottom=0\n\n"));
  assert(image_in(theme, "InputPanel/PrevPage") != image_in(theme, "InputPanel/NextPage"));
  // fcitx5-gtk 的客户端输入面板用 GKeyFile 读这份主题，顶层键会让它整份拒绝、退回 default，所以文件必须以分组开头。
  assert(theme.rfind("[Metadata]\n", 0) == 0);
  // @2x 图的倍率写成分组下的 Value（fcitx/fcitx5#1695 之后的写法），不写顶层的 SupportedScale=2。
  assert(contains(theme, "ScaleWithDPI=True\n\n[SupportedScale]\nValue=2\n\n[InputPanel]\n"));
  assert(!contains(theme, "SupportedScale="));
  // No outline: a transparent border of width 0, as before borders were drawn.
  assert(contains(host::fcitx_candidate_theme(willow, false), "BorderColor=#f4f5f300\nBorderWidth=0\n"));
  // A wider outline pushes the content in with it, so the highlight stays off the border.
  auto wide = wechat_dark;
  wide.border_width = 3;
  const auto wide_theme = host::fcitx_candidate_theme(wide, true);
  assert(contains(wide_theme, "BorderWidth=3\n\n[InputPanel/Background/Margin]\nLeft=22\nRight=22\nTop=22\nBottom=26\n"));
  assert(contains(wide_theme, "[InputPanel/ContentMargin]\nLeft=21\nRight=21\nTop=17\nBottom=25\n"));

  const auto unfilled = host::fcitx_candidate_theme(plain, false);
  assert(contains(unfilled, "[InputPanel/Highlight]\nColor=#fbfbfc00\n"));
  assert(contains(unfilled, "HighlightCandidateColor=#111827\n"));
  assert(contains(unfilled, "NormalColor=#586476\n"));
  // The number colours reach Fcitx5 releases that draw the label on its own; older ones ignore the keys.
  assert(contains(unfilled, "CandidateLabelColor=#586476\nHighlightCandidateLabelColor=#111827\n"));
  host::CandidateColors bare;
  bare.background = 0xFFFFFFu;
  assert(!contains(host::fcitx_candidate_theme(bare, false), "LabelColor"));

  // Without theme slots the menu takes the Linux menu tokens for the appearance: hover is 6% black or 8% white under unchanged text.
  assert(contains(unfilled, "[Menu]\nNormalColor=#2e2e2e\nHighlightCandidateColor=#2e2e2e\nSpacing=0\n\n[Menu/Background]\nImage=shape-"));
  assert(contains(unfilled, ".png\nColor=#ffffff\nBorderColor=#ebebeb\nBorderWidth=1\n\n[Menu/Background/Margin]\nLeft=12\nRight=12\nTop=12\nBottom=12\n\n"
                            "[Menu/ContentMargin]\nLeft=6\nRight=6\nTop=6\nBottom=6\n\n[Menu/Highlight]\nImage=shape-"));
  assert(contains(unfilled, ".png\nColor=#f0f0f0\n\n[Menu/Highlight/Margin]\nLeft=6\nRight=6\nTop=8\nBottom=8\n"));
  assert(contains(unfilled, ".png\nColor=#e5e5e5\n\n[Menu/CheckBox]\nImage=shape-"));
  assert(contains(unfilled, "[Menu/SubMenu]\nImage=shape-"));
  // A theme's candidate palette draws the menu (THEME_CONTRACT §3): surface, text and border are its own, and the hover it leaves null is the native 8% white over its surface.
  assert(contains(theme, "[Menu]\nNormalColor=#b7b7b7\nHighlightCandidateColor=#b7b7b7\n"));
  assert(contains(theme, ".png\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n\n[Menu/Background/Margin]"));
  assert(contains(theme, ".png\nColor=#272727\n\n[Menu/Highlight/Margin]"));
  assert(contains(theme, ".png\nColor=#292929\n\n[Menu/CheckBox]"));
  // `system` resolves to a null palette, so the menu is the native one.
  const auto system_menu = host::fcitx_menu_palette(true, colors_of(nullptr, "dark").menu);
  assert(system_menu.background == 0x383838u && system_menu.text == 0xFFFFFFu && system_menu.hover == 0x484848u &&
         system_menu.separator == 0x242424u && system_menu.ring_stroke.rgb == 0 && system_menu.ring_stroke.alpha == 0x14);
  // Translucent slots are composited over the menu's own surface; the outline keeps its alpha for the image stroke.
  const auto night = colors_of({{"surface", "#16262F"}, {"text", "#E0E6EA"}, {"hover", "#FFFFFF1A"}, {"border", "#FFFFFF14"}},
                               "dark");
  const auto night_menu = host::fcitx_menu_palette(true, night.menu);
  assert(night_menu.background == 0x16262Fu && night_menu.text == 0xE0E6EAu && night_menu.hover == 0x2E3C44u);
  assert(night_menu.separator == 0x28373Fu && night_menu.ring == 0x28373Fu);
  assert(night_menu.ring_stroke.rgb == 0xFFFFFFu && night_menu.ring_stroke.alpha == 0x14);
  // A fully transparent border draws no outline; the separators keep the native hairline over the theme's surface.
  const auto willow_menu = host::fcitx_menu_palette(false, willow.menu);
  assert(willow_menu.background == 0xF4F5F3u && willow_menu.text == 0x2C2C2Cu && willow_menu.hover == 0xE6E7E5u);
  assert(willow_menu.separator == 0xDBDCDAu && willow_menu.ring == 0xF4F5F3u && willow_menu.ring_stroke.alpha == 0);

  // The images: every name the theme gives is generated, with an @2x copy, and they decode as the shapes they stand for.
  const auto files = host::fcitx_candidate_theme_files(wechat_dark, true);
  assert(files.conf == theme);
  ShapeNameProbe shape_names;
  host::fcitx_collect_shape_names(files, shape_names);
  assert(shape_names.reserved == files.images.size());
  assert(shape_names.names.size() == files.images.size());
  std::vector<std::string> generated;
  for (const auto &image : files.images) generated.push_back(image.file);
  std::sort(generated.begin(), generated.end());
  assert(generated == shapes_named(theme) && generated.size() == 18);
  assert(host::fcitx_candidate_theme_files(plain, false).images.size() == 16);
  const auto image_of = [&](const std::string &name) {
    for (const auto &image : files.images)
      if (image.file == name) return decode(image.bytes);
    assert(false && "image generated");
    return Decoded{};
  };
  const auto panel_name = image_in(theme, "InputPanel/Background");
  const auto panel = image_of(panel_name);
  assert(panel.width == 46 && panel.height == 50);
  assert(panel.alpha(0, 0) == 0 && panel.alpha(12, 8) < 16);
  assert(panel.alpha(23, 25) == 255 && panel.rgb(23, 25) == 0x151515u);
  assert(panel.alpha(23, 8) == 255 && panel.rgb(23, 8) == 0x292929u);
  // The shadow falls below the card more than above it, and fades out before the image's edge.
  assert(panel.alpha(23, 36) > panel.alpha(23, 6) && panel.alpha(23, 6) > 0);
  assert(panel.alpha(23, 49) <= 1 && panel.alpha(0, 25) <= 1);
  const auto doubled = image_of(panel_name.substr(0, panel_name.size() - 4) + "@2x.png");
  assert(doubled.width == 92 && doubled.height == 100 && doubled.rgb(46, 50) == 0x151515u);
  // A decoration's band makes the panel image taller by its height: those rows are fully transparent, and below them the shadow margin and the card are the same as without a decoration.
  {
    const auto banded = host::fcitx_candidate_theme_files(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15});
    const auto banded_name = image_in(banded.conf, "InputPanel/Background");
    const auto banded_image = [&](const std::string &name) {
      for (const auto &image : banded.images)
        if (image.file == name) return decode(image.bytes);
      assert(false && "image generated");
      return Decoded{};
    };
    const auto card = banded_image(banded_name);
    assert(card.width == 46 && card.height == 75);
    for (std::uint32_t y = 0; y < 25; ++y)
      for (std::uint32_t x = 0; x < card.width; ++x) assert(card.alpha(x, y) == 0);
    for (std::uint32_t y = 0; y < 50; ++y)
      for (std::uint32_t x = 0; x < card.width; ++x)
        assert(card.alpha(x, y + 25) == panel.alpha(x, y) && (panel.alpha(x, y) == 0 || card.rgb(x, y + 25) == panel.rgb(x, y)));
    const auto card2x = banded_image(banded_name.substr(0, banded_name.size() - 4) + "@2x.png");
    assert(card2x.width == 92 && card2x.height == 150);
    for (std::uint32_t y = 0; y < 50; ++y)
      for (std::uint32_t x = 0; x < card2x.width; ++x) assert(card2x.alpha(x, y) == 0);
    assert(card2x.alpha(46, 66) == 255 && card2x.rgb(46, 66) == 0x292929u);
  }
  // The brand mark: painted into the card image's top-left corner slice at the content's top-left corner and the first row's text top, with the slices grown to hold it whole, and the content moved right by the mark and its gap. Nothing else in the theme changes.
  {
    const auto square = [](int side, float red, float green, float blue) {
      host::FcitxPixels pixels{side, side, {}};
      for (int index = 0; index < side * side; ++index) pixels.rgba.insert(pixels.rgba.end(), {red, green, blue, 1.0f});
      return pixels;
    };
    const host::FcitxThemeLogo logo{square(16, 1.0f, 0.0f, 0.0f), square(32, 0.0f, 0.0f, 1.0f)};
    const auto marked = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, logo);
    assert(contains(marked.conf, "[InputPanel/Background/Margin]\nLeft=35\nRight=22\nTop=37\nBottom=26\n\n"
                                 "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                                 "[InputPanel/ContentMargin]\nLeft=41\nRight=19\nTop=15\nBottom=23\n\n"));
    assert(marked.images.size() == files.images.size());
    const auto marked_name = image_in(marked.conf, "InputPanel/Background");
    assert(marked_name != panel_name);
    const auto marked_image = [&](const std::string &name) {
      for (const auto &image : marked.images)
        if (image.file == name) return decode(image.bytes);
      assert(false && "image generated");
      return Decoded{};
    };
    const auto card = marked_image(marked_name);
    assert(card.width == 59 && card.height == 65);
    for (std::uint32_t y = 21; y < 37; ++y)
      for (std::uint32_t x = 19; x < 35; ++x) assert(card.alpha(x, y) == 255 && card.rgb(x, y) == 0xFF0000u);
    // Around the mark is the card's own fill; the outline and the shadow are where they are without it.
    assert(card.rgb(18, 21) == 0x151515u && card.rgb(35, 21) == 0x151515u && card.rgb(19, 20) == 0x151515u && card.rgb(19, 37) == 0x151515u);
    assert(card.alpha(23, 8) == 255 && card.rgb(23, 8) == 0x292929u && card.alpha(0, 0) == 0);
    // The 2x image takes the 2x mark, at twice the position.
    const auto card2x = marked_image(marked_name.substr(0, marked_name.size() - 4) + "@2x.png");
    assert(card2x.width == 118 && card2x.height == 130);
    assert(card2x.rgb(38, 42) == 0x0000FFu && card2x.rgb(69, 73) == 0x0000FFu && card2x.rgb(70, 42) == 0x151515u);
    // A mark that is not opaque is composited over the card.
    auto faint = logo;
    for (auto *pixels : {&faint.one, &faint.two})
      for (std::size_t at = 0; at < pixels->rgba.size(); at += 4) {
        pixels->rgba[at] = 0.5f;
        pixels->rgba[at + 1] = pixels->rgba[at + 2] = 0.0f;
        pixels->rgba[at + 3] = 0.5f;
      }
    const auto blended = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, faint);
    const auto blended_name = image_in(blended.conf, "InputPanel/Background");
    for (const auto &image : blended.images)
      if (image.file == blended_name) {
        const auto mixed = decode(image.bytes);
        // Half of 0x80 red over half of the 0x15 fill: 0x8a red, and the fill's 0x0a or 0x0b in the others as the half rounds.
        const auto mix = mixed.rgb(20, 22);
        assert(mixed.alpha(20, 22) == 255 && (mix >> 16) == 0x8Au);
        assert(((mix >> 8) & 0xFFu) >= 0x0Au && ((mix >> 8) & 0xFFu) <= 0x0Bu && (mix & 0xFFu) >= 0x0Au && (mix & 0xFFu) <= 0x0Bu);
      }
    // A decoration's band moves the mark down with the card.
    const auto both = host::fcitx_candidate_theme_files(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15}, std::nullopt, logo);
    assert(contains(both.conf, "[InputPanel/Background/Margin]\nLeft=35\nRight=22\nTop=62\nBottom=26\n\n"));
    assert(contains(both.conf, "[InputPanel/ContentMargin]\nLeft=41\nRight=19\nTop=40\nBottom=23\n\n"));
    // Without a mark the theme is the one drawn before there was one.
    assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt) == theme);
  }
  const auto highlight = image_of(image_in(theme, "InputPanel/Highlight"));
  assert(highlight.width == 24 && highlight.height == 14);
  assert(highlight.alpha(0, 0) < 64 && highlight.alpha(12, 7) == 255 && highlight.rgb(12, 7) == 0x07C160u);
  const auto check = image_of(image_in(theme, "Menu/CheckBox"));
  assert(check.width == 22 && check.height == 16 && check.alpha(0, 0) == 0 && check.alpha(6, 11) > 128);
  // The page chevrons are drawn in the number colour, the design's secondary text (the native 55% ink where the theme sets none, as here): ‹ points left, › right.
  const auto prev_page = image_of(image_in(theme, "InputPanel/PrevPage"));
  const auto next_page = image_of(image_in(theme, "InputPanel/NextPage"));
  assert(prev_page.width == 16 && prev_page.height == 16 && next_page.width == 16 && next_page.height == 16);
  assert(prev_page.alpha(0, 0) == 0 && prev_page.alpha(6, 8) > 128 && prev_page.alpha(10, 8) == 0);
  assert(next_page.alpha(0, 0) == 0 && next_page.alpha(9, 8) > 128 && next_page.alpha(6, 8) == 0);
  assert(wechat_dark.number && wechat_dark.number != wechat_dark.text);
  assert(prev_page.rgb(6, 8) == *wechat_dark.number && next_page.rgb(9, 8) == *wechat_dark.number);
  const auto menu_background = image_of(image_in(theme, "Menu/Background"));
  assert(menu_background.width == 26 && menu_background.height == 26 && menu_background.rgb(13, 13) == 0x151515u);

  assert(host::fcitx_theme_replaceable(""));
  assert(host::fcitx_theme_replaceable("default"));
  assert(host::fcitx_theme_replaceable("default-dark"));
  assert(host::fcitx_theme_replaceable("msime"));
  assert(!host::fcitx_theme_replaceable("Nord-Dark"));
  // A user's own DarkTheme is left in place, so MSIME's colours are not drawn in dark mode even with a stock light theme.
  const std::string stock_dark = "default-dark";
  const std::string user_dark = "Nord-Dark";
  assert(host::fcitx_candidate_theme_drawn("default", nullptr));
  assert(host::fcitx_candidate_theme_drawn("default", &stock_dark));
  assert(!host::fcitx_candidate_theme_drawn("default", &user_dark));
  assert(!host::fcitx_candidate_theme_drawn("Nord-Dark", &stock_dark));

  assert(host::fcitx_theme_file("/data", "/home/u") == std::filesystem::path("/data/fcitx5/themes/msime/theme.conf"));
  assert(host::fcitx_theme_file("relative", "/home/u") ==
         std::filesystem::path("/home/u/.local/share/fcitx5/themes/msime/theme.conf"));
  assert(host::fcitx_theme_file(nullptr, "/home/u") ==
         std::filesystem::path("/home/u/.local/share/fcitx5/themes/msime/theme.conf"));
  assert(!host::fcitx_theme_file(nullptr, nullptr));

  char pattern[] = "/tmp/msime-fcitx-theme-XXXXXX";
  const std::filesystem::path root = mkdtemp(pattern);
  const auto file = *host::fcitx_theme_file(root.c_str(), nullptr);
  assert(host::write_fcitx_theme(file, theme));
  assert(read(file) == theme);
  const auto written = std::filesystem::last_write_time(file);
  assert(host::write_fcitx_theme(file, theme));
  assert(std::filesystem::last_write_time(file) == written);
  assert(host::write_fcitx_theme(file, unfilled));
  assert(read(file) == unfilled);
  assert(!std::filesystem::exists(file.string() + ".new"));
  std::filesystem::remove(file);
  assert(::mkfifo(file.c_str(), 0600) == 0);
  assert(host::write_fcitx_theme(file, theme));
  assert(read(file) == theme);
  {
    const auto outside = std::filesystem::path(root) / "outside-theme.conf";
    write(outside, "keep");
    const auto staging = file.string() + ".new";
    std::filesystem::create_symlink(outside, staging);
    assert(host::write_fcitx_theme(file, theme));
    assert(read(outside) == "keep");
    assert(read(file) == theme);
  }
  {
    std::ofstream oversized(file, std::ios::binary | std::ios::trunc);
    oversized << std::string(8 * 1024 * 1024, 'x');
  }
  assert(host::write_fcitx_theme(file, theme));
  assert(read(file) == theme);

  // A plain skin's theme has no overlay and keeps its content margin; one with a decoration names the image and makes the panel the band taller than the card: the nine-slice top margin, the shadow margin and the content margin all grow by the band (25), so the card starts at 8 + 25 = 33 and the candidates one padding (7) below that. The image's bottom is that padding below the card's top edge (33 + 7 - 15 = 25), its right edge that padding in from the card's side (12 + 7), and it may be drawn from the band's top (8) down to just inside the card's outline.
  assert(!contains(theme, "Overlay"));
  assert(!contains(theme, "Gravity"));
  const auto decorated = host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15});
  assert(contains(decorated, ".png\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n"
                             "Overlay=decoration-ab.png\nGravity=Top Right\nOverlayOffsetX=19\nOverlayOffsetY=25\n"
                             "HideOverlayIfOversize=False\n\n"
                             "[InputPanel/Background/OverlayClipMargin]\nLeft=13\nRight=13\nTop=8\nBottom=17\n\n"
                             "[InputPanel/Background/Margin]\nLeft=22\nRight=22\nTop=47\nBottom=26\n\n"
                             "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=33\nBottom=16\n\n"
                             "[InputPanel/ContentMargin]\nLeft=19\nRight=19\nTop=40\nBottom=23\n\n"));
  // An image exactly the band plus the padding tall starts at the band's top; a taller one keeps its bottom where it is and is cut at the band's top by the clip margin; of unknown height it starts at the band's top.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 32}),
                  "OverlayOffsetY=8\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 60}),
                  "OverlayOffsetY=-20\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.svg", 25, std::nullopt}),
                  "OverlayOffsetY=8\n"));
  // Without an outline the image keeps the same padding, measured from where the hairline would be.
  assert(contains(host::fcitx_candidate_theme(willow, false, host::FcitxThemeOverlay{"decoration-ab.png", 25, 25}),
                  "OverlayOffsetX=19\nOverlayOffsetY=15\n"));
  // A wider outline moves the content, and the image's bottom and side with it (inset 3, padding 9).
  assert(contains(host::fcitx_candidate_theme(wide, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15}),
                  "OverlayOffsetX=21\nOverlayOffsetY=27\n"));
  // A skin aligned to the left or centre moves the gravity, measured from that edge.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true,
                                              host::FcitxThemeOverlay{"decoration-ab.png", 25, 15, host::CandidateSkinAlign::left}),
                  "Gravity=Top Left\nOverlayOffsetX=19\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true,
                                              host::FcitxThemeOverlay{"decoration-ab.png", 25, 15, host::CandidateSkinAlign::center}),
                  "Gravity=Top Center\nOverlayOffsetX=0\n"));
  // A skin's corner radius replaces the design's 10 px in the card's corner slices; none keeps it.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 16.0),
                  "[InputPanel/Background/Margin]\nLeft=28\nRight=28\nTop=28\nBottom=32\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 0.0),
                  "[InputPanel/Background/Margin]\nLeft=12\nRight=12\nTop=12\nBottom=16\n"));
  assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt) ==
         host::fcitx_candidate_theme(wechat_dark, true));
  assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 16.0) != host::fcitx_candidate_theme(wechat_dark, true));
  // The selected row keeps its 6 px corners on any card at least that round, and follows a tighter card the user chose down to square.
  const auto highlight_at = [&](double radius, bool user_radius = true) {
    return image_in(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, radius, std::nullopt, user_radius),
                    "InputPanel/Highlight");
  };
  assert(highlight_at(16.0) == image_in(theme, "InputPanel/Highlight") && highlight_at(6.0) == highlight_at(16.0));
  assert(highlight_at(3.0) != highlight_at(6.0) && highlight_at(0.0) != highlight_at(3.0));
  // A skin package's tight radius leaves the highlight at the 6 px it was drawn with before the setting existed.
  assert(highlight_at(0.0, false) == highlight_at(16.0) && highlight_at(3.0, false) == highlight_at(16.0));
  assert(host::fcitx_png_height(png(48)) == 48);
  assert(!host::fcitx_png_height("GIF89a" + std::string(32, '\0')));
  assert(!host::fcitx_png_height(png(0)));

  // Writing the theme writes the images it names beside it, and a theme in other colours replaces them.
  const auto directory = file.parent_path();
  assert(host::write_fcitx_candidate_theme(file, plain, false, std::nullopt));
  assert(read(file) == unfilled && files_with(directory, "shape-") == shapes_named(unfilled));
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, std::nullopt));
  assert(read(file) == theme && files_with(directory, "shape-") == shapes_named(theme));
  for (const auto &image : files.images) assert(read(directory / image.file) == image.bytes);

  // Staging copies the image next to theme.conf under a content-hash name, and the theme names that copy.
  const auto skins = root / "skins";
  std::filesystem::create_directories(skins);
  write(skins / "ears.png", png(15, 'a'));
  write(skins / "tall.PNG", png(40, 'b'));
  write(skins / "halo.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
  write(skins / "notes.txt", "not an image");
  const host::CandidateSkinDecoration ears{(skins / "ears.png").string(), 24.5, 180};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, ears));
  const auto ears_theme = read(file);
  const auto ears_copy = overlay_of(ears_theme);
  assert(ears_copy.rfind("decoration-", 0) == 0 && ears_copy.size() == 11 + 16 + 4);
  assert(ears_copy.compare(ears_copy.size() - 4, 4, ".png") == 0);
  assert(read(directory / ears_copy) == png(15, 'a'));
  assert(contains(ears_theme, "OverlayOffsetY=25\n") && contains(ears_theme, "Top=40\n"));
  assert(staged(directory) == std::vector<std::string>{ears_copy});
  assert(files_with(directory, "shape-") == shapes_named(ears_theme) && shapes_named(ears_theme) != shapes_named(theme));
  // An unchanged image is not written again.
  const auto copied = std::filesystem::last_write_time(directory / ears_copy);
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, ears));
  assert(std::filesystem::last_write_time(directory / ears_copy) == copied);
  // A changed image gets a new name, so the classic UI cannot keep showing the old picture, and the old copy goes.
  write(skins / "ears.png", png(15, 'c'));
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, ears));
  const auto repainted = overlay_of(read(file));
  assert(repainted != ears_copy && staged(directory) == std::vector<std::string>{repainted});

  // Switching skins removes the previous skin's image; the extension is kept lower-cased, since it picks the loader.
  const host::CandidateSkinDecoration tall{(skins / "tall.PNG").string(), 24.5, 180};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, tall));
  const auto tall_theme = read(file);
  const auto tall_copy = overlay_of(tall_theme);
  assert(tall_copy.compare(tall_copy.size() - 4, 4, ".png") == 0);
  assert(contains(tall_theme, "OverlayOffsetY=0\n"));
  assert(staged(directory) == std::vector<std::string>{tall_copy});
  const host::CandidateSkinDecoration halo{(skins / "halo.svg").string(), 30, 100};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, halo));
  const auto halo_theme = read(file);
  assert(contains(halo_theme, "OverlayOffsetY=8\n") && contains(halo_theme, "Top=45\n"));
  assert(staged(directory) == std::vector<std::string>{overlay_of(halo_theme)});

  // A plain skin, or an image that cannot be staged, leaves MSIME's colours with no overlay and no copy behind.
  for (const auto &unusable : {host::CandidateSkinDecoration{(skins / "notes.txt").string(), 24, 180},
                               host::CandidateSkinDecoration{(skins / "missing.png").string(), 24, 180},
                               host::CandidateSkinDecoration{skins.string(), 24, 180}}) {
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, halo));
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, unusable));
    assert(read(file) == theme && staged(directory).empty());
  }
  write(skins / "empty.png", "");
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, host::CandidateSkinDecoration{(skins / "empty.png").string(), 24, 180}));
  assert(read(file) == theme && staged(directory).empty());
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, halo));
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, std::nullopt));
  assert(read(file) == theme && staged(directory).empty());
  assert(files_with(directory, "shape-") == shapes_named(theme));

  const auto outside = root / "outside-theme-state";
  const auto linked = root / "linked-theme-state";
  std::filesystem::create_directory(outside);
  std::filesystem::create_directory_symlink(outside, linked);
  // 以 root 身份运行时（Linux 容器里就是这样），root 自己不对外开放的目录里的链接会被当成受信任的系统链接（见 `src/core/SafePath.h`）；把目录改成其他人可写，这条链接就成了任何人都可能放进去的链接。
  std::filesystem::permissions(root, std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  const auto linked_file = linked / "new-dir" / "theme.conf";
  assert(!host::write_fcitx_candidate_theme(linked_file, plain, false, std::nullopt));
  assert(!std::filesystem::exists(outside / "new-dir"));

  // The stamp stands in for the image between refreshes: nothing without a decoration, and a different one once the image changes.
  assert(host::fcitx_overlay_stamp(std::nullopt).empty());
  const auto before = host::fcitx_overlay_stamp(ears);
  assert(contains(before, (skins / "ears.png").string()));
  assert(host::fcitx_overlay_stamp(ears) == before);
  write(skins / "ears.png", png(15, 'c') + "longer");
  assert(host::fcitx_overlay_stamp(ears) != before);
  std::filesystem::remove_all(root);
  return 0;
}
