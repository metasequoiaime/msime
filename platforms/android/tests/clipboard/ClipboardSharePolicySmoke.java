import app.msime.android.ClipboardSharePolicy;

/** #8517：剪贴板操作行只分享用户选中的一条非空历史文本。 */
public final class ClipboardSharePolicySmoke {
    public static void main(String[] args) {
        check(ClipboardSharePolicy.ACTION.equals("分享"), "the action is labelled 分享");
        check(ClipboardSharePolicy.CHOOSER_TITLE.equals("分享剪贴板记录"), "the chooser names clipboard records");
        check(ClipboardSharePolicy.canShare("合成的分享文本\n第二行"), "ordinary clipboard text is shareable");
        check(ClipboardSharePolicy.canShare("  保留首尾空格  "), "share keeps the selected text unchanged");
        check(!ClipboardSharePolicy.canShare(null), "a missing entry is not shareable");
        check(!ClipboardSharePolicy.canShare(" \t\n "), "blank text is not shareable");
        System.out.println("ClipboardSharePolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
