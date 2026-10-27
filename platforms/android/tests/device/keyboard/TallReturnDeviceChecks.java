package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 九键「右侧大回车」（共享偏好 `touch_nine_key_style = tall_return`，.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）的设备验收，由 {@link NineKeyNumericLayoutDeviceSmoke} 在自己的检查之后调用（API 35 的设备测试清单已到 100 项 instrumentation 的上限，不另加入口）。
 *
 * <p>量真实键帽，核对外框是一整块（没有单独的功能行）、符 在符号栏正下方且同宽、九宫格下方的 123 | 空格 | 中 与九宫格左右对齐、右列自上而下 ⌫ 重输 回车且回车从第三行顶边画到底行底边、右列不再有 ！ 而符号栏里有 ！；点回车落在底行那一段时照样换行。数字层照同一外框：符 和切换键横向位置不变，0 在空格的位置、右边是小数点，空格移到右列重输的位置，回车仍两倍高，点数字、小数点、空格按字面上屏。最后换回标准样式，功能行回来、右列的 ！ 回来。结束后恢复原偏好。
 */
final class TallReturnDeviceChecks {
    private final DeviceSmoke t;

    private TallReturnDeviceChecks(DeviceSmoke test) { t = test; }

    static void run(DeviceSmoke test) throws Exception { new TallReturnDeviceChecks(test).run(); }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        File local = new File(new File(new File(root, "bootstrap"), "state"), "android-settings.json");
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key")
            .put("ime_mode_scope", "app").put("default_ime_mode", "chinese")
            .put("character_width", "halfwidth")
            .put("touch_nine_key_style", "tall_return")
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            t.stage = "enable the tall-return nine-key style";
            t.publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            // 默认的左侧符号表、不交换中/英与 123、没有键盘底栏，量到的就是外框本身。
            t.publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.bottom_bar", false)).toString().getBytes(StandardCharsets.UTF_8));
            t.restartIme();
            t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
                + " --ez msime_test_multiline true");
            t.tap(t.field("msime-test-plain"));
            t.await(t.described("打开符号面板").and(AccessibilityNodeInfo::isEnabled));
            letterFrame();
            numberFrame();

            t.stage = "standard style restores the action row and the right-column ！";
            t.putSharedPreference("touch_nine_key_style", "standard");
            awaitCondition(() -> findVisible(t.described("键盘功能行")) != null);
            Rect delete = bounds(t.described("按键 删除"));
            Rect exclamation = bounds(t.key("！").and(node -> boundsOf(node).left >= delete.left - 3));
            if (exclamation.top < bounds(t.key("重输")).bottom)
                throw new AssertionError("标准样式下 ！ 应回到右列最下");
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath());
            else t.publish(local, originalLocal);
            t.restartIme();
        }
    }

    private int tolerance() {
        return Math.round(2 * t.getTargetContext().getResources().getDisplayMetrics().density) + 3;
    }

    private void letterFrame() throws Exception {
        // 输入法重启后先按皮肤片段画第一帧，实时偏好随后读到；等外框换成一整块（功能行隐藏）再量。
        t.stage = "tall-return frame replaces the action row";
        awaitCondition(() -> findVisible(t.described("键盘功能行")) == null);
        t.stage = "tall-return letter frame geometry";
        int tolerance = tolerance();
        Rect sidebar = bounds(t.described("符号栏，可上下滑动"));
        Rect symbol = bounds(t.key("符"));
        Rect topLeft = bounds(t.key("@#"));
        Rect topRight = bounds(t.key("DEF"));
        Rect bottomRow = bounds(t.key("WXYZ"));
        Rect layer = bounds(t.key("123"));
        Rect space = bounds(t.describedPrefix("空格"));
        Rect language = bounds(t.key("中"));
        Rect delete = bounds(t.described("按键 删除"));
        Rect rewrite = bounds(t.key("重输"));
        Rect enter = bounds(t.described("换行"));
        if (symbol.top < sidebar.bottom || Math.abs(symbol.left - sidebar.left) > tolerance
                || Math.abs(symbol.right - sidebar.right) > tolerance)
            throw new AssertionError("符 应在符号栏正下方且同宽：符 " + symbol + "，符号栏 " + sidebar);
        for (Rect key : new Rect[] {layer, space, language}) {
            if (key.top < bottomRow.bottom || Math.abs(key.centerY() - symbol.centerY()) > tolerance)
                throw new AssertionError("123、空格、中 应在九宫格下方与 符 同一行：" + key);
        }
        if (Math.abs(layer.left - topLeft.left) > tolerance || Math.abs(language.right - topRight.right) > tolerance
                || layer.right > space.left || space.right > language.left)
            throw new AssertionError("九宫格下方的 123 | 空格 | 中 应恰好与九宫格同宽：" + layer + " " + space + " " + language);
        for (Rect key : new Rect[] {rewrite, enter}) {
            if (Math.abs(key.left - delete.left) > tolerance || Math.abs(key.right - delete.right) > tolerance)
                throw new AssertionError("⌫、重输、回车应在同一列：" + key + "，⌫ " + delete);
        }
        if (delete.bottom > rewrite.top || rewrite.bottom > enter.top)
            throw new AssertionError("右列自上而下应是 ⌫、重输、回车");
        if (Math.abs(enter.top - bottomRow.top) > tolerance || Math.abs(enter.bottom - symbol.bottom) > tolerance)
            throw new AssertionError("回车应从九宫格第三行的顶边画到底行的底边：回车 " + enter + "，第三行 " + bottomRow + "，底行 " + symbol);
        if (enter.height() < 2 * rewrite.height())
            throw new AssertionError("回车应是两倍高：" + enter.height() + " / " + rewrite.height());
        if (findVisible(t.key("！").and(node -> boundsOf(node).left >= delete.left - tolerance)) != null)
            throw new AssertionError("右侧大回车的右列不应再有 ！");
        Rect exclamation = bounds(t.key("！"));
        if (exclamation.left < sidebar.left - tolerance || exclamation.right > sidebar.right + tolerance
                || exclamation.bottom > sidebar.bottom + tolerance)
            throw new AssertionError("！ 应接在符号栏里：" + exclamation);
        System.out.println("tall return letters: sidebar=" + sidebar + " symbol=" + symbol + " enter=" + enter
            + " strip=" + layer.left + ".." + language.right + " grid=" + topLeft.left + ".." + topRight.right);

        t.stage = "tall-return letter frame: ！ and the lower half of return";
        t.tap(t.key("！"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！", node.getText())));
        // 点回车落在底行那一段（原来功能行的位置）：多行输入框里换行。
        t.shellDiscard("input tap " + enter.centerX() + " " + (enter.bottom - Math.max(4, enter.height() / 8)));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n", node.getText())));
    }

    private void numberFrame() throws Exception {
        t.stage = "tall-return number frame geometry";
        int tolerance = tolerance();
        Rect letterSymbol = bounds(t.key("符"));
        Rect letterLayer = bounds(t.key("123"));
        Rect letterEnter = bounds(t.described("换行"));
        t.tap(t.key("123"));
        t.await(t.key("0"));
        if (findVisible(t.described("键盘功能行")) != null)
            throw new AssertionError("数字层同样没有单独的功能行");
        Rect symbol = bounds(t.key("符"));
        Rect back = bounds(t.key("九键"));
        Rect zero = bounds(t.key("0"));
        Rect decimal = bounds(t.key("."));
        Rect language = bounds(t.key("中"));
        Rect eight = bounds(t.key("8"));
        Rect nine = bounds(t.key("9"));
        Rect delete = bounds(t.described("按键 删除"));
        Rect space = bounds(t.describedPrefix("空格"));
        Rect enter = bounds(t.described("换行"));
        if (!near(symbol, letterSymbol, tolerance) || Math.abs(back.left - letterLayer.left) > tolerance
                || Math.abs(back.right - letterLayer.right) > tolerance)
            throw new AssertionError("数字层的 符 与切换键应保持字母层的位置：" + symbol + " " + back);
        if (zero.top < eight.bottom || zero.left < back.right || decimal.left < zero.right
                || language.left < decimal.right || Math.abs(zero.centerY() - symbol.centerY()) > tolerance)
            throw new AssertionError("数字层底行应是 切换键 | 0 | . | 中：" + back + " " + zero + " " + decimal + " " + language);
        if (Math.abs(space.left - delete.left) > tolerance || space.top < delete.bottom || space.bottom > enter.top
                || space.left < nine.right)
            throw new AssertionError("数字层的空格应在右列 ⌫ 与回车之间：" + space);
        if (!near(enter, letterEnter, tolerance))
            throw new AssertionError("数字层的回车应与字母层一样两倍高、同一位置：" + enter + " / " + letterEnter);

        t.stage = "tall-return number frame literal commit";
        for (String text : new String[] {"1", "0", "."}) t.tap(t.key(text));
        t.tap(t.describedPrefix("空格"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n10. ", node.getText())));
        t.tap(t.key("九键"));
        t.await(t.key("@#"));
    }

    private interface Condition {
        boolean holds() throws Exception;
    }

    private void awaitCondition(Condition condition) throws Exception {
        long deadline = android.os.SystemClock.uptimeMillis() + 10_000;
        while (android.os.SystemClock.uptimeMillis() < deadline) {
            t.dropStaleNodes();
            if (condition.holds()) return;
            android.os.SystemClock.sleep(200);
        }
        throw new AssertionError("Timed out at " + t.stage);
    }

    /** 屏幕上真正可见的节点；功能行隐藏后不在可见节点里。 */
    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : t.automation.getWindows()) {
            AccessibilityNodeInfo found = t.find(window.getRoot(), match.and(AccessibilityNodeInfo::isVisibleToUser));
            if (found != null) return found;
        }
        return null;
    }

    private static boolean near(Rect a, Rect b, int tolerance) {
        return Math.abs(a.left - b.left) <= tolerance && Math.abs(a.top - b.top) <= tolerance
            && Math.abs(a.right - b.right) <= tolerance && Math.abs(a.bottom - b.bottom) <= tolerance;
    }

    private static Rect boundsOf(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds;
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        t.awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
    }
}
