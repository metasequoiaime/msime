/**
 * The brand mark that leads a 2in1 candidate window's composition line, as the macOS candidate window's top row leads with it (platforms/macos/src/input/InputController.mm): a small full-colour mark, a gap, then the reading.
 *
 * Only a candidate window carries it. A phone's composition line sits under the idle strip's logo button, which opens the function panel; a mark in the same corner that looked like that button and did nothing would be a button that does not work, and the line's width is better spent on the spelling. The window carries it wherever its composition line is shown; a 2in1 that turns the line off collapses it to nothing, pager included, and the mark goes with it.
 */
export class CandidateLogoPolicy {
  /** The mark's side, the macOS top row's 16pt. */
  static readonly SIDE_VP: number = 16;
  /** The gap between the mark and the reading. */
  static readonly GAP_VP: number = 6;

  /** Whether the composition line leads with the mark: a candidate window whose composition line is shown. */
  static visible(desktop: boolean, showPreedit: boolean): boolean {
    return desktop && showPreedit;
  }

  /** The room the mark and its gap take at the start of the line; zero where it is not drawn. */
  static slotWidthVp(visible: boolean): number {
    return visible ? CandidateLogoPolicy.SIDE_VP + CandidateLogoPolicy.GAP_VP : 0;
  }
}
