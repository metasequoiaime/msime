package app.msime.android;

import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.PixelFormat;
import android.os.Build;
import android.view.Gravity;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityManager;
import android.widget.TextView;

/** 剪贴板添加常用语的临时结果：借用输入法窗口，不占键盘布局、不取编辑器焦点。 */
final class ImeOperationNotice {
    private final MSIMEInputService s;
    private long generation;
    private TextView noticeView;
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
        if (noticeView != null) {
            try { s.getSystemService(WindowManager.class).removeViewImmediate(noticeView); }
            catch (IllegalArgumentException detached) { /* 系统已撤销输入法令牌。 */ }
            noticeView = null;
        }
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
        label.setMaxLines(3);
        label.setEllipsize(android.text.TextUtils.TruncateAt.END);
        label.setPadding(s.pixels(16), s.pixels(10), s.pixels(16), s.pixels(10));
        label.setBackground(DrawablePolicy.rounded(Color.BLACK, s.pixels(12)));
        label.setContentDescription("常用语操作提示：" + message);
        label.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        // 不可触摸窗口会被系统从无障碍窗口列表里过滤；提示可点按关闭，但始终不取焦点。
        label.setOnClickListener(ignored -> cancel());
        Rect frame = new Rect();
        root.getWindowVisibleDisplayFrame(frame);
        int width = Math.max(1, Math.min(s.pixels(400), frame.width() - s.pixels(32)));
        label.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.AT_MOST),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        // 使用输入法自身令牌的独立窗口；应用子窗口在 API 28 上无法可靠越过 IME 父窗口。
        WindowManager.LayoutParams params = new WindowManager.LayoutParams(
            label.getMeasuredWidth(), WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_INPUT_METHOD_DIALOG,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT);
        params.token = s.getWindow().getWindow().getAttributes().token;
        params.gravity = Gravity.TOP | Gravity.CENTER_HORIZONTAL;
        params.setTitle("MSIME common phrase notice");
        int top = Math.max(0, frame.top);
        WindowInsets insets = root.getRootWindowInsets();
        if (insets != null && insets.getDisplayCutout() != null)
            top = Math.max(top, insets.getDisplayCutout().getSafeInsetTop());
        params.y = top + s.pixels(8);
        try {
            s.getSystemService(WindowManager.class).addView(label, params);
        } catch (WindowManager.BadTokenException | IllegalStateException unavailable) {
            return;
        }
        noticeView = label;
        int duration = 3000;
        AccessibilityManager accessibility = s.getSystemService(AccessibilityManager.class);
        if (Build.VERSION.SDK_INT >= 29 && accessibility != null)
            duration = accessibility.getRecommendedTimeoutMillis(duration, AccessibilityManager.FLAG_CONTENT_TEXT);
        dismissTask = () -> { if (request == generation) cancel(); };
        s.main.postDelayed(dismissTask, duration);
    }
}
