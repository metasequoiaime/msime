package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
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
                t.stage = "离开数字页后保留开关与排列 " + layout;
                physical(t.described("返回原键盘"));
                t.await(t.described("随机数字").and(AccessibilityNodeInfo::isSelected));
                SystemClock.sleep(500);
                physical(t.key("123"));
                check(Arrays.equals(previous, order()), "离页返回沿用上次排列");
                physical(t.described("随机数字"));
                t.await(t.described("随机数字").and(node -> !node.isSelected()));
                check(Arrays.equals(order(), STANDARD), "关闭恢复标准键序");
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
            check(DeviceSmoke.equalsText("数字 " + digit, node.getContentDescription()), "无障碍数字必须匹配键面");
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
