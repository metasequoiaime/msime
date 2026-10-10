#pragma once
#include <cmath>
#include <cstddef>
#include <optional>
#include <string>
#include <tuple>
#include <vector>

namespace msime::windows {
// Server 自绘窗口（候选窗、悬浮状态栏、托盘卡片）交给读屏软件的元素树。这里只有数据和换算，不碰 Win32：各窗口按刚画好的布局算出一棵树，AccessibleWindow（UI Automation 提供者）照着它回答读屏的查询，规则因此能在宿主机上测试。
//
// 文字一律是 UTF-8，交给 UI Automation 时才转成 UTF-16。坐标是窗口客户区的像素，和鼠标消息、悬停提示的区域同一个坐标系。

// 元素的种类，各对应一种 UI Automation 控件类型。
enum class AccessibleRole { Button, ListItem, MenuItem, Text, Image, Separator };
// 整棵树（窗口本身）的控件类型：候选窗是列表，悬浮状态栏是工具栏，托盘卡片和工具栏弹出的菜单是菜单。
enum class AccessibleContainer { List, ToolBar, Menu };

struct AccessibleBounds {
  double left = 0.0, top = 0.0, right = 0.0, bottom = 0.0;
};
inline bool operator==(const AccessibleBounds &a, const AccessibleBounds &b) {
  return a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom;
}

struct AccessibleElement {
  // 在一棵树里唯一，也是 UI Automation 的 RuntimeId 后缀和执行请求带回窗口的编号。同一个东西在前后两棵树里用同一个 id，读屏停在它上面时树换了也能接着读。
  int id = 0;
  AccessibleRole role = AccessibleRole::Text;
  // UI Automation 的 AutomationId，与 macOS 的 accessibilityIdentifier 对应；空时不报告。
  std::string automation_id;
  std::string name;
  // 补充说明（UI Automation 的 HelpText），与 macOS 的 toolTip 对应。
  std::string help;
  // 菜单行的快捷键（UI Automation 的 AcceleratorKey），比如 Ctrl + .。
  std::string accelerator;
  // 有值时提供只读的 Value 模式，比如候选窗预编辑里正在输入的拼音。
  std::optional<std::string> value;
  AccessibleBounds bounds;
  bool enabled = true;
  // 读屏可以执行它（Invoke 模式）：等同于用鼠标点它一下。
  bool invokable = false;
  // 有值时提供 Toggle 模式：勾选的菜单行、开着的工具格。
  std::optional<bool> checked;
  // 键盘导航正停在它上面，报告为键盘焦点。
  bool focused = false;
};
inline bool operator==(const AccessibleElement &a, const AccessibleElement &b) {
  return std::tie(a.id, a.role, a.automation_id, a.name, a.help, a.accelerator, a.value,
                  a.bounds, a.enabled, a.invokable, a.checked, a.focused) ==
         std::tie(b.id, b.role, b.automation_id, b.name, b.help, b.accelerator, b.value,
                  b.bounds, b.enabled, b.invokable, b.checked, b.focused);
}

struct AccessibleTree {
  AccessibleContainer container = AccessibleContainer::List;
  std::string name;
  std::vector<AccessibleElement> elements;
};
inline bool operator==(const AccessibleTree &a, const AccessibleTree &b) {
  return a.container == b.container && a.name == b.name && a.elements == b.elements;
}
inline bool operator!=(const AccessibleTree &a, const AccessibleTree &b) { return !(a == b); }

// 客户区到屏幕物理像素的换算：窗口左上角的屏幕物理坐标，以及客户区一个单位是几个物理像素。Server 进程本身不感知 DPI，各窗口的感知方式不同（候选窗和托盘卡片按每显示器感知建立，悬浮状态栏没有），读屏要的又总是物理像素，所以按窗口的物理大小和它自己坐标系里的客户区大小之比换算，不依赖调用线程的 DPI 上下文。
struct AccessibleFrame {
  double left = 0.0, top = 0.0, scale_x = 1.0, scale_y = 1.0;
};
inline std::optional<AccessibleFrame> accessible_frame(double screen_left, double screen_top,
                                                       double screen_width, double screen_height,
                                                       double client_width, double client_height) {
  if (!std::isfinite(screen_left) || !std::isfinite(screen_top) || !(screen_width > 0.0) ||
      !(screen_height > 0.0) || !(client_width > 0.0) || !(client_height > 0.0) ||
      !std::isfinite(screen_width) || !std::isfinite(screen_height) ||
      !std::isfinite(client_width) || !std::isfinite(client_height))
    return std::nullopt;
  return AccessibleFrame{screen_left, screen_top, screen_width / client_width,
                         screen_height / client_height};
}
inline AccessibleBounds accessible_screen_bounds(const AccessibleBounds &bounds,
                                                 const AccessibleFrame &frame) {
  return {frame.left + bounds.left * frame.scale_x, frame.top + bounds.top * frame.scale_y,
          frame.left + bounds.right * frame.scale_x, frame.top + bounds.bottom * frame.scale_y};
}
// 屏幕物理坐标下的一点落在哪个元素上，供 ElementProviderFromPoint 用。分隔线不算，只是一条细线；落在空白处时为空，由窗口自己（整棵树的根）回答。
inline std::optional<size_t> accessible_hit(const AccessibleTree &tree,
                                            const AccessibleFrame &frame, double x,
                                            double y) {
  for (size_t index = 0; index < tree.elements.size(); ++index) {
    const auto &element = tree.elements[index];
    if (element.role == AccessibleRole::Separator)
      continue;
    const auto box = accessible_screen_bounds(element.bounds, frame);
    if (x >= box.left && x < box.right && y >= box.top && y < box.bottom)
      return index;
  }
  return std::nullopt;
}
inline const AccessibleElement *accessible_element(const AccessibleTree &tree, int id) {
  for (const auto &element : tree.elements)
    if (element.id == id)
      return &element;
  return nullptr;
}
// 一棵树里键盘焦点所在的元素，没有时为空。
inline const AccessibleElement *accessible_focus(const AccessibleTree &tree) {
  for (const auto &element : tree.elements)
    if (element.focused)
      return &element;
  return nullptr;
}
} // namespace msime::windows
