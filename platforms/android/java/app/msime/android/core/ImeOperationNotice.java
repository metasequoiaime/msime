package app.msime.android;

import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.drawable.ColorDrawable;
import android.os.Build;
import android.view.Gravity;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityManager;
import android.widget.PopupWindow;
import android.widget.TextView;

/** 剪贴板添加常用语的临时结果：借用输入法窗口，不占键盘布局、不取编辑器焦点。 */
final class ImeOperationNotice {
    private final MSIMEInputService s;
    private long generation;
    private PopupWindow popup;
    private Runnable dismissTask;

    ImeOperationNotice(MSIMEInputService service) { s = service; }

    long begin() {
        cancel();
        return generation;
    }

    void cancel() {
        generation++;
        if (dismissTask != null) s.main.removeCallbacks(dismissTask);
        dismissTask = null;
        if (popup != null) popup.dismiss();
        popup = null;
    }

    void show(long request, String message) {
        View root = s.keyboardRoot;
        if (request != generation || message == null || message.isEmpty()
                || root == null || !root.isAttachedToWindow() || !s.isInputViewShown()) return;
        TextView label = new TextView(s);
        label.setText(message);
        label.setTextSize(15);
        label.setTextColor(Color.WHITE);
        label.setGravity(Gravity.CENTER);
        label.setMaxLines(2);
        label.setPadding(s.pixels(16), s.pixels(10), s.pixels(16), s.pixels(10));
        label.setBackground(DrawablePolicy.outlined(Color.BLACK, s.pixels(12), 0, Color.BLACK));
        label.setContentDescription("常用语操作提示：" + message);
        label.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        Rect frame = new Rect();
        root.getWindowVisibleDisplayFrame(frame);
        int width = Math.max(1, Math.min(s.pixels(400), frame.width() - s.pixels(32)));
        label.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.AT_MOST),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        PopupWindow next = new PopupWindow(label, label.getMeasuredWidth(), label.getMeasuredHeight(), false);
        next.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
        next.setTouchable(false);
        next.setOutsideTouchable(false);
        next.setInputMethodMode(PopupWindow.INPUT_METHOD_NOT_NEEDED);
        // 关闭父窗口裁剪；API 28 也必须能越过位于屏幕底部的 IME 窗口。
        next.setClippingEnabled(false);
        int[] screen = new int[2];
        int[] window = new int[2];
        root.getLocationOnScreen(screen);
        root.getLocationInWindow(window);
        int top = Math.max(0, frame.top);
        WindowInsets insets = root.getRootWindowInsets();
        if (insets != null && insets.getDisplayCutout() != null)
            top = Math.max(top, insets.getDisplayCutout().getSafeInsetTop());
        int x = frame.left + (frame.width() - label.getMeasuredWidth()) / 2 - screen[0] + window[0];
        int y = top + s.pixels(8) - screen[1] + window[1];
        try {
            next.showAtLocation(root, Gravity.TOP | Gravity.LEFT, x, y);
        } catch (WindowManager.BadTokenException | IllegalStateException unavailable) {
            next.dismiss();
            return;
        }
        popup = next;
        int duration = 3000;
        AccessibilityManager accessibility = s.getSystemService(AccessibilityManager.class);
        if (Build.VERSION.SDK_INT >= 29 && accessibility != null)
            duration = accessibility.getRecommendedTimeoutMillis(duration, AccessibilityManager.FLAG_CONTENT_TEXT);
        dismissTask = () -> { if (request == generation) cancel(); };
        s.main.postDelayed(dismissTask, duration);
    }
}
