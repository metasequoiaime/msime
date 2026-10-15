import { expect, test } from "vitest";
import { touchKeyboardSpacing } from "../../../../packages/ui/src/settings/touch-keyboard-spacing";

test("uses default spacing independently for omitted dimensions", () => {
  expect(touchKeyboardSpacing()).toEqual({ keySpacing: 6, rowSpacing: 7 });
  expect(touchKeyboardSpacing(35)).toEqual({ keySpacing: 3.5, rowSpacing: 7 });
  expect(touchKeyboardSpacing(undefined, 95)).toEqual({ keySpacing: 6, rowSpacing: 9.5 });
});

test.each([
  [35, 95, 3.5, 9.5],
  [0, 0, 3, 4],
  [65, 110, 6, 10],
  [30, 40, 3, 4],
  [60, 100, 6, 10],
  [-Infinity, Infinity, 3, 10],
])("converts %s/%s tenths to bounded dp", (key, row, keySpacing, rowSpacing) => {
  expect(touchKeyboardSpacing(key, row)).toEqual({ keySpacing, rowSpacing });
});

test("preserves non-finite rendering semantics independently per dimension", () => {
  expect(touchKeyboardSpacing(NaN, 70)).toEqual({ keySpacing: NaN, rowSpacing: 7 });
  expect(touchKeyboardSpacing(60, NaN)).toEqual({ keySpacing: 6, rowSpacing: NaN });
});
