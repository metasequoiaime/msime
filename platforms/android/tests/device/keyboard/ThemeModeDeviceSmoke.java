package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import app.msime.android.KeyboardSkinModeChoice;
import app.msime.android.CustomSkinLibrary;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 仅在专用 AVD 上切换系统深浅，验证真实键盘、候选与皮肤列表同步刷新。 */
public final class ThemeModeDeviceSmoke extends DeviceSmoke {
    private boolean tracedCompositionEditor;
    private final java.util.ArrayDeque<String> syntheticEvents = new java.util.ArrayDeque<>();

    /** 只记录本套件合成编辑器的选区和输入法控件点击，不收集文字内容。 */
    private void recordSyntheticEvent(android.view.accessibility.AccessibilityEvent event) {
        int kind = event.getEventType();
        boolean selection = kind == android.view.accessibility.AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED
            && equalsText("app.msime.android.test", event.getPackageName());
        boolean click = kind == android.view.accessibility.AccessibilityEvent.TYPE_VIEW_CLICKED
            && equalsText("app.msime.android", event.getPackageName());
        if (!selection && !click) return;
        AccessibilityNodeInfo source = event.getSource();
        if (selection && (source == null || !equalsText("msime-test-plain", source.getContentDescription()))) return;
        String entry = SystemClock.uptimeMillis() + " " + (selection ? "selection" : "click")
            + " " + event.getFromIndex() + ":" + event.getToIndex()
            + " " + event.getClassName() + " "
            + (source == null ? "source-unavailable" : source.getContentDescription());
        synchronized (syntheticEvents) {
            if (syntheticEvents.size() == 60) syntheticEvents.removeFirst();
            syntheticEvents.addLast(entry);
        }
    }

    /** 只记录合成编辑器中的首次覆盖，区分点击夹具与异步回声触发，不关闭面板或放宽配色断言。 */
    private void traceCompositionEditor() {
        if (tracedCompositionEditor || !visible(described("拼写编辑面板"))) return;
        tracedCompositionEditor = true;
        android.os.Bundle diagnostic = new android.os.Bundle();
        diagnostic.putString("theme_first_composition_overlay", stage);
        AccessibilityNodeInfo editor = await(field("msime-test-plain"));
        diagnostic.putInt("editor_selection_start", editor.getTextSelectionStart());
        diagnostic.putInt("editor_selection_end", editor.getTextSelectionEnd());
        synchronized (syntheticEvents) {
            diagnostic.putString("theme_synthetic_events", String.join("\n", syntheticEvents));
        }
        sendStatus(0, diagnostic);
        try { shellDiscard("screencap -p /data/local/tmp/msime-theme-first-overlay.png"); }
        catch (Exception failure) { throw new AssertionError("无法保存合成编辑器覆盖现场", failure); }
    }

    @Override protected void tap(Predicate<AccessibilityNodeInfo> match)
            throws java.util.concurrent.TimeoutException {
        AccessibilityNodeInfo target = awaitAny(match);
        synchronized (syntheticEvents) {
            if (syntheticEvents.size() == 60) syntheticEvents.removeFirst();
            syntheticEvents.addLast(SystemClock.uptimeMillis() + " before-tap " + stage
                + " " + target.getClassName() + " " + target.getContentDescription());
        }
        super.tap(match);
        traceCompositionEditor();
    }

    @Override protected String successDescription() {
        return "light-dark-light keyboard and candidate colours, skin picker filtering and live emoji colour preference";
    }

    @Override protected void runChecks() throws Exception {
        verifyKeyTarget();
        automation.setOnAccessibilityEventListener(this::recordSyntheticEvent);
        JSONObject missingDesign = new JSONObject().put("global_theme", "paper")
            .put("android_keyboard_skin_dark", "design:missing-synthetic");
        if (!"paper".equals(KeyboardSkinModeChoice.selected(missingDesign, true, List.of()).theme()))
            throw new AssertionError("missing design must use the legacy theme");
        JSONObject design = new JSONObject().put("background", 0x223344);
        JSONObject paired = new JSONObject().put("global_theme", "system")
            .put("android_keyboard_skin_light", "theme:paper")
            .put("android_keyboard_skin_dark", "design:synthetic-night");
        List<CustomSkinLibrary.Item> library = List.of(
            new CustomSkinLibrary.Item("synthetic-night", "合成深色", design));
        if (!"paper".equals(KeyboardSkinModeChoice.selected(paired, false, library).theme())
                || KeyboardSkinModeChoice.selected(paired, true, library).design() != design)
            throw new AssertionError("light and dark choices must remain independent");
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
        synthetic.remove("android_keyboard_skin_light");
        synthetic.remove("android_keyboard_skin_dark");
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
            stage = "candidate row is vertically centred in the strip";
            assertCentredInStrip(describedPrefix("候选 1："));
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
            stage = "keyboard skin choices repaint the candidate strip";
            restartIme();
            stage = "refocus existing editor after emoji checks";
            traceCompositionEditor();
            tap(field("msime-test-plain"));
            stage = "keyboard skin choices repaint the candidate strip";
            synthetic.put("theme", "system").put("emoji_theme", "follow")
                .put("android_keyboard_skin_light", "theme:paper")
                .put("android_keyboard_skin_dark", "theme:night");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 6)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColourChange(key("n"), lightLetter);
            int paperLetter = dominantColour(bounds(key("n")));
            tap(key("n"));
            tap(key("i"));
            // 候选条读和键盘同一明暗的皮肤选择（#10749）：换成纸白后候选不能停在跟随系统的配色上。
            awaitColourChange(describedPrefix("候选 1："), lightCandidate);
            shellDiscard("cmd uimode night yes");
            awaitColourChange(key("n"), paperLetter);
            shellDiscard("cmd uimode night no");
            stage = "light keyboard slot restores paper after system mode change";
            awaitColour(key("n"), paperLetter);
            stage = "missing mode slots preserve the legacy theme";
            synthetic.remove("android_keyboard_skin_light");
            synthetic.remove("android_keyboard_skin_dark");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 7)
                .put("preferences", synthetic).toString().getBytes(StandardCharsets.UTF_8));
            awaitColour(key("n"), lightLetter);
            // 没有槽位的旧偏好里，候选条也要回到 global_theme 的配色，和改动前一样（#10749）。
            tap(key("n"));
            tap(key("i"));
            awaitColour(describedPrefix("候选 1："), lightCandidate);
        } catch (Exception | AssertionError failure) {
            // finally 会离开合成编辑器；先留下失败当时的画面，避免事后只能看到已恢复的首页。
            try { shellDiscard("screencap -p /data/local/tmp/msime-theme-mode-failure.png"); }
            catch (Exception captureFailure) { failure.addSuppressed(captureFailure); }
            throw failure;
        } finally {
            automation.setOnAccessibilityEventListener(null);
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            shellDiscard("cmd uimode night no");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    /** 同文字的原始拼写候选不能满足按键查询；数字层等真正按钮仍可由原有描述找到。 */
    @SuppressWarnings("deprecation")
    private void verifyKeyTarget() {
        AccessibilityNodeInfo node = AccessibilityNodeInfo.obtain();
        try {
            node.setPackageName("app.msime.android");
            node.setClassName("android.widget.Button");
            node.setText("N");
            node.setContentDescription("候选 1：N");
            if (key("n").test(node)) throw new AssertionError("原始拼写候选被当作字母键");
            node.setContentDescription("字母 N");
            if (!key("n").test(node)) throw new AssertionError("大写键面不能按小写输入查找");
            node.setText("n");
            if (!key("n").test(node)) throw new AssertionError("小写键面不可查找");
            node.setClassName("android.widget.TextView");
            if (key("n").test(node)) throw new AssertionError("读音行被当作字母键");
            node.setClassName("android.widget.Button");
            node.setText("A");
            node.setContentDescription("按键 A");
            if (!key("a").test(node)) throw new AssertionError("数字层字母按钮不可查找");
        } finally { node.recycle(); }
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

    /** 候选按钮只有一行字高，外面的横向候选条按整行高量出；两者的纵向中心差不超过 2 px，否则候选词贴在候选条的上半截（#10749）。 */
    private void assertCentredInStrip(Predicate<AccessibilityNodeInfo> match) {
        AccessibilityNodeInfo candidate = awaitStableBounds(match);
        AccessibilityNodeInfo strip = candidate.getParent();
        while (strip != null && !equalsText("android.widget.HorizontalScrollView", strip.getClassName()))
            strip = strip.getParent();
        if (strip == null) throw new AssertionError("横向候选条不可用");
        Rect button = new Rect();
        Rect line = new Rect();
        candidate.getBoundsInScreen(button);
        strip.getBoundsInScreen(line);
        if (Math.abs(button.centerY() - line.centerY()) > 2)
            throw new AssertionError("候选没有在候选条里纵向居中：" + button + " / " + line);
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        traceCompositionEditor();
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
        int actual;
        do {
            actual = dominantColour(bounds(match));
            if (actual == expected) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Control did not restore its light-mode colour: expected="
            + Integer.toHexString(expected) + ", actual=" + Integer.toHexString(actual)
            + ", description=" + awaitStableBounds(match).getContentDescription()
            + ", bounds=" + bounds(match) + ", emojiVisible=" + visible(described("表情面板")));
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        for (var window : automation.getWindows())
            if (find(window.getRoot(), match) != null) return true;
        return false;
    }
}
