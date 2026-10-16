import { expect, test } from "vitest";
import { floatingToolbarButtonOptions, floatingToolbarSize, floatingToolbarSizes } from "@msime/ui";
import { defaultFloatingToolbar } from "../../../../packages/ui/src/settings/floating-toolbar-defaults";

test("悬浮工具栏按钮按显示顺序过滤宿主能力，中英文切换也能关", () => {
  const withoutOptional = floatingToolbarButtonOptions(defaultFloatingToolbar, {
    floating_toolbar_input_scheme: false,
    floating_toolbar_handwriting: false,
    floating_toolbar_voice: false,
  });
  expect(withoutOptional.map(({ value }) => value)).toEqual([
    "english_mode",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "screen_keyboard",
    "settings",
  ]);
  // 中/英按钮由 english_mode 管，每个宿主都读它，所以这一项和其他按钮一样可以取消，没有锁定的「始终显示」一项。
  expect(withoutOptional[0]).toEqual({ value: "english_mode", label: "中英文切换", checked: true });
  expect(withoutOptional.some((item) => "disabled" in item)).toBe(false);

  const withAll = floatingToolbarButtonOptions(defaultFloatingToolbar, {
    floating_toolbar_input_scheme: true,
    floating_toolbar_handwriting: true,
    floating_toolbar_voice: true,
  });
  expect(withAll.map(({ value }) => value)).toEqual([
    "english_mode",
    "input_scheme",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "handwriting",
    "screen_keyboard",
    "voice",
    "settings",
  ]);
});

test("工具栏大小的各档只用宿主已经接受的缩放和图标尺寸，最大一档不比以前小", () => {
  expect(floatingToolbarSizes.map(({ percent }) => percent)).toEqual([75, 100, 125, 150, 175]);
  for (const size of floatingToolbarSizes) {
    // macOS、Linux 和 Windows 的读取方只认这四个缩放和 16–28 的图标。
    expect([75, 100, 125, 150]).toContain(size.scale_percent);
    expect(size.font_size).toBeGreaterThanOrEqual(16);
    expect(size.font_size).toBeLessThanOrEqual(28);
    // 每一档的实际大小就是标准大小（100% × 24）乘以它的百分比。
    expect(size.scale_percent * size.font_size).toBe(size.percent * 24);
  }
  // 以前两个下拉都拉满是 150% × 28，合并后最大一档一样大。
  expect(Math.max(...floatingToolbarSizes.map((size) => size.scale_percent * size.font_size))).toBe(
    150 * 28,
  );
});

test("存下的缩放和图标尺寸凑成一档时认出那一档，单独调过的组合不归入任何一档", () => {
  expect(floatingToolbarSize(defaultFloatingToolbar)?.percent).toBe(100);
  expect(floatingToolbarSize({ scale_percent: 150, font_size: 28 })?.percent).toBe(175);
  expect(floatingToolbarSize({ scale_percent: 125, font_size: 24 })?.percent).toBe(125);
  expect(floatingToolbarSize({ scale_percent: 100, font_size: 20 })).toBeUndefined();
});
