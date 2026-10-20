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

export interface CandidateSkinLength {
  readonly value: number;
  readonly em: boolean;
}

export interface CandidateSkinGeometry {
  readonly itemGap: CandidateSkinLength | null;
  readonly itemPaddingRight: CandidateSkinLength | null;
  readonly itemMinWidth: CandidateSkinLength | null;
  readonly numberScale: number | null;
  readonly selectedBarWidth: CandidateSkinLength | null;
  readonly selectedBarHeightEm: number | null;
}

export interface CandidateResolvedLayout {
  readonly itemRadius: number | null;
  readonly borderWidth: number | null;
  readonly cornerRadius: number | null;
  readonly horizontal: CandidateSkinGeometry;
  readonly vertical: CandidateSkinGeometry;
  readonly fitContent: boolean;
}

/** 只读取共享层已合并的版式，长度在当前候选字号下换算；旧主题响应沿用原生默认。 */
export class CandidateLayoutPolicy {
  private static object(
    value: Object | null | undefined,
  ): Record<string, Object | null | undefined> | null {
    return value !== null &&
      value !== undefined &&
      typeof value === "object" &&
      !Array.isArray(value)
      ? (value as Record<string, Object | null | undefined>)
      : null;
  }

  private static number(value: Object | null | undefined, maximum: number): number | null {
    return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= maximum
      ? value
      : null;
  }

  private static readLength(value: Object | null | undefined): CandidateSkinLength | null {
    const raw = CandidateLayoutPolicy.object(value);
    if (raw === null) return null;
    const size = CandidateLayoutPolicy.number(raw["value"], 500);
    return size !== null && typeof raw["em"] === "boolean"
      ? { value: size, em: raw["em"] as boolean }
      : null;
  }

  private static geometry(value: Object | null | undefined): CandidateSkinGeometry {
    const raw = CandidateLayoutPolicy.object(value);
    return {
      itemGap: CandidateLayoutPolicy.readLength(raw?.["itemGap"]),
      itemPaddingRight: CandidateLayoutPolicy.readLength(raw?.["itemPaddingRight"]),
      itemMinWidth: CandidateLayoutPolicy.readLength(raw?.["itemMinWidth"]),
      numberScale: CandidateLayoutPolicy.number(raw?.["numberScale"], 2),
      selectedBarWidth: CandidateLayoutPolicy.readLength(raw?.["selectedBarWidth"]),
      selectedBarHeightEm: CandidateLayoutPolicy.number(raw?.["selectedBarHeightEm"], 10),
    };
  }

  static parse(value: Object | null | undefined): CandidateResolvedLayout | null {
    const raw = CandidateLayoutPolicy.object(value);
    if (raw === null) return null;
    return {
      itemRadius: CandidateLayoutPolicy.number(raw["itemCornerRadiusDip"], 32),
      borderWidth: CandidateLayoutPolicy.number(raw["borderWidthDip"], 10),
      cornerRadius: CandidateLayoutPolicy.number(raw["cornerRadiusDip"], 32),
      horizontal: CandidateLayoutPolicy.geometry(raw["horizontal"] ?? raw["geometry"]),
      vertical: CandidateLayoutPolicy.geometry(raw["vertical"] ?? raw["geometry"]),
      fitContent: raw["fitContent"] === true,
    };
  }

  static dip(
    length: CandidateSkinLength | null | undefined,
    fontSize: number,
    fallback: number,
  ): number {
    return length === null || length === undefined
      ? fallback
      : length.em
        ? Math.round(length.value * fontSize)
        : length.value;
  }

  static numberSize(
    geometry: CandidateSkinGeometry | null | undefined,
    fontSize: number,
    fallback: number,
  ): number {
    return geometry?.numberScale === null || geometry?.numberScale === undefined
      ? fallback
      : Math.round(fontSize * geometry.numberScale);
  }

  static gap(
    layout: CandidateResolvedLayout | null | undefined,
    vertical: boolean,
    fontSize: number,
  ): number {
    const geometry = vertical ? layout?.vertical : layout?.horizontal;
    if (geometry?.itemGap === null || geometry?.itemGap === undefined) return 0;
    return (
      CandidateLayoutPolicy.dip(geometry.itemGap, fontSize, 0) +
      (!vertical && !layout?.fitContent ? 6 : 0)
    );
  }

  static chipWidth(
    geometry: CandidateSkinGeometry | null | undefined,
    fontSize: number,
    content: number,
    padding: number,
  ): number {
    const right = CandidateLayoutPolicy.dip(geometry?.itemPaddingRight, fontSize, padding);
    return Math.max(
      CandidateLayoutPolicy.dip(geometry?.itemMinWidth, fontSize, 0),
      content + padding + right,
    );
  }
}
