import app.msime.android.SuperscriptShortcutPolicy;

public final class SuperscriptShortcutPolicySmoke {
    public static void main(String[] args) {
        check(SuperscriptShortcutPolicy.applies("!")
            && SuperscriptShortcutPolicy.applies("！"), "both punctuation faces use the shortcut");
        check(!SuperscriptShortcutPolicy.applies("?"), "question mark keeps its normal action");
        check(SuperscriptShortcutPolicy.swipedUp(100, 80), "an upward move past the threshold triggers");
        check(!SuperscriptShortcutPolicy.swipedUp(100, 90), "a short move does not trigger");
        check(SuperscriptShortcutPolicy.variants().equals(java.util.List.of("⁰", "₀")),
            "long press lists superscript before subscript");
        check(SuperscriptShortcutPolicy.ZERO.equals("0"), "swipe output is ordinary zero");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
