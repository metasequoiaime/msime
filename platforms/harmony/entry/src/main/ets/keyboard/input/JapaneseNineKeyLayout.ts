/**
 * Kana labels and Engine romanization strokes, ported from
 * platforms/android/java/app/msime/client/JapaneseNineKeyLayout.java.
 *
 * Each key carries five directions in the order centre, left, up, right, down — the same indices the
 * Apple host uses, so a flick means the same thing on every platform. An empty stroke marks a label
 * that commits directly instead of composing.
 */
export interface JapaneseKey {
  readonly kana: string[];
  readonly strokes: string[];
}

export interface VariantGroup {
  readonly title: string;
  readonly kana: string[];
  readonly strokes: string[];
}

/** Direction indices, matching the Apple host. */
export const DIRECTION_CENTRE: number = 0;
export const DIRECTION_LEFT: number = 1;
export const DIRECTION_UP: number = 2;
export const DIRECTION_RIGHT: number = 3;
export const DIRECTION_DOWN: number = 4;

function key(
  centre: string,
  left: string,
  up: string,
  right: string,
  down: string,
  centreStroke: string,
  leftStroke: string,
  upStroke: string,
  rightStroke: string,
  downStroke: string,
): JapaneseKey {
  return {
    kana: [centre, left, up, right, down],
    strokes: [centreStroke, leftStroke, upStroke, rightStroke, downStroke],
  };
}

function group(title: string, kana: string[], strokes: string[]): VariantGroup {
  if (kana.length !== strokes.length || kana.length === 0) {
    throw new Error("Japanese variant labels and strokes must match");
  }
  return { title: title, kana: kana, strokes: strokes };
}

const KEYS: JapaneseKey[] = [
  key("あ", "い", "う", "え", "お", "a", "i", "u", "e", "o"),
  key("か", "き", "く", "け", "こ", "ka", "ki", "ku", "ke", "ko"),
  key("さ", "し", "す", "せ", "そ", "sa", "shi", "su", "se", "so"),
  key("た", "ち", "つ", "て", "と", "ta", "chi", "tsu", "te", "to"),
  key("な", "に", "ぬ", "ね", "の", "na", "ni", "nu", "ne", "no"),
  key("は", "ひ", "ふ", "へ", "ほ", "ha", "hi", "fu", "he", "ho"),
  key("ま", "み", "む", "め", "も", "ma", "mi", "mu", "me", "mo"),
  key("や", "「", "ゆ", "」", "よ", "ya", "", "yu", "", "yo"),
  key("ら", "り", "る", "れ", "ろ", "ra", "ri", "ru", "re", "ro"),
  key("わ", "を", "ん", "ー", "〜", "wa", "wo", "n'", "-", ""),
  key("、", "。", "？", "！", "…", "", "", "", "", ""),
];

/** Symbols printed on the Japanese nine-key digit layer. All choices commit directly. */
const DIGIT_KEYS: JapaneseKey[] = [
  key("1", "☆", "♪", "→", "", "", "", "", "", ""),
  key("2", "¥", "$", "€", "", "", "", "", "", ""),
  key("3", "%", "°", "#", "", "", "", "", "", ""),
  key("4", "○", "*", "・", "", "", "", "", "", ""),
  key("5", "+", "-", "=", "", "", "", "", "", ""),
  key("6", "<", "^", ">", "", "", "", "", "", ""),
  key("7", "「", "」", "：", "", "", "", "", "", ""),
  key("8", "〒", "※", "♂", "", "", "", "", "", ""),
  key("9", "（", "）", "／", "", "", "", "", "", ""),
  key("0", "〜", "…", "ー", "", "", "", "", "", ""),
  key("、", "。", "？", "！", "…", "", "", "", "", ""),
];

const DIGIT_BRACKETS: string[] = ["（", "）", "「", "」", "『", "』", "【", "】"];

const VARIANTS: VariantGroup[] = [
  group(
    "小假名",
    ["ぁ", "ぃ", "ぅ", "ぇ", "ぉ", "ゃ", "ゅ", "ょ", "っ", "ゎ"],
    ["xa", "xi", "xu", "xe", "xo", "xya", "xyu", "xyo", "xtsu", "xwa"],
  ),
  group(
    "浊音",
    [
      "が",
      "ぎ",
      "ぐ",
      "げ",
      "ご",
      "ざ",
      "じ",
      "ず",
      "ぜ",
      "ぞ",
      "だ",
      "ぢ",
      "づ",
      "で",
      "ど",
      "ば",
      "び",
      "ぶ",
      "べ",
      "ぼ",
      "ゔ",
    ],
    [
      "ga",
      "gi",
      "gu",
      "ge",
      "go",
      "za",
      "ji",
      "zu",
      "ze",
      "zo",
      "da",
      "di",
      "du",
      "de",
      "do",
      "ba",
      "bi",
      "bu",
      "be",
      "bo",
      "vu",
    ],
  ),
  group("半浊音", ["ぱ", "ぴ", "ぷ", "ぺ", "ぽ"], ["pa", "pi", "pu", "pe", "po"]),
];

export class JapaneseNineKeyLayout {
  static keys(): JapaneseKey[] {
    return KEYS;
  }

  static digitKeys(): JapaneseKey[] {
    return DIGIT_KEYS;
  }

  static digitBrackets(): string[] {
    return DIGIT_BRACKETS;
  }

  static variants(): VariantGroup[] {
    return VARIANTS;
  }

  /**
   * Which of the five directions a flick landed on. A movement shorter than the threshold in both
   * axes is the centre, and the larger axis wins so a diagonal never falls between two directions.
   */
  static direction(offsetX: number, offsetY: number, threshold: number): number {
    if (threshold < 0) {
      throw new Error("Flick threshold cannot be negative");
    }
    if (Math.max(Math.abs(offsetX), Math.abs(offsetY)) < threshold) {
      return DIRECTION_CENTRE;
    }
    if (Math.abs(offsetX) > Math.abs(offsetY)) {
      return offsetX < 0 ? DIRECTION_LEFT : DIRECTION_RIGHT;
    }
    return offsetY < 0 ? DIRECTION_UP : DIRECTION_DOWN;
  }
}
