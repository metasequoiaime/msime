#pragma once

#include "FloatingToolbarModel.h"

#include <vector>

namespace msime::linux_host {

// 弹出菜单的几何，逻辑像素：14 的字，一行高 28，分隔线一行 9，左边留 28 给勾，右边留 14，上下各 6 的内边距，圆角 12。乘工具栏的 scale_percent，与工具栏一起放大。
// 单独成头是为了让候选菜单的头只带上这份几何，不牵进 FloatingToolbarPainter.h 的 pango 绘制依赖：
// 没有 pangocairo 的构建里 ClientEngine.cpp 仍要能编过（悬浮面板本身不会被编译）。
struct FloatingToolbarMenuLayout {
  double width = 0;
  double height = 0;
  double margin = 0;
  FloatingToolbarRect plate;
  std::vector<FloatingToolbarRect> rows;
  double font_size = 0;
  double check_column = 0;
  double radius = 0;
  double scale = 1;
};

}  // namespace msime::linux_host
