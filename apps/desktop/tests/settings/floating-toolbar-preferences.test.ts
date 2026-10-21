import { expect, expectTypeOf, test } from "vitest";
import type {
  Preferences,
  FloatingToolbarPreferences as PublicFloatingToolbarPreferences,
} from "@msime/ui";
import { defaultFloatingToolbar } from "../../../../packages/ui/src/settings/floating-toolbar-defaults";
import {
  floatingToolbarPreferences,
  type FloatingToolbarPreferences,
} from "../../../../packages/ui/src/settings/floating-toolbar-preferences";

test("悬浮工具栏叶子模型与公共导出和总偏好保持一致", () => {
  expectTypeOf<FloatingToolbarPreferences>().toEqualTypeOf<PublicFloatingToolbarPreferences>();
  expectTypeOf<FloatingToolbarPreferences>().toEqualTypeOf<
    NonNullable<Preferences["floating_toolbar"]>
  >();
});

test("fills missing floating toolbar fields from the shared defaults", () => {
  expect(floatingToolbarPreferences()).toEqual(defaultFloatingToolbar);
  expect(
    floatingToolbarPreferences({ floating_toolbar: { enabled: false, font_size: 20 } }),
  ).toMatchObject({ ...defaultFloatingToolbar, enabled: false, font_size: 20 });
});

test("自动隐藏按 Windows v0.11.1 出厂配置投影：关、延时 5 秒", () => {
  const toolbar = floatingToolbarPreferences();
  expect(toolbar.auto_hide).toBe(false);
  expect(toolbar.auto_hide_delay).toBe(5);
  // 旧文档缺这两个键时也走同一套缺省，不因缺键把自动隐藏当成开。
  expect(floatingToolbarPreferences({ floating_toolbar: { enabled: true } })).toMatchObject({
    auto_hide: false,
    auto_hide_delay: 5,
  });
});
