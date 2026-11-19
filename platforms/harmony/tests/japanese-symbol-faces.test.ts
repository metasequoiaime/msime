import { JapaneseSymbolFaces } from "../entry/src/main/ets/keyboard/input/JapaneseSymbolFaces";

let assertions: number = 0;
function check(actual: string, expected: string, message: string): void {
  assertions++;
  if (actual !== expected) throw new Error(`${message}: ${actual} != ${expected}`);
}

// 日语方案的符号键面与引擎 policy.json 的 japanese 表逐字一致：键面画什么就上屏什么。
check(JapaneseSymbolFaces.face(","), "、", "Japanese comma");
check(JapaneseSymbolFaces.face("\\"), "、", "Japanese ideographic comma key");
check(JapaneseSymbolFaces.face("."), "。", "Japanese period");
check(JapaneseSymbolFaces.face("["), "「", "Japanese opening corner bracket");
check(JapaneseSymbolFaces.face("]"), "」", "Japanese closing corner bracket");
check(JapaneseSymbolFaces.face("{"), "『", "Japanese opening double corner bracket");
check(JapaneseSymbolFaces.face("}"), "』", "Japanese closing double corner bracket");
check(JapaneseSymbolFaces.face("/"), "・", "Japanese middle dot");
check(JapaneseSymbolFaces.face("~"), "〜", "Japanese long vowel");
check(JapaneseSymbolFaces.face("?"), "？", "Japanese question mark");
check(JapaneseSymbolFaces.face("!"), "！", "Japanese exclamation mark");
check(JapaneseSymbolFaces.face("("), "（", "Japanese opening parenthesis");
check(JapaneseSymbolFaces.face(")"), "）", "Japanese closing parenthesis");
check(JapaneseSymbolFaces.face(":"), "：", "Japanese colon");
check(JapaneseSymbolFaces.face(";"), "；", "Japanese semicolon");
check(JapaneseSymbolFaces.face("@"), "@", "unmapped symbol keeps its face");

console.log(`Harmony Japanese symbol faces: ${assertions} checks passed`);
