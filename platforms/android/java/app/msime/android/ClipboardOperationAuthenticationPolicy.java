package app.msime.android;

/** 搜索、导出和清空剪贴板历史都必须先通过一次系统设备认证。 */
public final class ClipboardOperationAuthenticationPolicy {
    private ClipboardOperationAuthenticationPolicy() {}

    public static boolean requiresAuthentication() { return true; }
}
