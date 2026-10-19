#include "../../src/candidates/FcitxCanvasPanel.h"

#include <cassert>
#include <cstdlib>
#include <iostream>

namespace host = msime::linux_host;
using Json = nlohmann::json;

static std::uint32_t pixel(const std::filesystem::path &file, int x, int y) {
  auto *surface = cairo_image_surface_create_from_png(file.c_str());
  assert(cairo_surface_status(surface) == CAIRO_STATUS_SUCCESS);
  cairo_surface_flush(surface);
  std::uint32_t word;
  std::memcpy(&word, cairo_image_surface_get_data(surface) + y * cairo_image_surface_get_stride(surface) + x * 4, 4);
  cairo_surface_destroy(surface);
  return word;
}

int main() {
  char pattern[] = "/tmp/msime-fcitx-canvas-XXXXXX";
  const auto *name = mkdtemp(pattern);
  assert(name);
  const std::filesystem::path root(name), skin = root / "fixture";
  std::filesystem::create_directories(skin);
  host::FcitxCanvas canvas(6, 6, 1);
  constexpr std::uint32_t colors[] = {0xff0000, 0x00ff00, 0x0000ff, 0x00ffff, 0xff00ff, 0xffff00, 0x101010, 0x808080, 0xffffff};
  for (int y = 0; y < 6; ++y) for (int x = 0; x < 6; ++x) canvas.blend(x, y, colors[(y / 2) * 3 + x / 2], 1);
  std::ofstream(skin / "panel.png", std::ios::binary) << host::fcitx_png_encode(canvas);
  Json summary{{"id", "fixture"}, {"schemaVersion", 2}, {"canvas", true},
               {"borderImage", {{"source", "panel.png"}, {"sliceDip", {1}}, {"stretchDip", {2}},
                                 {"widthDip", {1, 3, 2, 4}}, {"fill", true}}},
               {"contentInsetsDip", {{"top", 2}, {"right", 1}, {"bottom", 3}, {"left", 6}}},
               {"outerInsetsDip", {{"top", 2}, {"right", 1}, {"bottom", 3}, {"left", 4}}}};
  auto panel = host::prepare_fcitx_canvas_panel(summary, root, "fixture", true);
  assert(panel && panel->images.size() == 2);
  assert((panel->margin == std::array<int, 4>{3, 4, 5, 8}));
  assert((panel->content == std::array<int, 4>{2, 3, 3, 6}));
  const auto theme = root / "theme/theme.conf";
  const auto colors_of = host::candidate_theme_colors(Json{{"appearance", "dark"}, {"candidate", {{"surface", "#201a30"}, {"text", "#ffffff"}}}}, true).colors;
  assert(host::write_fcitx_candidate_theme(theme, colors_of, true, std::nullopt, std::nullopt, std::nullopt, false, {}, panel));
  const auto image = theme.parent_path() / panel->file;
  const auto two = theme.parent_path() / host::fcitx_overlay_2x_name(panel->file);
  const int xs[] = {5, 8, 11}, ys[] = {2, 4, 6};
  for (int row = 0; row < 3; ++row) for (int col = 0; col < 3; ++col) {
    const auto expected = 0xff000000u | colors[row * 3 + col];
    assert(pixel(image, xs[col], ys[row]) == expected);
    assert(pixel(two, 2 * xs[col], 2 * ys[row]) == expected);
  }
  assert(pixel(image, 0, 0) == 0 && pixel(two, 1, 1) == 0);
  std::ifstream conf_input(theme);
  const std::string conf(std::istreambuf_iterator<char>(conf_input), {});
  assert(conf.find("Image=" + panel->file) != std::string::npos);
  assert(conf.find("[InputPanel/ShadowMargin]\nLeft=4\nRight=1\nTop=2\nBottom=3") != std::string::npos);
  assert(conf.find("[InputPanel/ContentMargin]\nLeft=10\nRight=4\nTop=4\nBottom=6") != std::string::npos);
  // 资源未变时不改写主题；图像变化生成新名称，提交后清理旧图。
  const auto before = std::filesystem::last_write_time(theme);
  assert(host::write_fcitx_candidate_theme(theme, colors_of, true, std::nullopt, std::nullopt, std::nullopt, false, {}, panel));
  assert(std::filesystem::last_write_time(theme) == before);
  summary["borderImage"]["fill"] = false;
  auto hollow = host::prepare_fcitx_canvas_panel(summary, root, "fixture", true);
  assert(hollow && hollow->file != panel->file);
  std::filesystem::create_directory(theme.parent_path() / hollow->file);
  assert(!host::write_fcitx_candidate_theme(theme, colors_of, true, std::nullopt, std::nullopt, std::nullopt, false, {}, hollow));
  std::ifstream failed_input(theme);
  assert(std::string(std::istreambuf_iterator<char>(failed_input), {}) == conf);
  assert(std::filesystem::exists(image) && std::filesystem::exists(two));
  std::filesystem::remove(theme.parent_path() / hollow->file);
  assert(host::write_fcitx_candidate_theme(theme, colors_of, true, std::nullopt, std::nullopt, std::nullopt, false, {}, hollow));
  assert(pixel(theme.parent_path() / hollow->file, xs[1], ys[1]) == 0);
  assert(!std::filesystem::exists(image) && !std::filesystem::exists(two));
  // 明暗槽位与损坏资源都以实际选中的源图为准，失败不能启用贴图配色。
  summary["borderImage"]["lightSource"] = "broken.png";
  std::ofstream(skin / "broken.png") << "broken";
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", false));
  assert(host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  for (const auto &path : {"../panel.png", "/panel.png", "nested/../../panel.png"}) {
    summary["borderImage"]["source"] = path;
    assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  }
  summary["borderImage"]["source"] = "linked.png";
  std::filesystem::create_symlink(skin / "panel.png", skin / "linked.png");
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  std::filesystem::remove(skin / "linked.png");
  std::filesystem::create_hard_link(skin / "panel.png", skin / "linked.png");
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  std::filesystem::remove(skin / "linked.png");
  summary["borderImage"]["source"] = "panel.png";
  summary["borderImage"]["widthDip"] = {501};
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  summary["borderImage"]["widthDip"] = {1};
  host::FcitxCanvas wide(2048, 1, 1);
  std::ofstream(skin / "wide.png", std::ios::binary) << host::fcitx_png_encode(wide);
  summary["borderImage"]["source"] = "wide.png";
  summary["borderImage"]["stretchDip"] = {0};
  summary["borderImage"]["widthDip"] = {500};
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  summary["borderImage"]["source"] = "panel.png";
  summary["borderImage"]["stretchDip"] = {2};
  summary["borderImage"]["widthDip"] = {1};
  auto oversized = host::fcitx_png_encode(canvas);
  oversized[16] = 0x7f;
  std::ofstream(skin / "panel.png", std::ios::binary) << oversized;
  assert(!host::prepare_fcitx_canvas_panel(summary, root, "fixture", true));
  assert(!host::fcitx_canvas_directory(Json{{"directory", "relative"}}));
  assert(host::fcitx_canvas_directory(Json{{"directory", root.string()}}) == root);
  std::filesystem::remove_all(root);
  std::cout << "canvas 九宫格像素、明暗、留白、更新与资源边界通过\n";
}
