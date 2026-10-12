package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

/**
 * 字母键角标在中文标点下打中文标点（#7069）。
 *
 * <p>中文标点生效时，下滑 v 交给 Engine 的标点路由：空闲时上屏「？」，组字中先按首选结束再接「？」，得到「你好？」而不是按字面上屏的拼音原文加 `?`；用工具面板关掉中文标点后同一手势仍按字面上屏 `?`。JVM 冒烟只覆盖键面表，覆盖不到经 Engine 结束组字这一步，所以在设备上驱动真实手势。下滑靠注入按下、移动、抬起三个触屏事件（「滑动输入符号」默认开、方向默认下滑），无障碍点击没有移动这一步。
 */
public final class LetterHintPunctuationDeviceSmoke extends DeviceSmoke {
    private static final String PLAIN = "msime-test-plain";
    /** 下滑距离（dp）：比 `SwipeHintPolicy.THRESHOLD_DP` 的 14 dp 大一截，不会落在阈值边上。 */
    private static final float SWIPE_DP = 28f;

    @Override protected String successDescription() {
        return "letter hint swipe types Chinese punctuation through the Engine and ASCII once Chinese punctuation is off";
    }

    @Override protected void runChecks() throws Exception {
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
