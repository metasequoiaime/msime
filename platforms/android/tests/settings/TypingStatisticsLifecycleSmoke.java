import app.msime.android.TypingStatisticsLifecyclePolicy;

public final class TypingStatisticsLifecycleSmoke {
    public static void main(String[] args) {
        check(TypingStatisticsLifecyclePolicy.acceptsFailure(7, 7),
            "the current input accepts its own statistics failure");
        check(!TypingStatisticsLifecyclePolicy.acceptsFailure(7, 8),
            "a previous input cannot report failure into the next input");
        System.out.println("Android typing statistics lifecycle: stale failures rejected");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
