package app.msime.android;

import android.animation.Animator;
import android.animation.AnimatorListenerAdapter;
import android.animation.ObjectAnimator;
import android.animation.PropertyValuesHolder;
import android.animation.ValueAnimator;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.graphics.RectF;
import android.graphics.drawable.Drawable;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewGroupOverlay;
import android.view.animation.DecelerateInterpolator;

/**
 * 按键动画（偏好 `touch_key_animation`）：弹起 bounce、涟漪 ripple、发光 glow、浮起 lift，四种都是 0.4 s ease-out，纯 `android.animation` 实现。
 *
 * <p>bounce：缩放 1 → .86 → 1.08 → 1；lift：上移 6 dp 并放大到 1.1 再回落；ripple：键外一圈 accent 环从 0 扩到 10 dp 并淡出；glow：键外 14 dp 的 accent 光晕淡出。环与光晕画在父容器的 overlay 上，所以能超出键的边界，也不碰键自身的 alpha。`none` 什么都不做，保持现有按压态。
 */
public final class KeyPressAnimator {
    /** 动画样式，与偏好值一一对应。 */
    public enum Style {
        NONE("none"), BOUNCE("bounce"), RIPPLE("ripple"), GLOW("glow"), LIFT("lift");

        private final String preference;

        Style(String preference) { this.preference = preference; }

        public String preference() { return preference; }

        /** 偏好值到样式；未知值按 `none`。 */
        public static Style fromPreference(String value) {
            if (value != null) {
                for (Style style : values()) {
                    if (style.preference.equals(value)) return style;
                }
            }
            return NONE;
        }
    }

    public static final long DURATION_MS = 400L;
    public static final float RIPPLE_SPREAD_DP = 10f;
    public static final float GLOW_SPREAD_DP = 14f;
    public static final float LIFT_DP = 6f;

    private KeyPressAnimator() {}

    /** bounce 的缩放关键帧。 */
    public static float[] bounceScales() { return new float[] {1f, .86f, 1.08f, 1f}; }

    /** 在 {@code key} 上播放一次按键动画；{@code accent} 用于涟漪与光晕。 */
    public static void play(View key, Style style, int accent) {
        switch (style) {
            case NONE -> { }
            case BOUNCE -> bounce(key);
            case LIFT -> lift(key);
            case RIPPLE -> halo(key, accent, RIPPLE_SPREAD_DP, false);
            case GLOW -> halo(key, accent, GLOW_SPREAD_DP, true);
        }
    }

    private static void bounce(View key) {
        float[] scales = bounceScales();
        ObjectAnimator animator = ObjectAnimator.ofPropertyValuesHolder(key,
            PropertyValuesHolder.ofFloat(View.SCALE_X, scales),
            PropertyValuesHolder.ofFloat(View.SCALE_Y, scales));
        animator.setDuration(DURATION_MS);
        animator.setInterpolator(new DecelerateInterpolator());
        animator.start();
    }

    private static void lift(View key) {
        float density = key.getResources().getDisplayMetrics().density;
        ObjectAnimator animator = ObjectAnimator.ofPropertyValuesHolder(key,
            PropertyValuesHolder.ofFloat(View.TRANSLATION_Y, 0f, -LIFT_DP * density, 0f),
            PropertyValuesHolder.ofFloat(View.SCALE_X, 1f, 1.1f, 1f),
            PropertyValuesHolder.ofFloat(View.SCALE_Y, 1f, 1.1f, 1f));
        animator.setDuration(DURATION_MS);
        animator.setInterpolator(new DecelerateInterpolator());
        animator.start();
    }

    private static void halo(View key, int accent, float spreadDp, boolean glow) {
        if (!(key.getParent() instanceof ViewGroup parent)) return;
        if (key.getWidth() <= 0 || key.getHeight() <= 0) return;
        float density = key.getResources().getDisplayMetrics().density;
        float spread = spreadDp * density;
        HaloDrawable halo = new HaloDrawable(accent, glow, density);
        int left = Math.round(key.getLeft() - spread);
        int top = Math.round(key.getTop() - spread);
        halo.setBounds(left, top, Math.round(key.getRight() + spread),
            Math.round(key.getBottom() + spread));
        halo.inset = spread;
        ViewGroupOverlay overlay = parent.getOverlay();
        overlay.add(halo);
        ValueAnimator animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(DURATION_MS);
        animator.setInterpolator(new DecelerateInterpolator());
        animator.addUpdateListener(update -> halo.setProgress((Float) update.getAnimatedValue()));
        animator.addListener(new AnimatorListenerAdapter() {
            @Override public void onAnimationEnd(Animator animation) { overlay.remove(halo); }
        });
        animator.start();
    }

    /** 涟漪环或光晕；进度 0 → 1 时外扩并淡出。 */
    private static final class HaloDrawable extends Drawable {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF rect = new RectF();
        private final int accent;
        private final boolean glow;
        private final float density;
        private float inset;
        private float progress;

        HaloDrawable(int accent, boolean glow, float density) {
            this.accent = accent;
            this.glow = glow;
            this.density = density;
        }

        void setProgress(float value) {
            progress = value;
            invalidateSelf();
        }

        @Override public void draw(Canvas canvas) {
            float spread = inset * progress;
            rect.set(getBounds());
            rect.inset(inset - spread, inset - spread);
            float radius = 8 * density + spread;
            int alpha = Math.round(Color.alpha(accent) * (1f - progress));
            if (glow) {
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(Color.argb(Math.round(alpha * .35f), Color.red(accent),
                    Color.green(accent), Color.blue(accent)));
            } else {
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(2 * density);
                paint.setColor(Color.argb(alpha, Color.red(accent), Color.green(accent),
                    Color.blue(accent)));
            }
            canvas.drawRoundRect(rect, radius, radius, paint);
        }

        @Override public void setAlpha(int alpha) {
            paint.setAlpha(alpha);
            invalidateSelf();
        }

        @Override public void setColorFilter(ColorFilter filter) {
            paint.setColorFilter(filter);
            invalidateSelf();
        }

        @Deprecated
        @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
    }
}
