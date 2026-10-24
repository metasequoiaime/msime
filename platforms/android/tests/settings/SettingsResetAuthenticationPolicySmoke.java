package app.msime.android;

/** #9607：重置所有设置前必须通过系统认证。 */
public final class SettingsResetAuthenticationPolicySmoke {
    public static void main(String[] args) {
        if (!SettingsResetAuthenticationPolicy.requiresAuthentication()) {
            throw new AssertionError("settings reset must require device authentication");
        }
        System.out.println("SettingsResetAuthenticationPolicySmoke ok");
    }
}
