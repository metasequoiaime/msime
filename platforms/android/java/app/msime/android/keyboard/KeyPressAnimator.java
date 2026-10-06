package app.msime.android;

import app.msime.android.KeyboardGeometry;

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
import java.util.Map;
import java.util.WeakHashMap;

/**
 * 按键动画（偏好 `touch_key_animation`）：弹起 bounce、涟漪 ripple、发光 glow、浮起 lift，四种都是 0.4 s ease-out，纯 `android.animation` 实现。
 *
 * <p>bounce：缩放 1 → .86 → 1.08 → 1；lift：上移 6 dp 并放大到 1.1 再回落；ripple：键外一圈 accent 环从 0 扩到 10 dp 并淡出；glow：键外 14 dp 的 accent 光晕淡出。环与光晕画在调用方给的覆盖层（IME 里是盖住整块键盘的气泡层）的 overlay 上，所以不会被所在行裁掉，也不碰键自身的 alpha；lift 播放时把键到覆盖层之间的容器设成不裁子视图，浮起的部分不被切掉。`none` 什么都不做，保持现有按压态。
 *
 * <p>bounce 与 lift 写的是 {@link KeyboardPressFeedback} 也在写的 scale / translationY：它们从当前值起步、收在静止态，运行期间 {@link KeyboardPressFeedback} 不再做松开回弹，下一次按下先 {@link #cancel} 掉它们。
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

    /** 每个键上正在跑的 bounce / lift，用于与按压态动画互斥。 */
    private static final Map<View, Animator> RUNNING = new WeakHashMap<>();
    /** Reusable X/Y keyframes; the previous animator is cancelled before values are rewritten. */
    private static final Map<View, float[][]> BOUNCE_VALUES = new WeakHashMap<>();
    /** All animation entry points run on the IME main thread; reuse the short-lived location buffers. */
    private static final int[] PARENT_LOCATION = new int[2];
    private static final int[] HOST_LOCATION = new int[2];

    private static final class Interpolators {
        static final DecelerateInterpolator DECELERATE = new DecelerateInterpolator();
    }

    private KeyPressAnimator() {}

    /** {@code key} 上是否有 bounce / lift 正在运行。 */
    public static boolean isAnimating(View key) {
        Animator animator = RUNNING.get(key);
        return animator != null && animator.isRunning();
    }

    /** 停掉 {@code key} 上的 bounce / lift（停在当前帧，由调用方接管变换）。 */
    public static void cancel(View key) {
        Animator animator = RUNNING.remove(key);
        if (animator != null) animator.cancel();
    }

    /** bounce 的缩放关键帧。 */
    public static float[] bounceScales() { return new float[] {1f, .86f, 1.08f, 1f}; }

    /**
     * 在 {@code key} 上播放一次按键动画；{@code accent} 用于涟漪与光晕。
     *
     * @param host 画涟漪与光晕、并作为 lift 不裁剪范围边界的覆盖层；null 时退回键的父容器
     */
    public static void play(View key, ViewGroup host, Style style, int accent) {
        ViewGroup layer = host != null ? host
            : key.getParent() instanceof ViewGroup parent ? parent : null;
        switch (style) {
            case NONE -> { }
            case BOUNCE -> bounce(key);
            case LIFT -> lift(key, layer);
            case RIPPLE -> halo(key, layer, accent, RIPPLE_SPREAD_DP, false);
            case GLOW -> halo(key, layer, accent, GLOW_SPREAD_DP, true);
        }
    }

    private static void bounce(View key) {
        cancel(key);
        float[][] values = BOUNCE_VALUES.computeIfAbsent(key,
            ignored -> new float[][] {new float[4], new float[4]});
        float[] scales = values[0];
        scales[0] = key.getScaleX();
        scales[1] = .86f;
        scales[2] = 1.08f;
        scales[3] = 1f;
        float[] scalesY = values[1];
        scalesY[0] = key.getScaleY();
        scalesY[1] = .86f;
        scalesY[2] = 1.08f;
        scalesY[3] = 1f;
        start(key, ObjectAnimator.ofPropertyValuesHolder(key,
            PropertyValuesHolder.ofFloat(View.SCALE_X, scales),
            PropertyValuesHolder.ofFloat(View.SCALE_Y, scalesY),
            // The press state sinks the key by 1 dp; the release spring is skipped while this runs.
            PropertyValuesHolder.ofFloat(View.TRANSLATION_Y, key.getTranslationY(), 0f)));
    }

    private static void lift(View key, ViewGroup host) {
        float density = KeyboardGeometry.density(key.getContext());
        unclipUpTo(key, host);
        start(key, ObjectAnimator.ofPropertyValuesHolder(key,
            PropertyValuesHolder.ofFloat(View.TRANSLATION_Y, key.getTranslationY(), -LIFT_DP * density, 0f),
            PropertyValuesHolder.ofFloat(View.SCALE_X, key.getScaleX(), 1.1f, 1f),
            PropertyValuesHolder.ofFloat(View.SCALE_Y, key.getScaleY(), 1.1f, 1f)));
    }

    /** 启动键上的变换动画：先停掉按压态动画与上一次的 bounce / lift，再登记这一次。 */
    private static void start(View key, ObjectAnimator animator) {
        cancel(key);
        key.animate().cancel();
        animator.setDuration(DURATION_MS);
        animator.setInterpolator(Interpolators.DECELERATE);
        animator.addListener(new AnimatorListenerAdapter() {
            @Override public void onAnimationEnd(Animator animation) {
                if (RUNNING.get(key) == animation) RUNNING.remove(key);
            }
        });
        RUNNING.put(key, animator);
        animator.start();
    }

    /**
     * 让键浮起时不被所在行和键区裁掉：从键的父容器往上，直到 {@code host} 所在的容器（不含）都设成不裁子视图。{@code host} 不在键的祖先链旁边时只放开键的父容器。
     */
    private static void unclipUpTo(View key, ViewGroup host) {
        android.view.ViewParent boundary = host == null ? null : host.getParent();
        android.view.ViewParent parent = key.getParent();
        while (parent instanceof ViewGroup group && parent != boundary) {
            group.setClipChildren(false);
            if (boundary == null) return;
            parent = group.getParent();
        }
    }

    private static void halo(View key, ViewGroup host, int accent, float spreadDp, boolean glow) {
        if (host == null || !(key.getParent() instanceof ViewGroup parent)) return;
        if (key.getWidth() <= 0 || key.getHeight() <= 0) return;
        float density = KeyboardGeometry.density(key.getContext());
        float spread = spreadDp * density;
        HaloDrawable halo = new HaloDrawable(accent, glow, density);
        // Untransformed key origin in host coordinates: the key itself may be mid press-scale.
        parent.getLocationInWindow(PARENT_LOCATION);
        host.getLocationInWindow(HOST_LOCATION);
        float keyLeft = PARENT_LOCATION[0] - HOST_LOCATION[0] + key.getLeft() - parent.getScrollX();
        float keyTop = PARENT_LOCATION[1] - HOST_LOCATION[1] + key.getTop() - parent.getScrollY();
        halo.setBounds(Math.round(keyLeft - spread), Math.round(keyTop - spread),
            Math.round(keyLeft + key.getWidth() + spread),
            Math.round(keyTop + key.getHeight() + spread));
        halo.inset = spread;
        ViewGroupOverlay overlay = host.getOverlay();
        overlay.add(halo);
        ValueAnimator animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(DURATION_MS);
        animator.setInterpolator(Interpolators.DECELERATE);
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
