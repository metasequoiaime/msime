package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 仅在专用 AVD 上切换系统深浅，验证真实键盘、候选与皮肤列表同步刷新。 */
public final class ThemeModeDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "light-dark-light keyboard and candidate colours, skin picker filtering and live emoji colour preference";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject synthetic = new JSONObject(options.getJSONObject("preferences").toString())
            .put("theme", "system").put("global_theme", "system")
            .put("screen_keyboard_theme", "follow").put("candidate_theme", "follow")
            .put("emoji_theme", "follow").put("handwriting_theme", "follow")
            .put("candidate_english_gloss", false);
        try {
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            shellDiscard("cmd uimode night no");
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            stage = "light composition baseline";
            tap(key("n"));
            tap(key("i"));
            Rect letter = bounds(key("n"));
            Rect candidate = bounds(describedPrefix("候选 1："));
            int lightLetter = dominantColour(letter);
            int lightCandidate = dominantColour(candidate);
            stage = "dark configuration updates candidates with the keyboard";
            shellDiscard("cmd uimode night yes");
            awaitColourChange(key("n"), lightLetter);
            // 系统重建输入视图会结束原组字，重新输入合成拼音后检查候选配色。
            tap(key("n"));
            tap(key("i"));
            awaitColourChange(describedPrefix("候选 1："), lightCandidate);
            stage = "light configuration restores both colours";
            shellDiscard("cmd uimode night no");
            awaitColour(key("n"), lightLetter);
            tap(key("n"));
            tap(key("i"));
            awaitColour(describedPrefix("候选 1："), lightCandidate);
            tap(key("空格"));
            stage = "light skin picker";
            tap(describedPrefix("切换键盘皮肤；当前"));
            await(described("屏幕键盘皮肤 纸白"));
            awaitGone(described("屏幕键盘皮肤 夜青"));
            stage = "picker switches to dark themes";
            shellDiscard("cmd uimode night yes");
            tap(describedPrefix("切换键盘皮肤；当前"));
            await(described("屏幕键盘皮肤 夜青"));
            awaitGone(described("屏幕键盘皮肤 纸白"));
            stage = "picker switches back to light themes";
            shellDiscard("cmd uimode night no");
            tap(describedPrefix("切换键盘皮肤；当前"));
            await(described("屏幕键盘皮肤 纸白"));
            awaitGone(described("屏幕键盘皮肤 夜青"));
            tap(describedPrefix("切换键盘皮肤；当前"));
            stage = "emoji follows light baseline";
            tap(described("表情"));
            int lightEmoji = dominantColour(bounds(described("表情面板")));
            stage = "live dark preference updates the open emoji panel";
            synthetic.put("theme", "dark");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 2)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColourChange(described("表情面板"), lightEmoji);
            stage = "live light preference restores the open emoji panel";
            synthetic.put("theme", "light");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 3)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColour(described("表情面板"), lightEmoji);
            stage = "emoji override repaints without changing the keyboard mode";
            synthetic.put("emoji_theme", "dark");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 4)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColourChange(described("表情面板"), lightEmoji);
            stage = "emoji light override restores its colour";
            synthetic.put("emoji_theme", "light");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 5)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColour(described("表情面板"), lightEmoji);
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            shellDiscard("cmd uimode night no");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            boolean present = false;
            for (var window : automation.getWindows()) {
                if (findAny(window.getRoot(), match) != null) { present = true; break; }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Picker retained a theme from the previous colour mode");
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect result = new Rect();
        awaitStableBounds(match).getBoundsInScreen(result);
        return result;
    }

    /** 只统计合成编辑器里指定输入法控件的像素；截图不落盘，不读取其他窗口。 */
    private int dominantColour(Rect bounds) {
        Bitmap screenshot = automation.takeScreenshot();
        if (screenshot == null) throw new AssertionError("Screenshot unavailable");
        try {
            java.util.Map<Integer, Integer> counts = new java.util.HashMap<>();
            int bestColour = 0;
            int bestCount = 0;
            for (int y = bounds.top + 2; y < Math.min(bounds.bottom - 2, screenshot.getHeight()); y += 3) {
                for (int x = bounds.left + 2; x < Math.min(bounds.right - 2, screenshot.getWidth()); x += 3) {
                    int colour = screenshot.getPixel(x, y);
                    int count = counts.getOrDefault(colour, 0) + 1;
                    counts.put(colour, count);
                    if (count > bestCount) { bestColour = colour; bestCount = count; }
                }
            }
            if (bestCount == 0) throw new AssertionError("Control has no visible pixels");
            return bestColour;
        } finally { screenshot.recycle(); }
    }

    private void awaitColourChange(Predicate<AccessibilityNodeInfo> match, int oldColour) {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            if (dominantColour(bounds(match)) != oldColour) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Control retained the previous colour mode");
    }

    private void awaitColour(Predicate<AccessibilityNodeInfo> match, int expected) {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            if (dominantColour(bounds(match)) == expected) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Control did not restore its light-mode colour");
    }
}
