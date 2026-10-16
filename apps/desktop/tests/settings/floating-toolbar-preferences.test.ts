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
