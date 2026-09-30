import { expect, test } from "vitest";
import {
  updateSavedTouchKeyboardSkinName,
  type SavedTouchKeyboardSkin,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

const saved: SavedTouchKeyboardSkin = {
  id: "33333333-3333-4333-8333-333333333333",
  name: "原始皮肤",
  design: {
    background: 0xe8f0eb,
    keyBackground: 0xffffff,
    keyForeground: 0x17251d,
    accent: 0x185c47,
    actionBackground: 0x185c47,
    cornerRadius: 8,
    borderWidth: 0,
    shadow: 0,
    pattern: 0,
    monospaced: false,
  },
};

test("updates a saved skin name without dropping its identity or design", () => {
  expect(updateSavedTouchKeyboardSkinName(saved, "新的皮肤")).toEqual({
    ...saved,
    name: "新的皮肤",
  });
});
