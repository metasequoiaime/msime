package app.msime.android;

/** 删除键长按连删：按下立即删一次，按住满 420 ms 后每 70 ms 再删一次。 */
public final class BackspaceRepeatPolicy {
    /** 开始连删前的等待。 */
    public static final long INITIAL_DELAY_MS = 420;
    /** 连删间隔。 */
    public static final long REPEAT_INTERVAL_MS = 70;

    private BackspaceRepeatPolicy() { }

    /**
     * 按住 `heldMs` 毫秒时累计应删除的次数（含按下时的一次）。
     *
     * @param heldMs 已按住的时长；负数按 0
     */
    public static int deletesAfter(long heldMs) {
        if (heldMs < INITIAL_DELAY_MS) return 1;
        return 2 + (int) BoundsPolicy.atMost(
            (heldMs - INITIAL_DELAY_MS) / REPEAT_INTERVAL_MS, Integer.MAX_VALUE - 2L);
    }

    /** 第 `index` 次删除（从 0 起，0 为按下时那次）相对按下的时刻。 */
    public static long delayOf(int index) {
        if (index <= 0) return 0;
        return INITIAL_DELAY_MS + (long) (index - 1) * REPEAT_INTERVAL_MS;
    }

    /** 当前这次删除之后，下一次删除距现在的等待：第一次后等 420 ms，之后每次 70 ms。 */
    public static long nextDelay(int deletesSoFar) {
        return deletesSoFar <= 1 ? INITIAL_DELAY_MS : REPEAT_INTERVAL_MS;
    }
}
