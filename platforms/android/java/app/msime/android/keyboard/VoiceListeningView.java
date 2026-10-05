package app.msime.android;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.TypedValue;
import android.view.animation.AccelerateDecelerateInterpolator;
import android.widget.TextView;

/**
 * 键盘内的语音聆听面板：72 dp 的 accent 圆盘托着麦克风，外圈一道 1.2 s 循环的脉冲环（扩到 18 dp），下面「正在聆听…」和「点任意处取消」。
 *
 * <p>节点 text 是「正在聆听…」，描述「正在聆听，点任意处取消」（§2.8）；整个面板可点，点击即取消，回调由调用方用 `setOnClickListener` 设置。颜色由调用方从皮肤传入。
 */
public final class VoiceListeningView extends TextView {
    public static final float ORB_DP = 72f;
    public static final float PULSE_DP = 18f;
    public static final float MIC_DP = 32f;
    private static final long PULSE_MS = 1200L;

    private final Paint orb = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint ring = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint title = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint hint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private int accent = Color.BLUE;
    private int onAccent = Color.WHITE;
    private float pulse;
    private ValueAnimator animator;
    private String hintText = "点任意处取消";

    public VoiceListeningView(Context context) {
        super(context);
        setText("正在聆听…");
        setContentDescription("正在聆听，点任意处取消");
        setClickable(true);
        setFocusable(true);
        setBackground(null);
        title.setTextAlign(Paint.Align.CENTER);
        hint.setTextAlign(Paint.Align.CENTER);
    }

    /** accent 圆盘、圆盘上的麦克风色、标题色（kbFg）、提示色（kbSub）。 */
    public void setColors(int accentColor, int onAccentColor, int foreground, int secondary) {
        accent = accentColor;
        onAccent = onAccentColor;
        title.setColor(foreground);
        hint.setColor(secondary);
        invalidate();
    }

    /** 替换第二行提示（例如识别完成时短暂显示「已识别：…」）。 */
    public void setHint(String value) {
        hintText = value == null ? "" : value;
        invalidate();
    }

    /** 脉冲环在动画进度 {@code t}（0–1）时的外扩半径（像素）与透明度（0–255）。 */
    public static float pulseSpread(float t, float maxSpread) {
        float clamped = Math.max(0f, Math.min(1f, t));
        return maxSpread * clamped;
    }

    public static int pulseAlpha(float t) {
        float clamped = Math.max(0f, Math.min(1f, t));
        return Math.round(90 * (1f - clamped));
    }

    @Override protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        ValueAnimator next = ValueAnimator.ofFloat(0f, 1f);
        next.setDuration(PULSE_MS);
        next.setRepeatCount(ValueAnimator.INFINITE);
        next.setInterpolator(new AccelerateDecelerateInterpolator());
        next.addUpdateListener(update -> {
            pulse = (Float) update.getAnimatedValue();
            invalidate();
        });
        animator = next;
        next.start();
    }

    @Override protected void onDetachedFromWindow() {
        if (animator != null) animator.cancel();
        animator = null;
        super.onDetachedFromWindow();
    }

    @Override protected void onDraw(Canvas canvas) {
        float density = getResources().getDisplayMetrics().density;
        float radius = ORB_DP * density / 2f;
        title.setTextSize(sp(16));
        hint.setTextSize(sp(13));
        Paint.FontMetrics titleMetrics = title.getFontMetrics();
        Paint.FontMetrics hintMetrics = hint.getFontMetrics();
        float titleHeight = titleMetrics.descent - titleMetrics.ascent;
        float hintHeight = hintMetrics.descent - hintMetrics.ascent;
        float gap = 16 * density;
        float total = radius * 2 + gap + titleHeight + 6 * density + hintHeight;
        float top = (getHeight() - total) / 2f;
        float cx = getWidth() / 2f;
        float cy = top + radius;
        ring.setColor(accent);
        ring.setAlpha(pulseAlpha(pulse));
        canvas.drawCircle(cx, cy, radius + pulseSpread(pulse, PULSE_DP * density), ring);
        orb.setColor(accent);
        canvas.drawCircle(cx, cy, radius, orb);
        float mic = MIC_DP * density;
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.MIC, cx - mic / 2f,
            cy - mic / 2f, mic, onAccent);
        float titleBaseline = cy + radius + gap - titleMetrics.ascent;
        canvas.drawText(String.valueOf(getText()), cx, titleBaseline, title);
        if (!hintText.isEmpty()) {
            canvas.drawText(hintText, cx, titleBaseline + titleMetrics.descent + 6 * density
                - hintMetrics.ascent, hint);
        }
    }

    private float sp(float value) {
        return TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value,
            getResources().getDisplayMetrics());
    }
}
