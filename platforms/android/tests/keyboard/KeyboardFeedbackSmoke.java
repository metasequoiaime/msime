import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackPreferences.HapticStrength;
import app.msime.android.KeyboardHaptics;
import java.util.Arrays;

public final class KeyboardFeedbackSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(KeyboardFeedbackPreferences.strength("light") == HapticStrength.LIGHT);
        check(KeyboardFeedbackPreferences.strength("medium") == HapticStrength.MEDIUM);
        check(KeyboardFeedbackPreferences.strength("strong") == HapticStrength.STRONG);
        check(KeyboardFeedbackPreferences.strength("system") == HapticStrength.SYSTEM);
        check(KeyboardFeedbackPreferences.strength("invalid") == HapticStrength.MEDIUM);
        check(KeyboardFeedbackPreferences.strength(null) == HapticStrength.MEDIUM);
        check(KeyboardFeedbackPreferences.known("system") && !KeyboardFeedbackPreferences.known("System"));
        check(!KeyboardFeedbackPreferences.known(null));
        // 功能面板按枚举顺序循环，「跟随系统」在其中。
        check(Arrays.asList(HapticStrength.values()).equals(Arrays.asList(
            HapticStrength.LIGHT, HapticStrength.MEDIUM, HapticStrength.STRONG, HapticStrength.SYSTEM)));
        check(HapticStrength.SYSTEM.title().equals("跟随系统"));
        check(HapticStrength.SYSTEM.shortTitle().equals("系统"));
        check(HapticStrength.MEDIUM.shortTitle().equals("中"));

        // 三档在每一种设备能力下都不同：原语（TICK 比 CLICK 轻，同为 CLICK 时按缩放分）、预定义效果、振幅加时长、只有时长。
        KeyboardHaptics.Plan light = KeyboardHaptics.plan(HapticStrength.LIGHT);
        KeyboardHaptics.Plan medium = KeyboardHaptics.plan(HapticStrength.MEDIUM);
        KeyboardHaptics.Plan strong = KeyboardHaptics.plan(HapticStrength.STRONG);
        check(light.primitive() != medium.primitive());
        check(medium.primitive() == strong.primitive() && medium.scale() < strong.scale());
        check(light.effect() != medium.effect() && medium.effect() != strong.effect()
            && light.effect() != strong.effect());
        check(light.amplitude() < medium.amplitude() && medium.amplitude() < strong.amplitude());
        check(light.durationMs() < medium.durationMs() && medium.durationMs() < strong.durationMs());
        check(light.fallbackDurationMs() < medium.fallbackDurationMs()
            && medium.fallbackDurationMs() < strong.fallbackDurationMs());
        // 原来的 10 ms 一次振动太短，振幅差摸不出来；有振幅控制时最短的一档也要长过它。
        check(light.durationMs() > 10);
        for (KeyboardHaptics.Plan plan : new KeyboardHaptics.Plan[] {light, medium, strong}) {
            check(plan.scale() > 0 && plan.scale() <= 1);
            check(plan.amplitude() >= 1 && plan.amplitude() <= 255);
        }
        // 「跟随系统」不自己振动，交给系统的触感反馈设置。
        check(KeyboardHaptics.plan(HapticStrength.SYSTEM) == null);
        System.out.println("Android keyboard feedback: strength normalization, system level and distinct haptic plans passed");
    }
}
