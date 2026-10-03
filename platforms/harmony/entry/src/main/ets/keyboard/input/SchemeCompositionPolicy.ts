import { SchemeTraits } from "../SchemeTraits";

/**
 * How the keyboard draws and drives a composition whose shape the scheme decides, read from the Engine's view.
 *
 * Korean, Zhuyin and Vietnamese compose text the user already reads as written (a Hangul syllable, a Zhuyin conversion, a Vietnamese word with its marks) rather than a spelling of it, and the Engine keeps the caret at its end (`locks_caret`). Their `editing_text` is the keys behind it ("sud" for 녕, "su3" for 你), or for Vietnamese the word itself with a byte offset for a caret, so the composition drawn is `preedit` and the caret is its end. Korean and Zhuyin also list candidates only in a list the user opens (`has_openable_candidate_list`), which the view reports as `candidate_list_open`.
 *
 * Every rule here is the scheme's own, so none holds while dedicated English or a local utility mode takes the keys; `rulesScheme` folds that in once.
 */
export class SchemeCompositionPolicy {
  /** The Engine scheme number whose own rules are in force, or -1 while dedicated English or a local utility mode takes the keys. */
  static rulesScheme(scheme: number, dedicatedEnglish: boolean, localMode: string): number {
    return dedicatedEnglish || localMode !== "none" ? -1 : scheme;
  }

  /** The keyboard's own scheme choice, by wire name, before the Engine has reported anything. */
  static selectedRulesScheme(schemeName: string, english: boolean, localMode: string): number {
    return SchemeCompositionPolicy.rulesScheme(
      SchemeTraits.fromName(schemeName),
      english,
      localMode,
    );
  }

  /** Whether the composition is drawn as the text it writes, with the caret at its end. */
  static drawsPreedit(rulesScheme: number): boolean {
    return SchemeTraits.locksCaret(rulesScheme);
  }

  /**
   * 笔画方案：`editing_text` 是键入的字母 hspnzx，`preedit` 是一笔一个字形的 一丨丿丶乛＊。画出来的是字形，但光标仍是引擎的：字母和字形一一对应，且都在 BMP 内，所以字母串里的下标就是字形串里的下标。
   */
  static drawsKeyGlyphs(rulesScheme: number): boolean {
    return rulesScheme === SchemeTraits.STROKE;
  }

  /** The composition to draw: the written text for a scheme that composes it, the Stroke glyphs for Stroke, the spelling for every other scheme. */
  static reading(rulesScheme: number, editing: string, preedit: string): string {
    return (SchemeCompositionPolicy.drawsPreedit(rulesScheme) ||
      SchemeCompositionPolicy.drawsKeyGlyphs(rulesScheme)) &&
      editing.length > 0
      ? preedit
      : editing;
  }

  /** The caret within `reading`: the Engine's own in a spelling, the end of a composition the scheme keeps the caret at the end of. */
  static caret(rulesScheme: number, editingCaret: number, reading: string): number {
    return SchemeCompositionPolicy.drawsPreedit(rulesScheme) ? reading.length : editingCaret;
  }

  /** Whether the scheme's openable candidate list is showing, as the view's flag says; never inferred from a candidate count. */
  static listOpen(rulesScheme: number, candidateListOpen: boolean): boolean {
    return SchemeTraits.hasOpenableCandidateList(rulesScheme) && candidateListOpen;
  }
}
