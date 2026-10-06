/**
 * What the host does differently while the Engine composes Korean Hangul (scheme 4).
 *
 * Korean is a syllable automaton. The view's `editing_text` holds only the key letters of the open syllable ("sud" for 녕), so it says whether a syllable is open but is never what the user should see: the syllable itself is `preedit`. A key the automaton does not take (Space, Return, a digit) commits the syllable and is left unhandled, and the host then does that key's normal work after the commit.
 *
 * The only candidates the scheme ever has are the Hanja of the composing syllable, listed after MSIME_CONVERT_HANJA and gone again when the list closes. The table and the list's rules are the Engine's (the Korean contract in msime_client.h); the host only decides which keys and taps reach the list while it is open.
 */
export const KOREAN_SCHEME: number = 4;

export class KoreanCompositionPolicy {
  /** Whether letters are going to the Hangul automaton: the Korean scheme, with neither English nor a local utility mode taking the keys. */
  static active(scheme: number, dedicatedEnglish: boolean, localMode: string): boolean {
    return scheme === KOREAN_SCHEME && !dedicatedEnglish && localMode === "none";
  }

  /** Whether the keyboard's own scheme choice is Korean, before the Engine has reported anything. */
  static selected(schemeName: string, english: boolean, localMode: string): boolean {
    return schemeName === "korean" && !english && localMode === "none";
  }

  /**
   * Whether the Hanja list of the composing syllable is open: the Korean rules hold (see `active`) and the view carries candidates, since the Engine offers none in this scheme until MSIME_CONVERT_HANJA, so no separate view field is needed.
   */
  static hanjaListOpen(korean: boolean, candidateCount: number): boolean {
    return korean && candidateCount > 0;
  }

  /**
   * Whether MSIME_CONVERT_HANJA applies, which is also when the candidate bar shows its 漢 button: while a syllable composes, its list open or not, since the command closes an open list.
   *
   * A lone jamo composes too and has no Hanja. The Engine answers the command unhandled then and nothing changes; the host does not tell the two apart, because that would take a jamo table of its own.
   */
  static convertsHanja(korean: boolean, editing: string): boolean {
    return korean && editing.length > 0;
  }

  /**
   * Whether a touch key the automaton leaves unhandled is typed by the keyboard after the commit: a space or a digit, the keys a phone has no application behind to type them.
   */
  static typesAfterCommit(character: number): boolean {
    return character === 0x20 || (character >= 0x30 && character <= 0x39);
  }
}
