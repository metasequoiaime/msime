import { expect, test } from "vitest";
import { relativeLuminance } from "../../../../packages/ui/src/core/color";

test.each([
  [0x000000, 0],
  [0xffffff, 1],
  [0xff0000, 0.2126],
  [0x00ff00, 0.7152],
  [0x0000ff, 0.0722],
  [0x808080, 0.21586050011389926],
  [0x0a0a0a, 0.003035269835488375],
  [0x0b0b0b, 0.003346535763899161],
])("RGB %i 的相对亮度为 %f", (rgb, expected) => {
  expect(relativeLuminance(rgb)).toBeCloseTo(expected, 15);
});
