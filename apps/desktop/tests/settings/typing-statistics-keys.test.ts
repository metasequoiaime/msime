import { expect, test } from "vitest";
import {
  keyboardHeatmapLayout,
  keyboardHeatmapModel,
  keyHeatLevel,
  keyLabel,
  scopedKeyCounts,
} from "@msime/ui";

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
