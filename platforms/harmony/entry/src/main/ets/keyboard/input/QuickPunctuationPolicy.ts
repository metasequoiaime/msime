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
  static entries(dedicatedEnglish: boolean, scheme: number, localMode: string): PunctuationEntry[] {
    if (dedicatedEnglish || localMode !== "none") {
      return ASCII;
    }
    return scheme === 3 ? JAPANESE : CHINESE;
  }
}
