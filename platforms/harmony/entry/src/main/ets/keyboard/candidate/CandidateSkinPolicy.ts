/**
 * Row colours for the candidate window that no theme palette carries.
 *
 * The palette itself (surface, text, numbers, secondary text, the selected fill and its text, hover, border and whether a selection bar is drawn) comes from the resolved global theme; see GlobalTheme.candidateColors. What is left here is the colour for a pinned candidate and the rule for which of the palette's two text colours a row takes. A row's translations, comments and hints are the palette's `secondary` whether or not it is selected, so they need no rule here.
 */
export class CandidateSkinPolicy {
  /**
   * The colour a candidate pinned to this position is drawn in.
   *
   * The source marks it with one fixed blue of its own (`CandidateViewHtml` in
   * `server/src/window/candidate_view_model.h` wraps the item in `color:#379AD3`), deliberately not
   * the selected-row colour: being pinned and being the current selection are different states and
   * a user has to be able to tell them apart. Drawing both in the skin's accent, as this panel did,
   * made them identical on every skin whose accent is also its selection colour.
   */
  static readonly FIXED_POSITION_COLOR = "#379AD3";

  /** The text colour for a candidate row, given whether it is selected and whether it is pinned. */
  static rowTextColor(
    highlighted: boolean,
    fixedPosition: boolean,
    textColor: string,
    selectedTextColor: string,
  ): string {
    if (fixedPosition) return CandidateSkinPolicy.FIXED_POSITION_COLOR;
    return highlighted ? selectedTextColor : textColor;
  }
}
