/**
 * Estimates the width a desktop candidate panel needs before ArkUI has laid out its Text nodes.
 *
 * Harmony cannot synchronously measure a candidate row from the input-method ability, while the
 * ability must resize and place the panel before it is shown. The estimate intentionally favours
 * stable bounds over font-specific precision: wide code points use one em, Latin text uses a
 * half-em, and the result is clamped so a long provider annotation cannot cover the whole desktop.
 */
export interface CandidateWidthEntry {
  readonly text: string;
  readonly badge: string;
  readonly hint: string;
  readonly annotation: string;
  /** A Korean Hanja row's 훈음, which the vertical list draws beside the word; see CandidateGlossPolicy.hunEum. */
  readonly hunEum?: string;
}

export class CandidateWidthPolicy {
  static readonly MIN_WIDTH_VP: number = 280;
  static readonly MAX_WIDTH_VP: number = 720;
  private static readonly LATIN_EM: number = 0.58;
  private static readonly WIDE_EM: number = 1.0;
  /** The window's own padding and border around its content. */
  static readonly EXTRA_WIDTH_VP: number = 48;
  /** The right padding the view puts after a desktop ordinal. */
  static readonly ORDINAL_GAP_VP: number = 4;
  /** Room for the bar the view draws in front of the highlighted candidate, kept in every chip so the highlight moving never changes a width. */
  static readonly SELECTION_BAR_VP: number = 3;

  /** Approximate text extent without leaking platform font or layout dependencies into the host. */
  static textWidthVp(text: string, fontSize: number): number {
    if (fontSize <= 0) {
      throw new Error("Invalid candidate font size");
    }
    let ems: number = 0;
    for (const character of text) {
      const codePoint: number = character.codePointAt(0) ?? 0;
      ems += CandidateWidthPolicy.isWide(codePoint)
        ? CandidateWidthPolicy.WIDE_EM
        : CandidateWidthPolicy.LATIN_EM;
    }
    return ems * fontSize;
  }

  /** Return the clamped panel width for the current preedit and candidate page. */
  static widthVp(
    entries: CandidateWidthEntry[],
    editing: string,
    candidateFontSize: number,
    preeditFontSize: number,
    minWidthVp: number = CandidateWidthPolicy.MIN_WIDTH_VP,
    maxWidthVp: number = CandidateWidthPolicy.MAX_WIDTH_VP,
  ): number {
    if (
      candidateFontSize <= 0 ||
      preeditFontSize <= 0 ||
      minWidthVp < 0 ||
      maxWidthVp < minWidthVp
    ) {
      throw new Error("Invalid candidate width dimensions");
    }
    let contentWidth: number = CandidateWidthPolicy.textWidthVp(editing, preeditFontSize);
    for (const entry of entries) {
      const suffix = entry.badge + entry.hint + (entry.hunEum ?? "") + entry.annotation;
      const candidateWidth =
        CandidateWidthPolicy.textWidthVp(entry.text, candidateFontSize) +
        CandidateWidthPolicy.textWidthVp(suffix, Math.max(12, candidateFontSize - 8));
      contentWidth = Math.max(contentWidth, candidateWidth);
    }
    return CandidateWidthPolicy.clampWidth(contentWidth, minWidthVp, maxWidthVp);
  }

  /**
   * The width a horizontal candidate window needs to show the whole page side by side.
   *
   * The Windows candidate window measures the preedit and every candidate on the page laid out in a row and sizes itself to that, clamped to half the monitor (`candidate_presenter.cpp`), so all of the page is visible and no candidate a number key selects is off-screen. `widthVp` takes the widest single entry, which is right for a vertical list and leaves most of a row scrolled away.
   */
  static rowWidthVp(
    entries: CandidateWidthEntry[],
    editing: string,
    candidateFontSize: number,
    preeditFontSize: number,
    chipPaddingVp: number,
    ordinalFontSize: number,
    minWidthVp: number = CandidateWidthPolicy.MIN_WIDTH_VP,
    maxWidthVp: number = CandidateWidthPolicy.MAX_WIDTH_VP,
  ): number {
    if (
      candidateFontSize <= 0 ||
      preeditFontSize <= 0 ||
      chipPaddingVp < 0 ||
      ordinalFontSize < 0 ||
      minWidthVp < 0 ||
      maxWidthVp < minWidthVp
    ) {
      throw new Error("Invalid candidate width dimensions");
    }
    let row: number = 0;
    entries.forEach((entry: CandidateWidthEntry, index: number): void => {
      row += Math.ceil(
        CandidateWidthPolicy.chipContentVp(entry, index, candidateFontSize, ordinalFontSize) +
          chipPaddingVp * 2,
      );
    });
    const contentWidth: number = Math.max(
      row,
      CandidateWidthPolicy.textWidthVp(editing, preeditFontSize),
    );
    return CandidateWidthPolicy.clampWidth(contentWidth, minWidthVp, maxWidthVp);
  }

  /** What one desktop chip holds between its paddings: the selection bar's room, the ordinal and its gap when `ordinalFontSize` is above zero, the word, and the badge, hint and annotation after it. The view sizes its chips with this too, so the window and the chips in it agree. */
  static chipContentVp(
    entry: CandidateWidthEntry,
    index: number,
    candidateFontSize: number,
    ordinalFontSize: number,
  ): number {
    const suffix: string = entry.badge + entry.hint + entry.annotation;
    const ordinal: number =
      ordinalFontSize > 0
        ? CandidateWidthPolicy.textWidthVp(`${index + 1}`, ordinalFontSize) +
          CandidateWidthPolicy.ORDINAL_GAP_VP
        : 0;
    return (
      CandidateWidthPolicy.SELECTION_BAR_VP +
      ordinal +
      CandidateWidthPolicy.textWidthVp(entry.text, candidateFontSize) +
      (suffix.length > 0
        ? CandidateWidthPolicy.textWidthVp(suffix, Math.max(12, candidateFontSize - 8))
        : 0)
    );
  }

  private static clampWidth(contentWidth: number, minWidthVp: number, maxWidthVp: number): number {
    const boundedMinimum: number = Math.max(
      CandidateWidthPolicy.MIN_WIDTH_VP,
      Math.min(maxWidthVp, minWidthVp),
    );
    return Math.min(
      maxWidthVp,
      Math.max(boundedMinimum, Math.ceil(contentWidth + CandidateWidthPolicy.EXTRA_WIDTH_VP)),
    );
  }

  private static isWide(codePoint: number): boolean {
    return (
      (codePoint >= 0x1100 && codePoint <= 0x115f) ||
      (codePoint >= 0x2e80 && codePoint <= 0xa4cf) ||
      (codePoint >= 0xac00 && codePoint <= 0xd7ff) ||
      (codePoint >= 0xf900 && codePoint <= 0xfaff) ||
      (codePoint >= 0xfe10 && codePoint <= 0xfe6f) ||
      (codePoint >= 0xff01 && codePoint <= 0xff60) ||
      (codePoint >= 0x1f300 && codePoint <= 0x1faff)
    );
  }
}
