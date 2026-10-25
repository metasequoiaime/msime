package app.msime.android;

/** #9612：剪贴板敏感操作不能绕过系统认证。 */
public final class ClipboardOperationAuthenticationPolicySmoke {
    public static void main(String[] args) {
        if (!ClipboardOperationAuthenticationPolicy.requiresAuthentication()) {
            throw new AssertionError("clipboard operations must require device authentication");
        }
        if (!ClipboardOperationAuthenticationPolicy.shouldStartAuthentication(false)
                || ClipboardOperationAuthenticationPolicy.shouldStartAuthentication(true)) {
            throw new AssertionError("clipboard export must not start a second prompt while authentication is active");
        }
        System.out.println("ClipboardOperationAuthenticationPolicySmoke ok");
    }
}
