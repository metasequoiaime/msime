/**
 * 展开候选网格分批建格子的规则，与安卓的 `CandidateGridBatchPolicy` 相同。
 *
 * 一个字母常有几百个候选（双拼只敲 J 有 606 个），一次把它们全建成 ArkUI 格子、逐个算宽度再排行，打开面板和面板里每次换候选都会明显卡顿。网格先建第一批，用户往下滚到离已建内容底边不到一屏时再追加下一批，所以打开的代价与候选总数无关。iOS 的 `UICollectionView` 本来就只建可见的格子。
 */
export class ExpandedCandidateBatchPolicy {
  /** 每批建的格子数。一屏网格约五行、每行四到六格，一批够填满两屏，打开时这一批之外不再多建。 */
  static readonly BATCH: number = 48;

  /** 这次建网格时先建多少格。同一代候选的重画（释义晚到）至少建回上次已建的数量，否则网格变短、滚动位置被截掉；换了一代从第一批开始。 */
  static initialCount(total: number, previouslyBuilt: number, sameGeneration: boolean): number {
    if (
      !Number.isInteger(total) ||
      !Number.isInteger(previouslyBuilt) ||
      total < 0 ||
      previouslyBuilt < 0
    ) {
      throw new Error("Invalid candidate grid counts");
    }
    const wanted: number = sameGeneration
      ? Math.max(previouslyBuilt, ExpandedCandidateBatchPolicy.BATCH)
      : ExpandedCandidateBatchPolicy.BATCH;
    return Math.min(wanted, total);
  }

  /** 追加一批后已建的格子数。 */
  static nextCount(built: number, total: number): number {
    if (
      !Number.isInteger(built) ||
      !Number.isInteger(total) ||
      built < 0 ||
      total < 0 ||
      built > total
    ) {
      throw new Error("Invalid candidate grid counts");
    }
    return Math.min(built + ExpandedCandidateBatchPolicy.BATCH, total);
  }

  /** 是否该追加下一批：还有没建的候选，并且视口底边再往下一屏就超过了已建网格的底边。`gridBottom` 是网格内容的高度；视口高度为 0（还没布局）时不追加。 */
  static shouldAppend(
    built: number,
    total: number,
    scrollY: number,
    viewportHeight: number,
    gridBottom: number,
  ): boolean {
    if (
      !Number.isInteger(built) ||
      !Number.isInteger(total) ||
      built < 0 ||
      total < 0 ||
      built > total ||
      !Number.isFinite(viewportHeight) ||
      viewportHeight < 0
    ) {
      throw new Error("Invalid candidate grid geometry");
    }
    if (built >= total || viewportHeight === 0) {
      return false;
    }
    return scrollY + 2 * viewportHeight >= gridBottom;
  }
}
