// @vitest-environment jsdom
// 鸿蒙手机设置页里「按键布局」子页以外的键盘预览（屏幕键盘页「尺寸」、皮肤卡片）画用户当前的按键布局：底行随 26 / 14/18 / 9 键各自的样式变，打开数字行时加 1 到 0，与子页和键盘同一张表。
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import { harmonyTouchRowsForPreferences } from "../../../../packages/ui/src/settings/touch-key-layout-settings";
import { harmonyTouchKeyboardRows } from "../../../../packages/ui/src/keyboard/keyboard-layouts";
import {
  fourteenKeyBottomRows,
  fourteenKeyStyleOptions,
  nineKeyBottomRows,
  nineKeyStyleOptions,
  twentySixKeyBottomRows,
  twentySixKeyStyleOptions,
  withLanguageKeyLeft,
} from "../../../../packages/ui/src/keyboard/touch-key-layout-styles";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

const fourteenKey: Partial<Preferences> = {
  touch_keyboard_layout: "fourteen_key",
  touch_keyboard_schemes: { enabled: ["fourteen_key"], selected: "fourteen_key" },
};
const nineKey: Partial<Preferences> = {
  touch_keyboard_layout: "nine_key",
  touch_keyboard_schemes: { enabled: ["nine_key"], selected: "nine_key" },
};
const japanese: Partial<Preferences> = {
  scheme: "japanese",
  touch_keyboard_schemes: { enabled: ["japanese"], selected: "japanese" },
};

const labels = (row: readonly { label: string }[]) => row.map((key) => key.label);
const rowsFor = (preferences: Partial<Preferences>) =>
  harmonyTouchRowsForPreferences({ ...base, ...preferences }).map(labels);

test.each(twentySixKeyStyleOptions.map((option) => option.value))(
  "26 键预览的底行是 %s 样式的底行",
  (style) => {
    const rows = rowsFor({ touch_twenty_six_key_style: style });
    expect(rows).toHaveLength(4);
    expect(rows.slice(0, 3)).toEqual(
      harmonyTouchKeyboardRows.twenty_six_key.slice(0, 3).map(labels),
    );
    expect(rows[3]).toEqual(labels(twentySixKeyBottomRows[style]));
  },
);

test.each(fourteenKeyStyleOptions.map((option) => option.value))(
  "14 键预览的底行是 %s 样式的底行",
  (style) => {
    const rows = rowsFor({ ...fourteenKey, touch_fourteen_key_style: style });
    expect(rows[0]).toEqual(["QW", "ER", "TY", "UI", "OP"]);
    expect(rows.at(-1)).toEqual(labels(fourteenKeyBottomRows[style]));
  },
);

test.each(nineKeyStyleOptions.map((option) => option.value))(
  "9 键预览的底行是 %s 样式的底行",
  (style) => {
    const rows = harmonyTouchRowsForPreferences({
      ...base,
      ...nineKey,
      touch_nine_key_style: style,
      touch_number_row: true,
    });
    // 九键不画数字行：九宫格三行加底行。
    expect(rows).toHaveLength(4);
    expect(labels(rows[0])).toEqual(["，", "@#", "ABC", "DEF", "⌫"]);
    const bottom = rows[3].filter((key) => key.label !== "");
    expect(labels(bottom)).toEqual(
      labels(nineKeyBottomRows[style]).filter((label) => style !== "tall_return" || label !== "↵"),
    );
    if (style === "tall_return") {
      // 右侧大回车：回车在第三行右列，往下跨到底行。
      expect(rows[2].at(-1)).toMatchObject({ label: "↵", rowSpan: 2 });
    }
  },
);

test("9 键左列换成用户的符号表时画那张表的一屏，与「按键布局」子页相同", () => {
  const rows = harmonyTouchRowsForPreferences({
    ...base,
    ...nineKey,
    touch_nine_key_sidebar_symbols: ["@", "#", "~", "%"],
  });
  expect(rows).toHaveLength(4);
  expect(rows[0][0]).toMatchObject({ rowSpan: 3, stack: ["@", "#", "~"] });
  expect(rows[1][0].label).toBe("");
  expect(rows[2][0].label).toBe("");
  expect(labels(rows[0]).slice(1)).toEqual(["@#", "ABC", "DEF", "⌫"]);
});

test("「符」与「123」互换和数字行跟着偏好", () => {
  const rows = rowsFor({ touch_twenty_six_symbol_first: true, touch_number_row: true });
  expect(rows[0]).toEqual([..."1234567890"]);
  expect(rows).toHaveLength(5);
  expect(rows.at(-1)).toEqual(["符", "123", "空格", "，", "中", "↵"]);
  // 14/18 键底行没有 符，互换不动它，数字行照样加。
  const fourteen = rowsFor({
    ...fourteenKey,
    touch_twenty_six_symbol_first: true,
    touch_number_row: true,
  });
  expect(fourteen[0]).toEqual([..."1234567890"]);
  expect(fourteen.at(-1)).toEqual(labels(fourteenKeyBottomRows.standard));
});

test("「中/英键放在空格左边」三类预览都跟着排，日语 26 键同样排，九键换了左列的表时底行照样排", () => {
  const left: Partial<Preferences> = { touch_language_key_side: "left" };
  expect(rowsFor(left).at(-1)).toEqual(["123", "符", "中", "空格", "，", "↵"]);
  expect(rowsFor({ ...left, touch_twenty_six_symbol_first: true }).at(-1)).toEqual([
    "符",
    "123",
    "中",
    "空格",
    "，",
    "↵",
  ]);
  expect(rowsFor({ ...left, ...fourteenKey }).at(-1)).toEqual(
    labels(withLanguageKeyLeft(fourteenKeyBottomRows.standard, true)),
  );
  expect(rowsFor({ ...left, ...japanese }).at(-1)).toEqual(
    labels(withLanguageKeyLeft(twentySixKeyBottomRows.standard, true)),
  );
  const nine = harmonyTouchRowsForPreferences({
    ...base,
    ...nineKey,
    ...left,
    touch_nine_key_sidebar_symbols: ["@", "#", "~"],
  });
  expect(nine[0][0]).toMatchObject({ rowSpan: 3, stack: ["@", "#", "~"] });
  expect(labels(nine[nine.length - 1])).toEqual(
    labels(withLanguageKeyLeft(nineKeyBottomRows.standard, true)),
  );
  expect(labels(nine[nine.length - 1])).not.toEqual(labels(nineKeyBottomRows.standard));
  // 缺键、`right` 和不认识的值都按右边。
  expect(rowsFor({ touch_language_key_side: "right" }).at(-1)).toEqual(rowsFor({}).at(-1));
});

test("九键双拼和笔画也画九宫格外框，底行跟着 9 键样式，不画数字行", () => {
  for (const scheme of ["nine_key_shuangpin", "stroke"] as const) {
    const rows = harmonyTouchRowsForPreferences(
      { ...base, touch_nine_key_style: "comma_period", touch_number_row: true },
      scheme,
    ).map(labels);
    expect(rows, scheme).toHaveLength(4);
    expect(rows.at(-1), scheme).toEqual(labels(nineKeyBottomRows.comma_period));
  }
});

test("日语 26 键不按中文 26 键的样式排，没有偏好时画标准 26 键", () => {
  expect(rowsFor({ ...japanese, touch_twenty_six_key_style: "long_space" }).at(-1)).toEqual(
    labels(twentySixKeyBottomRows.standard),
  );
  const rows = harmonyTouchRowsForPreferences(undefined).map(labels);
  expect(rows).toHaveLength(4);
  expect(rows.at(-1)).toEqual(labels(twentySixKeyBottomRows.standard));
});

function snapshot(preferences: Partial<Preferences>): Snapshot {
  return { format_version: 1, revision: 3, preferences: { ...base, ...preferences } } as Snapshot;
}

/** 预览里每一行画出的键（占位不画）。 */
const drawnRows = (svg: Element) =>
  [...svg.querySelectorAll("[data-keyboard-row]")].map((row) =>
    [...row.querySelectorAll("[data-keyboard-key]")].map((key) =>
      key.getAttribute("data-keyboard-key"),
    ),
  );

async function renderPage(page: "screen-keyboard" | "skin", preferences: Partial<Preferences>) {
  render(
    <SettingsPage
      initialPage={page}
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, touch_toolbar_tools: true }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        customTouchKeyboardSkins: true,
        customSkinLibrary: { load: vi.fn().mockResolvedValue([]), mutate: vi.fn() },
      }}
    />,
  );
  await settingsFormReady();
}

test("屏幕键盘页的预览画用户的底行样式和数字行，不画旧的 中/英 底行", async () => {
  await renderPage("screen-keyboard", {
    touch_twenty_six_key_style: "long_space",
    touch_number_row: true,
  });
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" }).querySelector("svg")!;
  const rows = drawnRows(preview);
  expect(rows).toHaveLength(5);
  expect(rows[0]).toEqual([..."1234567890"]);
  expect(rows.at(-1)).toEqual(labels(twentySixKeyBottomRows.long_space));
  expect(rows.flat()).not.toContain("中/英");
});

test("屏幕键盘页的 9 键预览画右侧大回车，回车跨两行", async () => {
  await renderPage("screen-keyboard", { ...nineKey, touch_nine_key_style: "tall_return" });
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" }).querySelector("svg")!;
  const rows = drawnRows(preview);
  expect(rows).toHaveLength(4);
  expect(rows[2].at(-1)).toBe("↵");
  expect(rows[3]).toEqual(["符", "123", "空格", "中"]);
  expect(preview.querySelector("[data-keyboard-key='↵']")?.getAttribute("data-row-span")).toBe("2");
});

test("屏幕键盘页的 9 键预览按用户的左侧符号表画左列，各项等高", async () => {
  await renderPage("screen-keyboard", {
    ...nineKey,
    touch_nine_key_sidebar_symbols: ["、", "……", "；"],
  });
  const preview = screen.getByRole("group", { name: "屏幕键盘预览" }).querySelector("svg")!;
  const rows = drawnRows(preview);
  expect(rows[0].slice(0, 3)).toEqual(["、", "……", "；"]);
  expect(rows[1][0]).toBe("GHI");
  const heights = ["、", "……", "；"].map((label) =>
    preview.querySelector(`[data-keyboard-key='${label}'] text`)!.getAttribute("y"),
  );
  expect(new Set(heights).size).toBe(3);
});

test("皮肤卡片的缩略图跟着 14 键样式，数字行挤在原来的高度里", async () => {
  await renderPage("skin", {
    ...fourteenKey,
    touch_fourteen_key_style: "emoji",
    touch_number_row: true,
  });
  const page = screen.getByRole("group", { name: "皮肤" });
  const thumbnails = [...page.querySelectorAll("[data-skin-frame] svg[viewBox='0 0 390 292']")];
  expect(thumbnails.length).toBeGreaterThan(0);
  for (const thumbnail of thumbnails) {
    const rows = drawnRows(thumbnail);
    expect(rows[0]).toEqual([..."1234567890"]);
    expect(rows.at(-1)).toEqual(labels(fourteenKeyBottomRows.emoji));
    // 五行仍画在设计里四行键占的高度内（工具栏下方到 263），不超出 292 的画布。
    const lastText = within(thumbnail.querySelector("[data-keyboard-row='4']") as HTMLElement)
      .getByText("↵")
      .getAttribute("y");
    expect(Number(lastText)).toBeLessThan(263);
  }
});
