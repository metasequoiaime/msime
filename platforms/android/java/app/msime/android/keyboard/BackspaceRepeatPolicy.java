package app.msime.android;

/**
 * 删除键长按连删：按下立即删一次，按住满 420 ms 后开始连删，越删越快（#5585）——前 8 次每 70 ms 一次，接下来 12 次每 45 ms 一次，之后每 30 ms 一次。只想删一个字时短按，只删按下那一次。
 */
public final class BackspaceRepeatPolicy {
    /** 开始连删前的等待。 */
    public static final long INITIAL_DELAY_MS = 420;
    /** 第一档连删间隔。 */
    public static final long REPEAT_INTERVAL_MS = 70;
    /** 连删满这么多次后换成第二档间隔。 */
    public static final int FAST_AFTER_REPEATS = 8;
    /** 第二档连删间隔。 */
    public static final long FAST_INTERVAL_MS = 45;
    /** 连删满这么多次后换成第三档间隔。 */
    public static final int FASTEST_AFTER_REPEATS = 20;
    /** 第三档连删间隔。 */
    public static final long FASTEST_INTERVAL_MS = 30;

    /** 第 FAST_AFTER_REPEATS 次连删的时刻。 */
    private static final long FAST_START_MS = INITIAL_DELAY_MS + (FAST_AFTER_REPEATS - 1) * REPEAT_INTERVAL_MS;
    /** 第 FASTEST_AFTER_REPEATS 次连删的时刻。 */
    private static final long FASTEST_START_MS = FAST_START_MS
        + (FASTEST_AFTER_REPEATS - FAST_AFTER_REPEATS) * FAST_INTERVAL_MS;

    private BackspaceRepeatPolicy() { }

    /**
     * 已经连删了 `repeatsDone` 次（不含按下时那次）之后，到下一次连删的等待。
     *
     * @param repeatsDone 已完成的连删次数；小于 1 时按 1
     */
    public static long repeatInterval(int repeatsDone) {
        if (repeatsDone < FAST_AFTER_REPEATS) return REPEAT_INTERVAL_MS;
        if (repeatsDone < FASTEST_AFTER_REPEATS) return FAST_INTERVAL_MS;
        return FASTEST_INTERVAL_MS;
    }

    /**
     * 按住 `heldMs` 毫秒时累计应删除的次数（含按下时的一次）。
     *
     * @param heldMs 已按住的时长；负数按 0
     */
    public static int deletesAfter(long heldMs) {
        if (heldMs < INITIAL_DELAY_MS) return 1;
        long repeats;
        if (heldMs < FAST_START_MS) {
            repeats = 1 + (heldMs - INITIAL_DELAY_MS) / REPEAT_INTERVAL_MS;
        } else if (heldMs < FASTEST_START_MS) {
            repeats = FAST_AFTER_REPEATS + (heldMs - FAST_START_MS) / FAST_INTERVAL_MS;
        } else {
            repeats = FASTEST_AFTER_REPEATS + (heldMs - FASTEST_START_MS) / FASTEST_INTERVAL_MS;
        }
        return 1 + (int) BoundsPolicy.atMost(repeats, Integer.MAX_VALUE - 1L);
    }

    /** 第 `index` 次删除（从 0 起，0 为按下时那次）相对按下的时刻。 */
    public static long delayOf(int index) {
        if (index <= 0) return 0;
        if (index <= FAST_AFTER_REPEATS) return INITIAL_DELAY_MS + (long) (index - 1) * REPEAT_INTERVAL_MS;
        if (index <= FASTEST_AFTER_REPEATS)
            return FAST_START_MS + (long) (index - FAST_AFTER_REPEATS) * FAST_INTERVAL_MS;
        return FASTEST_START_MS + (long) (index - FASTEST_AFTER_REPEATS) * FASTEST_INTERVAL_MS;
    }

    /** 当前这次删除之后，下一次删除距现在的等待：`deletesSoFar` 含按下时那次，第一次后等 420 ms，之后按 {@link #repeatInterval}。 */
    public static long nextDelay(int deletesSoFar) {
        return deletesSoFar <= 1 ? INITIAL_DELAY_MS : repeatInterval(deletesSoFar - 1);
    }
}
