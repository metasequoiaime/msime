package app.msime.android;

import android.view.WindowManager;

/**
 * 「防止截屏」（#10098）的切换规则与窗口标志。默认关闭；第一次打开不需要认证，打开过一次以后，再开或再关都要先通过系统认证（强生物识别或锁屏凭据，见 {@link SystemAuthenticators}），只有在系统设置里清除应用数据才能绕过。设备上没有可用的系统认证时一律不许切换：第一次打开也不行，否则打开之后就再也关不掉。
 */
public final class ScreenCaptureProtectionPolicy {
    private ScreenCaptureProtectionPolicy() {}

    /** 一次切换该怎么处理。 */
    public enum Decision {
        /** 下一值与当前值相同，什么也不做。 */
        UNCHANGED,
        /** 直接保存：只有从未打开过时的第一次打开。 */
        SAVE,
        /** 先通过系统认证再保存。 */
        AUTHENTICATE,
        /** 设备没有可用的系统认证，拒绝切换并保持原值。 */
        UNAVAILABLE,
    }

    /**
     * @param enabled 当前是否开着
     * @param enabledBefore 是否曾经打开过（本机设置里的标记，打开时写入，之后不随关闭清除）
     * @param next 用户要切换到的值
     * @param authenticationAvailable 系统认证此刻是否可用（`BiometricManager.canAuthenticate` 返回成功）
     */
    public static Decision decide(boolean enabled, boolean enabledBefore, boolean next,
            boolean authenticationAvailable) {
        if (next == enabled) return Decision.UNCHANGED;
        if (!authenticationAvailable) return Decision.UNAVAILABLE;
        // 开着却没有「打开过」的标记只可能来自被改过的文件；按打开过处理，关闭仍要认证。
        return enabledBefore || enabled ? Decision.AUTHENTICATE : Decision.SAVE;
    }

    /** 按开关算出窗口应有的标志：开着时带上 {@code FLAG_SECURE}，关着时去掉，其余位不动。 */
    public static int windowFlags(int flags, boolean protect) {
        return protect ? flags | WindowManager.LayoutParams.FLAG_SECURE
            : flags & ~WindowManager.LayoutParams.FLAG_SECURE;
    }
}
