package app.msime.android;

/** Android editor-action labels shared by return-key rendering and dispatch. */
public final class ReturnKeyAction {
    private static final int UNSPECIFIED = 0;
    private static final int NONE = 1;
    private static final int GO = 2;
    private static final int SEARCH = 3;
    private static final int SEND = 4;
    private static final int NEXT = 5;
    private static final int DONE = 6;
    private static final int PREVIOUS = 7;

    private ReturnKeyAction() {}

    public static boolean performsEditorAction(int action, boolean disabled) {
        if (disabled) return false;
        return switch (action) {
            case GO, SEARCH, SEND, NEXT, DONE, PREVIOUS -> true;
            default -> false;
        };
    }

    /** A handled composition consumes Return before any editor action or newline is considered. */
    public static boolean shouldPerformEditorAction(
            int action, boolean disabled, boolean compositionHandled) {
        return !compositionHandled && performsEditorAction(action, disabled);
    }

    public static String title(int action, boolean disabled) {
        if (!performsEditorAction(action, disabled)) return "换行";
        return switch (action) {
            case GO -> "前往";
            case SEARCH -> "搜索";
            case SEND -> "发送";
            case NEXT -> "下一项";
            case DONE -> "完成";
            case PREVIOUS -> "上一项";
            default -> "换行";
        };
    }
}
