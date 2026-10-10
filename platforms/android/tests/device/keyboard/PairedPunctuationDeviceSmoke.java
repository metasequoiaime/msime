package app.msime.android.test;

import android.content.Intent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/**
 * 成对标点补上后半个时光标留在两半之间（#6458）。
 *
 * <p>普通输入框照规矩处理 `commitText(closing, 0)`，用来确认原有行为不变；`msime-test-cursor-at-end` 不认第二个参数、总把光标放在新文字后面，重现 vivo「信息」里光标跑到括号外的情形。两边都从符号面板点（，再在面板里点 ） 跨过自动补上的后半个：跨过要靠选区回声认出补全那次写入，光标跑偏又被拉回时同样要认得出来。键盘标点键经 Engine 补全后写后半个用的是同一个 `commitClosingMark`；123 层的符号键按字面上屏、不补全，这里驱动不到 Engine 那条路。
 */
public final class PairedPunctuationDeviceSmoke extends DeviceSmoke {
    private static final String PLAIN = "msime-test-plain";
    private static final String CURSOR_AT_END = "msime-test-cursor-at-end";

    @Override protected String successDescription() {
        return "paired punctuation keeps the caret between the halves, including in an editor that ignores newCursorPosition";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        for (String editor : new String[] {PLAIN, CURSOR_AT_END}) {
            stage = editor + " focus";
            tap(field(editor));
            await(field(editor).and(AccessibilityNodeInfo::isFocused));
            await(key("n").and(AccessibilityNodeInfo::isClickable));

            stage = editor + " symbol panel pair";
            insertFromSymbolPanel("按键 符号 （，长按只输入这半个");
            await(editorState(editor, "（）", 1));

            stage = editor + " step over the closing half";
            insertFromSymbolPanel("按键 符号 ）");
            await(editorState(editor, "（）", 2));
        }
    }

    /** 长按 123 打开符号面板，切到「中文」，点 `description` 那个符号；点完面板自己关上。 */
    private void insertFromSymbolPanel(String description) throws Exception {
        AccessibilityNodeInfo layer = await(described("切换到数字和符号"));
        if (!layer.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Symbol panel hold failed");
        await(described("符号面板"));
        tap(describedPrefix("符号分类 中文"));
        await(described("符号分类 中文，已选中"));
        tap(described(description));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    /** 输入框的文字是 `text`，光标折叠在第 `caret` 个字符后面。 */
    private Predicate<AccessibilityNodeInfo> editorState(String editor, String text, int caret) {
        return field(editor).and(node -> equalsText(text, node.getText())
            && node.getTextSelectionStart() == caret && node.getTextSelectionEnd() == caret);
    }
}
