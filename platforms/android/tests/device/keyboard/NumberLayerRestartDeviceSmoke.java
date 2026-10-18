package app.msime.android.test;

import android.view.accessibility.AccessibilityNodeInfo;

/** 从真实系统输入连接验收连续数字、退格和切换输入框的键层边界。 */
public final class NumberLayerRestartDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "number layer survives same-editor restart and resets in another editor";
    }

    @Override protected void runChecks() throws Exception {
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.RestartingEditorActivity");
        tap(field("msime-restart-editor"));
        await(key("n"));
        tap(key("123"));
        String expected = "";
        for (String digit : new String[] {"1", "2", "3", "4", "5", "6"}) {
            stage = "digit retains number layer after editor restart " + digit;
            int previous = count();
            tap(key(digit));
            expected += digit;
            String text = expected;
            await(field("msime-restart-editor").and(node -> equalsText(text, node.getText())));
            await(field("msime-restart-count").and(node -> Integer.parseInt(node.getText().toString()) > previous));
            await(key("1"));
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), key("n").and(AccessibilityNodeInfo::isVisibleToUser)) != null)
                    throw new AssertionError("Restart returned to letter layer");
        }
        stage = "backspace retains number layer after editor restart";
        int previous = count();
        tap(key("⌫"));
        await(field("msime-restart-editor").and(node -> equalsText("12345", node.getText())));
        await(field("msime-restart-count").and(node -> Integer.parseInt(node.getText().toString()) > previous));
        await(key("1"));
        stage = "another editor resets the manually selected layer";
        tap(field("msime-restart-other"));
        await(key("n"));
    }

    private int count() {
        return Integer.parseInt(await(field("msime-restart-count")).getText().toString());
    }
}
