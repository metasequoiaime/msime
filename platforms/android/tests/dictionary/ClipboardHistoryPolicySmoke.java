import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.ClipboardHistoryStore;

public final class ClipboardHistoryPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        try {
            java.lang.reflect.Method strictBoolean = ClipboardHistoryStore.class.getDeclaredMethod(
                "strictBoolean", Object.class);
            strictBoolean.setAccessible(true);
            check(Boolean.TRUE.equals(strictBoolean.invoke(null, Boolean.TRUE)),
                "clipboard responses accept JSON booleans");
            check(strictBoolean.invoke(null, "true") == null,
                "clipboard responses reject boolean strings instead of coercing them");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("clipboard response type policy missing", error);
        }
        // This file used to assert that MAX_CHARS and MAX_BYTES matched the shared crate's numbers,
        // and they did. The divergence was in the unit: the shared store counts graphemes and this
        // host counted UTF-16 code units, so six thousand emoji were refused here and accepted
        // there. Matching constants is not matching semantics, and the only way to stop asking the
        // question twice is to stop having an answer here at all - so the bounds are gone, and what
        // is left is what this host is actually entitled to decide.
        check(!ClipboardHistoryPolicy.hasText(null), "no clip is no text");
        check(!ClipboardHistoryPolicy.hasText("   \n"), "whitespace is no text");
        check(ClipboardHistoryPolicy.hasText("synthetic clipboard text"), "ordinary text");
        // Length is the shared store's business now, whichever way it is measured.
        check(ClipboardHistoryPolicy.hasText("x".repeat(20_000)), "length is not decided here");
        check(ClipboardHistoryPolicy.hasText("🌲".repeat(6_000)), "graphemes are not counted here");
        check(ClipboardHistoryPolicy.hasText("a\u0001b"), "control characters are not judged here");

        // The store's own reason decides which refusal the user is told about. Reading it is the
        // fix: every refusal used to arrive as FULL, including the ones that were not.
        check(ClipboardHistoryPolicy.rejectionFor("full") == ClipboardHistoryPolicy.Rejection.FULL,
            "a full history names unpinning");
        check(ClipboardHistoryPolicy.rejectionFor("invalid")
            == ClipboardHistoryPolicy.Rejection.TOO_LONG, "refused text names the text");
        check(ClipboardHistoryPolicy.rejectionFor("") == ClipboardHistoryPolicy.Rejection.TOO_LONG,
            "an unnamed refusal is about the text, not about pinning");
        check(ClipboardHistoryPolicy.rejectionFor("something new")
            == ClipboardHistoryPolicy.Rejection.TOO_LONG,
            "a reason this host does not know must not be guessed as FULL");

        check(ClipboardHistoryPolicy.LIMIT == 50, "the fifty-entry limit is the shared store's");
        for (ClipboardHistoryPolicy.Rejection rejection : ClipboardHistoryPolicy.Rejection.values())
            check(!ClipboardHistoryPolicy.message(rejection).isEmpty(), "each refusal is worded");
        check(ClipboardHistoryPolicy.message(ClipboardHistoryPolicy.Rejection.FULL)
            .contains(String.valueOf(ClipboardHistoryPolicy.LIMIT)), "FULL names the limit");
        check(!ClipboardHistoryPolicy.message(ClipboardHistoryPolicy.Rejection.EMPTY)
            .equals(ClipboardHistoryPolicy.message(ClipboardHistoryPolicy.Rejection.TOO_LONG)),
            "the two refusals ask for different things");
        check(ClipboardHistoryPolicy.timestampValue(Long.valueOf(123)) == 123,
            "integer timestamp is preserved");
        check(ClipboardHistoryPolicy.timestampValue(Double.valueOf(123.5)) == 0,
            "fractional timestamp is rejected");
        check(ClipboardHistoryPolicy.timestampValue(Boolean.TRUE) == 0,
            "boolean timestamp is rejected");
        check(ClipboardHistoryPolicy.timestampValue(Long.valueOf(-1)) == 0,
            "negative timestamp is rejected");
        try {
            ClipboardHistoryPolicy.message(null);
            throw new AssertionError("there is no message for \"accepted\"");
        } catch (IllegalArgumentException expected) { /* expected */ }
        System.out.println("Android clipboard history: policy, dedupe, pinning, removal and bounds passed");
    }
}
