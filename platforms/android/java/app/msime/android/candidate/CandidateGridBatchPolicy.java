package app.msime.android;

/**
 * 完整候选网格分批建格子的规则，安卓宿主的 `ImeCandidateGrid` 与 JVM 冒烟共用。
 *
 * <p>一个字母常有几百个候选（#6471：双拼只敲 J 有 606 个），一次把它们全建成按钮再量一遍字宽，打开面板要卡上一秒。网格先建第一批，用户往下滚到离已建内容底边不到一屏时再追加下一批，所以打开的代价与候选总数无关。
 */
public final class CandidateGridBatchPolicy {
    /** 每批建的格子数。一屏网格约五行、每行四到六格，一批够填满两屏，打开时这一批之外不再多建。 */
    public static final int BATCH = 48;

    private CandidateGridBatchPolicy() {}

    /**
     * 这次建网格时先建多少格。同一代候选的重画（释义晚到、换皮肤）至少建回上次已建的数量，否则网格变短、滚动位置被截掉；换了一代从第一批开始。
     */
    public static int initialCount(int total, int previouslyBuilt, boolean sameGeneration) {
        if (total < 0 || previouslyBuilt < 0)
            throw new IllegalArgumentException("Invalid candidate grid counts");
        int wanted = sameGeneration ? BoundsPolicy.atLeast(previouslyBuilt, BATCH) : BATCH;
        return BoundsPolicy.atMost(wanted, total);
    }

    /** 追加一批后已建的格子数。 */
    public static int nextCount(int built, int total) {
        if (built < 0 || total < 0 || built > total)
            throw new IllegalArgumentException("Invalid candidate grid counts");
        return (int) BoundsPolicy.atMost((long) built + BATCH, (long) total);
    }

    /**
     * 是否该追加下一批：还有没建的候选，并且视口底边再往下一屏就超过了已建网格的底边。`gridBottom` 是网格底边在滚动内容里的坐标；视口高度为 0（还没布局）时不追加。
     */
    public static boolean shouldAppend(int built, int total, int scrollY, int viewportHeight,
                                       int gridBottom) {
        if (built < 0 || total < 0 || built > total || viewportHeight < 0)
            throw new IllegalArgumentException("Invalid candidate grid geometry");
        if (built >= total || viewportHeight == 0) return false;
        return (long) scrollY + 2L * viewportHeight >= gridBottom;
    }
}
