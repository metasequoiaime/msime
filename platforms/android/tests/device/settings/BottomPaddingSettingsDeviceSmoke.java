package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 原生设置页滑块提交并重开后保留留白高度，不经过开发用的 WebView。 */
public final class BottomPaddingSettingsDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "native bottom padding slider range, save and reopen persistence";
    }
    @Override protected void runChecks() throws Exception {
        android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        automation.setServiceInfo(info);
        File file = new File(getTargetContext().getFilesDir(), "bootstrap/state/android-settings.json");
        byte[] original = file.exists() ? Files.readAllBytes(file.toPath()) : null;
        try {
            stage = "open native keyboard options";
            openSettings();
            AccessibilityNodeInfo slider = paddingSlider();
            check(slider.getRangeInfo() != null && slider.getRangeInfo().getMin() == 0
                && slider.getRangeInfo().getMax() == 160, "native slider has wrong bounds");
            stage = "commit eighty dp from native slider";
            Bundle value = new Bundle();
            value.putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, 80);
            check(slider.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId(), value),
                "native slider did not accept a user adjustment");
            long deadline = SystemClock.uptimeMillis() + 15000;
            boolean saved = false;
            do {
                if (file.isFile()) {
                    JSONObject document = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
                    if (document.getJSONObject("settings").optInt("platform.android.bottom_padding_height", -1) == 80) {
                        saved = true;
                        break;
                    }
                }
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < deadline);
            check(saved, "native slider did not persist its value");
            stage = "reopen native keyboard options";
            openSettings();
            check(Math.round(paddingSlider().getRangeInfo().getCurrent()) == 80,
                "native slider lost the saved value on reopen");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity --ei msime_open_tab 0");
            if (original == null) Files.deleteIfExists(file.toPath());
            else publish(file, original);
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
    private AccessibilityNodeInfo paddingSlider() {
        for (int step = 0; step < 20; step++) {
            dropStaleNodes();
            for (var window : automation.getWindows()) {
                AccessibilityNodeInfo found = find(window.getRoot(), described("底部留白高度")
                    .and(node -> node.getRangeInfo() != null));
                if (found != null) return found;
            }
            await(node -> equalsText("app.msime.android", node.getPackageName()) && node.isScrollable())
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(150);
        }
        throw new AssertionError("Native bottom padding slider never became visible");
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
