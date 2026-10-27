#!/usr/bin/env python3
"""运行真实振动调度，确认慢服务期间不排队，完成后仍可继续振动。"""

from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"

# 只替换 Android 系统边界；KeyboardHaptics、偏好枚举与线程工厂编译完整产品源码。
STUBS = {
    "android/os/Build.java": r'''
package android.os;
public final class Build {
    public static final class VERSION { public static int SDK_INT = 35; }
}
''',
    "android/os/VibrationEffect.java": r'''
package android.os;
public class VibrationEffect {
    public static final int EFFECT_TICK = 1, EFFECT_CLICK = 2, EFFECT_HEAVY_CLICK = 3, DEFAULT_AMPLITUDE = -1;
    public static VibrationEffect createPredefined(int effect) { return new VibrationEffect(); }
    public static VibrationEffect createOneShot(long duration, int amplitude) { return new VibrationEffect(); }
    public static Composition startComposition() { return new Composition(); }
    public static final class Composition {
        public static final int PRIMITIVE_TICK = 1, PRIMITIVE_CLICK = 2;
        public Composition addPrimitive(int primitive, float scale) { return this; }
        public VibrationEffect compose() { return new VibrationEffect(); }
    }
}
''',
    "android/os/Vibrator.java": r'''
package android.os;
public class Vibrator {
    public boolean hasVibrator() { return true; }
    public boolean areAllPrimitivesSupported(int primitive) { return true; }
    public boolean hasAmplitudeControl() { return true; }
    public void vibrate(VibrationEffect effect) {}
}
''',
    "android/view/HapticFeedbackConstants.java": r'''
package android.view;
public final class HapticFeedbackConstants { public static final int KEYBOARD_TAP = 3; }
''',
    "android/view/View.java": r'''
package android.view;
public class View {
    public boolean post(Runnable action) { action.run(); return true; }
    public boolean performHapticFeedback(int type) { return true; }
}
''',
}

HARNESS = r'''
import app.msime.android.KeyboardHaptics;
import app.msime.android.KeyboardFeedbackPreferences.HapticStrength;
import android.os.Vibrator;
import android.os.VibrationEffect;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
public final class HapticsPendingCheck {
    public static void main(String[] args) throws Exception {
        CountDownLatch started = new CountDownLatch(1), release = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        Vibrator slow = new Vibrator() {
            @Override public boolean hasVibrator() {
                started.countDown();
                try {
                    if (!release.await(5, TimeUnit.SECONDS)) throw new AssertionError("release missing");
                } catch (InterruptedException error) {
                    throw new AssertionError(error);
                }
                return true;
            }
            @Override public void vibrate(VibrationEffect effect) { calls.incrementAndGet(); }
        };
        // 只用现有 executor 设置完成屏障，不向产品增加测试专用入口。
        var field = KeyboardHaptics.class.getDeclaredField("worker");
        field.setAccessible(true);
        ExecutorService worker = (ExecutorService) field.get(null);
        KeyboardHaptics.play(slow, null, HapticStrength.MEDIUM);
        if (!started.await(5, TimeUnit.SECONDS)) throw new AssertionError("worker did not enter vibrator");
        for (int i = 0; i < 20; i++) KeyboardHaptics.play(slow, null, HapticStrength.MEDIUM);
        release.countDown();
        worker.submit(() -> {}).get(5, TimeUnit.SECONDS);
        System.out.println("vibration calls after one blocked request and 20 extra taps=" + calls.get());
        if (calls.get() != 1) throw new AssertionError("slow vibrator queued stale feedback");
        KeyboardHaptics.play(slow, null, HapticStrength.MEDIUM);
        worker.submit(() -> {}).get(5, TimeUnit.SECONDS);
        if (calls.get() != 2) throw new AssertionError("completed feedback did not reopen slot");

        Vibrator absent = new Vibrator() {
            @Override public boolean hasVibrator() { return false; }
        };
        KeyboardHaptics.play(absent, null, HapticStrength.MEDIUM);
        worker.submit(() -> {}).get(5, TimeUnit.SECONDS);
        KeyboardHaptics.play(slow, null, HapticStrength.MEDIUM);
        worker.submit(() -> {}).get(5, TimeUnit.SECONDS);
        if (calls.get() != 3) throw new AssertionError("absent vibrator did not reopen slot");
    }
}
'''


def main():
    if not shutil.which("javac") or not shutil.which("java"):
        print("skipped: 振动调度回归需要 JDK")
        return
    with tempfile.TemporaryDirectory(prefix="msime-haptics-") as temporary:
        target = Path(temporary)
        sources = []
        for name, content in {**STUBS, "HapticsPendingCheck.java": HARNESS}.items():
            path = target / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            sources.append(str(path))
        subprocess.run([
            "javac", "--release", "17", "-Xlint:all", "-Werror", "-d", str(target),
            str(JAVA / "keyboard/KeyboardHaptics.java"),
            str(JAVA / "keyboard/KeyboardFeedbackPreferences.java"),
            str(JAVA / "ThreadPolicy.java"), *sources,
        ], check=True, timeout=30)
        subprocess.run(["java", "-cp", str(target), "HapticsPendingCheck"],
                       check=True, timeout=15)


if __name__ == "__main__":
    main()
