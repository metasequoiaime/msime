package app.msime.android;

/** 无释义时不预留空行；释义行、大字号和候选缩放仍有足够空间。 */
public final class CandidateStripHeightPolicySmoke {
    public static void main(String[] args) {
        check(CandidateStripHeightPolicy.heightDp(0, 100, 22) == 32);
        check(CandidateStripHeightPolicy.heightDp(1, 100, 22) == 42);
        check(CandidateStripHeightPolicy.heightDp(2, 100, 22) == 56);
        check(CandidateStripHeightPolicy.heightDp(0, 50, 11) == 16);
        check(CandidateStripHeightPolicy.heightDp(0, 200, 44) == 64);
        check(CandidateStripHeightPolicy.heightDp(0, 100, 40) >= 44);
        check(CandidateStripHeightPolicy.heightDp(0, 100, 40, 8) >= 48);
        check(CandidateStripHeightPolicy.heightDp(1, 100, 65) >= 69);
        check(CandidateStripHeightPolicy.heightDp(2, 50, 70) >= 72);
        check(CandidateStripHeightPolicy.heightDp(0, 100, Float.NaN) == 32);
        check(CandidateStripHeightPolicy.heightDp(0, 100, Float.POSITIVE_INFINITY) == 32);
        check(CandidateStripHeightPolicy.heightDp(0, 30, 22) == 32);
        System.out.println("Android compact candidate strip height: passed");
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("候选行高度不符合字形或释义预留规则");
    }
}
