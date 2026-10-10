import { CandidateWidthPolicy } from "./CandidateWidthPolicy";
import { KeyboardGeometry } from "../KeyboardGeometry";

/**
 * The page indicator at the end of a 2in1 candidate window's composition line: "current / total" and a previous and next arrow, as the Harmony design draws it (`pageInfo` and `pageBtn`).
 *
 * The Engine view already carries `page` (from zero) and `page_count`; this turns them into what the line shows and how much room it needs, so the panel is sized for it before ArkUI lays it out.
 */
export class CandidatePagerPolicy {
  /** The design's 13px indicator. */
  static readonly FONT_SIZE: number = 13;
  /** The horizontal padding on each side of an arrow, which is also its touch margin. */
  static readonly ARROW_PADDING_VP: number = 4;
  /** The gap between the composition and the indicator. */
  static readonly LEADING_GAP_VP: number = 8;
  static readonly PREVIOUS: string = "‹";
  static readonly NEXT: string = "›";

  /** Whether there is anything to page through: a single page has no indicator. */
  static visible(pageCount: number): boolean {
    return pageCount > 1;
  }

  /** "2 / 5" for the second of five pages; empty when there is only one. */
  static label(page: number, pageCount: number): string {
    if (!CandidatePagerPolicy.visible(pageCount)) {
      return "";
    }
    const current: number = KeyboardGeometry.bounded(page, 0, pageCount - 1) + 1;
    return `${current} / ${pageCount}`;
  }

  /** The screen-reader name of an arrow, which is otherwise a bare glyph. */
  static arrowLabel(next: boolean): string {
    return next ? "下一页" : "上一页";
  }

  /** The room the indicator and both arrows take, gap included; zero when it is not shown. */
  static widthVp(page: number, pageCount: number): number {
    if (!CandidatePagerPolicy.visible(pageCount)) {
      return 0;
    }
    const size: number = CandidatePagerPolicy.FONT_SIZE;
    const arrows: number =
      CandidateWidthPolicy.textWidthVp(
        CandidatePagerPolicy.PREVIOUS + CandidatePagerPolicy.NEXT,
        size,
      ) +
      CandidatePagerPolicy.ARROW_PADDING_VP * 4;
    return Math.ceil(
      CandidatePagerPolicy.LEADING_GAP_VP +
        CandidateWidthPolicy.textWidthVp(CandidatePagerPolicy.label(page, pageCount), size) +
        arrows,
    );
  }

  /** A panel width that also fits the composition line with the indicator after it, so a long spelling is not cut short to make room for it. `leadingVp` is what the line puts before the spelling (the brand mark and its gap, CandidateLogoPolicy). Never narrower than `baseWidthVp`, never wider than `maxWidthVp` unless the base already is. */
  static panelWidthVp(
    baseWidthVp: number,
    editing: string,
    preeditFontSize: number,
    page: number,
    pageCount: number,
    maxWidthVp: number = CandidateWidthPolicy.MAX_WIDTH_VP,
    leadingVp: number = 0,
  ): number {
    const pager: number = CandidatePagerPolicy.widthVp(page, pageCount);
    if (pager === 0 && leadingVp <= 0) {
      return baseWidthVp;
    }
    const line: number = Math.ceil(
      Math.max(0, leadingVp) +
        CandidateWidthPolicy.textWidthVp(editing, preeditFontSize) +
        pager +
        CandidateWidthPolicy.EXTRA_WIDTH_VP,
    );
    return Math.max(baseWidthVp, Math.min(maxWidthVp, line));
  }
}
