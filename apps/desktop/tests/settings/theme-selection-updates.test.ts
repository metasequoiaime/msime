import { expect, test } from "vitest";
import { updateCandidateColor, updateCustomKeyboard } from "../../../../packages/ui/src";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import type { Preferences } from "../../../../packages/ui/src";

const preferences = (patch: Partial<Preferences> = {}): Preferences =>
  ({
    scheme: "quanpin",
    global_theme: "night",
    custom_theme: {
      base: "night",
      candidate_skin: "community-skin",
      candidate_skin_dark: "community-dark",
      candidate_colors: { text: "#fff" },
    },
    ...patch,
  }) as Preferences;

test("choosing a candidate color selects custom while preserving the active base", () => {
  const next = updateCandidateColor(preferences(), "surface", "#123456");

  expect(next.global_theme).toBe("custom");
  expect(next.custom_theme).toMatchObject({
    base: "night",
    candidate_skin: null,
    candidate_skin_dark: null,
    candidate_colors: { text: "#fff", surface: "#123456" },
  });
});

test("clearing a candidate color does not change the selected theme", () => {
  const next = updateCandidateColor(preferences({ global_theme: "light" }), "text", null);

  expect(next.global_theme).toBe("light");
  expect(next.custom_theme?.candidate_colors).toMatchObject({ text: null });
});

test("choosing a custom keyboard preserves a custom theme base and package state", () => {
  const design = { ...defaultTouchKeyboardSkinDesign, key_radius: 9 };
  const next = updateCustomKeyboard(preferences({ global_theme: "custom" }), design);

  expect(next.global_theme).toBe("custom");
  expect(next.custom_theme).toMatchObject({
    base: "night",
    candidate_skin: "community-skin",
    candidate_skin_dark: "community-dark",
    keyboard: design,
  });
});

test("customizing the native theme draws the custom theme over system", () => {
  const colour = updateCandidateColor(preferences({ global_theme: "native" }), "text", "#123456");
  expect(colour.custom_theme?.base).toBe("system");
  const keyboard = updateCustomKeyboard(
    preferences({ global_theme: "native" }),
    defaultTouchKeyboardSkinDesign,
  );
  expect(keyboard.custom_theme?.base).toBe("system");
});

test("choosing a custom keyboard from another theme clears both skin slots", () => {
  const next = updateCustomKeyboard(preferences(), defaultTouchKeyboardSkinDesign);
  expect(next.custom_theme).toMatchObject({
    base: "night",
    candidate_skin: null,
    candidate_skin_dark: null,
  });
});
