package app.msime.android.home;

import android.view.animation.PathInterpolator;

/** Android 宿主界面共用的设计动效曲线。 */
public final class MotionCurves {
    /** M3 强调曲线 `cubic-bezier(.2, 0, 0, 1)`。 */
    public static final PathInterpolator EMPHASIZED = new PathInterpolator(0.2f, 0f, 0f, 1f);
    /** 开屏元素弹入曲线 `cubic-bezier(.16, 1, .3, 1)`。 */
    public static final PathInterpolator POP = new PathInterpolator(0.16f, 1f, 0.3f, 1f);
    /** 标准缓动曲线 `cubic-bezier(.25, .1, .25, 1)`。 */
    public static final PathInterpolator EASE = new PathInterpolator(0.25f, 0.1f, 0.25f, 1f);

    private MotionCurves() {}
}
