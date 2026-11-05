package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;

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
        s.tap(s.described("编辑拼写").and(node -> DeviceSmoke.equalsText(reading, node.getText())));
        s.await(s.described("拼写编辑面板"));
        s.await(s.described("放大拼写").and(node -> DeviceSmoke.equalsText(reading, node.getText())
            && node.getTextSelectionStart() == reading.length()));
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
