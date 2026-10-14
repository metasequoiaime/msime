import type { Preferences } from "../index";
import { defaultTouchKeyboardGeometry } from "./touch-keyboard-geometry-defaults";

export type TouchKeyboardGeometryPreferencesSource = Pick<
  Preferences,
  "touch_keyboard_height_adjustment" | "touch_key_spacing_tenths" | "touch_row_spacing_tenths"
>;

/** 解析触屏键盘的有效几何值，并保留显式的零。 */
export function touchKeyboardGeometryPreferences(source?: TouchKeyboardGeometryPreferencesSource) {
  return {
    heightAdjustment:
      source?.touch_keyboard_height_adjustment ?? defaultTouchKeyboardGeometry.heightAdjustment,
    keySpacingTenths:
      source?.touch_key_spacing_tenths ?? defaultTouchKeyboardGeometry.keySpacingTenths,
    rowSpacingTenths:
      source?.touch_row_spacing_tenths ?? defaultTouchKeyboardGeometry.rowSpacingTenths,
  };
}
