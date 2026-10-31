package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
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
 * <p>量真实键帽，核对外框是一整块（没有单独的功能行）、符 在符号栏正下方且同宽、九宫格下方的 123 | 空格 | 中 与九宫格左右对齐、右列自上而下 ⌫ 重输 回车且回车从第三行顶边画到底行底边、右列不再有 ！ 而符号栏里有 ！；符号栏的 ！ 接过右列 ！ 的手势，上滑输入 0、长按选上标 ⁰；点回车落在底行那一段时照样换行。数字层照同一外框（与 iOS、鸿蒙相同）：符 和切换键横向位置不变，0 在空格的位置、右边是与中/英同宽的空格，小数点在右列重输的位置，回车仍两倍高，点数字、小数点、空格按字面上屏。然后换回标准样式，功能行回来、右列的 ！ 回来，标准底行的 123 和中/英与大回车外框里的同宽。
 *
 * <p>另外两种键面各量一遍：九键双拼的网格有四行，⌫ 仍占网格前两行、重输在第三行，回车从第四行的顶边画到底行的底边；笔画外框是 0.9 / 3 / 0.9 的三列，符 与符号栏同宽、九宫格下方那一行与笔画网格左右对齐、回车从第三行画到底行，切到它借的数字九宫格时 符、切换键和回车留在原处（数字层按笔画外框的份额画，不按九键的）。结束后恢复原偏好。
 */
final class TallReturnDeviceChecks {
    private final DeviceSmoke t;

    /** 右侧大回车字母层九宫格下方 123 和中/英的宽度，换回标准样式后与标准底行的同名键比。 */
    private int tallLayerWidth;
    private int tallLanguageWidth;

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
            t.selectChineseKeyboard();
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
            // 右侧大回车九宫格下方的 123 和中/英保持标准九键底行里的绝对宽度，空格占剩下的。
            int standardLayer = bounds(t.key("123")).width();
            int standardLanguage = bounds(t.key("中")).width();
            if (Math.abs(standardLayer - tallLayerWidth) > tolerance() || Math.abs(standardLanguage - tallLanguageWidth) > tolerance())
                throw new AssertionError("右侧大回车的 123、中 应与标准底行同宽：123 " + tallLayerWidth + " / " + standardLayer
                    + "，中 " + tallLanguageWidth + " / " + standardLanguage);

            shuangpinFrame(preferences, new JSONObject(base.toString())
                .put("touch_keyboard_layout", "nine_key_shuangpin").put("nine_key_shuangpin_profile", "migala3")
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new JSONArray().put("nine_key_shuangpin")).put("selected", "nine_key_shuangpin")));
            strokeFrame(preferences, new JSONObject(base.toString())
                .put("scheme", "stroke").put("last_chinese_scheme", "stroke")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new JSONArray().put("stroke")).put("selected", "stroke")));
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
        AccessibilityNodeInfo exclamationNode = t.await(t.key("！"));
        if (!exclamationNode.isLongClickable() || exclamationNode.getContentDescription() == null
                || !exclamationNode.getContentDescription().toString().startsWith("！；上滑输入 0"))
            throw new AssertionError("符号栏的 ！ 应接过右列 ！ 的上滑 0 与长按上下标：" + exclamationNode.getContentDescription());
        tallLayerWidth = layer.width();
        tallLanguageWidth = language.width();
        System.out.println("tall return letters: sidebar=" + sidebar + " symbol=" + symbol + " enter=" + enter
            + " strip=" + layer.left + ".." + language.right + " grid=" + topLeft.left + ".." + topRight.right);

        t.stage = "tall-return letter frame: ！ and the lower half of return";
        t.tap(t.key("！"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！", node.getText())));
        // 点回车落在底行那一段（原来功能行的位置）：多行输入框里换行。
        t.shellDiscard("input tap " + enter.centerX() + " " + (enter.bottom - Math.max(4, enter.height() / 8)));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n", node.getText())));

        // 原来右列 ！ 上滑输入 0、长按选上标或下标 0，是九键字母层直接输入 0 的地方；接到符号栏的 ！ 照样能用。
        t.stage = "tall-return sidebar ！ swipe up commits zero";
        swipeUp(t.key("！"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n0", node.getText())));
        t.stage = "tall-return sidebar ！ hold offers superscript zero";
        hold(t.key("！"));
        t.tap(t.described("输入 ⁰"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n0⁰", node.getText())));
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
        if (zero.top < eight.bottom || zero.left < back.right || space.left < zero.right
                || language.left < space.right || Math.abs(zero.centerY() - symbol.centerY()) > tolerance
                || Math.abs(space.centerY() - symbol.centerY()) > tolerance)
            throw new AssertionError("数字层底行应是 切换键 | 0 | 空格 | 中：" + back + " " + zero + " " + space + " " + language);
        if (Math.abs(space.width() - language.width()) > tolerance || zero.width() <= space.width())
            throw new AssertionError("数字层的空格应与中同宽，0 占剩下的宽度：0 " + zero.width() + "，空格 " + space.width()
                + "，中 " + language.width());
        if (Math.abs(decimal.left - delete.left) > tolerance || Math.abs(decimal.right - delete.right) > tolerance
                || decimal.top < delete.bottom || decimal.bottom > enter.top || decimal.left < nine.right)
            throw new AssertionError("数字层的小数点应在右列 ⌫ 与回车之间：" + decimal);
        if (!near(enter, letterEnter, tolerance))
            throw new AssertionError("数字层的回车应与字母层一样两倍高、同一位置：" + enter + " / " + letterEnter);

        t.stage = "tall-return number frame literal commit";
        for (String text : new String[] {"1", "0", "."}) t.tap(t.key(text));
        t.tap(t.describedPrefix("空格"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("！\n0⁰10. ", node.getText())));
        t.tap(t.key("九键"));
        t.await(t.key("@#"));
    }

    /** 换一套偏好（新的修订号）、重启输入法、打开多行编辑器，等键面画出来、功能行隐藏。 */
    private void switchTo(File preferences, JSONObject values, Predicate<AccessibilityNodeInfo> face) throws Exception {
        long revision = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8))
            .getLong("revision");
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
        t.restartIme();
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_multiline true");
        t.tap(t.field("msime-test-plain"));
        t.await(face);
        if (findVisible(t.key("英")) != null) t.tap(t.key("英"));
        t.await(t.key("中"));
        awaitCondition(() -> findVisible(t.described("键盘功能行")) == null);
    }

    /** 九键双拼：网格四行，⌫ 占前两行、重输第三行，回车从第四行顶边画到底行底边；外框其余部分与拼音九键相同。 */
    private void shuangpinFrame(File preferences, JSONObject values) throws Exception {
        t.stage = "tall-return nine-key shuangpin frame";
        switchTo(preferences, values, t.described("按键 0，韵母 u ü ei"));
        int tolerance = tolerance();
        Rect sidebar = bounds(t.described("符号栏，可上下滑动"));
        Rect symbol = bounds(t.key("符"));
        Rect one = bounds(t.key("1"));
        Rect two = bounds(t.key("2"));
        Rect three = bounds(t.key("3"));
        Rect five = bounds(t.key("5"));
        Rect eight = bounds(t.key("8"));
        Rect zero = bounds(t.key("0"));
        Rect layer = bounds(t.key("123"));
        Rect language = bounds(t.key("中"));
        Rect delete = bounds(t.described("按键 删除"));
        Rect rewrite = bounds(t.key("重输"));
        Rect enter = bounds(t.described("换行"));
        System.out.println("tall return shuangpin: delete=" + delete + " rewrite=" + rewrite + " enter=" + enter
            + " rows=" + two + " " + five + " " + eight + " " + zero + " symbol=" + symbol);
        if (symbol.top < sidebar.bottom || Math.abs(symbol.left - sidebar.left) > tolerance
                || Math.abs(symbol.right - sidebar.right) > tolerance)
            throw new AssertionError("九键双拼：符 应在符号栏正下方且同宽：符 " + symbol + "，符号栏 " + sidebar);
        if (Math.abs(layer.left - one.left) > tolerance || Math.abs(language.right - three.right) > tolerance
                || layer.top < zero.bottom)
            throw new AssertionError("九键双拼：123 | 空格 | 中 应在网格下方且与网格同宽：" + layer + " " + language);
        if (Math.abs(delete.top - two.top) > tolerance || Math.abs(delete.bottom - five.bottom) > tolerance)
            throw new AssertionError("九键双拼：⌫ 应占网格前两行：⌫ " + delete + "，第一行 " + two + "，第二行 " + five);
        if (Math.abs(rewrite.top - eight.top) > tolerance || Math.abs(rewrite.bottom - eight.bottom) > tolerance)
            throw new AssertionError("九键双拼：重输 应在网格第三行：重输 " + rewrite + "，第三行 " + eight);
        if (Math.abs(enter.top - zero.top) > tolerance || Math.abs(enter.bottom - symbol.bottom) > tolerance)
            throw new AssertionError("九键双拼：回车应从网格第四行的顶边画到底行的底边：回车 " + enter + "，第四行 " + zero + "，底行 " + symbol);
        for (Rect key : new Rect[] {rewrite, enter}) {
            if (Math.abs(key.left - delete.left) > tolerance || Math.abs(key.right - delete.right) > tolerance)
                throw new AssertionError("九键双拼：⌫、重输、回车应在同一列：" + key + "，⌫ " + delete);
        }
        if (findVisible(t.key("！").and(node -> boundsOf(node).left >= delete.left - tolerance)) != null)
            throw new AssertionError("九键双拼：右侧大回车的右列不应再有 ！");
    }

    /** 笔画：0.9 / 3 / 0.9 的外框，字母层与它借的数字九宫格是同一个外框，切层时 符、切换键和回车不动。 */
    private void strokeFrame(File preferences, JSONObject values) throws Exception {
        t.stage = "tall-return stroke frame";
        switchTo(preferences, values, t.described("笔画 横"));
        int tolerance = tolerance();
        Rect sidebar = bounds(t.described("符号栏，可上下滑动"));
        Rect symbol = bounds(t.key("符"));
        Rect topLeft = bounds(t.described("笔画 横"));
        Rect topRight = bounds(t.described("笔画 撇"));
        Rect thirdRow = bounds(t.key("0"));
        Rect layer = bounds(t.key("123"));
        Rect space = bounds(t.describedPrefix("空格"));
        Rect language = bounds(t.key("中"));
        Rect delete = bounds(t.described("按键 删除"));
        Rect rewrite = bounds(t.key("重输"));
        Rect enter = bounds(t.described("换行"));
        System.out.println("tall return stroke: sidebar=" + sidebar + " symbol=" + symbol + " enter=" + enter
            + " strip=" + layer.left + ".." + language.right + " grid=" + topLeft.left + ".." + topRight.right);
        if (symbol.top < sidebar.bottom || Math.abs(symbol.left - sidebar.left) > tolerance
                || Math.abs(symbol.right - sidebar.right) > tolerance)
            throw new AssertionError("笔画：符 应在符号栏正下方且同宽：符 " + symbol + "，符号栏 " + sidebar);
        for (Rect key : new Rect[] {layer, space, language}) {
            if (key.top < thirdRow.bottom || Math.abs(key.centerY() - symbol.centerY()) > tolerance)
                throw new AssertionError("笔画：123、空格、中 应在笔画网格下方与 符 同一行：" + key);
        }
        if (Math.abs(layer.left - topLeft.left) > tolerance || Math.abs(language.right - topRight.right) > tolerance
                || layer.right > space.left || space.right > language.left)
            throw new AssertionError("笔画：123 | 空格 | 中 应恰好与笔画网格同宽：" + layer + " " + space + " " + language);
        for (Rect key : new Rect[] {rewrite, enter}) {
            if (Math.abs(key.left - delete.left) > tolerance || Math.abs(key.right - delete.right) > tolerance)
                throw new AssertionError("笔画：⌫、重输、回车应在同一列：" + key + "，⌫ " + delete);
        }
        if (delete.bottom > rewrite.top || rewrite.bottom > enter.top)
            throw new AssertionError("笔画：右列自上而下应是 ⌫、重输、回车");
        if (Math.abs(enter.top - thirdRow.top) > tolerance || Math.abs(enter.bottom - symbol.bottom) > tolerance)
            throw new AssertionError("笔画：回车应从网格第三行的顶边画到底行的底边：回车 " + enter + "，第三行 " + thirdRow + "，底行 " + symbol);
        // 0.9 / 3 / 0.9：左列比拼音九键的 3 / 14 / 3 宽（18.75% 对 15%），按外框整宽量。
        int frameWidth = enter.right - symbol.left;
        if (Math.abs(sidebar.width() - frameWidth * 0.1875f) > 2 * tolerance)
            throw new AssertionError("笔画：左列应占外框的 18.75%：" + sidebar.width() + " / " + frameWidth);
        if (findVisible(t.key("！").and(node -> boundsOf(node).left >= delete.left - tolerance)) != null)
            throw new AssertionError("笔画：右侧大回车的右列不应再有 ！");
        Rect exclamation = bounds(t.key("！"));
        if (exclamation.left < sidebar.left - tolerance || exclamation.right > sidebar.right + tolerance
                || exclamation.bottom > sidebar.bottom + tolerance)
            throw new AssertionError("笔画：！ 应接在符号栏里：" + exclamation);

        t.stage = "tall-return stroke number layer keeps the stroke frame";
        t.tap(t.key("123"));
        t.await(t.key("笔"));
        if (findVisible(t.described("键盘功能行")) != null)
            throw new AssertionError("笔画数字层同样没有单独的功能行");
        Rect numberSymbol = bounds(t.key("符"));
        Rect back = bounds(t.key("笔"));
        Rect numberEnter = bounds(t.described("换行"));
        System.out.println("tall return stroke digits: symbol=" + numberSymbol + " back=" + back + " enter=" + numberEnter);
        if (!near(numberSymbol, symbol, tolerance) || Math.abs(back.left - layer.left) > tolerance
                || Math.abs(back.right - layer.right) > tolerance)
            throw new AssertionError("笔画数字层的 符 与切换键应保持字母层的位置：" + numberSymbol + " / " + symbol
                + "，" + back + " / " + layer);
        if (!near(numberEnter, enter, tolerance))
            throw new AssertionError("笔画数字层的回车应与字母层同一位置：" + numberEnter + " / " + enter);
        t.tap(t.key("笔"));
        t.await(t.described("笔画 横"));
    }

    private void swipeUp(Predicate<AccessibilityNodeInfo> match) throws Exception {
        Rect bounds = bounds(match);
        float x = bounds.centerX();
        float startY = bounds.centerY();
        float endY = startY - 28f * t.getTargetContext().getResources().getDisplayMetrics().density;
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, startY, 0);
        MotionEvent move = MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_MOVE, x, endY, 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 80, MotionEvent.ACTION_UP, x, endY, 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, move, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!t.automation.injectInputEvent(event, true))
                    throw new AssertionError("Swipe injection failed at " + t.stage);
            }
        } finally {
            down.recycle();
            move.recycle();
            up.recycle();
        }
        SystemClock.sleep(150);
    }

    private void hold(Predicate<AccessibilityNodeInfo> match) throws Exception {
        Rect bounds = bounds(match);
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(down, true)) throw new AssertionError("Long-press down rejected");
            SystemClock.sleep(650);
        } finally {
            down.recycle();
            MotionEvent up = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP,
                bounds.centerX(), bounds.centerY(), 0);
            up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            try {
                if (!t.automation.injectInputEvent(up, true)) throw new AssertionError("Long-press up rejected");
            } finally { up.recycle(); }
        }
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
