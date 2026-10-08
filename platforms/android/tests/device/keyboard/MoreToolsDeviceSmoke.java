package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** Device-only acceptance for the full-surface Apple-style tools panel. */
public final class MoreToolsDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "toolbar row, three-page four-column function panel, local-input subpanel and keyboard return";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "more tools focus";
        tap(field("msime-test-plain"));
        stage = "keyboard shortcut bar";
        await(shortcutBar());
        // The scheme entry shows the scheme in use in its description, so only the prefix identifies it; the skin and hide entries keep fixed faces, and the brand key is 更多.
        await(describedPrefix("输入方案"));
        for (String label : new String[] {"更多", "皮肤", "收起"})
            await(key(label));
        await(described("表情"));
        await(described("常用语"));
        stage = "more tools open";
        tap(key("更多"));
        AccessibilityNodeInfo panel = await(toolPanel());
        await(described("返回键盘"));
        stage = "function panel first page";
        Rect panelBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
        String[] firstPage = {"全角", "中文标点", "模糊音", "繁体输出", "手写", "词库", "键盘高度", "设置"};
        AccessibilityNodeInfo[] tiles = new AccessibilityNodeInfo[firstPage.length];
        int expectedHeight = Math.round(52 * getTargetContext()
            .getResources().getDisplayMetrics().density);
        for (int index = 0; index < firstPage.length; index++) {
            AccessibilityNodeInfo tile = await(tool(firstPage[index]));
            tiles[index] = tile;
            Rect bounds = new Rect();
            tile.getBoundsInScreen(bounds);
            if (bounds.width() <= panelBounds.width() / 6 || bounds.width() >= panelBounds.width() / 3)
                throw new AssertionError("Function panel tile is not one of four columns: " + firstPage[index]
                    + " tile " + bounds.toShortString() + " panel " + panelBounds.toShortString());
            if (Math.abs(bounds.height() - expectedHeight) > 2)
                throw new AssertionError("Function panel tile height mismatch: " + firstPage[index]
                    + " height " + bounds.height() + " expected " + expectedHeight);
        }
        assertSameRow(tiles[0], tiles[3], "first row");
        assertSameRow(tiles[4], tiles[7], "second row");
        for (String toggle : new String[] {"全角", "中文标点", "模糊音", "繁体输出"})
            if (!validSettingState(await(tool(toggle))))
                throw new AssertionError("Toggle state was not exposed: " + toggle);
        stage = "function panel second page";
        AccessibilityNodeInfo sound = showTool("按键音");
        AccessibilityNodeInfo haptics = await(tool("按键振动"));
        if (!validFeedbackState(sound) || !validFeedbackState(haptics))
            throw new AssertionError("Feedback state was not exposed");
        for (String title : new String[] {"单手模式", "隐私模式", "反馈", "关于", "AI 回复与润色", "本地输入"})
            await(tool(title));
        String originalSound = switchState(sound).toString();
        String changedSound = "已开启".equals(originalSound) ? "已关闭" : "已开启";
        stage = "more tools sound update";
        tap(tool("按键音"));
        await(toolWithState("按键音", changedSound));
        tap(tool("按键音"));
        await(toolWithState("按键音", originalSound));
        stage = "function panel third page";
        AccessibilityNodeInfo strength = showTool("振动强度");
        if (!validStrengthCard(strength)) throw new AssertionError("Strength tile lost its level");
        for (String title : new String[] {"语音结果", "表情", "剪贴板历史"})
            awaitAny(tool(title));
        stage = "more tools local input subpanel";
        tap(tool("本地输入"));
        await(tool("返回工具"));
        await(tool("Unicode 码点"));
        tap(tool("返回工具"));
        showTool("全角");
        stage = "more tools return";
        tap(tool("返回键盘"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    private void assertSameRow(AccessibilityNodeInfo first, AccessibilityNodeInfo second,
                               String description) {
        Rect firstBounds = new Rect();
        Rect secondBounds = new Rect();
        first.getBoundsInScreen(firstBounds);
        second.getBoundsInScreen(secondBounds);
        if (Math.abs(firstBounds.top - secondBounds.top) > 2)
            throw new AssertionError(description + " is not horizontal");
    }

    private boolean validFeedbackState(AccessibilityNodeInfo node) {
        CharSequence state = switchState(node);
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state))
            && node.isSelected() == equalsText("已开启", state);
    }

    private boolean validSettingState(AccessibilityNodeInfo node) {
        CharSequence state = switchState(node);
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state)
            || equalsText("不可用", state));
    }

    /** 磁贴文字是「振动强度 」加当前档：轻、中、强或系统（跟随系统）。「强度」里本来就有「强」，所以只看后面那一截。 */
    private boolean validStrengthCard(AccessibilityNodeInfo node) {
        CharSequence text = node.getText();
        if (text == null || !text.toString().startsWith("振动强度 ")) return false;
        String level = text.toString().substring("振动强度 ".length());
        return level.equals("轻") || level.equals("中") || level.equals("强") || level.equals("系统");
    }

    private Predicate<AccessibilityNodeInfo> shortcutBar() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("键盘快捷栏", node.getContentDescription());
    }

}
