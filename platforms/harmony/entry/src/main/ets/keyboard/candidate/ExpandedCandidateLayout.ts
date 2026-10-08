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
    const natural: number = Array.from(text).length * font + padding * 2;
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
}
