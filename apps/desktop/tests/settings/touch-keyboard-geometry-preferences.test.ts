import { expect, test } from "vitest";
import { defaultTouchKeyboardGeometry } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-defaults";
import { touchKeyboardGeometryPreferences } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-preferences";

test("uses shared defaults for missing touch keyboard geometry fields", () => {
  expect(touchKeyboardGeometryPreferences()).toEqual(defaultTouchKeyboardGeometry);
  expect(touchKeyboardGeometryPreferences({ touch_keyboard_height_adjustment: -12 })).toEqual({
    ...defaultTouchKeyboardGeometry,
    heightAdjustment: -12,
  });
});

test("keeps explicitly stored zero and other geometry values", () => {
  expect(
    touchKeyboardGeometryPreferences({
      touch_keyboard_height_adjustment: 0,
      touch_key_spacing_tenths: 0,
      touch_row_spacing_tenths: 0,
    }),
  ).toEqual({ heightAdjustment: 0, keySpacingTenths: 0, rowSpacingTenths: 0 });
});

test("reads saved geometry without clamping or modifying preferences", () => {
  const source = Object.freeze({
    touch_keyboard_height_adjustment: -20,
    touch_key_spacing_tenths: 65,
    touch_row_spacing_tenths: 110,
  });
  expect(touchKeyboardGeometryPreferences(source)).toEqual({
    heightAdjustment: -20,
    keySpacingTenths: 65,
    rowSpacingTenths: 110,
  });
});
