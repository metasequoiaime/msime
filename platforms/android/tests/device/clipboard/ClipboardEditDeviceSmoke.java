package app.msime.android.test;

import android.content.ClipData;
import android.content.Context;
import android.content.ClipboardManager;
import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 从外部编辑器长按真实剪贴板卡片，验收条目键、编辑页、保存和返回。 */
public final class ClipboardEditDeviceSmoke extends DeviceSmoke {
    public static final String ORIGINAL = longText();
    public static final String REPLACEMENT = "MSIME 合成编辑完成🙂";
    public static final String SHORT = "MSIME 合成取消记录";
    private String product;

    private static String longText() {
        StringBuilder text = new StringBuilder("MSIME 合成剪贴板记录\n");
        for (int index = 0; index < 90; index++) text.append("甲乙丙丁");
        return text.toString();
    }

    @Override public void onCreate(Bundle args) {
        product = args == null ? "app.msime.android.pinyin" : args.getString("product", "app.msime.android.pinyin");
        super.onCreate(args);
    }

    @Override protected String successDescription() {
        return "external clipboard edit retains long text, saves, returns, inserts and cancels";
    }

    @Override protected void runChecks() throws Exception {
        if (!product.equals("app.msime.android") && !product.equals("app.msime.android.pinyin"))
            throw new AssertionError("Unknown test product");
        String ime = product.equals("app.msime.android") ? product + "/.MSIMEInputService"
            : product + "/app.msime.android.MSIMEInputService";
        restartIme(ime);
        openEditor();
        ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
        ClipData originalClip = clipboard.getPrimaryClip();
        clipboard.clearPrimaryClip();
        try {
            stage = "external long clipboard entry opens edit page";
            openClipboardEntry("MSIME 合成剪贴板记录");
            await(editor().and(node -> equalsText(ORIGINAL, node.getText())));
            stage = "clipboard editor survives delayed component change";
            long deadline = SystemClock.uptimeMillis() + 15_000;
            while (SystemClock.uptimeMillis() < deadline) {
                await(editor().and(node -> equalsText(ORIGINAL, node.getText())));
                SystemClock.sleep(250);
            }
            stage = "save changed clipboard text";
            Bundle replacement = new Bundle();
            replacement.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, REPLACEMENT);
            if (!await(editor()).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, replacement))
                throw new AssertionError("Synthetic clipboard edit action refused");
            await(editor().and(node -> equalsText(REPLACEMENT, node.getText())));
            click(hostText("保存").and(AccessibilityNodeInfo::isEnabled));
            stage = "save returns to original external editor";
            await(field("msime-test-plain"));
            tap(field("msime-test-plain"));
            await(key("n"));
            click(described("剪贴板"));
            stage = "saved clipboard entry inserts replacement";
            click(card(REPLACEMENT));
            await(field("msime-test-plain").and(node -> equalsText(REPLACEMENT, node.getText())));
            stage = "short clipboard entry cancels and returns";
            openClipboardEntry(SHORT);
            await(editor().and(node -> equalsText(SHORT, node.getText())));
            click(hostText("取消"));
            await(field("msime-test-plain"));
            stage = "guessed legacy edit token is rejected";
            Context host = getContext().createPackageContext(product, Context.CONTEXT_IGNORE_SECURITY);
            Intent guessed = new Intent().setClassName(host, product + ".home.HomeActivity")
                .putExtra("msime_open_page", "CLIPBOARD_EDIT")
                .putExtra("msime_page_args", guessedEntry(1700000000002L, ORIGINAL))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
            getContext().startActivity(guessed);
            SystemClock.sleep(2000);
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo root = window.getRoot();
                if (find(root, editor()) != null || find(root, node -> equalsText(product, node.getPackageName())
                        && equalsText(ORIGINAL, node.getText())) != null) {
                    throw new AssertionError("Guessed clipboard edit token exposed the original text");
                }
            }
        } finally {
            // 恢复系统剪贴板时让本测试 UID 位于前台，不读取或输出原剪贴板文字。
            openEditor();
            // 先停掉被测键盘，避免监听器把恢复的旧剪贴板写进合成历史，干扰后续元信息核验。
            shellDiscard("am force-stop " + product);
            if (originalClip == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(originalClip);
        }
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(key("n"));
    }

    private void openClipboardEntry(String prefix) {
        stage = "clipboard toolbar entry";
        click(described("剪贴板"));
        stage = "long press synthetic clipboard card";
        if (!await(card(prefix)).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Synthetic clipboard long press refused");
        stage = "clipboard edit action";
        click(hostText("编辑"));
        stage = "clipboard editor prefilled";
    }

    private static Bundle guessedEntry(long timestamp, String text) {
        Bundle args = new Bundle();
        args.putString("entry", timestamp + ":" + Integer.toHexString(text.hashCode()) + ":" + text.length());
        return args;
    }

    private Predicate<AccessibilityNodeInfo> card(String prefix) {
        return node -> equalsText(product, node.getPackageName()) && node.isLongClickable()
            && node.getText() != null && node.getText().toString().startsWith(prefix);
    }

    private Predicate<AccessibilityNodeInfo> editor() { return described("剪贴板记录的文字"); }

    private Predicate<AccessibilityNodeInfo> hostText(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText(text, node.getText());
    }

    private void click(Predicate<AccessibilityNodeInfo> match) {
        if (!await(match).performAction(AccessibilityNodeInfo.ACTION_CLICK))
            throw new AssertionError("Synthetic clipboard control action refused");
        SystemClock.sleep(200);
    }

    @Override protected Predicate<AccessibilityNodeInfo> key(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText("android.widget.Button", node.getClassName())
            && node.getText() != null && text.equalsIgnoreCase(node.getText().toString());
    }

    @Override protected Predicate<AccessibilityNodeInfo> described(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText(text, node.getContentDescription());
    }

    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected String imeTexts() { return "[omitted]"; }
}
