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
  // 共享像素是非预乘 RGBA，Cairo 必须得到本机字节序的预乘 ARGB。
  host::FcitxCanvas alpha(2, 1, 1);
  alpha.blend(0, 0, 0x2864a0, 128.0f / 255);
  auto *decoded = host::decode_fcitx_canvas_image(host::fcitx_png_encode(alpha));
  assert(decoded && cairo_image_surface_get_width(decoded) == 2);
  cairo_surface_flush(decoded);
  std::uint32_t words[2];
  std::memcpy(words, cairo_image_surface_get_data(decoded), sizeof(words));
  assert(words[0] == 0x80143250 && words[1] == 0);
  cairo_surface_destroy(decoded);
  assert(!host::decode_fcitx_canvas_image("broken"));
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

  // 皮肤背景图按 fit 画进框里：一张 4x2 的图，左半红、右半蓝，放进 20x20 的框。
  {
    host::FcitxCanvas photo(4, 2, 1);
    for (int y = 0; y < 2; ++y) for (int x = 0; x < 4; ++x) photo.blend(x, y, x < 2 ? 0xff0000 : 0x0000ff, 1);
    const auto source = skin / "background.png";
    std::ofstream(source, std::ios::binary) << host::fcitx_png_encode(photo);
    const auto rendered = [&](host::CandidateSkinBackgroundFit fit, double opacity) {
      const auto result = host::render_fcitx_background({source.string(), fit, 1.0}, 20, 20, opacity);
      assert(result);
      const auto one = root / "background-1x.png";
      const auto two = root / "background-2x.png";
      std::ofstream(one, std::ios::binary) << result->first;
      std::ofstream(two, std::ios::binary) << result->second;
      auto *doubled = cairo_image_surface_create_from_png(two.c_str());
      assert(cairo_image_surface_get_width(doubled) == 40 && cairo_image_surface_get_height(doubled) == 40);
      cairo_surface_destroy(doubled);
      return one;
    };
    // cover 铺满整个框，两边裁掉：左上角是红色，右上角是蓝色，四角都不透明。
    const auto cover = rendered(host::CandidateSkinBackgroundFit::cover, 1.0);
    assert(pixel(cover, 1, 1) == 0xffff0000u && pixel(cover, 18, 1) == 0xff0000ffu && pixel(cover, 1, 18) >> 24 == 0xff);
    // contain 完整放进框里，上下留空：顶上一行透明，中间一行是图。
    const auto contain = rendered(host::CandidateSkinBackgroundFit::contain, 1.0);
    assert(pixel(contain, 10, 1) == 0 && pixel(contain, 1, 10) == 0xffff0000u && pixel(contain, 18, 10) == 0xff0000ffu);
    // stretch 两个方向各自拉满；不透明度乘进 alpha。
    const auto stretch = rendered(host::CandidateSkinBackgroundFit::stretch, 0.5);
    assert(pixel(stretch, 1, 1) >> 24 >= 0x7f && pixel(stretch, 1, 1) >> 24 <= 0x80 && pixel(stretch, 1, 18) >> 24 >= 0x7f);
    // 读不到或解不开的图不画。
    assert(!host::render_fcitx_background({(skin / "missing.png").string()}, 20, 20, 1.0));
    std::ofstream(skin / "broken.png", std::ios::binary) << "not an image";
    assert(!host::render_fcitx_background({(skin / "broken.png").string()}, 20, 20, 1.0));
    assert(!host::render_fcitx_background({source.string()}, 0, 20, 1.0));
  }
  // 自由挂件：整个落在角切片里的画进面板图（Wayland 上 classicui 是唯一画法），跨过拉伸区的不画，免得被九宫格拉变形。
  {
    const std::array<int, 4> margin{3, 4, 5, 8};
    // 面板图 28x15，外留白上 2、右 1、下 3、左 4，卡片是中间那块。
    const std::array<double, 4> card{4, 2, 23, 10};
    Json corner{{"anchor", "top-left"}, {"offsetXDip", -4}, {"offsetYDip", -2}, {"widthDip", 3}, {"heightDip", 2}};
    const auto box = host::fcitx_canvas_widget_box(corner, card, 28, 15, margin);
    assert(box && (*box == std::array<double, 4>{0, 0, 3, 2}));
    Json bottom_right{{"anchor", "bottom-right"}, {"offsetXDip", 1}, {"offsetYDip", 3}, {"widthDip", 3}, {"heightDip", 3}};
    assert(host::fcitx_canvas_widget_box(bottom_right, card, 28, 15, margin));
    Json centred{{"anchor", "top"}, {"widthDip", 3}, {"heightDip", 2}};
    assert(!host::fcitx_canvas_widget_box(centred, card, 28, 15, margin));
    Json outside{{"anchor", "top-left"}, {"offsetXDip", -9}, {"widthDip", 3}, {"heightDip", 2}};
    assert(!host::fcitx_canvas_widget_box(outside, card, 28, 15, margin));
    host::FcitxCanvas widget(3, 2, 1);
    for (int y = 0; y < 2; ++y) for (int x = 0; x < 3; ++x) widget.blend(x, y, 0x123456, 1);
    std::ofstream(skin / "widget.png", std::ios::binary) << host::fcitx_png_encode(widget);
    // 上面的边界用例把 panel.png 换成了坏图，这里放回合成的九宫格。
    std::ofstream(skin / "panel.png", std::ios::binary) << host::fcitx_png_encode(canvas);
    Json decorated{{"id", "fixture"}, {"schemaVersion", 2}, {"canvas", true},
                   {"borderImage", {{"source", "panel.png"}, {"sliceDip", {2}}, {"widthDip", {2}}, {"fill", true}}},
                   {"outerInsetsDip", {{"top", 2}, {"right", 2}, {"bottom", 2}, {"left", 4}}},
                   {"decorations", Json::array({Json{{"image", "widget.png"}, {"anchor", "top-left"}, {"offsetXDip", -4},
                                                     {"offsetYDip", -2}, {"widthDip", 3}, {"heightDip", 2}, {"zIndex", 2}},
                                                Json{{"image", "widget.png"}, {"anchor", "top"}, {"widthDip", 3}, {"heightDip", 2}}})}};
    const auto with_widget = host::prepare_fcitx_canvas_panel(decorated, root, "fixture", true);
    assert(with_widget);
    const auto file = root / "widget-panel.png";
    std::ofstream(file, std::ios::binary) << with_widget->images[0].bytes;
    assert(pixel(file, 1, 1) == 0xff123456u);
    decorated["decorations"][0]["image"] = "missing.png";
    const auto without = host::prepare_fcitx_canvas_panel(decorated, root, "fixture", true);
    assert(without && without->file != with_widget->file);
  }
  std::filesystem::remove_all(root);
  std::cout << "canvas 九宫格像素、明暗、留白、角上挂件、更新与资源边界通过\n";
}
