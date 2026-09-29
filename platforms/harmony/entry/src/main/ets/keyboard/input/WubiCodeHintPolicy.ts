/**
 * Presentation rule for the optional remaining-code hint on Wubi candidates, ported from
 * platforms/android/java/app/msime/client/WubiCodeHintPolicy.java.
 */
const MAX_CODE_LENGTH: number = 64;

export class WubiCodeHintPolicy {
  static readonly WUBI_SCHEME: number = 2;

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
