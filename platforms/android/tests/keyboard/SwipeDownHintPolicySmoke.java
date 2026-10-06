import app.msime.android.SwipeDownHintPolicy;

public final class SwipeDownHintPolicySmoke {
    public static void main(String[] args) {
        check(!SwipeDownHintPolicy.swiped(10, 24), "exactly 14 dp is not a swipe");
        check(SwipeDownHintPolicy.swiped(10, 24.5f), "past 14 dp is a swipe");
        check(!SwipeDownHintPolicy.swiped(30, 10), "an upward drag is not a swipe");
        check("5".equals(SwipeDownHintPolicy.output("t", true, 0, 20)), "t swiped types 5");
        check("t".equals(SwipeDownHintPolicy.output("t", true, 0, 10)), "a tap types the letter");
        check("t".equals(SwipeDownHintPolicy.output("t", false, 0, 20)),
            "with hints off the swipe types the letter");
        check("，".equals(SwipeDownHintPolicy.output("，", true, 0, 20)),
            "a key without a hint types itself");
        System.out.println("Android swipe-down hint: >14 dp threshold and hint output passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
