package app.msime.android.test;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.graphics.Rect;
import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 从设置首页输入合成查询并点击真实结果，验证两类结果把目标交到正确分类和可见行。 */
final class SettingsSearchAnchorDeviceChecks {
    private final DeviceSmoke test;

    SettingsSearchAnchorDeviceChecks(DeviceSmoke test) { this.test = test; }
    private static Predicate<AccessibilityNodeInfo> id(String name) {
        return node -> DeviceSmoke.equalsText("app.msime.android:id/" + name, node.getViewIdResourceName());
    }

    void run() throws Exception {
        AccessibilityServiceInfo info = test.automation.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        test.automation.setServiceInfo(info);
        test.stage = "settings search home";
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
        AccessibilityNodeInfo ready = test.await(id("onboarding_skip").or(id("tab_settings")));
        if (id("onboarding_skip").test(ready)) test.tap(id("onboarding_skip"));
        test.tap(id("tab_settings"));

        // 「键盘」来自首页已有行；「使用帮助」来自额外可搜索页面，分别覆盖 applySearch 的两个分支。
        search("显示页码", "键盘");
        test.await(test.key("工具栏").and(AccessibilityNodeInfo::isChecked));
        Rect tabs = new Rect();
        test.await(id("tab_settings")).getBoundsInScreen(tabs);
        test.await(test.key("显示页码").and(node -> {
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            return !bounds.isEmpty() && bounds.top >= 0 && bounds.bottom <= tabs.top;
        }));

        test.stage = "settings search section result";
        test.tap(id("ms_detail_back"));
        search("更新后仍有旧问题", "使用帮助");
        test.await(test.key("问题").and(AccessibilityNodeInfo::isChecked));
        test.await(test.key("更新后仍有旧问题"));
        test.tap(id("ms_detail_back"));

        test.stage = "settings search named section";
        search("社区插件", "插件");
        test.await(test.key("社区插件").and(AccessibilityNodeInfo::isChecked));
        test.tap(id("ms_detail_back"));
    }

    private void search(String query, String page) throws Exception {
        test.stage = "settings search " + query;
        AccessibilityNodeInfo field = test.await(node -> DeviceSmoke.equalsText("app.msime.android", node.getPackageName())
            && DeviceSmoke.equalsText("android.widget.EditText", node.getClassName())
            && ancestor(node, id("keyboard_search")));
        Bundle arguments = new Bundle();
        arguments.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, query);
        if (!field.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments))
            throw new AssertionError("Settings search rejected the synthetic query");
        test.tap(node -> node.isClickable() && ancestor(node, id("keyboard_search_results"))
            && test.findAny(node, id("row_title").and(label -> DeviceSmoke.equalsText(page, label.getText()))) != null);
        test.await(id("ms_detail_bar_title").or(id("ms_detail_title"))
            .and(node -> DeviceSmoke.equalsText(page, node.getText())));
    }

    private static boolean ancestor(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityNodeInfo parent = node.getParent(); parent != null; parent = parent.getParent()) {
            if (match.test(parent)) return true;
        }
        return false;
    }
}
