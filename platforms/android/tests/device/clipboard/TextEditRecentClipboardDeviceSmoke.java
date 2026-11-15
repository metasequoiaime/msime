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
    private boolean reportedDisplay;
    private boolean trackpadOnly;
    private static final String TEXT = "synthetic-copy-alpha";
    private static final String PREVIEW = "粘贴最近复制的内容：" + TEXT;

    @Override public void onCreate(Bundle arguments) {
        spaceCopyOnly = arguments != null && "true".equals(arguments.getString("space-copy-only"));
        reportedDisplay = arguments != null && "true".equals(arguments.getString("reported-display"));
        layoutOnly = arguments != null && "true".equals(arguments.getString("layout-only"));
        trackpadOnly = arguments != null && "true".equals(arguments.getString("trackpad"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (layoutOnly) return "saved/cancelled/reset side order, document boundaries, newline-only return and all sixteen keys in portrait, landscape, one-handed and floating modes";
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
                if (reportedDisplay) awaitReportedDisplay();
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
                for (String description : new String[] {"全选", "复制", "剪切", "粘贴", "打开数字键盘", "插入空格", "插入换行", "退格"}) {
                    Rect keyBounds = bounds(await(described(description)));
                    if (keyBounds.isEmpty() || !panelBounds.contains(keyBounds))
                        throw new AssertionError("Editor key is outside panel: " + description);
                }
                // 排序页把「复制」往下挪了一格：左列第一格变成「粘贴」，「复制」在它下面。
                Rect pasted = bounds(await(described("粘贴")));
                Rect copied = bounds(await(described("复制")));
                if (pasted.top >= copied.top || pasted.left != copied.left)
                    throw new AssertionError("Keyboard ignored the saved side order");
                stage = "editor space shortcut";
                click("插入空格");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " ", node.getText())));
                stage = "editor return shortcut";
                click("插入换行");
                await(field("msime-test-plain").and(node -> equalsText(TEXT + " \n", node.getText())));
                stage = "editor number shortcut";
                click("打开数字键盘");
                await(described("返回原键盘"));
                click("返回原键盘");
                openPanel();
            }
            Rect before = bounds(awaitStableBounds(described("文本编辑面板")));
            stage = "selection label follows selection mode";
            int idleFace = settledKeyFace("选择模式", face -> true);
            click("选择模式");
            await(described("取消选择").and(node -> equalsText("取消选择", node.getText()) && node.isSelected()));
            stage = "selection key face fills with accent";
            settledKeyFace("取消选择", face -> face != idleFace);
            click("光标左移");
            await(field("msime-test-plain").and(node -> node.getTextSelectionStart() != node.getTextSelectionEnd()));
            stage = "selection key face stays filled while extending";
            settledKeyFace("取消选择", face -> face != idleFace);
            click("取消选择");
            await(described("选择模式").and(node -> equalsText("开始选择", node.getText()) && !node.isSelected()));
            stage = "selection key face restores";
            settledKeyFace("选择模式", face -> face == idleFace);
            click("光标右移");
            await(field("msime-test-plain").and(node -> node.getTextSelectionStart() == node.getTextSelectionEnd()));
            stage = "select all using panel";
            click("全选");
            stage = "copy using panel";
            click("复制");
            stage = "preview while panel stays open";
            await(described(PREVIEW));
            saveScreenshot("text-edit-recent-copy.png");
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

    /** 验收所用参数必须实际进入产品配置，不能只把 shell 命令返回当作切换成功。 */
    @SuppressWarnings("deprecation")
    private void awaitReportedDisplay() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            Configuration config = getTargetContext().getResources().getConfiguration();
            android.graphics.Point size = new android.graphics.Point();
            getTargetContext().getSystemService(android.view.WindowManager.class).getDefaultDisplay().getRealSize(size);
            if (config.densityDpi == 480 && Math.abs(config.fontScale - 1.25f) < .01f
                    && Math.min(size.x, size.y) == 1080 && Math.max(size.x, size.y) == 2376) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("报告屏幕的密度和字体缩放未生效");
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

    /** 与触控板共存：触控板不能遮住侧边操作、文档首尾和底排正中的开关；触控板模式的「选择」在触控板右上角。 */
    private void checkTrackpadCoexistence(String mode) throws Exception {
        click("触控板");
        await(described("触控板").and(AccessibilityNodeInfo::isChecked));
        Rect pad = bounds(awaitStableBounds(describedPrefix("文本编辑触控板")));
        Rect panel = bounds(awaitStableBounds(described("文本编辑面板")));
        if (pad.isEmpty() || !panel.contains(pad))
            throw new AssertionError("Trackpad is outside " + mode + " panel");
        for (String description : new String[] {"全选", "复制", "剪切", "粘贴", "移到行首，双击移到开头", "移到行尾，双击移到结尾",
                "退格", "打开数字键盘", "插入空格", "插入换行", "触控板"}) {
            Rect key = bounds(awaitStableBounds(described(description).and(AccessibilityNodeInfo::isClickable)));
            if (key.isEmpty() || !panel.contains(key) || Rect.intersects(pad, key))
                throw new AssertionError("Trackpad covered editor key in " + mode + ": " + description);
        }
        Rect select = bounds(awaitStableBounds(described("选择模式").and(AccessibilityNodeInfo::isClickable)));
        if (select.isEmpty() || !pad.contains(select) || select.centerX() < pad.centerX() || select.centerY() > pad.centerY())
            throw new AssertionError("Trackpad selection toggle is not in the trackpad's top-right corner in " + mode);
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        Rect leftBottom = bounds(awaitStableBounds(described("移到行首，双击移到开头")));
        Rect rightBottom = bounds(awaitStableBounds(described("移到行尾，双击移到结尾")));
        // 底排不能被触控板削去高度，也不能在下方留下原先整行大小的空白。
        if (Math.abs(leftBottom.top - rightBottom.top) > 1
                || Math.abs(leftBottom.bottom - rightBottom.bottom) > 1
                || panel.bottom - Math.min(leftBottom.bottom, rightBottom.bottom) > Math.ceil(8 * density))
            throw new AssertionError("触控板底排裁切或下方留白：" + mode);
        saveScreenshot("text-trackpad-" + mode + ".png");
        click("触控板");
        await(described("光标左移"));
        await(described("触控板").and(node -> !node.isChecked()));
        checkAllKeyBounds(mode);
    }

    private void checkAllKeyBounds(String mode) throws Exception {
        Rect panel = bounds(awaitStableBounds(described("文本编辑面板")));
        String[] descriptions = {"光标左移", "光标上移", "选择模式", "光标下移", "光标右移",
            "全选", "复制", "剪切", "粘贴", "移到行首，双击移到开头", "触控板", "移到行尾，双击移到结尾", "退格", "打开数字键盘", "插入空格", "插入换行"};
        Rect[] keys = new Rect[descriptions.length];
        int minimumWidth = Integer.MAX_VALUE, minimumHeight = Integer.MAX_VALUE, lowestBottom = Integer.MIN_VALUE;
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
            lowestBottom = Math.max(lowestBottom, keys[index].bottom);
        }
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        // #10967：底排五个键排满：左列第四个侧键、⇤、触控板开关、⇥ 和换行。
        int bottomRow = 0;
        for (Rect key : keys) if (Math.abs(key.bottom - lowestBottom) <= 1) bottomRow++;
        if (bottomRow < 5)
            throw new AssertionError("Text edit panel bottom row has only " + bottomRow + " keys in " + mode + " panel");
        // 只记录合成窗口的几何尺寸；可见与可点击不等于已验收触控舒适度。
        System.out.println("Text editor layout " + mode + ": panel=" + panel.width() + "x" + panel.height()
            + "px, minimum key=" + minimumWidth / density + "x" + minimumHeight / density + "dp");
        saveScreenshot("text-edit-line-" + mode + ".png");
    }

    /** 截图存到产品包的外部文件目录，供人工核对；只是留证，不参与判定。 */
    private void saveScreenshot(String name) throws Exception {
        android.graphics.Bitmap image = automation.takeScreenshot();
        if (image == null) throw new AssertionError("Synthetic editor screenshot unavailable");
        try (var output = new java.io.FileOutputStream(new File(getTargetContext().getExternalFilesDir(null), name))) {
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
        moveOrderRowDown("复制");
        tap(node -> equalsText("取消", node.getText()) && node.isClickable());
        if (!java.util.Arrays.equals(beforeCancel, Files.readAllBytes(preferences.toPath())))
            throw new AssertionError("Cancelling order draft wrote preferences");
        openOrderSettings();
        moveOrderRowDown("复制");
        tap(node -> equalsText("保存", node.getText()) && node.isClickable());
        JSONArray changed = new JSONArray(defaults.toString());
        Object first = changed.get(0);
        changed.put(0, changed.get(1));
        changed.put(1, first);
        awaitOrder(preferences, changed);
        openOrderSettings();
        await(node -> equalsText("1. 粘贴", node.getText()));
        // 排序页只列七个可见动作，选择行不在里面。
        if (visible(node -> node.getText() != null && node.getText().toString().endsWith(". 选择行")))
            throw new AssertionError("Text edit order sheet lists the hidden select-line action");
        tap(node -> equalsText("重置", node.getText()) && node.isClickable());
        await(node -> equalsText("1. 复制", node.getText()));
        tap(node -> equalsText("保存", node.getText()) && node.isClickable());
        awaitOrder(preferences, defaults);
        openOrderSettings();
        moveOrderRowDown("复制");
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
        await(orderHandle("复制"));
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

    /**
     * #10454：选择键开着时键面要整块换成皮肤强调色。只看无障碍节点的 selected 标志查不出来，那个 bug 里标志变了、键面却一直没重画，所以这里直接取屏幕像素。
     *
     * <p>取键帽水平居中、上沿往下四分之一高度的一点，避开中间的字，也在按下缩放时仍落在键帽里。按下动画还没走完时颜色会变，所以要隔 200 毫秒连续两次读到同一个颜色、且满足 `expected` 才算稳定。
     */
    private int settledKeyFace(String description, java.util.function.IntPredicate expected) throws Exception {
        long until = SystemClock.uptimeMillis() + 5000;
        Integer previous = null;
        int face = 0;
        while (true) {
            dropStaleNodes();
            Rect key = bounds(awaitStableBounds(described(description)));
            android.graphics.Bitmap image = automation.takeScreenshot();
            if (image == null) throw new AssertionError("Synthetic editor screenshot unavailable");
            try { face = image.getPixel(key.centerX(), key.top + key.height() / 4); }
            finally { image.recycle(); }
            if (previous != null && previous == face && expected.test(face)) return face;
            if (SystemClock.uptimeMillis() >= until)
                throw new AssertionError("Selection key face did not settle as expected: #" + Integer.toHexString(face));
            previous = face;
            SystemClock.sleep(200);
        }
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
