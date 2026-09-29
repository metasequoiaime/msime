package app.msime.client.test;

import android.content.Intent;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** Device-only acceptance for the full-surface Apple-style tools panel. */
public final class MoreToolsDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "four-column tool and setting tiles, local-input subpanel and keyboard return";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "more tools focus";
        tap(field("msime-test-plain"));
        stage = "keyboard shortcut bar";
        await(shortcutBar());
        // The scheme entry shows the scheme in use in both its face and its description, so only
        // the prefix identifies it; the rest keep fixed faces.
        await(describedPrefix("输入方案"));
        for (String label : new String[] {"皮肤", "设置", "收起"})
            await(key(label));
        stage = "more tools open";
        tap(key("更多"));
        AccessibilityNodeInfo panel = await(toolPanel());
        stage = "more tools primary cards";
        Rect panelBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
        AccessibilityNodeInfo[] primaryCards = new AccessibilityNodeInfo[5];
        String[] primaryTitles = {"表情", "剪贴板历史", "AI 润色", "本地输入", "语音结果"};
        for (int index = 0; index < primaryTitles.length; index++) {
            String title = primaryTitles[index];
            AccessibilityNodeInfo card = await(tool(title));
            primaryCards[index] = card;
            Rect cardBounds = new Rect();
            card.getBoundsInScreen(cardBounds);
            if (cardBounds.width() <= panelBounds.width() / 6
                    || cardBounds.width() >= panelBounds.width() / 3)
                throw new AssertionError("Primary tool is not a four-column tile");
            int expectedHeight = Math.round(52 * getTargetContext()
                .getResources().getDisplayMetrics().density);
            if (Math.abs(cardBounds.height() - expectedHeight) > 2)
                throw new AssertionError("Primary card height mismatch");
        }
        assertSameRow(primaryCards[0], primaryCards[1], "first primary row");
        assertSameRow(primaryCards[0], primaryCards[3], "first primary row end");
        stage = "more tools settings cards";
        AccessibilityNodeInfo traditional = await(tool("繁体输出"));
        AccessibilityNodeInfo sound = await(tool("按键音"));
        AccessibilityNodeInfo haptics = await(tool("按键振动"));
        AccessibilityNodeInfo strength = await(tool("振动强度"));
        if (!validSettingState(traditional) || !validFeedbackState(sound)
                || !validFeedbackState(haptics) || !validStrengthCard(strength))
            throw new AssertionError("Settings state was not exposed");
        String originalSound = sound.getStateDescription().toString();
        String changedSound = "已开启".equals(originalSound) ? "已关闭" : "已开启";
        stage = "more tools sound update";
        tap(tool("按键音"));
        await(toolWithState("按键音", changedSound));
        tap(tool("按键音"));
        await(toolWithState("按键音", originalSound));
        stage = "more tools local input subpanel";
        tap(tool("本地输入"));
        await(tool("返回工具"));
        await(tool("Unicode 码点"));
        tap(tool("返回工具"));
        await(tool("表情"));
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
        CharSequence state = node.getStateDescription();
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state))
            && node.isSelected() == equalsText("已开启", state);
    }

    private boolean validSettingState(AccessibilityNodeInfo node) {
        CharSequence state = node.getStateDescription();
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state)
            || equalsText("不可用", state));
    }

    private boolean validStrengthCard(AccessibilityNodeInfo node) {
        CharSequence text = node.getText();
        return text != null && (text.toString().contains("轻")
            || text.toString().contains("中") || text.toString().contains("强"));
    }

    private Predicate<AccessibilityNodeInfo> shortcutBar() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("键盘快捷栏", node.getContentDescription());
    }

}
