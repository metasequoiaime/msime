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
        // 设置页滑块与 client-core 校验的 50–200 一致，步长 5 能停在默认的 100%。
        check(CandidateScalePolicy.MIN_PERCENT == 50 && CandidateScalePolicy.MAX_PERCENT == 200);
        check((CandidateScalePolicy.DEFAULT_PERCENT - CandidateScalePolicy.MIN_PERCENT) % CandidateScalePolicy.STEP_PERCENT == 0);
    }
}
