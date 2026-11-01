package app.msime.android.test;

import android.graphics.Rect;
import android.content.res.Configuration;
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
    private boolean layoutOnly;
    private boolean trackpadOnly;
    private static final String TEXT = "synthetic-copy-alpha";
    private static final String PREVIEW = "粘贴最近复制的内容：" + TEXT;

    @Override public void onCreate(Bundle arguments) {
        spaceCopyOnly = arguments != null && "true".equals(arguments.getString("space-copy-only"));
        layoutOnly = arguments != null && "true".equals(arguments.getString("layout-only"));
        trackpadOnly = arguments != null && "true".equals(arguments.getString("trackpad"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (layoutOnly) return "saved/cancelled/reset side order and line boundaries, return long press and all seventeen keys in portrait, landscape, one-handed and floating modes";
        if (trackpadOnly) return "text trackpad through system editors, selection, temporary mode and key receiver";
        return spaceCopyOnly ? "space upward copy, editor permissions, selection restoration and clipboard preview"
            : "fixed editor grid, saved/cancelled/reset side order, shortcuts, selection and clipboard preview";
    }

    @Override protected void runChecks() throws Exception {
        if (trackpadOnly) { new TextTrackpadDeviceChecks(this).run(); return; }
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
            if (layoutOnly) {
                checkOrderSettings(preferences, values, revision);
                new TextEditLineDeviceChecks(this).run();
                checkLayoutModes(local, settings);
                return;
            }
            new SpaceCopyDeviceChecks(this).run(true);
            if (spaceCopyOnly) {
                values.put("clipboard_history", false);
                save(preferences, ++revision, values);
                new SpaceCopyDeviceChecks(this).run(false);
                return;
            }
            stage = "text editor side order settings";
            revision = checkOrderSettings(preferences, values, revision);
            stage = "prepare editor before opening panel";
            openPanel();
            // 所有按键必须在面板内可见，不先滑动或滚动。
            {
                stage = "all editor shortcuts visible without scrolling";
                Rect panelBounds = bounds(awaitStableBounds(described("文本编辑面板")));
                for (String description : new String[] {"全选", "复制", "剪切", "粘贴", "打开数字键盘", "插入空格", "插入换行，长按打开符号", "退格"}) {
                    Rect keyBounds = bounds(await(described(description)));
                    if (keyBounds.isEmpty() || !panelBounds.contains(keyBounds))
                        throw new AssertionError("Editor key is outside panel: " + description);
                }
                Rect pasted = bounds(await(described("粘贴")));
                Rect copied = bounds(await(described("复制")));
                if (pasted.top >= copied.top || pasted.left != copied.left)
                    throw new AssertionError("Keyboard ignored the saved side order");
                stage = "editor space shortcut";
                click("插入空格");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " ", node.getText())));
                stage = "editor return shortcut";
                click("插入换行，长按打开符号");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " \n", node.getText())));
                stage = "editor number shortcut";
                click("打开数字键盘");
                await(described("返回原键盘"));
                click("返回原键盘");
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

    /** 专项不依赖剪贴板手势；逐一确认实际窗口中的所有操作，不用滚动或隐藏按钮。 */
    private void checkLayoutModes(File local, JSONObject settings) throws Exception {
        String automatic = shellText("settings get system accelerometer_rotation").trim();
        String rotation = shellText("settings get system user_rotation").trim();
        JSONObject localValues = settings.getJSONObject("settings");
        try {
            shellDiscard("settings put system accelerometer_rotation 0");
            for (String mode : new String[] {"portrait", "left", "right", "floating", "landscape"}) {
                stage = "text editor layout " + mode;
                boolean landscape = mode.equals("landscape");
                shellDiscard("settings put system user_rotation " + (landscape ? "1" : "0"));
                localValues.put("platform.android.one_handed", mode.equals("left") || mode.equals("right") ? mode : "off")
                    .put("platform.android.floating_keyboard", mode.equals("floating"));
                publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
                long until = SystemClock.uptimeMillis() + 15000;
                while ((getTargetContext().getResources().getConfiguration().orientation
                        == Configuration.ORIENTATION_LANDSCAPE) != landscape) {
                    if (SystemClock.uptimeMillis() >= until)
                        throw new AssertionError("Synthetic editor orientation did not settle");
                    SystemClock.sleep(100);
                }
                openPanel(mode);
                checkAllKeyBounds(mode);
                click("插入空格");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " ", node.getText())));
                click("退格");
                await(field("msime-test-plain").and(node -> equalsText(TEXT, node.getText())));
                checkTrackpadCoexistence(mode);
            }
        } finally {
            restoreSystem("user_rotation", rotation);
            restoreSystem("accelerometer_rotation", automatic);
        }
    }

    private void restoreSystem(String key, String value) throws Exception {
        shellDiscard(value.matches("[0-9]+") ? "settings put system " + key + " " + value
            : "settings delete system " + key);
    }

    /** 与已合并的触控板共存；切换移动区不能遮住侧边操作与文档边界。 */
    private void checkTrackpadCoexistence(String mode) throws Exception {
        click("切换为触控板");
        Rect pad = bounds(awaitStableBounds(describedPrefix("文本编辑触控板")));
        Rect panel = bounds(awaitStableBounds(described("文本编辑面板")));
        if (pad.isEmpty() || !panel.contains(pad))
            throw new AssertionError("Trackpad is outside " + mode + " panel");
        for (String description : new String[] {"全选", "复制", "剪切", "粘贴", "移到开头", "移到结尾",
                "退格", "打开数字键盘", "插入空格", "插入换行，长按打开符号", "选择模式", "移到行首", "移到行尾"}) {
            Rect key = bounds(awaitStableBounds(described(description).and(AccessibilityNodeInfo::isClickable)));
            if (key.isEmpty() || !panel.contains(key) || Rect.intersects(pad, key))
                throw new AssertionError("Trackpad covered editor key in " + mode + ": " + description);
        }
        click("切换为方向键");
        await(described("光标左移"));
        checkAllKeyBounds(mode);
    }

    private void checkAllKeyBounds(String mode) throws Exception {
        Rect panel = bounds(awaitStableBounds(described("文本编辑面板")));
        Rect[] keys = new Rect[17];
        String[] descriptions = {"光标左移", "光标上移", "选择模式", "光标下移", "光标右移",
            "全选", "复制", "剪切", "粘贴", "移到开头", "移到结尾", "退格", "打开数字键盘", "插入空格", "插入换行，长按打开符号", "移到行首", "移到行尾"};
        int minimumWidth = Integer.MAX_VALUE, minimumHeight = Integer.MAX_VALUE;
        for (int index = 0; index < descriptions.length; index++) {
            keys[index] = bounds(awaitStableBounds(described(descriptions[index]).and(AccessibilityNodeInfo::isClickable)));
            if (keys[index].isEmpty() || !panel.contains(keys[index]))
                throw new AssertionError("Editor key is outside " + mode + " panel: " + descriptions[index]);
            for (int previous = 0; previous < index; previous++) {
                if (Rect.intersects(keys[index], keys[previous]))
                    throw new AssertionError("Editor keys overlap in " + mode + " panel");
            }
            minimumWidth = Math.min(minimumWidth, keys[index].width());
            minimumHeight = Math.min(minimumHeight, keys[index].height());
        }
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        // 只记录合成窗口的几何尺寸；可见与可点击不等于已验收触控舒适度。
        System.out.println("Text editor layout " + mode + ": panel=" + panel.width() + "x" + panel.height()
            + "px, minimum key=" + minimumWidth / density + "x" + minimumHeight / density + "dp");
        android.graphics.Bitmap image = automation.takeScreenshot();
        if (image == null) throw new AssertionError("Synthetic editor screenshot unavailable");
        try (var output = new java.io.FileOutputStream(new File(getTargetContext().getExternalFilesDir(null),
                "text-edit-line-" + mode + ".png"))) {
            if (!image.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("Synthetic editor screenshot failed");
        } finally { image.recycle(); }
    }

    /** 使用真实设置页保存、取消、重置和重开，再由键盘进程读取同一顺序。 */
    private long checkOrderSettings(File preferences, JSONObject values, long revision) throws Exception {
        Class<?> nativeClient = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        JSONObject response = new JSONObject((String) nativeClient.getMethod("defaultPreferences").invoke(null));
        JSONArray defaults = response.getJSONObject("value").getJSONArray("touch_text_edit_button_order");
        values.put("touch_text_edit_button_order", defaults);
        save(preferences, ++revision, values);
        byte[] beforeCancel = Files.readAllBytes(preferences.toPath());
        openOrderSettings();
        moveDown("复制");
        tap(node -> equalsText("取消", node.getText()) && node.isClickable());
        if (!java.util.Arrays.equals(beforeCancel, Files.readAllBytes(preferences.toPath())))
            throw new AssertionError("Cancelling order draft wrote preferences");
        openOrderSettings();
        moveDown("复制");
        tap(node -> equalsText("保存", node.getText()) && node.isClickable());
        JSONArray changed = new JSONArray(defaults.toString());
        Object first = changed.get(0);
        changed.put(0, changed.get(1));
        changed.put(1, first);
        awaitOrder(preferences, changed);
        openOrderSettings();
        await(node -> equalsText("1. 粘贴", node.getText()));
        tap(node -> equalsText("重置", node.getText()) && node.isClickable());
        await(node -> equalsText("1. 复制", node.getText()));
        tap(node -> equalsText("保存", node.getText()) && node.isClickable());
        awaitOrder(preferences, defaults);
        openOrderSettings();
        moveDown("复制");
        tap(node -> equalsText("保存", node.getText()) && node.isClickable());
        awaitOrder(preferences, changed);
        values.put("touch_text_edit_button_order", changed);
        return read(preferences).getLong("revision");
    }

    private void openOrderSettings() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        dropStaleNodes();
        AccessibilityNodeInfo entry = awaitAny(node -> equalsText("文本编辑按键顺序", node.getText()));
        if (!entry.isVisibleToUser()) {
            if (!entry.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()))
                throw new AssertionError("Text edit order could not be scrolled into view");
            dropStaleNodes();
            entry = await(node -> equalsText("文本编辑按键顺序", node.getText()));
        }
        while (!entry.isClickable() && entry.getParent() != null) entry = entry.getParent();
        if (!entry.performAction(AccessibilityNodeInfo.ACTION_CLICK))
            throw new AssertionError("Text edit order row did not open");
        await(orderRow("复制"));
    }

    /** 排序面板一行的拖动把手：读屏描述是「拖动调整 名称 的顺序」，操作菜单里带「下移」。箭头去掉后（#10392）读屏用户就靠把手上的这个操作排序，这里照读屏的路径走。 */
    private Predicate<AccessibilityNodeInfo> orderRow(String label) {
        return node -> node.getContentDescription() != null
            && node.getContentDescription().toString().equals("拖动调整 " + label + " 的顺序")
            && action(node, "下移") != null;
    }

    private void moveDown(String label) throws Exception {
        dropStaleNodes();
        AccessibilityNodeInfo handle = await(orderRow(label));
        if (!handle.performAction(action(handle, "下移").getId()))
            throw new AssertionError("Order handle refused 下移 for " + label);
    }

    private static AccessibilityNodeInfo.AccessibilityAction action(AccessibilityNodeInfo node, String label) {
        for (AccessibilityNodeInfo.AccessibilityAction action : node.getActionList())
            if (equalsText(label, action.getLabel())) return action;
        return null;
    }

    private void awaitOrder(File preferences, JSONArray expected) throws Exception {
        long until = SystemClock.uptimeMillis() + 5000;
        do {
            if (expected.toString().equals(loadPreferences(preferences).getJSONObject("preferences")
                    .getJSONArray("touch_text_edit_button_order").toString())) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("Text edit order did not reach persisted value");
    }

    /** 磁盘可以省略默认排列；由共享层补齐后再比较，不在测试中复制默认值规则。 */
    private JSONObject loadPreferences(File preferences) throws Exception {
        Class<?> nativeClient = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        JSONObject response = new JSONObject((String) nativeClient.getMethod("loadPreferences", String.class)
            .invoke(null, preferences.getParentFile().getAbsolutePath()));
        if (!response.optBoolean("ok")) throw new AssertionError("Shared text edit preferences unavailable");
        return response.getJSONObject("value");
    }

    private void openPanel() throws Exception {
        openPanel(null);
    }

    private void openPanel(String mode) throws Exception {
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
        tap(field("msime-test-plain"));
        await(key("n"));
        if ("left".equals(mode) || "right".equals(mode)) await(described("退出单手模式"));
        else if ("floating".equals(mode)) await(described("停靠键盘"));
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
        tap(described(description).and(AccessibilityNodeInfo::isClickable).and(AccessibilityNodeInfo::isEnabled));
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
