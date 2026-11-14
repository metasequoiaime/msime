package app.msime.android;

import android.os.Build;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.view.HapticFeedbackConstants;
import android.view.View;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * 按键振动：强度档怎样变成真实的振动。键盘按键和设置页的试听都走这里，试的就是键盘上的那一下。
 *
 * <p>原来一律是 10 ms 的 `createOneShot` 加三档振幅：没有振幅控制的马达把三档都按满振幅播，有振幅控制的，10 ms 也短到摸不出轻重，于是三档感觉一样；设置页试听又是 18 ms，和键盘上的不是同一种振动。现在按设备能力依次取：API 30 起设备支持所需原语时用 Composition 原语（轻是 TICK，中、强是 CLICK）并按档缩放；API 29 起用预定义效果 TICK / CLICK / HEAVY_CLICK；有振幅控制时用更长的一次振动配三档振幅；都没有时只能靠时长区分。三档不带触摸用途（`USAGE_TOUCH`）：带上它，系统关掉触感反馈的用户在这三档下就完全不振了，而三档本来就是给想要自己定强度、不跟系统走的人用的；跟系统走是「跟随系统」那一档。
 *
 * <p>「跟随系统」不自己振动，交给 `View.performHapticFeedback(KEYBOARD_TAP)`，不带 `FLAG_IGNORE_GLOBAL_SETTING`，振不振、振多强由系统的触感反馈设置决定。
 *
 * <p>三档的 `Vibrator` 调用放在单独的 {@link #worker} 线程上：`vibrate`、`hasVibrator`、`areAllPrimitivesSupported` 都是到 system_server 的同步 binder 调用，部分厂商 ROM 上一次要几到几十毫秒，在主线程上每按一下键就把这段时间加进触摸处理，开了振动后快速打字跟不上手、按下态也晚出来。上一下还没交给系统时又按一下，这一下不再排队：排起来的振动会在手指早已离开后才一串串地振。触控板的一次移动则把实际步数合成一个连续效果，避免快速拖动只留下第一步。
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

    /** 只跑振动的后台线程；daemon，进程退出时不用收尾。 */
    private static final ExecutorService worker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-haptics"));
    /** 触控板序列会按真实时长等待，不能占住普通按键反馈的线程。 */
    private static final ExecutorService stepsWorker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-trackpad-haptics"));
    /** 已经交给 {@link #worker}、还没完成振动服务调用的那一下。 */
    private static final AtomicBoolean pending = new AtomicBoolean();
    /** 触控板步数合并队列；拖动事件频繁到来时仍按每一步播放，不让任务互相覆盖。 */
    private static final Object stepsLock = new Object();
    private static int queuedSteps;
    private static Vibrator queuedVibrator;
    private static View queuedView;
    private static KeyboardFeedbackPreferences.HapticStrength queuedStrength;
    private static boolean stepsScheduled;

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
     * 振一下。`view` 是按下的那个视图（设置页试听时是页面的根视图），「跟随系统」和没有振动器时由它交给系统；`vibrator` 可为 null。在主线程上调用，三档的振动在 {@link #worker} 上发出，本方法不等它。
     */
    public static void play(Vibrator vibrator, View view,
                            KeyboardFeedbackPreferences.HapticStrength strength) {
        Plan plan = plan(strength);
        if (plan == null || vibrator == null) {
            systemTap(view);
            return;
        }
        if (!pending.compareAndSet(false, true)) return;
        try {
            worker.execute(() -> {
                try {
                    if (!vibrator.hasVibrator()) {
                        if (view != null) view.post(() -> systemTap(view));
                        return;
                    }
                    vibrator.vibrate(effect(vibrator, plan));
                } finally {
                    pending.set(false);
                }
            });
        } catch (RejectedExecutionException error) {
            pending.set(false);
        }
    }

    /** 为同一次触控板移动播放每一步的振动，避免一次拖动只振第一步。 */
    public static void playSteps(Vibrator vibrator, View view,
                                 KeyboardFeedbackPreferences.HapticStrength strength, int count) {
        if (count <= 0) return;
        Plan plan = plan(strength);
        if (plan == null || vibrator == null) {
            for (int index = 0; index < count; index++) systemTap(view);
            return;
        }
        try {
            synchronized (stepsLock) {
                queuedSteps += count;
                queuedVibrator = vibrator;
                queuedView = view;
                queuedStrength = strength;
                if (stepsScheduled) return;
                stepsScheduled = true;
            }
            stepsWorker.execute(KeyboardHaptics::drainSteps);
        } catch (RejectedExecutionException ignored) {
            synchronized (stepsLock) { clearStepsLocked(); }
        }
    }

    private static void drainSteps() {
        try {
            while (true) {
                int count;
                Vibrator vibrator;
                View view;
                KeyboardFeedbackPreferences.HapticStrength strength;
                synchronized (stepsLock) {
                    count = queuedSteps;
                    queuedSteps = 0;
                    vibrator = queuedVibrator;
                    view = queuedView;
                    strength = queuedStrength;
                    if (count <= 0 || vibrator == null || strength == null) {
                        clearStepsLocked();
                        return;
                    }
                }
                if (!vibrator.hasVibrator()) {
                    if (view != null) view.post(() -> {
                        for (int index = 0; index < count; index++) systemTap(view);
                    });
                    continue;
                }
                Plan plan = plan(strength);
                if (plan == null) continue;
                vibrator.vibrate(sequenceEffect(vibrator, plan, count));
                try {
                    Thread.sleep(stepDuration(vibrator, plan) * count);
                } catch (InterruptedException error) {
                    Thread.currentThread().interrupt();
                    synchronized (stepsLock) { clearStepsLocked(); }
                    return;
                }
                synchronized (stepsLock) {
                    if (queuedSteps == 0) {
                        clearStepsLocked();
                        return;
                    }
                }
            }
        } catch (RuntimeException ignored) {
            // system_server 断开时丢弃这一批，解除占用让下一次拖动可以重新调度。
            synchronized (stepsLock) { clearStepsLocked(); }
        }
    }

    private static void clearStepsLocked() {
        queuedSteps = 0;
        queuedVibrator = null;
        queuedView = null;
        queuedStrength = null;
        stepsScheduled = false;
    }

    private static void systemTap(View view) {
        if (view != null) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
    }

    private static VibrationEffect effect(Vibrator vibrator, Plan plan) {
        if (Build.VERSION.SDK_INT >= 30 && vibrator.areAllPrimitivesSupported(plan.primitive())) {
            return VibrationEffect.startComposition()
                .addPrimitive(plan.primitive(), plan.scale()).compose();
        }
        if (Build.VERSION.SDK_INT >= 29) return VibrationEffect.createPredefined(plan.effect());
        if (vibrator.hasAmplitudeControl()) {
            return VibrationEffect.createOneShot(plan.durationMs(), plan.amplitude());
        }
        return VibrationEffect.createOneShot(plan.fallbackDurationMs(), VibrationEffect.DEFAULT_AMPLITUDE);
    }

    private static VibrationEffect sequenceEffect(Vibrator vibrator, Plan plan, int count) {
        if (count == 1) return effect(vibrator, plan);
        if (Build.VERSION.SDK_INT >= 30 && vibrator.areAllPrimitivesSupported(plan.primitive())) {
            VibrationEffect.Composition composition = VibrationEffect.startComposition();
            for (int index = 0; index < count; index++) {
                composition.addPrimitive(plan.primitive(), plan.scale());
            }
            return composition.compose();
        }
        long duration = stepDuration(vibrator, plan);
        long[] timings = new long[count * 2 + 1];
        int[] amplitudes = new int[count * 2 + 1];
        int amplitude = vibrator.hasAmplitudeControl() ? plan.amplitude() : VibrationEffect.DEFAULT_AMPLITUDE;
        for (int index = 0; index < count; index++) {
            timings[index * 2 + 1] = duration;
            amplitudes[index * 2 + 1] = amplitude;
        }
        return VibrationEffect.createWaveform(timings, amplitudes, -1);
    }

    private static long stepDuration(Vibrator vibrator, Plan plan) {
        return vibrator.hasAmplitudeControl() ? plan.durationMs() : plan.fallbackDurationMs();
    }
}
