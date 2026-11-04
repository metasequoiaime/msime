/**
 * 手机候选栏读音行点开编辑拼写（#10591，笔记 2026-11-02-harmony-pronunciation-row-edit.md）：什么时候能点、点开前后画什么、什么时候收起。
 */
import { ReadingRowEditPolicy as Policy } from "../entry/src/main/ets/keyboard/candidate/ReadingRowEditPolicy";
import { VisibleComposition } from "../entry/src/main/ets/keyboard/candidate/CandidatePreeditStylePolicy";

let assertions = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}
function equal(actual: Object, expected: Object, message: string): void {
  check(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${message}: ${JSON.stringify(actual)}`,
  );
}

// 能不能点开。
check(Policy.editable(false, true, "nihao", false), "手机行内组字、方案不锁光标时可点开");
check(!Policy.editable(false, true, "nihao", true), "锁定组字光标的方案（韩语、注音等）不接触摸");
check(!Policy.editable(false, true, "", false), "没有组字时不接触摸");
check(
  !Policy.editable(false, false, "nihao", false),
  "拼写没写进输入框（读音已画在候选栏上）时不接触摸",
);
check(!Policy.editable(true, true, "nihao", false), "2in1 候选窗口不接触摸");

// 画出来的组字。
equal(
  Policy.visible(false, false, true, false, "", "nihao", ""),
  { text: "", caret: false },
  "手机行内组字未点开时不画拼写",
);
equal(
  Policy.visible(false, false, true, true, "", "nihao", ""),
  { text: "nihao", caret: true },
  "点开后画拼写和光标",
);
equal(
  Policy.visible(false, false, false, false, "", "nihao", ""),
  { text: "nihao", caret: true },
  "拼写写不进输入框时照旧画",
);
equal(
  Policy.visible(true, false, true, true, "", "nihao", ""),
  { text: "", caret: false },
  "2in1 照「候选窗口预编辑」关着时不画",
);
equal(
  Policy.visible(true, true, true, false, "", "nihao", ""),
  { text: "nihao", caret: true },
  "2in1 照「候选窗口预编辑」开着时画",
);

// 未点开时的弱化提示：可点开的读音行不能是一行空白。
const hidden: VisibleComposition = Policy.visible(false, false, true, false, "", "nihao", "");
equal(Policy.hint(true, false, "", "nihao", hidden), "nihao", "可点开、未点开时弱化画出拼写");
const expanded: VisibleComposition = Policy.visible(false, false, true, true, "", "nihao", "");
equal(Policy.hint(true, true, "", "nihao", expanded), "", "点开后不再画提示");
equal(Policy.hint(false, false, "", "nihao", hidden), "", "不可点开时没有提示，读音行照旧");
const phrase: VisibleComposition = Policy.visible(false, false, true, false, "你好", "你好shi", "");
equal(phrase.text, "你好", "短语已选定的部分照常画");
equal(
  Policy.hint(true, false, "你好", "你好shi", phrase),
  "shi",
  "提示只接在已选定部分后面画剩下的拼写",
);

// 收起。
check(Policy.keepsExpanded(true, true), "仍可编辑时保持点开");
check(!Policy.keepsExpanded(true, false), "组字结束、离开行内组字或换成锁光标方案时收起");
check(!Policy.keepsExpanded(false, true), "没点过就不会自己点开");

console.log(`reading-row-edit: ${assertions} assertions passed`);
