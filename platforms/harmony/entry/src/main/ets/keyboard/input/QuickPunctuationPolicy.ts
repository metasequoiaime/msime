/**
 * The quick punctuation menu for the alphabetic keyboard, ported from
 * platforms/android/java/app/msime/android/QuickPunctuationPolicy.java.
 *
 * Each entry prints one face and sends a different ASCII character: the Engine decides the shape from
 * the current scheme, so the host must send the key rather than the glyph.
 */
export interface PunctuationEntry {
  readonly face: string;
  /** The ASCII character handed to the Engine, not the printed face. */
  readonly input: string;
}

function entry(face: string, input: string): PunctuationEntry {
  const code: number = input.charCodeAt(0);
  if (face.length === 0 || code < 32 || code > 126) {
    throw new Error("Invalid quick punctuation entry");
  }
  return { face: face, input: input };
}

const ASCII: PunctuationEntry[] = [
  entry(",", ","),
  entry(".", "."),
  entry("?", "?"),
  entry("!", "!"),
  entry(":", ":"),
  entry(";", ";"),
  entry("@", "@"),
];
const CHINESE: PunctuationEntry[] = [
  entry("，", ","),
  entry("。", "."),
  entry("？", "?"),
  entry("！", "!"),
  entry("、", "\\"),
  entry("；", ";"),
  entry("：", ":"),
];
const JAPANESE: PunctuationEntry[] = [
  entry("、", "\\"),
  entry("。", "."),
  entry("？", "?"),
  entry("！", "!"),
  entry("「", "["),
  entry("」", "]"),
  entry("・", "/"),
];

export class QuickPunctuationPolicy {
  /** Display faces paired with the ASCII input each one sends to the Engine. */
  /** `preserveEnglish` 是 View 的 `preserve_english_punctuation`（#11890）：中文表里设成保留英文的键 Engine 原样写出 ASCII，这一项就画这个键，键面与上屏一致（与 `ChineseSymbolFaces.face` 同一条规则）。日文表和注音（scheme 6，Engine 先按注音规则处理这些键）不看这个集合。 */
  static entries(dedicatedEnglish: boolean, scheme: number, localMode: string,
    preserveEnglish: string = ""): PunctuationEntry[] {
    // Korean (scheme 4) writes half-width ASCII punctuation, so its menu is the ASCII one.
    if (dedicatedEnglish || localMode !== "none" || scheme === 4) {
      return ASCII;
    }
    if (scheme === 3) return JAPANESE;
    if (preserveEnglish.length === 0 || scheme === 6) return CHINESE;
    return CHINESE.map((item: PunctuationEntry): PunctuationEntry =>
      preserveEnglish.includes(item.input) ? entry(item.input, item.input) : item);
  }
}
