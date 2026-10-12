import { TextPolicy } from "../TextPolicy";
import { CandidateWrapPolicy } from "./CandidateWrapPolicy";

/** 网格里的一格：候选在完整列表里的下标，以及它实际占的列数（行尾那格可能被拉宽）。 */
export interface ExpandedCandidateCell {
  index: number;
  span: number;
}

/** 展开网格的行分配与实际 ArkUI 单元格共用的宽度。 */
export class ExpandedCandidateLayout {
  /** 候选在一行上所需的宽度：按字号计算的码点宽度加上单元格两侧内边距，上限为可见行宽，让一个很长的候选仍能独占一行放下。 */
  static width(
    text: string,
    fontSize: number,
    horizontalPadding: number,
    availableWidth: number,
  ): number {
    const font: number = Number.isFinite(fontSize) ? Math.max(1, fontSize) : 1;
    const padding: number = Number.isFinite(horizontalPadding) ? Math.max(0, horizontalPadding) : 0;
    const natural: number = TextPolicy.characters(text).length * font + padding * 2;
    if (!Number.isFinite(availableWidth) || availableWidth <= 0) {
      return Math.ceil(natural);
    }
    return Math.ceil(Math.min(natural, availableWidth));
  }

  /** 宽为 `width` 的单元格占几列网格：普通词占一列，长词按需占多列，但不超过一行的列数。 */
  static span(width: number, columnWidth: number, gap: number, columns: number): number {
    const count: number = Number.isFinite(columns) ? Math.max(1, Math.floor(columns)) : 1;
    if (!Number.isFinite(width) || !Number.isFinite(columnWidth) || columnWidth <= 0) {
      return 1;
    }
    const spacing: number = Number.isFinite(gap) ? Math.max(0, gap) : 0;
    return Math.max(1, Math.min(count, Math.ceil((width + spacing) / (columnWidth + spacing))));
  }

  /**
   * 把占 `spans` 列的格子按顺序装进 `columns` 列的各行：一行剩下的列放不下下一格时换行，行的分配与 `CandidateWrapPolicy.rows` 相同。换行前那一格拉宽到行尾，网格右边不留空；最后一行不拉宽，一两个候选不会被撑成一整条。iOS（`KeyboardCandidatePanelView.place`）与 Android（`CandidateGridPolicy.place`）是同一条规则。
   */
  static rows(indices: number[], spans: number[], columns: number): ExpandedCandidateCell[][] {
    if (indices.length !== spans.length) {
      throw new Error("Invalid expanded candidate spans");
    }
    const count: number = Number.isFinite(columns) ? Math.max(1, Math.floor(columns)) : 1;
    const assignment: number[] = CandidateWrapPolicy.rows(count, 0, spans);
    const rows: ExpandedCandidateCell[][] = [];
    let column: number = 0;
    for (let position: number = 0; position < spans.length; position++) {
      if (rows.length <= assignment[position]) {
        rows.push([]);
        column = 0;
      }
      const rowEnds: boolean =
        position + 1 < spans.length && assignment[position + 1] !== assignment[position];
      const cell: ExpandedCandidateCell = {
        index: indices[position],
        span: rowEnds ? Math.max(spans[position], count - column) : spans[position],
      };
      rows[assignment[position]].push(cell);
      column += spans[position];
    }
    return rows;
  }
}
