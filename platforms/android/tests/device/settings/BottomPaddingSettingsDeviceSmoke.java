package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;

/** 原生设置页不再显示重复的底部留白开关和滑块。 */
public final class BottomPaddingSettingsDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "native keyboard settings omit redundant bottom padding controls";
    }
    @Override protected void runChecks() throws Exception {
        android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        automation.setServiceInfo(info);
        stage = "open native keyboard options";
        openSettings();
        dropStaleNodes();
        for (var window : automation.getWindows()) {
            AccessibilityNodeInfo root = window.getRoot();
            check(find(root, described("底部留白")) == null,
                "native keyboard settings still show the redundant bottom padding switch");
            check(find(root, described("底部留白高度")) == null,
                "native keyboard settings still show the redundant bottom padding slider");
        }
    }
    private void openSettings() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        // 全新安装先走开屏和引导；通过真实按钮跳过，再重发被引导遮住的页面请求。
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            dropStaleNodes();
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            AccessibilityNodeInfo skip = find(root,
                node -> "app.msime.android:id/onboarding_skip".equals(node.getViewIdResourceName()));
            if (skip != null) {
                check(skip.performAction(AccessibilityNodeInfo.ACTION_CLICK), "onboarding skip did not respond");
                SystemClock.sleep(500);
                break;
            }
            AccessibilityNodeInfo intro = find(root,
                node -> "app.msime.android:id/home_intro".equals(node.getViewIdResourceName()));
            if (intro != null && intro.isClickable()) intro.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            else if (find(root, node -> equalsText("app.msime.android", node.getPackageName())
                    && node.isScrollable()) != null) break;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        SystemClock.sleep(500);
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
