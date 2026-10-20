#include "../src/overlay/FloatingToolbarPainter.h"
#include "../src/overlay/ShuangpinKeymapPainter.h"

#include <cairo/cairo.h>

#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <string>

namespace host = msime::linux_host;

namespace {

struct Pixel {
  int alpha;
  std::uint32_t rgb;
};

// 预乘 ARGB32 的一个像素：alpha 和按 alpha 还原的 RGB。
Pixel pixel_at(cairo_surface_t *surface, int x, int y) {
  cairo_surface_flush(surface);
  const auto *data = cairo_image_surface_get_data(surface);
  const auto stride = cairo_image_surface_get_stride(surface);
  const auto value = *reinterpret_cast<const std::uint32_t *>(data + y * stride + x * 4);
  const int alpha = static_cast<int>(value >> 24);
  if (alpha == 0) return {0, 0};
  const auto channel = [&](int shift) {
    return static_cast<std::uint32_t>((((value >> shift) & 0xff) * 255 + alpha / 2) / alpha);
  };
  return {alpha, (channel(16) << 16) | (channel(8) << 8) | channel(0)};
}

// 设置了 MSIME_TEST_CANVAS_ARTIFACTS 时把画好的图存成 PNG 给人看，与候选画布和切换徽章的测试同一个目录约定。
void save(cairo_surface_t *surface, const char *name) {
  const char *directory = std::getenv("MSIME_TEST_CANVAS_ARTIFACTS");
  if (!directory || !*directory) return;
  cairo_surface_write_to_png(surface, (std::string(directory) + "/" + name).c_str());
}

cairo_surface_t *canvas(double width, double height) {
  return cairo_image_surface_create(CAIRO_FORMAT_ARGB32, static_cast<int>(std::ceil(width)),
                                    static_cast<int>(std::ceil(height)));
}

void keymap(bool dark, bool shadow, const char *name) {
  host::CandidateTheme theme;
  theme.dark = dark;
  const auto colors = host::shuangpin_keymap_colors(theme);
  host::ShuangpinKeymapContent content;
  content.title = host::shuangpin_keymap_title("xiaohe");
  content.rows = host::shuangpin_keymap_rows(nlohmann::json{
      {"Q", "q / iu"}, {"W", "w / ei"}, {"H", "h / ang"}, {"J", "j / an"}, {"S", "s / iong ong"}});
  content.zero_initials = host::shuangpin_keymap_zero_initial_text(nlohmann::json{{"a", "aa"}, {"ang", "ah"}});
  content.highlighted = 'H';
  auto *surface = canvas(host::shuangpin_keymap_surface_width(shadow), host::shuangpin_keymap_surface_height(shadow));
  auto *cairo = cairo_create(surface);
  host::paint_shuangpin_keymap(cairo, content, colors, shadow);
  cairo_destroy(cairo);
  save(surface, name);
  const int margin = static_cast<int>(host::shuangpin_keymap_margin(shadow));
  // 13 圆角之外是透明的；画阴影时那里只有半透明的阴影。
  if (shadow) assert(pixel_at(surface, margin, margin).alpha < 64);
  else assert(pixel_at(surface, margin, margin).alpha == 0);
  // 卡片左边那 14 的留白里是不透明的底色。
  const auto inside = pixel_at(surface, margin + 6, margin + 120);
  assert(inside.alpha == 255 && inside.rgb == colors.surface);
  // 高亮的 H（中排第六个）右侧空白处是选中色，旁边的 J 是键帽色。
  const auto lit = host::shuangpin_keymap_key_rect(1, 5, 9);
  const auto at = [&](const host::ShuangpinKeymapRect &rect) {
    return pixel_at(surface, margin + static_cast<int>(rect.x + rect.width - 4), margin + static_cast<int>(rect.y + rect.height / 2));
  };
  assert(at(lit).rgb == colors.accent && at(lit).alpha == 255);
  const auto plain = host::shuangpin_keymap_key_rect(1, 6, 9);
  assert(at(plain).rgb == colors.key);
  if (shadow) {
    // 阴影在卡片下方，比上方深。
    assert(pixel_at(surface, margin + 300, margin + 203 + 3).alpha > pixel_at(surface, margin + 300, margin - 3).alpha);
  }
  cairo_surface_destroy(surface);
}

void toolbar(bool logo, bool shadow, const char *name) {
  auto config = host::floating_toolbar_config(nlohmann::json{{"show_app_logo", logo},
                                                             {"floating_toolbar", {{"emoji", true}, {"voice", true}}}},
                                              true, true);
  const auto layout = host::floating_toolbar_layout(config, shadow);
  host::CandidateTheme theme;
  const auto colors = host::floating_toolbar_colors(theme);
  host::FloatingToolbarState state;
  state.scheme_title = "全拼";
  host::FloatingToolbarFrame frame;
  for (const auto button : config.buttons) frame.faces.push_back(host::floating_toolbar_face(button, state, "水杉输入法"));
  frame.hovered = 1;
  frame.show_logo = logo;
  auto *surface = canvas(layout.width, layout.height);
  auto *cairo = cairo_create(surface);
  host::paint_floating_toolbar(cairo, layout, frame, colors);
  cairo_destroy(cairo);
  save(surface, name);
  const auto &plate = layout.plate;
  // 10 圆角之外透明（画阴影时只有半透明的阴影），把手区上沿是底色。
  assert(pixel_at(surface, static_cast<int>(plate.x), static_cast<int>(plate.y)).alpha < (shadow ? 64 : 1));
  assert(pixel_at(surface, static_cast<int>(layout.handle.x + layout.handle.width - 2), static_cast<int>(plate.y + 3)).rgb ==
         colors.surface);
  // 悬停的第二个按钮左上角内侧垫着悬停色，第三个按钮同一处是底色。
  const auto corner = [&](std::size_t index) {
    const auto &cell = layout.buttons[index];
    return pixel_at(surface, static_cast<int>(cell.x + 3), static_cast<int>(cell.y + cell.height / 2));
  };
  assert(corner(1).rgb == colors.hover);
  assert(corner(2).rgb == colors.surface);
  // 分隔线在把手和第一个按钮之间。
  const auto divider = pixel_at(surface, static_cast<int>(layout.divider.x),
                                static_cast<int>(layout.divider.y + layout.divider.height / 2));
  assert(divider.rgb != colors.surface);
  cairo_surface_destroy(surface);
}

// 属主失焦、要输入框的按钮不能用时（IBus）：悬停也不垫底，字画成淡色；菜单里不能用的项点不中。
void disabled_controls() {
  const auto config = host::floating_toolbar_config(nlohmann::json::object(), true, true);
  const auto layout = host::floating_toolbar_layout(config, false);
  const auto colors = host::floating_toolbar_colors(host::CandidateTheme{});
  host::FloatingToolbarState state;
  host::FloatingToolbarFrame frame;
  for (const auto button : config.buttons) {
    frame.faces.push_back(host::floating_toolbar_face(button, state, "水杉输入法"));
    frame.faces.back().enabled = !host::floating_toolbar_button_needs_focus(button);
  }
  frame.hovered = 0;
  auto *surface = canvas(layout.width, layout.height);
  auto *cairo = cairo_create(surface);
  host::paint_floating_toolbar(cairo, layout, frame, colors);
  cairo_destroy(cairo);
  save(surface, "floating-toolbar-unfocused.png");
  const auto &cell = layout.buttons[0];
  assert(pixel_at(surface, static_cast<int>(cell.x + 3), static_cast<int>(cell.y + cell.height / 2)).rgb == colors.surface);
  // 淡色比正常的字浅：按钮里最深的像素也比文字色亮。
  int darkest = 255;
  for (int y = static_cast<int>(cell.y); y < static_cast<int>(cell.y + cell.height); ++y)
    for (int x = static_cast<int>(cell.x); x < static_cast<int>(cell.x + cell.width); ++x)
      darkest = std::min(darkest, static_cast<int>(pixel_at(surface, x, y).rgb & 0xFF));
  assert(darkest > static_cast<int>(colors.text & 0xFF) + 40);
  cairo_surface_destroy(surface);

  auto utility = host::floating_toolbar_utility_menu("水杉输入法");
  for (auto &item : utility)
    item.disabled = !item.separator && host::floating_toolbar_action_needs_focus(item.action);
  const auto menu = host::floating_toolbar_menu_layout(utility, 1.0, false);
  const auto &promotion = menu.rows[3];
  assert(utility[3].action == host::FloatingToolbarAction::Promotion);
  assert(!host::floating_toolbar_menu_hit(menu, utility, promotion.x + 2, promotion.y + 2));
  const auto &settings = menu.rows[1];
  assert(host::floating_toolbar_menu_hit(menu, utility, settings.x + 2, settings.y + 2) == 1u);
}

void menu_and_tooltip() {
  host::CandidateTheme theme;
  theme.dark = true;
  const auto colors = host::floating_toolbar_colors(theme);
  host::FloatingToolbarState state;
  state.scheme = "shuangpin";
  state.schemes = {{"quanpin", "全拼"}, {"shuangpin", "双拼（小鹤）"}, {"wubi", "五笔 86"}};
  const auto items = host::floating_toolbar_scheme_menu(state);
  const auto layout = host::floating_toolbar_menu_layout(items, 1.0, true);
  assert(layout.rows.size() == 3 && layout.plate.width > layout.check_column);
  auto *surface = canvas(layout.width, layout.height);
  auto *cairo = cairo_create(surface);
  host::paint_floating_toolbar_menu(cairo, layout, items, 0, colors);
  cairo_destroy(cairo);
  save(surface, "floating-toolbar-menu-dark.png");
  // 悬停的第一项垫着悬停色，没有悬停的第三项是底色。
  const auto &first = layout.rows[0];
  assert(pixel_at(surface, static_cast<int>(first.x + first.width - 3), static_cast<int>(first.y + first.height / 2)).rgb ==
         colors.hover);
  const auto &third = layout.rows[2];
  assert(pixel_at(surface, static_cast<int>(third.x + third.width - 3), static_cast<int>(third.y + third.height / 2)).rgb ==
         colors.surface);
  // 命中按行算，分隔线不算。
  assert(host::floating_toolbar_menu_hit(layout, items, first.x + 2, first.y + 2) == 0u);
  const auto utility = host::floating_toolbar_utility_menu("水杉输入法");
  const auto utility_layout = host::floating_toolbar_menu_layout(utility, 1.0, false);
  const auto &separator = utility_layout.rows[2];
  assert(!host::floating_toolbar_menu_hit(utility_layout, utility, separator.x + 2, separator.y + 2));
  cairo_surface_destroy(surface);

  const auto tooltip = host::floating_toolbar_tooltip_layout("切换到英文输入", 1.0, false);
  surface = canvas(tooltip.width, tooltip.height);
  cairo = cairo_create(surface);
  host::paint_floating_toolbar_tooltip(cairo, tooltip, "切换到英文输入", colors);
  cairo_destroy(cairo);
  save(surface, "floating-toolbar-tooltip-dark.png");
  assert(pixel_at(surface, 2, static_cast<int>(tooltip.height / 2)).alpha == 255);
  cairo_surface_destroy(surface);
}

}  // namespace

// 用双拼键位图和悬浮工具栏真正的画法画进内存缓冲，检查圆角、底色、高亮键、悬停底、分隔线和菜单，不需要显示服务器。
int main() {
  keymap(false, false, "shuangpin-keymap-light.png");
  keymap(true, true, "shuangpin-keymap-dark-shadow.png");
  toolbar(false, false, "floating-toolbar-grip.png");
  toolbar(true, true, "floating-toolbar-logo-shadow.png");
  disabled_controls();
  menu_and_tooltip();
  return 0;
}
