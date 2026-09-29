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
}
