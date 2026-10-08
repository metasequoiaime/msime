import app.msime.android.ClipboardCapturePolicy;
import app.msime.android.ClipboardCapturePolicy.Trigger;

/** #5605：打开面板时的补读只记还没处理过的那一条，清空或删掉的内容不会被系统剪贴板里剩下的那条带回来。 */
public final class ClipboardCapturePolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        String copied = ClipboardCapturePolicy.identity(1_791_389_562_434L, "synthetic copied text");
        check(copied.equals(ClipboardCapturePolicy.identity(1_791_389_562_434L, "synthetic copied text")),
            "the same clip has the same identity");
        check(!copied.contains("synthetic"), "the identity never carries the clipboard text");
        check(!copied.equals(ClipboardCapturePolicy.identity(1_791_389_570_000L, "synthetic copied text")),
            "copying the same text again later is a new clip");
        check(!copied.equals(ClipboardCapturePolicy.identity(1_791_389_562_434L, "other synthetic text")),
            "different text is a different clip");
        check(ClipboardCapturePolicy.identity(-5, "x").equals(ClipboardCapturePolicy.identity(0, "x")),
            "an unknown copy time falls back to the text alone");

        // 补读：清空历史后系统剪贴板里还是这一条，不能再记回来。
        check(!ClipboardCapturePolicy.captures(Trigger.PANEL_OPENED, copied, copied),
            "opening the panel does not bring back a clip already handled");
        check(ClipboardCapturePolicy.captures(Trigger.PANEL_OPENED, copied, null),
            "opening the panel records a clip copied while the keyboard was not running");
        check(ClipboardCapturePolicy.captures(Trigger.PANEL_OPENED, copied,
                ClipboardCapturePolicy.identity(1, "older")),
            "opening the panel records a clip newer than the one handled");

        // 用户刚复制：一律记，哪怕和上一次处理过的是同一条。
        check(ClipboardCapturePolicy.captures(Trigger.COPIED, copied, copied),
            "a fresh copy is always recorded");
        check(!ClipboardCapturePolicy.captures(null, copied, null), "no trigger records nothing");
        check(!ClipboardCapturePolicy.captures(Trigger.COPIED, null, null), "no clip records nothing");
        try {
            ClipboardCapturePolicy.identity(0, null);
            throw new AssertionError("a missing text has no identity");
        } catch (IllegalArgumentException expected) {
            // 没有文字就没有身份。
        }
        System.out.println("ClipboardCapturePolicySmoke ok");
    }
}
