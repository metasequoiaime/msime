package app.msime.android;

public final class NativeHandlePolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void rejects(long value) {
        try {
            NativeHandlePolicy.requirePositive(value);
            throw new AssertionError("accepted invalid native handle " + value);
        } catch (IllegalArgumentException expected) {
            // expected
        }
    }

    public static void main(String[] args) {
        check(NativeHandlePolicy.requirePositive(1) == 1, "positive native handle changed");
        rejects(0);
        rejects(-1);
        check(NativeHandlePolicy.isOptional(0), "zero optional handle must be a no-op");
        check(NativeHandlePolicy.isOptional(1), "positive optional handle must be accepted");
        check(!NativeHandlePolicy.isOptional(-1), "negative optional handle must be rejected");
    }
}
