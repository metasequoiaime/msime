package app.msime.android;

/** 搜索、导出和清空剪贴板历史都必须先通过一次系统设备认证。 */
public final class ClipboardOperationAuthenticationPolicy {
    private ClipboardOperationAuthenticationPolicy() {}

    public static boolean requiresAuthentication() { return true; }

    /** 认证窗口已显示时，重复点击不应再启动一条认证流程。 */
    public static boolean shouldStartAuthentication(boolean authenticationActive) {
        return !authenticationActive;
    }
}
