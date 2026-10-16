#include "../../settings/FloatingToolbarSizeModel.h"
#include <cstdio>

using namespace msime::settings::toolbar_size;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
bool same(Size const &a, Size const &b) {
  return a.percent == b.percent && a.scale_percent == b.scale_percent && a.font_size == b.font_size;
}
} // namespace

int main() {
  // 与共享页 floating-toolbar-options.ts 的 floatingToolbarSizes 逐档相同：前四档只改缩放，175% 写成 150% 缩放加 28 的图标。
  constexpr Size expected[] = {
      {75, 75, 24}, {100, 100, 24}, {125, 125, 24}, {150, 150, 24}, {175, 150, 28}};
  check(sizes.size() == 5, "five sizes");
  for (std::size_t i = 0; i < sizes.size(); ++i)
    check(same(sizes[i], expected[i]), "each size matches the shared table");
  // 每档都只用宿主接受的缩放（75–150）和图标尺寸（16–28），宿主不用跟着改。
  for (const auto &size : sizes) {
    check(size.scale_percent >= 75 && size.scale_percent <= 150, "scale stays inside 75-150");
    check(size.font_size >= 16 && size.font_size <= 28, "icon size stays inside 16-28");
    check(size.scale_percent * size.font_size == size.percent * 24, "percent is scale times icon over the standard 24");
  }
  // 存下的两个值找回自己那一档。
  for (const auto &size : sizes) {
    const auto found = size_for(size.scale_percent, size.font_size);
    check(found.has_value() && same(*found, size), "a stored step finds itself");
  }
  check(size_for(100, 24)->percent == 100, "the defaults are 100%");
  check(size_for(150, 28)->percent == 175, "150% with 28 is 175%");
  // 以前分开调出来的组合凑不成任何一档，页面显示「自定义」而不改写。
  check(!size_for(125, 20).has_value(), "125% with 20 is custom");
  check(!size_for(75, 28).has_value(), "75% with 28 is custom");
  check(!size_for(175, 24).has_value(), "a literal 175% scale is not a step");
  static_assert(size_for(150, 28).has_value());
  static_assert(!size_for(125, 20).has_value());
  return failures == 0 ? 0 : 1;
}
