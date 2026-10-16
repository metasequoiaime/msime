package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 在原生宿主中验证全部设置、AI 二级页、软键盘和顶部返回。 */
public final class ChatNavigationDeviceSmoke extends DeviceSmoke {
    private static Predicate<AccessibilityNodeInfo> id(String name) {
        return node -> equalsText("app.msime.android:id/" + name, node.getViewIdResourceName());
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (find(window.getRoot(), match) != null) return true;
        }
        return false;
    }

    private void reveal(String text) {
        Predicate<AccessibilityNodeInfo> match = node -> equalsText(text, node.getText());
        for (int attempt = 0; attempt < 8 && !visible(match); attempt++) {
            await(id("keyboard_scroll")).performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(300);
        }
        await(match);
    }

    @Override protected String successDescription() {
        return "native all-settings tab, secondary AI chat, keyboard focus and top back navigation";
    }

    @Override protected void runChecks() throws Exception {
        android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        automation.setServiceInfo(info);
        stage = "native all-settings tab";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
        // 首次安装可能仍有原生引导页，先按真实用户路径跳过它。
        if (visible(id("onboarding_skip"))) tap(id("onboarding_skip"));
        await(id("tab_allsettings"));
        tap(id("tab_allsettings"));
        await(id("keyboard_title").and(node -> equalsText("全部设置", node.getText())));
        SystemClock.sleep(800);
        if (visible(id("keyboard_status_card")) || visible(id("keyboard_try"))) {
            throw new AssertionError("All settings must omit the home status card after refresh");
        }
        stage = "AI settings open secondary chat";
        reveal("AI 润色与回复");
        tap(node -> equalsText("AI 润色与回复", node.getText()));
        tap(node -> equalsText("AI 对话", node.getText()));
        await(id("tryout_title").and(node -> equalsText("AI 对话", node.getText())));
        await(id("tab_allsettings").and(AccessibilityNodeInfo::isSelected));
        stage = "chat has top back while keyboard focused";
        tap(id("tryout_field"));
        await(id("tryout_field").and(AccessibilityNodeInfo::isFocused));
        tap(id("tryout_back"));
        await(id("ms_detail_bar_title").and(node -> equalsText("AI 润色与回复", node.getText())));
        shellDiscard("input keyevent 4");
        awaitAny(id("keyboard_title").and(node -> equalsText("全部设置", node.getText())));
        stage = "home tryout uses secondary chat";
        tap(id("tab_settings"));
        tap(id("keyboard_try"));
        await(id("tryout_field").and(AccessibilityNodeInfo::isFocused));
        tap(id("tryout_back"));
        await(id("keyboard_title").and(node -> equalsText("设置", node.getText())));
        await(id("tab_settings").and(AccessibilityNodeInfo::isSelected));
    }
}
