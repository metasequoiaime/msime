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

// 读取 Margin 块的键值，即 classicui 九宫格不参与中心拉伸的边距。
int margin_in(const std::string &theme, const std::string &section, const std::string &key) {
  const auto header = "[" + section + "]\n";
  const auto start = theme.find(header);
  assert(start != std::string::npos);
  const auto at = theme.find(key + "=", start);
  assert(at != std::string::npos && at < theme.find("\n\n", start));
  const auto value = at + key.size() + 1;
  return std::stoi(theme.substr(value, theme.find('\n', value) - value));
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
  // The corner slices hold the 10 px corners and the shadow, which the classic UI leaves out when it places the panel on X11; the content sits macOS's 4 px inside the card's outer edge (the 1 px hairline within it), and the items take macOS's 8 px sides and 6 px above and below.
  assert(contains(theme, "[InputPanel/Background/Margin]\nLeft=24\nRight=22\nTop=24\nBottom=26\n\n"
                         "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                         "[InputPanel/ContentMargin]\nLeft=16\nRight=16\nTop=12\nBottom=20\n\n"
                         "[InputPanel/TextMargin]\nLeft=8\nRight=8\nTop=6\nBottom=6\n\n"));
  assert(contains(theme, "[InputPanel/Highlight/Margin]\nLeft=8\nRight=8\nTop=6\nBottom=6\n"));
  // 序号按候选字号的 80% 画，与 macOS 相同；不透明的卡片不开模糊。
  assert(contains(theme, "LabelTextSizeFactor=80\nEnableBlur=False\nFullWidthHighlight=True\n"));
  assert(!contains(theme, "BlurMask") && !contains(theme, "BlurMargin"));
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
  // The outline sits inside the padding, as macOS takes the larger of the two (CandidateInset): a 3 px outline keeps the 4 px padding, and one wider than the padding pushes the content in with it, so the highlight stays off the border.
  auto wide = wechat_dark;
  wide.border_width = 3;
  const auto wide_theme = host::fcitx_candidate_theme(wide, true);
  assert(contains(wide_theme, "BorderWidth=3\n\n[InputPanel/Background/Margin]\nLeft=24\nRight=22\nTop=24\nBottom=26\n"));
  assert(contains(wide_theme, "[InputPanel/ContentMargin]\nLeft=16\nRight=16\nTop=12\nBottom=20\n"));
  auto wider = wechat_dark;
  wider.border_width = 6;
  assert(contains(host::fcitx_candidate_theme(wider, true), "[InputPanel/ContentMargin]\nLeft=18\nRight=18\nTop=14\nBottom=22\n"));
  // 负的描边宽度按 0 处理：内边距和固定切片与宽度 0 时一致，不会凭空变宽。
  auto no_border = wechat_dark;
  no_border.border_width = 0;
  auto negative_border = wechat_dark;
  negative_border.border_width = -3;
  assert(host::fcitx_candidate_theme(negative_border, true) == host::fcitx_candidate_theme(no_border, true));

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
  assert(panel.width == 48 && panel.height == 52);
  assert(panel.alpha(0, 0) == 0 && panel.alpha(12, 8) < 16);
  assert(panel.alpha(23, 25) == 255 && panel.rgb(23, 25) == 0x151515u);
  assert(panel.alpha(23, 8) == 255 && panel.rgb(23, 8) == 0x292929u);
  // The shadow falls below the card more than above it, and fades out before the image's edge.
  assert(panel.alpha(23, 38) > panel.alpha(23, 6) && panel.alpha(23, 6) > 0);
  assert(panel.alpha(23, 51) <= 1 && panel.alpha(0, 25) <= 1);
  const auto doubled = image_of(panel_name.substr(0, panel_name.size() - 4) + "@2x.png");
  assert(doubled.width == 96 && doubled.height == 104 && doubled.rgb(46, 50) == 0x151515u);
  // 九宫格的固定切片必须容下整条描边：描边比圆角宽时（用户选的直角卡片、宽边框），边框色不能落进被拉伸的中心，否则整块卡片都会铺上边框色。直角卡片的角还要照常画满。
  {
    const auto stretched_center = [&](const host::CandidateColors &palette, int corner) {
      const auto card_files =
          host::fcitx_candidate_theme_files(palette, true, std::nullopt, static_cast<double>(corner));
      const auto name = image_in(card_files.conf, "InputPanel/Background");
      Decoded card;
      for (const auto &image : card_files.images)
        if (image.file == name) card = decode(image.bytes);
      const auto fill = palette.background.value_or(0xffffffu);
      const auto slice = [&](const char *key) { return margin_in(card_files.conf, "InputPanel/Background/Margin", key); };
      // 拉伸中心的每一格都只是填充色。
      for (int y = slice("Top"); y < static_cast<int>(card.height) - slice("Bottom"); ++y)
        for (int x = slice("Left"); x < static_cast<int>(card.width) - slice("Right"); ++x)
          assert(card.alpha(static_cast<std::uint32_t>(x), static_cast<std::uint32_t>(y)) == 255 &&
                 card.rgb(static_cast<std::uint32_t>(x), static_cast<std::uint32_t>(y)) == fill);
      // 描边仍在卡片最外侧的一行一列里，直角卡片的左上角也照常画满。
      assert(card.rgb(12, 12) == *palette.border);
      assert(card.rgb(static_cast<std::uint32_t>(card.width / 2), 8) == *palette.border);
      if (corner == 0) assert(card.alpha(12, 8) == 255 && card.rgb(12, 8) == *palette.border);
    };
    stretched_center(wide, 0);
    stretched_center(wide, 3);
    stretched_center(wechat_dark, 0);
  }
  // 装饰带顶端保持透明，卡片顶边随带下移；上切片对齐可能额外补齐 0–3 px，内容与阴影的偏移不变。
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
    assert(card.width == 48 && card.height == 76);
    for (std::uint32_t y = 0; y < 25; ++y)
      for (std::uint32_t x = 0; x < card.width; ++x) assert(card.alpha(x, y) == 0);
    // 两张图都不拉伸的行逐像素比较：顶部固定切片按装饰带高度平移，底部固定切片按图高差平移；上切片对齐补出的纯色行不参与比较。
    const auto slice = [](const std::string &conf, const char *key) {
      return static_cast<std::uint32_t>(margin_in(conf, "InputPanel/Background/Margin", key));
    };
    const auto top_rows = std::min(slice(theme, "Top"), slice(banded.conf, "Top") - 25);
    const auto bottom_rows = std::min(slice(theme, "Bottom"), slice(banded.conf, "Bottom"));
    for (std::uint32_t y = 0; y < panel.height; ++y) {
      if (y >= top_rows && y < panel.height - bottom_rows) continue;
      const auto shifted = y < top_rows ? y + 25 : y + card.height - panel.height;
      for (std::uint32_t x = 0; x < card.width; ++x)
        assert(card.alpha(x, shifted) == panel.alpha(x, y) &&
               (panel.alpha(x, y) == 0 || card.rgb(x, shifted) == panel.rgb(x, y)));
    }
    const auto card2x = banded_image(banded_name.substr(0, banded_name.size() - 4) + "@2x.png");
    assert(card2x.width == 96 && card2x.height == 152);
    for (std::uint32_t y = 0; y < 50; ++y)
      for (std::uint32_t x = 0; x < card2x.width; ++x) assert(card2x.alpha(x, y) == 0);
    assert(card2x.alpha(46, 66) == 255 && card2x.rgb(46, 66) == 0x292929u);
  }
  // 标识位于左上固定切片内，与首行文字对齐；切线越过标识后的安全带，内容仍按标识宽度与间距右移。
  {
    const auto square = [](int side, float red, float green, float blue) {
      host::FcitxPixels pixels{side, side, {}};
      for (int index = 0; index < side * side; ++index) pixels.rgba.insert(pixels.rgba.end(), {red, green, blue, 1.0f});
      return pixels;
    };
    const host::FcitxThemeLogo logo{square(16, 1.0f, 0.0f, 0.0f), square(32, 0.0f, 0.0f, 1.0f)};
    const auto marked = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, logo);
    assert(contains(marked.conf, "[InputPanel/Background/Margin]\nLeft=36\nRight=22\nTop=36\nBottom=26\n\n"
                                 "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                                 "[InputPanel/ContentMargin]\nLeft=38\nRight=16\nTop=12\nBottom=20\n\n"));
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
    assert(card.width == 60 && card.height == 64);
    for (std::uint32_t y = 18; y < 34; ++y)
      for (std::uint32_t x = 16; x < 32; ++x) assert(card.alpha(x, y) == 255 && card.rgb(x, y) == 0xFF0000u);
    // Around the mark is the card's own fill; the outline and the shadow are where they are without it.
    assert(card.rgb(15, 18) == 0x151515u && card.rgb(32, 18) == 0x151515u && card.rgb(16, 17) == 0x151515u && card.rgb(16, 34) == 0x151515u);
    assert(card.alpha(23, 8) == 255 && card.rgb(23, 8) == 0x292929u && card.alpha(0, 0) == 0);
    // The 2x image takes the 2x mark, at twice the position.
    const auto card2x = marked_image(marked_name.substr(0, marked_name.size() - 4) + "@2x.png");
    assert(card2x.width == 120 && card2x.height == 128);
    assert(card2x.rgb(32, 36) == 0x0000FFu && card2x.rgb(63, 67) == 0x0000FFu && card2x.rgb(64, 36) == 0x151515u);
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
    assert(contains(both.conf, "[InputPanel/Background/Margin]\nLeft=36\nRight=22\nTop=64\nBottom=26\n\n"));
    assert(contains(both.conf, "[InputPanel/ContentMargin]\nLeft=38\nRight=16\nTop=37\nBottom=20\n\n"));
    // Without a mark the theme is the one drawn before there was one.
    assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt) == theme);
  }
  // 切线要落在标识外至少两像素的底色中；左、上切线从面板原点量起，按 4 对齐，避免 1.25／1.5 倍缩放时切到半像素；右、下切线从内容决定的宽高往回量，对齐无用，保持容下阴影和圆角的原值。
  for (double scale : {0.75, 1.0, 1.25, 1.5, 2.0}) {
    for (double padding : {0.0, 4.0, 16.0}) {
      for (double corner : {0.0, 10.0, 32.0}) {
        host::FcitxCandidateStyle style;
        style.scale = scale;
        style.padding = padding;
        const auto metrics = host::fcitx_panel_metrics(style);
        const auto square = [](int side) {
          host::FcitxPixels pixels{side, side, {}};
          for (int index = 0; index < side * side; ++index) pixels.rgba.insert(pixels.rgba.end(), {0, 1, 0, 1});
          return pixels;
        };
        const host::FcitxThemeLogo logo{square(metrics.logo_side), square(2 * metrics.logo_side)};
        const auto generated = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, corner, logo,
                                                                true, std::nullopt, style);
        const auto margin = [&](const char *key) { return margin_in(generated.conf, "InputPanel/Background/Margin", key); };
        assert(margin("Left") % 4 == 0 && margin("Top") % 4 == 0);
        const auto inset = host::fcitx_card_inset(wechat_dark, style, corner, true);
        const int edge = std::max(inset.radius, wechat_dark.border_width);
        assert(margin("Right") == metrics.shadow_right + edge && margin("Bottom") == metrics.shadow_bottom + edge);
        const int logo_right = metrics.shadow_left + inset.pad + metrics.logo_side;
        const int logo_bottom = metrics.shadow_top + inset.pad + metrics.item_vertical + metrics.logo_side;
        assert(margin("Left") >= logo_right + 2 && margin("Top") >= logo_bottom + 2);
        const auto name = image_in(generated.conf, "InputPanel/Background");
        for (int density : {1, 2}) {
          const auto file = density == 1 ? name : name.substr(0, name.size() - 4) + "@2x.png";
          bool checked = false;
          for (const auto &image : generated.images) if (image.file == file) {
            checked = true;
            const auto card = decode(image.bytes);
            // 检查切线两侧的真实像素，而不只检查配置中的边距数字。
            for (int offset = -2 * density; offset <= density; ++offset) {
              const int x = margin("Left") * density + offset;
              const int y = margin("Top") * density + offset;
              for (int row = metrics.shadow_top * density; row < logo_bottom * density; ++row)
                assert(card.rgb(x, row) != 0x00FF00u);
              for (int col = metrics.shadow_left * density; col < logo_right * density; ++col)
                assert(card.rgb(col, y) != 0x00FF00u);
            }
          }
          assert(checked);
        }
      }
    }
  }
  const auto highlight = image_of(image_in(theme, "InputPanel/Highlight"));
  assert(highlight.width == 18 && highlight.height == 14);
  assert(highlight.alpha(0, 0) < 64 && highlight.alpha(9, 7) == 255 && highlight.rgb(9, 7) == 0x07C160u);
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

  // 用户的候选窗样式，照 macOS 的 StyledCandidateSkin：偏好按共享层的范围读取，超出范围的值按没设处理。
  {
    const auto style = host::read_fcitx_candidate_style(
        Json{{"candidate_scale_percent", 150}, {"candidate_opacity_percent", 80}, {"candidate_padding", 0},
             {"show_candidate_page_number", false}});
    assert(style.scale == 1.5 && style.opacity == 0.8 && style.padding == 0.0 && !style.page_arrows && !style.item_radius);
    const auto ignored = host::read_fcitx_candidate_style(
        Json{{"candidate_scale_percent", 300}, {"candidate_opacity_percent", 20}, {"candidate_padding", 17},
             {"show_candidate_page_number", "no"}});
    assert(ignored.scale == 1.0 && ignored.opacity == 1.0 && !ignored.padding && ignored.page_arrows);
    assert(host::read_fcitx_candidate_style(Json::array()).scale == 1.0);
    // 缺省样式画出的主题与不传样式时完全相同。
    assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, {}) == theme);
  }
  // 缩放乘到阴影以外的每一个长度上：圆角、内边距、行的文字边距和翻页按钮（150% 时 10 → 15、4 → 6、8 → 12、6 → 9、16 → 24）。阴影边距保持 12/12/8/16：Wayland 上 classicui 不读 ShadowMargin，阴影边就是候选窗离光标的偏移，不能随缩放变大。
  {
    host::FcitxCandidateStyle large;
    large.scale = 1.5;
    const auto scaled = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false,
                                                          std::nullopt, large);
    assert(contains(scaled.conf, "[InputPanel/Background/Margin]\nLeft=28\nRight=27\nTop=28\nBottom=31\n\n"
                                 "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                                 "[InputPanel/ContentMargin]\nLeft=18\nRight=18\nTop=14\nBottom=22\n\n"
                                 "[InputPanel/TextMargin]\nLeft=12\nRight=12\nTop=9\nBottom=9\n\n"));
    for (const auto &image : scaled.images)
      if (image.file == image_in(scaled.conf, "InputPanel/PrevPage")) {
        const auto button = decode(image.bytes);
        assert(button.width == 24 && button.height == 24 && button.alpha(9, 12) > 128 && button.alpha(15, 12) == 0);
      }
    // 自己的卡片圆角同样按缩放后的长度推导行圆角。
    assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 16.0, std::nullopt, true, large),
                    "[InputPanel/Background/Margin]\nLeft=36\nRight=36\nTop=36\nBottom=40\n"));
  }
  // 用户的内边距代替 4 px，描边仍在里面。0 px 时 macOS 的内容只离卡片外沿 1 px 发丝线，行圆角放大到与卡片同心；classicui 的行圆角最多 6 px，高亮的角会伸出 10 px 的卡片圆角，所以内边距留到 2 px（fcitx_card_inset）。
  {
    host::FcitxCandidateStyle tight;
    tight.padding = 0.0;
    assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, tight),
                    "[InputPanel/ContentMargin]\nLeft=14\nRight=14\nTop=10\nBottom=18\n"));
    tight.padding = 10.0;
    assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, tight),
                    "[InputPanel/ContentMargin]\nLeft=22\nRight=22\nTop=18\nBottom=26\n"));
    assert(host::fcitx_card_padding(wechat_dark, tight) == 10);
  }
  // 关掉页码时不画 ‹ ›：两节和两张图都不生成，classicui 也就不画翻页按钮。
  {
    host::FcitxCandidateStyle no_pages;
    no_pages.page_arrows = false;
    const auto pageless = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false,
                                                            std::nullopt, no_pages);
    assert(!contains(pageless.conf, "PrevPage") && !contains(pageless.conf, "NextPage"));
    assert(pageless.images.size() == files.images.size() - 4);
    assert(contains(pageless.conf, "[InputPanel/Highlight/Margin]\nLeft=8\nRight=8\nTop=6\nBottom=6\n\n[Menu]\n"));
  }
  // 不透明度只淡化卡片底色和描边：卡片底下的阴影被擦掉，桌面才透得出来；文字和高亮的颜色不变。KWin 按卡片形状的 BlurMask 模糊卡片后面的内容。
  {
    host::FcitxCandidateStyle faded;
    faded.opacity = 0.8;
    const auto translucent = host::fcitx_candidate_theme_files(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt,
                                                               false, std::nullopt, faded);
    assert(contains(translucent.conf, "LabelTextSizeFactor=80\nEnableBlur=True\nBlurMask=shape-"));
    assert(contains(translucent.conf, "PageButtonAlignment=Top\n\n[InputPanel/BlurMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n\n"
                                      "[InputPanel/Background]\n"));
    assert(contains(translucent.conf, "HighlightBackgroundColor=#07c160\n"));
    assert(translucent.images.size() == files.images.size() + 2);
    const auto decoded = [&](const std::string &name) {
      for (const auto &image : translucent.images)
        if (image.file == name) return decode(image.bytes);
      assert(false && "image generated");
      return Decoded{};
    };
    const auto card = decoded(image_in(translucent.conf, "InputPanel/Background"));
    assert(card.width == 48 && card.height == 52);
    // 卡片中心是 80% 的底色，靠近底边、阴影本来最浓的地方也一样，没有被阴影压暗。
    assert(card.alpha(23, 25) == 204 && card.rgb(23, 25) == 0x151515u);
    assert(card.alpha(23, 32) == 204 && card.rgb(23, 32) == 0x151515u);
    // 描边也按 80% 画在同样半透明的底色上，两者叠起来比底色稍不透明。
    assert(card.alpha(23, 8) > 204 && card.alpha(23, 8) < 255);
    // 卡片外的阴影照旧。
    assert(card.alpha(23, 38) > 0 && card.alpha(23, 38) == panel.alpha(23, 38));
    const auto mask_line = translucent.conf.find("BlurMask=");
    const auto mask_name = translucent.conf.substr(mask_line + 9, translucent.conf.find('\n', mask_line) - mask_line - 9);
    const auto mask = decoded(mask_name);
    assert(mask.width == card.width && mask.height == card.height);
    assert(mask.alpha(23, 25) == 255 && mask.alpha(0, 0) == 0 && mask.alpha(23, 40) == 0 && mask.alpha(12, 8) < 64);
  }
  // 皮肤背景图是居中贴在卡片上的叠图，裁到卡片里面（四边各收进描边和圆角弧高 3 px），上下阴影边距不等时向上挪半个差值；有装饰图时叠图让给装饰图。
  {
    const auto painted = host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, {},
                                                     host::FcitxThemeBackground{"background-ab.png"});
    assert(contains(painted, ".png\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n"
                             "Overlay=background-ab.png\nGravity=Center\nOverlayOffsetX=0\nOverlayOffsetY=-4\n"
                             "HideOverlayIfOversize=False\n\n"
                             "[InputPanel/Background/OverlayClipMargin]\nLeft=15\nRight=15\nTop=11\nBottom=19\n\n"
                             "[InputPanel/Background/Margin]\n"));
    const auto decorated_and_painted =
        host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15}, std::nullopt,
                                    std::nullopt, false, {}, host::FcitxThemeBackground{"background-ab.png"});
    assert(contains(decorated_and_painted, "Overlay=decoration-ab.png\n") && !contains(decorated_and_painted, "background-ab"));
    // 背景图按布局估的卡片大小排，随缩放变化。
    host::FcitxCandidateStyle layout;
    assert(host::fcitx_background_box(layout) == std::make_pair(240, 280));
    layout.horizontal = true;
    layout.scale = 1.5;
    assert(host::fcitx_background_box(layout) == std::make_pair(840, 120));
    assert(host::read_fcitx_candidate_style(Json{{"candidate_layout", "horizontal"}}).horizontal);
  }
  // 皮肤包 [toolbar] 的分隔线色画菜单的分隔线，没有时跟随外框。
  {
    host::CandidateMenuSlots toolbar;
    toolbar.surface = host::ThemeColor{0x141B33u, 0xFF};
    toolbar.divider = host::ThemeColor{0xFFFFFFu, 0x80};
    const auto menu = host::fcitx_menu_palette(true, toolbar);
    assert(menu.background == 0x141B33u && menu.separator == host::composite_color(0xFFFFFFu, 0x80, 0x141B33u));
  }
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
  // 只有显式的 0 关闭主题自动生成；没设、1、空串和其他值都照常生成。
  assert(host::fcitx_theme_autogenerate_enabled(nullptr) && host::fcitx_theme_autogenerate_enabled("1") &&
         host::fcitx_theme_autogenerate_enabled("") && host::fcitx_theme_autogenerate_enabled("false") &&
         host::fcitx_theme_autogenerate_enabled("00") && !host::fcitx_theme_autogenerate_enabled("0"));

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

  // A plain skin's theme has no overlay and keeps its content margin; one with a decoration names the image and makes the panel the band taller than the card: the nine-slice top margin, the shadow margin and the content margin all grow by the band (25), so the card starts at 8 + 25 = 33 and the candidates one padding (4) below that. The image's bottom is that padding below the card's top edge (33 + 4 - 15 = 22), its right edge that padding in from the card's side (12 + 4), and it may be drawn from the band's top (8) down to just inside the card's outline.
  assert(!contains(theme, "Overlay"));
  assert(!contains(theme, "Gravity"));
  const auto decorated = host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15});
  assert(contains(decorated, ".png\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n"
                             "Overlay=decoration-ab.png\nGravity=Top Right\nOverlayOffsetX=16\nOverlayOffsetY=22\n"
                             "HideOverlayIfOversize=False\n\n"
                             "[InputPanel/Background/OverlayClipMargin]\nLeft=13\nRight=13\nTop=8\nBottom=17\n\n"
                             "[InputPanel/Background/Margin]\nLeft=24\nRight=22\nTop=48\nBottom=26\n\n"
                             "[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=33\nBottom=16\n\n"
                             "[InputPanel/ContentMargin]\nLeft=16\nRight=16\nTop=37\nBottom=20\n\n"));
  // An image exactly the band plus the padding tall starts at the band's top; a taller one keeps its bottom where it is and is cut at the band's top by the clip margin; of unknown height it starts at the band's top.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 29}),
                  "OverlayOffsetY=8\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 60}),
                  "OverlayOffsetY=-23\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, host::FcitxThemeOverlay{"decoration-ab.svg", 25, std::nullopt}),
                  "OverlayOffsetY=8\n"));
  // Without an outline the image keeps the same padding, measured from where the hairline would be.
  assert(contains(host::fcitx_candidate_theme(willow, false, host::FcitxThemeOverlay{"decoration-ab.png", 25, 25}),
                  "OverlayOffsetX=16\nOverlayOffsetY=12\n"));
  // An outline wider than the padding moves the content, and the image's bottom and side with it (padding 6).
  assert(contains(host::fcitx_candidate_theme(wider, true, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15}),
                  "OverlayOffsetX=18\nOverlayOffsetY=24\n"));
  // A skin aligned to the left or centre moves the gravity, measured from that edge.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true,
                                              host::FcitxThemeOverlay{"decoration-ab.png", 25, 15, host::CandidateSkinAlign::left}),
                  "Gravity=Top Left\nOverlayOffsetX=16\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true,
                                              host::FcitxThemeOverlay{"decoration-ab.png", 25, 15, host::CandidateSkinAlign::center}),
                  "Gravity=Top Center\nOverlayOffsetX=0\n"));
  // A skin's corner radius replaces the design's 10 px in the card's corner slices; none keeps it.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 16.0),
                  "[InputPanel/Background/Margin]\nLeft=28\nRight=28\nTop=28\nBottom=32\n"));
  // 直角卡片的固定切片容下 1 px 描边，左、上切片再按 4 对齐，描边不能被拉伸进卡片中心。
  assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 0.0),
                  "[InputPanel/Background/Margin]\nLeft=16\nRight=13\nTop=16\nBottom=17\n"));
  assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt) ==
         host::fcitx_candidate_theme(wechat_dark, true));
  assert(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 16.0) != host::fcitx_candidate_theme(wechat_dark, true));
  // The user's card radius makes the selected row concentric with the card, as macOS does: the radius less the 4 px padding, never rounder than the 6 px the row's text margin leaves for the highlight's corner slices.
  const auto highlight_at = [&](double radius, bool user_radius = true) {
    return image_in(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, radius, std::nullopt, user_radius),
                    "InputPanel/Highlight");
  };
  assert(highlight_at(16.0) == image_in(theme, "InputPanel/Highlight") && highlight_at(10.0) == highlight_at(16.0));
  assert(highlight_at(8.0) != highlight_at(10.0) && highlight_at(4.0) != highlight_at(8.0) && highlight_at(0.0) == highlight_at(4.0));
  // A skin package's tight radius leaves the highlight at the 6 px it was drawn with before the setting existed.
  assert(highlight_at(0.0, false) == highlight_at(16.0) && highlight_at(3.0, false) == highlight_at(16.0));
  // 行圆角被 6 px 的文字边距截短时，内边距加大到选中高亮的角不伸出卡片圆角：对角线上高亮圆弧的最外点（pad + r(1 − 1/√2)）不越过卡片圆弧（描边半宽处起量）。用户设 32 的卡片圆角时内边距从 4 加到 8，ContentMargin 跟着变；默认的 10 px 卡片不受影响。
  {
    const double arc = 1.0 - 1.0 / std::sqrt(2.0);
    for (const bool user : {true, false})
      for (int radius = 0; radius <= 32; ++radius) {
        const auto inset = host::fcitx_card_inset(wechat_dark, {}, static_cast<double>(radius), user);
        assert(inset.pad >= 4 && inset.item_radius <= 6);
        assert(inset.pad + inset.item_radius * arc + 1e-9 >= 0.5 + (inset.radius - 0.5) * arc);
      }
    const auto round = host::fcitx_card_inset(wechat_dark, {}, 32.0, true);
    assert(round.radius == 32 && round.item_radius == 6 && round.pad == 8);
    assert(host::fcitx_card_inset(wechat_dark, {}, 24.0, false).pad == 6);
    assert(host::fcitx_card_inset(wechat_dark).pad == 4 && host::fcitx_card_inset(wechat_dark, {}, 16.0, false).pad == 4);
    assert(contains(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, 32.0, std::nullopt, true),
                    "[InputPanel/ContentMargin]\nLeft=20\nRight=20\nTop=16\nBottom=24\n"));
    assert(host::fcitx_card_padding(wechat_dark, {}, 32.0, true) == 8);
  }
  // 皮肤包自己的行圆角（item_corner_radius_dip）代替 6 px；再圆也画不出比文字边距更圆的角。
  {
    host::FcitxCandidateStyle package_rows;
    package_rows.item_radius = 2.0;
    const auto rows = [&](const host::FcitxCandidateStyle &style) {
      return image_in(host::fcitx_candidate_theme(wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, style),
                      "InputPanel/Highlight");
    };
    assert(rows(package_rows) != image_in(theme, "InputPanel/Highlight"));
    package_rows.item_radius = 16.0;
    assert(rows(package_rows) == image_in(theme, "InputPanel/Highlight"));
  }
  assert(host::fcitx_png_height(png(48)) == 48);
  assert(!host::fcitx_png_height("GIF89a" + std::string(32, '\0')));
  assert(!host::fcitx_png_height(png(0)));
  // 宽高从同一处解析：放置用得到高度，暂存时还要知道图片是否允许解码。任一边超出共享层的上限就当作无法读取，退回原图。
  assert(host::fcitx_png_width(png(48)) == 16 && host::fcitx_png_size(png(48))->height == 48);
  assert(!host::fcitx_png_width("GIF89a" + std::string(32, '\0')));
  assert(!host::fcitx_png_width(png(4800)) && !host::fcitx_png_height(png(4800)));
  assert(!host::fcitx_png_size(png(0)) && !host::fcitx_png_size(png(48).substr(0, 20)));

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
  assert(contains(ears_theme, "OverlayOffsetY=22\n") && contains(ears_theme, "Top=37\n"));
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
  assert(contains(tall_theme, "OverlayOffsetY=-3\n"));
  assert(staged(directory) == std::vector<std::string>{tall_copy});
  const host::CandidateSkinDecoration halo{(skins / "halo.svg").string(), 30, 100};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, halo));
  const auto halo_theme = read(file);
  assert(contains(halo_theme, "OverlayOffsetY=8\n") && contains(halo_theme, "Top=42\n"));
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

  // 背景图由传入的渲染器画成 1x 和 2x 两张，副本按内容哈希命名、写在 theme.conf 旁边；渲染器拿到的是按布局估的框和两层不透明度的乘积。换回没有背景图的皮肤时副本删掉。
  {
    const host::CandidateSkinBackground photo{(skins / "photo.png").string(), host::CandidateSkinBackgroundFit::contain, 0.5};
    host::FcitxCandidateStyle faded;
    faded.opacity = 0.8;
    int asked_width = 0, asked_height = 0;
    double asked_opacity = 0;
    const host::FcitxBackgroundRenderer render = [&](const host::CandidateSkinBackground &background, int width, int height,
                                                     double opacity) {
      assert(background.image == photo.image && background.fit == host::CandidateSkinBackgroundFit::contain);
      asked_width = width;
      asked_height = height;
      asked_opacity = opacity;
      return std::optional<std::pair<std::string, std::string>>{{png(10, 'x'), png(20, 'y')}};
    };
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, {},
                                             std::nullopt, faded, photo, render));
    assert(asked_width == 240 && asked_height == 280 && std::abs(asked_opacity - 0.4) < 1e-9);
    const auto copies = files_with(directory, "background-");
    assert(copies.size() == 2 && contains(read(file), "Overlay=" + copies[0] + "\n"));
    assert(read(directory / copies[0]) == png(10, 'x') && read(directory / copies[1]) == png(20, 'y'));
    // 渲染器画不出来时主题不带背景图，旧副本也不留。
    const host::FcitxBackgroundRenderer refuse = [](const host::CandidateSkinBackground &, int, int, double) {
      return std::optional<std::pair<std::string, std::string>>{};
    };
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, std::nullopt, std::nullopt, std::nullopt, false, {},
                                             std::nullopt, {}, photo, refuse));
    assert(read(file) == theme && files_with(directory, "background-").empty());
    // 装饰图画得出来时背景图让给它，渲染器都不会被叫到。
    bool rendered = false;
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, ears, std::nullopt, std::nullopt, false, {}, std::nullopt,
                                             {}, photo, [&](const host::CandidateSkinBackground &, int, int, double) {
                                               rendered = true;
                                               return std::optional<std::pair<std::string, std::string>>{};
                                             }));
    assert(!rendered && files_with(directory, "background-").empty());
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, true, std::nullopt));
  }

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
