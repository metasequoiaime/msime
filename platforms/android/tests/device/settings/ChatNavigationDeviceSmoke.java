package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 在原生宿主中验证设置列表有「皮肤」一行、底部第二格是社区（根页没有返回按钮，顶部是「皮肤 / 词库」两个分类）、AI 二级页、软键盘和顶部返回。 */
public final class ChatNavigationDeviceSmoke extends DeviceSmoke {
    private static Predicate<AccessibilityNodeInfo> id(String name) {
        return node -> equalsText("app.msime.android:id/" + name, node.getViewIdResourceName());
    }

    /** 设置列表里标题为 `title` 的一行。 */
    private static Predicate<AccessibilityNodeInfo> row(String title) {
        return id("row_title").and(node -> equalsText(title, node.getText()));
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

    private int left(String name) {
        Rect bounds = new Rect();
        await(id(name)).getBoundsInScreen(bounds);
        return bounds.left;
    }

    @Override protected String successDescription() {
        return "settings list with a skin row, native community tab root with skin and dictionary options, secondary AI chat, keyboard focus and top back navigation";
    }

    @Override protected void runChecks() throws Exception {
        android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        automation.setServiceInfo(info);
        stage = "native settings tab";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
        // 首次安装可能仍有原生引导页，先按真实用户路径跳过它。
        if (visible(id("onboarding_skip"))) tap(id("onboarding_skip"));
        await(id("tab_settings"));
        tap(id("tab_settings"));
        await(id("keyboard_title").and(node -> equalsText("设置", node.getText())));
        await(id("tab_settings").and(AccessibilityNodeInfo::isSelected));
        stage = "settings list has the skin row";
        AccessibilityNodeInfo rows = awaitAny(id("keyboard_rows"));
        if (findAny(rows, row("键盘")) == null) throw new AssertionError("The settings list must still list its rows");
        if (findAny(rows, row("皮肤")) == null) throw new AssertionError("The settings list must list the 皮肤 row");
        stage = "second bottom tab is the community page";
        if (!(left("tab_settings") < left("tab_community") && left("tab_community") < left("tab_statistics"))) {
            throw new AssertionError("The community tab must be the second bottom item");
        }
        await(id("tab_community").and(node -> equalsText("社区", node.getContentDescription())
            || findAny(node, n -> equalsText("社区", n.getText())) != null));
        tap(id("tab_community"));
        await(id("tab_community").and(AccessibilityNodeInfo::isSelected));
        await(id("ms_detail_bar_title").and(node -> equalsText("社区", node.getText())));
        await(node -> equalsText("皮肤", node.getText()));
        await(node -> equalsText("词库", node.getText()));
        // 皮肤在前且默认选中：只有皮肤列表有分类筛选条，第一个筛选项是「全部」。
        await(node -> equalsText("全部", node.getText()));
        SystemClock.sleep(500);
        if (visible(id("ms_detail_back"))) throw new AssertionError("The community tab root must not show a back button");
        if (visible(id("community_back"))) throw new AssertionError("The embedded community list must not show its own back button");
        stage = "community tab switches to dictionaries";
        tap(node -> equalsText("词库", node.getText()));
        await(id("community_search"));
        SystemClock.sleep(500);
        if (visible(node -> equalsText("全部", node.getText()))) {
            throw new AssertionError("The dictionary option must replace the skin list, whose category filter belongs to skins only");
        }
        stage = "back from the community tab returns to settings";
        shellDiscard("input keyevent 4");
        await(id("tab_settings").and(AccessibilityNodeInfo::isSelected));
        await(id("keyboard_title").and(node -> equalsText("设置", node.getText())));
        stage = "AI settings open secondary chat";
        reveal("AI 润色与回复");
        tap(node -> equalsText("AI 润色与回复", node.getText()));
        tap(node -> equalsText("AI 对话", node.getText()));
        await(id("tryout_title").and(node -> equalsText("AI 对话", node.getText())));
        await(id("tab_settings").and(AccessibilityNodeInfo::isSelected));
        stage = "chat has top back while keyboard focused";
        tap(id("tryout_field"));
        await(id("tryout_field").and(AccessibilityNodeInfo::isFocused));
        tap(id("tryout_back"));
        await(id("ms_detail_bar_title").and(node -> equalsText("AI 润色与回复", node.getText())));
        shellDiscard("input keyevent 4");
        awaitAny(id("keyboard_title").and(node -> equalsText("设置", node.getText())));
        stage = "home tryout uses secondary chat";
        tap(id("tab_settings"));
        tap(id("keyboard_try"));
        await(id("tryout_field").and(AccessibilityNodeInfo::isFocused));
        tap(id("tryout_back"));
        await(id("keyboard_title").and(node -> equalsText("设置", node.getText())));
        await(id("tab_settings").and(AccessibilityNodeInfo::isSelected));
    }
}
