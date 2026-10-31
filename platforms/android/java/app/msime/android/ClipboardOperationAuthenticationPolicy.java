package app.msime.android;

/** 搜索、导出和清空剪贴板历史都必须先通过一次系统设备认证。 */
public final class ClipboardOperationAuthenticationPolicy {
    private ClipboardOperationAuthenticationPolicy() {}

    /** 认证桥 `ClipboardAuthenticationActivity` 回给输入法服务的结果码：认证通过。放在这里而不是 Activity 上，输入法核心不必为了一个常量依赖 Activity 类。 */
    public static final int RESULT_AUTHENTICATED = 2001;
    /** 认证桥的结果码：设备此刻没有可用的系统认证（没设锁屏凭据），调用方据此提示先去系统设置开启锁屏，而不是说「未通过系统认证」（#10408）。 */
    public static final int RESULT_UNAVAILABLE = 2002;

    public static boolean requiresAuthentication() { return true; }

    /** 认证窗口已显示时，重复点击不应再启动一条认证流程。 */
    public static boolean shouldStartAuthentication(boolean authenticationActive) {
        return !authenticationActive;
    }
}
