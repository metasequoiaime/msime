/**
 * Presentation rule for the optional remaining-code hint on Wubi candidates, ported from
 * platforms/android/java/app/msime/android/WubiCodeHintPolicy.java.
 */
const MAX_CODE_LENGTH: number = 64;

/** 五笔 14 键每一组的代表字母（引擎 `wubi::grid`）到这一组的字母；ZX 只代表 `x`。 */
const FOURTEEN_KEY_GROUPS: Record<string, string> = {
  q: "qw",
  e: "er",
  t: "ty",
  u: "ui",
  o: "op",
  a: "as",
  d: "df",
  g: "gh",
  j: "jk",
  l: "l",
  x: "x",
  c: "cv",
  b: "bn",
  m: "m",
};

export class WubiCodeHintPolicy {
  static readonly WUBI_SCHEME: number = 2;

  /**
   * 候选编码里已经打过的那一段。26 键就是预编辑；五笔 14 键的 `editing_text` 是组码（`qq`），预编辑只是首选的编码，
   * 别的编码的候选要按组对照：编码的前几位逐位落在这几组里，这几位就是它已打的部分，剩下的才是提示。对不上组时返回预编辑，交给 `hint` 判为不提示。
   */
  static typed(
    code: string | null,
    preedit: string | null,
    keyGrid: string,
    editingText: string,
  ): string | null {
    if (keyGrid !== "fourteen_key" || code === null || editingText.length === 0) {
      return preedit;
    }
    if (code.length < editingText.length || editingText.length > MAX_CODE_LENGTH) {
      return preedit;
    }
    for (let index = 0; index < editingText.length; index++) {
      const letters = FOURTEEN_KEY_GROUPS[editingText.charAt(index)];
      if (letters === undefined || letters.indexOf(code.charAt(index)) < 0) {
        return preedit;
      }
    }
    return code.substring(0, editingText.length);
  }

  /**
   * Only the untyped suffix, and only when the candidate code strictly extends the current preedit.
   * Fallback and local candidates are deliberately left unannotated: their code is not the one the
   * user is partway through typing.
   */
  static hint(
    code: string | null,
    typed: string | null,
    enabled: boolean,
    scheme: number,
    localMode: string,
    answeredByPinyinFallback: boolean,
  ): string {
    if (
      !enabled ||
      scheme !== WubiCodeHintPolicy.WUBI_SCHEME ||
      answeredByPinyinFallback ||
      localMode !== "none" ||
      code === null ||
      typed === null ||
      typed.length === 0 ||
      code.length > MAX_CODE_LENGTH ||
      typed.length > MAX_CODE_LENGTH ||
      code.length <= typed.length ||
      !code.startsWith(typed)
    ) {
      return "";
    }
    return code.substring(typed.length);
  }
}
