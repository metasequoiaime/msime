package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;

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
        return "letter hints and comma hold route punctuation once, commit composition and preserve the symbol panel entry";
    }

    @Override protected void runChecks() throws Exception {
        if (arguments != null && "true".equals(arguments.getString("english-hold"))) {
            runEnglishHoldChecks();
            return;
        }
        if (arguments != null && "true".equals(arguments.getString("letter-hold"))) {
            runLetterHoldChecks();
            return;
        }
        // 前置生命周期用例可能强停宿主，先恢复系统输入法绑定再打开编辑器。
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "focus";
        tap(field(PLAIN));
        await(field(PLAIN).and(AccessibilityNodeInfo::isFocused));
        await(key("v").and(AccessibilityNodeInfo::isClickable));

        stage = "chinese punctuation on";
        setChinesePunctuation(true);

        stage = "idle swipe";
        swipeDown("v");
        await(field(PLAIN).and(node -> equalsText("？", node.getText())));

        stage = "composition";
        for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
        await(field(PLAIN).and(node -> equalsText("？nihao", node.getText())));
        stage = "composition swipe";
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
        for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
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
        await(key("v"));
    }

    /** 通过真实触屏长按覆盖三排英文键与多行弹窗，不用产品字符表生成期望值。 */
    private void runEnglishHoldChecks() throws Exception {
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        startActivitySync(new Intent(getTargetContext(), EditorActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        tap(field(PLAIN));
        await(key("q"));
        if (findAcrossWindows(described("切换到英文输入")) != null)
            tap(described("切换到英文输入"));
        await(described("切换到所选输入方案"));
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
        startActivitySync(new Intent(getTargetContext(), EditorActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        tap(field(PLAIN));
        await(key("q"));
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
        Rect bounds = bounds(await(describedPrefix("常用标点：")));
        long started = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        try {
            down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            if (!automation.injectInputEvent(down, true)) throw new AssertionError("长按按下失败");
        } finally { down.recycle(); }
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 180);
        MotionEvent up = MotionEvent.obtain(started, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP,
            bounds.centerX(), bounds.centerY(), 0);
        try {
            up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            if (!automation.injectInputEvent(up, true)) throw new AssertionError("长按抬起失败");
        } finally { up.recycle(); }
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
        await(key("v").and(AccessibilityNodeInfo::isClickable));
    }

    /** 在 `letter` 键中心按下，向下移过阈值后抬起。 */
    private void swipeDown(String letter) throws Exception {
        automation.waitForIdle(500, 5000);
        AccessibilityNodeInfo target = await(key(letter));
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
