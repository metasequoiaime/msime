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
        return TextPolicy.hasText(text);
    }

    public static long timestampValue(Object raw) {
        long timestamp = NumberPolicy.strictLong(raw, 0);
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

    /**
     * 编辑一条历史的结果（#5971）。
     *
     * <p>{@link #SAVED} 和 {@link #MERGED} 都算存好了，后者是改成了另一条已有的文字、两条合并成了一条，要说出来，否则用户会以为少了一条。
     */
    public enum EditResult { SAVED, MERGED, NOT_FOUND, INVALID }

    /** 共享存储没改时给的 `reason`。 */
    private static final String REASON_NOT_FOUND = "not_found";

    /** 共享存储拒绝编辑的原因换成结果：旧条目不在了是 {@link EditResult#NOT_FOUND}，其余（`invalid` 或不认识的原因）都按新文字不能保存说，那是用户能改的。 */
    public static EditResult editRejection(String reason) {
        return REASON_NOT_FOUND.equals(reason) ? EditResult.NOT_FOUND : EditResult.INVALID;
    }

    /** 编辑之后对用户说的话。 */
    public static String editMessage(EditResult result) {
        if (result == null) throw new IllegalArgumentException("No clipboard edit result");
        return switch (result) {
            case SAVED -> "已保存";
            case MERGED -> "已保存，和已有的相同记录合并成了一条";
            case NOT_FOUND -> "这条记录已经不在剪贴板历史里了，可能已被删除或清空";
            case INVALID -> message(Rejection.TOO_LONG);
        };
    }

    /** 键盘打开应用里的编辑页时用的页面名（`PageId` 的枚举名）；键盘进程不能引用 `home/` 的类，所以写成字符串。 */
    public static final String EDIT_PAGE = "CLIPBOARD_EDIT";
    /** 编辑页参数里那一条的键：值是 {@link #editKey}，不是文字本身。 */
    public static final String EDIT_ENTRY_ARG = "entry";
    /**
     * boolean 参数：编辑页、剪贴板历史页（#5973）做完之后回到原来的应用（`moveTaskToBack`）。
     *
     * <p>键盘只在当前输入框属于别的应用时带上它（{@link #returnsToCaller}）。光看深链的外部标记不够：键盘的深链在用户正在水杉自己的输入框里打字时也走同一条路，这时回到「原来的应用」就是把水杉自己送到后台。
     */
    public static final String RETURN_TO_CALLER_ARG = "return_to_caller";

    /** 键盘打开编辑页或剪贴板历史页时要不要带 {@link #RETURN_TO_CALLER_ARG}：当前输入框属于别的应用才带。读不到输入框的包名时照样带，和这个参数出现之前的做法一样。 */
    public static boolean returnsToCaller(String editorPackage, String ownPackage) {
        return editorPackage == null || !editorPackage.equals(ownPackage);
    }

    /**
     * 键盘交给编辑页、用来认出要编辑哪一条的键：时间戳加文字的散列和长度。
     *
     * <p>不直接传文字：深链参数的字符串最长 256 个字符（{@link HostDeepLink#MAX_STRING_ARG}），剪贴板记录可以长得多；文字也不该进 Intent。编辑页按这个键在共享存储里找到那一条再显示。宿主的入口是 exported 的，别的应用也能发来这个参数，但它猜不出某一条的键，猜错了编辑页只会说这条已经不在了，保存仍要用户自己点。
     */
    public static String editKey(long timestamp, String text) {
        if (text == null) throw new IllegalArgumentException("No clipboard text");
        return BoundsPolicy.nonNegative(timestamp) + ":" + Integer.toHexString(text.hashCode()) + ":" + text.length();
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
