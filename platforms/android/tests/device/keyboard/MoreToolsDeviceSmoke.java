package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 真机验收功能菜单的排列、状态、子页和返回键盘。 */
public final class MoreToolsDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "toolbar row, two-page four-column function panel, local-input subpanel and keyboard return";
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
        String[] firstPage = {"全角", "中文标点", "模糊音", "繁体输出", "手写", "本地输入", "AI 回复与润色", "设置",
            "单手模式", "浮动键盘", "无痕输入", "键盘高度"};
        // 面板打开时磁贴有一段展开动画（同一磁贴先后量到过 73、109 像素宽），等它停下再量。
        awaitStableBounds(tool(firstPage[0]));
        panel = await(toolPanel());
        Rect panelBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
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
        assertSameRow(tiles[8], tiles[11], "third row");
        for (String toggle : new String[] {"全角", "中文标点", "模糊音", "繁体输出"})
            if (!validSettingState(await(tool(toggle))))
                throw new AssertionError("Toggle state was not exposed: " + toggle);
        stage = "function panel second page";
        AccessibilityNodeInfo sound = showTool("按键音");
        AccessibilityNodeInfo haptics = await(tool("按键振动"));
        if (!validFeedbackState(sound) || !validFeedbackState(haptics))
            throw new AssertionError("Feedback state was not exposed");
        for (String title : new String[] {"反馈", "关于", "词库", "文本编辑"})
            await(tool(title));
        String originalSound = switchState(sound).toString();
        String changedSound = "已开启".equals(originalSound) ? "已关闭" : "已开启";
        stage = "more tools sound update";
        tap(tool("按键音"));
        await(toolWithState("按键音", changedSound));
        tap(tool("按键音"));
        await(toolWithState("按键音", originalSound));
        stage = "function panel strength";
        AccessibilityNodeInfo strength = showTool("振动强度");
        if (!validStrengthCard(strength)) throw new AssertionError("Strength tile lost its level");
        stage = "more tools local input subpanel";
        showTool("本地输入");
        tap(tool("本地输入"));
        await(tool("返回工具"));
        AccessibilityNodeInfo unicode = await(tool("Unicode 码点"));
        AccessibilityNodeInfo date = await(tool("日期时间"));
        AccessibilityNodeInfo initials = await(tool("超级简拼"));
        assertSameRow(unicode, initials, "local input first row");
        Rect unicodeBounds = new Rect();
        Rect dateBounds = new Rect();
        unicode.getBoundsInScreen(unicodeBounds);
        date.getBoundsInScreen(dateBounds);
        if (unicodeBounds.left >= dateBounds.left || unicodeBounds.right > dateBounds.left)
            throw new AssertionError("Local input entries overlap");
        Rect backBounds = new Rect();
        await(tool("返回工具")).getBoundsInScreen(backBounds);
        if (backBounds.width() >= unicodeBounds.width())
            throw new AssertionError("Subpage back action still occupies a full row");
        tap(tool("返回工具"));
        stage = "AI chooser entries and compact back";
        showTool("AI 回复与润色");
        tap(tool("AI 回复与润色"));
        AccessibilityNodeInfo reply = await(tool("生成高情商回复"));
        AccessibilityNodeInfo polish = await(tool("打开 AI 润色"));
        assertSameRow(reply, polish, "AI actions");
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
