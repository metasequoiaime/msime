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
        // 开着云候选时第二格是云占位行「☁」（来源 15）。运行时的「下一个」越过它（#10665），空格只数看得见的三行，第四次才回到首条。
        int[] counted = JapaneseSpacePolicy.countedSources(new int[] {0, 15, 0, 0});
        check(java.util.Arrays.equals(counted, new int[] {0, 0, 0}),
            "Space counts the rows the highlight can stop on, without the cloud placeholder");
        // 第一行是占位行时，firstSource 取第一条看得见的行：唯一看得见的是 Fallback，空格照常上屏。
        int[] fallback = JapaneseSpacePolicy.countedSources(new int[] {15, JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK});
        check(fallback.length == 1 && !JapaneseSpacePolicy.converts(fallback.length, fallback[0]),
            "A lone Fallback row behind the cloud placeholder still commits on Space");
        check(JapaneseSpacePolicy.countedSources(new int[] {15}).length == 0,
            "A page holding only the cloud placeholder has nothing to convert");
        // 没写来源的行（记作 -1）照常数进去；已退役的 AI 占位行（12）照旧画出来，也照常数。
        check(JapaneseSpacePolicy.countedSources(new int[] {-1, 12}).length == 2,
            "Only the cloud placeholder is left out");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
