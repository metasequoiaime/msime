import { ChineseSymbolFaces } from "./ChineseSymbolFaces";

/** 14/18 键字母键右上角的符号，与 Android、iOS 共用同一张表。 */
const FOURTEEN: Map<string, string> = new Map<string, string>([
  ["qw", ";"],
  ["er", "#"],
  ["ty", "…"],
  ["ui", "("],
  ["op", ")"],
  ["as", "@"],
  ["df", "-"],
  ["gh", "、"],
  ["jk", ":"],
  ["l", '"'],
  ["zx", "~"],
  ["cv", "?"],
  ["bn", "!"],
  ["m", "/"],
]);

const EIGHTEEN: Map<string, string> = new Map<string, string>([
  ["q", ";"],
  ["we", "…"],
  ["rt", "-"],
  ["y", "¥"],
  ["u", "&"],
  ["io", "("],
  ["p", ")"],
  ["a", "@"],
  ["sd", "#"],
  ["fg", "%"],
  ["h", "*"],
  ["jk", ":"],
  ["l", '"'],
  ["z", "~"],
  ["xc", "、"],
  ["v", "?"],
  ["bn", "!"],
  ["m", "/"],
]);

/** 中文标点设置下由 Engine 路由的 ASCII 标点。 */
const ENGINE_PUNCTUATION: Set<string> = new Set<string>(["(", ")", '"', "?", "!", ":", ";"]);

export class GridLetterHintTable {
  static hint(letters: string | null | undefined, eighteen: boolean): string | null {
    if (letters === null || letters === undefined) return null;
    const hint: string | undefined = (eighteen ? EIGHTEEN : FOURTEEN).get(letters.toLowerCase());
    return hint === undefined ? null : hint;
  }

  static face(
    letters: string | null | undefined,
    eighteen: boolean,
    chinese: boolean,
    cornerQuotes: boolean,
    hollowBrackets: boolean,
  ): string | null {
    const hint: string | null = GridLetterHintTable.hint(letters, eighteen);
    if (hint === null || !chinese) return hint;
    return ChineseSymbolFaces.face(hint, cornerQuotes, hollowBrackets) ?? hint;
  }

  static routesThroughEngine(hint: string): boolean {
    return ENGINE_PUNCTUATION.has(hint);
  }
}
