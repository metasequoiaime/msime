package app.msime.android;

/** 重置所有设置会改写本机配置，必须先通过系统设备认证。 */
public final class SettingsResetAuthenticationPolicy {
    private SettingsResetAuthenticationPolicy() {}

    public static boolean requiresAuthentication() { return true; }
}
