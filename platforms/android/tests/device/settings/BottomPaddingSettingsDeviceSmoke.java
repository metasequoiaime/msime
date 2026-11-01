package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 原生设置页只有一个「键盘底部高度」滑块（#10447）：范围 0–160 dp，拖到正值打开留白并记下高度，拖到 0 只关开关、保留高度，重开后照样显示；不再有独立的「底部留白」开关；保存成功后不重建页面，滑块节点保持不变。 */
public final class BottomPaddingSettingsDeviceSmoke extends DeviceSmoke {
    private static final String TITLE = "键盘底部高度";
    private static final String ENABLED = "platform.android.bottom_padding";
    private static final String HEIGHT = "platform.android.bottom_padding_height";
    /** MsSlider 建在 SeekBar 上，读屏的取值按格算：0–160 dp、每格 4 dp，共 40 格。 */
    private static final int STEP_DP = 4;

    @Override protected String successDescription() {
        return "native keyboard bottom height slider range, save, zero and reopen persistence";
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
            AccessibilityNodeInfo slider = heightSlider();
            check(slider.getRangeInfo() != null && slider.getRangeInfo().getMin() == 0
                && slider.getRangeInfo().getMax() == 160 / STEP_DP, "native slider has wrong bounds");
            dropStaleNodes();
            for (var window : automation.getWindows()) {
                check(find(window.getRoot(), described("底部留白")) == null,
                    "native keyboard settings still show a separate bottom padding switch");
            }
            stage = "commit eighty dp from native slider";
            setProgress(slider, 80 / STEP_DP);
            awaitSaved(file, true, 80);
            // 保存成功后不重建页面：读屏逐格调整时焦点留在同一个滑块节点上。
            SystemClock.sleep(500);
            check(slider.refresh() && Math.round(slider.getRangeInfo().getCurrent()) * STEP_DP == 80,
                "native settings page rebuilt the slider after a successful save");
            stage = "reopen native keyboard options";
            openSettings();
            check(Math.round(heightSlider().getRangeInfo().getCurrent()) * STEP_DP == 80,
                "native slider lost the saved value on reopen");
            stage = "commit zero from native slider";
            setProgress(heightSlider(), 0);
            awaitSaved(file, false, 80);
            stage = "reopen after zero";
            openSettings();
            check(Math.round(heightSlider().getRangeInfo().getCurrent()) == 0,
                "native slider does not show zero after turning the padding off");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity --ei msime_open_tab 0");
            if (original == null) Files.deleteIfExists(file.toPath());
            else publish(file, original);
        }
    }
    private static void setProgress(AccessibilityNodeInfo slider, float value) {
        Bundle arguments = new Bundle();
        arguments.putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, value);
        check(slider.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId(), arguments),
            "native slider did not accept a user adjustment");
    }
    /** 等设置文件里两个字段都到位；拖到 0 时高度应保留原值。 */
    private static void awaitSaved(File file, boolean enabled, int height) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (file.isFile()) {
                JSONObject settings = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8))
                    .getJSONObject("settings");
                if (settings.optBoolean(ENABLED, !enabled) == enabled && settings.optInt(HEIGHT, -1) == height) return;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("native slider did not persist " + ENABLED + "=" + enabled + ", " + HEIGHT + "=" + height);
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
    private AccessibilityNodeInfo heightSlider() {
        for (int step = 0; step < 20; step++) {
            dropStaleNodes();
            for (var window : automation.getWindows()) {
                AccessibilityNodeInfo found = find(window.getRoot(), described(TITLE)
                    .and(node -> node.getRangeInfo() != null));
                if (found != null) return found;
            }
            await(node -> equalsText("app.msime.android", node.getPackageName()) && node.isScrollable())
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(150);
        }
        throw new AssertionError("Native keyboard bottom height slider never became visible");
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
