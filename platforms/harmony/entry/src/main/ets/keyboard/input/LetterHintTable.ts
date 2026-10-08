/**
 * 26 个字母键右上角的符号提示，移植自 `platforms/android/java/app/msime/android/keyboard/LetterHintTable.java`：设计里的 q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。在键上滑动（向下，或设置为向上时向上）或长按时打出的就是它。
 */
const HINTS: Map<string, string> = new Map<string, string>([
  ["q", "1"],
  ["w", "2"],
  ["e", "3"],
  ["r", "4"],
  ["t", "5"],
  ["y", "6"],
  ["u", "7"],
  ["i", "8"],
  ["o", "9"],
  ["p", "0"],
  ["a", "@"],
  ["s", "#"],
  ["d", "¥"],
  ["f", "%"],
  ["g", "&"],
  ["h", "*"],
  ["j", "("],
  ["k", ")"],
  ["l", '"'],
  ["z", "~"],
  ["x", "…"],
  ["c", "、"],
  ["v", "?"],
  ["b", "!"],
  ["n", "-"],
  ["m", "/"],
]);

export class LetterHintTable {
  /** 一个字母键的提示，大小写均可；不是 26 个 ASCII 字母之一时为 null。 */
  static hint(letter: string | null | undefined): string | null {
    if (letter === null || letter === undefined || letter.length !== 1) {
      return null;
    }
    const hint: string | undefined = HINTS.get(letter.toLowerCase());
    return hint === undefined ? null : hint;
  }
}
