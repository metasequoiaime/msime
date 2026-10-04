package app.msime.android;

/**
 * What to tell the user when the shared store refuses a clipboard entry.
 *
 * <p>This used to decide the refusal as well, with its own size arithmetic, and the two answers had
 * drifted apart in three ways. It counted UTF-16 code units where the shared store counts graphemes,
 * so six thousand emoji were refused here and accepted there. It had no control-character check at
 * all, so text the shared store refuses passed this and then failed remotely. And the remote refusal
 * was collapsed to a boolean at the call site, so both of those arrived at the user as 「50 条历史均
 * 已固定」, which is a third thing that was not true.
 *
 * <p>So the bounds are gone. {@code crates/client-core/src/clipboard.rs} owns them - trimmed
 * non-blank, forty thousand UTF-8 bytes, ten thousand graphemes, no control characters other than
 * newline, carriage return and tab - and the shared entry already names which one failed. What stays
 * here is the wording, which is this host's, and the one question the store cannot answer: whether
 * the system clipboard held any text to offer it in the first place.
 */
public final class ClipboardHistoryPolicy {
    public static final int LIMIT = 50;

    /**
     * Why one save was refused.
     *
     * <p>Apple names each of these to the user, because they ask for different things: shorten the
     * text, or unpin something. A single "could not save" leaves a full history looking broken.
     */
    public enum Rejection { EMPTY, TOO_LONG, FULL }

    /** The shared store's reason for refusing a capture. */
    private static final String REASON_INVALID = "invalid";
    private static final String REASON_FULL = "full";

    private ClipboardHistoryPolicy() {}

    /**
     * Whether there is text here at all to hand the shared store.
     *
     * <p>Deliberately only this. Length and character rules belong to the shared store, and asking
     * them twice is what let the two answers diverge.
     */
    public static boolean hasText(String text) {
        return text != null && !text.trim().isEmpty();
    }

    public static long timestampValue(Object raw) {
        long timestamp = KeyboardGeometry.strictLong(raw, 0);
        return timestamp < 0 ? 0 : timestamp;
    }

    /** The shared store's `reason` turned into the refusal this host words. */
    public static Rejection rejectionFor(String reason) {
        if (REASON_FULL.equals(reason)) return Rejection.FULL;
        if (REASON_INVALID.equals(reason)) return Rejection.TOO_LONG;
        // A reason this host does not recognise is still a refusal, and the text is the thing the
        // user can act on; naming the pinned-history case for it would be a guess.
        return Rejection.TOO_LONG;
    }

    /** What to tell the user, naming the action that would let the save succeed. */
    public static String message(Rejection rejection) {
        if (rejection == null) throw new IllegalArgumentException("No clipboard rejection");
        return switch (rejection) {
            // Android has no paste permission prompt, so the Apple wording drops that clause.
            case EMPTY -> "剪贴板中没有可保存的文本";
            case TOO_LONG -> "这段文本无法保存，请缩短或去掉其中的控制字符后重试";
            case FULL -> LIMIT + " 条历史均已固定，请先取消固定或删除一条";
        };
    }
}
