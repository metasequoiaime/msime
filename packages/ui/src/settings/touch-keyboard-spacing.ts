import { clamp } from "../core/number";
import { defaultTouchKeyboardGeometry } from "./touch-keyboard-geometry-defaults";

/** 设置滑块、拖动与渲染共用的间距范围，单位为十分之一 dp。 */
export const touchKeyboardSpacingLimits = {
  key: { min: 30, max: 60 },
  row: { min: 40, max: 100 },
} as const;

/** 将存储间距换算为绘制用的 dp，并按可编辑范围裁剪。 */
export function touchKeyboardSpacing(
  keySpacingTenths: number = defaultTouchKeyboardGeometry.keySpacingTenths,
  rowSpacingTenths: number = defaultTouchKeyboardGeometry.rowSpacingTenths,
) {
  const { key, row } = touchKeyboardSpacingLimits;
  return {
    keySpacing: clamp(keySpacingTenths / 10, key.min / 10, key.max / 10),
    rowSpacing: clamp(rowSpacingTenths / 10, row.min / 10, row.max / 10),
  };
}
