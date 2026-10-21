package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

/** 在真实编辑器里检查组合表情原样输入、分页尾项与最近项重启恢复。 */
public final class EmojiCombinationDeviceSmoke extends DeviceSmoke {
    private static final String SHORT = "8️⃣🅱️Q了";
    private static final String LONG = "🌑🌒🌓🌔🌕🌖🌗🌘🌑";
    @Override protected String successDescription() {
        return "combination category, full Unicode insertion, pagination tail, recent restart and ordinary emoji";
    }
    @Override protected void runChecks() throws Exception {
        openEditor();
        stage = "open emoji panel";
        physicalTap("表情");
        await(described("表情面板"));
        stage = "select combinations category";
        physicalTap("表情分类 组合表情");
        await(described("表情分类 组合表情").and(AccessibilityNodeInfo::isSelected));
        await(described("按键 表情 🌝🌝"));
        stage = "physical combination insertion";
        scrollToItem(SHORT);
        physicalTap("按键 表情 " + SHORT);
        await(field("msime-test-plain").and(node -> equalsText(SHORT, node.getText())));
        stage = "combination pagination tail";
        scrollToItem(LONG);
        physicalTap("按键 表情 " + LONG);
        await(field("msime-test-plain").and(node -> equalsText(SHORT + LONG, node.getText())));
        tap(described("返回键盘"));
        SystemClock.sleep(500);
        stage = "recent combinations survive restart";
        openEditor();
        physicalTap("表情");
        // 面板默认打开常用，直接显示先前选过的组合表情。
        await(described("表情分类 常用").and(AccessibilityNodeInfo::isSelected));
        await(described("按键 表情 " + LONG));
        physicalTap("按键 表情 " + SHORT);
        await(field("msime-test-plain").and(node -> equalsText(SHORT, node.getText())));
        stage = "ordinary emoji still inserts";
        physicalTap("表情分类 笑脸");
        await(described("表情网格；每行八个"));
        physicalTap("按键 表情 😀");
        await(field("msime-test-plain").and(node -> equalsText(SHORT + "😀", node.getText())));
        tap(described("返回键盘"));
    }
    private void openEditor() throws Exception {
        restartIme();
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        tap(field("msime-test-plain"));
        await(key("n"));
    }
    private void scrollToItem(String text) throws Exception {
        for (int step = 0; step < 100; step++) {
            dropStaleNodes();
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), described("按键 表情 " + text)) != null) return;
            await(described("表情网格；组合表情自动适配宽度"))
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(200);
        }
        throw new AssertionError("Combination pagination did not reach requested source item");
    }
    private void physicalTap(String description) throws Exception {
        Rect bounds = new Rect();
        awaitStableBounds(described(description)).getBoundsInScreen(bounds);
        shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
    }
}
