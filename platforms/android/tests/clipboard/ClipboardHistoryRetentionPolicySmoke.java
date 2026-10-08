import app.msime.android.ClipboardHistoryRetentionPolicy;
import app.msime.android.ClipboardHistoryRetentionPolicy.Source;

/** #5602：只有实时读到的偏好能改剪贴板历史开关、能触发清空；runtime-options 副本和读不到偏好都不动它。 */
public final class ClipboardHistoryRetentionPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 副本里的 clipboard_history 永远是出厂默认的关；按它改开关、清历史，就是每换一个输入框历史都被清掉的那个 bug。
        check(ClipboardHistoryRetentionPolicy.enabledAfter(true, Source.RUNTIME_OPTIONS_COPY, false),
            "the runtime-options copy must not switch an enabled history off");
        check(!ClipboardHistoryRetentionPolicy.clearsHistory(Source.RUNTIME_OPTIONS_COPY, false),
            "the runtime-options copy must never clear the history");
        check(!ClipboardHistoryRetentionPolicy.enabledAfter(false, Source.RUNTIME_OPTIONS_COPY, true),
            "the copy cannot switch the history on either; only the user's live choice can");

        // 读不到偏好不是「关」。
        check(ClipboardHistoryRetentionPolicy.enabledAfter(true, Source.LIVE, null),
            "unreadable live preferences keep the current switch");
        check(!ClipboardHistoryRetentionPolicy.clearsHistory(Source.LIVE, null),
            "unreadable live preferences never clear the history");

        // 实时偏好照常生效：用户在设置里关掉，下一次实时读到就清空。
        check(!ClipboardHistoryRetentionPolicy.enabledAfter(true, Source.LIVE, false),
            "a live 'off' switches the history off");
        check(ClipboardHistoryRetentionPolicy.clearsHistory(Source.LIVE, false),
            "a live 'off' clears the history, as the settings page promises");
        check(ClipboardHistoryRetentionPolicy.enabledAfter(false, Source.LIVE, true),
            "a live 'on' switches the history on");
        check(!ClipboardHistoryRetentionPolicy.clearsHistory(Source.LIVE, true),
            "a live 'on' keeps the history");
        System.out.println("ClipboardHistoryRetentionPolicySmoke ok");
    }
}
