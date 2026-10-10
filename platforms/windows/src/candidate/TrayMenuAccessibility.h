#pragma once
#include "AccessibleElements.h"
#include "TrayMenuLayout.h"
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// 托盘卡片（以及悬浮状态栏弹出的两个菜单）交给读屏的元素，对应 macOS 输入菜单那个原生 NSMenu 自带的读屏：每一行是一个菜单项，名字是行的文字，勾选的行报告为已勾选，带快捷键的行报告快捷键，翻页行右侧的当前选择作为补充说明。元素 id 是行号加一，执行一个元素等同于点那一行。

// `keyboard_highlight` 是键盘导航正停着的行；鼠标悬停的高亮不算焦点，免得读屏跟着指针抢读。`scale` 是 DIP 到客户区像素的倍数（窗口 DPI / 96）。
inline AccessibleTree tray_menu_accessible_tree(const std::vector<TrayMenuItem> &items,
                                                const TrayMenuGeometry &geometry, double scale,
                                                std::optional<size_t> keyboard_highlight) {
  AccessibleTree tree{AccessibleContainer::Menu, {}, {}};
  if (geometry.rows.size() != items.size())
    return tree;
  for (size_t index = 0; index < items.size(); ++index) {
    const auto &item = items[index];
    const auto &row = geometry.rows[index];
    AccessibleElement element;
    element.id = static_cast<int>(index + 1);
    element.name = item.label;
    element.bounds = {row.left * scale, row.top * scale, row.right * scale, row.bottom * scale};
    switch (item.kind) {
    case TrayMenuRowKind::Header:
      // 标题行是标志和产品名，菜单本身也用它当名字。
      element.role = AccessibleRole::Text;
      if (tree.name.empty())
        tree.name = item.label;
      break;
    case TrayMenuRowKind::Separator:
      element.role = AccessibleRole::Separator;
      element.enabled = false;
      break;
    case TrayMenuRowKind::Label:
      element.role = AccessibleRole::Text;
      break;
    case TrayMenuRowKind::Item:
      element.role = AccessibleRole::MenuItem;
      element.enabled = item.available;
      element.invokable = tray_menu_actionable(item);
      // 翻页行右侧写的是当前的选择（方案名、主题名），其余行写的是做同一件事的按键。
      if (item.submenu)
        element.help = item.hint;
      else
        element.accelerator = item.hint;
      // 和原生菜单一样只报告勾上的行：大多数行（打开设置之类）根本没有勾选这回事，报告「未勾选」反而误导。
      if (item.checked)
        element.checked = true;
      element.focused = keyboard_highlight == index && element.invokable;
      break;
    case TrayMenuRowKind::Tool:
      element.role = AccessibleRole::MenuItem;
      element.enabled = item.available;
      element.invokable = tray_menu_actionable(item);
      // 开关型的工具格（悬浮状态栏）开着、关着都报告，打开面板的格子没有开关。
      if (item.toggle)
        element.checked = item.checked;
      element.focused = keyboard_highlight == index && element.invokable;
      break;
    }
    tree.elements.push_back(std::move(element));
  }
  return tree;
}

// 讲述人和 NVDA 的读屏键是 Caps Lock 或 Insert。读屏键单按或按着时的按键是读屏的命令（比如 NVDA+Tab 读焦点、讲述人键+方向键逐项读），托盘卡片的键盘钩子既不拿它导航、也不因它收起卡片，原样交给读屏，并算作在用卡片。否则 NVDA 用户每按一次 Insert 卡片就收起，讲述人键+方向键也被卡片吞掉。
inline constexpr unsigned tray_menu_caps_lock_key = 0x14;
inline constexpr unsigned tray_menu_insert_key = 0x2D;
inline bool tray_menu_screen_reader_key(unsigned virtual_key, bool reader_key_down) {
  return reader_key_down || virtual_key == tray_menu_caps_lock_key ||
         virtual_key == tray_menu_insert_key;
}
} // namespace msime::windows
