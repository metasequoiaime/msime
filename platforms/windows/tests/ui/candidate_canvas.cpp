#include "CandidateCanvas.h"
#include "CandidateCardSize.h"
#include "CandidateSkin.h"
#include "CandidateSkinAssets.h"
#include <chrono>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
using Json = nlohmann::json;
namespace {
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate canvas validation failed");
}
bool near(double value, double expected) { return std::fabs(value - expected) < 0.001; }
bool same(const CandidateCanvasEdges &edges, double top, double right, double bottom, double left) {
  return near(edges.top, top) && near(edges.right, right) && near(edges.bottom, bottom) &&
         near(edges.left, left);
}
bool same(const CandidateCanvasRect &rect, double left, double top, double right, double bottom) {
  return near(rect.left, left) && near(rect.top, top) && near(rect.right, right) &&
         near(rect.bottom, bottom);
}
// msime_client_skin_catalog 的一项 canvas 包（SkinSummary 的 camelCase）；数值和文件都是合成的。
Json canvas_package(const char *id) {
  return Json{{"id", id},
              {"name", "Canvas"},
              {"version", "1.0"},
              {"base", "system"},
              {"canvas", true},
              {"layouts", Json::array({"horizontal", "vertical"})},
              {"themes", Json::array({"dark", "light"})},
              {"minWidthDip", 0.0},
              {"borderImage",
               Json{{"source", "assets/panel-dark.png"},
                    {"lightSource", "assets/panel-light.png"},
                    {"sliceDip", Json::array({16.0})},
                    {"widthDip", Json::array({8.0})},
                    {"fill", true}}},
              {"contentInsetsDip", Json{{"top", 10.0}, {"right", 14.0}, {"bottom", 10.0}, {"left", 14.0}}},
              {"outerInsetsDip", Json{{"top", 18.0}, {"right", 18.0}, {"bottom", 12.0}, {"left", 10.0}}},
              {"lineGapDip", 5.0},
              {"decorations",
               Json::array({Json{{"image", "assets/moon.png"},
                                 {"anchor", "top-right"},
                                 {"offsetXDip", 18.0},
                                 {"offsetYDip", -18.0},
                                 {"widthDip", 40.0},
                                 {"heightDip", 40.0},
                                 {"zIndex", -1}},
                            Json{{"image", "assets/cloud.png"},
                                 {"anchor", "bottom"},
                                 {"offsetXDip", 40.0},
                                 {"offsetYDip", 10.0},
                                 {"widthDip", 48.0},
                                 {"heightDip", 20.0},
                                 {"zIndex", 1}}})}};
}
CandidateCanvasDecoration widget(CandidateCanvasAnchor anchor, double x, double y, double width,
                                 double height, int z = 2, const wchar_t *image = L"w.png") {
  CandidateCanvasDecoration decoration;
  decoration.image = image;
  decoration.anchor = anchor;
  decoration.offset_x = x;
  decoration.offset_y = y;
  decoration.width = width;
  decoration.height = height;
  decoration.z_index = z;
  return decoration;
}
} // namespace

int main() {
  // CSS 四值简写：1、2、4 个值，其余个数四边都是 0。
  require(same(candidate_canvas_box({5.0}), 5.0, 5.0, 5.0, 5.0));
  require(same(candidate_canvas_box({1.0, 2.0}), 1.0, 2.0, 1.0, 2.0));
  require(same(candidate_canvas_box({1.0, 2.0, 3.0, 4.0}), 1.0, 2.0, 3.0, 4.0));
  require(same(candidate_canvas_box({1.0, 2.0, 3.0}), 0.0, 0.0, 0.0, 0.0));
  require(same(candidate_canvas_box({}), 0.0, 0.0, 0.0, 0.0));
  // 九个锚点名，别的都不认。
  require(candidate_canvas_anchor("top-left") == CandidateCanvasAnchor::top_left &&
          candidate_canvas_anchor("bottom-right") == CandidateCanvasAnchor::bottom_right &&
          candidate_canvas_anchor("center") == CandidateCanvasAnchor::center &&
          candidate_canvas_anchor("right") == CandidateCanvasAnchor::right &&
          !candidate_canvas_anchor("middle") && !candidate_canvas_anchor("Top"));

  // 从 catalog 读画布：图片按包内相对路径解析成绝对路径，文件要在。
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-candidate-canvas-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  for (const char *id : {"starry", "stretched", "broken"}) {
    std::filesystem::create_directories(root / id / "assets");
    for (const char *file : {"panel-dark.png", "panel-light.png", "moon.png", "cloud.png"})
      std::ofstream(root / id / "assets" / file) << "synthetic";
  }
  {
    const Json catalog{{"packages", Json::array({canvas_package("starry")})}};
    const auto canvas = candidate_skin_canvas(catalog, "starry", root);
    require(canvas.active && canvas.border);
    require(canvas.border->source == (root / "starry" / "assets" / "panel-dark.png").wstring());
    require(canvas.border->light_source ==
            (root / "starry" / "assets" / "panel-light.png").wstring());
    require(same(canvas.border->slice, 16.0, 16.0, 16.0, 16.0) &&
            same(canvas.border->width, 8.0, 8.0, 8.0, 8.0) && canvas.border->fill);
    require(canvas.content_insets && same(*canvas.content_insets, 10.0, 14.0, 10.0, 14.0));
    require(canvas.outer_insets && same(*canvas.outer_insets, 18.0, 18.0, 12.0, 10.0));
    require(canvas.line_gap && near(*canvas.line_gap, 5.0));
    require(canvas.decorations.size() == 2);
    const auto &moon = canvas.decorations[0];
    require(moon.image == (root / "starry" / "assets" / "moon.png").wstring() &&
            moon.anchor == CandidateCanvasAnchor::top_right && near(moon.offset_x, 18.0) &&
            near(moon.offset_y, -18.0) && near(moon.width, 40.0) && near(moon.height, 40.0) &&
            moon.z_index == -1);
    require(canvas.decorations[1].anchor == CandidateCanvasAnchor::bottom &&
            canvas.decorations[1].z_index == 1);
    // 资源经 candidate_skin_assets 一起带上。
    const auto assets = candidate_skin_assets(catalog, "starry", root);
    require(assets.canvas && assets.canvas_skin.active && assets.canvas_skin.decorations.size() == 2);
    // 不是 canvas 包（catalog 不会给出这些键，即使给了也不读）、不在目录里的包，画布都是空的。
    auto plain = canvas_package("starry");
    plain.erase("canvas");
    const Json plain_catalog{{"packages", Json::array({plain})}};
    require(!candidate_skin_canvas(plain_catalog, "starry", root).active);
    require(!candidate_skin_canvas(catalog, "absent", root).active);
    require(!candidate_skin_assets(plain_catalog, "starry", root).canvas_skin.active);
  }
  {
    // stretch_dip 盖过 slice_dip，没写 width_dip 时边宽取盖过之后的切片；fill = false；没写 light_source。
    auto package = canvas_package("stretched");
    package["borderImage"] = Json{{"source", "assets/panel-dark.png"},
                                  {"sliceDip", Json::array({20.0})},
                                  {"stretchDip", Json::array({12.0, 24.0})},
                                  {"fill", false}};
    const Json catalog{{"packages", Json::array({package})}};
    const auto canvas = candidate_skin_canvas(catalog, "stretched", root);
    require(canvas.border && canvas.border->light_source.empty() && !canvas.border->fill);
    require(same(canvas.border->slice, 12.0, 24.0, 12.0, 24.0) &&
            same(canvas.border->width, 12.0, 24.0, 12.0, 24.0));
  }
  {
    // 读不懂的面板整个不要；读不懂的挂件只丢那一个；读不懂的内边距和行距当作没写。
    auto package = canvas_package("broken");
    package["borderImage"]["source"] = "assets/missing.png";
    package["contentInsetsDip"].erase("left");
    package["outerInsetsDip"]["top"] = 900.0;
    package["lineGapDip"] = 120.0;
    auto &decorations = package["decorations"];
    decorations.push_back(Json{{"image", "../starry/assets/moon.png"}, {"widthDip", 10.0}, {"heightDip", 10.0}});
    decorations.push_back(Json{{"image", "assets/moon.png"}, {"anchor", "middle"}, {"widthDip", 10.0}, {"heightDip", 10.0}});
    decorations.push_back(Json{{"image", "assets/moon.png"}, {"offsetXDip", 2400.0}, {"widthDip", 10.0}, {"heightDip", 10.0}});
    decorations.push_back(Json{{"image", "assets/moon.png"}, {"widthDip", 0.0}, {"heightDip", 10.0}});
    decorations.push_back(Json{{"image", "assets/moon.png"}, {"widthDip", 10.0}, {"heightDip", 10.0}, {"zIndex", 1.5}});
    decorations.push_back(Json{{"image", "assets/moon.png"}, {"widthDip", 10.0}, {"heightDip", 10.0}, {"zIndex", 2000}});
    // 没写锚点、偏移和层级时：贴右边、不偏移、z_index 2。
    decorations.push_back(Json{{"image", "assets/cloud.png"}, {"widthDip", 12.0}, {"heightDip", 6.0}});
    const Json catalog{{"packages", Json::array({package})}};
    const auto canvas = candidate_skin_canvas(catalog, "broken", root);
    require(canvas.active && !canvas.border && !canvas.content_insets && !canvas.outer_insets &&
            !canvas.line_gap);
    require(canvas.decorations.size() == 3);
    const auto &plain = canvas.decorations[2];
    require(plain.anchor == CandidateCanvasAnchor::right && near(plain.offset_x, 0.0) &&
            near(plain.offset_y, 0.0) && plain.z_index == 2 && near(plain.width, 12.0));
    // 一个包最多 32 个挂件。
    auto many = canvas_package("starry");
    many["decorations"] = Json::array();
    for (int index = 0; index < 40; ++index)
      many["decorations"].push_back(Json{{"image", "assets/moon.png"}, {"widthDip", 4.0}, {"heightDip", 4.0}});
    require(candidate_skin_canvas(Json{{"packages", Json::array({many})}}, "starry", root)
                .decorations.size() == 32);
  }
  std::filesystem::remove_all(root);

  // 这一种明暗下要画的画布：浅色有 light_source 时用它；解不出来的面板留空、挂件丢掉。
  CandidateSkinCanvas manifest;
  manifest.active = true;
  manifest.border = CandidateCanvasBorder{L"dark.png", L"light.png", {16, 16, 16, 16}, {8, 8, 8, 8}, true};
  manifest.decorations = {widget(CandidateCanvasAnchor::top_left, 0, 0, 10, 10, 2, L"ok.png"),
                          widget(CandidateCanvasAnchor::top_left, 0, 0, 10, 10, 2, L"bad.png")};
  const auto all = [](const std::wstring &) { return true; };
  const auto light = candidate_canvas_drawable(manifest, true, all);
  require(light.border->source == L"light.png" && light.border->light_source.empty() &&
          candidate_canvas_panel_drawn(light));
  require(candidate_canvas_drawable(manifest, false, all).border->source == L"dark.png");
  auto only_dark = manifest;
  only_dark.border->light_source.clear();
  require(candidate_canvas_drawable(only_dark, true, all).border->source == L"dark.png");
  const auto failing = candidate_canvas_drawable(
      manifest, true, [](const std::wstring &path) { return path == L"ok.png"; });
  require(failing.active && failing.border && failing.border->source.empty() &&
          !candidate_canvas_panel_drawn(failing));
  require(failing.decorations.size() == 1 && failing.decorations[0].image == L"ok.png");
  // 版式不因为图解不出来而变：边宽照旧。
  require(same(failing.border->width, 8.0, 8.0, 8.0, 8.0));
  require(!candidate_canvas_drawable(CandidateSkinCanvas{}, true, all).active &&
          !candidate_canvas_panel_drawn(CandidateSkinCanvas{}));

  // 内容内边距：写了 content_insets 时每边不小于边宽，减去宿主自己的内边距；没写时就是边宽；不是 canvas 时为 0。
  const auto metrics = candidate_card_metrics(16.0, 16.0, true);
  const auto base = candidate_canvas_base_padding(metrics);
  require(same(base, 4.0, 4.0, 4.0, 4.0));
  CandidateSkinCanvas insets = manifest;
  insets.content_insets = CandidateCanvasEdges{10.0, 14.0, 4.0, 3.0};
  require(same(candidate_canvas_content_offset(insets, base), 6.0, 10.0, 4.0, 4.0));
  require(same(candidate_canvas_content_offset(manifest, base), 8.0, 8.0, 8.0, 8.0));
  CandidateSkinCanvas bare;
  bare.active = true;
  require(same(candidate_canvas_content_offset(bare, base), 0.0, 0.0, 0.0, 0.0));
  // 只写了内边距、没有面板：可以比宿主的内边距小，内容往外挪。
  bare.content_insets = CandidateCanvasEdges{2.0, 2.0, 2.0, 2.0};
  require(same(candidate_canvas_content_offset(bare, base), -2.0, -2.0, -2.0, -2.0));
  require(same(candidate_canvas_content_offset(CandidateSkinCanvas{}, base), 0.0, 0.0, 0.0, 0.0));
  // 皮肤的 padding 改了宿主的内边距，基准跟着变。
  CandidateSkinMetrics padded;
  padded.padding = 3.0;
  require(same(candidate_canvas_base_padding(candidate_card_metrics(16.0, 16.0, true, true, true, padded)),
               3.0, 3.0, 3.0, 3.0));

  // line_gap_dip 落在拼音行的下外边距上，盖过版式表写的；没有 canvas 时不动。
  CandidateSkinMetrics skin;
  skin.preedit_margin = {1.0, 2.0, 3.0, 4.0};
  CandidateSkinCanvas gap;
  gap.active = true;
  gap.line_gap = 7.0;
  const auto gapped = candidate_canvas_skin_metrics(skin, gap);
  require(near(gapped.preedit_margin.bottom, 7.0) && near(gapped.preedit_margin.top, 1.0) &&
          near(gapped.preedit_margin.left, 4.0));
  require(near(candidate_canvas_skin_metrics(skin, CandidateSkinCanvas{}).preedit_margin.bottom, 3.0));
  gap.active = false;
  require(near(candidate_canvas_skin_metrics(skin, gap).preedit_margin.bottom, 3.0));
  // 显示拼音时拼音行因此长高，候选往下挪。
  const auto with_gap = candidate_card_metrics(16.0, 16.0, true, true, true, gapped);
  require(near(with_gap.preedit_row, 16.0 * 1.4 + 6.0 + 1.0 + 7.0));

  // 挂件的框：九个锚点、偏移正数向右向下。
  const CandidateCanvasRect panel{10.0, 20.0, 210.0, 120.0};
  using A = CandidateCanvasAnchor;
  require(same(candidate_canvas_decoration_box(widget(A::top_left, 0, 0, 40, 20), panel), 10, 20, 50, 40));
  require(same(candidate_canvas_decoration_box(widget(A::top_right, 18, -18, 40, 40), panel), 188, 2, 228, 42));
  require(same(candidate_canvas_decoration_box(widget(A::bottom_left, -14, 12, 32, 32), panel), -4, 100, 28, 132));
  require(same(candidate_canvas_decoration_box(widget(A::bottom_right, 0, 0, 40, 20), panel), 170, 100, 210, 120));
  require(same(candidate_canvas_decoration_box(widget(A::left, 0, 0, 40, 20), panel), 10, 60, 50, 80));
  require(same(candidate_canvas_decoration_box(widget(A::right, 0, 0, 40, 20), panel), 170, 60, 210, 80));
  require(same(candidate_canvas_decoration_box(widget(A::top, 0, 0, 40, 20), panel), 90, 20, 130, 40));
  require(same(candidate_canvas_decoration_box(widget(A::bottom, 40, 10, 48, 20), panel), 126, 110, 174, 130));
  require(same(candidate_canvas_decoration_box(widget(A::center, 0, 0, 40, 20), panel), 90, 60, 130, 80));
  // 图片在框里等比缩放、居中。
  require(same(*candidate_canvas_contain({0, 0, 40, 20}, 64, 64), 10, 0, 30, 20));
  require(same(*candidate_canvas_contain({0, 0, 40, 40}, 128, 64), 0, 10, 40, 30));
  require(!candidate_canvas_contain({0, 0, 40, 40}, 0, 64) &&
          !candidate_canvas_contain({0, 0, 0, 40}, 64, 64));

  // 分层：按 z_index 稳定排序，低于 1 的在面板之下，同层保持书写顺序。
  const auto layers = candidate_canvas_layers(
      {widget(A::top, 0, 0, 1, 1, 3, L"a"), widget(A::top, 0, 0, 1, 1, 1, L"b"),
       widget(A::top, 0, 0, 1, 1, -1, L"c"), widget(A::top, 0, 0, 1, 1, 0, L"d"),
       widget(A::top, 0, 0, 1, 1, 1, L"e"), widget(A::top, 0, 0, 1, 1, -1, L"f")});
  require(layers.below.size() == 3 && layers.below[0].image == L"c" &&
          layers.below[1].image == L"f" && layers.below[2].image == L"d");
  require(layers.above.size() == 3 && layers.above[0].image == L"b" &&
          layers.above[1].image == L"e" && layers.above[2].image == L"a");

  // 窗口空白：不是 canvas 时就是阴影边距本身，逐位不变。
  const CandidateCanvasEdges shadow{20.0, 32.0, 40.0, 32.0};
  const auto untouched = candidate_canvas_margins(CandidateSkinCanvas{}, shadow, 200.0, 100.0, 400.0, 300.0);
  require(untouched.top == 20.0 && untouched.right == 32.0 && untouched.bottom == 40.0 &&
          untouched.left == 32.0);
  CandidateSkinCanvas reach;
  reach.active = true;
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0), 20.0, 32.0, 40.0, 32.0));
  // 阴影边距之外加外留白（msime-windows 的 framePad + outer_insets_dip）。
  reach.outer_insets = CandidateCanvasEdges{18.0, 18.0, 12.0, 10.0};
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0), 38.0, 50.0, 52.0, 42.0));
  // 挂件伸得比外留白远时按挂件留：右上角的月亮伸出 18，左下的叶子伸出左边 14、下边 12，底边的云伸出 30。
  reach.decorations = {widget(A::top_right, 30, -30, 72, 72), widget(A::bottom_left, -14, 12, 32, 32),
                       widget(A::bottom, 0, 30, 10, 10)};
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0), 50.0, 62.0, 70.0, 46.0));
  // 完全在面板里的挂件不多留。
  reach.outer_insets.reset();
  reach.decorations = {widget(A::center, 0, 0, 20, 20)};
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0), 20.0, 32.0, 40.0, 32.0));
  // 偏移写得再远，每边最多为它多留 500。
  reach.decorations = {widget(A::left, -1900, 0, 20, 20)};
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0), 20.0, 32.0, 40.0, 532.0));
  // 工作区放不下时只按比例收小为挂件多留的那部分，阴影边距不收。
  reach.outer_insets = CandidateCanvasEdges{0.0, 100.0, 0.0, 0.0};
  reach.decorations.clear();
  const auto fitted = candidate_canvas_margins(reach, shadow, 200.0, 100.0, 314.0, 0.0);
  require(same(fitted, 20.0, 82.0, 40.0, 32.0));
  require(same(candidate_canvas_margins(reach, shadow, 200.0, 100.0, 200.0, 0.0), 20.0, 32.0, 40.0, 32.0));

  // 九宫格：64×64 的图切 16、边宽 8 画进 100×50，九块。
  const CandidateCanvasRect box{0.0, 0.0, 100.0, 50.0};
  const CandidateCanvasEdges slice16{16, 16, 16, 16}, width8{8, 8, 8, 8};
  const auto tiles = candidate_nine_slice(64, 64, 64, 64, box, slice16, width8, true);
  require(tiles.size() == 9);
  require(same(tiles[0].source, 0, 0, 16, 16) && same(tiles[0].destination, 0, 0, 8, 8));
  require(same(tiles[1].source, 16, 0, 48, 16) && same(tiles[1].destination, 8, 0, 92, 8));
  require(same(tiles[4].source, 16, 16, 48, 48) && same(tiles[4].destination, 8, 8, 92, 42));
  require(same(tiles[8].source, 48, 48, 64, 64) && same(tiles[8].destination, 92, 42, 100, 50));
  // fill = false 不画中心块。
  const auto hollow = candidate_nine_slice(64, 64, 64, 64, box, slice16, width8, false);
  require(hollow.size() == 8 && same(hollow[4].destination, 92, 8, 100, 42));
  // 各边不同的切片和边宽。
  const auto mixed = candidate_nine_slice(64, 32, 64, 32, box, {4, 8, 12, 16}, {2, 6, 10, 14}, true);
  require(mixed.size() == 9 && same(mixed[0].source, 0, 0, 16, 4) &&
          same(mixed[0].destination, 0, 0, 14, 2) && same(mixed[8].source, 56, 20, 64, 32) &&
          same(mixed[8].destination, 94, 40, 100, 50));
  // 位图的 DIP 和像素不一样大时，源矩形按 DIP 给。
  const auto doubled = candidate_nine_slice(128, 128, 64, 64, box, {32, 32, 32, 32}, width8, true);
  require(same(doubled[0].source, 0, 0, 16, 16) && same(doubled[4].source, 16, 16, 48, 48));
  // 超出图片的切片按整张图算，左右切片交叉时中间一列是空的。
  const auto crossed = candidate_nine_slice(64, 64, 64, 64, box, {16, 100, 16, 100}, width8, true);
  require(crossed.size() == 6);
  for (const auto &tile : crossed)
    require(tile.source.width() > 0.0 && tile.destination.width() > 0.0);
  // 对边的宽放不下时四边按同一个比例缩小：高 10 放不下上下各 8，四边都乘 10 / 16。
  const auto squeezed = candidate_nine_slice(64, 64, 64, 64, {0, 0, 100, 10}, slice16, width8, true);
  // 上下两条边各 5，中间那一行高 0 不画。
  require(squeezed.size() == 6 && same(squeezed[0].destination, 0, 0, 5, 5) &&
          same(squeezed[2].destination, 95, 0, 100, 5) && same(squeezed[3].destination, 0, 5, 5, 10));
  // 对齐设备像素：150% 时块的边落在 1/1.5 DIP 的整数倍上，相邻两块共用同一条边。
  CandidateCanvasSnap snap;
  snap.enabled = true;
  snap.scale_x = snap.scale_y = 1.5;
  const auto snapped = candidate_nine_slice(64, 64, 64, 64, {0.3, 0.3, 100.3, 50.3}, slice16,
                                            {7.7, 7.7, 7.7, 7.7}, true, snap);
  for (const auto &tile : snapped)
    for (double edge : {tile.destination.left, tile.destination.top, tile.destination.right,
                        tile.destination.bottom})
      require(near(edge * 1.5, std::round(edge * 1.5)));
  require(near(snapped[0].destination.right, snapped[1].destination.left) &&
          near(snapped[1].destination.right, snapped[2].destination.left));
  // 平移也算进去：偏移 0.25 像素时边落在 (n - 0.25) / 1.5 上。
  snap.offset_x = snap.offset_y = 0.25;
  for (const auto &tile : candidate_nine_slice(64, 64, 64, 64, box, slice16, width8, true, snap))
    require(near(tile.destination.left * 1.5 + 0.25, std::round(tile.destination.left * 1.5 + 0.25)));
  // 没有大小的图或目标什么也不画。
  require(candidate_nine_slice(0, 64, 64, 64, box, slice16, width8, true).empty() &&
          candidate_nine_slice(64, 64, 64, 64, {0, 0, 0, 50}, slice16, width8, true).empty());

  // 最小宽度量的是整块面板：面板比卡片左右多出 canvas_extra_width，卡片就少垫这么多；为 0 时和以前逐位相同。
  CandidateCardInput narrow;
  narrow.items = {{20.0}};
  narrow.preedit_width = 10.0;
  const auto plain = candidate_card_size(narrow);
  require(near(plain.width, 16.0 * 7.0));
  narrow.canvas_extra_width = 16.0;
  require(near(candidate_card_size(narrow).width, 96.0));
  narrow.canvas_extra_width = -8.0;
  require(near(candidate_card_size(narrow).width, 120.0));
  // 内容比最小宽度宽时不受影响。
  CandidateCardInput wide;
  wide.items = {{300.0}};
  const auto wide_plain = candidate_card_size(wide);
  wide.canvas_extra_width = 16.0;
  require(near(candidate_card_size(wide).width, wide_plain.width));
  return 0;
}
