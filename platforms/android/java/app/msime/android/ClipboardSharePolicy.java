package app.msime.android;

/** 剪贴板条目分享的界面文案和输入边界。 */
public final class ClipboardSharePolicy {
    public static final String ACTION = "分享";
    public static final String CHOOSER_TITLE = "分享剪贴板记录";

    private ClipboardSharePolicy() {}

    /** 只允许把历史存储认可的非空文本交给系统分享选择器。 */
    public static boolean canShare(String text) {
        return ClipboardHistoryPolicy.hasText(text);
    }
}
