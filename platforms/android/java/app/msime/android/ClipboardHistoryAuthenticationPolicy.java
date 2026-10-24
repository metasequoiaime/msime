package app.msime.android;

/** 关闭剪贴板历史会删除本机记录，只有从开到关的动作需要系统设备认证。 */
public final class ClipboardHistoryAuthenticationPolicy {
    private ClipboardHistoryAuthenticationPolicy() {}

    public static boolean requiresAuthentication(boolean currentlyEnabled, boolean nextEnabled) {
        return currentlyEnabled && !nextEnabled;
    }
}
