package app.msime.android.test;

import android.graphics.Rect;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** #8104、#8175：在真实编辑器里检查选择状态、全选复制剪切、顶部预览与返回键。 */
public final class TextEditRecentClipboardDeviceSmoke extends DeviceSmoke {
    private boolean spaceCopyOnly;
    private static final String TEXT = "synthetic-copy-alpha";
    private static final String PREVIEW = "粘贴最近复制的内容：" + TEXT;

    @Override public void onCreate(Bundle arguments) {
        spaceCopyOnly = arguments != null && "true".equals(arguments.getString("space-copy-only"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return spaceCopyOnly ? "space upward copy, editor permissions, selection restoration and clipboard preview"
            : "space upward copy, selection mode, editor shortcuts, copy/cut preview and disabled suggestions";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("Preferences escaped sandbox");
        File local = new File(root, "bootstrap/state/android-settings.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local), originalHistory = bytes(history);
        String originalIme = shellText("settings get secure default_input_method").trim();
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("privacy_mode", false).put("clipboard_history", true)
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
            .put("settings", new JSONObject()) : read(local);
        settings.getJSONObject("settings").put("platform.android.toolbar_text_edit", true)
            .put("platform.android.space_cursor", true).put("platform.android.space_voice", true)
            .put("platform.android.clipboard_suggestion", true).put("platform.android.toolbar_hidden", false);
        try {
            save(preferences, ++revision, values);
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            new SpaceCopyDeviceChecks(this).run(true);
            if (spaceCopyOnly) {
                values.put("clipboard_history", false);
                save(preferences, ++revision, values);
                new SpaceCopyDeviceChecks(this).run(false);
                return;
            }
            stage = "prepare editor before opening panel";
            openPanel();
            // API 28 的 UiAutomation 看不到面板内部节点；API 30+ 验证右列滚动快捷键及其真实编辑结果。
            if (Build.VERSION.SDK_INT >= 30) {
                stage = "scroll editor shortcuts";
                AccessibilityNodeInfo shortcuts = await(described("文本编辑快捷操作"));
                Rect shortcutBounds = bounds(shortcuts);
                int centerX = shortcutBounds.centerX();
                shellDiscard("input swipe " + centerX + " " + (shortcutBounds.bottom - 12)
                    + " " + centerX + " " + (shortcutBounds.top + 12) + " 350");
                await(described("打开数字键盘"));
                stage = "editor space shortcut";
                click("插入空格");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " ", node.getText())));
                stage = "editor return shortcut";
                click("插入换行");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " \n", node.getText())));
                stage = "editor number shortcut";
                click("打开数字键盘");
                await(described("切换到字母键盘"));
                click("切换到字母键盘");
                openPanel();
            }
            Rect before = bounds(awaitStableBounds(described("文本编辑面板")));
            stage = "selection label follows selection mode";
            click("选择模式");
            await(described("取消选择").and(node -> equalsText("取消选择", node.getText()) && node.isSelected()));
            click("光标左移");
            await(field("msime-test-plain").and(node -> node.getTextSelectionStart() != node.getTextSelectionEnd()));
            click("取消选择");
            await(described("选择模式").and(node -> equalsText("选择", node.getText()) && !node.isSelected()));
            click("光标右移");
            await(field("msime-test-plain").and(node -> node.getTextSelectionStart() == node.getTextSelectionEnd()));
            stage = "select all using panel";
            click("全选");
            stage = "copy using panel";
            click("复制");
            stage = "preview while panel stays open";
            await(described(PREVIEW));
            if (!before.equals(bounds(awaitStableBounds(described("文本编辑面板")))))
                throw new AssertionError("Copy moved text edit panel");
            stage = "cut using panel";
            click("全选");
            click("剪切");
            stage = "cut emptied selected text";
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            stage = "cut refreshed preview";
            await(described(PREVIEW));
            stage = "paste preview using panel";
            click(PREVIEW);
            await(field("msime-test-plain").and(node -> equalsText(TEXT, node.getText())));
            await(described("文本编辑面板"));
            absent(described("最近复制"));
            stage = "dismiss preview keeps panel";
            click("全选");
            click("复制");
            await(described(PREVIEW));
            click("不再显示这条复制的内容");
            absent(described("最近复制"));
            await(described("文本编辑面板"));
            stage = "return from preview without hiding keyboard";
            click("复制");
            await(described(PREVIEW));
            click("返回键盘");
            await(key("n"));
            absent(described("文本编辑面板"));
            await(described(PREVIEW));
            stage = "other panels hide preview";
            click("剪贴板");
            absent(described("最近复制"));
            stage = "history switch disables preview";
            values.put("clipboard_history", false);
            save(preferences, ++revision, values);
            openPanel();
            click("全选");
            click("复制");
            absent(described("最近复制"));
            new SpaceCopyDeviceChecks(this).run(false);
            stage = "local suggestion switch disables preview";
            values.put("clipboard_history", true);
            save(preferences, ++revision, values);
            settings.getJSONObject("settings").put("platform.android.clipboard_suggestion", false);
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            openPanel();
            click("全选");
            click("复制");
            absent(described("最近复制"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            restore(history, originalHistory);
            restartIme();
            if (!originalIme.isEmpty() && !originalIme.equals("null")) shellDiscard("ime set " + originalIme);
        }
    }

    private void openPanel() throws Exception {
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
        tap(field("msime-test-plain"));
        await(key("n"));
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, TEXT);
        if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Synthetic text setup failed");
        await(field("msime-test-plain").and(node -> equalsText(TEXT, node.getText())));
        // 设置文字可能使编辑器重建 InputConnection；面板只在重建完成后打开。
        automation.waitForIdle(500, 5000);
        SystemClock.sleep(1500);
        if (visible(described("最近复制"))) click("不再显示这条复制的内容");
        click("文本编辑");
        await(described("文本编辑面板"));
    }

    private void click(String description) throws Exception {
        dropStaleNodes();
        tap(described(description).and(AccessibilityNodeInfo::isClickable));
    }

    private void absent(Predicate<AccessibilityNodeInfo> match) {
        SystemClock.sleep(700);
        if (visible(match)) throw new AssertionError("Unexpected clipboard row or text edit panel");
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        dropStaleNodes();
        for (AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), match) != null) return true;
        return false;
    }

    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect();
        node.getBoundsInScreen(result);
        return result;
    }

    private String shellText(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            for (int count; (count = input.read(buffer)) != -1;) output.write(buffer, 0, count);
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }

    private void save(File file, long revision, JSONObject values) throws Exception {
        publish(file, new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static byte[] bytes(File file) throws Exception { return file.isFile() ? Files.readAllBytes(file.toPath()) : null; }

    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else publish(file, original);
    }
}
