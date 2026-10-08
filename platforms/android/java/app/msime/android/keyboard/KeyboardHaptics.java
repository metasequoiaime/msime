package app.msime.android;

import android.os.Build;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.view.HapticFeedbackConstants;
import android.view.View;

/**
 * 按键振动：强度档怎样变成真实的振动。键盘按键和设置页的试听都走这里，试的就是键盘上的那一下。
 *
 * <p>原来一律是 10 ms 的 `createOneShot` 加三档振幅：没有振幅控制的马达把三档都按满振幅播，有振幅控制的，10 ms 也短到摸不出轻重，于是三档感觉一样；设置页试听又是 18 ms，和键盘上的不是同一种振动。现在按设备能力依次取：API 30 起设备支持所需原语时用 Composition 原语（轻是 TICK，中、强是 CLICK）并按档缩放；API 29 起用预定义效果 TICK / CLICK / HEAVY_CLICK；有振幅控制时用更长的一次振动配三档振幅；都没有时只能靠时长区分。三档不带触摸用途（`USAGE_TOUCH`）：带上它，系统关掉触感反馈的用户在这三档下就完全不振了，而三档本来就是给想要自己定强度、不跟系统走的人用的；跟系统走是「跟随系统」那一档。
 *
 * <p>「跟随系统」不自己振动，交给 `View.performHapticFeedback(KEYBOARD_TAP)`，不带 `FLAG_IGNORE_GLOBAL_SETTING`，振不振、振多强由系统的触感反馈设置决定。
 */
public final class KeyboardHaptics {
    /**
     * 一档强度的振动方案；纯数据，在 JVM 回归里核对三档确实不同。
     *
     * @param primitive Composition 原语（API 30+）
     * @param scale 原语的强度缩放，0–1
     * @param effect 预定义效果（API 29+）
     * @param durationMs 有振幅控制时一次振动的时长
     * @param amplitude 有振幅控制时的振幅，1–255
     * @param fallbackDurationMs 没有振幅控制时只靠时长区分的那次振动
     */
    public record Plan(int primitive, float scale, int effect, long durationMs, int amplitude,
                       long fallbackDurationMs) {}

    private static final Plan LIGHT = new Plan(VibrationEffect.Composition.PRIMITIVE_TICK, 0.7f,
        VibrationEffect.EFFECT_TICK, 12, 70, 8);
    private static final Plan MEDIUM = new Plan(VibrationEffect.Composition.PRIMITIVE_CLICK, 0.6f,
        VibrationEffect.EFFECT_CLICK, 20, 150, 16);
    private static final Plan STRONG = new Plan(VibrationEffect.Composition.PRIMITIVE_CLICK, 1f,
        VibrationEffect.EFFECT_HEAVY_CLICK, 30, 255, 28);

    private KeyboardHaptics() {}

    /** 一档强度的振动方案；「跟随系统」没有自己的方案，返回 null。 */
    public static Plan plan(KeyboardFeedbackPreferences.HapticStrength strength) {
        return switch (strength) {
            case LIGHT -> LIGHT;
            case MEDIUM -> MEDIUM;
            case STRONG -> STRONG;
            case SYSTEM -> null;
        };
    }

    /**
     * 振一下。`view` 是按下的那个视图（设置页试听时是页面的根视图），「跟随系统」和没有振动器时由它交给系统；`vibrator` 可为 null。
     */
    public static void play(Vibrator vibrator, View view,
                            KeyboardFeedbackPreferences.HapticStrength strength) {
        Plan plan = plan(strength);
        if (plan == null || vibrator == null || !vibrator.hasVibrator()) {
            if (view != null) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
            return;
        }
        VibrationEffect effect;
        if (Build.VERSION.SDK_INT >= 30 && vibrator.areAllPrimitivesSupported(plan.primitive())) {
            effect = VibrationEffect.startComposition()
                .addPrimitive(plan.primitive(), plan.scale()).compose();
        } else if (Build.VERSION.SDK_INT >= 29) {
            effect = VibrationEffect.createPredefined(plan.effect());
        } else if (vibrator.hasAmplitudeControl()) {
            effect = VibrationEffect.createOneShot(plan.durationMs(), plan.amplitude());
        } else {
            effect = VibrationEffect.createOneShot(plan.fallbackDurationMs(),
                VibrationEffect.DEFAULT_AMPLITUDE);
        }
        vibrator.vibrate(effect);
    }
}
