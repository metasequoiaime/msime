package app.msime.android;

public final class CandidateScalePolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    public static void main(String[] args) {
        check(CandidateScalePolicy.percent(50) == 50);
        check(CandidateScalePolicy.percent(200) == 200);
        check(CandidateScalePolicy.percent(49) == 100);
        check(CandidateScalePolicy.percent(201) == 100);
        check(CandidateScalePolicy.dimension(42, 100) == 42);
        check(CandidateScalePolicy.dimension(42, 150) == 63);
        check(CandidateScalePolicy.dimension(42, 50) == 21);
    }
}
