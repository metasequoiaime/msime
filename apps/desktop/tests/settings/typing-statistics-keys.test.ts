import { expect, test } from "vitest";
import {
  keyboardHeatmapLayout,
  keyboardHeatmapModel,
  keyHeatLevel,
  keyLabel,
  scopedKeyCounts,
} from "@msime/ui";
import {
  phoneHeatmapCount,
  phoneHeatmapLayoutRows,
  phoneHeatmapLeader,
  phoneHeatmapOthers,
  phoneHeatmapPeak,
  phoneHeatmapRows,
  phoneHeatmapSharedCodes,
  type PhoneHeatmapLayout,
} from "../../../../packages/ui/src/settings/keyboard-heatmap";
import {
  touchKeyLayoutStyle,
  type TouchKeyLayoutStyle,
} from "../../../../packages/ui/src/keyboard/touch-key-layout-styles";
import {
  DesignRowSlot,
  StyledFace,
  TouchLayoutStylePolicy,
} from "../../../../platforms/harmony/entry/src/main/ets/keyboard/TouchLayoutStylePolicy";
// 直接读 HarmonyOS 键盘自己的布局表和键位 id，鸿蒙键盘改了九键网格或计数 id 时这里跟着变红，热力图不会再悄悄过时。
import { NineKeyLayout } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/input/NineKeyLayout";
import {
  KEY_IDS,
  KeyIdPolicy,
} from "../../../../platforms/harmony/entry/src/main/ets/keyboard/KeyIdPolicy";

const dailyKeys = {
  "2026-09-29": { KeyA: 2, Space: 1 },
  "2026-09-30": { KeyA: 3, Enter: 4 },
  "2026-10-01": { KeyB: 5 },
};

test("scoped key counts sum the days in scope, and every day when the scope is cumulative", () => {
  expect(scopedKeyCounts(dailyKeys, ["2026-09-30", "2026-10-01"])).toEqual({
    KeyA: 3,
    Enter: 4,
    KeyB: 5,
  });
  expect(scopedKeyCounts(dailyKeys, null)).toEqual({ KeyA: 5, Space: 1, Enter: 4, KeyB: 5 });
  // A day the scope names but the document lacks adds nothing.
  expect(scopedKeyCounts(dailyKeys, ["2026-01-01"])).toEqual({});
});

test("documents written before keys were counted have no key counts", () => {
  expect(scopedKeyCounts(undefined, null)).toEqual({});
  expect(scopedKeyCounts(undefined, ["2026-10-01"])).toEqual({});
  const model = keyboardHeatmapModel({}, false);
  expect(model.total).toBe(0);
  expect(model.top).toEqual([]);
  expect(model.others).toEqual([]);
  expect(model.maximum).toBe(1);
});

test("zero and non-finite counts are not counted", () => {
  expect(
    scopedKeyCounts({ "2026-10-01": { KeyA: 0, KeyB: -1, KeyC: Number.NaN, KeyD: 1 } }, null),
  ).toEqual({
    KeyD: 1,
  });
});

test("the touch layout is chosen on a phone or when a soft-keyboard-only key was counted", () => {
  expect(keyboardHeatmapLayout({ KeyA: 1 }, false)).toBe("ansi");
  expect(keyboardHeatmapLayout({}, false)).toBe("ansi");
  expect(keyboardHeatmapLayout({}, true)).toBe("touch");
  expect(keyboardHeatmapLayout({ KeyA: 1, Nine3: 1 }, false)).toBe("touch");
  expect(keyboardHeatmapLayout({ SoftGlobe: 1 }, false)).toBe("touch");
  expect(keyboardHeatmapLayout({ FourteenQW: 1 }, false)).toBe("touch");
});

test("the ANSI board places physical keys and lists the rest under 其他键", () => {
  const model = keyboardHeatmapModel(
    { KeyQ: 7, Backquote: 1, F5: 2, ContextMenu: 1, ArrowUp: 4, Numpad1: 1, ShiftRight: 3 },
    false,
    "windows",
  );
  expect(model.layout).toBe("ansi");
  expect(model.nineRows).toBeNull();
  const drawn = model.rows.flat().filter((key) => key.code);
  const find = (code: string) => drawn.find((key) => key.code === code);
  expect(find("KeyQ")).toMatchObject({ label: "Q", name: "Q", count: 7 });
  expect(find("Backquote")?.count).toBe(1);
  expect(find("F5")?.count).toBe(2);
  expect(find("ShiftRight")).toMatchObject({ label: "Shift", name: "右 Shift", count: 3 });
  expect(find("MetaLeft")).toMatchObject({ label: "Win", count: 0 });
  // Every whitelisted key on the board appears once.
  const codes = drawn.map((key) => key.code);
  expect(new Set(codes).size).toBe(codes.length);
  expect(codes).toHaveLength(13 + 61);
  expect(model.others.map((key) => key.code)).toEqual(["ArrowUp", "Numpad1"]);
  expect(model.others[0]).toEqual({ code: "ArrowUp", label: "上箭头", count: 4 });
  expect(model.total).toBe(19);
  expect(model.maximum).toBe(7);
});

test("the touch layout draws the 26-key keyboard and adds the nine-key grid only when a cell was pressed", () => {
  const plain = keyboardHeatmapModel({ KeyA: 2, SoftSymbol: 1, Comma: 3, Digit1: 1 }, true, "ios");
  expect(plain.layout).toBe("touch");
  expect(plain.nineRows).toBeNull();
  const drawn = plain.rows.flat().filter((key) => key.code);
  expect(drawn).toHaveLength(26 + 2 + 5);
  expect(drawn.find((key) => key.code === "SoftSymbol")).toMatchObject({ label: "符号", count: 1 });
  expect(drawn.find((key) => key.code === "Comma")?.count).toBe(3);
  expect(plain.others).toEqual([{ code: "Digit1", label: "1", count: 1 }]);

  const nine = keyboardHeatmapModel({ Nine2: 4, Nine1: 1, SoftPunctuation: 2 }, true);
  expect(nine.nineRows).not.toBeNull();
  const cells = nine.nineRows?.flat().filter((key) => key.code) ?? [];
  expect(cells.map((key) => key.code)).toEqual([
    "Nine1",
    "Nine2",
    "Nine3",
    "Nine4",
    "Nine5",
    "Nine6",
    "Nine7",
    "Nine8",
    "Nine9",
    "Nine0",
  ]);
  expect(cells[1]).toMatchObject({ name: "九宫格 2", count: 4 });
  expect(nine.others.map((key) => key.code)).toEqual(["SoftPunctuation"]);
});

test("the top five are the most pressed keys, ties broken by id", () => {
  const model = keyboardHeatmapModel(
    { KeyA: 5, KeyB: 9, KeyC: 5, Space: 20, ArrowUp: 1, Enter: 5 },
    false,
  );
  expect(model.top.map((key) => [key.code, key.count])).toEqual([
    ["Space", 20],
    ["KeyB", 9],
    ["Enter", 5],
    ["KeyA", 5],
    ["KeyC", 5],
  ]);
});

test("key names follow the host platform's modifier names", () => {
  expect(keyLabel("KeyA")).toBe("A");
  expect(keyLabel("Digit7")).toBe("7");
  expect(keyLabel("Space")).toBe("空格");
  expect(keyLabel("F11")).toBe("F11");
  expect(keyLabel("Numpad4")).toBe("小键盘 4");
  expect(keyLabel("MetaLeft", "macos")).toBe("左 Command");
  expect(keyLabel("AltRight", "macos")).toBe("右 Option");
  expect(keyLabel("MetaRight", "linux")).toBe("右 Super");
  expect(keyLabel("MetaLeft", "windows")).toBe("左 Win");
  expect(keyLabel("ControlLeft")).toBe("左 Ctrl");
  expect(keyLabel("Nine1")).toBe("九宫格 1（标点）");
  expect(keyLabel("Nine0")).toBe("九宫格 0");
  expect(keyLabel("FourteenQW")).toBe("14 键 QW");
  expect(keyLabel("FourteenL")).toBe("14 键 L");
  expect(keyLabel("EighteenWE")).toBe("18 键 WE");
  expect(keyLabel("EighteenQ")).toBe("18 键 Q");
  expect(keyLabel("SoftVoice")).toBe("语音");
});

test("shade levels match the calendar heatmap's scale", () => {
  expect(keyHeatLevel(0, 100)).toBe(0);
  expect(keyHeatLevel(1, 100)).toBe(1);
  expect(keyHeatLevel(25, 100)).toBe(1);
  expect(keyHeatLevel(26, 100)).toBe(2);
  expect(keyHeatLevel(75, 100)).toBe(3);
  expect(keyHeatLevel(100, 100)).toBe(4);
});

// ---- HarmonyOS 手机统计概览的按键热力图 ----

const phoneRow = (layout: PhoneHeatmapLayout, index: number) =>
  phoneHeatmapRows[layout][index].map((key) => [key.label, key.code ?? null, key.weight]);

test("the phone's 26-key heatmap is the HarmonyOS touch keyboard in Chinese 全拼", () => {
  // `KeyboardView.ets`：中文模式字母键面大写，第二行两端各缩进半键，全拼的 Shift 位是「分词」（按下记 `'` 的键），底行是 `designActionRow`。
  expect(phoneRow("full", 0)).toEqual(
    [..."QWERTYUIOP"].map((letter) => [letter, `Key${letter}`, 1]),
  );
  expect(phoneRow("full", 1)).toEqual([
    ["", null, 0.5],
    ...[..."ASDFGHJKL"].map((letter) => [letter, `Key${letter}`, 1]),
    ["", null, 0.5],
  ]);
  expect(phoneRow("full", 2)).toEqual([
    ["分词", "Quote", 1.4],
    ...[..."ZXCVBNM"].map((letter) => [letter, `Key${letter}`, 1]),
    ["⌫", "Backspace", 1.4],
  ]);
  expect(phoneRow("full", 3)).toEqual([
    ["123", "SoftLayer", 1.25],
    ["符", "SoftSymbol", 1.05],
    ["空格", "Space", 4],
    ["，", "Comma", 1],
    ["中", "SoftLanguage", 1.05],
    ["↵", "Enter", 1.9],
  ]);
  expect(phoneHeatmapRows.full).toHaveLength(4);
  // 分词格只算 `Quote`；英文模式下同一键位的 ⇧ 记 `ShiftLeft`，不并进这一格。
  const separator = phoneHeatmapRows.full[2][0];
  expect(separator.also).toBeUndefined();
  expect(phoneHeatmapCount({ Quote: 3, ShiftLeft: 2 }, separator)).toBe(3);
});

test("the phone's 9-key heatmap is the HarmonyOS touch grid and its action row", () => {
  // 网格与 `NineKeyLayout.touchRows()` 同序，每格记 `KeyIdPolicy.nineKey(input)`；两侧列宽 1.15、每格 2（`NINE_KEY_SIDE_SHARE`、`NINE_KEY_GRID_SHARE` / 3）。
  const grid = NineKeyLayout.touchRows();
  const side = NineKeyLayout.sideMarks();
  const right = [
    ["⌫", "Backspace"],
    ["分词", KeyIdPolicy.character(NineKeyLayout.separator().input)],
    ["！", "SoftPunctuation"],
  ];
  const fullWidth: Record<string, string> = { ",": "，", ".": "。", "?": "？" };
  grid.forEach((cells, index) => {
    expect(phoneRow("nine", index)).toEqual([
      [fullWidth[side[index]], "SoftPunctuation", 1.15],
      ...cells.map((cell) => [cell.label, KeyIdPolicy.nineKey(cell.input), 2]),
      [...right[index], 1.15],
    ]);
  });
  expect(NineKeyLayout.closingMark()).toBe("!");
  const faces: Record<string, [string, string]> = {
    symbol_panel: ["符", "SoftSymbol"],
    layer_toggle: ["123", "SoftLayer"],
    space: ["空格", "Space"],
    language: ["中", "SoftLanguage"],
    return: ["↵", "Enter"],
  };
  expect(phoneRow("nine", 3)).toEqual(
    NineKeyLayout.actionRow(false).map((action) => [...faces[action.kind], action.weight]),
  );
  expect(phoneHeatmapRows.nine).toHaveLength(4);
  // 九键已经没有 0 键，字母层也没有它。
  expect(phoneHeatmapRows.nine.flat().some((key) => key.code === "Nine0")).toBe(false);
});

test("every key the phone heatmap draws is an id the HarmonyOS keyboard records", () => {
  for (const layout of ["full", "nine"] as const)
    for (const key of phoneHeatmapRows[layout].flat())
      for (const code of [key.code ?? [], key.also ?? []].flat()) expect(KEY_IDS).toContain(code);
});

test("keys pressed but not drawn on the shown phone keyboard are listed as other keys", () => {
  const counts = { KeyQ: 4, Quote: 2, Digit1: 3, SoftEmoji: 1, Nine2: 5, ShiftLeft: 1 };
  expect(phoneHeatmapOthers(counts, "full").map((key) => [key.code, key.count])).toEqual([
    ["Nine2", 5],
    ["Digit1", 3],
    ["ShiftLeft", 1],
    ["SoftEmoji", 1],
  ]);
  expect(phoneHeatmapOthers(counts, "nine").map((key) => key.code)).toEqual([
    "KeyQ",
    "Digit1",
    "ShiftLeft",
    "SoftEmoji",
  ]);
  expect(phoneHeatmapSharedCodes("nine")).toEqual(new Set(["SoftPunctuation"]));
  expect(phoneHeatmapSharedCodes("full").size).toBe(0);
});

// ---- 按键布局样式与数字行：热力图照用户当前的键盘画 ----

const styled = (style: Partial<TouchKeyLayoutStyle>): TouchKeyLayoutStyle => ({
  twentySixKeyStyle: "standard",
  nineKeyStyle: "standard",
  numberRow: false,
  twentySixSymbolFirst: false,
  languageKeyLeft: false,
  ...style,
});

const slotIds: Record<string, string> = {
  [DesignRowSlot.LAYER]: "SoftLayer",
  [DesignRowSlot.SYMBOL_PANEL]: "SoftSymbol",
  [DesignRowSlot.EMOJI]: "SoftEmoji",
  [DesignRowSlot.COMMA]: "Comma",
  [DesignRowSlot.PERIOD]: "Period",
  [DesignRowSlot.SPACE]: "Space",
  [DesignRowSlot.LANGUAGE]: "SoftLanguage",
  [DesignRowSlot.RETURN]: "Enter",
};

const round = (value: number) => Math.round(value * 100) / 100;

test("the default style and no style draw exactly the heatmap the keyboard had before styles", () => {
  for (const layout of ["full", "nine"] as const) {
    expect(phoneHeatmapLayoutRows(layout)).toBe(phoneHeatmapRows[layout]);
    expect(phoneHeatmapLayoutRows(layout, styled({}))).toEqual(phoneHeatmapRows[layout]);
  }
  // 文档里没有这几个键或存着本页不认识的样式时按标准画。
  expect(touchKeyLayoutStyle({} as never)).toEqual(styled({}));
  expect(
    touchKeyLayoutStyle({
      touch_twenty_six_key_style: "toString",
      touch_nine_key_style: "split",
    } as never),
  ).toEqual(styled({}));
});

test("each 26-key style's heatmap bottom row is the HarmonyOS keyboard's row for that style", () => {
  for (const style of ["standard", "comma_period", "long_space", "emoji"] as const) {
    const rows = phoneHeatmapLayoutRows("full", styled({ twentySixKeyStyle: style }));
    expect(rows.slice(0, 3)).toEqual(phoneHeatmapRows.full.slice(0, 3));
    const keyboard = TouchLayoutStylePolicy.designRow(true, false, StyledFace.TWENTY_SIX, style);
    expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual(
      keyboard.map((entry) => [slotIds[entry.slot], round(entry.share)]),
    );
  }
  expect(
    phoneHeatmapLayoutRows("full", styled({ twentySixKeyStyle: "comma_period" }))[3].map(
      (key) => key.label,
    ),
  ).toEqual(["123", "符", "，", "空格", "。", "中", "↵"]);
  expect(
    phoneHeatmapLayoutRows("full", styled({ twentySixKeyStyle: "emoji" }))[3].map(
      (key) => key.label,
    ),
  ).toEqual(["123", "😀", "空格", "，", "中", "↵"]);
});

test("with 「符」与「123」互换 the 26-key heatmap bottom row is the HarmonyOS keyboard's swapped row (#10056)", () => {
  for (const style of ["standard", "comma_period", "long_space", "emoji"] as const) {
    const rows = phoneHeatmapLayoutRows(
      "full",
      styled({ twentySixKeyStyle: style, twentySixSymbolFirst: true }),
    );
    const keyboard = TouchLayoutStylePolicy.symbolFirst(
      TouchLayoutStylePolicy.designRow(true, false, StyledFace.TWENTY_SIX, style),
      true,
    );
    expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual(
      keyboard.map((entry) => [slotIds[entry.slot], round(entry.share)]),
    );
  }
  expect(
    phoneHeatmapLayoutRows("full", styled({ twentySixSymbolFirst: true }))[3].map(
      (key) => key.label,
    ),
  ).toEqual(["符", "123", "空格", "，", "中", "↵"]);
  expect(touchKeyLayoutStyle({ touch_twenty_six_symbol_first: true } as never)).toEqual(
    styled({ twentySixSymbolFirst: true }),
  );
  // 九键底行不受它影响。
  expect(phoneHeatmapLayoutRows("nine", styled({ twentySixSymbolFirst: true }))).toEqual(
    phoneHeatmapRows.nine,
  );
});

test("with 中/英键放在空格左边 both heatmaps draw the HarmonyOS keyboard's rows with 中/英 left of the space (#10228)", () => {
  for (const style of ["standard", "comma_period", "long_space", "emoji"] as const)
    for (const symbolFirst of [false, true]) {
      const rows = phoneHeatmapLayoutRows(
        "full",
        styled({
          twentySixKeyStyle: style,
          twentySixSymbolFirst: symbolFirst,
          languageKeyLeft: true,
        }),
      );
      const keyboard = TouchLayoutStylePolicy.languageBeforeSpace(
        TouchLayoutStylePolicy.symbolFirst(
          TouchLayoutStylePolicy.designRow(true, false, StyledFace.TWENTY_SIX, style),
          symbolFirst,
        ),
        true,
      );
      expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual(
        keyboard.map((entry) => [slotIds[entry.slot], round(entry.share)]),
      );
    }
  expect(
    phoneHeatmapLayoutRows("full", styled({ languageKeyLeft: true }))[3].map((key) => key.label),
  ).toEqual(["123", "符", "中", "空格", "，", "↵"]);
  expect(
    phoneHeatmapLayoutRows(
      "full",
      styled({ twentySixKeyStyle: "comma_period", languageKeyLeft: true }),
    )[3].map((key) => key.label),
  ).toEqual(["123", "符", "中", "，", "空格", "。", "↵"]);
  const faces: Record<string, string> = {
    symbol_panel: "SoftSymbol",
    layer_toggle: "SoftLayer",
    comma: "Comma",
    period: "Period",
    emoji: "SoftEmoji",
    space: "Space",
    language: "SoftLanguage",
    return: "Enter",
  };
  for (const style of ["standard", "comma_period", "emoji"] as const) {
    const rows = phoneHeatmapLayoutRows(
      "nine",
      styled({ nineKeyStyle: style, languageKeyLeft: true }),
    );
    expect(rows.slice(0, 3)).toEqual(phoneHeatmapRows.nine.slice(0, 3));
    expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual(
      NineKeyLayout.actionRow(false, false, style, false, "before_zero", true).map((action) => [
        faces[action.kind],
        round(action.weight),
      ]),
    );
  }
  // 右侧大回车：底行 符 | 123 | 中 | 空格，回车仍从上一行跨下来；每个键的统计 id 和份额不变。
  const tall = phoneHeatmapLayoutRows(
    "nine",
    styled({ nineKeyStyle: "tall_return", languageKeyLeft: true }),
  );
  const right = phoneHeatmapLayoutRows("nine", styled({ nineKeyStyle: "tall_return" }));
  expect(tall.slice(0, 3)).toEqual(right.slice(0, 3));
  expect(tall[3].map((key) => key.code)).toEqual([
    "SoftSymbol",
    "SoftLayer",
    "SoftLanguage",
    "Space",
    undefined,
  ]);
  for (const key of right[3]) expect(tall[3]).toContainEqual(key);
  // 文档里写 left 才算左。
  expect(touchKeyLayoutStyle({ touch_language_key_side: "left" } as never)).toEqual(
    styled({ languageKeyLeft: true }),
  );
  for (const value of ["right", "middle", undefined])
    expect(touchKeyLayoutStyle({ touch_language_key_side: value } as never)).toEqual(styled({}));
});

test("each 9-key style's heatmap bottom row is the HarmonyOS nine-key frame's row", () => {
  const faces: Record<string, string> = {
    symbol_panel: "SoftSymbol",
    layer_toggle: "SoftLayer",
    comma: "Comma",
    period: "Period",
    emoji: "SoftEmoji",
    space: "Space",
    language: "SoftLanguage",
    return: "Enter",
  };
  for (const style of ["standard", "comma_period", "emoji"] as const) {
    const rows = phoneHeatmapLayoutRows("nine", styled({ nineKeyStyle: style }));
    expect(rows.slice(0, 3)).toEqual(phoneHeatmapRows.nine.slice(0, 3));
    expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual(
      NineKeyLayout.actionRow(false, false, style).map((action) => [
        faces[action.kind],
        round(action.weight),
      ]),
    );
  }
});

test("the 9-key tall return heatmap draws the HarmonyOS frame: return spans two rows, 符 under the sidebar", () => {
  const rows = phoneHeatmapLayoutRows("nine", styled({ nineKeyStyle: "tall_return" }));
  expect(rows).toHaveLength(4);
  // 网格前两行（含右列 ⌫ 和 分词）不变。
  expect(rows.slice(0, 2)).toEqual(phoneHeatmapRows.nine.slice(0, 2));
  // 第三行：左列「？！」仍记 SoftPunctuation，网格 PQRS TUV WXYZ，右列是跨两行的回车；！ 不再单独画在右列。
  expect(rows[2].map((key) => [key.label, key.code, key.weight, key.rowSpan])).toEqual([
    ["？！", "SoftPunctuation", NineKeyLayout.SIDE_SHARE, undefined],
    ["PQRS", "Nine7", 2, undefined],
    ["TUV", "Nine8", 2, undefined],
    ["WXYZ", "Nine9", 2, undefined],
    ["↵", "Enter", NineKeyLayout.SIDE_SHARE, 2],
  ]);
  // 底行：符 与左列同宽，123 | 空格 | 中 恰好是网格的宽度（键盘的 NineKeyLayout.actionRow），回车下伸的位置是占位。
  const faces: Record<string, string> = {
    layer_toggle: "SoftLayer",
    space: "Space",
    language: "SoftLanguage",
  };
  expect(rows[3].map((key) => [key.code, round(key.weight)])).toEqual([
    ["SoftSymbol", NineKeyLayout.SIDE_SHARE],
    ...NineKeyLayout.actionRow(false, false, "tall_return").map((action) => [
      faces[action.kind],
      round(action.weight),
    ]),
    [undefined, NineKeyLayout.SIDE_SHARE],
  ]);
  // 每一行的份额加起来相同，各列上下对齐。
  const widths = rows.map((row) => round(row.reduce((sum, key) => sum + key.weight, 0)));
  expect(new Set(widths)).toEqual(
    new Set([round(NineKeyLayout.SIDE_SHARE * 2 + NineKeyLayout.GRID_SHARE)]),
  );
  // 统计 id 不变：回车、符、侧栏标点照常有计数，侧栏标点仍是几个键帽共用。
  const counts = { Enter: 7, SoftSymbol: 2, SoftPunctuation: 3, Nine7: 9 };
  expect(phoneHeatmapOthers(counts, "nine", styled({ nineKeyStyle: "tall_return" }))).toEqual([]);
  expect(phoneHeatmapPeak(counts, "nine", styled({ nineKeyStyle: "tall_return" }))).toBe(9);
  expect(phoneHeatmapSharedCodes("nine", styled({ nineKeyStyle: "tall_return" }))).toEqual(
    new Set(["SoftPunctuation"]),
  );
  expect(phoneHeatmapLeader(counts, "nine", styled({ nineKeyStyle: "tall_return" }))?.label).toBe(
    "PQRS",
  );
  // 26 键不受九键样式影响。
  expect(phoneHeatmapLayoutRows("full", styled({ nineKeyStyle: "tall_return" }))).toEqual(
    phoneHeatmapRows.full,
  );
});

test("the number row is drawn over the 26-key heatmap only and counts Digit1 to Digit0", () => {
  const full = phoneHeatmapLayoutRows("full", styled({ numberRow: true }));
  expect(full).toHaveLength(5);
  expect(full[0].map((key) => [key.label, key.code, key.weight])).toEqual(
    [..."1234567890"].map((digit) => [digit, `Digit${digit}`, 1]),
  );
  expect(full.slice(1)).toEqual(phoneHeatmapRows.full);
  expect(phoneHeatmapLayoutRows("nine", styled({ numberRow: true }))).toEqual(
    phoneHeatmapRows.nine,
  );
  const counts = { Digit1: 9, KeyQ: 4, SoftEmoji: 2, Period: 1 };
  // 画着数字行时数字不再列进「其他键」，计数落在数字键帽上；没画时照旧是其他键。
  expect(
    phoneHeatmapOthers(counts, "full", styled({ numberRow: true })).map((key) => key.code),
  ).toEqual(["SoftEmoji", "Period"]);
  expect(phoneHeatmapOthers(counts, "full").map((key) => key.code)).toContain("Digit1");
  expect(phoneHeatmapPeak(counts, "full", styled({ numberRow: true }))).toBe(9);
  expect(phoneHeatmapPeak(counts, "full")).toBe(4);
  // 表情键和独立句号画出来时也不再是其他键。
  expect(
    phoneHeatmapOthers(counts, "full", styled({ twentySixKeyStyle: "emoji" })).map(
      (key) => key.code,
    ),
  ).toEqual(["Digit1", "Period"]);
  expect(
    phoneHeatmapOthers(counts, "full", styled({ twentySixKeyStyle: "comma_period" })).map(
      (key) => key.code,
    ),
  ).toEqual(["Digit1", "SoftEmoji"]);
  // 「字母键最常按」只看字母，数字行再忙也不是它。
  expect(phoneHeatmapLeader(counts, "full", styled({ numberRow: true }))?.label).toBe("Q");
  expect(phoneHeatmapSharedCodes("full", styled({ numberRow: true })).size).toBe(0);
});

test("every key a styled phone heatmap draws is an id the HarmonyOS keyboard records", () => {
  for (const twentySixKeyStyle of ["standard", "comma_period", "long_space", "emoji"] as const)
    for (const nineKeyStyle of ["standard", "comma_period", "emoji", "tall_return"] as const)
      for (const layout of ["full", "nine"] as const)
        for (const key of phoneHeatmapLayoutRows(
          layout,
          styled({ twentySixKeyStyle, nineKeyStyle, numberRow: true }),
        ).flat())
          if (key.code) expect(KEY_IDS).toContain(key.code);
});
