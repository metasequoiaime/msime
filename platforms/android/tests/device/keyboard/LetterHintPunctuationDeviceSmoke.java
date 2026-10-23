package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 字母角标与底行逗号长按通过共享标点路由提交，长按松手不追加逗号。
 *
 * <p>中文标点生效时，下滑 v 交给 Engine 的标点路由：空闲时上屏「？」，组字中先按首选结束再接「？」，得到「你好？」而不是按字面上屏的拼音原文加 `?`；用工具面板关掉中文标点后同一手势仍按字面上屏 `?`。JVM 冒烟只覆盖键面表，覆盖不到经 Engine 结束组字这一步，所以在设备上驱动真实手势。下滑靠注入按下、移动、抬起三个触屏事件（「滑动输入符号」默认开、方向默认下滑），无障碍点击没有移动这一步。
 */
public final class LetterHintPunctuationDeviceSmoke extends DeviceSmoke {
    private static final String PLAIN = "msime-test-plain";
    /** 下滑距离（dp）：比 `SwipeHintPolicy.THRESHOLD_DP` 的 14 dp 大一截，不会落在阈值边上。 */
    private static final float SWIPE_DP = 28f;
    private android.os.Bundle arguments;

    @Override public void onCreate(android.os.Bundle arguments) {
        this.arguments = arguments;
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (arguments != null && "true".equals(arguments.getString("letter-swipe")))
            return "letter option drag, release/cancel, engine punctuation and swipe direction/disable";
        return "letter hints and comma hold route punctuation once, commit composition and preserve the symbol panel entry";
    }

    @Override protected void runChecks() throws Exception {
        if (arguments != null && "true".equals(arguments.getString("letter-swipe"))) {
            runConfiguredChecks(true);
            return;
        }
        if (arguments != null && ("true".equals(arguments.getString("english-hold"))
                || "true".equals(arguments.getString("unicode18-hold")))) {
            runEnglishHoldChecks();
            return;
        }
        if (arguments != null && "true".equals(arguments.getString("letter-hold"))) {
            runLetterHoldChecks();
            return;
        }
        runConfiguredChecks(false);
    }

    private void runPunctuationChecks() throws Exception {
        stage = "chinese punctuation on";
        setChinesePunctuation(true);

        stage = "idle swipe";
        swipeDown("v");
        await(field(PLAIN).and(node -> equalsText("？", node.getText())));

        stage = "composition";
        for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(described("字母 " + key.toUpperCase(java.util.Locale.ROOT)));
        await(field(PLAIN).and(node -> equalsText("？nihao", node.getText())));
        stage = "commit composition before symbol swipe";
        tap(key("空格"));
        await(field(PLAIN).and(node -> equalsText("？你好", node.getText())));
        stage = "committed composition swipe";
        swipeDown("v");
        await(field(PLAIN).and(node -> equalsText("？你好？", node.getText())));

        stage = "chinese punctuation off";
        setChinesePunctuation(false);
        stage = "ascii swipe";
        swipeDown("v");
        await(field(PLAIN).and(node -> equalsText("？你好？?", node.getText())));

        stage = "restore chinese punctuation";
        setChinesePunctuation(true);

        stage = "comma position and tap";
        Rect comma = bounds(await(describedPrefix("常用标点：")));
        Rect space = bounds(await(describedPrefix("空格")));
        Rect symbols = bounds(await(described("打开符号面板")));
        if (symbols.centerX() >= space.centerX() || comma.centerX() <= space.centerX())
            throw new AssertionError("符号入口与逗号没有分居空格两侧");
        tap(describedPrefix("常用标点："));
        await(field(PLAIN).and(node -> equalsText("？你好？?，", node.getText())));

        stage = "comma hold types one period";
        holdComma();
        await(field(PLAIN).and(node -> equalsText("？你好？?，。", node.getText())));

        stage = "comma hold commits composition";
        for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(described("字母 " + letter.toUpperCase(java.util.Locale.ROOT)));
        holdComma();
        await(field(PLAIN).and(node -> equalsText("？你好？?，。你好。", node.getText())));

        stage = "ascii comma hold";
        setChinesePunctuation(false);
        holdComma();
        await(field(PLAIN).and(node -> equalsText("？你好？?，。你好。.", node.getText())));
        setChinesePunctuation(true);

        stage = "26-key symbol panel entry";
        tap(described("打开符号面板"));
        await(described("符号面板"));
        tap(described("关闭符号面板"));
        await(described("字母 V"));
    }

    private File swipePreferences;
    private File swipeLocal;
    private JSONObject swipeBase;
    private JSONObject swipeSettings;
    private long swipeRevision;

    /** 配置只写专用模拟器中的产品沙盒，结束时恢复；输入与断言均使用合成值。 */
    private void runConfiguredChecks(boolean swipeOptions) throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        swipePreferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!swipePreferences.toPath().startsWith(root.toPath())) throw new AssertionError("偏好路径越出测试沙盒");
        swipeLocal = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = swipePreferences.isFile() ? Files.readAllBytes(swipePreferences.toPath()) : null;
        byte[] originalLocal = swipeLocal.isFile() ? Files.readAllBytes(swipeLocal.toPath()) : null;
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        swipeRevision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        swipeSettings = originalLocal == null ? new JSONObject()
            : new JSONObject(new String(originalLocal, StandardCharsets.UTF_8)).getJSONObject("settings");
        swipeBase = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
            .put("cloud_candidates", false).put("learning", false)
            .put("character_width", "halfwidth").put("traditional_chinese_output", false)
            .put("punctuation_lock", "follow")
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        try {
            if (!modes.edit().clear().commit()) throw new AssertionError("输入模式准备失败");
            configureSwipe(true, "down", true, false);
            if (!swipeOptions) {
                runPunctuationChecks();
                return;
            }
            stage = "up swipe drag uppercase";
            gestureOption("v", false, "up", "V", false);
            expect("V");
            stage = "up swipe drag lowercase";
            gestureOption("q", false, "up", "q", false);
            expect("Vq");
            tap(key("⌫"));
            tap(key("⌫"));
            expect("");
            stage = "quote choices use engine alternation";
            gestureOption("l", false, "up", "“", false);
            gestureOption("l", false, "up", "“", false);
            expect("“”");
            stage = "long hold drag uppercase";
            gestureOption("q", true, "up", "Q", false);
            expect("“”Q");
            stage = "cancel selected option";
            gestureOption("w", false, "up", "W", true);
            expect("“”Q");
            stage = "release outside the selected option";
            gestureOption("w", false, "up", "W", false, true);
            expect("“”Q");
            stage = "long hold still supports separate tap";
            holdLetterDescribed("a");
            tap(described("输入 @"));
            expect("“”Q@");

            configureSwipe(true, "down", false, false);
            stage = "ASCII option when Chinese punctuation is off";
            gestureOption("v", false, "up", "?", false);
            expect("?");
            configureSwipe(true, "up", true, false);
            stage = "configured up swipe directly enters the hint";
            simpleSwipe("v", "up");
            expect("？");
            assertNoOptions();
            stage = "opposite down swipe opens options";
            gestureOption("w", false, "down", "W", false);
            expect("？W");

            for (String direction : new String[] {"up", "down"}) {
                configureSwipe(false, "down", true, false);
                stage = "disabled " + direction + " swipe keeps the letter";
                simpleSwipe("v", direction);
                expect("v");
                assertNoOptions();
            }
            configureSwipe(false, "down", true, true);
            stage = "disabled swipes preserve long hold and paired punctuation";
            gestureOption("j", true, "up", "（", false);
            await(field(PLAIN).and(node -> equalsText("（）", node.getText())
                && node.getTextSelectionStart() == 1 && node.getTextSelectionEnd() == 1));
            configureSwipe(true, "down", true, false);
            stage = "candidate swipe is a normal click";
            tap(described("字母 N"));
            tap(described("字母 I"));
            await(describedPrefix("候选 1："));
            simpleSwipe("v", "down");
            await(field(PLAIN).and(node -> {
                String text = String.valueOf(node.getText());
                return text.contains("niv") && text.indexOf('？') < 0;
            }));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(swipePreferences.toPath()); else publish(swipePreferences, original);
            if (originalLocal == null) Files.deleteIfExists(swipeLocal.toPath()); else publish(swipeLocal, originalLocal);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("输入模式恢复失败");
            restartIme();
        }
    }

    private void configureSwipe(boolean enabled, String direction, boolean chinese, boolean paired) throws Exception {
        stage = "configure " + direction + " enabled " + enabled + " Chinese " + chinese;
        swipeBase.put("chinese_punctuation", chinese).put("paired_punctuation", paired);
        publish(swipePreferences, new JSONObject().put("format_version", 1).put("revision", ++swipeRevision)
            .put("preferences", swipeBase).toString().getBytes(StandardCharsets.UTF_8));
        swipeSettings.put("platform.android.swipe_down_symbols", enabled)
            .put("platform.android.swipe_symbols_direction", direction);
        publish(swipeLocal, new JSONObject().put("version", 1).put("settings", swipeSettings)
            .toString().getBytes(StandardCharsets.UTF_8));
        restartIme();
        openSyntheticEditor();
        tap(field(PLAIN));
        await(described("字母 V"));
        // 字母键会先于后台会话出现，先确认引擎已就绪再验证中文标点。
        tap(described("字母 N"));
        tap(described("字母 I"));
        await(describedPrefix("候选 1："));
        tap(key("⌫"));
        tap(key("⌫"));
        expect("");
    }

    private void openSyntheticEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void expect(String value) {
        await(field(PLAIN).and(node -> equalsText(value, node.getText())
            || value.isEmpty() && (node.isShowingHintText() || node.getText() == null)));
    }

    private void assertNoOptions() {
        dropStaleNodes();
        for (AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), describedPrefix("输入 ")) != null)
                throw new AssertionError("关闭或改向后仍弹出了字母选项");
    }

    private void holdLetterDescribed(String letter) throws Exception {
        Rect keyBounds = bounds(awaitStableBounds(described("字母 " + letter.toUpperCase(java.util.Locale.ROOT))));
        long started = SystemClock.uptimeMillis();
        inject(started, MotionEvent.ACTION_DOWN, keyBounds.centerX(), keyBounds.centerY());
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 100);
        await(described("输入 " + letter.toUpperCase(java.util.Locale.ROOT)));
        inject(started, MotionEvent.ACTION_MOVE, keyBounds.centerX() + 1, keyBounds.centerY());
        inject(started, MotionEvent.ACTION_UP, keyBounds.centerX() + 1, keyBounds.centerY());
    }

    /** 一次按下到抬起之间完成打开、左右移动和选择，禁止用无障碍点击替代。 */
    private void gestureOption(String letter, boolean hold, String direction, String choice, boolean cancel) throws Exception {
        gestureOption(letter, hold, direction, choice, cancel, false);
    }

    private void gestureOption(String letter, boolean hold, String direction, String choice,
            boolean cancel, boolean outside) throws Exception {
        String originalStage = stage;
        Rect keyBounds = bounds(awaitStableBounds(described("字母 " + letter.toUpperCase(java.util.Locale.ROOT))));
        long started = SystemClock.uptimeMillis();
        inject(started, MotionEvent.ACTION_DOWN, keyBounds.centerX(), keyBounds.centerY());
        if (hold) SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 100);
        else {
            float dy = 20 * getTargetContext().getResources().getDisplayMetrics().density;
            inject(started, MotionEvent.ACTION_MOVE, keyBounds.centerX(),
                keyBounds.centerY() + ("up".equals(direction) ? -dy : dy));
        }
        stage = originalStage + " open options";
        Predicate<AccessibilityNodeInfo> option = described("输入 " + choice);
        Rect choiceBounds = bounds(awaitStableBounds(option));
        String upper = letter.toUpperCase(java.util.Locale.ROOT);
        Predicate<AccessibilityNodeInfo> alternate = described("输入 " + (choice.equals(upper) ? letter : upper));
        Rect alternateBounds = bounds(awaitStableBounds(alternate));
        stage = originalStage + " hover alternate";
        inject(started, MotionEvent.ACTION_MOVE, alternateBounds.centerX(), alternateBounds.centerY());
        await(alternate.and(AccessibilityNodeInfo::isSelected));
        stage = originalStage + " hover target";
        inject(started, MotionEvent.ACTION_MOVE, choiceBounds.centerX(), choiceBounds.centerY());
        await(option.and(AccessibilityNodeInfo::isSelected));
        stage = originalStage + " release";
        inject(started, cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP,
            choiceBounds.centerX(), outside ? keyBounds.bottom + 30 : choiceBounds.centerY());
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            dropStaleNodes();
            boolean found = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), option) != null) { found = true; break; }
            if (!found) { stage = originalStage; return; }
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("选择或取消后选项未关闭");
    }

    private void simpleSwipe(String letter, String direction) throws Exception {
        Rect keyBounds = bounds(awaitStableBounds(described("字母 " + letter.toUpperCase(java.util.Locale.ROOT))));
        long started = SystemClock.uptimeMillis();
        float dy = 20 * getTargetContext().getResources().getDisplayMetrics().density;
        inject(started, MotionEvent.ACTION_DOWN, keyBounds.centerX(), keyBounds.centerY());
        inject(started, MotionEvent.ACTION_MOVE, keyBounds.centerX(),
            keyBounds.centerY() + ("up".equals(direction) ? -dy : dy));
        inject(started, MotionEvent.ACTION_UP, keyBounds.centerX(),
            keyBounds.centerY() + ("up".equals(direction) ? -dy : dy));
    }

    private void inject(long started, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(started, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(event, true)) throw new AssertionError("触摸事件未送达");
        } finally { event.recycle(); }
        SystemClock.sleep(40);
    }

    /** 通过真实触屏长按覆盖三排英文键与多行弹窗，不用产品字符表生成期望值。 */
    private void runEnglishHoldChecks() throws Exception {
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        openSyntheticEditor();
        tap(field(PLAIN));
        await(key("q"));
        if (findAcrossWindows(described("切换到英文输入")) != null)
            tap(described("切换到英文输入"));
        await(described("切换到所选输入方案"));
        if (arguments != null && "true".equals(arguments.getString("unicode18-hold"))) {
            runUnicode18SubscriptHolds(new StringBuilder());
            return;
        }
        String[] letters = {"q", "w", "e", "r", "t", "y", "u", "i", "o", "p",
            "a", "s", "d", "f", "g", "h", "j", "k", "l", "z", "x", "c", "v", "b", "n", "m"};
        String[] values = {"Q", "²", "₃", "肆", "⁵", "₆", "ū", "ǐ", "ò", "零",
            "ₐ", "ˢ", "ᵈ", "ᶠ", "ᵍ", "ₕ", "ⱼ", "ₖ", "ₗ", "ᶻ", "ₓ", "ᶜ", "ǚ", "ᵇ", "ň", "ḿ"};
        StringBuilder expected = new StringBuilder();
        for (int index = 0; index < letters.length; index++) {
            stage = "English hold " + letters[index];
            Rect anchor = bounds(await(key(letters[index])));
            holdLetter(letters[index]);
            Rect first = bounds(await(described("输入 " + letters[index].toUpperCase(java.util.Locale.ROOT))));
            if (first.top > anchor.bottom)
                throw new AssertionError("弹窗不应脱离按住的键而贴到键盘底部");
            if (findAcrossWindows(described("输入 " + letters[index])) != null)
                throw new AssertionError("长按不应重复小写");
            AccessibilityNodeInfo option = await(described("输入 " + values[index]));
            Rect rectangle = bounds(option);
            Rect display = new Rect(0, 0, getTargetContext().getResources().getDisplayMetrics().widthPixels,
                getTargetContext().getResources().getDisplayMetrics().heightPixels);
            if (rectangle.isEmpty() || !display.contains(rectangle) || !option.isVisibleToUser())
                throw new AssertionError("多行选项被屏幕裁剪");
            touch(rectangle);
            expected.append(values[index]);
            await(field(PLAIN).and(node -> equalsText(expected.toString(), node.getText())));
            await(described("切换到所选输入方案"));
        }
        runUnicode18SubscriptHolds(expected);
        stage = "English hold tone on last row";
        holdLetter("e");
        touch(bounds(await(described("输入 è"))));
        expected.append("è");
        await(field(PLAIN).and(node -> equalsText(expected.toString(), node.getText())));
        stage = "English downward swipe remains literal";
        swipeDown("w");
        expected.append("2");
        await(field(PLAIN).and(node -> equalsText(expected.toString(), node.getText())));
        stage = "O retains Chinese keyboard entry";
        holdLetter("o");
        touch(bounds(await(described("切换到拼音键盘"))));
        await(described("切换到英文输入"));
        // 回到中文后的候选提交仍留在中文，延续 O 键入口的既有规则。
        for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
        tap(key("空格"));
        expected.append("你好");
        await(field(PLAIN).and(node -> equalsText(expected.toString(), node.getText())));
        await(described("切换到英文输入"));
    }

    /** 专项可独立跑，也保留在完整英文长按套件内，验证原始码位通过真实触摸上屏。 */
    private void runUnicode18SubscriptHolds(StringBuilder expected) throws Exception {
        for (String[] entry : new String[][] {{"w", "\u209d"}, {"y", "\u209e"}, {"z", "\u209f"}}) {
            stage = "English hold Unicode 18 subscript " + entry[0];
            holdLetter(entry[0]);
            touch(bounds(await(described("输入 " + entry[1]))));
            expected.append(entry[1]);
            await(field(PLAIN).and(node -> equalsText(expected.toString(), node.getText())));
            await(described("切换到所选输入方案"));
        }
    }

    private AccessibilityNodeInfo findAcrossWindows(java.util.function.Predicate<AccessibilityNodeInfo> match) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    /** 点真实坐标，防止无障碍 ACTION_CLICK 让被窗口裁剪的选项假通过。 */
    private void touch(Rect rectangle) {
        long started = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN,
            rectangle.centerX(), rectangle.centerY(), 0);
        MotionEvent up = MotionEvent.obtain(started, started + 60, MotionEvent.ACTION_UP,
            rectangle.centerX(), rectangle.centerY(), 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(event, true)) throw new AssertionError("长按选项触摸失败");
            }
        } finally { down.recycle(); up.recycle(); }
    }

    private void runLetterHoldChecks() throws Exception {
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        openSyntheticEditor();
        tap(field(PLAIN));
        await(described("字母 Q"));
        stage = "long press uppercase choice";
        holdLetter("q");
        tap(described("输入 Q"));
        await(field(PLAIN).and(node -> equalsText("Q", node.getText())));
        stage = "long press symbol choice";
        holdLetter("a");
        tap(described("输入 @"));
        await(field(PLAIN).and(node -> equalsText("Q@", node.getText())));
        stage = "upward swipe uppercase choice";
        swipeUpLetter("w");
        tap(described("输入 W"));
        await(field(PLAIN).and(node -> equalsText("Q@W", node.getText())));
    }

    private void holdLetter(String value) throws Exception {
        Rect bounds = bounds(awaitStableBounds(key(value)));
        long started = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        try { down.setSource(InputDevice.SOURCE_TOUCHSCREEN); automation.injectInputEvent(down, true); }
        finally { down.recycle(); }
        // 像真人一样按住到选择条出现再松手：固定睡到长按时长之后就松开，输入法主线程一忙，长按回调会排在松手之后，这一下就成了点按。
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout());
        String choice = "输入 " + value.toUpperCase(java.util.Locale.ROOT);
        await(described(choice));
        MotionEvent up = MotionEvent.obtain(started, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP,
            bounds.centerX(), bounds.centerY(), 0);
        try { up.setSource(InputDevice.SOURCE_TOUCHSCREEN); automation.injectInputEvent(up, true); }
        finally { up.recycle(); }
        await(described(choice));
    }

    private void swipeUpLetter(String value) throws Exception {
        Rect bounds = bounds(awaitStableBounds(key(value)));
        float x = bounds.centerX();
        float startY = bounds.centerY();
        float endY = startY - SWIPE_DP * getTargetContext().getResources().getDisplayMetrics().density;
        long started = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN, x, startY, 0);
        MotionEvent move = MotionEvent.obtain(started, started + 40, MotionEvent.ACTION_MOVE, x, endY, 0);
        MotionEvent up = MotionEvent.obtain(started, started + 80, MotionEvent.ACTION_UP, x, endY, 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, move, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                automation.injectInputEvent(event, true);
            }
        } finally { down.recycle(); move.recycle(); up.recycle(); }
        await(described("输入 " + value.toUpperCase(java.util.Locale.ROOT)));
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect();
        node.getBoundsInScreen(result);
        return result;
    }

    /** 驱动真实按住和抬起，覆盖长按结束后不得追加普通点按。 */
    private void holdComma() throws Exception {
        automation.waitForIdle(500, 5000);
        Rect bounds = bounds(awaitStableBounds(describedPrefix("常用标点：")));
        String before = String.valueOf(await(field(PLAIN)).getText());
        long started = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        try {
            down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            if (!automation.injectInputEvent(down, true)) throw new AssertionError("长按按下失败");
        } finally { down.recycle(); }
        try {
            // 等实际长按提交再松手，避免主线程繁忙时把固定等待后的抬起误当成点按。
            await(field(PLAIN).and(node -> !before.equals(String.valueOf(node.getText()))));
        } finally {
            MotionEvent up = MotionEvent.obtain(started, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP,
                bounds.centerX(), bounds.centerY(), 0);
            try {
                up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(up, true)) throw new AssertionError("长按抬起失败");
            } finally { up.recycle(); }
        }
        SystemClock.sleep(250);
    }

    /** 在「更多」面板里把「中文标点」拨到 `enabled`，再回到键盘。这个开关只作用于当前会话，不写共享设置。 */
    private void setChinesePunctuation(boolean enabled) throws Exception {
        tap(key("更多"));
        await(toolPanel());
        String wanted = enabled ? "已开启" : "已关闭";
        if (!equalsText(wanted, switchState(await(tool("中文标点"))))) {
            tap(tool("中文标点"));
            await(toolWithState("中文标点", wanted));
        }
        tap(tool("返回键盘"));
        await(described("字母 V").and(AccessibilityNodeInfo::isClickable));
    }

    /** 在 `letter` 键中心按下，向下移过阈值后抬起。 */
    private void swipeDown(String letter) throws Exception {
        automation.waitForIdle(500, 5000);
        AccessibilityNodeInfo target = await(described("字母 " + letter.toUpperCase(java.util.Locale.ROOT)));
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        float x = bounds.centerX();
        float startY = bounds.centerY();
        float endY = startY + SWIPE_DP * getTargetContext().getResources().getDisplayMetrics().density;
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, startY, 0);
        MotionEvent move = MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_MOVE, x, endY, 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 80, MotionEvent.ACTION_UP, x, endY, 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, move, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(event, true))
                    throw new AssertionError("Swipe injection failed");
            }
        } finally {
            down.recycle();
            move.recycle();
            up.recycle();
        }
        SystemClock.sleep(150);
    }
}
