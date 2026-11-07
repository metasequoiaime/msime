package app.msime.android;

import android.content.Context;
import android.graphics.Matrix;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import java.util.Arrays;
import java.util.function.Predicate;

/**
 * 按键区（26 键各行、九键的标点列和网格、底部功能行）的容器：按下落在键与键之间的空隙时，交给拥有这段空隙的那个键。
 *
 * <p>键距和行距是每个键的布局外边距。外边距不属于按钮，按下落在那里时没有子视图接收，行和它的祖先都不可点击，整个手势就悄无声息地丢了：默认间距下 26 键约四分之一、九键约五分之一的面积是这样的死区，打字快、落点不准时就像随机丢键。多指连按时第二根手指落进空隙，框架还会把它并进第一根手指按着的键，两次按键都可能丢。
 *
 * <p>字母行里的占位格（{@link KeyboardSpacer}）指定了所属的键时，落在占位格或它的外边距里的按下同样交给那个键，按下点挪进它靠这一侧的边缘；没有指定所属的键的占位格照旧不接收触摸。所属的键只能在同一排里，按下不跨排归属。
 *
 * <p>装了 {@link GlideTracker} 时，每个事件先交给它：它接管一次触摸（滑行输入）后，这里给子视图补一个 CANCEL，之后这一次触摸的事件都不再往下分发，直到最后一根手指抬起。
 *
 * <p>装了 {@link TouchPointPolicy} 时，每根手指按下（`ACTION_DOWN` / `ACTION_POINTER_DOWN`）落在一个键或它的空隙里、而那个键是调用方要记的（点击时会取走按下点的键），就按那个键记下按下点：坐标是按键区自己的、挪进键边缘之前的那一个，和量键位几何（{@link LetterKeyMeasure}）用的是同一个坐标系。整次触摸被取消或被滑行接管时全部丢掉。
 *
 * <p>装了 {@link PressListener} 时，每根手指按下落在一个可用的键或它的空隙里，就在把按下交给键之前回调 {@link PressListener#pressed}：按键音和振动在这里给出，不等点击（#10757，规则见 {@link KeyPressFeedbackPolicy}）。这根手指抬起时，在把抬起交给键之前回调 {@link PressListener#releasing}。两次回调返回的令牌都在事件交给键之后，经 `post` 排在框架投递的点击后面交还给 {@link PressListener#settled}。整次触摸被取消或被滑行接管时不再回调抬起。
 *
 * <p>这里不改布局也不改绘制，画面和以前逐像素相同：只在 `ACTION_DOWN` / `ACTION_POINTER_DOWN` 落在某个键的外边距里、又没有落在任何键或其它可点击视图上时，把这一个指针的按下坐标挪进那个键的边缘（挪动距离就是空隙宽度），再交给 `LinearLayout` 照常分发。这个挪动量按指针 id 记下，之后同一个指针的 MOVE、POINTER_UP、UP、CANCEL（连同其中的历史采样）都挪同样的距离，直到它抬起。键看到的是从键内起点出发的真实位移：删除键按出界就取消的判断没有容差，空格的光标拖动和日文九键的滑动都从按下点量位移，坐标不能在按下之后跳回空隙里。
 */
public final class KeyboardKeyArea extends LinearLayout {
    /** `MotionEvent` 的指针 id 不超过 31，按 id 直接下标。 */
    private static final int MAX_POINTER_ID = 31;

    /** 按键区按下即反馈（#10757）：哪个键被按下、抬起，和两段武装各自何时结束。 */
    public interface PressListener {
        /** 一根手指按下命中 `key`（或拥有那段空隙的键），在把按下交给键之前。给了按下反馈时返回令牌，没给（这个键此刻点了也不做事）时返回 null，这根手指抬起时也就不再回调。 */
        Object pressed(View key);

        /** 按下时回调过 {@link #pressed} 的手指抬起，在把抬起交给键之前。 */
        Object releasing(View key);

        /** 按下或抬起已经交给键、框架随后投递的点击也已执行，交还那一次的令牌。 */
        void settled(Object token);
    }

    /** 在键之前看到按键区的每一个触摸事件（坐标是按键区自己的，没有经过空隙挪动）。 */
    public interface GlideTracker {
        /** 返回 true 表示这次触摸已被接管，事件不再交给键。 */
        boolean track(KeyboardKeyArea area, MotionEvent event);
    }

    private final Predicate<View> spacedKey;
    private GlideTracker glideTracker;
    private PressListener pressListener;
    /** 每根手指按下时给过反馈的键，按指针 id 下标；抬起时回调 {@link PressListener#releasing}。 */
    private final View[] pressedKeys = new View[MAX_POINTER_ID + 1];
    /** `route` 算出的这次按下命中的键（或拥有那段空隙的键），没有时为 null。 */
    private View routedKey;
    private TouchPointPolicy touchPoints;
    /** 哪些键记按下点：只记点击时会取走它的键，数字行、Shift、退格的按下不占槽位。 */
    private Predicate<View> touchKey;
    /** 接管开始时已经给子视图发过 CANCEL。 */
    private boolean glideOwned;
    /** 按下时被挪进键里的指针（按 id 的位掩码）和各自的挪动量。 */
    private int routedPointers;
    private final float[] offsetX = new float[MAX_POINTER_ID + 1];
    private final float[] offsetY = new float[MAX_POINTER_ID + 1];
    private final Matrix inverse = new Matrix();
    private final float[] point = new float[2];
    private boolean hit;
    /** 按下落在指定了所属键的占位格里：`target` 就是那个键，不再找空隙。 */
    private boolean claimed;
    /** 命中的那个键和按下在它自己坐标里的位置，`route` 用来判断是否落在让出的那一段里。 */
    private View hitKey;
    private float hitX;
    private float hitY;
    /** 把键帽左侧 `yieldWidth` 像素让给 `yieldReceiver` 的键，见 {@link #setYield}。 */
    private View yieldingKey;
    private View yieldReceiver;
    private float yieldWidth;
    private View target;
    private float targetX;
    private float targetY;
    private float targetDistance;
    /** 选中的键自己坐标里按下的横坐标（挪进键帽之前），`route` 用来判断空隙里的按下是否落在让出的那一段里。 */
    private float targetRawX;

    /** `spacedKey` 判断一个视图是否是由键距设置留出外边距的键，与布局时套外边距的判断一致。 */
    public KeyboardKeyArea(Context context, Predicate<View> spacedKey) {
        super(context);
        this.spacedKey = spacedKey;
    }

    public void setGlideTracker(GlideTracker tracker) {
        glideTracker = tracker;
    }

    /** 按下即反馈的回调，见类说明；传 null 不回调。 */
    public void setPressListener(PressListener listener) {
        pressListener = listener;
        clearPressedKeys();
    }

    /** 按键记下每根手指的按下点，见类说明；只记 `keys` 认可的键，`points` 传 null 不记。 */
    public void setTouchPoints(TouchPointPolicy points, Predicate<View> keys) {
        touchPoints = points;
        touchKey = keys;
    }

    /**
     * 让 `yielding` 把键帽左侧 `widthPx` 像素让给紧挨在它左边的 `receiver`：按下落在那一段里时，像落在空隙里一样挪进 `receiver` 的右边缘。底行的回车用它把与中/英之间拿不准的按下让给中/英（{@link KeyboardActionRow#RETURN_YIELD_DP}）。传 null 取消。
     */
    public void setYield(View yielding, View receiver, float widthPx) {
        yieldingKey = yielding;
        yieldReceiver = receiver;
        yieldWidth = yielding == null || receiver == null ? 0f : widthPx;
    }

    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) glideOwned = false;
        if (glideTracker != null && glideTracker.track(this, event)) {
            if (!glideOwned) {
                glideOwned = true;
                // 滑行接管了这次触摸，按下的键收到 CANCEL、不会点击，它的按下点也不再有用，抬起时也不再武装。
                if (touchPoints != null) touchPoints.clear();
                clearPressedKeys();
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
        if (action == MotionEvent.ACTION_DOWN) {
            routedPointers = 0;
            clearPressedKeys();
        }
        Object token = null;
        if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN) {
            route(event);
            token = pressed(event.getPointerId(event.getActionIndex()));
        } else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_POINTER_UP) {
            token = releasing(event.getPointerId(event.getActionIndex()));
        }
        boolean handled;
        if (routedPointers == 0) {
            handled = super.dispatchTouchEvent(event);
        } else {
            MotionEvent shifted = shifted(event);
            handled = super.dispatchTouchEvent(shifted);
            shifted.recycle();
        }
        if (action == MotionEvent.ACTION_CANCEL && touchPoints != null) touchPoints.clear();
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            routedPointers = 0;
            clearPressedKeys();
        } else if (action == MotionEvent.ACTION_POINTER_UP) {
            routedPointers &= ~pointerBit(event.getPointerId(event.getActionIndex()));
        }
        settleLater(token);
        return handled;
    }

    /** 刚按下的指针命中可用的键时给按下反馈，记下这个键等它抬起；返回要在分发之后交还的令牌。 */
    private Object pressed(int id) {
        View key = routedKey;
        routedKey = null;
        if (pressListener == null || key == null || !key.isEnabled() || id < 0 || id > MAX_POINTER_ID)
            return null;
        Object token = pressListener.pressed(key);
        pressedKeys[id] = token == null ? null : key;
        return token;
    }

    /** 按下时给过反馈的指针抬起：在把抬起交给键之前武装那个键。 */
    private Object releasing(int id) {
        if (pressListener == null || id < 0 || id > MAX_POINTER_ID) return null;
        View key = pressedKeys[id];
        pressedKeys[id] = null;
        return key == null ? null : pressListener.releasing(key);
    }

    /** 令牌排在这次分发里框架投递的点击后面交还；已经离开窗口（没有 Handler）时当场交还，免得一直排到下次挂上窗口。 */
    private void settleLater(Object token) {
        if (token == null) return;
        PressListener listener = pressListener;
        if (listener == null) return;
        if (getHandler() == null || !post(() -> listener.settled(token))) listener.settled(token);
    }

    private void clearPressedKeys() {
        Arrays.fill(pressedKeys, null);
    }

    /** 刚按下的指针落在某个键的空隙里时，记下把它挪进那个键边缘的挪动量。 */
    private void route(MotionEvent event) {
        routedKey = null;
        int index = event.getActionIndex();
        int id = event.getPointerId(index);
        if (id < 0 || id > MAX_POINTER_ID) return;
        routedPointers &= ~pointerBit(id);
        float x = event.getX(index);
        float y = event.getY(index);
        hit = false;
        claimed = false;
        hitKey = null;
        target = null;
        targetDistance = Float.MAX_VALUE;
        search(this, x, y);
        claimed = false;
        View owner = hit ? null : target;
        // 让出的那一段：键帽左侧几 dp（这时 hit 为真），或者它左侧原本按最近的键归它的那半段空隙。
        boolean yielded = hit ? hitKey != null && hitKey == yieldingKey && yieldsHere(hitX)
            : target != null && target == yieldingKey && yieldsHere(targetRawX);
        if (yielded) {
            owner = yieldReceiver;
            targetX = KeyboardGapPolicy.inside(yieldReceiver.getWidth(), yieldReceiver.getWidth());
            targetY = KeyboardGapPolicy.inside(hit ? hitY : targetY, yieldReceiver.getHeight());
        }
        // 落在键上记给那个键，落在空隙里记给拥有空隙的键；坐标都是挪动之前的。
        View pressed = owner != null ? owner : hitKey;
        routedKey = pressed;
        if (touchPoints != null && pressed != null && touchKey.test(pressed))
            touchPoints.record(pressed, x, y, event.getEventTime());
        hitKey = null;
        target = null;
        if (owner == null) return;
        point[0] = targetX;
        point[1] = targetY;
        toAreaCoordinates(owner, point);
        offsetX[id] = point[0] - x;
        offsetY[id] = point[1] - y;
        routedPointers |= pointerBit(id);
    }

    /** 按下（让出的键自己坐标里的 `x`）落在它让出的那一段里，而接收的键就在同一行里可见。 */
    private boolean yieldsHere(float x) {
        int leftMargin = yieldingKey.getLayoutParams() instanceof MarginLayoutParams margins
            ? margins.leftMargin : 0;
        return yieldReceiver != null && yieldReceiver.getVisibility() == VISIBLE
            && yieldReceiver.getParent() == yieldingKey.getParent()
            && yieldReceiver.getWidth() >= 2 && yieldReceiver.getHeight() >= 2
            && KeyboardGapPolicy.yieldsToLeft(x, leftMargin, yieldWidth);
    }

    private static int pointerBit(int id) {
        return id < 0 || id > MAX_POINTER_ID ? 0 : 1 << id;
    }

    /**
     * 按框架命中测试的同样方式往下找：`x`、`y` 是 `group` 自己的坐标。点落在键或其它可点击视图上就不必改写；落在指定了所属键的占位格或它的外边距里就选那个键；否则记下外边距包含这个点、离得最近的键，以及挪进它边缘后的局部坐标。
     */
    private void search(ViewGroup group, float x, float y) {
        for (int index = group.getChildCount() - 1; index >= 0 && !hit && !claimed; index--) {
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
            if (child instanceof KeyboardSpacer spacer) {
                // 占位格连同它的外边距（宽窄第二排两端朝外补的半键距）都归所属的键；没有所属的键时不接收。
                View owner = spacer.usableOwner();
                if (owner != null && child.getLayoutParams() instanceof MarginLayoutParams margins
                        && KeyboardGapPolicy.gapDistance(layoutX, layoutY, width, height, margins.leftMargin,
                            margins.topMargin, margins.rightMargin, margins.bottomMargin) >= 0) {
                    claim(group, owner, x, y);
                    return;
                }
                continue;
            }
            if (spacedKey.test(child)) {
                if (inside) {
                    hit = true;
                    hitKey = child;
                    hitX = localX;
                    hitY = localY;
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

    /** 占位格把按下交给同一排里的 `key`：`x`、`y` 是这一排（`group`）的坐标，挪进键帽的位置按键当前的变换算，与 {@link #consider} 相同。 */
    private void claim(ViewGroup group, View key, float x, float y) {
        float localX = x + group.getScrollX() - key.getLeft();
        float localY = y + group.getScrollY() - key.getTop();
        Matrix matrix = key.getMatrix();
        if (!matrix.isIdentity() && matrix.invert(inverse)) {
            point[0] = localX;
            point[1] = localY;
            inverse.mapPoints(point);
            localX = point[0];
            localY = point[1];
        }
        claimed = true;
        target = key;
        targetDistance = 0f;
        targetRawX = localX;
        targetX = KeyboardGapPolicy.inside(localX, key.getWidth());
        targetY = KeyboardGapPolicy.inside(localY, key.getHeight());
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
        targetRawX = localX;
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
