package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.view.Gravity;
import android.view.View;
import android.widget.PopupWindow;

/** 输入法自己的文字放大镜；只重画有界文字，不读取其他应用的像素。 */
public final class TextTrackpadMagnifier {
    private final PreviewView content;
    private final PopupWindow popup;
    private final float density;
    private View pendingKeyboard;
    private Runnable pendingShow;

    public TextTrackpadMagnifier(Context context) {
        density = context.getResources().getDisplayMetrics().density;
        content = new PreviewView(context);
        popup = new PopupWindow(content, 1, 1, false);
        popup.setTouchable(false);
        popup.setClippingEnabled(false);
        popup.setElevation(8 * density);
        popup.setInputMethodMode(PopupWindow.INPUT_METHOD_NOT_NEEDED);
    }

    public void show(View keyboard, RectF anchor, TextTrackpadText.Preview preview,
            int background, int foreground, int accent) {
        clearPendingShow();
        if (keyboard == null || !keyboard.isAttachedToWindow() || keyboard.getWidth() == 0) return;
        if (keyboard.isLayoutRequested() || keyboard.getRootView().isLayoutRequested()) {
            pendingKeyboard = keyboard;
            pendingShow = () -> show(keyboard, anchor, preview, background, foreground, accent);
            keyboard.postOnAnimation(pendingShow);
            return;
        }
        int margin = Math.round(8 * density);
        int screenWidth = keyboard.getResources().getDisplayMetrics().widthPixels;
        int screenHeight = keyboard.getResources().getDisplayMetrics().heightPixels;
        int width = Math.min(Math.round(280 * density), screenWidth - margin * 2);
        int height = Math.round(76 * density);
        android.view.WindowInsets insets = keyboard.getRootWindowInsets();
        android.graphics.Rect visible = new android.graphics.Rect();
        keyboard.getWindowVisibleDisplayFrame(visible);
        // 输入法窗口的局部 Insets 可不含顶部系统栏；可见显示区域提供屏幕上的安全边界。
        int safeTop = Math.max(0, visible.top) + margin;
        if (insets != null) {
            safeTop = Math.max(safeTop, WindowLayout.systemBars(insets).top + margin);
            if (insets.getDisplayCutout() != null)
                safeTop = Math.max(safeTop, insets.getDisplayCutout().getSafeInsetTop() + margin);
        }
        if (width <= 0 || screenHeight <= height) return;
        int[] location = new int[2];
        keyboard.getLocationOnScreen(location);
        boolean anchored = anchor != null && anchor.top >= safeTop - margin && anchor.bottom < location[1] - margin;
        float center = anchored ? anchor.centerX() : location[0] + keyboard.getWidth() / 2f;
        int x = Math.max(margin, Math.min(screenWidth - width - margin, Math.round(center - width / 2f)));
        int y = anchored ? Math.round(anchor.top) - height - margin : location[1] - height - margin;
        if (y < safeTop && anchored) y = Math.round(anchor.bottom) + margin;
        if (y + height >= location[1]) y = location[1] - height - margin;
        y = Math.max(safeTop, Math.min(screenHeight - height - margin, y));
        // PopupWindow 按父窗口坐标布局，光标与键盘位置是屏幕坐标；窗口原点并不总在屏幕顶部。
        int[] inWindow = new int[2];
        keyboard.getLocationInWindow(inWindow);
        x += inWindow[0] - location[0];
        y += inWindow[1] - location[1];
        content.update(preview, background, foreground, accent, width);
        try {
            if (popup.isShowing()) popup.update(x, y, width, height);
            else {
                popup.setWidth(width);
                popup.setHeight(height);
                popup.showAtLocation(keyboard, Gravity.TOP | Gravity.LEFT, x, y);
            }
        } catch (RuntimeException ignored) {
            // 输入法窗口可在跨进程回读期间失去 token，迟到预览直接丢弃。
            hide();
        }
    }

    private void clearPendingShow() {
        if (pendingKeyboard != null && pendingShow != null) pendingKeyboard.removeCallbacks(pendingShow);
        pendingKeyboard = null;
        pendingShow = null;
    }

    public void hide() {
        clearPendingShow();
        if (popup.isShowing()) popup.dismiss();
        content.clear();
    }

    private static final class PreviewView extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
        private final float density;
        private StaticLayout layout;
        private TextTrackpadText.Preview preview;
        private int background;
        private int accent;

        PreviewView(Context context) {
            super(context);
            density = context.getResources().getDisplayMetrics().density;
            textPaint.setTextSize(android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
                22, context.getResources().getDisplayMetrics()));
            setContentDescription("光标文字放大镜");
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        }

        void update(TextTrackpadText.Preview value, int background, int foreground, int accent, int width) {
            this.preview = value;
            this.background = background;
            this.accent = accent;
            textPaint.setColor(foreground);
            String text = value == null ? "当前编辑器不提供文字" : value.text();
            layout = StaticLayout.Builder.obtain(text, 0, text.length(), textPaint, Math.max(1, width - Math.round(24 * density)))
                .setAlignment(Layout.Alignment.ALIGN_NORMAL).setIncludePad(false).build();
            invalidate();
        }

        void clear() {
            preview = null;
            layout = null;
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            paint.setColor(background);
            paint.setStyle(Paint.Style.FILL);
            RectF frame = new RectF(density, density, getWidth() - density, getHeight() - density);
            canvas.drawRoundRect(frame, 14 * density, 14 * density, paint);
            paint.setColor(accent);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(density);
            canvas.drawRoundRect(frame, 14 * density, 14 * density, paint);
            paint.setStyle(Paint.Style.FILL);
            if (layout == null) return;
            int focus = preview == null ? 0 : preview.selectionEnd();
            int line = layout.getLineForOffset(focus);
            float top = (getHeight() - (layout.getLineBottom(line) - layout.getLineTop(line))) / 2f - layout.getLineTop(line);
            canvas.save();
            canvas.clipRect(10 * density, 8 * density, getWidth() - 10 * density, getHeight() - 8 * density);
            canvas.translate(12 * density, top);
            if (preview != null && preview.selectionStart() != preview.selectionEnd()) {
                Path selection = new Path();
                layout.getSelectionPath(Math.min(preview.selectionStart(), preview.selectionEnd()),
                    Math.max(preview.selectionStart(), preview.selectionEnd()), selection);
                paint.setColor((accent & 0x00FFFFFF) | 0x44000000);
                canvas.drawPath(selection, paint);
            }
            layout.draw(canvas);
            if (preview != null) {
                paint.setColor(accent);
                float x = layout.getPrimaryHorizontal(focus);
                canvas.drawRect(x, layout.getLineTop(line), x + 1.5f * density, layout.getLineBottom(line), paint);
            }
            canvas.restore();
        }
    }
}
