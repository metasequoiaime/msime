import { expect, test } from "vitest";
import { readableKeyboardText } from "../../../../packages/ui/src/keyboard/keyboard-colors";
import {
  readableSkinText,
  skinContrast,
  skinLuminance,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import {
  candidatePaletteStyle,
  customCandidatePalette,
  type CandidateThemePalette,
} from "../../../../packages/ui/src/theme/global-theme";

test("皮肤旧接口与文字可读性保留原有阈值和输出格式", () => {
  expect(skinLuminance(0xff0000)).toBeCloseTo(0.2126, 15);
  expect(skinContrast(0x000000, 0xffffff)).toBe(21);
  expect(readableSkinText(0x757575)).toBe(0xffffff);
  expect(readableSkinText(0x767676)).toBe(0x000000);
  expect(readableKeyboardText("#757575")).toBe("#ffffff");
  expect(readableKeyboardText("#767676")).toBe("#000000");
  expect(readableKeyboardText("invalid")).toBe("#ffffff");
});

test.each([
  ["#FF0000", "#000000", "#FFFFFF"],
  ["#B2B2B2", "#000000", "#FFFFFF"],
  ["#B3B3B3", "#000000", "#1A1A1A"],
])("选中底色 %s 的候选和 macOS 前景保留各自规则", (selected, text, macText) => {
  const palette = customCandidatePalette("system", { selected })!;
  expect(palette.selected_text).toBe(text);
  const unsetForegrounds: CandidateThemePalette = {
    ...palette,
    selected_text: null,
    selected_number: null,
  };
  const opaque = candidatePaletteStyle({ ...unsetForegrounds, selected: `${selected}FF` });
  expect(opaque).toMatchObject({
    "--cand-mac-selected-text": macText,
    "--cand-mac-selected-num": `${macText}D1`,
  });
  expect(candidatePaletteStyle({ ...unsetForegrounds, selected: `${selected}40` })).toMatchObject({
    "--cand-mac-selected-text": "var(--cand-text)",
    "--cand-mac-selected-num": "var(--cand-num)",
  });
});
