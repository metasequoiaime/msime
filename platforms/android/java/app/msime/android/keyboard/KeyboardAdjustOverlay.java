package app.msime.android;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.os.Bundle;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import java.util.EnumMap;
import java.util.Map;

/**
 * 「键盘调整」遮罩：盖在整块键盘（工具栏或候选行加按键区）上，下面的键盘照常画成实时预览，但触摸全被遮罩接走。
 *
 * <p>遮罩是约 45% 不透明的黑色，四角画皮肤强调色的直角括号，标出键盘外框。上沿正中、下沿正中各一个横向把手（三道横线），左右两沿垂直居中各一个竖向把手（两道竖线），都画在外框以内：调整期间输入法窗口只把键盘外框算作可触摸区域，画到框外的柄点不到。中间一排三个按钮「重置」「移动」「确认」，白色图标加文字；「移动」按住拖动整块键盘。
 *
 * <p>遮罩只把手势和读屏动作转给 {@link Listener}，不持有任何数值：拖动按按下处起算的位移报告，由调用方按 {@link KeyboardAdjustPolicy} 从按下时的值重新算，手指来回拖不会累积误差。每个柄和「移动」都是读屏可调节元素，带 RangeInfo 和增减动作；「移动」能左右移时，前后滚动调位置，上下滚动调抬高。各部件的颜色不随皮肤的整树上色走，它们不是 Button 或 TextView。
 */
public final class KeyboardAdjustOverlay extends FrameLayout {
    /** 拖动、读屏和两个按钮的回调。 */
    public interface Listener {
        void dragStarted(KeyboardAdjustPolicy.Part part);

        /** 手指从按下处移动了 {@code dxPx}、{@code dyPx}（向右、向下为正）。 */
        void dragged(KeyboardAdjustPolicy.Part part, float dxPx, float dyPx);

        void dragEnded(KeyboardAdjustPolicy.Part part);

        /** 读屏增减一步（{@code direction} 为 1 或 -1）；{@code vertical} 只对能左右移的「移动」为真，表示调抬高。 */
        void step(KeyboardAdjustPolicy.Part part, int direction, boolean vertical);

        /** 读屏范围；{@code vertical} 的含义同 {@link #step}。 */
        KeyboardAdjustPolicy.Range range(KeyboardAdjustPolicy.Part part, boolean vertical);

        void reset();

        void confirm();
    }

    /** 整块遮罩的无障碍描述。 */
    public static final String DESCRIPTION = "键盘调整";
    /** 遮罩底色：黑色，约 45% 不透明。 */
    public static final int DIM_COLOR = 0x73000000;
    public static final String RESET_LABEL = "重置";
    public static final String MOVE_LABEL = "移动";
    public static final String CONFIRM_LABEL = "确认";

    private static final float BRACKET_ARM_DP = 18f;
    private static final float BRACKET_STROKE_DP = 3f;
    /** 横向把手（上下沿）与竖向把手（左右沿）的触摸区。 */
    private static final float GRIP_LONG_DP = 96f;
    private static final float GRIP_SHORT_DP = 28f;
    /** 中间三个按钮各自的宽高：三个加左右两个竖向把手不超过最窄的键盘（{@link KeyboardAdjustPolicy#MIN_WIDTH_DP}）。 */
    private static final float ACTION_WIDTH_DP = 60f;
    private static final float ACTION_HEIGHT_DP = 60f;
    private static final int FOREGROUND = Color.WHITE;

    /** 柄的读屏描述。 */
    public static String description(KeyboardAdjustPolicy.Part part) {
        return switch (part) {
            case TOP -> "键盘高度";
            case BOTTOM -> "键盘底部高度";
            case LEFT -> "键盘宽度（左边）";
            case RIGHT -> "键盘宽度（右边）";
            case MOVE -> MOVE_LABEL;
        };
    }

    private final Paint dim = new Paint();
    private final Paint bracket = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Map<KeyboardAdjustPolicy.Part, Grip> grips = new EnumMap<>(KeyboardAdjustPolicy.Part.class);
    private final Action reset;
    private final Action move;
    private final Action confirm;
    private Listener listener;
    private boolean moveHorizontal = true;

    public KeyboardAdjustOverlay(Context context) {
        super(context);
        setWillNotDraw(false);
        setContentDescription(DESCRIPTION);
        dim.setColor(DIM_COLOR);
        bracket.setStyle(Paint.Style.STROKE);
        bracket.setStrokeWidth(DimensionPolicy.floatPixels(context, BRACKET_STROKE_DP));
        bracket.setStrokeCap(Paint.Cap.SQUARE);
        bracket.setColor(FOREGROUND);
        int longSide = DimensionPolicy.pixels(context, GRIP_LONG_DP);
        int shortSide = DimensionPolicy.pixels(context, GRIP_SHORT_DP);
        addGrip(KeyboardAdjustPolicy.Part.TOP, longSide, shortSide, Gravity.TOP | Gravity.CENTER_HORIZONTAL);
        addGrip(KeyboardAdjustPolicy.Part.BOTTOM, longSide, shortSide, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        addGrip(KeyboardAdjustPolicy.Part.LEFT, shortSide, longSide, Gravity.LEFT | Gravity.CENTER_VERTICAL);
        addGrip(KeyboardAdjustPolicy.Part.RIGHT, shortSide, longSide, Gravity.RIGHT | Gravity.CENTER_VERTICAL);
        LinearLayout actions = new LinearLayout(context);
        actions.setOrientation(LinearLayout.HORIZONTAL);
        actions.setGravity(Gravity.CENTER);
        reset = new Action(context, this, null, KeyboardIconPaths.Icon.ADJUST_RESET, RESET_LABEL);
        move = new Action(context, this, KeyboardAdjustPolicy.Part.MOVE, KeyboardIconPaths.Icon.ADJUST_MOVE, MOVE_LABEL);
        confirm = new Action(context, this, null, KeyboardIconPaths.Icon.ADJUST_CONFIRM, CONFIRM_LABEL);
        int actionWidth = DimensionPolicy.pixels(context, ACTION_WIDTH_DP);
        int actionHeight = DimensionPolicy.pixels(context, ACTION_HEIGHT_DP);
        for (Action action : new Action[] {reset, move, confirm})
            actions.addView(action, new LinearLayout.LayoutParams(actionWidth, actionHeight));
        addView(actions, new FrameLayout.LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT,
            Gravity.CENTER));
        reset.setOnClickListener(ignored -> { if (listener != null) listener.reset(); });
        confirm.setOnClickListener(ignored -> { if (listener != null) listener.confirm(); });
    }

    private void addGrip(KeyboardAdjustPolicy.Part part, int width, int height, int gravity) {
        Grip grip = new Grip(getContext(), this, part);
        grips.put(part, grip);
        addView(grip, new FrameLayout.LayoutParams(width, height, gravity));
    }

    public void setListener(Listener value) { listener = value; }

    /** 括号的颜色：皮肤强调色。 */
    public void setAccent(int color) {
        if (bracket.getColor() == color) return;
        bracket.setColor(color);
        invalidate();
    }

    /** 按此刻生效的项显示柄：不生效的项不显示对应的柄，「移动」不能左右移时只调抬高。 */
    public void setAvailability(KeyboardAdjustPolicy.Availability availability) {
        for (Map.Entry<KeyboardAdjustPolicy.Part, Grip> entry : grips.entrySet())
            ViewPolicy.setVisible(entry.getValue(), availability.shows(entry.getKey()));
        ViewPolicy.setVisible(move, availability.shows(KeyboardAdjustPolicy.Part.MOVE));
        moveHorizontal = availability.movesHorizontally();
    }

    /** 某个部件的视图，给测试和读屏焦点用；没有这个部件时为 null。 */
    public View part(KeyboardAdjustPolicy.Part part) {
        return part == KeyboardAdjustPolicy.Part.MOVE ? move : grips.get(part);
    }

    public View resetButton() { return reset; }

    public View confirmButton() { return confirm; }

    // 遮罩接走落在它上面的全部触摸：下面的按键只做预览，不能按。
    @SuppressLint("ClickableViewAccessibility")
    @Override public boolean onTouchEvent(MotionEvent event) {
        return true;
    }

    @Override protected void onDraw(Canvas canvas) {
        int width = getWidth();
        int height = getHeight();
        canvas.drawRect(0, 0, width, height, dim);
        float half = bracket.getStrokeWidth() / 2f;
        float arm = DimensionPolicy.floatPixels(getContext(), BRACKET_ARM_DP);
        float left = half;
        float top = half;
        float right = width - half;
        float bottom = height - half;
        canvas.drawLine(left, top, left + arm, top, bracket);
        canvas.drawLine(left, top, left, top + arm, bracket);
        canvas.drawLine(right, top, right - arm, top, bracket);
        canvas.drawLine(right, top, right, top + arm, bracket);
        canvas.drawLine(left, bottom, left + arm, bottom, bracket);
        canvas.drawLine(left, bottom, left, bottom - arm, bracket);
        canvas.drawLine(right, bottom, right - arm, bottom, bracket);
        canvas.drawLine(right, bottom, right, bottom - arm, bracket);
    }

    private void started(KeyboardAdjustPolicy.Part part) {
        if (listener != null) listener.dragStarted(part);
    }

    private void dragged(KeyboardAdjustPolicy.Part part, float dx, float dy) {
        if (listener != null) listener.dragged(part, dx, dy);
    }

    private void ended(KeyboardAdjustPolicy.Part part) {
        if (listener != null) listener.dragEnded(part);
    }

    /** 柄和「移动」共用的拖动：按屏幕坐标记下按下处，之后报告相对它的位移。键盘在拖动中伸缩、移动，用视图内坐标会跟着柄自己一起动。 */
    private static final class Drag {
        private float downX;
        private float downY;

        @SuppressLint("ClickableViewAccessibility")
        boolean handle(View view, KeyboardAdjustOverlay overlay, KeyboardAdjustPolicy.Part part, MotionEvent event) {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downX = event.getRawX();
                    downY = event.getRawY();
                    view.getParent().requestDisallowInterceptTouchEvent(true);
                    view.setPressed(true);
                    overlay.started(part);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    overlay.dragged(part, event.getRawX() - downX, event.getRawY() - downY);
                    return true;
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    view.setPressed(false);
                    overlay.ended(part);
                    return true;
                }
                default -> {
                    return true;
                }
            }
        }
    }

    /** 柄与「移动」的读屏：RangeInfo 加增减动作；能左右移的「移动」另带上下滚动调抬高。 */
    private static final class Adjustable {
        private Adjustable() { }

        @SuppressWarnings("deprecation")
        static void describe(KeyboardAdjustOverlay overlay, KeyboardAdjustPolicy.Part part,
                AccessibilityNodeInfo info) {
            info.setContentDescription(description(part));
            if (overlay.listener == null) return;
            KeyboardAdjustPolicy.Range range = overlay.listener.range(part, false);
            info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(
                AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_INT, range.min(), range.max(), range.current()));
            info.setScrollable(true);
            if (range.canIncrease()) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD);
            if (range.canDecrease()) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD);
            if (part == KeyboardAdjustPolicy.Part.MOVE && overlay.moveHorizontal) {
                KeyboardAdjustPolicy.Range lift = overlay.listener.range(part, true);
                if (lift.canIncrease()) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_UP);
                if (lift.canDecrease()) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_DOWN);
            }
        }

        static boolean perform(View view, KeyboardAdjustOverlay overlay, KeyboardAdjustPolicy.Part part, int action) {
            if (overlay.listener == null) return false;
            int direction;
            boolean vertical = false;
            if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) {
                direction = 1;
            } else if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) {
                direction = -1;
            } else if (part == KeyboardAdjustPolicy.Part.MOVE && overlay.moveHorizontal
                    && action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_UP.getId()) {
                direction = 1;
                vertical = true;
            } else if (part == KeyboardAdjustPolicy.Part.MOVE && overlay.moveHorizontal
                    && action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_DOWN.getId()) {
                direction = -1;
                vertical = true;
            } else {
                return false;
            }
            KeyboardAdjustPolicy.Range range = overlay.listener.range(part, vertical);
            if (direction > 0 ? !range.canIncrease() : !range.canDecrease()) return false;
            overlay.listener.step(part, direction, vertical);
            view.sendAccessibilityEvent(AccessibilityEvent.TYPE_VIEW_SCROLLED);
            return true;
        }
    }

    /** 四条边上的把手：上下沿三道横线，左右沿两道竖线。 */
    private static final class Grip extends View {
        private static final float LINE_LONG_DP = 24f;
        private static final float LINE_SHORT_DP = 20f;
        private static final float LINE_GAP_DP = 4f;
        private static final float LINE_STROKE_DP = 1.5f;

        private final KeyboardAdjustOverlay overlay;
        private final KeyboardAdjustPolicy.Part part;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Drag drag = new Drag();

        Grip(Context context, KeyboardAdjustOverlay overlay, KeyboardAdjustPolicy.Part part) {
            super(context);
            this.overlay = overlay;
            this.part = part;
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeCap(Paint.Cap.ROUND);
            paint.setStrokeWidth(DimensionPolicy.floatPixels(context, LINE_STROKE_DP));
            setContentDescription(description(part));
            setFocusable(true);
        }

        // 把手只换算拖动距离，没有点击语义；读屏用户经 RangeInfo 和增减动作调。
        @SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(MotionEvent event) {
            return drag.handle(this, overlay, part, event);
        }

        @Override protected void drawableStateChanged() {
            super.drawableStateChanged();
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            paint.setColor(isPressed() ? ColorPolicy.withAlpha(FOREGROUND, .6f) : FOREGROUND);
            float gap = DimensionPolicy.floatPixels(getContext(), LINE_GAP_DP);
            float cx = getWidth() / 2f;
            float cy = getHeight() / 2f;
            boolean horizontal = part == KeyboardAdjustPolicy.Part.TOP || part == KeyboardAdjustPolicy.Part.BOTTOM;
            if (horizontal) {
                float half = DimensionPolicy.floatPixels(getContext(), LINE_LONG_DP) / 2f;
                for (int line = -1; line <= 1; line++)
                    canvas.drawLine(cx - half, cy + line * gap, cx + half, cy + line * gap, paint);
            } else {
                float half = DimensionPolicy.floatPixels(getContext(), LINE_SHORT_DP) / 2f;
                for (float offset : new float[] {-gap / 2f, gap / 2f})
                    canvas.drawLine(cx + offset, cy - half, cx + offset, cy + half, paint);
            }
        }

        @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
            super.onInitializeAccessibilityNodeInfo(info);
            Adjustable.describe(overlay, part, info);
        }

        @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
            if (Adjustable.perform(this, overlay, part, action)) return true;
            return super.performAccessibilityAction(action, arguments);
        }
    }

    /** 中间一排的按钮：上面白色图标，下面文字。「移动」按住拖动，「重置」「确认」是普通点击。 */
    private static final class Action extends View {
        private static final float ICON_DP = 28f;
        private static final float LABEL_SP = 13f;
        private static final float GAP_DP = 6f;

        private final KeyboardAdjustOverlay overlay;
        /** 「移动」为 {@link KeyboardAdjustPolicy.Part#MOVE}，两个点击按钮为 null。 */
        private final KeyboardAdjustPolicy.Part part;
        private final KeyboardIconPaths.Icon icon;
        private final String label;
        private final Paint iconPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Drag drag = new Drag();

        Action(Context context, KeyboardAdjustOverlay overlay, KeyboardAdjustPolicy.Part part,
                KeyboardIconPaths.Icon icon, String label) {
            super(context);
            this.overlay = overlay;
            this.part = part;
            this.icon = icon;
            this.label = label;
            text.setTextAlign(Paint.Align.CENTER);
            setContentDescription(label);
            setFocusable(true);
            if (part == null) setClickable(true);
        }

        @SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(MotionEvent event) {
            if (part != null) return drag.handle(this, overlay, part, event);
            return super.onTouchEvent(event);
        }

        @Override protected void drawableStateChanged() {
            super.drawableStateChanged();
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            int color = isPressed() ? ColorPolicy.withAlpha(FOREGROUND, .6f) : FOREGROUND;
            ViewPolicy.setTextSize(text, KeyboardTypographyPolicy.keySp(getContext(), LABEL_SP));
            text.setColor(color);
            Paint.FontMetrics metrics = text.getFontMetrics();
            float iconSize = DimensionPolicy.floatPixels(getContext(), ICON_DP);
            float gap = DimensionPolicy.floatPixels(getContext(), GAP_DP);
            float textHeight = metrics.descent - metrics.ascent;
            float top = Math.max(0f, (getHeight() - iconSize - gap - textHeight) / 2f);
            KeyboardIconPaths.draw(canvas, iconPaint, icon, (getWidth() - iconSize) / 2f, top, iconSize, color);
            canvas.drawText(label, getWidth() / 2f, top + iconSize + gap - metrics.ascent, text);
        }

        @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
            super.onInitializeAccessibilityNodeInfo(info);
            if (part != null) Adjustable.describe(overlay, part, info);
        }

        @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
            if (part != null && Adjustable.perform(this, overlay, part, action)) return true;
            return super.performAccessibilityAction(action, arguments);
        }
    }
}
