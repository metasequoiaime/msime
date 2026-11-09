package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

/** 真实编辑器里 ⇤ ⇥ 的单击行首行尾与双击文首文尾、全选上滑选择行、Shift 选择与换行，不依赖宿主内部实现。 */
final class TextEditLineDeviceChecks {
    private final DeviceSmoke t;

    TextEditLineDeviceChecks(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        int end = TrackpadEditorActivity.TEXT.length();
        open("edit");
        // TEXT 的第二行是 11..22；行按换行符分（#11111）。
        t.stage = "a tap moves to the line start or end";
        setSelection(16);
        tap(START);
        selection(11, 11);
        setSelection(16);
        tap(END);
        selection(22, 22);
        t.stage = "a double tap moves to the document start or end";
        setSelection(16);
        doubleTap(START);
        selection(0, 0);
        doubleTap(END);
        selection(end, end);
        t.stage = "a late second tap counts as a single tap";
        setSelection(16);
        tap(END);
        selection(22, 22);
        settle();
        tap(END);
        // 超时的第二下仍是单击：留在行尾，不跳到文尾。多等一会，免得在第二下生效前就判过。
        SystemClock.sleep(300);
        selection(22, 22);
        t.stage = "edge keys no longer swipe up";
        // 不用 16：下一段要 setSelection(16)，选区没变时无障碍的设置选区返回 false。
        setSelection(13);
        swipeUp(START);
        swipeUp(END);
        selection(13, 13);
        t.stage = "select all keeps its upward swipe to select the line";
        // 上滑改由「选行」角标提示（只画、不进节点），三个键都不再有长按提示气泡。
        for (String key : new String[] {"全选", START, END})
            t.await(t.described(key).and(node -> android.os.Build.VERSION.SDK_INT < 28 || node.getTooltipText() == null));
        setSelection(16);
        swipeUp("全选");
        selection(11, 22);
        t.stage = "selection mode extends to the line, then to the document";
        setSelection(16);
        tap("选择模式");
        tap(START);
        selection(11, 16);
        settle();
        doubleTap(START);
        selection(0, 16);
        tap("取消选择");
        setSelection(16);
        tap("选择模式");
        tap(END);
        selection(16, 22);
        settle();
        doubleTap(END);
        selection(16, end);
        tap("取消选择");
        t.stage = "edge keys stay usable in trackpad mode";
        tap("触控板");
        t.await(t.described("触控板").and(AccessibilityNodeInfo::isChecked));
        setSelection(16);
        tap(START);
        selection(11, 11);
        doubleTap(END);
        selection(end, end);
        tap("触控板");
        t.await(t.described("触控板").and(node -> !node.isChecked()));

        open("wrapped");
        t.stage = "a wrapped paragraph moves to its start and end, not the displayed line's";
        // WRAPPED_TEXT 没有换行符，在 160 dp 宽的输入框里自动折行；宿主报告第二个显示行的边界。
        int[] wrap = wrapLine();
        int paragraphEnd = TrackpadEditorActivity.WRAPPED_TEXT.length();
        if (wrap[1] <= 0 || wrap[2] >= paragraphEnd - 1) throw new AssertionError("Wrapped fixture did not wrap");
        // 先挪开：输入框打开时的光标可能正好在 wrap[0]，选区不变时设置选区会返回 false。
        setSelection(paragraphEnd);
        setSelection(wrap[0]);
        tap(END);
        selection(paragraphEnd, paragraphEnd);
        setSelection(wrap[0]);
        tap(START);
        selection(0, 0);

        open("terminal");
        t.stage = "edge keys fall back to Home and End without surrounding text";
        // 读不到全文时单击退回不带修饰键的 Home / End；双击的第二下是文档首尾，带 Ctrl（META_CTRL_ON | META_CTRL_LEFT_ON = 12288）；选择模式再加 Shift（65）。
        tap(START);
        t.await(t.field("trackpad-status").and(node -> DeviceSmoke.equalsText("0:122:0,1:122:0", node.getText())));
        settle();
        doubleTap(START);
        status("0:122:0,1:122:0,0:122:0,1:122:0,0:122:12288,1:122:12288");
        tap("选择模式");
        tap(END);
        status("0:123:65,1:123:65");
        settle();
        doubleTap(END);
        status("0:123:12353,1:123:12353");

        open("edit");
        t.stage = "return short tap inserts exactly one newline";
        setSelection(TrackpadEditorActivity.TEXT.length());
        Rect key = returnBounds();
        gesture(key, false);
        text(TrackpadEditorActivity.TEXT + "\n");
        t.stage = "return slide out cancels the newline";
        key = returnBounds();
        gesture(key, true);
        text(TrackpadEditorActivity.TEXT + "\n");
        if (visible("符号面板")) throw new AssertionError("Sliding out opened symbols");
        // 换行键只管换行：长按不打开符号面板，松手照常换一行，面板留着。
        t.stage = "return long press only inserts one newline";
        key = returnBounds();
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        SystemClock.sleep(android.view.ViewConfiguration.getLongPressTimeout() * 2L);
        touch(down, MotionEvent.ACTION_UP, key.centerX(), key.centerY());
        t.automation.waitForIdle(300, 5000);
        text(TrackpadEditorActivity.TEXT + "\n\n");
        if (visible("符号面板")) throw new AssertionError("Return long press opened symbols");
        if (!visible("文本编辑面板")) throw new AssertionError("Return long press closed the text edit panel");
    }

    /** ⇤ ⇥ 的读屏名称（TextEditPanelModel）：先说单击，再说双击。 */
    private static final String START = "移到行首，双击移到开头";
    private static final String END = "移到行尾，双击移到结尾";

    /** 等过 ⇤ ⇥ 的双击窗口（TextEditEdgeTapPolicy，400 ms），下一下按单击算。 */
    private static void settle() { SystemClock.sleep(600); }

    /** 在键中心连点两下，两下之间远短于双击窗口；无障碍点按要等界面空闲，两下会隔得太久。 */
    private void doubleTap(String description) throws Exception {
        Rect key = new Rect();
        t.awaitStableBounds(t.described(description)).getBoundsInScreen(key);
        for (int i = 0; i < 2; i++) {
            long down = SystemClock.uptimeMillis();
            touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
            touch(down, MotionEvent.ACTION_UP, key.centerX(), key.centerY());
            if (i == 0) SystemClock.sleep(60);
        }
        t.automation.waitForIdle(300, 5000);
    }

    /** 折行输入框报告的第二个显示行：{光标放这里, 显示行起点, 显示行终点}。 */
    private int[] wrapLine() {
        AccessibilityNodeInfo status = t.await(t.field("trackpad-status").and(node -> node.getText() != null
            && node.getText().toString().startsWith("wrap-anchor=")));
        String[] parts = status.getText().toString().split(";");
        int[] values = new int[3];
        for (int i = 0; i < 3; i++) values[i] = Integer.parseInt(parts[i].substring(parts[i].indexOf('=') + 1));
        return values;
    }

    private void status(String contains) {
        t.await(t.field("trackpad-status").and(node -> node.getText() != null
            && node.getText().toString().contains(contains)));
    }

    private void open(String kind) throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/"
            + "app.msime.android.test.TrackpadEditorActivity --es kind " + kind);
        t.dropStaleNodes();
        t.await(t.field("trackpad-editor"));
        t.await(t.key("n"));
        tap("文本编辑");
        t.await(t.described("文本编辑面板"));
    }

    private void tap(String description) throws Exception {
        t.dropStaleNodes();
        t.tap(t.described(description).and(AccessibilityNodeInfo::isClickable));
    }

    private void setSelection(int at) {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, at);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, at);
        if (!t.await(t.field("trackpad-editor")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic cursor setup failed");
        selection(at, at);
    }

    private void selection(int start, int end) {
        t.await(t.field("trackpad-editor").and(node ->
            Math.min(node.getTextSelectionStart(), node.getTextSelectionEnd()) == start
                && Math.max(node.getTextSelectionStart(), node.getTextSelectionEnd()) == end));
    }

    private void text(String expected) {
        t.await(t.field("trackpad-editor").and(node -> DeviceSmoke.equalsText(expected, node.getText())));
    }

    private Rect returnBounds() {
        Rect bounds = new Rect();
        t.awaitStableBounds(t.describedPrefix("插入换行")).getBoundsInScreen(bounds);
        return bounds;
    }

    private void swipeUp(String description) throws Exception {
        Rect key = new Rect();
        t.awaitStableBounds(t.described(description)).getBoundsInScreen(key);
        float distance = 40 * t.getTargetContext().getResources().getDisplayMetrics().density;
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        touch(down, MotionEvent.ACTION_MOVE, key.centerX(), key.centerY() - distance);
        touch(down, MotionEvent.ACTION_UP, key.centerX(), key.centerY() - distance);
        t.automation.waitForIdle(300, 5000);
    }

    private void gesture(Rect key, boolean cancel) throws Exception {
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        float x = cancel ? key.left - key.width() : key.centerX();
        if (cancel) touch(down, MotionEvent.ACTION_MOVE, x, key.centerY());
        touch(down, MotionEvent.ACTION_UP, x, key.centerY());
        t.automation.waitForIdle(300, 5000);
    }

    private void touch(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(event, false)) throw new AssertionError("Synthetic touch rejected");
        } finally { event.recycle(); }
    }

    private boolean visible(String description) {
        t.dropStaleNodes();
        for (AccessibilityWindowInfo window : t.automation.getWindows())
            if (t.find(window.getRoot(), t.described(description)) != null) return true;
        return false;
    }
}
