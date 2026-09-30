// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  allTouchKeyboardSchemes,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type Preferences,
  useTouchKeyboardSchemeSelection,
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

test("selecting a visible scheme updates the draft through the shared hook", () => {
  const setDraft = vi.fn();
  const { result } = renderHook(() =>
    useTouchKeyboardSchemeSelection({ draft: preferences, setDraft }),
  );

  act(() => result.current.select("wubi"));

  const update = setDraft.mock.calls[0]?.[0];
  const next = typeof update === "function" ? update(preferences) : update;
  expect(next).toEqual(
    expect.objectContaining({
      scheme: "wubi",
      touch_keyboard_schemes: {
        enabled: ["quanpin", "xiaohe", "wubi"],
        selected: "wubi",
      },
    }),
  );
});

test("the helper exposes the complete stable scheme order", () => {
  expect(allTouchKeyboardSchemes).toContain("thoughtful_reply");
});

test("applies queued scheme changes to the latest draft", () => {
  let current: Preferences | undefined = preferences;
  const setDraft = vi.fn(
    (
      update:
        | Preferences
        | undefined
        | ((value: Preferences | undefined) => Preferences | undefined),
    ) => {
      current = typeof update === "function" ? update(current) : update;
    },
  );
  const { result } = renderHook(() =>
    useTouchKeyboardSchemeSelection({ draft: preferences, setDraft }),
  );

  act(() => {
    result.current.select("wubi");
    result.current.setEnabled("xiaohe", false);
  });

  expect(current?.scheme).toBe("wubi");
  expect(current?.touch_keyboard_schemes).toEqual({
    enabled: ["quanpin", "wubi"],
    selected: "wubi",
  });
});
