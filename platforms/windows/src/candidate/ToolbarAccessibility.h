#pragma once
#include "AccessibleElements.h"
#include "ToolbarIcons.h"
#include "ToolbarLayout.h"
#include <cmath>
#include <string>
#include <vector>

namespace msime::windows {
// 悬浮状态栏交给读屏的元素：每个按钮的名称就是它的悬停提示（toolbar_tooltip），与 macOS 工具栏的 accessibilityLabel 和 toolTip 一致；logo 打开时还有一个以产品名为名的图片。AutomationId 沿用 macOS 的 accessibilityIdentifier。

inline constexpr int toolbar_accessible_logo = 100;
inline constexpr const char *toolbar_accessible_name = "悬浮状态栏";
// 按钮的元素 id 是按钮编号加一（kToolbarLanguage 是 0），按钮增减、换位置时同一个按钮的 id 不变。
inline int toolbar_accessible_id(int button) { return button + 1; }

inline const char *toolbar_accessible_automation_id(int button) {
  switch (button) {
  case kToolbarLanguage:
    return "MetasequoiaFloatingToolbarInputMode";
  case kToolbarInputScheme:
    return "MetasequoiaFloatingToolbarInputScheme";
  case kToolbarPunctuation:
    return "MetasequoiaFloatingToolbarPunctuation";
  case kToolbarFullwidth:
    return "MetasequoiaFloatingToolbarFullWidth";
  case kToolbarCharacterSet:
    return "MetasequoiaFloatingToolbarTraditionalOutput";
  case kToolbarEmoji:
    return "MetasequoiaFloatingToolbarEmoji";
  case kToolbarHandwriting:
    return "MetasequoiaFloatingToolbarHandwriting";
  case kToolbarScreenKeyboard:
    return "MetasequoiaFloatingToolbarScreenKeyboard";
  case kToolbarVoice:
    return "MetasequoiaFloatingToolbarVoice";
  case kToolbarSettings:
    return "MetasequoiaFloatingToolbarSettings";
  case kToolbarHide:
    return "MetasequoiaFloatingToolbarHide";
  default:
    return "";
  }
}

// 一个画出来的按钮：编号、名称（UTF-8 的悬停提示）和能不能用。
struct ToolbarAccessibleButton {
  int button = 0;
  std::string name;
  bool usable = true;
};

// `unit` 是工具栏坐标到客户区像素的倍数（toolbar_pixel_unit），与悬停提示的区域同一套换算。
inline AccessibleTree toolbar_accessible_tree(const std::vector<ToolbarAccessibleButton> &buttons,
                                              const ToolbarMetrics &metrics, double unit,
                                              bool show_logo, const std::string &product) {
  AccessibleTree tree{AccessibleContainer::ToolBar, toolbar_accessible_name, {}};
  const auto client = [unit](const ToolbarRect &box) {
    return AccessibleBounds{std::floor(box.left * unit), std::floor(box.top * unit),
                            std::ceil(box.right * unit), std::ceil(box.bottom * unit)};
  };
  // logo 关掉时那里只是拖动用的握把，没有可读的东西。
  if (show_logo) {
    AccessibleElement logo;
    logo.id = toolbar_accessible_logo;
    logo.role = AccessibleRole::Image;
    logo.automation_id = "MetasequoiaFloatingToolbarLogo";
    logo.name = product;
    logo.bounds = client(toolbar_logo(metrics));
    tree.elements.push_back(std::move(logo));
  }
  for (size_t index = 0; index < buttons.size(); ++index) {
    const auto &button = buttons[index];
    AccessibleElement element;
    element.id = toolbar_accessible_id(button.button);
    element.role = AccessibleRole::Button;
    element.automation_id = toolbar_accessible_automation_id(button.button);
    element.name = button.name;
    element.enabled = button.usable;
    element.invokable = button.usable;
    element.bounds = client(toolbar_cell(index, metrics));
    tree.elements.push_back(std::move(element));
  }
  return tree;
}
} // namespace msime::windows
