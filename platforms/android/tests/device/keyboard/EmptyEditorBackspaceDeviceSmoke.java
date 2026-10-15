package app.msime.android.test;

import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 合成引用由编辑器处理；组字、普通文字、代理对与选区仍只删一次。 */
public final class EmptyEditorBackspaceDeviceSmoke extends DeviceSmoke {
    private static final String EDITOR = "msime-test-quote-editor";
    private String product;

    @Override public void onCreate(Bundle args) {
        product = args == null ? "app.msime.android" : args.getString("product", "app.msime.android");
        super.onCreate(args);
    }

    @Override protected String successDescription() {
        return "empty editor forwards backspace while composition, selection and code point deletion remain intact";
    }

    @Override protected void runChecks() throws Exception {
        if (!product.equals("app.msime.android") && !product.equals("app.msime.android.pinyin"))
            throw new AssertionError("Unknown test product");
        String ime = product.equals("app.msime.android") ? "app.msime.android/.MSIMEInputService"
            : "app.msime.android.pinyin/app.msime.android.MSIMEInputService";
        restartIme(ime);

        openEditor(false);
        stage = "initial empty editor removes synthetic mention";
        tap(key("⌫"));
        await(status(false, true, 1));
        stage = "initial empty editor removes synthetic quote";
        tap(key("⌫"));
        await(status(false, false, 2));

        openEditor(false);
        stage = "composition backspace";
        tap(key("n"));
        tap(key("i"));
        stage = "composition input ni";
        await(editorText("ni"));
        tap(key("⌫"));
        stage = "composition delete i";
        await(editorText("n"));
        tap(key("⌫"));
        stage = "composition delete n";
        await(editorText(""));
        stage = "composition retains synthetic quote";
        await(status(true, true, 0));
        stage = "empty editor removes synthetic mention";
        tap(key("⌫"));
        await(status(false, true, 1));
        stage = "empty editor removes synthetic quote";
        tap(key("⌫"));
        await(status(false, false, 2));

        openEditor(false);
        stage = "code point backspace preserves surrogate pair";
        setText("甲🙂");
        tap(key("⌫"));
        await(editorText("甲"));
        await(status(true, true, 0));
        tap(key("⌫"));
        await(editorText(""));
        await(status(true, true, 0));

        stage = "selection deletion does not touch preceding text or quote";
        setText("前合成后");
        select(1, 3);
        tap(key("⌫"));
        await(editorText("前后"));
        await(status(true, true, 0));

        stage = "text at cursor start is not deleted";
        setText("甲乙");
        select(0, 0);
        tap(key("⌫"));
        await(editorText("甲乙"));
        await(status(true, true, 1));

        openEditor(true);
        stage = "unavailable preceding text is not treated as empty";
        setText("甲");
        tap(key("⌫"));
        await(editorText(""));
        tap(key("⌫"));
        SystemClock.sleep(300);
        await(status(true, true, 0));
    }

    private void openEditor(boolean unavailable) throws java.util.concurrent.TimeoutException {
        stage = "quote editor focus";
        Intent intent = new Intent(getTargetContext(), QuoteEditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        intent.putExtra("unavailable_before", unavailable);
        startActivitySync(intent);
        tap(field(EDITOR));
        await(key("n"));
        await(status(true, true, 0));
    }

    private Predicate<AccessibilityNodeInfo> editorText(String text) {
        // 没有 hint 的空 EditText 在无障碍树中可能返回 null，与 InputConnection 读取失败的 null 不同。
        return field(EDITOR).and(node -> text.isEmpty()
            ? node.getText() == null || node.getText().length() == 0
            : equalsText(text, node.getText()));
    }

    private Predicate<AccessibilityNodeInfo> status(boolean mention, boolean quote, int events) {
        String expected = "mention=" + mention + ";quote=" + quote + ";del=" + events;
        return field("msime-test-quote-status").and(node -> equalsText(expected, node.getText()));
    }

    private void setText(String text) {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!await(field(EDITOR)).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Synthetic text setup failed");
        await(editorText(text));
        select(text.length(), text.length());
    }

    private void select(int start, int end) {
        AccessibilityNodeInfo editor = await(field(EDITOR));
        // ACTION_SET_TEXT 已把光标放在末尾；TextView 对没有变化的选区动作可以返回 false。
        if (editor.getTextSelectionStart() == start && editor.getTextSelectionEnd() == end) return;
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end);
        if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic selection setup failed");
        await(field(EDITOR).and(node -> node.getTextSelectionStart() == start && node.getTextSelectionEnd() == end));
    }

    @Override protected Predicate<AccessibilityNodeInfo> key(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText("android.widget.Button", node.getClassName())
            && node.getText() != null && text.equalsIgnoreCase(node.getText().toString());
    }

    /** 拼音版同样经真实按钮的无障碍动作调用 InputConnection，避免 API 35 拒绝键盘区域的坐标注入。 */
    @Override protected void tap(Predicate<AccessibilityNodeInfo> match) throws java.util.concurrent.TimeoutException {
        AccessibilityNodeInfo target = await(match);
        if (equalsText(product, target.getPackageName()) && target.isClickable()) {
            if (!target.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("Synthetic product action failed");
            SystemClock.sleep(150);
            return;
        }
        super.tap(match);
    }

    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }
}
