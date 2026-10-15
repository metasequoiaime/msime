import { expect, test } from "vitest";
import {
  skinDecorationGeometry,
  skinPreviewBackground,
} from "../../../../packages/ui/src/skin/skin-preview-geometry";

test.each([
  [32.5, 150, true, 32.5, 150],
  [500, 1000, true, 500, 1000],
  [0, 150, false, 0, 0],
  [32, 0, false, 0, 0],
  [-1, 150, false, 0, 0],
  [501, 150, false, 0, 0],
  [32, 1001, false, 0, 0],
  [Infinity, 150, false, 0, 0],
  [32, NaN, false, 0, 0],
] as const)(
  "resolves decoration %s/%s without leaving a partial band",
  (top, width, decorated, drawnTop, drawnWidth) => {
    expect(skinDecorationGeometry({ decorationTopDip: top, decorationWidthDip: width })).toEqual({
      decorated,
      top: drawnTop,
      width: drawnWidth,
    });
  },
);

test.each([
  [undefined, 0.35],
  [50, 0.175],
  [60, 0.21],
  [100, 0.35],
  [0, 0.35],
  [101, 0.35],
  [60.5, 0.35],
  [NaN, 0.35],
  [Infinity, 0.35],
])("fades a prepared background with window opacity %s", (percent, opacity) => {
  const background = Object.freeze({
    url: "data:image/png;base64,AA==",
    fit: "stretch" as const,
    opacity: 0.35,
  });
  const drawn = skinPreviewBackground(background, percent);
  expect(drawn?.opacity).toBeCloseTo(opacity);
  expect(drawn).toMatchObject({ url: background.url, fit: "stretch" });
  expect(background.opacity).toBe(0.35);
});

test("keeps absent and fully transparent backgrounds unchanged in meaning", () => {
  expect(skinPreviewBackground(undefined, 60)).toBeUndefined();
  expect(
    skinPreviewBackground({ url: "data:image/png;base64,AA==", fit: "cover", opacity: 0 }, 60),
  ).toEqual({
    url: "data:image/png;base64,AA==",
    fit: "cover",
    opacity: 0,
  });
});
