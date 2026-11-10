package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

/** 全拼 14 键、18 键的放大拼写条画读音，不画组码（#10591）：「你好」的组码是 `bugao`（18 键 `bihai`），条上是 `ni'hao`，点读音的字母按读音放光标。 */
final class GridSpellingEditorChecks {
    private GridSpellingEditorChecks() { }

    /**
     * 组字刚打完、读音行画着 {@code reading} 时调用。结束时放大条已经收起，组字还在，光标回到末尾。
     *
     * @param reading 读音行此刻画的读音（带引擎的音节分隔，例如 {@code ni'hao}）
     * @param keys 这串组字的组码（{@code bugao}），放大条上不能出现
     */
    static void run(DeviceSmoke s, String reading, String keys) throws Exception {
        s.stage = "grid spelling editor draws the reading, not the group codes";
        Rect keyboardBefore = settledImeBounds(s);
        s.tap(s.described("编辑拼写").and(node -> DeviceSmoke.equalsText(reading, node.getText())));
        s.await(s.described("拼写编辑面板"));
        s.await(s.described("放大拼写").and(node -> DeviceSmoke.equalsText(reading, node.getText())
            && node.getTextSelectionStart() == reading.length()));
        assertImeWindowStable(s, keyboardBefore, 600);
        if (DeviceSmoke.equalsText(keys, s.await(s.described("放大拼写")).getText()))
            throw new AssertionError("Spelling editor draws the group codes " + keys);
        int split = reading.indexOf('\'');
        if (split > 0) {
            // 点在第二个音节的首字母前：引擎光标落在第一个音节之后，网格组字的光标画在分隔后面（ni'|hao）。
            s.stage = "grid spelling editor places the caret by the reading";
            select(s.await(s.described("放大拼写")), split + 1);
            s.await(s.described("放大拼写").and(node -> DeviceSmoke.equalsText(reading, node.getText())
                && node.getTextSelectionStart() == split + 1));
        }
        s.stage = "grid spelling editor closes on done";
        s.tap(s.described("完成拼写编辑"));
        long deadline = SystemClock.uptimeMillis() + 5000;
        while (panelVisible(s)) {
            if (SystemClock.uptimeMillis() > deadline) throw new AssertionError("Done did not close the spelling editor");
            SystemClock.sleep(100);
        }
        assertImeWindowStable(s, keyboardBefore, 600);
    }

    /** 14/18 键放大拼写只显示覆盖层，不应让输入法窗口消失或改变边界。 */
    private static Rect settledImeBounds(DeviceSmoke s) {
        Rect previous = null;
        int stable = 0;
        long deadline = SystemClock.uptimeMillis() + 5000;
        while (SystemClock.uptimeMillis() < deadline) {
            Rect current = imeBounds(s);
            if (current != null && current.equals(previous)) stable++;
            else {
                previous = current;
                stable = current == null ? 0 : 1;
            }
            if (stable >= 4) return new Rect(previous);
            SystemClock.sleep(100);
        }
        throw new AssertionError("Input method window bounds did not settle");
    }

    private static void assertImeWindowStable(DeviceSmoke s, Rect expected, long durationMillis) {
        long deadline = SystemClock.uptimeMillis() + durationMillis;
        while (SystemClock.uptimeMillis() < deadline) {
            Rect current = imeBounds(s);
            if (current == null) throw new AssertionError("Input method window disappeared while editing grid spelling");
            if (!expected.equals(current)) {
                throw new AssertionError("Input method window moved while editing grid spelling: expected "
                    + expected + ", got " + current);
            }
            SystemClock.sleep(40);
        }
    }

    private static Rect imeBounds(DeviceSmoke s) {
        for (AccessibilityWindowInfo window : s.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect bounds = new Rect();
            window.getBoundsInScreen(bounds);
            return bounds;
        }
        return null;
    }

    private static void select(AccessibilityNodeInfo node, int position) {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, position);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, position);
        if (!node.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic caret selection rejected");
    }

    private static boolean panelVisible(DeviceSmoke s) {
        s.dropStaleNodes();
        for (var window : s.automation.getWindows()) {
            if (s.find(window.getRoot(), s.described("拼写编辑面板")) != null) return true;
        }
        return false;
    }
}
