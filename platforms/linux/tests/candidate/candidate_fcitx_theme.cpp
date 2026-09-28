#include "../src/candidates/CandidateFcitxTheme.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <unistd.h>
#include <vector>

namespace {

using Json = nlohmann::json;
namespace host = msime::linux_host;

bool contains(const std::string &text, const std::string &needle) {
  return text.find(needle) != std::string::npos;
}

const std::vector<host::CandidateSkin> builtin = {
    {"fluent", "Fluent"}, {"wechat", "微信"}, {"graphite", "石墨"}, {"willow_green", "柳绿"}};

host::CandidateColors resolve(Json preferences, bool system_dark = false, const Json &catalog = Json()) {
  return host::resolve_candidate_colors(
      host::candidate_display_preferences(std::move(preferences), system_dark, builtin, "fluent", catalog),
      "fluent");
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

std::vector<std::string> staged(const std::filesystem::path &directory) {
  std::vector<std::string> names;
  for (const auto &entry : std::filesystem::directory_iterator(directory)) {
    const auto name = entry.path().filename().string();
    if (name.rfind("decoration-", 0) == 0) names.push_back(name);
  }
  return names;
}

}  // namespace

int main() {
  // "follow" resolves against the system appearance, as the IBus panel does.
  const auto wechat_dark = resolve({{"candidate_skin", "wechat"}}, true);
  assert(wechat_dark.background == 0x151515u && wechat_dark.text == 0xB7B7B7u);
  assert(wechat_dark.selected == 0x07C160u && wechat_dark.selected_text == 0xFFFFFFu);
  const auto wechat_light = resolve({{"candidate_skin", "wechat"}, {"candidate_theme", "follow"}}, false);
  assert(wechat_light.background == 0xF7F7F7u);

  // A custom text colour covers the selected row too; a custom surface wins over the skin's.
  const auto custom = resolve({{"candidate_skin", "wechat"},
                               {"candidate_theme", "light"},
                               {"candidate_text_color", "#123456"},
                               {"candidate_background_color", "#abcdef"}});
  assert(custom.text == 0x123456u && custom.selected_text == 0x123456u);
  assert(custom.background == 0xABCDEFu);

  // An installed skin supplies its palette for the resolved appearance; unset colours fall back to contrast.
  const Json catalog = {{"packages",
                         Json::array({{{"id", "sakura"},
                                       {"candidate",
                                        {{"light", {{"surface", "#fff0f5"}, {"selected", "#ff69b4"}}},
                                         {"dark", {{"surface", "#301020"}, {"text", "#ffe4e1"}}}}}}})}};
  const auto sakura_light = resolve({{"candidate_skin", "sakura"}}, false, catalog);
  assert(sakura_light.background == 0xFFF0F5u && sakura_light.selected == 0xFF69B4u);
  assert(sakura_light.text == 0x000000u);
  const auto sakura_dark = resolve({{"candidate_skin", "sakura"}}, true, catalog);
  assert(sakura_dark.background == 0x301020u && sakura_dark.text == 0xFFE4E1u);
  assert(!sakura_dark.selected);
  // An installed skin sits on fluent's card, so without a border of its own it has fluent's outline.
  assert(sakura_light.border == 0xE0D3D7u && sakura_light.border_width == 1);

  // A package border is honoured, with alpha; the user's colour still wins over it; a form this host does not parse keeps fluent's.
  const Json outlined = {{"packages",
                          Json::array({{{"id", "sakura"},
                                        {"candidate",
                                         {{"light", {{"surface", "#fff0f5"}, {"border", "rgba(255,0,0,0.5)"}}},
                                          {"dark", {{"surface", "#301020"}, {"border", "#ff000080"}}}}}}})}};
  assert(resolve({{"candidate_skin", "sakura"}}, true, outlined).border == 0x980810u);
  assert(resolve({{"candidate_skin", "sakura"}, {"candidate_border_color", "#00ff00"}}, true, outlined).border == 0x00FF00u);
  assert(resolve({{"candidate_skin", "sakura"}}, false, outlined).border == 0xE0D3D7u);
  const Json hidden = {{"packages", Json::array({{{"id", "sakura"}, {"candidate", {{"light", {{"border", "transparent"}}}}}}})}};
  const auto borderless = resolve({{"candidate_skin", "sakura"}}, false, hidden);
  assert(!borderless.border && borderless.border_width == 0);
  // A built-in skin never takes a package's colours, even with a package of the same id in the catalog.
  const Json shadow = {{"packages", Json::array({{{"id", "wechat"}, {"candidate", {{"dark", {{"border", "#ff0000"}}}}}}})}};
  assert(resolve({{"candidate_skin", "wechat"}}, true, shadow).border == 0x292929u);

  // The outline is composited over the surface, since the classic UI would otherwise punch a translucent border through to the desktop.
  const auto fluent_light = resolve({{"candidate_skin", "fluent"}, {"candidate_theme", "light"}});
  assert(fluent_light.border == 0xE0E0E0u && fluent_light.border_width == 1);
  const auto fluent_dark = resolve({{"candidate_skin", "fluent"}, {"candidate_theme", "dark"}});
  assert(fluent_dark.border == 0x363636u && fluent_dark.border_width == 1);
  assert(wechat_dark.border == 0x292929u && wechat_dark.border_width == 1);
  const auto willow = resolve({{"candidate_skin", "willow_green"}, {"candidate_theme", "light"}});
  assert(!willow.border && willow.border_width == 0);
  // A custom border colour replaces the skin's but keeps its width, so willow_green stays unoutlined.
  const auto custom_border = resolve({{"candidate_skin", "graphite"}, {"candidate_theme", "light"}, {"candidate_border_color", "#123456"}});
  assert(custom_border.border == 0x123456u && custom_border.border_width == 1);
  assert(!resolve({{"candidate_skin", "willow_green"}, {"candidate_border_color", "#123456"}}).border);
  // The border is composited over a custom surface, not the skin's.
  assert(resolve({{"candidate_skin", "fluent"}, {"candidate_theme", "light"}, {"candidate_background_color", "#fff0f5"}}).border == 0xE0D3D7u);

  assert(host::candidate_border_color(Json("#ff000080"))->alpha == 0x80);
  assert(host::candidate_border_color(Json("#ABCDEF"))->rgb == 0xABCDEFu);
  assert(host::candidate_border_color(Json("transparent"))->alpha == 0);
  assert(!host::candidate_border_color(Json("rgba(0,0,0,0.1)")));
  assert(!host::candidate_border_color(Json("#12345")));
  assert(!host::candidate_border_color(Json("#1234567g")));
  assert(!host::candidate_border_color(Json(nullptr)));

  assert(host::fcitx_theme_color(0x07C160u) == "#07c160");
  assert(host::fcitx_theme_color(0xFBFBFCu, true) == "#fbfbfc00");

  const auto theme = host::fcitx_candidate_theme(wechat_dark);
  assert(contains(theme, "[InputPanel]\nNormalColor=#b7b7b7\nHighlightCandidateColor=#ffffff\n"));
  assert(contains(theme, "HighlightBackgroundColor=#07c160\n"));
  assert(contains(theme, "[InputPanel/Background]\nColor=#151515\n"));
  assert(contains(theme, "[InputPanel/Highlight]\nColor=#07c160\n"));
  assert(contains(theme, "[Menu/Background]\nColor=#151515\n"));
  assert(contains(theme, "[InputPanel/Background]\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n"));
  assert(contains(theme, "[InputPanel/Background/Margin]\nLeft=2\nRight=2\nTop=2\nBottom=2\n"));
  assert(contains(theme, "[InputPanel/ContentMargin]\nLeft=2\nRight=2\nTop=2\nBottom=2\n"));
  // No outline: a transparent border of width 0, as before borders were drawn.
  assert(contains(host::fcitx_candidate_theme(willow), "BorderColor=#f4f5f300\nBorderWidth=0\n"));
  // A wider outline widens both margins, so the border stays inside the background and the highlight off it.
  auto wide = wechat_dark;
  wide.border_width = 3;
  const auto wide_theme = host::fcitx_candidate_theme(wide);
  assert(contains(wide_theme, "BorderWidth=3\n\n[InputPanel/Background/Margin]\nLeft=4\nRight=4\nTop=4\nBottom=4\n"));
  assert(contains(wide_theme, "[InputPanel/ContentMargin]\nLeft=4\nRight=4\nTop=4\nBottom=4\n"));

  // Graphite has no selected fill: the highlight is transparent and only the text colour changes.
  const auto graphite = host::fcitx_candidate_theme(resolve({{"candidate_skin", "graphite"}, {"candidate_theme", "light"}}));
  assert(contains(graphite, "[InputPanel/Highlight]\nColor=#fbfbfc00\n"));
  assert(contains(graphite, "HighlightCandidateColor=#111827\n"));
  assert(contains(graphite, "NormalColor=#586476\n"));

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
  assert(host::write_fcitx_theme(file, graphite));
  assert(read(file) == graphite);
  assert(!std::filesystem::exists(file.string() + ".new"));
  {
    std::ofstream oversized(file, std::ios::binary | std::ios::trunc);
    oversized << std::string(8 * 1024 * 1024, 'x');
  }
  assert(host::write_fcitx_theme(file, theme));
  assert(read(file) == theme);

  // A plain skin's theme has no overlay and keeps its content margin; one with a decoration names the image, pins it top right inside the outline, and reserves the band above the candidates.
  assert(!contains(theme, "Overlay"));
  assert(!contains(theme, "Gravity"));
  const auto decorated = host::fcitx_candidate_theme(wechat_dark, host::FcitxThemeOverlay{"decoration-ab.png", 25, 15});
  assert(contains(decorated, "[InputPanel/Background]\nColor=#151515\nBorderColor=#292929\nBorderWidth=1\n"
                             "Overlay=decoration-ab.png\nGravity=Top Right\nOverlayOffsetX=1\nOverlayOffsetY=7\n"
                             "HideOverlayIfOversize=False\n\n"
                             "[InputPanel/Background/OverlayClipMargin]\nLeft=1\nRight=1\nTop=1\nBottom=1\n\n"
                             "[InputPanel/Background/Margin]\nLeft=2\nRight=2\nTop=2\nBottom=2\n\n"
                             "[InputPanel/ContentMargin]\nLeft=2\nRight=2\nTop=27\nBottom=2\n\n"));
  // Taller than the band, the image is anchored to the band's bottom and cut at the panel's top; of unknown height it starts at the band's top.
  assert(contains(host::fcitx_candidate_theme(wechat_dark, host::FcitxThemeOverlay{"decoration-ab.png", 25, 40}),
                  "OverlayOffsetY=-13\n"));
  assert(contains(host::fcitx_candidate_theme(wechat_dark, host::FcitxThemeOverlay{"decoration-ab.svg", 25, std::nullopt}),
                  "OverlayOffsetY=2\n"));
  // Without an outline the image reaches the panel's edge.
  assert(contains(host::fcitx_candidate_theme(willow, host::FcitxThemeOverlay{"decoration-ab.png", 25, 25}),
                  "OverlayOffsetX=0\nOverlayOffsetY=2\n"));
  assert(host::fcitx_png_height(png(48)) == 48);
  assert(!host::fcitx_png_height("GIF89a" + std::string(32, '\0')));
  assert(!host::fcitx_png_height(png(0)));

  // Staging copies the image next to theme.conf under a content-hash name, and the theme names that copy.
  const auto skins = root / "skins";
  std::filesystem::create_directories(skins);
  write(skins / "ears.png", png(15, 'a'));
  write(skins / "tall.PNG", png(40, 'b'));
  write(skins / "halo.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
  write(skins / "notes.txt", "not an image");
  const auto directory = file.parent_path();
  const host::CandidateSkinDecoration ears{(skins / "ears.png").string(), 24.5, 180};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, ears));
  const auto ears_theme = read(file);
  const auto ears_copy = overlay_of(ears_theme);
  assert(ears_copy.rfind("decoration-", 0) == 0 && ears_copy.size() == 11 + 16 + 4);
  assert(ears_copy.compare(ears_copy.size() - 4, 4, ".png") == 0);
  assert(read(directory / ears_copy) == png(15, 'a'));
  assert(contains(ears_theme, "OverlayOffsetY=7\n") && contains(ears_theme, "Top=27\n"));
  assert(staged(directory) == std::vector<std::string>{ears_copy});
  // An unchanged image is not written again.
  const auto copied = std::filesystem::last_write_time(directory / ears_copy);
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, ears));
  assert(std::filesystem::last_write_time(directory / ears_copy) == copied);
  // A changed image gets a new name, so the classic UI cannot keep showing the old picture, and the old copy goes.
  write(skins / "ears.png", png(15, 'c'));
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, ears));
  const auto repainted = overlay_of(read(file));
  assert(repainted != ears_copy && staged(directory) == std::vector<std::string>{repainted});

  // Switching skins removes the previous skin's image; the extension is kept lower-cased, since it picks the loader.
  const host::CandidateSkinDecoration tall{(skins / "tall.PNG").string(), 24.5, 180};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, tall));
  const auto tall_theme = read(file);
  const auto tall_copy = overlay_of(tall_theme);
  assert(tall_copy.compare(tall_copy.size() - 4, 4, ".png") == 0);
  assert(contains(tall_theme, "OverlayOffsetY=-13\n"));
  assert(staged(directory) == std::vector<std::string>{tall_copy});
  const host::CandidateSkinDecoration halo{(skins / "halo.svg").string(), 30, 100};
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, halo));
  const auto halo_theme = read(file);
  assert(contains(halo_theme, "OverlayOffsetY=2\n") && contains(halo_theme, "Top=32\n"));
  assert(staged(directory) == std::vector<std::string>{overlay_of(halo_theme)});

  // A plain skin, or an image that cannot be staged, leaves MSIME's colours with no overlay and no copy behind.
  for (const auto &unusable : {host::CandidateSkinDecoration{(skins / "notes.txt").string(), 24, 180},
                               host::CandidateSkinDecoration{(skins / "missing.png").string(), 24, 180},
                               host::CandidateSkinDecoration{skins.string(), 24, 180}}) {
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, halo));
    assert(host::write_fcitx_candidate_theme(file, wechat_dark, unusable));
    assert(read(file) == theme && staged(directory).empty());
  }
  write(skins / "empty.png", "");
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, host::CandidateSkinDecoration{(skins / "empty.png").string(), 24, 180}));
  assert(read(file) == theme && staged(directory).empty());
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, halo));
  assert(host::write_fcitx_candidate_theme(file, wechat_dark, std::nullopt));
  assert(read(file) == theme && staged(directory).empty());

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
