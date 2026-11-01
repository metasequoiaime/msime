// 原生画布按 macOS 候选窗结构画的那条路径（NativeCanvasChrome.h）：首行的 logo、拼音、「1 / 3」、‹ ›、展开键，按共享 CandidateItemLayout 排的行和 12pt 释义，网格，装饰带，悬停与选中，以及与 macOS 共用的定位、动效和减弱动态效果规则。
#include "../../src/candidates/NativeCanvas.h"
#include "../../src/candidates/ReduceMotionPolicy.h"
#include <cassert>
#include <cstring>
#include <iostream>
#include <map>

namespace host = msime::linux_host;
namespace shared = msime::candidate;
using Json = nlohmann::json;

namespace {

host::NativeCanvasImage image(int width, int height, std::uint32_t color, double alpha = 1) {
  host::NativeCanvasImage surface(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height), cairo_surface_destroy);
  auto *cr = cairo_create(surface.get());
  cairo_set_source_rgba(cr, ((color >> 16) & 255) / 255.0, ((color >> 8) & 255) / 255.0, (color & 255) / 255.0, alpha);
  cairo_paint(cr);
  cairo_destroy(cr);
  return surface;
}

std::uint32_t pixel(cairo_surface_t *surface, int x, int y) {
  cairo_surface_flush(surface);
  std::uint32_t value = 0;
  std::memcpy(&value, cairo_image_surface_get_data(surface) + y * cairo_image_surface_get_stride(surface) + x * 4, 4);
  return value;
}

bool near(double value, double expected, double tolerance = 0.5) { return std::abs(value - expected) <= tolerance; }

std::shared_ptr<host::NativeCanvasAssets> theme_assets(double scale = 1) {
  host::NativeCanvasThemeStyle style;
  style.scale = scale;
  style.padding = 4 * scale;
  style.card_radius = 10 * scale;
  style.item_radius = 6 * scale;
  style.border_width = 1;
  return host::native_canvas_theme_assets(style, Json::object());
}

host::NativeCanvasContent page(bool horizontal) {
  host::NativeCanvasContent content;
  content.font = "Sans 18px";
  content.horizontal = horizontal;
  content.preedit = "ni hao";
  content.preedit_cursor = 6;
  content.page_index = 0;
  content.page_count = 3;
  content.next = true;
  content.selected = 0;
  content.items = {{"1", "你好"}, {"2", "拟好"}, {"3", "你"}};
  return content;
}

const Json kPalette = {{"surface", "#000000"}, {"text", "#FFFFFF"}, {"number", "#808080"}, {"accent", "#00FF00"},
    {"selected", "#0000FF"}, {"selected_text", "#FFFFFF"}, {"selected_number", "#FFFFFF"}, {"hover", "#FF0000"},
    {"border", "#000000"}};

std::map<std::uint32_t, int> colors(cairo_surface_t *surface, const host::NativeCanvasRect &box) {
  std::map<std::uint32_t, int> result;
  for (int y = static_cast<int>(box.y); y < box.y + box.height; ++y)
    for (int x = static_cast<int>(box.x); x < box.x + box.width; ++x) ++result[pixel(surface, x, y) & 0xffffff];
  return result;
}

// 首行：logo 16pt 加 6pt 间距、拼音在 logo 右边、「1 / 3」和 28pt 宽的 ‹ › 靠右，首行 26pt 高；候选行从首行下面开始。
void header_row() {
  const auto target = image(1200, 800, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(false);
  content.logo = image(64, 64, 0xffaa00);
  const auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(layout.chrome);
  assert(near(layout.logo.width, 16) && near(layout.logo.x, 4 + 2));
  assert(layout.preedit.x >= layout.logo.x + layout.logo.width + 6 - 2.01);
  assert(layout.page_controls && layout.indicator_text == "1 / 3");
  assert(near(layout.previous.width, 28) && near(layout.previous.height, 26));
  assert(near(layout.next.x, layout.previous.x + 28));
  assert(near(layout.next.x + layout.next.width, layout.panel.x + layout.panel.width - 4));
  assert(layout.indicator.x + layout.indicator.width <= layout.previous.x - 4 + 0.5);
  // 首行的内容比 26pt 高时取拼音的高度；候选行紧跟在首行下面。
  const double header = std::max(26.0, layout.preedit.height);
  assert(near(layout.items.front().y, layout.panel.y + 4 + header, 1));
  // 第一页的 ‹ 不可用：落在上面不算命中，› 是下一页。
  const auto centre = [](const host::NativeCanvasRect &box) { return std::make_pair(box.x + box.width / 2, box.y + box.height / 2); };
  const auto previous = centre(layout.previous), next = centre(layout.next);
  assert(host::native_canvas_hit(content, layout, previous.first, previous.second) == -1);
  assert(host::native_canvas_hit(content, layout, next.first, next.second) == host::kNativeCanvasHitNext);
  // 卡片至少是候选字号的 7 倍宽，有翻页时至少 76pt。
  assert(layout.panel.width >= 18 * 7 - 0.01 && layout.panel.width >= 76);
  // 画出来：拼音画成强调色（皮肤没给 preedit_text 时），选中行是选中色，logo 在它的框里。
  host::paint_native_canvas(cr, *assets, content, layout, kPalette);
  assert(colors(target.get(), layout.preedit)[0x00ff00] > 10);
  assert(colors(target.get(), layout.items.front())[0x0000ff] > 100);
  assert(colors(target.get(), layout.logo)[0xffaa00] > 100);
  // 关掉 show_candidate_page_number：「1 / 3」和箭头一起不画，首行只剩 logo 和拼音。
  content.page_arrows = false;
  const auto hidden = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(!hidden.page_controls && hidden.previous.width == 0);
  assert(host::native_canvas_hit(content, hidden, next.first, next.second) != host::kNativeCanvasHitNext);
  // 没有 logo、拼音和翻页时首行不占高度。
  content.logo = {};
  content.preedit.clear();
  const auto bare = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(near(bare.items.front().y, bare.panel.y + 4));
  cairo_destroy(cr);
}

// 行：序号 0.8 倍字号、正文在序号后 1.5pt，行内上下各 6pt；竖排释义放得下时和正文同一行，横排总在正文下面；释义固定 12pt，不随候选字号变。
void rows_and_glosses() {
  const auto target = image(1600, 1000, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  for (const auto *font : {"Sans 18px", "Sans 30px"}) {
    auto content = page(false);
    content.font = font;
    content.preedit.clear();
    content.items[0].translation = "hello";
    const auto vertical = host::measure_native_canvas(cr, *assets, content, 1600);
    assert(!vertical.rows[0].translation.below && vertical.rows[0].translation.width > 0);
    // 12pt 的释义宽度与候选字号无关。
    assert(near(vertical.rows[0].translation.width, host::native_canvas_text_size(cr, vertical.gloss_font, "hello").first, 1));
    content.horizontal = true;
    const auto horizontal = host::measure_native_canvas(cr, *assets, content, 1600);
    assert(horizontal.rows[0].translation.below);
    assert(horizontal.items[0].height > vertical.items[1].height);
  }
  // 横排为还没到的释义预留一行：所有列一样高，等于正文一行加一行释义。
  auto content = page(true);
  content.preedit.clear();
  const auto plain = host::measure_native_canvas(cr, *assets, content, 1600);
  content.reserved_gloss_lines = 1;
  const auto reserved = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(reserved.items[0].height > plain.items[0].height);
  assert(reserved.items[0].height == reserved.items[2].height);
  // 训音在释义行最前，插件注释在最后；三者都画在行框里。
  content.reserved_gloss_lines = 0;
  content.items[0].translation = "hello";
  content.items[0].reading = "reading";
  content.items[0].script_annotation = "note";
  const auto gloss = host::native_canvas_gloss(content.items[0]);
  assert(gloss.text == "reading\nhello\nnote" && gloss.translation_start == 8 && gloss.script_start == 14);
  const auto stacked = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(stacked.rows[0].translation.below && stacked.items[0].height > plain.items[0].height + 30);
  // 读音在它那一行释义下面单独一行，排在插件注释之前，位置记下来减淡画；释义行的格子跟着多一行高。
  content.items[0].pronunciation = "/h\xC9\x99\xCB\x88l\xC9\x99\xCA\x8A/";
  const auto spoken = host::native_canvas_gloss(content.items[0]);
  assert(spoken.text == "reading\nhello\n" + content.items[0].pronunciation + "\nnote" && spoken.translation_start == 8);
  assert(spoken.translation.readings.size() == 1 &&
         spoken.text.substr(spoken.translation_start + spoken.translation.readings[0].first,
                            spoken.translation.readings[0].second) == content.items[0].pronunciation);
  assert(host::measure_native_canvas(cr, *assets, content, 1600).items[0].height > stacked.items[0].height + 8);
  content.items[0].pronunciation.clear();
  host::paint_native_canvas(cr, *assets, content, stacked, kPalette);
  // 序号和释义（没有皮肤色时用序号色，灰）只画在各自的行框里；首行的「1 / 3」也是这个颜色，不在这里数。
  for (int y = static_cast<int>(stacked.rows_area.y); y < stacked.height; ++y) for (int x = 0; x < stacked.width; ++x) {
    const auto color = pixel(target.get(), x, y) & 0xffffff;
    if (color == 0x808080) {
      const auto hits = std::count_if(stacked.items.begin(), stacked.items.end(), [&](const auto &box) { return box.contains(x, y); });
      assert(hits == 1);
    }
  }
  // 横排卡片这次显示以来只变宽不变窄。
  content = page(true);
  content.preedit.clear();
  const auto natural = host::measure_native_canvas(cr, *assets, content, 1600);
  content.minimum_width = natural.panel.width + 120;
  assert(near(host::measure_native_canvas(cr, *assets, content, 1600).panel.width, natural.panel.width + 120));
  cairo_destroy(cr);
}

// 光标后编辑：提示画在底部一行的右端，只有提示时这一行也出现，和本地模式、连击同在时它们从左边排、互不重叠；首选开头固定的那几个字用序号色画淡，没有提示和画淡时什么都不变。
void caret_edit_suffix() {
  const auto target = image(1600, 1000, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(false);
  // 没有拼音、只有一页：首行不出现，卡片只有两个短候选那么宽，比提示窄。
  content.preedit.clear();
  content.page_count = 1;
  content.next = false;
  const auto plain = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(plain.auxiliary_hint.width == 0 && plain.auxiliary.width == 0);
  content.auxiliary_hint = "候选编辑：光标后";
  const auto hinted = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(hinted.auxiliary_hint.width > 0 && hinted.auxiliary.width == 0);
  assert(hinted.panel.height > plain.panel.height);
  // 提示算进卡片的最小宽度：候选再短，提示也能整段画在内边距以内。
  assert(hinted.panel.width >= plain.panel.width && hinted.auxiliary_hint.x >= hinted.panel.x + 4 + hinted.text_left - 1);
  assert(hinted.auxiliary_hint.y >= hinted.items.back().y + hinted.items.back().height);
  // 这一行至少有释义字体本身那么高：提示整段落在更矮的回退字体里（没有中文字体的 Fedora 上缺字方框只有 15px，Noto Sans 一行 17px）时也不例外，下面加上连击后卡片才不会长高。
  assert(hinted.auxiliary_hint.height >= host::native_canvas_text_size(cr, hinted.gloss_font, "X").second);
  // 贴右边距：提示的右缘就是行内文字的右缘（卡片右缘减内边距和右边距）。
  assert(near(hinted.auxiliary_hint.x + hinted.auxiliary_hint.width,
              hinted.panel.x + hinted.panel.width - 4 - hinted.text_right, 1));
  content.auxiliary = "连击 ×12";
  const auto both = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(both.auxiliary.width > 0 && both.auxiliary_hint.width > 0);
  assert(both.auxiliary.x + both.auxiliary.width <= both.auxiliary_hint.x);
  assert(near(both.auxiliary.y, both.auxiliary_hint.y) && near(both.panel.height, hinted.panel.height, 1));
  // 提示比卡片的最小宽度还宽时卡片跟着变宽，连击仍排在它左边，两段都画全、不重叠（不同系统的字体宽度不同，这里用一段加长的提示让它在哪里都比卡片宽）。
  {
    auto wide = content;
    wide.auxiliary_hint = "候选编辑：光标后候选编辑：光标后候选编辑：光标后";
    const auto widened = host::measure_native_canvas(cr, *assets, wide, 1600);
    assert(widened.panel.width > both.panel.width);
    assert(widened.auxiliary.x + widened.auxiliary.width <= widened.auxiliary_hint.x);
    assert(widened.auxiliary.width >= std::ceil(host::native_canvas_text_size(cr, widened.gloss_font, wide.auxiliary).first));
    assert(widened.auxiliary_hint.width >= std::ceil(host::native_canvas_text_size(cr, widened.gloss_font, wide.auxiliary_hint).first));
  }
  host::paint_native_canvas(cr, *assets, content, both, kPalette);
  // 提示用次要色（没有皮肤色时是序号色，灰），画在它自己的框里。
  assert(colors(target.get(), both.auxiliary_hint)[0x808080] > 10);

  // 画淡：用绿色区分序号色与白字抗锯齿产生的灰像素，避免缺字回退字体改变灰像素数。
  // 第一行不选中；画淡前两个字符后，序号色像素变多，白色变少，后两个字符仍保持白色。
  content.auxiliary.clear();
  content.auxiliary_hint.clear();
  content.selected = 1;
  // CI 不保证安装中文字体；颜色像素断言使用基础字体的实心字形，UTF-8 范围由 caret_edit_suffix.cpp 验证。
  content.font = "Sans 30px";
  content.items[0].text = "MMMM";
  auto dim_palette = kPalette;
  dim_palette["number"] = "#00FF00";
  const auto ink = [&](std::size_t dim_bytes) {
    content.items[0].dim_prefix_bytes = dim_bytes;
    const auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
    cairo_save(cr); cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR); cairo_paint(cr); cairo_restore(cr);
    host::paint_native_canvas(cr, *assets, content, layout, dim_palette);
    return std::make_pair(layout, colors(target.get(), layout.items[0]));
  };
  auto [undimmed_layout, undimmed] = ink(0);
  auto [dimmed_layout, dimmed] = ink(2);
  assert(dimmed[0x00ff00] > undimmed[0x00ff00] && dimmed[0xffffff] < undimmed[0xffffff] && dimmed[0xffffff] > 0);
  // 只改颜色：行的大小不变。
  assert(near(dimmed_layout.items[0].width, undimmed_layout.items[0].width) &&
         near(dimmed_layout.items[0].height, undimmed_layout.items[0].height));
  // 整行都画淡时正文里不剩白色；越界的字节数按整段算。
  auto [whole_layout, whole] = ink(100);
  assert(whole[0xffffff] == 0 && whole[0x00ff00] > dimmed[0x00ff00]);
  cairo_destroy(cr);
}

// 悬停和按下：悬停填悬停色、不改变键盘选中；按下画选中色。展开键在首行最右，网格首行是「行 / 总行数」和收起键。
void pointer_and_grid() {
  const auto target = image(1600, 1000, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(true);
  content.expandable = true;
  auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(layout.expand.width > 0 && near(layout.expand.x + layout.expand.width, layout.panel.width - 4));
  assert(near(layout.next.x + layout.next.width, layout.expand.x));
  assert(host::native_canvas_hit(content, layout, layout.expand.x + 5, layout.expand.y + 5) == host::kNativeCanvasHitExpand);
  host::paint_native_canvas(cr, *assets, content, layout, kPalette, 0, {1, -1});
  assert(colors(target.get(), layout.items[1])[0xff0000] > 50);
  assert(colors(target.get(), layout.items[0])[0x0000ff] > 50);
  host::paint_native_canvas(cr, *assets, content, layout, kPalette, 0, {1, 1});
  assert(colors(target.get(), layout.items[1])[0x0000ff] > 50);
  // 首行什么都不画时，展开键在第一行候选的右端。
  content.preedit.clear();
  content.page_arrows = false;
  layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(layout.expand.width > 0 && layout.expand.y >= layout.items.front().y - 0.01);
  // 网格：两行三列，高亮在第二行；只有高亮那一行有序号，首行「2 / 4」和收起键。
  content = page(true);
  content.preedit.clear();
  content.items = {{"", "甲"}, {"", "乙"}, {"", "丙"}, {"1", "丁"}, {"2", "戊"}, {"3", "己"}};
  content.selected = 4;
  content.grid_columns = 3;
  content.grid_row = 1;
  content.grid_rows = 4;
  layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(layout.indicator_text == "2 / 4" && layout.expand.width > 0 && layout.previous.width == 0);
  assert(layout.items[3].y > layout.items[0].y && near(layout.items[3].x, layout.items[0].x));
  // 序号列每行都按「1」…「n」量，没画序号的行与高亮行的文字对齐，高亮换行时不左右跳（与 macOS 相同）。
  assert(near(layout.content_lefts[3], layout.content_lefts[0]) && near(layout.items[0].width, layout.items[3].width, 1));
  host::paint_native_canvas(cr, *assets, content, layout, kPalette);
  assert(colors(target.get(), layout.items[4])[0x0000ff] > 50);
  cairo_destroy(cr);
}

// 缩放：首行、行内边距、logo 和释义字号都乘 candidate_scale_percent。
void scaled() {
  const auto target = image(10, 10, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets(1.5);
  auto content = page(false);
  content.logo = image(64, 64, 0xffaa00);
  const auto layout = host::measure_native_canvas(cr, *assets, content, 2400);
  assert(near(layout.logo.width, 24) && near(layout.previous.width, 42) && near(layout.previous.height, 39));
  assert(near(host::native_canvas_text_size(cr, layout.gloss_font, "X").second,
              host::native_canvas_text_size(cr, host::native_canvas_font_variant("Sans", 18), "X").second, 1.5));
  cairo_destroy(cr);
}

// 顶带装饰：窗口比卡片高出装饰带，图片底边压在卡片顶边下 pad 处；有待机动效时四周留 4pt。
void band() {
  const auto target = image(1200, 800, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto animation = std::make_shared<host::NativeCanvasAnimation>();
  animation->frames = {image(40, 20, 0x00ffff)};
  animation->delays_ms = {0};
  assets->band.image = animation;
  assets->band.top = 30;
  assets->band.width = 80;
  const auto content = page(false);
  auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(near(layout.panel.y, 30) && near(layout.height, layout.panel.height + 30, 1));
  assert(near(layout.band.y + layout.band.height, 30 + 4) && near(layout.band.width, 68));
  assert(near(layout.band.x + layout.band.width, layout.panel.width - 4));
  host::paint_native_canvas(cr, *assets, content, layout, kPalette);
  assert(colors(target.get(), layout.band)[0x00ffff] > 500);
  assets->band.motion = shared::DecorationMotionKind::floating;
  layout = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(layout.band.y >= shared::kDecorationMotionMargin - 0.01);
  assert(layout.band.x + layout.band.width <= layout.panel.width - shared::kDecorationMotionMargin + 0.01);
  cairo_destroy(cr);
}

// 皮肤背景图：画在卡片底色之上、候选之下，裁在圆角里；cover 铺满，contain 两边留底色，不透明度再乘候选窗不透明度。读不出的图片只少背景。
void background() {
  const auto target = image(1200, 800, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(false);
  content.preedit.clear();
  assets->background = {image(10, 400, 0xff00ff), host::CandidateSkinBackgroundFit::cover, 1};
  const auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  const auto &panel = layout.panel;
  const int left = static_cast<int>(panel.x + 2), middle = static_cast<int>(panel.y + panel.height / 2);
  host::paint_native_canvas(cr, *assets, content, layout, kPalette);
  assert((pixel(target.get(), left, middle) & 0xffffff) == 0xff00ff);
  assert((pixel(target.get(), static_cast<int>(panel.x), static_cast<int>(panel.y)) & 0xffffff) != 0xff00ff);
  // 选中行画在背景之上。
  assert(colors(target.get(), layout.items[0])[0x0000ff] > 50);
  // contain：又窄又高的图等比放进卡片后居中，左边留出底色。
  assets->background.fit = host::CandidateSkinBackgroundFit::contain;
  host::paint_native_canvas(cr, *assets, content, layout, kPalette);
  assert((pixel(target.get(), left, middle) & 0xffffff) == 0x000000);
  assert((pixel(target.get(), static_cast<int>(panel.x + panel.width / 2), static_cast<int>(panel.y + 1)) & 0xffffff) == 0xff00ff);
  // 背景不透明度 0.5、候选窗不透明度 0.5：合到黑底上约 25%。
  assets->background.fit = host::CandidateSkinBackgroundFit::stretch;
  assets->background.opacity = 0.5;
  assets->opacity = 0.5;
  const auto clear = image(1200, 800, 0, 0);
  auto *transparent = cairo_create(clear.get());
  host::paint_native_canvas(transparent, *assets, content, layout, kPalette);
  const auto red = (pixel(clear.get(), left, middle) >> 16) & 0xff;
  assert(red >= 0x38 && red <= 0x48);
  cairo_destroy(transparent);
  cairo_destroy(cr);
  host::NativeCanvasThemeStyle missing;
  missing.background = host::CandidateSkinBackground{"/nonexistent/msime-background.png"};
  assert(!host::native_canvas_theme_assets(missing, Json::object())->background.image);
}

// 卡片投影：只有合成器在时、只给 macOS 结构的卡片留边距；装饰带够高时上边不多留。投影在卡片下方最重，卡片底下擦掉，离开边距就没有。
void shadow() {
  const auto target = image(10, 10, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  const auto content = page(false);
  const auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  const auto none = host::native_canvas_shadow_insets(layout, false);
  assert(none.left == 0 && none.top == 0 && none.right == 0 && none.bottom == 0);
  const auto inset = host::native_canvas_shadow_insets(layout, true);
  assert(inset.left == 12 && inset.top == 8 && inset.right == 12 && inset.bottom == 16);
  host::NativeCanvasAssets skin;
  assert(host::native_canvas_shadow_insets(host::measure_native_canvas(cr, skin, content), true).bottom == 0);
  auto animation = std::make_shared<host::NativeCanvasAnimation>();
  animation->frames = {image(40, 20, 0x00ffff)};
  animation->delays_ms = {0};
  assets->band.image = animation;
  assets->band.top = 30;
  assets->band.width = 80;
  assert(host::native_canvas_shadow_insets(host::measure_native_canvas(cr, *assets, content, 1600), true).top == 0);
  cairo_destroy(cr);
  for (const double scale : {1.0, 2.0}) {
    const host::NativeCanvasRect card{12, 8, 200, 100};
    const int width = static_cast<int>((12 + 200 + 12) * scale), height = static_cast<int>((8 + 100 + 16) * scale);
    const auto shade = host::native_canvas_card_shadow(width, height, card, 10, scale);
    assert(shade && cairo_image_surface_get_width(shade.get()) == width);
    const auto alpha = [&](double x, double y) {
      return pixel(shade.get(), static_cast<int>(x * scale), static_cast<int>(y * scale)) >> 24;
    };
    // 卡片正下方 4px 处最重，上方较轻（向下偏 4px），卡片里面为 0，左右对称。
    const auto below = alpha(112, 112), above = alpha(112, 4);
    assert(below > 0x10 && below < 0x38 && above > 0 && above < below);
    assert(alpha(112, 58) == 0);
    const int side = static_cast<int>(5 * scale), middle = static_cast<int>(58 * scale);
    const auto left = pixel(shade.get(), side, middle) >> 24, right = pixel(shade.get(), width - 1 - side, middle) >> 24;
    assert(left + 1 >= right && right + 1 >= left && left > 0);
    assert(alpha(0, 0) < 0x04);
    // 跳过卡片内部只省计算，不改像素：逐像素按完整公式重算一遍，结果完全相同。
    using G = host::FcitxPanelGeometry;
    const host::FcitxRect body{card.x, card.y, card.x + card.width, card.y + card.height};
    const host::FcitxRect cast{body.left, body.top + G::shadow_offset, body.right, body.bottom + G::shadow_offset};
    const double spread = G::shadow_sigma * std::sqrt(2.0), unit = 1 / scale;
    for (int y = 0; y < height; ++y)
      for (int x = 0; x < width; ++x) {
        const double px = (x + 0.5) * unit, py = (y + 0.5) * unit;
        const double keep = 1.0 - host::fcitx_coverage(host::fcitx_rounded_rect_distance(px, py, body, 10), unit);
        const double distance = host::fcitx_rounded_rect_distance(px, py, cast, 10);
        const std::uint32_t expected = keep <= 0 || distance > 4 * spread ? 0
            : static_cast<std::uint32_t>(std::lround(std::clamp(G::shadow_alpha * 0.5 * std::erfc(distance / spread) * keep, 0.0, 1.0) * 255));
        assert((pixel(shade.get(), x, y) >> 24) == expected);
      }
  }
}

// 主题调色板：system 主题没有候选配色，铺 Adwaita；主题给了的槽位盖过它，半透明的保留 alpha。
void palette() {
  const auto light = host::native_canvas_theme_palette(Json::object(), false);
  assert(light.at("selected") == "#1A5FB4" && light.at("selected_text") == "#FFFFFF" && light.at("hover") == "#0000000F");
  assert(host::native_canvas_theme_palette(Json::object(), true).at("surface") == "#303030");
  const auto themed = host::native_canvas_theme_palette({{"candidate", {{"selected", "#11223380"}, {"text", nullptr}}},
      {"candidate_details", {{"preedit_text", "#445566"}, {"show_selected_bar", true}}}}, false);
  assert(themed.at("selected") == "#11223380" && themed.at("text") == light.at("text"));
  assert(themed.at("preedit_text") == "#445566" && themed.at("show_selected_bar") == true);
  // 选中竖条：system 是 Adwaita 蓝；主题给了 accent 没给竖条色时跟随 accent；两个都给时用竖条色。
  assert(light.at("selected_bar") == "#3584E4" && themed.at("selected_bar") == "#3584E4");
  const auto green = host::native_canvas_theme_palette({{"candidate", {{"accent", "#07C160"}}},
      {"candidate_details", {{"selected_bar", nullptr}, {"show_selected_bar", true}}}}, false);
  assert(green.at("selected_bar") == "#07C160");
  const auto both = host::native_canvas_theme_palette({{"candidate", {{"accent", "#07C160"}}},
      {"candidate_details", {{"selected_bar", "#FF0000"}}}}, false);
  assert(both.at("selected_bar") == "#FF0000");
  // 共享版式的 expandRows 夹在 2–8；没有共享版式时为空，网格用 kExpandedGridVisibleRows。
  const auto rows = [](Json layout) {
    return host::native_canvas_theme_assets(host::NativeCanvasThemeStyle{}, {{"resolvedLayout", std::move(layout)}})->expand_rows;
  };
  assert(rows({{"expandRows", 3}}) == 3u && rows({{"expandRows", 20}}) == 8u && rows({{"expandRows", 0}}) == 2u);
  assert(!rows(Json::object()) && !host::native_canvas_theme_assets(host::NativeCanvasThemeStyle{}, Json::object())->expand_rows);
  assert(!host::native_canvas_renderer_enabled("classicui") && host::native_canvas_renderer_enabled(nullptr) &&
         host::native_canvas_renderer_enabled("native"));
}

// 定位：与 macOS 共用 CandidateOrigin，X11 的 y 朝下、物理像素。光标下方 4pt；放不下时翻到光标上方；按记住的最高一页决定翻不翻；装饰带朝上伸、不占光标间隙。
void placement() {
  const int caret_x = 400, caret_y = 300, caret_h = 20;
  auto [x, y] = host::native_canvas_window_origin(1, caret_x, caret_y, caret_h, 300, 120, 0, 0, 1600, 1000);
  assert(x == 400 && y == 300 + 20 + 4);
  std::tie(x, y) = host::native_canvas_window_origin(2, caret_x, caret_y, caret_h, 300, 120, 0, 0, 1600, 1000);
  assert(y == 300 + 20 + 8);
  std::tie(x, y) = host::native_canvas_window_origin(1, caret_x, 950, caret_h, 300, 120, 0, 0, 1600, 1000);
  assert(y == 950 - 4 - 120);
  // 记住的最高一页放不下光标下方时，这一页也放在上方，贴着光标。
  std::tie(x, y) = host::native_canvas_window_origin(1, caret_x, 700, caret_h, 300, 100, 0, 0, 1600, 1000, 300);
  assert(y == 700 - 4 - 100);
  std::tie(x, y) = host::native_canvas_window_origin(1, caret_x, 700, caret_h, 300, 100, 0, 0, 1600, 1000);
  assert(y == 700 + 20 + 4);
  // 装饰带 40：卡片顶边在光标下方 4pt，带朝上盖住光标这一行。
  std::tie(x, y) = host::native_canvas_window_origin(1, caret_x, caret_y, caret_h, 300, 160, 0, 0, 1600, 1000, 0, 40);
  assert(y == 300 + 20 + 4 - 40);
  // 左外留白不参与对齐；右边放不下时停在工作区边上。
  std::tie(x, y) = host::native_canvas_window_origin(1, caret_x, caret_y, caret_h, 300, 120, 0, 0, 1600, 1000, 0, 0, 10);
  assert(x == 390);
  std::tie(x, y) = host::native_canvas_window_origin(1, 1500, caret_y, caret_h, 300, 120, 0, 0, 1600, 1000);
  assert(x == 1300);
  // 记住的高度：竖排取最高，横排不加；隐藏后从头记。
  assert(shared::TallestCandidateHeight(200, 100, true, true) == 200 && shared::TallestCandidateHeight(200, 100, true, false) == 100);
  assert(shared::WidestCandidateWidth(300, 200, false, true) == 300 && shared::WidestCandidateWidth(300, 200, true, true) == 0);
}

// 动效采样：进场从 0 到完整，退场回到 0；只有 fade_scale、slide_up 错峰，延迟有上限；待机动效位移不超过余量；减弱动态效果的来源。
void motion() {
  host::NativeCanvasAssets static_assets;
  const Json declared_animation = {
      {"enter", "fade_scale"}, {"exit", "fade"}, {"selection", "spring"},
      {"durationMs", 180}, {"staggerMs", 14},
      {"declared", {{"enter", true}, {"exit", true}, {"selection", true}}}};
  host::read_native_canvas_animation(static_assets, Json{{"animation", declared_animation}});
  assert(static_assets.enter == shared::MotionPreset::none && static_assets.exit == shared::MotionPreset::none
      && static_assets.selection == shared::MotionPreset::none && static_assets.duration_ms == 0 && static_assets.stagger_ms == 0);
  for (const auto preset : {shared::MotionPreset::fade, shared::MotionPreset::fade_scale, shared::MotionPreset::slide_up,
                            shared::MotionPreset::spring}) {
    const auto start = shared::EnterFrame(preset, 0), end = shared::EnterFrame(preset, 1);
    assert(start.opacity == 0 && near(end.opacity, 1, 1e-6) && near(end.scale, 1, 1e-6) && near(end.lift, 0, 1e-6));
    const auto gone = shared::ExitFrame(preset, 1);
    assert(near(gone.opacity, 0, 1e-6));
  }
  assert(near(shared::EnterFrame(shared::MotionPreset::fade_scale, 0).scale, 0.96, 1e-9));
  assert(near(shared::EnterFrame(shared::MotionPreset::slide_up, 0).lift, -8, 1e-9));
  assert(near(shared::EnterFrame(shared::MotionPreset::glow, 0).opacity, 0.82, 1e-9));
  assert(near(shared::ExitFrame(shared::MotionPreset::glow, 1).opacity, 0.82, 1e-6));
  assert(shared::MotionStaggersRows(shared::MotionPreset::fade_scale) && !shared::MotionStaggersRows(shared::MotionPreset::fade));
  assert(near(shared::MotionRowDelay(20, 30), shared::kMotionStaggerCapSeconds, 1e-9));
  assert(near(shared::MotionSeconds(5), 0.04, 1e-9) && near(shared::MotionSeconds(5000), 0.6, 1e-9));
  assert(near(shared::SelectionFrame(shared::MotionPreset::fade, 0).fillOpacity, 0.45, 1e-9));
  assert(near(shared::SelectionFrame(shared::MotionPreset::glow, 0).ringOpacity, 0.65, 1e-9));
  for (double phase = 0; phase < 1; phase += 0.05) {
    const auto floating = shared::DecorationMotionAt(shared::DecorationMotionKind::floating, phase, 80, 40, 4);
    assert(std::abs(floating.lift) <= 3 + 1e-9);
    const auto bounce = shared::DecorationMotionAt(shared::DecorationMotionKind::bounce, phase, 80, 40, 4);
    assert(bounce.lift >= -1e-9 && bounce.lift <= 3.5 + 1e-9);
    const auto breathe = shared::DecorationMotionAt(shared::DecorationMotionKind::breathe, phase, 80, 40, 4);
    assert(breathe.scale >= 1 - 1e-9 && breathe.scale <= 1.03 + 1e-9);
  }
  assert(near(shared::DecorationMotionAt(shared::DecorationMotionKind::bounce, 0.3, 80, 40, 4).lift, 0, 1e-9));
  assert(shared::MotionEaseOut(0.5) > 0.5 && shared::MotionEaseIn(0.5) < 0.5);
  assert(host::kde_reduce_motion("[KDE]\nAnimationDurationFactor=0\n") == true);
  assert(host::kde_reduce_motion("[KDE]\nAnimationDurationFactor=0.5\n") == false);
  assert(!host::kde_reduce_motion("[General]\nAnimationDurationFactor=0\n"));
  assert(host::reduce_motion_from(false, std::nullopt) && !host::reduce_motion_from(true, false));
  assert(!host::reduce_motion_from(std::nullopt, std::nullopt));
}

// 卡片动效画在窗口里：选中装饰的底色淡入、行错峰时整行透明度；canvas 包仍走 Windows 版式，不受 chrome 影响。
void selection_motion() {
  const auto target = image(1200, 800, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(false);
  content.preedit.clear();
  const auto layout = host::measure_native_canvas(cr, *assets, content, 1600);
  host::NativeCanvasMotion motion;
  motion.selection = shared::SelectionFrame(shared::MotionPreset::fade, 0);
  host::paint_native_canvas(cr, *assets, content, layout, kPalette, 0, {}, motion);
  const auto &box = layout.items[0];
  const auto faded = pixel(target.get(), static_cast<int>(box.x + box.width - 4), static_cast<int>(box.y + box.height / 2)) & 0xff;
  assert(faded > 0x60 && faded < 0x90);
  motion = {};
  motion.rows = {shared::CardMotionFrame{0, 1, 0}, {}, {}};
  host::paint_native_canvas(cr, *assets, content, layout, kPalette, 0, {}, motion);
  assert(colors(target.get(), layout.items[0])[0x0000ff] == 0);
  host::NativeCanvasAssets skin;
  assert(!host::measure_native_canvas(cr, skin, content).chrome);
  cairo_destroy(cr);
}

} // namespace

// 「仅当前高亮」：竖排的高亮行（视图的 translation_reserved）一直留出与横排同样行数的释义高度，回答还没到也留；释义到了、高亮换了一行，卡片都一样高，下面的行跟着下移。
void reserved_highlight_gloss() {
  const auto target = image(1200, 900, 0);
  auto *cr = cairo_create(target.get());
  auto assets = theme_assets();
  auto content = page(false);
  content.preedit.clear();
  const auto plain = host::measure_native_canvas(cr, *assets, content, 1600);
  content.items[0].reserved_gloss_lines = 1;
  const auto waiting = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(waiting.items[0].height > plain.items[0].height + 8);
  assert(near(waiting.items[1].height, plain.items[1].height) && near(waiting.items[2].height, plain.items[2].height));
  assert(near(waiting.items[1].y, waiting.items[0].y + waiting.items[0].height, 1));
  assert(waiting.panel.height > plain.panel.height + 8);
  // 正文仍从行顶开始，留出的高度在它下面。
  assert(near(waiting.rows[0].textHeight, plain.rows[0].textHeight));
  // 回答到了（放得下时和正文同一行）：高度不变。
  content.items[0].translation = "hello";
  const auto answered = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(near(answered.panel.height, waiting.panel.height) && near(answered.items[0].height, waiting.items[0].height));
  // 高亮移到第二行：第一行的译文和预留跟着走，卡片高度不变，第三行仍在原处。
  content.items[0].translation.clear();
  content.items[0].reserved_gloss_lines = 0;
  content.items[1].reserved_gloss_lines = 1;
  content.selected = 1;
  const auto moved = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(near(moved.panel.height, waiting.panel.height));
  assert(near(moved.items[0].height, plain.items[0].height) && near(moved.items[1].height, waiting.items[0].height));
  assert(near(moved.items[2].y, waiting.items[2].y));
  // 点选仍按移下去的行框命中。
  const auto &third = moved.items[2];
  assert(host::native_canvas_hit(content, moved, third.x + third.width / 2, third.y + third.height / 2) == 2);
  // 两种目标语言：留两行。
  content.items[1].reserved_gloss_lines = 2;
  const auto two = host::measure_native_canvas(cr, *assets, content, 1600);
  assert(two.items[1].height > moved.items[1].height + 8);
  // 横排条照旧由 reserved_gloss_lines（整张卡片）统一预留，高亮行的标记不让它更高。
  content = page(true);
  content.preedit.clear();
  content.reserved_gloss_lines = 1;
  const auto strip = host::measure_native_canvas(cr, *assets, content, 1600);
  content.items[0].reserved_gloss_lines = 1;
  assert(near(host::measure_native_canvas(cr, *assets, content, 1600).panel.height, strip.panel.height));
  cairo_destroy(cr);
}

int main() {
  header_row();
  rows_and_glosses();
  reserved_highlight_gloss();
  pointer_and_grid();
  caret_edit_suffix();
  scaled();
  band();
  background();
  shadow();
  palette();
  placement();
  motion();
  selection_motion();
  std::cout << "原生画布的 macOS 结构：首行、行与释义、光标后编辑、高亮行预留释义、悬停、网格、缩放、装饰带、背景图、投影、调色板、定位与动效通过\n";
}
