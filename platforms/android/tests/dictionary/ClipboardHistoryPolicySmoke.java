import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.JsonPolicy;

public final class ClipboardHistoryPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
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
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "clipboard text accepts JSON strings");
        check(JsonPolicy.strictString(Integer.valueOf(7)) == null,
            "clipboard text rejects numbers instead of coercing them");
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)),
            "clipboard pinning accepts JSON booleans");
        check(JsonPolicy.strictBoolean("true") == null,
            "clipboard pinning rejects strings instead of coercing them");
        try {
            ClipboardHistoryPolicy.message(null);
            throw new AssertionError("there is no message for \"accepted\"");
        } catch (IllegalArgumentException expected) { /* expected */ }

        // #5971：在应用里编辑一条历史。
        check(ClipboardHistoryPolicy.editRejection("not_found") == ClipboardHistoryPolicy.EditResult.NOT_FOUND,
            "an entry removed elsewhere is reported as gone");
        check(ClipboardHistoryPolicy.editRejection("invalid") == ClipboardHistoryPolicy.EditResult.INVALID,
            "invalid new text is reported as unsavable");
        check(ClipboardHistoryPolicy.editRejection("") == ClipboardHistoryPolicy.EditResult.INVALID,
            "an unknown refusal is still a refusal the user can act on");
        check("已保存".equals(ClipboardHistoryPolicy.editMessage(ClipboardHistoryPolicy.EditResult.SAVED)), "a saved edit");
        check(ClipboardHistoryPolicy.editMessage(ClipboardHistoryPolicy.EditResult.MERGED).contains("合并"),
            "a merge is named, so the missing entry is not a surprise");
        check(ClipboardHistoryPolicy.editMessage(ClipboardHistoryPolicy.EditResult.NOT_FOUND).contains("不在"),
            "a vanished entry is named rather than failing silently");
        check(ClipboardHistoryPolicy.editMessage(ClipboardHistoryPolicy.EditResult.INVALID)
                .equals(ClipboardHistoryPolicy.message(ClipboardHistoryPolicy.Rejection.TOO_LONG)),
            "unsavable text uses the capture wording");
        try {
            ClipboardHistoryPolicy.editMessage(null);
            throw new AssertionError("there is no message for no result");
        } catch (IllegalArgumentException expected) { /* expected */ }
        String key = ClipboardHistoryPolicy.editKey(1_760_000_000_000L, "synthetic entry");
        check(key.equals(ClipboardHistoryPolicy.editKey(1_760_000_000_000L, "synthetic entry")), "the key is stable");
        check(!key.contains("synthetic"), "the key carries no text");
        check(!key.equals(ClipboardHistoryPolicy.editKey(1_760_000_000_001L, "synthetic entry")), "the timestamp counts");
        check(!key.equals(ClipboardHistoryPolicy.editKey(1_760_000_000_000L, "synthetic entry.")), "the text counts");
        String longest = ClipboardHistoryPolicy.editKey(Long.MAX_VALUE, "家".repeat(10_000));
        check(longest.length() <= app.msime.android.HostDeepLink.MAX_STRING_ARG,
            "the key fits a deep link argument however long the entry is");
        check(app.msime.android.HostDeepLink.isAllowedArgKey(ClipboardHistoryPolicy.EDIT_ENTRY_ARG),
            "the argument name survives the deep link filter");
        check(app.msime.android.HostDeepLink.isAllowedArgKey(ClipboardHistoryPolicy.RETURN_TO_CALLER_ARG),
            "the return argument survives the deep link filter");
        // #5973：在别的应用的输入框里打开就回到那个应用；在水杉自己的输入框里打开时不能把水杉送到后台。
        check(ClipboardHistoryPolicy.returnsToCaller("com.example.synthetic", "app.msime.android"),
            "another app's field returns to that app");
        check(!ClipboardHistoryPolicy.returnsToCaller("app.msime.android", "app.msime.android"),
            "the app's own field stays in the app");
        check(ClipboardHistoryPolicy.returnsToCaller(null, "app.msime.android"),
            "an unknown field keeps the old return");
        check(ClipboardHistoryPolicy.EDIT_PAGE.equals(app.msime.android.HostDeepLink.pageName(ClipboardHistoryPolicy.EDIT_PAGE)),
            "the page name survives the deep link filter");
        System.out.println("Android clipboard history: policy, dedupe, pinning, removal and bounds passed");
    }
}
