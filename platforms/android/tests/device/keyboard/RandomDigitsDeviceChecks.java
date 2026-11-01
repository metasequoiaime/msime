package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.Comparator;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用真实工具栏开关和触摸验证随机键面与上屏一致，复用数字页的设备入口。 */
final class RandomDigitsDeviceChecks {
    private final DeviceSmoke t;
    private static final String SETTING = "platform.android.toolbar_random_digits";
    private static final String TOOLBAR_HIDDEN = "platform.android.toolbar_hidden";
    private static final int[] STANDARD = {1, 2, 3, 4, 5, 6, 7, 8, 9, 0};

    RandomDigitsDeviceChecks(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("合成偏好越出沙箱");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = Files.readAllBytes(preferences.toPath());
        byte[] originalLocal = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        long revision = read(preferences).getLong("revision");
        try {
            for (String layout : new String[] {"twenty_six_key", "nine_key"}) {
                t.stage = "随机数字准备 " + layout;
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                    .put("touch_keyboard_layout", layout).put("ime_mode_scope", "global")
                    .put("default_ime_mode", "chinese").put("character_width", "halfwidth")
                    .put("touch_toolbar", new JSONObject().put("emoji", false).put("clipboard", false)
                        .put("skin", false).put("undo", false).put("redo", false))
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                        .put(layout.equals("nine_key") ? "nine_key" : "quanpin"))
                        .put("selected", layout.equals("nine_key") ? "nine_key" : "quanpin"));
                t.publish(preferences, bytes(new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", values)));
                t.publish(local, bytes(new JSONObject().put("version", 1).put("settings", new JSONObject()
                    .put("platform.android.toolbar_order", "random_digits")
                    .put("platform.android.toolbar_scheme", false).put("platform.android.toolbar_phrase", false))));
                t.restartIme();
                openEditor();
                if (visible(t.key("英"))) t.tap(t.key("英"));
                t.await(t.key("中"));
                if (visible(t.described("随机数字"))) throw new AssertionError("默认不应显示随入口");
                t.stage = "原生设置只打开入口 " + layout;
                setEntry(true);
                openEditor();
                chineseEditor();
                t.stage = "中文键盘显示未开启的随机数字入口 " + layout;
                t.await(t.described("随机数字").and(node -> !node.isSelected()));
                physical(t.key("123"));
                check(Arrays.equals(order(), STANDARD), "设置打开不能开启重排");
                physical(t.described("返回原键盘"));
                physical(t.described("随机数字"));
                t.await(t.described("随机数字").and(AccessibilityNodeInfo::isSelected));
                t.await(t.key("123"));
                if (visible(t.key("#+="))) throw new AssertionError("随开关不能自动跳数字页");
                physical(t.key("123"));
                int[] previous = order();
                check(!Arrays.equals(previous, STANDARD), "首次开启应产生新键序");
                Rect[] fixed = fixedBounds();
                StringBuilder expected = new StringBuilder();
                for (int digit = 0; digit < 10; digit++) {
                    t.stage = "真实触摸随机数字 " + layout + " " + digit;
                    physical(t.key(Integer.toString(digit)));
                    expected.append(digit);
                    String text = expected.toString();
                    t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(text, node.getText())));
                    int[] next = order();
                    check(!Arrays.equals(previous, next), "每次数字输入必须换序");
                    check(Arrays.equals(fixed, fixedBounds()), "重排不能移动其他按键");
                    previous = next;
                }
                if (layout.equals("twenty_six_key")) previous = unicodeDigitComposes(expected.toString(), previous);
                t.stage = "离开数字页后保留开关与排列 " + layout;
                physical(t.described("返回原键盘"));
                t.await(t.described("随机数字").and(AccessibilityNodeInfo::isSelected));
                SystemClock.sleep(500);
                physical(t.key("123"));
                check(Arrays.equals(previous, order()), "离页返回沿用上次排列");
                physical(t.described("随机数字"));
                t.await(t.described("随机数字").and(node -> !node.isSelected()));
                check(Arrays.equals(order(), STANDARD), "关闭恢复标准键序");
                t.stage = "隐藏整条工具栏同时关闭运行时状态 " + layout;
                physical(t.described("返回原键盘"));
                physical(t.described("随机数字"));
                t.await(t.described("随机数字").and(AccessibilityNodeInfo::isSelected));
                setToolbarHidden(local, true);
                openEditor();
                chineseEditor();
                if (visible(t.described("随机数字"))) throw new AssertionError("隐藏工具栏后随入口仍可见");
                physical(t.key("123"));
                check(Arrays.equals(order(), STANDARD), "隐藏工具栏后中文数字恢复标准排列");
                physical(t.key("1"));
                t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("1", node.getText())));
                check(Arrays.equals(order(), STANDARD), "隐藏工具栏后输入数字不能再换序");
                t.stage = "取消隐藏后随保持未开启 " + layout;
                setToolbarHidden(local, false);
                openEditor();
                chineseEditor();
                t.await(t.described("随机数字").and(node -> !node.isSelected()));
                physical(t.key("123"));
                check(Arrays.equals(order(), STANDARD), "取消隐藏不能恢复随机排列");
                t.stage = "英文键盘隔离 " + layout;
                physical(t.described("返回原键盘"));
                physical(t.described("随机数字"));
                physical(t.key("中"));
                t.await(t.key("英"));
                if (visible(t.described("随机数字"))) throw new AssertionError("英文不显示随入口");
                physical(t.key("123"));
                Rect one = bounds(t.awaitStableBounds(t.key("1")));
                Rect two = bounds(t.awaitStableBounds(t.key("2")));
                check(one.top == two.top && one.left < two.left, "英文数字页保持横排顺序");
                t.stage = "关闭入口同时关闭运行时状态 " + layout;
                setEntry(false);
                openEditor();
                chineseEditor();
                t.stage = "中文键盘关闭随机数字入口 " + layout;
                if (visible(t.described("随机数字"))) throw new AssertionError("已关闭入口仍可见");
                physical(t.key("123"));
                check(Arrays.equals(order(), STANDARD), "关闭入口后中文数字恢复标准排列");
            }
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            t.restartIme();
            t.publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else t.publish(local, originalLocal);
            t.restartIme();
        }
    }

    /**
     * 临时模式里数字页不重排：U 模式时随机资格不成立，键面是标准顺序，点下去的数字照常进组字（Engine 把数字列为拼写，不能按字面上屏），也不换序。退出 U 模式时人还在数字页、键行没有重建，键面要当场换回进入前的随机排列，否则键面与上屏会对不上。返回收尾后数字页的键序，供后面的离页检查沿用。
     */
    private int[] unicodeDigitComposes(String committed, int[] random) throws Exception {
        t.stage = "26 键用 Shift+U 进入 U 模式";
        physical(t.described("返回原键盘"));
        // 触摸和下面的 Shift+U 走两条互不等待的注入路径，点完「返回」不等它生效就发硬件键，键盘可能还停在数字页。2026-10-31 API 35 的一次失败就是这样：数字 9 之后再没有按键音（「返回」的点击没有触发），输入法在数字页上进了 U 模式，后面找不到字母页的 123。先等字母页的 123 出现，确认已经回到字母页再进 U 模式；点击真丢了也停在这一步，不会到后面才超时。
        t.await(t.key("123"));
        // 用带 Shift 的键盘事件在空组字时进入 U 模式。经 UiAutomation 注入而不是 Instrumentation.sendKeySync：API 28 上后者经 InputManager 直接注入，目标窗口属于合成编辑器那个应用，没有 INJECT_EVENTS 权限就抛 SecurityException（#10485）；UiAutomation 在 API 28 到 35 上都能注入其他应用的窗口，触摸注入用的也是它。
        long time = SystemClock.uptimeMillis();
        KeyEvent shiftU = new KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_U,
            0, KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON);
        shiftU.setSource(InputDevice.SOURCE_KEYBOARD);
        if (!t.automation.injectInputEvent(shiftU, true)) throw new AssertionError("Shift+U 按下注入失败");
        if (!t.automation.injectInputEvent(KeyEvent.changeAction(shiftU, KeyEvent.ACTION_UP), true))
            throw new AssertionError("Shift+U 抬起注入失败");
        awaitPlain(committed + "U");
        t.stage = "U 模式里数字页保持标准顺序";
        physical(t.key("123"));
        check(Arrays.equals(order(), STANDARD), "U 模式里数字页应为标准顺序");
        t.stage = "U 模式里点数字页的 4 进组字";
        physical(t.key("4"));
        awaitPlain(committed + "U4");
        check(Arrays.equals(order(), STANDARD), "U 模式里的数字输入不能换序");
        t.stage = "U 模式组字退格收尾";
        // 第一次退格删掉 4；U 模式在只剩入口字母时可能已经退出，所以只在仍显示 `U` 时再退一次。
        physical(t.key("⌫"));
        AccessibilityNodeInfo field = t.await(t.field("msime-test-plain").and(node ->
            DeviceSmoke.equalsText(committed + "U", node.getText()) || DeviceSmoke.equalsText(committed, node.getText())));
        if (!DeviceSmoke.equalsText(committed, field.getText())) {
            physical(t.key("⌫"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(committed, node.getText())));
        }
        t.stage = "退出 U 模式后数字页当场恢复随机排列";
        int[] after = order();
        check(Arrays.equals(random, after), "退出 U 模式后应恢复进入前的随机排列");
        return after;
    }

    /**
     * 等合成输入框变成 `expected`。超时时报出输入框此刻的合成文字、各窗口的类型与范围、输入法窗口里全部文字（`*` 标出用户看不见的节点）：`DeviceSmoke.imeTexts` 只列前 30 条，数字页的组字状态和盖在键上的窗口都在那之外。
     */
    private void awaitPlain(String expected) {
        try {
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(expected, node.getText())));
        } catch (AssertionError error) {
            AssertionError detailed = new AssertionError(error.getMessage() + "; field=" + plainText()
                + "; windows=" + windows() + "; ime=" + allImeTexts());
            detailed.initCause(error);
            throw detailed;
        }
    }

    private String plainText() {
        t.dropStaleNodes();
        for (var window : t.automation.getWindows()) {
            AccessibilityNodeInfo field = t.find(window.getRoot(), t.field("msime-test-plain"));
            if (field != null) return String.valueOf(field.getText());
        }
        return "<missing>";
    }

    private String windows() {
        StringBuilder out = new StringBuilder();
        for (AccessibilityWindowInfo window : t.automation.getWindows()) {
            Rect rect = new Rect();
            window.getBoundsInScreen(rect);
            AccessibilityNodeInfo root = window.getRoot();
            out.append('[').append(window.getType()).append(' ').append(window.getLayer()).append(' ')
                .append(root == null ? "-" : root.getPackageName()).append(' ').append(rect.toShortString())
                .append(']');
        }
        return out.toString();
    }

    private String allImeTexts() {
        java.util.List<String> texts = new java.util.ArrayList<>();
        for (AccessibilityWindowInfo window : t.automation.getWindows()) collectImeTexts(window.getRoot(), texts);
        return texts.toString();
    }

    private static void collectImeTexts(AccessibilityNodeInfo node, java.util.List<String> texts) {
        if (node == null || texts.size() >= 200) return;
        if (DeviceSmoke.equalsText("app.msime.android", node.getPackageName())) {
            CharSequence text = node.getText() != null ? node.getText() : node.getContentDescription();
            if (text != null && text.length() > 0) texts.add((node.isVisibleToUser() ? "" : "*") + text);
        }
        for (int index = 0; index < node.getChildCount(); index++) collectImeTexts(node.getChild(index), texts);
    }

    private void setEntry(boolean enabled) throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TOOLBAR_EDITOR");
        Predicate<AccessibilityNodeInfo> toggle = node -> "android.widget.Switch".contentEquals(node.getClassName())
            && node.isCheckable() && containsLabel(node, "随机数字");
        // 覆盖安装后会先显示一次临时说明；只确认这张已知提示，不绕过其他弹窗。
        AccessibilityNodeInfo ready = t.await(t.key("编辑工具栏").or(t.key("临时说明（首次运行）")));
        if (DeviceSmoke.equalsText("临时说明（首次运行）", ready.getText())) t.tap(t.key("知道了"));
        t.await(t.key("编辑工具栏"));
        t.stage = "工具栏设置滚动到随机数字";
        AccessibilityNodeInfo node = t.awaitAny(toggle);
        node.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        node = t.await(toggle);
        t.stage = "工具栏设置保存随机数字入口";
        if (node.isChecked() != enabled) t.tap(toggle);
        t.await(toggle.and(item -> item.isChecked() == enabled));
        File local = new File(t.getTargetContext().getFilesDir(), "bootstrap/state/android-settings.json");
        check(read(local).getJSONObject("settings").getBoolean(SETTING) == enabled, "原生设置必须持久化入口开关");
    }

    /** 工具栏显示方式是会同步的设置，可能来自其他设备，所以直接写本地设置文件；输入法在下一次 `onStartInput` 时重读。 */
    private void setToolbarHidden(File local, boolean hidden) throws Exception {
        JSONObject document = read(local);
        document.getJSONObject("settings").put(TOOLBAR_HIDDEN, hidden);
        t.publish(local, bytes(document));
    }

    private void openEditor() throws Exception {
        t.shellDiscard("am force-stop app.msime.android.test");
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.await(t.key("123"));
    }

    /** 设置窗口与编辑器是两个焦点会话，验收中文数字前明确选回中文，不依赖上一焦点的模式。 */
    private void chineseEditor() throws Exception {
        AccessibilityNodeInfo language = t.await(t.key("中").or(t.key("英")).and(AccessibilityNodeInfo::isEnabled));
        if (DeviceSmoke.equalsText("英", language.getText())) physical(t.key("英"));
        t.await(t.key("中"));
    }

    private int[] order() {
        Rect[] cells = new Rect[10];
        for (int digit = 0; digit < 10; digit++) {
            AccessibilityNodeInfo node = t.awaitStableBounds(t.key(Integer.toString(digit)));
            check(DeviceSmoke.equalsText("按键 数字 " + digit, node.getContentDescription()), "无障碍数字必须匹配键面");
            cells[digit] = bounds(node);
        }
        Integer[] digits = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
        Arrays.sort(digits, Comparator.comparingInt((Integer digit) -> cells[digit].top)
            .thenComparingInt(digit -> cells[digit].left));
        return Arrays.stream(digits).mapToInt(Integer::intValue).toArray();
    }

    private Rect[] fixedBounds() {
        return Arrays.stream(new String[] {".", "+", "-", "*", "/", "⌫"})
            .map(label -> bounds(t.awaitStableBounds(t.key(label)))).toArray(Rect[]::new);
    }

    private void physical(Predicate<AccessibilityNodeInfo> match) throws Exception {
        Rect rect = bounds(t.awaitStableBounds(match));
        t.shellDiscard("input tap " + rect.centerX() + " " + rect.centerY());
        t.dropStaleNodes();
    }
    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        t.dropStaleNodes();
        for (var window : t.automation.getWindows()) if (t.find(window.getRoot(), match) != null) return true;
        return false;
    }
    private static boolean containsLabel(AccessibilityNodeInfo node, String label) {
        if (DeviceSmoke.equalsText(label, node.getText())) return true;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo child = node.getChild(index);
            if (child != null && containsLabel(child, label)) return true;
        }
        return false;
    }
    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect(); node.getBoundsInScreen(result); return result;
    }
    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private static byte[] bytes(JSONObject value) { return value.toString().getBytes(StandardCharsets.UTF_8); }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
