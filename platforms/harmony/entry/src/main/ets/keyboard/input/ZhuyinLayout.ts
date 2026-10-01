/**
 * The Zhuyin Dachen (大千) touch layout: which ASCII keys the keyboard lays out, which bopomofo symbol or tone mark each one wears, and how the symbol layer stays clear of the keys the Dachen editor claims.
 *
 * The Engine reads Dachen as the ASCII keys of a standard keyboard (crates/engine/src/zhuyin/layout.rs, after libchewing), so the keyboard draws those 41 keys in their physical rows and sends the ASCII key; the editor owns the syllable and the conversion. Space is tone 1 and needs no key of its own.
 */

/** The ASCII keys in their physical rows: the digit row with `-`, then the three letter rows, each extended by the punctuation keys Dachen gives a symbol. */
const ROWS: string[][] = [
  ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"],
  ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
  ["a", "s", "d", "f", "g", "h", "j", "k", "l", ";"],
  ["z", "x", "c", "v", "b", "n", "m", ",", ".", "/"],
];

const FACES: Map<string, string> = new Map<string, string>([
  ["1", "ㄅ"],
  ["2", "ㄉ"],
  ["3", "ˇ"],
  ["4", "ˋ"],
  ["5", "ㄓ"],
  ["6", "ˊ"],
  ["7", "˙"],
  ["8", "ㄚ"],
  ["9", "ㄞ"],
  ["0", "ㄢ"],
  ["-", "ㄦ"],
  ["q", "ㄆ"],
  ["w", "ㄊ"],
  ["e", "ㄍ"],
  ["r", "ㄐ"],
  ["t", "ㄔ"],
  ["y", "ㄗ"],
  ["u", "ㄧ"],
  ["i", "ㄛ"],
  ["o", "ㄟ"],
  ["p", "ㄣ"],
  ["a", "ㄇ"],
  ["s", "ㄋ"],
  ["d", "ㄎ"],
  ["f", "ㄑ"],
  ["g", "ㄕ"],
  ["h", "ㄘ"],
  ["j", "ㄨ"],
  ["k", "ㄜ"],
  ["l", "ㄠ"],
  [";", "ㄤ"],
  ["z", "ㄈ"],
  ["x", "ㄌ"],
  ["c", "ㄏ"],
  ["v", "ㄒ"],
  ["b", "ㄖ"],
  ["n", "ㄙ"],
  ["m", "ㄩ"],
  [",", "ㄝ"],
  [".", "ㄡ"],
  ["/", "ㄥ"],
]);

/** What a screen reader says for the four tone keys, whose marks alone read as nothing. */
const TONE_LABELS: Map<string, string> = new Map<string, string>([
  ["6", "二声"],
  ["3", "三声"],
  ["4", "四声"],
  ["7", "轻声"],
]);

/** Every non-letter key the Dachen editor may claim (`DACHEN_SYMBOLS` in crates/engine/src/zhuyin/layout.rs, without Space). */
const CLAIMED_SYMBOLS: string = "1234567890,./;-";

export class ZhuyinLayout {
  static readonly ROWS: string[][] = ROWS;

  /** The bopomofo symbol or tone mark a key wears, or the key itself for one the layout does not cover. */
  static face(key: string): string {
    return FACES.get(key) ?? key;
  }

  /** What a screen reader says for a key: the symbol, or the tone's name for a tone key. */
  static label(key: string): string {
    return TONE_LABELS.get(key) ?? ZhuyinLayout.face(key);
  }

  /**
   * Whether the Engine would take a key as a row pick while the candidate list is open: there the digits 1-9 choose a row instead of typing ㄅ, ㄉ or a tone (crates/engine/src/session/input.rs). A touch keyboard picks rows on the strip, so the keyboard closes the list before sending such a key and it types what its face shows.
   */
  static selectsWhileListOpen(key: string): boolean {
    return key.length === 1 && key >= "1" && key <= "9";
  }

  /**
   * Whether a symbol-layer key is one the Dachen editor claims as a phonetic or tone key. Sent to the Engine it would compose ㄝ for a comma rather than type the comma, so the keyboard finishes the conversion and types the mark itself.
   */
  static claimsSymbol(symbol: string): boolean {
    return symbol.length === 1 && CLAIMED_SYMBOLS.includes(symbol);
  }

  /** The height of each of the four Dachen rows, drawn in the space the three letter rows and the two gaps between them take on every other layout, so the keyboard does not change height when Zhuyin is picked. */
  static rowHeight(letterRowsHeight: number, rowSpacing: number): number {
    return (letterRowsHeight - rowSpacing) / ROWS.length;
  }
}
