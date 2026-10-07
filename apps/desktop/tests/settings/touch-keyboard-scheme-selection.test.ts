// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  allTouchKeyboardSchemes,
  defaultTouchKeyboardSchemes,
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type Preferences,
  useTouchKeyboardSchemeSelection,
} from "@msime/ui";
import {
  selectTouchKeyboardScheme,
  touchKeyboardSchemeTitle,
} from "../../../../packages/ui/src/settings/touch-keyboard-scheme-helpers";

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
  expect(allTouchKeyboardSchemes).toEqual([
    "quanpin",
    "nine_key",
    "xiaohe",
    "ziranma",
    "microsoft",
    "shoudao",
    "wubi",
    "japanese_nine_key",
    "japanese",
    "handwriting",
    "korean",
    "cantonese",
    "zhuyin",
    "vietnamese",
    "tibetan",
    "stroke",
    "zhuyin_nine_key",
  ]);
  // 高情商回复是键盘工具栏上的工具，不再是输入方案。
  expect(allTouchKeyboardSchemes as string[]).not.toContain("thoughtful_reply");
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

test("selecting Korean remembers the Chinese scheme and uses the 26-key layout", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, scheme: "wubi", touch_keyboard_layout: "nine_key" },
    "korean",
  );

  expect(next.scheme).toBe("korean");
  expect(next.last_chinese_scheme).toBe("wubi");
  expect(next.touch_keyboard_layout).toBe("twenty_six_key");
  expect(next.touch_keyboard_schemes?.selected).toBe("korean");
});

test("switching from Japanese to Korean keeps the remembered Chinese scheme", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, scheme: "japanese", last_chinese_scheme: "shuangpin" },
    "korean",
  );

  expect(next.last_chinese_scheme).toBe("shuangpin");
});

test.each(["cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"] as const)(
  "%s, which has no touch keyboard, maps to the remembered Chinese touch scheme",
  (scheme) => {
    const untouched = { ...preferences, scheme, touch_keyboard_schemes: undefined };
    expect(
      inferredTouchKeyboardScheme({
        ...untouched,
        last_chinese_scheme: "shuangpin",
        shuangpin_profile: "ziranma",
      }),
    ).toBe("ziranma");
    expect(inferredTouchKeyboardScheme({ ...untouched, last_chinese_scheme: "wubi" })).toBe("wubi");
    expect(inferredTouchKeyboardScheme({ ...untouched, last_chinese_scheme: "zhuyin" })).toBe(
      "quanpin",
    );
    expect(inferredTouchKeyboardScheme({ ...untouched, last_chinese_scheme: null })).toBe(
      "quanpin",
    );
  },
);

test("selecting Japanese from Vietnamese keeps the remembered Chinese scheme", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, scheme: "vietnamese", last_chinese_scheme: "cantonese" },
    "japanese",
  );

  expect(next.last_chinese_scheme).toBe("cantonese");
});

test("Cantonese, Zhuyin, Vietnamese, Tibetan and Stroke are appended after Korean and are opt-in", () => {
  expect(allTouchKeyboardSchemes).toHaveLength(17);
  expect(allTouchKeyboardSchemes.slice(10)).toEqual([
    "korean",
    "cantonese",
    "zhuyin",
    "vietnamese",
    "tibetan",
    "stroke",
    "zhuyin_nine_key",
  ]);
  expect(defaultTouchKeyboardSchemes).toEqual(allTouchKeyboardSchemes.slice(0, 11));
  // 没有列表的文档出自默认值改成只有中文之前的版本，按那时的默认列表回退（client-core 的 `LEGACY_DEFAULT_ENABLED`），日语和韩语不会在升级后消失。
  expect(defaultTouchKeyboardSchemes).toEqual(
    expect.arrayContaining(["japanese_nine_key", "japanese", "korean"]),
  );
});

test("Zhuyin nine-key writes the zhuyin scheme on the nine-key layout and is only inferred when enabled", () => {
  const next = selectTouchKeyboardScheme(
    { ...preferences, touch_keyboard_schemes: { enabled: ["quanpin", "zhuyin_nine_key"] } },
    "zhuyin_nine_key",
  );
  expect(next.scheme).toBe("zhuyin");
  expect(next.touch_keyboard_layout).toBe("nine_key");
  expect(next.touch_keyboard_schemes?.selected).toBe("zhuyin_nine_key");
  expect(touchKeyboardSchemeTitle(next)).toBe("注音 9 键");
  // 没打开注音 9 键时，全拼九宫格留下的 nine_key 不会把注音认成九键。
  const dachen = {
    ...preferences,
    scheme: "zhuyin" as const,
    touch_keyboard_layout: "nine_key" as const,
    touch_keyboard_schemes: { enabled: ["quanpin" as const, "zhuyin" as const] },
  };
  expect(inferredTouchKeyboardScheme(dachen)).toBe("zhuyin");
  expect(
    inferredTouchKeyboardScheme({
      ...dachen,
      touch_keyboard_schemes: { enabled: ["quanpin", "zhuyin", "zhuyin_nine_key"] },
    }),
  ).toBe("zhuyin_nine_key");
});

test("a document without a stored list does not show the opt-in schemes", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, touch_keyboard_schemes: undefined },
    "wubi",
  );
  expect(next.touch_keyboard_schemes?.enabled).toEqual(defaultTouchKeyboardSchemes);
  expect(
    updateTouchKeyboardSchemeEnabled(
      { ...preferences, touch_keyboard_schemes: undefined },
      "zhuyin",
      true,
    )?.touch_keyboard_schemes?.enabled,
  ).toEqual([...defaultTouchKeyboardSchemes, "zhuyin"]);
});

test.each(["cantonese", "zhuyin", "stroke"] as const)(
  "selecting %s selects and remembers it as the Chinese scheme on the 26-key layout",
  (scheme) => {
    const next = selectHomeTouchKeyboardScheme(
      { ...preferences, scheme: "wubi", touch_keyboard_layout: "nine_key" },
      scheme,
    );

    expect(next.scheme).toBe(scheme);
    expect(next.last_chinese_scheme).toBe(scheme);
    expect(next.touch_keyboard_layout).toBe("twenty_six_key");
    expect(next.touch_keyboard_schemes).toEqual({
      enabled: ["quanpin", "xiaohe", "wubi", scheme],
      selected: scheme,
    });
  },
);

test("selecting Vietnamese remembers the Chinese scheme and uses the 26-key layout", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, scheme: "zhuyin", touch_keyboard_layout: "nine_key" },
    "vietnamese",
  );

  expect(next.scheme).toBe("vietnamese");
  expect(next.last_chinese_scheme).toBe("zhuyin");
  expect(next.touch_keyboard_layout).toBe("twenty_six_key");
  expect(next.touch_keyboard_schemes?.selected).toBe("vietnamese");
});

test("selecting Tibetan from Vietnamese keeps the remembered Chinese scheme on the 26-key layout", () => {
  const next = selectHomeTouchKeyboardScheme(
    { ...preferences, scheme: "vietnamese", last_chinese_scheme: "wubi" },
    "tibetan",
  );

  expect(next.scheme).toBe("tibetan");
  expect(next.last_chinese_scheme).toBe("wubi");
  expect(next.touch_keyboard_layout).toBe("twenty_six_key");
  expect(next.touch_keyboard_schemes?.selected).toBe("tibetan");
  expect(touchKeyboardSchemeTitle(next)).toBe("藏文 26 键");
});

test.each(["cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"] as const)(
  "%s infers its own touch scheme once that is enabled",
  (scheme) => {
    expect(
      inferredTouchKeyboardScheme({
        ...preferences,
        scheme,
        last_chinese_scheme: "wubi",
        touch_keyboard_layout: "nine_key",
        touch_keyboard_schemes: { enabled: ["quanpin", "wubi", scheme] },
      }),
    ).toBe(scheme);
  },
);

test("the single Wubi touch scheme is titled by the Wubi profile and keeps it when selected", () => {
  expect(
    touchKeyboardSchemeTitle({ ...preferences, scheme: "wubi", touch_keyboard_schemes: undefined }),
  ).toBe("86 五笔");
  const wubi98: Preferences = { ...preferences, wubi_profile: "wubi98" };
  const next = selectHomeTouchKeyboardScheme(wubi98, "wubi");
  expect(next.wubi_profile).toBe("wubi98");
  expect(touchKeyboardSchemeTitle(next)).toBe("98 五笔");
  expect(
    touchKeyboardSchemeTitle({ ...wubi98, scheme: "wubi", touch_keyboard_schemes: undefined }),
  ).toBe("98 五笔");
});

test("handwriting writes the edition's default scheme, so the wubi edition keeps it", () => {
  const wubi: Preferences = {
    ...preferences,
    scheme: "wubi",
    last_chinese_scheme: "wubi",
    touch_keyboard_schemes: { enabled: ["wubi", "handwriting"], selected: "wubi" },
  };
  const next = selectHomeTouchKeyboardScheme(wubi, "handwriting", "wubi");

  expect(next.scheme).toBe("wubi");
  expect(next.last_chinese_scheme).toBe("wubi");
  expect(next.touch_keyboard_layout).toBe("handwriting");
  expect(inferredTouchKeyboardScheme({ ...next, touch_keyboard_schemes: undefined }, "wubi")).toBe(
    "handwriting",
  );
  // full 的手写照旧写全拼，五笔加手写布局在 full 里不是手写。
  expect(selectHomeTouchKeyboardScheme(preferences, "handwriting").scheme).toBe("quanpin");
  expect(inferredTouchKeyboardScheme({ ...next, touch_keyboard_schemes: undefined })).toBe("wubi");
});

test("the selection hook writes the edition's handwriting scheme", () => {
  let draft: Preferences | undefined = {
    ...preferences,
    scheme: "wubi",
    touch_keyboard_schemes: { enabled: ["wubi", "handwriting"], selected: "wubi" },
  };
  const setDraft = vi.fn((update) => {
    draft = typeof update === "function" ? update(draft) : update;
  });
  const { result } = renderHook(() =>
    useTouchKeyboardSchemeSelection({ draft, setDraft, handwritingScheme: "wubi" }),
  );

  act(() => result.current.select("handwriting"));

  expect(draft?.scheme).toBe("wubi");
  expect(draft?.touch_keyboard_layout).toBe("handwriting");
});

test("the Stroke keypad keeps its own title under either layout", () => {
  for (const layout of ["twenty_six_key", "nine_key"] as const) {
    const stroke: Preferences = {
      ...preferences,
      scheme: "stroke",
      touch_keyboard_layout: layout,
      touch_keyboard_schemes: { enabled: ["quanpin", "stroke"] },
    };
    expect(inferredTouchKeyboardScheme(stroke)).toBe("stroke");
    expect(touchKeyboardSchemeTitle({ ...stroke, touch_keyboard_schemes: undefined })).toBe("笔画");
    expect(
      touchKeyboardSchemeTitle({
        ...stroke,
        touch_keyboard_schemes: { enabled: [], selected: "stroke" },
      }),
    ).toBe("笔画");
  }
});
