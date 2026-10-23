package app.msime.android.test;

import android.graphics.Rect;
import android.os.Build;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 中文数字入口的真实网格、连续字面输入、符号往返和键位统计。 */
public final class NumberPadDeviceSmoke extends DeviceSmoke {
    private boolean passwordNumberPad;

    @Override public void onCreate(android.os.Bundle arguments) {
        passwordNumberPad = arguments != null && "true".equals(arguments.getString("password-number-pad"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (passwordNumberPad) return "文字密码框可在字母与数字九宫格间往返，数字密码直接显示九宫格";
        return "Chinese number-pad geometry, continuous arithmetic input, symbols, return and key statistics";
    }

    @Override protected void runChecks() throws Exception {
        if (passwordNumberPad) {
            verifyPasswordNumberPad();
            return;
        }
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(read(new File(root, "runtime-options.json")));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        File statistics = new File(directory, "typing-statistics.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] originalPreferences = snapshot(preferences);
        byte[] originalStatistics = snapshot(statistics);
        byte[] originalLocal = snapshot(local);
        long revision = originalPreferences == null ? 0
            : new JSONObject(new String(originalPreferences, StandardCharsets.UTF_8)).getLong("revision");
        try {
            // 使用专用 AVD 的合成偏好和统计，不读写用户输入，也不继承自定义侧栏。
            publish(local, "{\"version\":1,\"settings\":{}}".getBytes(StandardCharsets.UTF_8));
            // 直接数字布局只验几何和字面输入，先关闭统计，避免这轮异步刷盘污染后面的方案增量断言。
            publish(statistics, "{\"enabled\":false,\"days\":{}}".getBytes(StandardCharsets.UTF_8));
            restartIme();
            // 同一个输入法服务先停在普通文本框的 #+= 页，再进入数字框，验证编辑器切换不会泄漏符号页状态。
            stage = "direct number editor resets stale symbols";
            openEditor();
            tap(key("123"));
            tap(key("#+="));
            await(key("123"));
            openNumberEditor();
            assertGrid();
            stage = "direct number editor grid";
            typeLiteral("1203");
            await(field("msime-test-number").and(node -> equalsText("1203", node.getText())));
            // 直接数字输入的布局验收不与后续方案统计共用异步文件；清掉这轮合成按键后再建立数字页基线。
            publish(statistics, "{\"enabled\":true,\"days\":{}}".getBytes(StandardCharsets.UTF_8));
            restartIme();
            long digitOnes = 0, nineOnes = 0, digitZeros = 0, nineZeros = 0;
            for (String[] layout : new String[][] {
                    {"quanpin", "quanpin", "twenty_six_key"},
                    {"xiaohe", "shuangpin", "twenty_six_key"},
                    {"fourteen_key", "quanpin", "fourteen_key"},
                    {"nine_key", "quanpin", "nine_key"},
                    {"stroke", "stroke", "twenty_six_key"},
                    {"zhuyin_nine_key", "zhuyin", "nine_key"},
                    {"quanpin", "quanpin", "twenty_six_key", "fullwidth"}}) {
                boolean fullwidth = layout.length > 3;
                stage = layout[0] + " prepare";
                android.os.Bundle progress = new android.os.Bundle();
                progress.putString("stream", "Number pad case: " + layout[0] + (fullwidth ? " fullwidth" : " halfwidth") + "\n");
                sendStatus(0, progress);
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", layout[1]).put("last_chinese_scheme", layout[1])
                    .put("shuangpin_profile", "xiaohe").put("touch_keyboard_layout", layout[2])
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("character_width", fullwidth ? "fullwidth" : "halfwidth").put("chinese_punctuation", true)
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put(layout[0]))
                        .put("selected", layout[0]));
                publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
                restartIme();
                openEditor();
                stage = layout[0] + " letter face";
                // 全局输入模式可沿用上一套用例的英文状态，先通过真实语言键切回中文。
                Predicate<AccessibilityNodeInfo> mode = node -> node.isClickable()
                    && node.getContentDescription() != null
                    && node.getContentDescription().toString().startsWith("切换到");
                await(key("中").or(key("英")).and(mode));
                if (hasKey(key("英"))) tap(key("英"));
                awaitStableBounds(key("中").and(mode));
                await(letterFace(layout[0]));
                tap(key("123"));
                stage = layout[0] + " number grid";
                assertGrid();
                stage = layout[0] + " continuous arithmetic";
                String expression = "12.3+4-5*6/70";
                typeLiteral(expression);
                assertField(output(expression, fullwidth));
                assertGrid();
                if (layout[0].equals("quanpin")) {
                    stage = "long number remains in pad";
                    String digits = "12345678901234567890";
                    typeLiteral(digits);
                    assertField(output(expression + digits, fullwidth));
                    tap(key("⌫"));
                    assertField(output(expression + digits.substring(0, digits.length() - 1), fullwidth));
                }
                stage = layout[0] + " extended symbols and punctuation toggle";
                tap(key("#+="));
                await(key("中标"));
                tap(key("中标"));
                await(key("英标"));
                tap(key("英标"));
                await(key("中标"));
                tap(key("123"));
                assertGrid();
                stage = layout[0] + " return to original face";
                tap(described("切换到字母键盘"));
                await(letterFace(layout[0]));
                JSONObject stored = new JSONObject(read(preferences)).getJSONObject("preferences");
                if (!layout[2].equals(stored.getString("touch_keyboard_layout"))
                        || !layout[1].equals(stored.getString("scheme")))
                    throw new AssertionError("Number pad rewrote Chinese scheme or layout");
                if (layout[0].equals("quanpin") && !fullwidth) {
                    stage = "composition ends as raw text before literal digit";
                    openEditor();
                    tap(key("n"));
                    tap(key("i"));
                    tap(key("123"));
                    tap(key("1"));
                    assertField("ni1");
                    await(key("#+="));
                }
                // 保留同一份累计文档，避免清空文件与旧服务的异步刷盘争用；每轮检查精确增量。
                if (layout[0].equals("nine_key")) {
                    nineOnes++;
                    nineZeros++;
                } else {
                    digitOnes += layout[0].equals("quanpin") ? (fullwidth ? 3 : 4) : 1;
                    // 底行常驻的 0 跟随当前键面登记，借用数字页不能记成 Nine0。
                    digitZeros += layout[0].equals("quanpin") ? 3 : 1;
                }
                stage = layout[0] + " ordinary versus nine-key statistics";
                shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
                awaitKeyStatistics(statistics, digitOnes, nineOnes, digitZeros, nineZeros);
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            restore(preferences, originalPreferences);
            restore(statistics, originalStatistics);
            restore(local, originalLocal);
            restartIme();
        }
    }

    private void verifyPasswordNumberPad() throws Exception {
        restartIme();
        for (int type : new int[] {
                android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD,
                android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
                android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
                android.text.InputType.TYPE_CLASS_NUMBER | android.text.InputType.TYPE_NUMBER_VARIATION_PASSWORD}) {
            stage = "password keyboard type " + type;
            shellDiscard("am force-stop app.msime.android.test");
            shellDiscard("am start -f 0x10008000 -n app.msime.android.test/.EditorActivity"
                + " --ei msime_test_password_type " + type);
            tap(field("msime-test-password"));
            if ((type & android.text.InputType.TYPE_MASK_CLASS) == android.text.InputType.TYPE_CLASS_TEXT) {
                await(key("q"));
                tap(described("切换到数字和符号"));
            }
            stage = "password number grid type " + type;
            // 先确认已经切到数字页，不能把字母页的自动数字行当作稳定网格。
            await(key("#+="));
            assertGrid();
            typeLiteral("1203");
            awaitPasswordLength(4);
            tap(key("⌫"));
            awaitPasswordLength(3);
            tap(described("切换到字母键盘"));
            await(key("q"));
            // 文字密码仍能输入字母；数字密码的编辑器会过滤字母，因此只检查往返键面。
            if ((type & android.text.InputType.TYPE_MASK_CLASS) == android.text.InputType.TYPE_CLASS_TEXT) {
                tap(key("q"));
                awaitPasswordLength(4);
            }
            tap(described("切换到数字和符号"));
            await(key("#+="));
            assertGrid();
            if ((type & android.text.InputType.TYPE_MASK_CLASS) == android.text.InputType.TYPE_CLASS_TEXT) {
                tap(key("#+="));
                await(key("123"));
                tap(key("123"));
                await(key("#+="));
                assertGrid();
            }
        }
    }

    /** 只核对合成密码的长度，避免依赖系统是否暴露密码节点的明文。 */
    private void awaitPasswordLength(int length) {
        await(field("msime-test-password").and(node -> !node.isShowingHintText()
            && node.getText() != null && node.getText().length() == length));
    }

    private void assertGrid() {
        Rect[] cells = awaitStableGrid();
        for (int index = 0; index < cells.length; index++) {
            if (index % 3 > 0 && (cells[index].top != cells[index - 1].top
                    || cells[index].left <= cells[index - 1].left))
                throw new AssertionError("Number keys are not arranged in three columns: " + java.util.Arrays.toString(cells));
            if (index >= 3 && (cells[index].left != cells[index - 3].left
                    || cells[index].top <= cells[index - 3].top))
                throw new AssertionError("Number keys are not arranged in three rows: " + java.util.Arrays.toString(cells)
                    + "; " + imeTexts());
        }
        for (String value : new String[] {"0", ".", "+", "-", "*", "/"}) {
            Rect bounds = new Rect();
            AccessibilityNodeInfo node = awaitStableBounds(key(value));
            node.getBoundsInScreen(bounds);
            if (!node.isClickable() || bounds.isEmpty()) throw new AssertionError("Numeric action is not reachable");
        }
    }

    /** 整组坐标连续两拍一致后才比较同行同列，避免逐键等待把窗口过渡的不同帧拼到一起。 */
    private Rect[] awaitStableGrid() {
        Rect[] previous = null;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            dropStaleNodes();
            Rect[] current = new Rect[9];
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
                collectGridBounds(window.getRoot(), current);
            }
            boolean complete = java.util.Arrays.stream(current).allMatch(java.util.Objects::nonNull);
            if (complete && java.util.Arrays.equals(previous, current)) return current;
            previous = complete ? current : null;
            SystemClock.sleep(250);
        }
        throw new AssertionError("Number grid bounds never settled: " + java.util.Arrays.toString(previous));
    }

    /** 一次遍历取得整组坐标，避免 API 28 为九个数字各刷新整棵键盘子树。 */
    private void collectGridBounds(AccessibilityNodeInfo node, Rect[] cells) {
        if (node == null || Build.VERSION.SDK_INT < 34 && !node.refresh()) return;
        if (node.isVisibleToUser()) {
            for (int index = 0; index < cells.length; index++) {
                if (cells[index] != null || !key(Integer.toString(index + 1)).test(node)) continue;
                Rect bounds = new Rect();
                node.getBoundsInScreen(bounds);
                if (!bounds.isEmpty()) cells[index] = bounds;
            }
        }
        for (int index = 0; index < node.getChildCount(); index++) {
            if (java.util.Arrays.stream(cells).allMatch(java.util.Objects::nonNull)) return;
            collectGridBounds(node.getChild(index), cells);
        }
    }

    private void typeLiteral(String text) throws Exception {
        for (int index = 0; index < text.length(); index++) tap(key(text.substring(index, index + 1)));
    }

    private void assertField(String expected) {
        await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
    }

    private void openNumberEditor() throws Exception {
        shellDiscard("am force-stop app.msime.android.test");
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-number"));
    }

    private String output(String text, boolean fullwidth) {
        if (!fullwidth) return text;
        StringBuilder result = new StringBuilder();
        for (int index = 0; index < text.length(); index++) result.append((char) (text.charAt(index) + 0xfee0));
        return result.toString();
    }

    private Predicate<AccessibilityNodeInfo> letterFace(String scheme) {
        return switch (scheme) {
            case "nine_key" -> key("ABC");
            case "fourteen_key" -> described("按键 Q W");
            case "stroke" -> described("笔画 横");
            case "zhuyin_nine_key" -> described("注音 1 ㄅㄆㄇㄈ");
            default -> key("q");
        };
    }

    private boolean hasKey(Predicate<AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), predicate) != null) return true;
        return false;
    }

    private void awaitKeyStatistics(File file, long expectedDigits, long expectedNine,
            long expectedDigitZeros, long expectedNineZeros) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            JSONObject days = new JSONObject(read(file)).optJSONObject("dailyKeys");
            long digits = 0, nine = 0, digitZeros = 0, nineZeros = 0;
            if (days != null) for (var iterator = days.keys(); iterator.hasNext();) {
                JSONObject keys = days.getJSONObject(iterator.next());
                digits += keys.optLong("Digit1");
                nine += keys.optLong("Nine1");
                digitZeros += keys.optLong("Digit0");
                nineZeros += keys.optLong("Nine0");
            }
            if (digits > expectedDigits || nine > expectedNine
                    || digitZeros > expectedDigitZeros || nineZeros > expectedNineZeros)
                throw new AssertionError("Number pad counted the wrong layout's keys");
            if (digits == expectedDigits && nine == expectedNine
                    && digitZeros == expectedDigitZeros && nineZeros == expectedNineZeros) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Number pad key statistics were not flushed");
    }

    private void openEditor() throws Exception {
        shellDiscard("am force-stop app.msime.android.test");
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }

    // 稳定性检查必须读取当前节点；连续命中同一份过渡坐标缓存不能证明布局已经稳定。
    @Override protected AccessibilityNodeInfo await(Predicate<AccessibilityNodeInfo> match) {
        dropStaleNodes();
        return super.await(match);
    }

    // API 28 没有 clearCache，布局切换后逐节点刷新，避免查询已经拆掉的键面。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private static String read(File file) throws Exception {
        return new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
    }

    private static byte[] snapshot(File file) throws Exception {
        return file.exists() ? Files.readAllBytes(file.toPath()) : null;
    }

    private void restore(File file, byte[] contents) throws Exception {
        if (contents == null) Files.deleteIfExists(file.toPath());
        else publish(file, contents);
    }
}
