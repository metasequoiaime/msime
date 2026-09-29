package app.msime.android;

import android.provider.Settings;
import android.view.View;
import android.view.animation.DecelerateInterpolator;
import android.view.animation.OvershootInterpolator;

/**
 * Apple's key press feedback, on any view.
 *
 * <p>Lives apart from {@link KeyboardPressButton} because a scheme card is a small layout rather
 * than a button -- it carries a bordered glyph, a badge, a title and a check mark -- and a card
 * that does not sink under the finger reads as a label rather than as something to press.
 */
public final class KeyboardPressFeedback {
    private static final float PRESSED_SCALE = 0.94f;
    private static final float PRESSED_TRANSLATION_DP = 1f;
    private static final long PRESS_DURATION_MILLIS = 60L;
    private static final long RELEASE_DURATION_MILLIS = 180L;

    private KeyboardPressFeedback() {}

    /** Animate `view` into or out of its pressed state, or settle it when it cannot animate. */
    public static void update(View view, boolean pressed) {
        view.animate().cancel();
        if (!view.isAttachedToWindow() || !view.isEnabled() || !animationsEnabled(view)) {
            reset(view);
            return;
        }
        if (pressed) {
            view.animate().translationY(PRESSED_TRANSLATION_DP
                    * view.getResources().getDisplayMetrics().density)
                .scaleX(PRESSED_SCALE).scaleY(PRESSED_SCALE)
                .setDuration(PRESS_DURATION_MILLIS)
                .setInterpolator(new DecelerateInterpolator()).start();
        } else {
            view.animate().translationY(0f).scaleX(1f).scaleY(1f)
                .setDuration(RELEASE_DURATION_MILLIS)
                .setInterpolator(new OvershootInterpolator(1.1f)).start();
        }
    }

    /** Drop the view back to its resting transform without animating. */
    public static void reset(View view) {
        view.setTranslationY(0f);
        view.setScaleX(1f);
        view.setScaleY(1f);
    }

    private static boolean animationsEnabled(View view) {
        try {
            float animatorScale = Settings.Global.getFloat(view.getContext().getContentResolver(),
                Settings.Global.ANIMATOR_DURATION_SCALE, 1f);
            float transitionScale = Settings.Global.getFloat(view.getContext().getContentResolver(),
                Settings.Global.TRANSITION_ANIMATION_SCALE, 1f);
            return animatorScale > 0f && transitionScale > 0f;
        } catch (RuntimeException error) {
            return true;
        }
    }
}
