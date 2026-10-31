package app.msime.android;

/** 回车长按的空闲换行规则；组字状态继续交给确认和译文动作。 */
public final class ReturnKeyHoldPolicy {
    private ReturnKeyHoldPolicy() {}

    public static boolean insertsNewline(boolean hasConnection, boolean compositionActive,
            boolean returnConfirms) {
        return hasConnection && !compositionActive && !returnConfirms;
    }
}
