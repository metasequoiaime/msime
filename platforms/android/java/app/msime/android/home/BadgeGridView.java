package app.msime.android.home;

import app.msime.android.KeyboardGeometry;
import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.Shader;
import android.graphics.Typeface;
import android.os.Bundle;
import android.util.AttributeSet;
import android.view.KeyEvent;
import android.view.MotionEvent;
import app.msime.android.BoundsPolicy;
import android.view.View;
import android.view.animation.LinearInterpolator;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.ColorPolicy;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import androidx.customview.widget.ExploreByTouchHelper;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.Achievement;
import java.util.List;
import java.util.function.Consumer;

/**
 * 成就页的 3 列徽章网格：每枚一张圆角卡片，已解锁的画成斜放的渐变菱形奖章、中间写奖章字，未解锁的画成环形进度、环里写奖章字和百分数，下面是标题和一行说明。
 *
 * <p>配色按徽章分组，和原型一致：数量用 accent，连续用 accent 混黑，技巧混蓝 `#3A6EA5`，趣味混紫 `#7A5BA8`。第一次显示时奖章依次弹出（msMedalPop），点按时那一枚抖动 0.6 秒（msWiggle），并回调给页面显示提示。每枚徽章对读屏是一个可点按的虚拟节点。
 */
public final class BadgeGridView extends View {
    private static final int COLUMNS = 3;
    private static final float GAP = 10f;
    private static final float TILE_HEIGHT = 136f;
    private static final float MEDAL = 54f;
    private static final float RING = 52f;
    private static final float RING_STROKE = 4f;
    private static final long WIGGLE_MILLIS = 600;
    private static final long POP_MILLIS = 600;
    private static final long POP_STAGGER = 45;
    private static final long POP_DELAY = 120;
    /** msWiggle 的关键帧：时间点、相对 45° 的旋转、缩放。 */
    private static final float[][] WIGGLE = {
        {0f, 0f, 1f}, {.2f, -15f, 1.15f}, {.4f, 13f, 1.1f}, {.6f, -7f, 1.05f}, {.8f, 5f, 1f}, {1f, 0f, 1f}};
    /** msMedalPop 的关键帧：时间点、缩放。 */
    private static final float[][] POP = {{0f, .3f}, {.6f, 1.12f}, {.8f, .96f}, {1f, 1f}};

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint ring = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint glyph = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint title = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint caption = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private final RectF medalBox = new RectF();
    private final RectF ringBox = new RectF();
    private final RectF hitBox = new RectF();
    private final LinearGradient[] medalGradients = new LinearGradient[4];
    private final long[] medalGradientKeys = {Long.MIN_VALUE, Long.MIN_VALUE, Long.MIN_VALUE, Long.MIN_VALUE};
    private int cachedAccent;
    private boolean coloursCached;
    private final int[] streakColours = new int[2];
    private final int[] skillColours = new int[2];
    private final int[] funColours = new int[2];
    private final int[] defaultColours = new int[2];
    private final Nodes nodes;
    private List<Achievement> badges = List.of();
    @Nullable private Consumer<Achievement> onTap;
    private int wiggling = -1;
    private float wiggle;
    private long popStart = -1;
    @Nullable private ValueAnimator wiggleAnimator;
    @Nullable private ValueAnimator popAnimator;

    public BadgeGridView(Context context) {
        this(context, null);
    }

    public BadgeGridView(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
        glyph.setTextAlign(Paint.Align.CENTER);
        glyph.setTypeface(Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        title.setTextAlign(Paint.Align.CENTER);
        title.setTypeface(Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        title.setTextSize(Ui.sp(context, 14));
        caption.setTextAlign(Paint.Align.CENTER);
        caption.setTextSize(Ui.sp(context, 11));
        ring.setStyle(Paint.Style.STROKE);
        ring.setStrokeCap(Paint.Cap.ROUND);
        ring.setStrokeWidth(Ui.dp(context, RING_STROKE));
        setClickable(true);
        setFocusable(true);
        nodes = new Nodes(this);
        ViewCompat.setAccessibilityDelegate(this, nodes);
    }

    /** 点按徽章后回调。 */
    public void setOnBadgeTap(@Nullable Consumer<Achievement> listener) {
        onTap = listener;
    }

    /** 换一组徽章；第一次有内容时播放弹出动画。 */
    public void setBadges(List<Achievement> values) {
        boolean first = badges.isEmpty();
        badges = values == null ? List.of() : List.copyOf(values);
        requestLayout();
        invalidate();
        nodes.invalidateRoot();
        if (first && !badges.isEmpty()) pop();
    }

    private void pop() {
        if (popAnimator != null) popAnimator.cancel();
        long duration = POP_DELAY + POP_STAGGER * badges.size() + POP_MILLIS;
        ValueAnimator animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(duration);
        animator.setInterpolator(new LinearInterpolator());
        popStart = 0;
        animator.addUpdateListener(update -> {
            popStart = update.getCurrentPlayTime();
            invalidate();
        });
        animator.addListener(new android.animation.AnimatorListenerAdapter() {
            @Override public void onAnimationEnd(android.animation.Animator animation) {
                popStart = -1;
                invalidate();
            }
        });
        popAnimator = animator;
        animator.start();
    }

    private void tap(int index) {
        if (index < 0 || index >= badges.size()) return;
        if (wiggleAnimator != null) wiggleAnimator.cancel();
        wiggling = index;
        ValueAnimator animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(WIGGLE_MILLIS);
        animator.setInterpolator(new LinearInterpolator());
        animator.addUpdateListener(update -> {
            wiggle = (float) update.getAnimatedValue();
            invalidate();
        });
        animator.addListener(new android.animation.AnimatorListenerAdapter() {
            @Override public void onAnimationEnd(android.animation.Animator animation) {
                wiggling = -1;
                invalidate();
            }
        });
        wiggleAnimator = animator;
        animator.start();
        nodes.sendEventForVirtualView(index, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_CLICKED);
        if (onTap != null) onTap.accept(badges.get(index));
    }

    @Override protected void onDetachedFromWindow() {
        if (wiggleAnimator != null) wiggleAnimator.cancel();
        if (popAnimator != null) popAnimator.cancel();
        super.onDetachedFromWindow();
    }

    private float tileWidth() {
        return (getWidth() - Ui.dp(getContext(), GAP) * (COLUMNS - 1)) / COLUMNS;
    }

    private void tile(int index, RectF out) {
        float gap = Ui.dp(getContext(), GAP);
        float width = tileWidth();
        float height = Ui.dp(getContext(), TILE_HEIGHT);
        float left = (index % COLUMNS) * (width + gap);
        float top = (index / COLUMNS) * (height + gap);
        out.set(left, top, left + width, top + height);
    }

    private int indexAt(float x, float y) {
        for (int index = 0; index < badges.size(); index++) {
            tile(index, hitBox);
            if (hitBox.contains(x, y)) return index;
        }
        return -1;
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        int rows = (badges.size() + COLUMNS - 1) / COLUMNS;
        float height = rows * TILE_HEIGHT + BoundsPolicy.nonNegative(rows - 1) * GAP;
        setMeasuredDimension(MeasureSpec.getSize(widthSpec),
            resolveSize(Ui.dp(getContext(), height), heightSpec));
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (event.getAction() == MotionEvent.ACTION_UP) {
            int index = indexAt(event.getX(), event.getY());
            if (index >= 0) {
                performClick();
                tap(index);
            }
            return true;
        }
        return event.getAction() == MotionEvent.ACTION_DOWN || super.onTouchEvent(event);
    }

    @Override public boolean performClick() {
        return super.performClick();
    }

    @Override public boolean dispatchKeyEvent(KeyEvent event) {
        return nodes.dispatchKeyEvent(event) || super.dispatchKeyEvent(event);
    }

    private boolean dark() {
        return Ui.isNight(getContext());
    }

    /** 分组的两种颜色：深色端和浅色端。 */
    private int[] colours(String group) {
        return switch (group) {
            case "streak" -> streakColours;
            case "skill" -> skillColours;
            case "fun" -> funColours;
            default -> defaultColours;
        };
    }

    private void refreshColours(Context context) {
        int accent = Ui.accent(context);
        if (coloursCached && cachedAccent == accent) return;
        cachedAccent = accent;
        coloursCached = true;
        streakColours[0] = ColorPolicy.blend(Color.BLACK, accent, .82f);
        streakColours[1] = ColorPolicy.blend(Color.WHITE, accent, .8f);
        skillColours[0] = ColorPolicy.blend(0xFF3A6EA5, accent, .7f);
        skillColours[1] = ColorPolicy.blend(Color.WHITE, accent, .55f);
        funColours[0] = ColorPolicy.blend(0xFF7A5BA8, accent, .68f);
        funColours[1] = ColorPolicy.blend(Color.WHITE, accent, .55f);
        defaultColours[0] = accent;
        defaultColours[1] = ColorPolicy.blend(Color.WHITE, accent, .7f);
        java.util.Arrays.fill(medalGradients, null);
        java.util.Arrays.fill(medalGradientKeys, Long.MIN_VALUE);
    }

    private static float frame(float[][] frames, float t, int column) {
        for (int index = 1; index < frames.length; index++) {
            if (t <= frames[index][0]) {
                float span = frames[index][0] - frames[index - 1][0];
                float local = span <= 0 ? 1 : (t - frames[index - 1][0]) / span;
                return frames[index - 1][column] + (frames[index][column] - frames[index - 1][column]) * local;
            }
        }
        return frames[frames.length - 1][column];
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        int card = Ui.card(context);
        int text = Ui.text(context);
        int sub = Ui.subText(context);
        int track = dark() ? Ui.withAlpha(Color.WHITE, .1f) : Ui.withAlpha(Color.BLACK, .07f);
        float radius = Ui.dp(context, 20);
        refreshColours(context);
        for (int index = 0; index < badges.size(); index++) {
            Achievement badge = badges.get(index);
            tile(index, box);
            fill.setShader(null);
            fill.setColor(card);
            canvas.drawRoundRect(box, radius, radius, fill);
            float centreX = box.centerX();
            float centreY = box.top + Ui.dp(context, 46);
            float rotate = 0;
            float scale = 1;
            if (index == wiggling) {
                rotate = frame(WIGGLE, wiggle, 1);
                scale = frame(WIGGLE, wiggle, 2);
            } else if (popStart >= 0) {
                long local = popStart - POP_DELAY - POP_STAGGER * index;
                float t = KeyboardGeometry.bounded(local / (float) POP_MILLIS, 0f, 1f);
                scale = frame(POP, t, 1);
            }
            int[] colours = colours(badge.group());
            canvas.save();
            canvas.translate(centreX, centreY);
            canvas.scale(scale, scale);
            if (badge.unlocked()) drawMedal(canvas, badge, colours, rotate);
            else drawRing(canvas, badge, colours, track, sub, rotate);
            canvas.restore();
            title.setColor(badge.unlocked() ? text : sub);
            canvas.drawText(badge.title(), centreX, box.top + Ui.dp(context, 104), title);
            caption.setColor(sub);
            canvas.drawText(ellipsize(TypingStatisticsSummary.caption(badge), box.width() - Ui.dp(context, 12)),
                centreX, box.top + Ui.dp(context, 123), caption);
        }
    }

    private String ellipsize(String value, float width) {
        if (caption.measureText(value) <= width) return value;
        String text = value;
        while (text.length() > 1 && caption.measureText(text + "…") > width) {
            text = text.substring(0, text.length() - 1);
        }
        return text + "…";
    }

    private void drawMedal(Canvas canvas, Achievement badge, int[] colours, float rotate) {
        Context context = getContext();
        float half = Ui.dp(context, MEDAL) / 2;
        canvas.save();
        canvas.rotate(45 + rotate);
        medalBox.set(-half, -half, half, half);
        // 145° 的渐变：浅色端在左上，深色端到 70% 处。
        int slot = colours == streakColours ? 0 : colours == skillColours ? 1 : colours == funColours ? 2 : 3;
        long key = ((long) colours[0] << 32) ^ (colours[1] & 0xffffffffL) ^ Float.floatToIntBits(half);
        if (medalGradients[slot] == null || medalGradientKeys[slot] != key) {
            medalGradients[slot] = new LinearGradient(-half, -half, half * .4f, half * .4f, colours[1], colours[0],
                Shader.TileMode.CLAMP);
            medalGradientKeys[slot] = key;
        }
        fill.setShader(medalGradients[slot]);
        float corner = Ui.dp(context, 17);
        canvas.drawRoundRect(medalBox, corner, corner, fill);
        fill.setShader(null);
        canvas.restore();
        canvas.save();
        canvas.rotate(rotate);
        String face = badge.glyph();
        glyph.setColor(Color.WHITE);
        glyph.setTextSize(Ui.sp(context, face.length() > 2 ? 13 : face.length() > 1 ? 15 : 19));
        Paint.FontMetrics metrics = glyph.getFontMetrics();
        canvas.drawText(face, 0, -(metrics.ascent + metrics.descent) / 2, glyph);
        canvas.restore();
    }

    private void drawRing(Canvas canvas, Achievement badge, int[] colours, int track, int sub, float rotate) {
        Context context = getContext();
        float half = Ui.dp(context, RING) / 2 - Ui.dp(context, RING_STROKE) / 2;
        canvas.save();
        canvas.rotate(rotate);
        ringBox.set(-half, -half, half, half);
        ring.setColor(track);
        canvas.drawArc(ringBox, 0, 360, false, ring);
        float sweep = (float) (360 * badge.progress());
        if (sweep > 0) {
            ring.setColor(colours[0]);
            canvas.drawArc(ringBox, -90, sweep, false, ring);
        }
        String face = badge.glyph();
        glyph.setColor(sub);
        glyph.setTextSize(Ui.sp(context, face.length() > 2 ? 12 : 15));
        canvas.drawText(face, 0, Ui.dp(context, 2), glyph);
        glyph.setColor(colours[0]);
        glyph.setTextSize(Ui.sp(context, 9));
        canvas.drawText(TypingStatisticsSummary.progressLabel(badge), 0, Ui.dp(context, 14), glyph);
        canvas.restore();
    }

    /** 每枚徽章一个虚拟节点：读「标题，已解锁/未解锁，说明」，可点按。 */
    private final class Nodes extends ExploreByTouchHelper {
        private final Rect bounds = new Rect();
        private final RectF scratch = new RectF();

        Nodes(View host) {
            super(host);
        }

        @Override protected int getVirtualViewAt(float x, float y) {
            int index = indexAt(x, y);
            return index < 0 ? INVALID_ID : index;
        }

        @Override protected void getVisibleVirtualViews(List<Integer> ids) {
            for (int index = 0; index < badges.size(); index++) ids.add(index);
        }

        @Override protected void onPopulateNodeForVirtualView(int id, @NonNull AccessibilityNodeInfoCompat node) {
            if (id < 0 || id >= badges.size()) {
                node.setContentDescription("");
                node.setBoundsInParent(bounds);
                return;
            }
            Achievement badge = badges.get(id);
            node.setContentDescription(badge.title() + "，" + (badge.unlocked() ? "已解锁" : "未解锁")
                + "，" + TypingStatisticsSummary.caption(badge));
            node.setClassName(android.widget.Button.class.getName());
            node.addAction(AccessibilityNodeInfoCompat.ACTION_CLICK);
            tile(id, scratch);
            scratch.roundOut(bounds);
            node.setBoundsInParent(bounds);
        }

        @Override protected boolean onPerformActionForVirtualView(int id, int action, @Nullable Bundle arguments) {
            if (action != AccessibilityNodeInfoCompat.ACTION_CLICK) return false;
            tap(id);
            return true;
        }
    }
}
