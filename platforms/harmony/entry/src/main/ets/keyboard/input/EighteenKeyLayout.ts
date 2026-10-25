import { FourteenKey } from "./FourteenKeyLayout";

/** 全拼 18 键的键表：相邻字母按 QWERTY 位置合并，组码交给共享引擎消歧。 */
export class EighteenKeyLayout {
  private static key(letters: string): FourteenKey {
    const upper: string = letters.toUpperCase();
    return {
      letters,
      input: letters.charAt(0),
      label: upper,
      description: letters.length === 1 ? `字母 ${upper}` : `按键 ${upper.split("").join(" ")}`,
    };
  }

  static readonly ROWS: FourteenKey[][] = [
    ["q", "we", "rt", "y", "u", "io", "p"].map((letters) => EighteenKeyLayout.key(letters)),
    ["a", "sd", "fg", "h", "jk", "l"].map((letters) => EighteenKeyLayout.key(letters)),
    ["z", "xc", "v", "bn", "m"].map((letters) => EighteenKeyLayout.key(letters)),
  ];

  static rows(): FourteenKey[][] {
    return EighteenKeyLayout.ROWS;
  }

  static holdOptions(key: FourteenKey): string[] {
    return key.letters.length > 1 ? key.letters.split("") : [];
  }
}
