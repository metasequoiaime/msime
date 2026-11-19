/**
 * 日语方案符号键画的键面，即引擎按标点契约（`shared/contracts/punctuation/policy.json` 的 `japanese` 表）为这个 ASCII 键写出的字。键仍发送 ASCII，由引擎决定上屏的字，这里只决定键上画什么。
 *
 * 键面与上屏是一张表：`,` 和 `\` 出读点、`[` `]` 出直角引号、`/` 出中点，中文引号样式、顿号开关和书名号嵌套都不作用于日语。
 */
const FACES: Map<string, string> = new Map<string, string>([
  [",", "、"],
  ["\\", "、"],
  [".", "。"],
  ["[", "「"],
  ["]", "」"],
  ["{", "『"],
  ["}", "』"],
  ["/", "・"],
  ["~", "〜"],
  ["?", "？"],
  ["!", "！"],
  ["(", "（"],
  [")", "）"],
  [":", "："],
  [";", "；"],
]);

export class JapaneseSymbolFaces {
  /** `symbol` 在日语方案下的键面；没有日文键面时返回 ASCII 本身（引擎同样原样上屏）。 */
  static face(symbol: string): string {
    return FACES.get(symbol) ?? symbol;
  }
}
