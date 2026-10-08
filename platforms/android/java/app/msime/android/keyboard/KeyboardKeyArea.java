package app.msime.android;

import android.content.Context;
import android.graphics.Matrix;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import java.util.function.Predicate;

/**
 * 按键区（26 键各行、九键的标点列和网格、底部功能行）的容器：按下落在键与键之间的空隙时，交给拥有这段空隙的那个键。
 *
 * <p>键距和行距是每个键的布局外边距。外边距不属于按钮，按下落在那里时没有子视图接收，行和它的祖先都不可点击，整个手势就悄无声息地丢了：默认间距下 26 键约四分之一、九键约五分之一的面积是这样的死区，打字快、落点不准时就像随机丢键。多指连按时第二根手指落进空隙，框架还会把它并进第一根手指按着的键，两次按键都可能丢。
 *
 * <p>装了 {@link GlideTracker} 时，每个事件先交给它：它接管一次触摸（滑行输入）后，这里给子视图补一个 CANCEL，之后这一次触摸的事件都不再往下分发，直到最后一根手指抬起。
 *
 * <p>这里不改布局也不改绘制，画面和以前逐像素相同：只在 `ACTION_DOWN` / `ACTION_POINTER_DOWN` 落在某个键的外边距里、又没有落在任何键或其它可点击视图上时，把这一个指针的按下坐标挪进那个键的边缘（挪动距离就是空隙宽度），再交给 `LinearLayout` 照常分发。这个挪动量按指针 id 记下，之后同一个指针的 MOVE、POINTER_UP、UP、CANCEL（连同其中的历史采样）都挪同样的距离，直到它抬起。键看到的是从键内起点出发的真实位移：删除键按出界就取消的判断没有容差，空格的光标拖动和日文九键的滑动都从按下点量位移，坐标不能在按下之后跳回空隙里。
 */
public final class KeyboardKeyArea extends LinearLayout {
    /** `MotionEvent` 的指针 id 不超过 31，按 id 直接下标。 */
    private static final int MAX_POINTER_ID = 31;

    /** 在键之前看到按键区的每一个触摸事件（坐标是按键区自己的，没有经过空隙挪动）。 */
    public interface GlideTracker {
        /** 返回 true 表示这次触摸已被接管，事件不再交给键。 */
        boolean track(KeyboardKeyArea area, MotionEvent event);
    }

    private final Predicate<View> spacedKey;
    private GlideTracker glideTracker;
    /** 接管开始时已经给子视图发过 CANCEL。 */
    private boolean glideOwned;
    /** 按下时被挪进键里的指针（按 id 的位掩码）和各自的挪动量。 */
    private int routedPointers;
    private final float[] offsetX = new float[MAX_POINTER_ID + 1];
    private final float[] offsetY = new float[MAX_POINTER_ID + 1];
    private final Matrix inverse = new Matrix();
    private final float[] point = new float[2];
    private boolean hit;
    private View target;
    private float targetX;
    private float targetY;
    private float targetDistance;

    /** `spacedKey` 判断一个视图是否是由键距设置留出外边距的键，与布局时套外边距的判断一致。 */
    public KeyboardKeyArea(Context context, Predicate<View> spacedKey) {
        super(context);
        this.spacedKey = spacedKey;
    }

    public void setGlideTracker(GlideTracker tracker) {
        glideTracker = tracker;
    }

    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) glideOwned = false;
        if (glideTracker != null && glideTracker.track(this, event)) {
            if (!glideOwned) {
                glideOwned = true;
                MotionEvent cancel = MotionEvent.obtain(event);
                cancel.setAction(MotionEvent.ACTION_CANCEL);
                super.dispatchTouchEvent(cancel);
                cancel.recycle();
            }
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                glideOwned = false;
                routedPointers = 0;
            }
            return true;
        }
        if (action == MotionEvent.ACTION_DOWN) routedPointers = 0;
        if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN)
            route(event);
        boolean handled;
        if (routedPointers == 0) {
            handled = super.dispatchTouchEvent(event);
        } else {
            MotionEvent shifted = shifted(event);
            handled = super.dispatchTouchEvent(shifted);
            shifted.recycle();
        }
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            routedPointers = 0;
        } else if (action == MotionEvent.ACTION_POINTER_UP) {
            routedPointers &= ~pointerBit(event.getPointerId(event.getActionIndex()));
        }
        return handled;
    }

    /** 刚按下的指针落在某个键的空隙里时，记下把它挪进那个键边缘的挪动量。 */
    private void route(MotionEvent event) {
        int index = event.getActionIndex();
        int id = event.getPointerId(index);
        if (id < 0 || id > MAX_POINTER_ID) return;
        routedPointers &= ~pointerBit(id);
        float x = event.getX(index);
        float y = event.getY(index);
        hit = false;
        target = null;
        targetDistance = Float.MAX_VALUE;
        search(this, x, y);
        View owner = hit ? null : target;
        target = null;
        if (owner == null) return;
        point[0] = targetX;
        point[1] = targetY;
        toAreaCoordinates(owner, point);
        offsetX[id] = point[0] - x;
        offsetY[id] = point[1] - y;
        routedPointers |= pointerBit(id);
    }

    private static int pointerBit(int id) {
        return id < 0 || id > MAX_POINTER_ID ? 0 : 1 << id;
    }

    /**
     * 按框架命中测试的同样方式往下找：`x`、`y` 是 `group` 自己的坐标。点落在键或其它可点击视图上就不必改写；否则记下外边距包含这个点、离得最近的键，以及挪进它边缘后的局部坐标。
     */
    private void search(ViewGroup group, float x, float y) {
        for (int index = group.getChildCount() - 1; index >= 0 && !hit; index--) {
            View child = group.getChildAt(index);
            if (child.getVisibility() != VISIBLE) continue;
            float layoutX = x + group.getScrollX() - child.getLeft();
            float layoutY = y + group.getScrollY() - child.getTop();
            float localX = layoutX;
            float localY = layoutY;
            Matrix matrix = child.getMatrix();
            if (!matrix.isIdentity() && matrix.invert(inverse)) {
                point[0] = layoutX;
                point[1] = layoutY;
                inverse.mapPoints(point);
                localX = point[0];
                localY = point[1];
            }
            int width = child.getWidth();
            int height = child.getHeight();
            boolean inside = localX >= 0 && localY >= 0 && localX < width && localY < height;
            if (spacedKey.test(child)) {
                if (inside) {
                    hit = true;
                    return;
                }
                consider(child, layoutX, layoutY, localX, localY);
            } else if (inside) {
                if (child.isClickable() || child.isLongClickable()) {
                    hit = true;
                    return;
                }
                if (child instanceof ViewGroup inner) search(inner, localX, localY);
            }
        }
    }

    /** 外边距按布局位置算（按下动画的缩放不算在内），挪进去的位置按键当前的变换算，保证正在回弹的键也能命中。 */
    private void consider(View key, float layoutX, float layoutY, float localX, float localY) {
        if (!(key.getLayoutParams() instanceof MarginLayoutParams margins)) return;
        int width = key.getWidth();
        int height = key.getHeight();
        if (width < 2 || height < 2) return;
        float distance = KeyboardGapPolicy.gapDistance(layoutX, layoutY, width, height,
            margins.leftMargin, margins.topMargin, margins.rightMargin, margins.bottomMargin);
        if (distance < 0 || distance >= targetDistance) return;
        target = key;
        targetDistance = distance;
        targetX = KeyboardGapPolicy.inside(localX, width);
        targetY = KeyboardGapPolicy.inside(localY, height);
    }

    /** 把 `view` 自己坐标里的点换算到这个容器的坐标里，和框架往下分发时的换算互逆。 */
    private void toAreaCoordinates(View view, float[] coordinates) {
        View current = view;
        while (current != this) {
            Matrix matrix = current.getMatrix();
            if (!matrix.isIdentity()) matrix.mapPoints(coordinates);
            ViewGroup parent = (ViewGroup) current.getParent();
            coordinates[0] += current.getLeft() - parent.getScrollX();
            coordinates[1] += current.getTop() - parent.getScrollY();
            current = parent;
        }
    }

    /** 复制一份事件，把挪进键里的那些指针（包括历史采样）挪同样的距离；其它指针的坐标原样保留。 */
    private MotionEvent shifted(MotionEvent event) {
        int count = event.getPointerCount();
        if (count == 1) {
            int id = event.getPointerId(0);
            MotionEvent copy = MotionEvent.obtain(event);
            if ((routedPointers & pointerBit(id)) != 0) copy.offsetLocation(offsetX[id], offsetY[id]);
            return copy;
        }
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[count];
        MotionEvent.PointerCoords[] coordinates = new MotionEvent.PointerCoords[count];
        for (int pointer = 0; pointer < count; pointer++) {
            properties[pointer] = new MotionEvent.PointerProperties();
            event.getPointerProperties(pointer, properties[pointer]);
            coordinates[pointer] = new MotionEvent.PointerCoords();
        }
        int history = event.getHistorySize();
        MotionEvent copy = null;
        for (int sample = 0; sample <= history; sample++) {
            for (int pointer = 0; pointer < count; pointer++) {
                if (sample < history) event.getHistoricalPointerCoords(pointer, sample, coordinates[pointer]);
                else event.getPointerCoords(pointer, coordinates[pointer]);
                int id = properties[pointer].id;
                if ((routedPointers & pointerBit(id)) != 0) {
                    coordinates[pointer].x += offsetX[id];
                    coordinates[pointer].y += offsetY[id];
                }
            }
            long time = sample < history ? event.getHistoricalEventTime(sample) : event.getEventTime();
            if (copy == null) {
                copy = MotionEvent.obtain(event.getDownTime(), time, event.getAction(),
                    count, properties, coordinates, event.getMetaState(), event.getButtonState(),
                    event.getXPrecision(), event.getYPrecision(), event.getDeviceId(),
                    event.getEdgeFlags(), event.getSource(), event.getFlags());
            } else {
                copy.addBatch(time, coordinates, event.getMetaState());
            }
        }
        return copy;
    }
}
