#!/usr/bin/env python3
"""执行空格反馈逻辑；模拟已有宿主接口，底层发键、边界和振动由各自回归覆盖。"""

from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"
STUBS = {
    "android/view/View.java": "package android.view; public class View {}",
    "android/view/KeyEvent.java": """
package android.view;
public final class KeyEvent { public static final int KEYCODE_DPAD_LEFT = 21, KEYCODE_DPAD_RIGHT = 22; }
""",
    "app/msime/android/KeyboardHaptics.java": """
package app.msime.android;
import android.view.View;
import java.util.ArrayList;
import java.util.List;
final class KeyboardHaptics {
    static final List<String> events = new ArrayList<>();
    static void play(Object vibrator, View source, KeyboardFeedbackPreferences.HapticStrength strength) {
        events.add("haptic:" + strength);
    }
}
""",
    "app/msime/android/MSIMEInputService.java": """
package app.msime.android;
final class MSIMEInputService {
    Object connection = new Object(), vibrator = new Object();
    final SpaceCursorMovement cursorMovement = new SpaceCursorMovement();
    final Invalidations selectionEcho = new Invalidations();
    final Panel imeTextEditPanel = new Panel();
    boolean hapticsEnabled = true, sent = true;
    Runnable duringSend = () -> {};
    KeyboardFeedbackPreferences.HapticStrength hapticStrength = KeyboardFeedbackPreferences.HapticStrength.SYSTEM;
    boolean sendEditorKeyEvents(int keyCode) {
        KeyboardHaptics.events.add("move:" + keyCode);
        duringSend.run();
        return sent;
    }
    static final class Invalidations { void invalidate() {} }
    static final class Panel {
        Integer filteredSteps;
        record Plan(int horizontal) {}
        Plan cursorPlan(Object target, int offset, int vertical, boolean selecting) {
            return new Plan(filteredSteps == null ? offset : filteredSteps);
        }
    }
}
""",
}

HARNESS = """
package app.msime.android;
import android.view.View;
import java.util.List;
public final class SpaceCursorHapticsCheck {
    static MSIMEInputService s;
    static final View source = new View();
    static void reset() {
        s = new MSIMEInputService();
        s.cursorMovement.begin(0, s.connection);
        KeyboardHaptics.events.clear();
    }
    static void drag(float x) {
        ImeSpaceCursor.move(s, source, s.cursorMovement.advance(x, s.connection, 12));
    }
    static void expect(String... events) {
        if (!KeyboardHaptics.events.equals(List.of(events)))
            throw new AssertionError("预期 " + List.of(events) + "，实际 " + KeyboardHaptics.events);
    }
    public static void main(String[] args) {
        reset(); drag(11); expect();
        drag(24); expect("move:22", "haptic:SYSTEM", "move:22", "haptic:SYSTEM");
        reset(); s.imeTextEditPanel.filteredSteps = -1; drag(-36);
        expect("move:21", "haptic:SYSTEM");
        for (int direction : new int[] {-1, 1}) {
            reset(); s.imeTextEditPanel.filteredSteps = 0; drag(direction * 24); expect();
        }
        reset(); s.sent = false; drag(12); expect("move:22");
        reset(); s.hapticsEnabled = false; drag(12); expect("move:22");
        reset(); s.cursorMovement.cancel(); drag(24); expect();
        reset(); s.duringSend = s.cursorMovement::cancel; drag(36); expect("move:22");
        reset(); s.duringSend = () -> s.connection = new Object(); drag(36); expect("move:22");
        reset(); s.connection = null; drag(24); expect();
        for (var strength : KeyboardFeedbackPreferences.HapticStrength.values()) {
            reset(); s.hapticStrength = strength; drag(12); expect("move:22", "haptic:" + strength);
        }
        System.out.println("空格逐步反馈：阈值、过滤、发送失败、取消、连接变化与振动偏好通过");
    }
}
"""


def main():
    if not shutil.which("javac") or not shutil.which("java"):
        print("skipped: 空格逐步反馈回归需要 JDK 17+")
        return
    with tempfile.TemporaryDirectory(prefix="msime-space-haptics-") as temporary:
        target = Path(temporary)
        sources = []
        for name, content in {**STUBS, "app/msime/android/SpaceCursorHapticsCheck.java": HARNESS}.items():
            file = target / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(content, encoding="utf-8")
            sources.append(str(file))
        production = ["core/ImeSpaceCursor.java", "keyboard/SpaceCursorMovement.java",
                      "keyboard/KeyboardFeedbackPreferences.java"]
        subprocess.run(["javac", "--release", "17", "-Xlint:all", "-Werror", "-d", str(target),
                        *[str(JAVA / name) for name in production], *sources], check=True, timeout=30)
        subprocess.run(["java", "-cp", str(target), "app.msime.android.SpaceCursorHapticsCheck"],
                       check=True, timeout=15)


if __name__ == "__main__":
    main()
