/** 辅助码前导键插回按键原文（#10731）：2in1 行内「原始按键」样式和读音行都画原文，原文里没有前导键。 */
import {
  HelpcodeTriggerPolicy,
  HelpcodeTriggerView,
} from "../entry/src/main/ets/keyboard/input/HelpcodeTriggerPolicy";
import { InlinePreeditPolicy } from "../entry/src/main/ets/keyboard/input/InlinePreeditPolicy";

let failures = 0;

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

const P = HelpcodeTriggerPolicy;
const atEnd: HelpcodeTriggerView = { key: ";", position: 2 };

group("前导键插回原文，光标跟到它后面", () => {
  check(P.text("ni", atEnd) === "ni;", "刚按下前导键时画 ni;");
  check(P.caret("ni", 2, atEnd) === 3, "光标在前导键后面");
  check(P.text("niF", atEnd) === "ni;F", "输了一位码时前导键在码前面");
  check(P.caret("niF", 3, atEnd) === 4, "光标仍在末尾");
  check(P.caret("ni", 1, { key: "/", position: 2 }) === 1, "光标在前导键前面时不动");
});

group("没有前导键或字段不合法时原样返回", () => {
  check(P.text("ni", undefined) === "ni" && P.caret("ni", 2, undefined) === 2, "没有字段");
  check(P.text("ni", { key: "", position: 2 }) === "ni", "空的前导键");
  check(P.text("ni", { key: ";", position: 3 }) === "ni", "位置越界");
  check(P.text("ni", { key: ";", position: -1 }) === "ni", "位置为负");
  check(P.text("ni", { key: ";", position: 1.5 }) === "ni", "位置不是整数");
  check(P.caret("ni", 2, { key: ";", position: 3 }) === 2, "越界时光标不动");
});

group("2in1 原始按键样式写进输入框的是插回前导键的原文", () => {
  const drawn: string = P.text("ni", atEnd);
  check(
    InlinePreeditPolicy.text("raw", true, true, drawn, "ni;", "") === "ni;",
    "原始按键样式画 ni;",
  );
  check(
    InlinePreeditPolicy.text("pinyin", true, true, drawn, "ni;", "") === "ni;",
    "拼音样式画引擎的 preedit",
  );
});

if (failures > 0) {
  console.log(`${failures} helpcode trigger group(s) failed`);
  throw new Error("helpcode trigger checks failed");
}
