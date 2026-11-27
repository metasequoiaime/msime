#pragma once

// 设置页「?」的说明气泡点开后会固定显示，之后要能被普通的操作收起：点页面别处、按 Esc、滚动、换页、切换明暗时的整窗重画、切到别的窗口，以及再点一次同一颗问号。共享设置页（`packages/ui/src/core/info-tip.tsx` 的 outside pointerdown / Escape / scroll / resize）、iOS、Android 和 Win7 设置都是这样；Win11 设置页原来只在点同一颗问号时才关，点开的说明会一直挂在窗口上，点几个就叠几张，整页重画把问号换掉了它也不走。
//
// 收起不去遍历整棵树关掉那几百颗「?」：悬停弹出的那些归 WinUI 的 ToolTipService 自己管，只有点开的那一颗是我们让它保持打开的。所以这里只记「当前点开的那一颗」，由窗口在那些收起点上叫一次 close。记的是弱引用加 ABI 指针：整页重画会换掉问号和气泡，原来的固定状态随之作废，此后什么也不做。

#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <winrt/Microsoft.UI.Xaml.h>

namespace msime::settings::ui {

namespace {
using ToolTip = winrt::Microsoft::UI::Xaml::Controls::ToolTip;
using ToolTipService = winrt::Microsoft::UI::Xaml::Controls::ToolTipService;
using DependencyObject = winrt::Microsoft::UI::Xaml::DependencyObject;
using FrameworkElement = winrt::Microsoft::UI::Xaml::FrameworkElement;
}  // namespace

// `element` 是不是 `ancestor` 本身或它的子孙。`ancestor` 为空时恒为假；遍历只走 VisualTreeHelper 的父链，气泡里有自己的弹出结构，走到头自然停下。
inline bool within(winrt::Microsoft::UI::Xaml::DependencyObject const &element,
                   winrt::Microsoft::UI::Xaml::DependencyObject const &ancestor) {
  if (!element || !ancestor)
    return false;
  for (auto current = element; current; current = winrt::Microsoft::UI::Xaml::Media::VisualTreeHelper::GetParent(current))
    if (winrt::get_abi(current) == winrt::get_abi(ancestor))
      return true;
  return false;
}

class InfoTipPins {
 public:
  InfoTipPins() = default;
  InfoTipPins(InfoTipPins const &) = delete;
  InfoTipPins &operator=(InfoTipPins const &) = delete;

  // 点了一颗问号：它正固定着就收起，否则固定它。同一时刻只固定一颗，点了另一颗时原来那颗先收起。
  void toggle(winrt::Microsoft::UI::Xaml::FrameworkElement const &anchor) {
    auto tip = tooltip_of(anchor);
    if (!tip)
      return;
    if (tip.IsOpen() && winrt::get_abi(anchor) == anchor_abi_)
      dismiss();
    else
      pin(anchor, tip);
  }

  // 点了页面别处。落在问号或它的气泡上时不算「别处」：那里的点击先到达按钮、跑完 toggle，再冒到窗口的指针处理里，这里若再关一次，就把刚固定下来的又关掉。
  void close_unless_inside(bool on_the_tip) {
    if (!on_the_tip)
      dismiss();
  }

  // Esc、换页、滚动、窗口失活、整窗重画：无条件收起。
  void close() { dismiss(); }

  // 有没有正固定着的气泡。Esc 只在有的时候才吞掉按键，免得抢走别的用途。
  bool open() { return static_cast<bool>(current().anchor); }

  // 指针落点是否在当前固定的问号或它的气泡里。气泡是独立的弹出层，点在里面的事件走不到窗口，这里多判一次不影响行为。
  bool contains(winrt::Microsoft::UI::Xaml::DependencyObject const &source) {
    const auto pinned = current();
    if (!pinned.anchor)
      return false;
    return within(source, pinned.anchor) || within(source, pinned.tip);
  }

 private:
  struct Pinned {
    FrameworkElement anchor{nullptr};
    ToolTip tip{nullptr};
  };

  static ToolTip tooltip_of(FrameworkElement const &anchor) {
    return ToolTipService::GetToolTip(anchor).try_as<ToolTip>();
  }

  void pin(FrameworkElement const &anchor, ToolTip const &tip) {
    dismiss();
    anchor_ = winrt::make_weak(anchor);
    pinned_ = winrt::make_weak(tip);
    anchor_abi_ = winrt::get_abi(anchor);
    tip_abi_ = winrt::get_abi(tip);
    tip.IsOpen(true);
  }

  // 只关我们自己固定过的那一颗，悬停弹出的交回 WinUI 按指针离开自己关。
  void dismiss() {
    const auto pinned = current();
    if (pinned.tip)
      pinned.tip.IsOpen(false);
    anchor_ = {};
    pinned_ = {};
    anchor_abi_ = nullptr;
    tip_abi_ = nullptr;
  }

  // 两个弱引用都还在、都还是当初记下的那两个对象时返回它们；整页重画换掉了问号或气泡就是空的。
  Pinned current() {
    const auto anchor = anchor_.get();
    const auto tip = pinned_.get();
    if (!anchor || !tip || winrt::get_abi(anchor) != anchor_abi_ ||
        winrt::get_abi(tip) != tip_abi_ || winrt::get_abi(tooltip_of(anchor)) != tip_abi_)
      return {};
    return {anchor, tip};
  }

  winrt::weak_ref<FrameworkElement> anchor_{};
  winrt::weak_ref<ToolTip> pinned_{};
  const void *anchor_abi_ = nullptr;
  const void *tip_abi_ = nullptr;
};

}  // namespace msime::settings::ui
