import { expect, test } from "vitest";
import {
  allTouchKeyboardSchemes,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type Preferences,
} from "@msime/ui";

const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
  touch_keyboard_schemes: {
    enabled: ["quanpin", "xiaohe", "wubi"],
    selected: "xiaohe",
  },
};

test("disabling the selected scheme selects the first remaining scheme", () => {
  const next = updateTouchKeyboardSchemeEnabled(preferences, "xiaohe", false);

  expect(next?.touch_keyboard_schemes).toEqual({
    enabled: ["quanpin", "wubi"],
    selected: "quanpin",
  });
  expect(next?.scheme).toBe("quanpin");
});

test("refuses to disable the final visible scheme", () => {
  const only: Preferences = {
    ...preferences,
    touch_keyboard_schemes: { enabled: ["quanpin"] },
  };

  expect(updateTouchKeyboardSchemeEnabled(only, "quanpin", false)).toBeNull();
});

test("selecting a home scheme enables it and updates the selected value", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, touch_keyboard_schemes: { enabled: ["quanpin"] } },
    "wubi",
  );

  expect(next.touch_keyboard_schemes).toEqual({
    enabled: ["quanpin", "wubi"],
    selected: "wubi",
  });
  expect(next.scheme).toBe("wubi");
});

test("the helper exposes the complete stable scheme order", () => {
  expect(allTouchKeyboardSchemes).toContain("thoughtful_reply");
});
