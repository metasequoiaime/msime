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

// 引擎只在中文标点开关开着时写日文标点，关着时写 ASCII，键面要跟着。
check(String(JapaneseSymbolFaces.active(false, "none", "japanese", true)), "true", "switch on draws Japanese faces");
check(String(JapaneseSymbolFaces.active(false, "none", "japanese", false)), "false", "switch off draws ASCII faces");
check(String(JapaneseSymbolFaces.active(true, "none", "japanese", true)), "false", "English draws ASCII faces");
check(String(JapaneseSymbolFaces.active(false, "calculator", "japanese", true)), "false", "local tool draws ASCII faces");
check(String(JapaneseSymbolFaces.active(false, "none", "pinyin", true)), "false", "other schemes are not Japanese");

console.log(`Harmony Japanese symbol faces: ${assertions} checks passed`);
