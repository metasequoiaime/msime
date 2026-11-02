package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 工具栏编辑套件的撤销/重做切片，复用入口以保持 instrumentation 数量不超过系统上限。 */
final class ToolbarHistoryDeviceChecks {
    private final DeviceSmoke t;

    ToolbarHistoryDeviceChecks(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("Preferences escaped sandbox");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local);
        JSONObject values = new JSONObject((original == null ? options : read(preferences))
            .getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("touch_toolbar", new JSONObject().put("emoji", false).put("clipboard", false).put("skin", false)
                .put("undo", false).put("redo", false))
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject settings = new JSONObject().put("version", 1).put("settings", new JSONObject()
            .put("platform.android.toolbar_hidden", false).put("platform.android.toolbar_phrase", false)
            .put("platform.android.toolbar_scheme", false).put("platform.android.toolbar_order", "undo,redo")
            .put("platform.android.clipboard_suggestion", false));
        try {
            t.stage = "history preferences";
            t.publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", original == null ? 1 : read(preferences).getLong("revision") + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            t.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            t.restartIme();
            t.stage = "history native toolbar switches";
            openSettings();
            toggle("撤销", false);
            awaitSetting(preferences, "undo", true);
            toggle("重做", false);
            awaitSetting(preferences, "redo", true);
            // 排序也通过产品页面保存；新按钮位于最前面，便于窄屏实际操作。
            openSettings();
            t.tap(node -> DeviceSmoke.equalsText("按钮顺序", node.getText()));
            // 「按钮顺序」没有箭头（#10546），照读屏的路径执行「撤销」那一行把手上的「下移」。
            t.moveOrderRowDown("撤销");
            t.tap(t.described("保存，保存按钮顺序"));
            t.await(node -> DeviceSmoke.equalsText("编辑工具栏", node.getText()));
            if (!read(local).getJSONObject("settings").getString("platform.android.toolbar_order").startsWith("redo,undo,"))
                throw new AssertionError("Native history ordering was not saved");
            t.restartIme();
            t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.HistoryEditorActivity");
            t.tap(t.field("history-first"));
            t.await(t.key("撤销"));
            Rect undo = bounds(t.await(t.key("撤销"))), redo = bounds(t.await(t.key("重做")));
            if (redo.centerX() >= undo.centerX()) throw new AssertionError("History order was not restored after restart");
            t.stage = "history native Chinese commit";
            for (String letter : new String[] {"n", "i", "h", "a", "o"}) t.tap(t.key(letter));
            t.tap(t.key("空格"));
            text("history-first", "你好");
            t.stage = "history native commit undo returns precommit spelling";
            t.tap(t.key("撤销"));
            text("history-first", "ni'hao");
            t.stage = "history native commit redo";
            t.tap(t.key("重做"));
            text("history-first", "你好");
            t.stage = "history deletion";
            t.tap(t.key("⌫"));
            text("history-first", "你");
            t.stage = "history deletion undo";
            t.tap(t.key("撤销"));
            text("history-first", "你好");
            t.stage = "history deletion redo";
            t.tap(t.key("重做"));
            text("history-first", "你");
            // 新提交会清空编辑器的重做分支，不能由输入法私下重放旧内容。
            t.tap(t.key("撤销"));
            text("history-first", "你好");
            for (String letter : new String[] {"n", "i", "h", "a", "o"}) t.tap(t.key(letter));
            t.tap(t.key("空格"));
            t.stage = "history new edit clears redo";
            text("history-first", "你好你好");
            t.tap(t.key("重做"));
            text("history-first", "你好你好");
            t.stage = "history new editor cannot restore previous editor";
            t.tap(t.field("history-second"));
            t.tap(t.key("重做"));
            t.tap(t.key("撤销"));
            text("history-second", "");
            text("history-first", "你好你好");
            t.stage = "history unsupported editor remains unchanged";
            t.tap(t.field("history-unsupported"));
            t.tap(t.key("撤销"));
            t.tap(t.key("重做"));
            text("history-unsupported", "");
            t.stage = "history password controls disabled";
            t.tap(t.field("history-password"));
            t.await(t.key("撤销").and(node -> !node.isEnabled()));
            t.await(t.key("重做").and(node -> !node.isEnabled()));
            t.stage = "history native switch off";
            openSettings();
            toggle("撤销", true);
            awaitSetting(preferences, "undo", false);
            t.restartIme();
            t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.HistoryEditorActivity");
            t.tap(t.field("history-first"));
            t.await(t.key("重做").and(AccessibilityNodeInfo::isEnabled));
            for (var window : t.automation.getWindows())
                if (t.find(window.getRoot(), t.key("撤销").and(AccessibilityNodeInfo::isVisibleToUser)) != null)
                    throw new AssertionError("Disabled undo shortcut remained visible");
        } finally {
            restore(preferences, original);
            restore(local, originalLocal);
            t.restartIme();
        }
    }

    private void restore(File file, byte[] contents) throws Exception {
        if (contents == null) Files.deleteIfExists(file.toPath());
        else t.publish(file, contents);
    }

    private void text(String field, String expected) {
        t.dropStaleNodes();
        t.await(t.field(field).and(node -> node.refresh() && (expected.isEmpty()
            ? node.isShowingHintText() || node.getText() == null || node.getText().length() == 0
            : DeviceSmoke.equalsText(expected, node.getText()))));
    }

    private void openSettings() throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TOOLBAR_EDITOR");
        t.dropStaleNodes();
        t.await(node -> DeviceSmoke.equalsText("编辑工具栏", node.getText()));
    }

    private void toggle(String label, boolean before) throws Exception {
        Predicate<AccessibilityNodeInfo> match = node -> node.isCheckable() && contains(node, label);
        for (int attempt = 0; attempt < 10; attempt++) {
            t.dropStaleNodes();
            AccessibilityNodeInfo row = null;
            for (var window : t.automation.getWindows()) {
                row = t.find(window.getRoot(), node -> node.isVisibleToUser() && match.test(node));
                if (row != null) break;
            }
            if (row != null) {
                if (row.isChecked() != before) throw new AssertionError("Unexpected native history switch state");
                t.tap(match);
                return;
            }
            // 只滑动产品设置页；顶部预览和排序不参与滚动手势。
            t.await(node -> node.isScrollable() && !DeviceSmoke.equalsText(
                "android.widget.HorizontalScrollView", node.getClassName())
                && DeviceSmoke.equalsText("app.msime.android", node.getPackageName()))
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(300);
        }
        throw new AssertionError("Native history switch was not found");
    }

    private static boolean contains(AccessibilityNodeInfo node, String label) {
        if (DeviceSmoke.equalsText(label, node.getText())) return true;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo child = node.getChild(index);
            if (child != null && contains(child, label)) return true;
        }
        return false;
    }

    private static void awaitSetting(File file, String key, boolean expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (read(file).getJSONObject("preferences").getJSONObject("touch_toolbar").optBoolean(key, !expected) == expected) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Native history switch did not persist");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect rect = new Rect(); node.getBoundsInScreen(rect); return rect;
    }
    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private static byte[] bytes(File file) throws Exception { return file.isFile() ? Files.readAllBytes(file.toPath()) : null; }
}
