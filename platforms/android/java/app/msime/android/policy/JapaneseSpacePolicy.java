package app.msime.android;

/**
 * Whether Japanese Space converts the composition or commits it.
 *
 * <p>Space normally starts a conversion and later presses step through the candidates. A lone Fallback row is the raw composition the Engine shows when there is nothing to convert (a bare Shift+R prefix, or romaji it cannot read); Windows commits it on the first Space, so Space takes the normal commit path instead.
 */
public final class JapaneseSpacePolicy {
    /** The Engine's `CandidateSource::Fallback`, as it appears in a view candidate's `source`. */
    public static final int CANDIDATE_SOURCE_FALLBACK = 9;

    private JapaneseSpacePolicy() { }

    /** True when Space should arm or step a conversion over {@code candidateCount} rows whose first row has {@code firstSource}. */
    public static boolean converts(int candidateCount, int firstSource) {
        if (candidateCount <= 0) return false;
        return !(candidateCount == 1 && firstSource == CANDIDATE_SOURCE_FALLBACK);
    }

    /**
     * 空格数的行：这一页去掉云占位行（{@link AiPendingCandidatePolicy#isVisible}）之后各行的来源，次序不变。候选条不画占位行，运行时的「下一个」也越过它（#10665）；按原始行数数，走到本页末尾时要多按一次才回到首条。行数取返回数组的长度，{@link #converts} 的 {@code firstSource} 取它的第一项。
     */
    public static int[] countedSources(int[] sources) {
        int[] counted = new int[sources.length];
        int count = 0;
        for (int source : sources) {
            if (AiPendingCandidatePolicy.isVisible(source)) counted[count++] = source;
        }
        return java.util.Arrays.copyOf(counted, count);
    }
}
