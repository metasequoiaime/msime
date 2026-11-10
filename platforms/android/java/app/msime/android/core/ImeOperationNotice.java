package app.msime.android;

import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.PixelFormat;
import android.os.Build;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewParent;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityManager;
import android.widget.TextView;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.Toast;

/** 剪贴板添加常用语的临时结果：独立显示，不占键盘布局、不取编辑器焦点。 */
final class ImeOperationNotice {
    private final MSIMEInputService s;
    private long generation;
    private TextView noticeView;
    private Toast legacyToast;
    private Runnable dismissTask;
    private View acknowledgementOverlay;
    private boolean acknowledgementWindow;

    ImeOperationNotice(MSIMEInputService service) { s = service; }

    long begin() {
        cancel();
        return generation;
    }

    void cancel() {
        generation++;
        if (dismissTask != null) s.main.removeCallbacks(dismissTask);
        dismissTask = null;
        if (legacyToast != null) legacyToast.cancel();
        legacyToast = null;
        if (noticeView != null) {
            try { s.getSystemService(WindowManager.class).removeViewImmediate(noticeView); }
            catch (IllegalArgumentException detached) { /* 系统已撤销输入法令牌。 */ }
            noticeView = null;
        }
        if (acknowledgementOverlay != null) {
            if (acknowledgementWindow) {
                try { s.getSystemService(WindowManager.class).removeViewImmediate(acknowledgementOverlay); }
                catch (IllegalArgumentException detached) { /* 系统已撤销输入法令牌。 */ }
            } else {
                ViewParent parent = acknowledgementOverlay.getParent();
                if (parent instanceof ViewGroup group) group.removeView(acknowledgementOverlay);
            }
            acknowledgementOverlay = null;
            acknowledgementWindow = false;
        }
    }

    /** 在不改变输入法窗口测量边界的独立层中显示需要确认的操作提示。 */
    void showAcknowledgement(String message) {
        cancel();
        View root = s.keyboardRoot;
        if (message == null || message.isEmpty() || root == null || !root.isAttachedToWindow()
                || !s.isInputViewShown()) return;
        LinearLayout card = LayoutPolicy.column(s);
        card.setPadding(s.pixels(20), s.pixels(16), s.pixels(20), s.pixels(8));
        card.setBackground(DrawablePolicy.rounded(Color.BLACK, s.pixels(12)));
        TextView label = new TextView(s);
        label.setText(message);
        label.setTextSize(16);
        label.setTextColor(Color.WHITE);
        label.setGravity(Gravity.CENTER);
        label.setMaxLines(4);
        label.setContentDescription(message);
        label.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        card.addView(label, LayoutPolicy.matchWidthWrapParams());
        Button done = new Button(s);
        done.setText("知道了");
        done.setTextColor(Color.WHITE);
        done.setContentDescription("知道了");
        done.setOnClickListener(ignored -> cancel());
        LinearLayout.LayoutParams doneParams = LayoutPolicy.wrapParams();
        doneParams.gravity = Gravity.END;
        card.addView(done, doneParams);

        if (Build.VERSION.SDK_INT >= 30) {
            WindowManager.LayoutParams params = new WindowManager.LayoutParams(
                Math.min(s.pixels(360), Math.max(s.pixels(240), root.getWidth() - s.pixels(32))),
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_APPLICATION_ATTACHED_DIALOG,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                    | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                PixelFormat.TRANSLUCENT);
            params.token = root.getWindowToken();
            params.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING;
            params.gravity = Gravity.CENTER;
            params.setTitle("MSIME clipboard insertion notice");
            params.flags = ScreenCaptureProtectionPolicy.windowFlags(params.flags, s.screenCaptureProtected);
            try {
                s.getSystemService(WindowManager.class).addView(card, params);
            } catch (WindowManager.BadTokenException | IllegalStateException unavailable) {
                return;
            }
            acknowledgementOverlay = card;
            acknowledgementWindow = true;
            return;
        }

        // Android 9 的输入法子窗口会被父窗口裁剪，退回同一输入法表面的覆盖层；它仍不参与主体测量。
        FrameLayout overlay = new FrameLayout(s);
        overlay.setClickable(true);
        overlay.addView(card, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER));
        if (root instanceof ViewGroup group) {
            group.addView(overlay, LayoutPolicy.frameMatchParentParams());
            acknowledgementOverlay = overlay;
            acknowledgementWindow = false;
        }
    }

    // Toast.setView 只用于 Android 11 之前仍支持自定义顶部提示的分支。
    @SuppressWarnings("deprecation")
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
        // 现代子窗口允许点按关闭以保留无障碍可见性；旧版 Toast 由系统处理触摸。
        label.setOnClickListener(ignored -> cancel());
        Rect frame = new Rect();
        root.getWindowVisibleDisplayFrame(frame);
        int width = Math.max(1, Math.min(s.pixels(400), frame.width() - s.pixels(32)));
        label.setMaxWidth(width);
        label.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.AT_MOST),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        int top = Math.max(0, frame.top);
        WindowInsets insets = root.getRootWindowInsets();
        if (insets != null && insets.getDisplayCutout() != null)
            top = Math.max(top, insets.getDisplayCutout().getSafeInsetTop());
        if (Build.VERSION.SDK_INT < 30) {
            // Android 9 会裁剪输入法父窗口外的子窗口；旧版 Toast 仍支持顶部定位。
            legacyToast = new Toast(s);
            legacyToast.setView(label);
            legacyToast.setGravity(Gravity.TOP | Gravity.CENTER_HORIZONTAL, 0, top + s.pixels(8));
            legacyToast.setDuration(Toast.LENGTH_LONG);
            legacyToast.show();
        } else {
            // Android 11 起文字 Toast 忽略定位，使用不取焦点的输入法子窗口。
            WindowManager.LayoutParams params = new WindowManager.LayoutParams(
                label.getMeasuredWidth(), WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_APPLICATION_ATTACHED_DIALOG,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                    | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                PixelFormat.TRANSLUCENT);
            params.token = root.getWindowToken();
            params.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING;
            params.gravity = Gravity.TOP | Gravity.CENTER_HORIZONTAL;
            params.setTitle("MSIME common phrase notice");
            // 提示是输入法窗口之外单开的子窗口，「防止截屏」要单独给它加 FLAG_SECURE（#10098）。
            params.flags = ScreenCaptureProtectionPolicy.windowFlags(params.flags, s.screenCaptureProtected);
            params.y = top + s.pixels(8);
            try {
                s.getSystemService(WindowManager.class).addView(label, params);
            } catch (WindowManager.BadTokenException | IllegalStateException unavailable) {
                return;
            }
            noticeView = label;
        }
        int duration = 3000;
        AccessibilityManager accessibility = s.getSystemService(AccessibilityManager.class);
        if (Build.VERSION.SDK_INT >= 29 && accessibility != null)
            duration = accessibility.getRecommendedTimeoutMillis(duration, AccessibilityManager.FLAG_CONTENT_TEXT);
        dismissTask = () -> { if (request == generation) cancel(); };
        s.main.postDelayed(dismissTask, duration);
    }
}
