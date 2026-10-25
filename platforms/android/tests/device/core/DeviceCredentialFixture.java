package app.msime.android.test;

import android.app.KeyguardManager;
import android.app.UiAutomation;
import android.content.Context;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

/**
 * 专用 AVD 上的合成锁屏 PIN：剪贴板导出、搜索和清空要先通过系统认证，设备套件用它驱动真实的系统认证界面。
 *
 * <p>专用 AVD 默认没有锁屏凭据，套件先验证无凭据时产品拒绝操作，再设置这个 PIN 走认证成功的路径，结束时清除。clipboard-export.sh 和 smoke.sh 在套件退出后用同一个值兜底清除，避免失败的套件把锁屏留给后面的套件。
 */
final class DeviceCredentialFixture {
    static final String PIN = "2580";

    private DeviceCredentialFixture() {}

    static void set(Context context, UiAutomation automation) throws Exception {
        shell(automation, "locksettings set-pin " + PIN);
        await(() -> secure(context), "synthetic device credential was not set");
    }

    static void clear(Context context, UiAutomation automation) throws Exception {
        if (!secure(context)) return;
        shell(automation, "locksettings clear --old " + PIN);
        await(() -> !secure(context), "synthetic device credential was not cleared");
    }

    static boolean secure(Context context) {
        return context.getSystemService(KeyguardManager.class).isDeviceSecure();
    }

    /** 系统认证界面里的密码框；输入法窗口和被测应用自己的窗口都不算。 */
    static boolean promptVisible(UiAutomation automation) {
        return passwordField(automation) != null;
    }

    static void awaitPrompt(UiAutomation automation) {
        await(() -> promptVisible(automation), "system credential prompt did not appear");
    }

    /** 等系统认证界面出现，在它的密码框里输入合成 PIN 并确认，直到界面关闭。 */
    static void enter(UiAutomation automation) throws Exception {
        awaitPrompt(automation);
        AccessibilityNodeInfo field = passwordField(automation);
        // 认证界面打开时通常已聚焦密码框；未聚焦时先点一下，避免合成按键落到其他窗口。
        if (field != null && !field.isFocused()) field.performAction(AccessibilityNodeInfo.ACTION_CLICK);
        automation.waitForIdle(300, 3000);
        shell(automation, "input text " + PIN);
        shell(automation, "input keyevent 66");
        await(() -> !promptVisible(automation), "system credential prompt did not accept the synthetic PIN");
    }

    private static AccessibilityNodeInfo passwordField(UiAutomation automation) {
        if (Build.VERSION.SDK_INT >= 34) automation.clearCache();
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot());
            if (found != null) return found;
        }
        return null;
    }

    private static AccessibilityNodeInfo find(AccessibilityNodeInfo node) {
        if (node == null || (Build.VERSION.SDK_INT < 34 && !node.refresh())) return null;
        CharSequence owner = node.getPackageName();
        if (owner != null && owner.toString().startsWith("app.msime.android")) return null;
        if (node.isPassword() && node.isVisibleToUser()) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = find(node.getChild(index));
            if (found != null) return found;
        }
        return null;
    }

    private static void await(java.util.function.BooleanSupplier condition, String message) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            if (condition.getAsBoolean()) return;
            SystemClock.sleep(100);
        }
        throw new AssertionError(message);
    }

    private static void shell(UiAutomation automation, String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }
}
