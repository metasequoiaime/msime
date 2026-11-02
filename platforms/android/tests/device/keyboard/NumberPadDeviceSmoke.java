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

/** 数字九宫格：中文各键面的真实网格、连续字面输入、符号往返和键位统计，以及英文、长空格样式、日文罗马字和假名、韩文、藏文、大千注音借来的数字九宫格（半角写法的左列多出的半角符号、藏文 ' 交给会话、注音 #+= 的共享符号行）。 */
public final class NumberPadDeviceSmoke extends DeviceSmoke {
    private boolean passwordNumberPad;
    private boolean randomDigits;

    @Override public void onCreate(android.os.Bundle arguments) {
        randomDigits = arguments != null && "true".equals(arguments.getString("random-digits"));
        passwordNumberPad = arguments != null && "true".equals(arguments.getString("password-number-pad"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (randomDigits) return "随机数字入口默认关闭、真实开关、逐次按键面上屏（U 模式进组字）与重排、离页保持及英文隔离";
        if (passwordNumberPad) return "文字密码框可在字母与数字九宫格间往返，数字密码直接显示九宫格";
        return "Number-pad geometry, continuous arithmetic input, symbols, return and key statistics in every scheme";
    }

    @Override protected void runChecks() throws Exception {
        if (randomDigits) {
            new RandomDigitsDeviceChecks(this).run();
            return;
        }
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
                settleLetterFace(key("中").or(key("英")).and(mode));
                if (hasKey(key("英"))) tap(key("英"));
                awaitStableBounds(key("中").and(mode));
                await(letterFace(layout[0]));
                // 往返时返回键留在字母层 123 那一格，符 留在字母层底行 符 那一格（#9113、#10061）。注音九键的 123 在键块里，不比。
                boolean bottomRowEntry = !layout[0].equals("zhuyin_nine_key");
                boolean bottomRowSymbol = bottomRowEntry && !layout[0].equals("fourteen_key");
                Rect letterLayer = bottomRowEntry ? stableBounds(key("123")) : null;
                Rect letterSymbol = bottomRowSymbol ? stableBounds(described("打开符号面板")) : null;
                tap(key("123"));
                stage = layout[0] + " number grid";
                assertGrid();
                stage = layout[0] + " entry keys keep their slots";
                if (letterLayer != null) {
                    Rect back = stableBounds(described("返回原键盘"));
                    if (!(back.left < letterLayer.centerX() && back.right > letterLayer.centerX()))
                        throw new AssertionError("返回键应在字母层 123 那一格：" + letterLayer + " -> " + back);
                }
                if (letterSymbol != null) {
                    Rect symbol = stableBounds(described("打开符号面板"));
                    if (!(symbol.left < letterSymbol.centerX() && symbol.right > letterSymbol.centerX()))
                        throw new AssertionError("符 应在字母层 符 那一格：" + letterSymbol + " -> " + symbol);
                }
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
                if (layout[0].equals("zhuyin_nine_key")) {
                    // 注音九键与大千一样，#+= 是原来三行十键的共享符号行（中文标点下是 ； ：），没有设计稿 #+= 层的标点切换（#10394）。
                    stage = layout[0] + " extended symbols are the shared symbol rows";
                    tap(key("#+="));
                    await(key("；"));
                    await(key("："));
                    await(key("@"));
                    if (hasKey(key("中标")) || hasKey(key("{")))
                        throw new AssertionError("Zhuyin nine-key #+= drew the design page: " + imeTexts());
                } else {
                    stage = layout[0] + " extended symbols and punctuation toggle";
                    tap(key("#+="));
                    await(key("中标"));
                    tap(key("中标"));
                    await(key("英标"));
                    tap(key("英标"));
                    await(key("中标"));
                }
                tap(key("123"));
                assertGrid();
                stage = layout[0] + " return to original face";
                tap(described("返回原键盘"));
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
            // 2026-10-31 起每种方案和触屏键面的 123 都进数字九宫格。这一段只验网格、标点写法、返回键的名字、空格、字面输入和 #+= 往返，不计统计，上面的精确增量不受影响。半角写法（英文、韩文、藏文）的左列在出厂表后面接 ; ' " $（排在 NumberPadPunctuationSmoke 里钉住），这里只验它们在、能上屏；藏文的 ' 交给会话打 achung；大千注音的 #+= 是原来的共享符号行。
            publish(statistics, "{\"enabled\":false,\"days\":{}}".getBytes(StandardCharsets.UTF_8));
            for (String[] face : new String[][] {
                    // 用例、Engine 方案、触屏布局、键盘方案、返回键键面、数字九宫格左列的逗号
                    {"english", "quanpin", "twenty_six_key", "quanpin", "ABC", ","},
                    {"long_space", "quanpin", "twenty_six_key", "quanpin", "返回", "，"},
                    {"japanese", "japanese", "twenty_six_key", "japanese", "ABC", "、"},
                    {"japanese_nine_key", "japanese", "nine_key", "japanese_nine_key", "あいう", "、"},
                    {"korean", "korean", "twenty_six_key", "korean", "한", ","},
                    {"tibetan", "tibetan", "twenty_six_key", "tibetan", "ABC", ","},
                    {"zhuyin", "zhuyin", "twenty_six_key", "zhuyin", "注", "，"}}) {
                boolean english = face[0].equals("english");
                boolean ascii = face[5].equals(",");
                stage = face[0] + " prepare";
                android.os.Bundle progress = new android.os.Bundle();
                progress.putString("stream", "Number pad case: " + face[0] + "\n");
                sendStatus(0, progress);
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", face[1]).put("touch_keyboard_layout", face[2])
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("character_width", "halfwidth").put("chinese_punctuation", true)
                    .put("touch_twenty_six_key_style", face[0].equals("long_space") ? "long_space" : "standard")
                    // 出厂的数字表：半角写法只给出厂表接半角符号，自定义的表不接。
                    .put("touch_nine_key_digit_sidebar_symbols", new JSONArray())
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put(face[3]))
                        .put("selected", face[3]));
                if (!face[1].equals("quanpin")) values.put("last_chinese_scheme", "quanpin");
                publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
                restartIme();
                openEditor();
                stage = face[0] + " letter face";
                // 语言键的描述写「这一下切到哪里」：在所选方案里是「切换到英文输入」，在英文里是「切换到所选输入方案」。全局输入模式可能沿用上一套用例的英文。
                Predicate<AccessibilityNodeInfo> toEnglish = languageTarget("切换到英文输入");
                Predicate<AccessibilityNodeInfo> toScheme = languageTarget("切换到所选输入方案");
                settleLetterFace(toEnglish.or(toScheme));
                if (english ? hasKey(toEnglish) : hasKey(toScheme)) tap(english ? toEnglish : toScheme);
                await(english ? toScheme : toEnglish);
                Predicate<AccessibilityNodeInfo> entry = node -> equalsText("app.msime.android", node.getPackageName())
                    && node.isClickable() && node.getContentDescription() != null
                    && node.getContentDescription().toString().endsWith("切换到数字和符号");
                if (face[0].equals("japanese_nine_key")) await(key("あ"));
                tap(entry);
                stage = face[0] + " number grid";
                assertGrid();
                Predicate<AccessibilityNodeInfo> back = described("返回原键盘").and(node -> equalsText(face[4], node.getText()));
                await(back);
                stage = face[0] + " punctuation follows the language";
                // 逗号在出厂表第 11 项，左列一屏最多五项（`NineKeySidebarPolicy.DIGIT_VISIBLE_ROWS`），停在顶部时总在可见区外，只能按整棵树找；别的写法的标点也可能排在可见区外，左列一并按整棵树查。
                awaitAny(key(face[5]));
                for (String foreign : face[5].equals("，") ? new String[] {"、", ","}
                        : face[5].equals("、") ? new String[] {"，"} : new String[] {"，", "。", "、", "！"})
                    if (hasKey(key(foreign)) || hasAnyKey(sidebarKey(foreign)))
                        throw new AssertionError(face[0] + " number pad shows " + foreign + ": " + imeTexts());
                // 半角写法的左列在出厂表后面接上 ; ' " $（滚出可见区也算），中文和日文写法不接。
                for (String extra : new String[] {";", "'", "\"", "$"})
                    if (hasAnyKey(sidebarKey(extra)) != ascii)
                        throw new AssertionError(face[0] + " number pad sidebar " + (ascii ? "lacks " : "adds ") + extra);
                // 逗号先上屏（输入框还是空的，英文不会按单词边界纠错），再连续输入算式：数字和运算符都按字面上屏，大千注音也不把数字读成注音。
                tap(key(face[5]));
                String expression = "12.3+4-5*6/70";
                typeLiteral(expression);
                String typed = face[5] + expression;
                assertField(typed);
                if (ascii) {
                    stage = face[0] + " half-width extras commit literally";
                    tap(sidebarKey("$"));
                    typed += "$";
                    assertField(typed);
                }
                assertGrid();
                if (face[0].equals("long_space")) {
                    stage = "long space number pad keeps a space key";
                    tap(key("空格"));
                    typed += " ";
                    assertField(typed);
                }
                stage = face[0] + " extended symbols return to the number pad";
                tap(key("#+="));
                await(key("123"));
                if (face[0].equals("zhuyin")) {
                    // 大千注音的 #+= 是原来那一页三行十键的共享符号行（中文标点下是 ； ：），不是设计稿的 #+= 层；大千读成音键的数字和 - 按字面上屏，不变成注音。
                    stage = "zhuyin #+= is the shared symbol rows";
                    await(key("；"));
                    await(key("："));
                    await(key("@"));
                    if (hasKey(key("#")) || hasKey(key("{")))
                        throw new AssertionError("Zhuyin #+= drew the design page: " + imeTexts());
                    tap(key("1"));
                    tap(key("-"));
                    typed += "1-";
                    assertField(typed);
                } else {
                    await(key("["));
                }
                tap(key("123"));
                assertGrid();
                await(back);
                stage = face[0] + " return to original face";
                tap(back);
                await(entry);
                if (face[0].equals("japanese_nine_key")) await(key("あ"));
                if (hasKey(key("#+="))) throw new AssertionError(face[0] + " stayed on the number pad");
                JSONObject stored = new JSONObject(read(preferences)).getJSONObject("preferences");
                if (!face[2].equals(stored.getString("touch_keyboard_layout")) || !face[1].equals(stored.getString("scheme")))
                    throw new AssertionError("Number pad rewrote the scheme or layout of " + face[0]);
                if (face[0].equals("tibetan")) {
                    // 藏文左列的 ' 是 achung：交给会话起头一个音节，回到字母层接着打 od，空格上屏藏文加音节点（'od 是 U+0F60 U+0F7C U+0F51，音节点 U+0F0B）。按字面上屏的话这里会是 'od 加空格。
                    stage = "tibetan achung from the number pad";
                    tap(entry);
                    assertGrid();
                    tap(sidebarKey("'"));
                    tap(back);
                    await(entry);
                    tap(key("o"));
                    tap(key("d"));
                    tap(key("空格"));
                    typed += "\u0F60\u0F7C\u0F51\u0F0B";
                    assertField(typed);
                }
                if (english) {
                    // 数字九宫格上的中/英与字母层同一个动作：切回所选方案并回到字母层。
                    stage = "language key on the English number pad";
                    tap(entry);
                    assertGrid();
                    tap(toScheme);
                    await(toEnglish);
                    await(entry);
                    if (hasKey(key("#+="))) throw new AssertionError("Language key left the number pad drawn");
                }
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
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(read(new File(root, "runtime-options.json")));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] originalPreferences = snapshot(preferences);
        long revision = originalPreferences == null ? 0
            : new JSONObject(new String(originalPreferences, StandardCharsets.UTF_8)).getLong("revision");
        try {
            // 以九键用户进密码框（#10481）：没有引擎会话的数字密码框按记住的中文键面画字母层，先在中文框里显示一次九键，让这份记忆确定是九键，而不是取决于前一个套件停在哪种键面。
            stage = "password nine-key user prepare";
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "nine_key").put("ime_mode_scope", "global")
                .put("default_ime_mode", "chinese")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("nine_key"))
                    .put("selected", "nine_key"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            Predicate<AccessibilityNodeInfo> mode = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            await(key("中").or(key("英")).and(mode));
            if (hasKey(key("英"))) tap(key("英"));
            await(letterFace("nine_key"));
            verifyPasswordTypes();
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            restore(preferences, originalPreferences);
            restartIme();
        }
    }

    private void verifyPasswordTypes() throws Exception {
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
            // 数字密码也是英文字母层（#10481）：上一套件留下的九键、注音键面记忆不能画进密码框。
            stage = "password letters type " + type;
            tap(described("返回原键盘"));
            await(key("q"));
            // 文字密码仍能输入字母；数字密码的编辑器会过滤字母，因此只检查往返键面。
            if ((type & android.text.InputType.TYPE_MASK_CLASS) == android.text.InputType.TYPE_CLASS_TEXT) {
                tap(key("q"));
                awaitPasswordLength(4);
                // 密码框不建引擎会话，长按 123 照样打开符号面板（#10517）；点「（」只上屏这半个，不补「）」。
                stage = "password symbol panel type " + type;
                AccessibilityNodeInfo layer = await(described("切换到数字和符号"));
                if (!layer.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                    throw new AssertionError("Symbol panel hold failed in password type " + type);
                await(described("符号面板"));
                tap(describedPrefix("符号分类 中文"));
                await(described("符号分类 中文，已选中"));
                tap(described("按键 符号 （，长按只输入这半个"));
                awaitPasswordLength(5);
                tap(described("关闭符号面板"));
                await(key("q"));
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

    /** 控件位置稳定后在屏幕上的范围。 */
    private Rect stableBounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
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

    /** 语言键：可点，描述以这一下要切到的目标结尾（共用底行的键和日文假名网格自己的键前缀不同）。 */
    private Predicate<AccessibilityNodeInfo> languageTarget(String target) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isClickable()
            && node.getContentDescription() != null && node.getContentDescription().toString().endsWith(target);
    }

    /** 数字九宫格左列的一个符号键（描述带长按修改符号栏的提示）。 */
    private Predicate<AccessibilityNodeInfo> sidebarKey(String symbol) {
        return described("按键 符号 " + symbol + "；长按快速修改符号栏");
    }

    /** 同 {@link #hasKey}，也算滚出可见区的节点（左列符号栏可以上下滚动）。 */
    private boolean hasAnyKey(Predicate<AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            if (findAny(window.getRoot(), predicate) != null) return true;
        return false;
    }

    private boolean hasKey(Predicate<AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), predicate) != null) return true;
        return false;
    }

    /** 输入法重绑后符号面板可能晚于编辑器出现；轮询时把它关掉，再开始语言键断言。 */
    private void settleLetterFace(Predicate<AccessibilityNodeInfo> target) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            // API 34+ 的 find 不再逐节点刷新；每拍先清 UiAutomation 缓存，避免读到已拆掉的面板或旧语言键。
            dropStaleNodes();
            if (hasKey(target)) return;
            // 只点本拍查到的关闭键且不阻塞：面板在检查和点击之间自己消失时，下一拍重新查询即可。
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo close = find(window.getRoot(), described("关闭符号面板"));
                if (close != null && close.isClickable()) {
                    close.performAction(AccessibilityNodeInfo.ACTION_CLICK);
                    break;
                }
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("等待字母键面超时：" + imeTexts());
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
        // 前一个设备套件可能把输入法留在符号面板；重新打开合成编辑器时先回到字母面，避免语言键等待读到旧面板。
        if (hasKey(described("关闭符号面板"))) tap(described("关闭符号面板"));
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
