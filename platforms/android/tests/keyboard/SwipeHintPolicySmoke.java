import app.msime.android.SwipeHintPolicy;

public final class SwipeHintPolicySmoke {
    public static void main(String[] args) {
        String down = SwipeHintPolicy.DOWN;
        String up = SwipeHintPolicy.UP;
        check(!SwipeHintPolicy.swiped(down, 10, 24), "exactly 14 dp is not a swipe");
        check(SwipeHintPolicy.swiped(down, 10, 24.5f), "past 14 dp is a swipe");
        check(!SwipeHintPolicy.swiped(down, 30, 10), "an upward drag is not a swipe down");
        check(!SwipeHintPolicy.swiped(up, 24, 10), "exactly 14 dp up is not a swipe");
        check(SwipeHintPolicy.swiped(up, 24.5f, 10), "past 14 dp up is a swipe up");
        check(!SwipeHintPolicy.swiped(up, 10, 30), "a downward drag is not a swipe up");
        check(SwipeHintPolicy.swiped("bogus", 10, 30), "an unknown direction reads as down");
        check("5".equals(SwipeHintPolicy.output("t", true, down, 0, 20)), "t swiped down types 5");
        check("5".equals(SwipeHintPolicy.output("t", true, up, 20, 0)), "t swiped up types 5");
        check("t".equals(SwipeHintPolicy.output("t", true, up, 0, 20)),
            "swiping down with the up setting types the letter");
        check("t".equals(SwipeHintPolicy.output("t", true, down, 0, 10)), "a tap types the letter");
        check("t".equals(SwipeHintPolicy.output("t", false, down, 0, 20)),
            "with swipes off the swipe types the letter");
        check("，".equals(SwipeHintPolicy.output("，", true, down, 0, 20)),
            "a key without a hint types itself");
        System.out.println("Android swipe hint: >14 dp threshold in either direction and hint output passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
