import app.msime.android.JapaneseSpacePolicy;

public final class JapaneseSpacePolicySmoke {
    public static void main(String[] args) {
        check(!JapaneseSpacePolicy.converts(1, JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK),
            "A lone Fallback row commits on Space instead of converting");
        check(JapaneseSpacePolicy.converts(1, 0),
            "A lone real candidate still converts");
        check(JapaneseSpacePolicy.converts(2, JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK),
            "Several rows still convert even when the first is Fallback");
        check(!JapaneseSpacePolicy.converts(0, -1),
            "No candidates leaves Space to its normal meaning");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
