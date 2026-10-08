package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;

/**
 * 删除键上滑时在键上方弹出的「快速删除」框（#5585）。位置和待命的判断都在 {@link BackspaceSwipePolicy}，这里只负责把删除键换算到自己的坐标系并画框：没待命时是按键底色加细边，待命时换成强调色，表示松手就会删掉光标前的全部文字。不接收触摸，读屏也不念它，手势仍由删除键自己的触摸监听处理。
 */
final class QuickDeleteOverlay extends View {
    static final String LABEL = "快速删除";

    private final MSIMEInputService s;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final int[] ownLocation = new int[2];
    private final int[] keyLocation = new int[2];
    private BackspaceSwipePolicy.Box box;
    private boolean armed;

    QuickDeleteOverlay(MSIMEInputService s) {
        super(s);
        this.s = s;
        ViewPolicy.hide(this);
        ViewPolicy.setNonInteractive(this);
        ViewPolicy.hideFromAccessibility(this);
    }

    /**
     * 坐标系的参照：本视图铺满父视图（键盘外框）、左上角与它重合。按父视图换算，因为本视图平时是 GONE，第一次弹出前从没排版过，自己的位置和宽度都还是 0。
     */
    private View frame() {
        return getParent() instanceof View parent ? parent : this;
    }

    /** 删除键上沿在本视图坐标系里的 y。 */
    float keyTop(View key) {
        frame().getLocationOnScreen(ownLocation);
        key.getLocationOnScreen(keyLocation);
        return keyLocation[1] - ownLocation[1];
    }

    /** 在 `key` 上方显示框；`armed` 为真时画成待命的样子。 */
    void show(View key, boolean armed) {
        View frame = frame();
        frame.getLocationOnScreen(ownLocation);
        key.getLocationOnScreen(keyLocation);
        float left = keyLocation[0] - ownLocation[0];
        float top = keyLocation[1] - ownLocation[1];
        box = BackspaceSwipePolicy.box(left, top, left + key.getWidth(), frame.getWidth(),
            KeyboardGeometry.density(getContext()));
        this.armed = armed;
        if (getParent() instanceof ViewGroup parent) parent.bringChildToFront(this);
        ViewPolicy.show(this);
        invalidate();
    }

    void hide() {
        box = null;
        armed = false;
        ViewPolicy.hide(this);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        BackspaceSwipePolicy.Box current = box;
        if (current == null) return;
        float density = KeyboardGeometry.density(getContext());
        float radius = KeyboardGeometry.floatPixels(10, density);
        KeyboardSkin skin = s.imeStyler.themed(s.skin);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.parseColor(armed ? skin.accent() : skin.keyBackground()));
        // 和日语九键的 flick 浮层一样先带一层投影：框要看得出压在键盘上面，而不是键盘本身的一部分。
        paint.setShadowLayer(KeyboardGeometry.floatPixels(10, density), 0,
            KeyboardGeometry.floatPixels(3, density), 0x40000000);
        canvas.drawRoundRect(current.left(), current.top(), current.right(), current.bottom(),
            radius, radius, paint);
        paint.clearShadowLayer();
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(BoundsPolicy.bounded(density, 1f, Float.MAX_VALUE));
        paint.setColor(Color.parseColor(skin.hairline()));
        canvas.drawRoundRect(current.left(), current.top(), current.right(), current.bottom(),
            radius, radius, paint);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.parseColor(armed ? skin.onAccent() : skin.keyForeground()));
        paint.setTextSize(KeyboardGeometry.keySp(getContext(), 15));
        paint.setTypeface(Typeface.DEFAULT_BOLD);
        Paint.FontMetrics metrics = paint.getFontMetrics();
        float baseline = (current.top() + current.bottom()) / 2 - (metrics.ascent + metrics.descent) / 2;
        float width = paint.measureText(LABEL);
        canvas.drawText(LABEL, (current.left() + current.right() - width) / 2, baseline, paint);
    }

    @Override public boolean onTouchEvent(MotionEvent event) { return false; }
}
