/**
 * Row allocation for the candidate flow, ported from
 * platforms/android/java/app/msime/client/CandidateWrapPolicy.java.
 *
 * Kept separate from the view so the decision is testable without laying anything out: given the
 * measured width of each candidate, it answers which row each one belongs to.
 */
export class CandidateWrapPolicy {
  static shouldWrap(
    occupiedWidth: number,
    childWidth: number,
    availableWidth: number,
    spacing: number,
  ): boolean {
    return occupiedWidth > 0 && occupiedWidth + spacing + childWidth > availableWidth;
  }

  static rows(availableWidth: number, spacing: number, childWidths: number[]): number[] {
    if (availableWidth < 0 || spacing < 0) {
      throw new Error("Invalid candidate wrap dimensions");
    }
    const rows: number[] = new Array<number>(childWidths.length);
    let row: number = 0;
    let occupied: number = 0;
    for (let index: number = 0; index < childWidths.length; index++) {
      const width: number = childWidths[index];
      if (width < 0) {
        throw new Error("Invalid candidate width");
      }
      if (CandidateWrapPolicy.shouldWrap(occupied, width, availableWidth, spacing)) {
        row++;
        occupied = width;
      } else {
        occupied = occupied === 0 ? width : occupied + spacing + width;
      }
      rows[index] = row;
    }
    return rows;
  }
}
