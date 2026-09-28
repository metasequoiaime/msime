import { KeyboardGeometry } from '../KeyboardGeometry';

/**
 * Where the caret sits in the composition being spelled.
 *
 * The source draws it — `.cursor` is a 1.5px bar in every candidate stylesheet — and this host did
 * not, because the shared view's `caret_position` was never declared on the ArkTS side and so was
 * never read. The keys that move it were already implemented: `Ctrl+Left`, `Ctrl+Right` and
 * `Ctrl+Backspace` edit one Engine segment at a time on a 2in1, and they were moving a caret
 * nobody could see. A segment editor with an invisible caret is not a hard feature to use; it is an
 * unusable one, because there is no way to tell what the next key will affect.
 *
 * `caret_position` is a byte offset into the Engine's ASCII editing text, so for this text it is
 * also the character index: the editing text is letters and apostrophes, never a multi-byte
 * scalar. That equivalence is why the split below can index the string directly, and it is worth
 * saying out loud because it would stop being true if the Engine ever put Chinese in this field.
 */

export interface PreeditSegments {
  readonly before: string;
  readonly after: string;
}

export class PreeditCaretPolicy {
  /**
   * The composition split at the caret.
   *
   * A position outside the text is clamped rather than refused. The view and the caret arrive in
   * the same message so they cannot normally disagree, but a clamp costs nothing and the
   * alternative — indexing past the end — is a caret that vanishes exactly when someone is editing.
   */
  static split(editing: string, caret: number): PreeditSegments {
    const text: string = editing ?? "";
    if (!Number.isFinite(caret)) {
      return { before: text, after: "" };
    }
    const position: number = KeyboardGeometry.bounded(Math.trunc(caret), 0, text.length);
    return { before: text.substring(0, position), after: text.substring(position) };
  }

  /**
   * How tall the caret is drawn, from the source's `height: 1.2em` on `.cursor`.
   *
   * Rounded to whole units, as ArkUI lays out in, and never smaller than one so a rounding-down
   * cannot make it disappear.
   */
  static height(preeditFontSize: number): number {
    if (!Number.isFinite(preeditFontSize) || preeditFontSize <= 0) {
      return 1;
    }
    return Math.max(1, Math.round(preeditFontSize * 1.2));
  }
}
