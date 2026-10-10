// 悬浮状态栏和托盘卡片交给读屏（UI Automation）的元素树，以及元素坐标到屏幕物理像素的换算。名称和 macOS 工具栏、输入菜单读屏读到的一致。
#include "AccessibleElements.h"
#include "ToolbarAccessibility.h"
#include "ToolbarTooltips.h"
#include "TrayMenuAccessibility.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Accessible element check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

const AccessibleElement &element(const AccessibleTree &tree, int id) {
  const auto *found = accessible_element(tree, id);
  if (!found)
    throw std::runtime_error("Missing accessible element " + std::to_string(id));
  return *found;
}

void frame_and_hit() {
  // 不感知 DPI 的窗口在 150% 的屏幕上：客户区 100x40，屏幕上占 150x60 物理像素。
  const auto frame = accessible_frame(1000.0, 500.0, 150.0, 60.0, 100.0, 40.0);
  require(frame);
  require(frame->scale_x == 1.5 && frame->scale_y == 1.5);
  const auto box = accessible_screen_bounds({10.0, 4.0, 30.0, 20.0}, *frame);
  require(box.left == 1015.0 && box.top == 506.0 && box.right == 1045.0 && box.bottom == 530.0);
  // 窗口还没有大小（刚建好、已经隐藏）时没有换算。
  require(!accessible_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
  require(!accessible_frame(0.0, 0.0, 10.0, 10.0, 0.0, 10.0));

  AccessibleTree tree{AccessibleContainer::Menu, "菜单", {}};
  AccessibleElement line;
  line.id = 1;
  line.role = AccessibleRole::Separator;
  line.bounds = {0.0, 0.0, 100.0, 10.0};
  AccessibleElement row;
  row.id = 2;
  row.role = AccessibleRole::MenuItem;
  row.bounds = {0.0, 0.0, 100.0, 20.0};
  tree.elements = {line, row};
  // 分隔线不接点：同一处落在下面的菜单行上。
  require(accessible_hit(tree, *frame, 1010.0, 505.0) == std::optional<size_t>(1));
  // 右边和下边不含，和鼠标的命中一致。
  require(!accessible_hit(tree, *frame, 1150.0, 505.0));
  require(!accessible_hit(tree, *frame, 1010.0, 530.0));
  require(!accessible_focus(tree));
  tree.elements[1].focused = true;
  require(accessible_focus(tree) == &tree.elements[1]);
  // 树相同才算相同：焦点换了要重新发布。
  AccessibleTree copy = tree;
  require(copy == tree);
  copy.elements[1].focused = false;
  require(copy != tree);
}

void toolbar() {
  const auto metrics = toolbar_metrics(24.0, true, true);
  ToolbarLanguageState language;
  const auto title = toolbar_scheme_title("shuangpin", "xiaohe", "wubi86");
  // 名称就是悬停提示，与 macOS 工具栏 accessibilityLabel == toolTip 一致。
  const std::wstring language_tip =
      toolbar_tooltip(kToolbarLanguage, true, language, title, true, L"水杉输入法");
  require(language_tip == L"小鹤双拼 · 切换到英文输入");
  const std::vector<ToolbarAccessibleButton> buttons{
      {kToolbarLanguage, "小鹤双拼 · 切换到英文输入", true},
      {kToolbarPunctuation, "切换到西文标点", true},
      {kToolbarHandwriting, "打开水杉手写识别板", false},
      {kToolbarSettings, "打开水杉输入法设置", true}};
  const auto tree = toolbar_accessible_tree(buttons, metrics, 1.5, true, "水杉输入法");
  require(tree.container == AccessibleContainer::ToolBar);
  require(tree.name == "悬浮状态栏");
  require(tree.elements.size() == 5);
  // logo 在最前，是以产品名为名的图片，和 macOS 的 MetasequoiaFloatingToolbarLogo 一样。
  const auto &logo = tree.elements.front();
  require(logo.id == toolbar_accessible_logo && logo.role == AccessibleRole::Image);
  require(logo.name == "水杉输入法" && logo.automation_id == "MetasequoiaFloatingToolbarLogo");
  require(!logo.invokable);
  // 按钮的 id 跟着按钮编号走，与它排第几无关。
  const auto &punctuation = element(tree, toolbar_accessible_id(kToolbarPunctuation));
  require(punctuation.role == AccessibleRole::Button && punctuation.name == "切换到西文标点");
  require(punctuation.automation_id == "MetasequoiaFloatingToolbarPunctuation");
  require(punctuation.enabled && punctuation.invokable);
  // 区域与悬停提示的区域同一套换算：第二个格子按 1.5 倍取整。
  const auto cell = toolbar_cell(1, metrics);
  require(punctuation.bounds.left == std::floor(cell.left * 1.5));
  require(punctuation.bounds.right == std::ceil(cell.right * 1.5));
  require(punctuation.bounds.top == std::floor(cell.top * 1.5));
  // 共享应用不在时手写按钮不可用，读屏也执行不了它。
  const auto &handwriting = element(tree, toolbar_accessible_id(kToolbarHandwriting));
  require(!handwriting.enabled && !handwriting.invokable);
  require(element(tree, toolbar_accessible_id(kToolbarSettings)).name == "打开水杉输入法设置");
  // logo 关掉时那里只是握把，没有图片元素。
  const auto grip = toolbar_accessible_tree(buttons, toolbar_metrics(24.0, true, false), 1.0, false,
                                            "水杉输入法");
  require(grip.elements.size() == 4 && !accessible_element(grip, toolbar_accessible_logo));
  require(toolbar_accessible_automation_id(kToolbarLanguage) ==
          std::string("MetasequoiaFloatingToolbarInputMode"));
  require(toolbar_accessible_automation_id(kToolbarCharacterSet) ==
          std::string("MetasequoiaFloatingToolbarTraditionalOutput"));
}

void tray() {
  TrayMenuCapabilities capabilities;
  capabilities.settings = true;
  capabilities.emoji_panel = true;
  capabilities.product_name = "水杉输入法";
  TrayMenuState state;
  state.chinese = true;
  state.fullwidth = false;
  state.chinese_punctuation = true;
  state.floating_toolbar = true;
  state.theme_title = "跟随系统";
  state.themes = {{"system", "跟随系统"}};
  const auto items = tray_menu_items(capabilities, state);
  const auto geometry = tray_menu_geometry(items, TrayMenuMetrics{});
  const auto find = [&](const std::string &label) {
    for (size_t index = 0; index < items.size(); ++index)
      if (items[index].label == label)
        return index;
    throw std::runtime_error("Missing tray row " + label);
  };
  const size_t chinese = find("中文");
  const auto tree = tray_menu_accessible_tree(items, geometry, 1.25, chinese);
  require(tree.container == AccessibleContainer::Menu);
  // 菜单以标题行的产品名为名。
  require(tree.name == "水杉输入法");
  require(tree.elements.size() == items.size());
  for (size_t index = 0; index < items.size(); ++index)
    require(tree.elements[index].id == static_cast<int>(index + 1));
  require(tree.elements.front().role == AccessibleRole::Text);
  require(tree.elements[1].role == AccessibleRole::Separator && !tree.elements[1].enabled);
  // 勾上的行报告已勾选，键盘高亮的行报告焦点，区域按 DPI 缩放。
  const auto &row = tree.elements[chinese];
  require(row.role == AccessibleRole::MenuItem && row.name == "中文");
  require(row.checked == std::optional<bool>(true) && row.focused && row.invokable);
  require(row.bounds.top == geometry.rows[chinese].top * 1.25);
  require(row.bounds.right == geometry.rows[chinese].right * 1.25);
  require(accessible_focus(tree) == &row);
  // 没勾上的行不报告勾选状态，免得「打开设置」之类的行被读成「未勾选」。
  require(!tree.elements[find("英文")].checked);
  require(!tree.elements[find("设置…")].checked);
  // 快捷键归 AcceleratorKey，翻页行右侧的当前选择归补充说明。
  require(tree.elements[find("中文标点")].accelerator == tray_menu_punctuation_hint);
  require(tree.elements[find("中文标点")].checked == std::optional<bool>(true));
  const auto &theme = tree.elements[find("主题")];
  require(theme.help == "跟随系统" && theme.accelerator.empty());
  // 开关型工具格开着、关着都报告；打开面板的格子没有开关。
  require(tree.elements[find("工具栏")].checked == std::optional<bool>(true));
  require(!tree.elements[find("表情")].checked);
  // 没有对应能力的格子不可用，也执行不了。
  const auto &voice = tree.elements[find("语音")];
  require(!voice.enabled && !voice.invokable);
  // 鼠标悬停（没有键盘高亮）时没有焦点。
  require(!accessible_focus(tray_menu_accessible_tree(items, geometry, 1.0, std::nullopt)));
  // 高亮停在不可执行的行上不算焦点。
  require(!accessible_focus(tray_menu_accessible_tree(items, geometry, 1.0, size_t{0})));
  // 行和几何对不上（正在重建）时给一棵空树，不按错位的区域报告。
  require(tray_menu_accessible_tree(items, TrayMenuGeometry{}, 1.0, std::nullopt).elements.empty());
}
// 读屏键（Caps Lock、Insert）单按或按着时的按键交给读屏，卡片的键盘钩子不拿它导航也不收起卡片；没按读屏键时照常按卡片的规则走。
void screen_reader_keys() {
  require(tray_menu_screen_reader_key(tray_menu_caps_lock_key, false));
  require(tray_menu_screen_reader_key(tray_menu_insert_key, false));
  require(tray_menu_screen_reader_key(0x28, true) && tray_menu_screen_reader_key(0x09, true));
  require(!tray_menu_screen_reader_key(0x28, false) && !tray_menu_screen_reader_key(0x0D, false));
  require(!tray_menu_screen_reader_key('A', false));
}
} // namespace

int main() {
  try {
    frame_and_hit();
    toolbar();
    tray();
    screen_reader_keys();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
