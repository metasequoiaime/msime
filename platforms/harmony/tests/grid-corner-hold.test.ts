export {};

import {
  FourteenKey,
  FourteenKeyLayout,
} from "../entry/src/main/ets/keyboard/input/FourteenKeyLayout";
import { EighteenKeyLayout } from "../entry/src/main/ets/keyboard/input/EighteenKeyLayout";
import { KeyIdPolicy } from "../entry/src/main/ets/keyboard/KeyIdPolicy";
import { GridLetterHintTable } from "../entry/src/main/ets/keyboard/input/GridLetterHintTable";
import {
  GridKeyGesturePolicy,
  GridKeyGestureMode,
} from "../entry/src/main/ets/keyboard/input/GridKeyGesturePolicy";
import { EighteenKeySwipePolicy } from "../entry/src/main/ets/keyboard/input/EighteenKeySwipePolicy";
import { SwipeSymbolsDirection } from "../entry/src/main/ets/keyboard/input/SwipeHintPolicy";

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

// 执行真实宿主方法，替换原生会话和 ArkUI 事件；不在测试里重新实现提交或点击吞噬。
const source = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets",
  "utf8",
);
const methods = [
  "holdGridKey",
  "typeGridLetterHint",
  "typeLetterHint",
  "gridKeyTouch",
  "clickGridKey",
  "tapGridKey",
];
const compiled = require("module").stripTypeScriptTypes(
  `class GridView {${methods
    .map((name) => {
      const start = source.indexOf(`  private ${name}(`);
      const end = source.indexOf("\n  private ", start + 1);
      check(start >= 0 && end > start, `没有找到真实方法 ${name}`);
      return source.slice(start, end);
    })
    .join("\n")}\n}`,
);

interface GridView {
  gridGestures: Map<string, GridKeyGesturePolicy>;
  gridTapSwallows: Set<string>;
  holdGridKey(key: FourteenKey, eighteen: boolean): void;
  typeLetterHint(letter: string, hint: string): void;
  gridKeyTouch(key: FourteenKey, eighteen: boolean, event: object): void;
  clickGridKey(key: FourteenKey, eighteen: boolean): void;
}

function fixture(composing: boolean, swipe = true): { view: GridView; calls: string[] } {
  const calls: string[] = [];
  const Constructor = new Function(
    "KeyboardSession",
    "GridLetterHintTable",
    "GridKeyGesturePolicy",
    "GridKeyGestureMode",
    "KeyIdPolicy",
    "EighteenKeySwipePolicy",
    "TouchType",
    `${compiled}; return GridView;`,
  )(
    {
      shared: {
        composing: (): boolean => composing,
        softKeyPress: (id: string): void => {
          calls.push(`key:${id}`);
        },
        commitRaw: (): void => {
          calls.push("raw");
        },
        punctuation: (code: number): void => {
          calls.push(`punctuation:${String.fromCharCode(code)}`);
        },
        insert: (text: string): void => {
          calls.push(`insert:${text}`);
        },
        pressGridKey: (code: number): void => {
          calls.push(`grid:${String.fromCharCode(code)}`);
        },
      },
    },
    GridLetterHintTable,
    GridKeyGesturePolicy,
    GridKeyGestureMode,
    KeyIdPolicy,
    EighteenKeySwipePolicy,
    { Down: "down", Move: "move", Up: "up", Cancel: "cancel" },
  ) as new () => GridView;
  const view = new Constructor();
  Object.assign(view, {
    gridGestures: new Map(),
    gridTapSwallows: new Set(),
    feedback: {
      swipeSymbols: swipe,
      swipeSymbolsDirection: SwipeSymbolsDirection.UP,
      keyPopup: false,
    },
    swipesEighteenKey: (): boolean => true,
    hideKeyPreview: (): void => {},
    showKeyPreview: (): void => {},
    typeEighteenKeyLetter: (_key: FourteenKey, letter: string): void => {
      calls.push(`letter:${letter}`);
    },
  });
  return { view, calls };
}

function touch(
  view: GridView,
  key: FourteenKey,
  eighteen: boolean,
  type: string,
  x = 100,
  y = 100,
  pointers = 1,
): void {
  const finger = { id: 1, windowX: x, windowY: y };
  view.gridKeyTouch(key, eighteen, {
    type,
    changedTouches: [finger],
    touches: type === "up" ? [] : Array.from({ length: pointers }, () => finger),
    target: { area: { globalPosition: { x: 0, y: 0 }, width: 50, height: 50 } },
  });
}

for (const eighteen of [false, true]) {
  for (const key of (eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows()).flat()) {
    for (const composing of [false, true]) {
      for (const swipe of [false, true]) {
        const { view, calls } = fixture(composing, swipe);
        touch(view, key, eighteen, "down");
        view.holdGridKey(key, eighteen);
        view.holdGridKey(key, eighteen);
        touch(view, key, eighteen, "up");
        view.clickGridKey(key, eighteen);
        const hint = GridLetterHintTable.hint(key.letters, eighteen)!;
        const expected = [
          `key:${eighteen ? KeyIdPolicy.eighteenKey(key.letters) : KeyIdPolicy.fourteenKey(key.letters)}`,
          "raw",
          `${GridLetterHintTable.routesThroughEngine(hint) ? "punctuation" : "insert"}:${hint}`,
        ];
        check(
          JSON.stringify(calls) === JSON.stringify(expected),
          `${key.letters} 组字=${composing} 滑动=${swipe}：长按先原样上屏再输入角标一次`,
        );
      }
    }
  }
}

const pair = EighteenKeyLayout.rows()[0].find((key) => key.letters === "we")!;
for (const action of ["cancel", "multi-down", "multi-move", "horizontal", "vertical"]) {
  const { view, calls } = fixture(false);
  touch(view, pair, true, "down", 100, 100, action === "multi-down" ? 2 : 1);
  if (action === "cancel") touch(view, pair, true, "cancel");
  if (action === "multi-move") touch(view, pair, true, "move", 100, 100, 2);
  if (action === "horizontal") touch(view, pair, true, "move", 130, 100);
  if (action === "vertical") touch(view, pair, true, "move", 100, 80);
  view.holdGridKey(pair, true);
  check(calls.length === 0, `${action} 已取消或被滑动接管的按压不再长按输入`);
}

const composingSwipe = fixture(true);
touch(composingSwipe.view, pair, true, "down");
touch(composingSwipe.view, pair, true, "move", 100, 80);
touch(composingSwipe.view, pair, true, "up", 100, 80);
composingSwipe.view.clickGridKey(pair, true);
check(
  composingSwipe.calls.filter((call) => call.startsWith("grid:")).length === 1,
  "组字中纵滑只送一次整组",
);
check(!composingSwipe.calls.includes("raw"), "组字中纵滑不结束组字");

const untouchedQwerty = fixture(true);
untouchedQwerty.view.typeLetterHint("l", "『");
check(
  JSON.stringify(untouchedQwerty.calls) ===
    JSON.stringify([`key:${KeyIdPolicy.character("l")}`, "raw", "insert:『"]),
  "26 键保持既有原样上屏和字面插入路径",
);

const policy = new GridKeyGesturePolicy("we", true);
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.move(1, 1, 100, 80);
check(policy.move(1, 1, 140, 100) === GridKeyGestureMode.SYMBOL, "纵滑先触发后不会切成横滑");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.move(1, 1, 130, 100);
check(policy.move(1, 1, 130, 80) === GridKeyGestureMode.LETTER, "横滑先触发后不会切成符号");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.up(1, 1, 100, 100);
check(!policy.hold(), "松手后的过期长按回调不能输入角标");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.move(1, 1, 100, 80);
check(policy.up(1, 2, 100, 80) === GridKeyGestureMode.CANCELLED, "松手时仍为多指的纵滑不输入符号");
check(!new GridKeyGesturePolicy(null, false).hold(), "没有有效按压或角标不能触发长按");
console.log("14/18 键真实宿主：长按、组字、标点路由、单次统计、点击吞噬和取消均通过");
