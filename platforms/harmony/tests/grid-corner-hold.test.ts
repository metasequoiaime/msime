export {};

import {
  FourteenKey,
  FourteenKeyLayout,
} from "../entry/src/main/ets/keyboard/input/FourteenKeyLayout";
import { EighteenKeyLayout } from "../entry/src/main/ets/keyboard/input/EighteenKeyLayout";
import { KeyIdPolicy } from "../entry/src/main/ets/keyboard/KeyIdPolicy";
import { GridLetterHintTable } from "../entry/src/main/ets/keyboard/input/GridLetterHintTable";
import {
  GridKeyCommit,
  GridKeyGesturePolicy,
  GridKeyGestureMode,
} from "../entry/src/main/ets/keyboard/input/GridKeyGesturePolicy";
import {
  GridKeyPoint,
  GridKeyTouchController,
} from "../entry/src/main/ets/keyboard/input/GridKeyTouchController";
import { EighteenKeySwipePolicy } from "../entry/src/main/ets/keyboard/input/EighteenKeySwipePolicy";
import { SwipeSymbolsDirection } from "../entry/src/main/ets/keyboard/input/SwipeHintPolicy";

declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };
declare function require(name: "module"): { stripTypeScriptTypes(source: string): string };

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const RECT = { left: 0, top: 0, width: 50, height: 50 };

interface Fixture {
  keys: GridKeyTouchController;
  calls: string[];
  preview: { letter: string };
  /** 宿主此刻是否在组字；测试中途改它，模拟另一根手指开始组字。 */
  state: { composing: boolean };
}

// 真实的 GridKeyTouchController 和 GridKeyGesturePolicy，宿主只记录它被要求做的事。
function fixture(composing: boolean, swipe = true): Fixture {
  const calls: string[] = [];
  const preview = { letter: "" };
  const state = { composing };
  const keys = new GridKeyTouchController({
    composing: (): boolean => state.composing,
    swipeDirection: (): SwipeSymbolsDirection | null => (swipe ? SwipeSymbolsDirection.UP : null),
    exactSwipes: (letters: string): boolean => EighteenKeySwipePolicy.swipeable(letters),
    keyPopup: (): boolean => false,
    keyFace: (key: FourteenKey): string => key.label,
    hintFace: (key: FourteenKey, eighteen: boolean): string | null =>
      GridLetterHintTable.hint(key.letters, eighteen),
    showPreview: (): void => {},
    hidePreview: (): void => {},
    showLetter: (_letters: string, letter: string): void => {
      preview.letter = letter;
    },
    endLetter: (): void => {
      preview.letter = "";
    },
    keyUp: (): void => {},
    tap: (key: FourteenKey, eighteen: boolean): void => {
      calls.push(`key:${eighteen ? KeyIdPolicy.eighteenKey(key.letters) : KeyIdPolicy.fourteenKey(key.letters)}`);
      calls.push(`grid:${key.input}`);
    },
    typeHint: (key: FourteenKey, eighteen: boolean, hint: string): void => {
      calls.push(`key:${eighteen ? KeyIdPolicy.eighteenKey(key.letters) : KeyIdPolicy.fourteenKey(key.letters)}`);
      calls.push(`hint:${hint}`);
    },
    typeLetter: (_key: FourteenKey, eighteen: boolean, letter: string): void => {
      if (letter.length > 0) calls.push(`${eighteen ? "letter" : "fourteen"}:${letter}`);
    },
  });
  return { keys, calls, preview, state };
}

function point(id: number, x = 100, y = 100): GridKeyPoint {
  return { id, x, y };
}

function same(actual: string[], expected: string[], message: string): void {
  check(JSON.stringify(actual) === JSON.stringify(expected), `${message}: ${JSON.stringify(actual)}`);
}

function keyId(key: FourteenKey, eighteen: boolean): string {
  return `key:${eighteen ? KeyIdPolicy.eighteenKey(key.letters) : KeyIdPolicy.fourteenKey(key.letters)}`;
}

// 长按：每个网格键都只输入一次角标，松手和随后的点击都不再输入或计数。
for (const eighteen of [false, true]) {
  for (const key of (eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows()).flat()) {
    for (const composing of [false, true]) {
      for (const swipe of [false, true]) {
        for (const clickFirst of [false, true]) {
          const { keys, calls } = fixture(composing, swipe);
          keys.down(key, eighteen, point(1), RECT);
          keys.hold(key, eighteen);
          keys.hold(key, eighteen);
          if (clickFirst) keys.click(key, eighteen, 100, 100);
          keys.up(key, eighteen, point(1));
          if (!clickFirst) keys.click(key, eighteen, 100, 100);
          const hint = GridLetterHintTable.hint(key.letters, eighteen)!;
          same(
            calls,
            [keyId(key, eighteen), `hint:${hint}`],
            `${key.letters} 组字=${composing} 滑动=${swipe} 点击先到=${clickFirst}：长按只输入角标一次`,
          );
        }
      }
    }
  }
}

const pair = EighteenKeyLayout.rows()[0].find((key) => key.letters === "we")!;
const neighbour = EighteenKeyLayout.rows()[0].find((key) => key.letters === "rt")!;

// 普通点按：无论点击和抬手谁先到，都只送一次整组。
for (const composing of [false, true]) {
  for (const clickFirst of [false, true]) {
    const { keys, calls } = fixture(composing);
    keys.down(pair, true, point(1), RECT);
    if (clickFirst) keys.click(pair, true, 100, 100);
    keys.up(pair, true, point(1));
    if (!clickFirst) keys.click(pair, true, 100, 100);
    same(calls, [keyId(pair, true), "grid:w"], `组字=${composing} 点击先到=${clickFirst}：点按只送一次整组`);
  }
}

// 组字中纵滑仍是点按：抬手先到时由抬手送整组，ArkUI 不送点击也不丢，送了点击也不重复。
for (const clickArrives of [false, true]) {
  const { keys, calls } = fixture(true);
  keys.down(pair, true, point(1), RECT);
  keys.move(pair, true, [point(1, 100, 80)], RECT);
  keys.up(pair, true, point(1, 100, 80));
  if (clickArrives) keys.click(pair, true, 100, 80);
  same(calls, [keyId(pair, true), "grid:w"], `组字中纵滑 点击=${clickArrives}：只送一次整组，不结束组字`);
}

// 空闲时按下、另一根手指在按压中途开始组字：之后越过阈值的纵滑（拖动中或只在抬手那一点）仍是点按，不把刚开始的音节原样上屏再接角标。
for (const thresholdAtLift of [false, true]) {
  for (const clickFirst of [false, true]) {
    const { keys, calls, state } = fixture(false);
    keys.down(pair, true, point(1), RECT);
    state.composing = true;
    if (!thresholdAtLift) keys.move(pair, true, [point(1, 100, 80)], RECT);
    if (clickFirst) keys.click(pair, true, 100, 80);
    keys.up(pair, true, point(1, 100, 80));
    if (!clickFirst) keys.click(pair, true, 100, 80);
    same(
      calls,
      [keyId(pair, true), "grid:w"],
      `按压中途开始组字 抬手才过阈值=${thresholdAtLift} 点击先到=${clickFirst}：纵滑只送一次整组`,
    );
  }
}

// 快速一甩只在抬手那一点越过阈值：点击先到时按点击位置判定为精确字母，抬手不再送整组。
for (const eighteen of [false, true]) {
  const key = (eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows())
    .flat()
    .find((candidate) => candidate.letters.length === 2)!;
  for (const composing of [false, true]) {
    for (const clickFirst of [false, true]) {
      const { keys, calls } = fixture(composing);
      keys.down(key, eighteen, point(1), RECT);
      if (clickFirst) keys.click(key, eighteen, 130, 100);
      keys.up(key, eighteen, point(1, 130, 100));
      if (!clickFirst) keys.click(key, eighteen, 130, 100);
      same(
        calls,
        [`${eighteen ? "letter" : "fourteen"}:${key.letters[1].toUpperCase()}`],
        `${eighteen ? 18 : 14} 键组字=${composing} 点击先到=${clickFirst}：抬手才过阈值的横滑只输入一次精确字母`,
      );
    }
  }
}

// 双拇指交替：另一个键上的手指不影响这个键，两个键各送一次整组。
{
  const { keys, calls } = fixture(false);
  keys.down(pair, true, point(1), RECT);
  keys.down(neighbour, true, point(2, 160, 100), RECT);
  keys.move(pair, true, [point(1, 101, 100), point(2, 161, 100)], RECT);
  keys.up(pair, true, point(1));
  keys.click(pair, true, 100, 100);
  keys.up(neighbour, true, point(2, 160, 100));
  keys.click(neighbour, true, 160, 100);
  same(
    calls,
    [keyId(pair, true), "grid:w", keyId(neighbour, true), "grid:r"],
    "两个拇指按在两个键上各自输入",
  );
}

// 同一键上落下第二根手指：整次按压作废，两根手指抬起和点击都不输入。
{
  const { keys, calls } = fixture(false);
  keys.down(pair, true, point(1), RECT);
  keys.down(pair, true, point(2, 110, 100), RECT);
  keys.hold(pair, true);
  keys.up(pair, true, point(2, 110, 100));
  keys.up(pair, true, point(1));
  keys.click(pair, true, 100, 100);
  same(calls, [], "同一键上的第二根手指作废这次按压");
}

for (const action of ["cancel", "second-finger", "horizontal", "vertical"]) {
  const { keys, calls } = fixture(false);
  keys.down(pair, true, point(1), RECT);
  if (action === "cancel") keys.cancel(pair, [1]);
  if (action === "second-finger") keys.down(pair, true, point(2), RECT);
  if (action === "horizontal") keys.move(pair, true, [point(1, 130, 100)], RECT);
  if (action === "vertical") keys.move(pair, true, [point(1, 100, 80)], RECT);
  keys.hold(pair, true);
  check(calls.length === 0, `${action} 已取消或被滑动接管的按压不再长按输入`);
}

// 横滑过程中键面显示精确字母，松手恢复。
{
  const { keys, calls, preview } = fixture(false);
  keys.down(pair, true, point(1), RECT);
  keys.move(pair, true, [point(1, 130, 100)], RECT);
  check(preview.letter === "E", "横滑时键面显示精确字母");
  keys.up(pair, true, point(1, 130, 100));
  check(preview.letter === "", "松手后键面恢复整组");
  keys.click(pair, true, 130, 100);
  same(calls, ["letter:E"], "横滑只输入一次精确字母");
}

// 空闲纵滑输入角标，点击先到也一样。
for (const clickFirst of [false, true]) {
  const { keys, calls } = fixture(false);
  keys.down(pair, true, point(1), RECT);
  keys.move(pair, true, [point(1, 100, 80)], RECT);
  if (clickFirst) keys.click(pair, true, 100, 80);
  keys.up(pair, true, point(1, 100, 80));
  if (!clickFirst) keys.click(pair, true, 100, 80);
  same(calls, [keyId(pair, true), "hint:…"], `点击先到=${clickFirst}：纵滑只输入一次角标`);
}

// 没有自己按压的点击（读屏激活）照常送整组。
{
  const { keys, calls } = fixture(false);
  keys.click(pair, true, 0, 0);
  keys.down(pair, true, point(1), RECT);
  keys.up(pair, true, point(1));
  keys.click(pair, true, 100, 100);
  keys.click(pair, true, 0, 0);
  same(
    calls,
    [keyId(pair, true), "grid:w", keyId(pair, true), "grid:w", keyId(pair, true), "grid:w"],
    "读屏激活的点击各送一次整组",
  );
}

const policy = new GridKeyGesturePolicy("we", true);
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.move(1, 100, 80);
check(policy.move(1, 140, 100) === GridKeyGestureMode.SYMBOL, "纵滑先触发后不会切成横滑");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.move(1, 130, 100);
check(policy.move(1, 130, 80) === GridKeyGestureMode.LETTER, "横滑先触发后不会切成符号");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
policy.up(1, 100, 100);
check(!policy.hold(), "松手后的过期长按回调不能输入角标");
policy.start(1, 100, 100, SwipeSymbolsDirection.UP, false);
check(policy.move(2, 100, 60) === GridKeyGestureMode.PENDING, "别的手指移动不影响这次按压");
check(policy.up(2, 100, 60) === GridKeyCommit.NONE, "别的手指抬起不判定这次按压");
check(policy.up(1, 100, 100) === GridKeyCommit.NONE && policy.click(100, 100) === GridKeyCommit.GROUP,
  "空闲点按由点击送整组");
check(!new GridKeyGesturePolicy(null, false).hold(), "没有有效按压或角标不能触发长按");

// 角标的标点路由仍走视图里的真实方法：ASCII 标点交给引擎，其余字面插入；26 键路径保持原样。
const source = require("fs").readFileSync(
  "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets",
  "utf8",
);
const methods = ["typeGridLetterHint", "typeLetterHint"];
const compiled = require("module").stripTypeScriptTypes(
  `class HintView {${methods
    .map((name) => {
      const start = source.indexOf(`  private ${name}(`);
      const end = source.indexOf("\n  private ", start + 1);
      check(start >= 0 && end > start, `没有找到真实方法 ${name}`);
      return source.slice(start, end);
    })
    .join("\n")}\n}`,
);
interface HintView {
  typeGridLetterHint(hint: string): void;
  typeLetterHint(letter: string, hint: string): void;
}
const routed: string[] = [];
const HintViewClass = new Function(
  "KeyboardSession",
  "GridLetterHintTable",
  "KeyIdPolicy",
  `${compiled}; return HintView;`,
)(
  {
    shared: {
      softKeyPress: (id: string): void => {
        routed.push(`key:${id}`);
      },
      commitRaw: (): void => {
        routed.push("raw");
      },
      punctuation: (code: number): void => {
        routed.push(`punctuation:${String.fromCharCode(code)}`);
      },
      insert: (text: string): void => {
        routed.push(`insert:${text}`);
      },
    },
  },
  GridLetterHintTable,
  KeyIdPolicy,
) as new () => HintView;
const hintView = new HintViewClass();
hintView.typeGridLetterHint(";");
hintView.typeGridLetterHint("…");
same(routed, ["raw", "punctuation:;", "raw", "insert:…"], "网格角标先原样上屏，ASCII 标点走引擎");
routed.length = 0;
hintView.typeLetterHint("l", "『");
same(routed, [`key:${KeyIdPolicy.character("l")}`, "raw", "insert:『"], "26 键保持既有原样上屏和字面插入路径");

console.log("14/18 键触摸控制器：长按、点击与抬手不分先后、双拇指、同键多指、横滑、纵滑和标点路由均通过");
