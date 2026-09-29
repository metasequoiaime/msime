import app.msime.android.KeyboardFormFactorPolicy;

public final class KeyboardFormFactorPolicySmoke {
    public static void main(String[] args) {
        check(!KeyboardFormFactorPolicy.expanded(599), "a phone stays compact");
        check(KeyboardFormFactorPolicy.expanded(600), "the Android large-screen boundary expands");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(411, 915) == 0,
            "rotating a phone does not turn it into a convertible");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(600, 600) == 600,
            "a narrow foldable uses the available large-screen width");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(720, 1280) == 720,
            "a two-in-one gets a centered bounded keyboard");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(720, 0) == 720,
            "an unknown expanded width has a safe bound");
        System.out.println("Android keyboard form factors: handset fill and bounded large-screen surface passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
