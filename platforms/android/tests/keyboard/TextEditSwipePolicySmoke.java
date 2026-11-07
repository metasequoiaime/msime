import app.msime.android.TextEditSwipePolicy;

public final class TextEditSwipePolicySmoke {
    public static void main(String[] args) {
        check(TextEditSwipePolicy.swipedUp(24, 100, 100, 100, 75), "超过阈值的垂直上滑");
        check(!TextEditSwipePolicy.swipedUp(24, 100, 100, 100, 76), "阈值边界不接管");
        check(!TextEditSwipePolicy.swipedUp(24, 100, 100, 120, 75), "横向位移过大不接管");
        check(!TextEditSwipePolicy.swipedUp(24, 100, 100, 100, 125), "向下滑不接管");
        System.out.println("Text edit upward swipe policy passed");
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
