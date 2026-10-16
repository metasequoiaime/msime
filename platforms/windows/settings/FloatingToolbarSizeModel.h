#pragma once
#include <array>
#include <optional>

// 「悬浮工具栏」页的「工具栏大小」，照共享设置的 `packages/ui/src/settings/floating-toolbar-options.ts`（`floatingToolbarSizes`、`floatingToolbarSize`）。工具栏的按钮和图标按 `font_size × scale_percent` 画，原来的「工具栏缩放」和「图标尺寸」两个下拉调的是同一个结果，所以页面只给一个大小，选一档同时写 `floating_toolbar.scale_percent` 和 `floating_toolbar.font_size`。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::toolbar_size {

// 一档：`percent` 是相对标准大小（100% 缩放、24 图标）的大小。
struct Size {
  int percent;
  int scale_percent;
  int font_size;
};

// 前四档只改缩放、图标保持默认的 24；175% 用 150% 缩放加 28 的图标（150 × 28 = 175 × 24），和以前两个下拉都拉满时一样大。各档只用宿主已经接受的缩放（75–150，`FloatingToolbarSettings::valid` 对超出的缩放会整份拒绝工具栏设置）和图标尺寸（16–28）。
inline constexpr std::array<Size, 5> sizes{{
    {75, 75, 24},
    {100, 100, 24},
    {125, 125, 24},
    {150, 150, 24},
    {175, 150, 28},
}};

// 存下的缩放和图标尺寸对应的那一档；以前分开调过、凑不成任何一档时为空，页面照实显示「自定义」而不改写存储。
constexpr std::optional<Size> size_for(int scale_percent, int font_size) {
  for (const auto &size : sizes)
    if (size.scale_percent == scale_percent && size.font_size == font_size)
      return size;
  return std::nullopt;
}

} // namespace msime::settings::toolbar_size
