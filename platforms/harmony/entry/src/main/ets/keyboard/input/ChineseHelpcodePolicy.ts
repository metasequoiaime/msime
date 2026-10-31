/** Every key a helpcode trigger can be: `;` and the backtick for the shuangpin indirect triggers, `/` ending a direct helpcode, and the choices of the configurable trigger key. */
const TRIGGER_KEYS: string = ";`/\\=";

/**
 * Host-side gate for sending an uppercase letter to the Engine as composition helpcode, ported from
 * platforms/android/java/app/msime/android/ChineseHelpcodePolicy.java.
 *
 * Scheme 0 and 1 are quanpin and shuangpin; helpcode means nothing in the others.
 */
export class ChineseHelpcodePolicy {
  static eligible(
    dedicatedEnglish: boolean,
    editingText: string | null,
    scheme: number,
    localMode: string,
  ): boolean {
    return (
      !dedicatedEnglish &&
      editingText !== null &&
      editingText.length > 0 &&
      localMode === "none" &&
      (scheme === 0 || scheme === 1)
    );
  }

  static entersHelpcode(
    dedicatedEnglish: boolean,
    shifted: boolean,
    editingText: string | null,
    scheme: number,
    localMode: string,
  ): boolean {
    return (
      shifted && ChineseHelpcodePolicy.eligible(dedicatedEnglish, editingText, scheme, localMode)
    );
  }

  /**
   * The character a letter key sends when the Engine is told whether it was shifted. The flag does not change the case: the Engine reads helpcode from an uppercase letter and an English composition keeps the case it receives, so a shifted key sends the capital, as on Android and iOS.
   */
  static letterInput(letter: string, shifted: boolean): string {
    return shifted ? letter.toUpperCase() : letter;
  }

  /**
   * The helpcode trigger a touch 123-layer key types while a pinyin composition is open, or null when the key inserts its face as usual. The Chinese layer draws `；` for the semicolon key and draws the other trigger keys as themselves. A key counts only while the Engine lists it in `spelling_symbols`, which it does exactly when the key would open a helpcode segment: the shuangpin indirect triggers, `/` after a direct helpcode, and the configurable trigger key. Any other key, and a trigger key whose trigger is off, still commits the composition and inserts its face.
   */
  static layerTrigger(face: string, spellingSymbols: string, eligible: boolean): string | null {
    if (!eligible) {
      return null;
    }
    const key: string = face === "；" ? ";" : face;
    return key.length === 1 && TRIGGER_KEYS.includes(key) && spellingSymbols.includes(key)
      ? key
      : null;
  }
}
