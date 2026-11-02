package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真实输入法键位交换：中文、英文、数字层及外语九键都使用同一个本机设置。 */
public final class LanguageNumberKeySwapDeviceSmoke extends DeviceSmoke {
    private static final String SETTING = "platform.android.swap_language_number_keys";

    @Override protected String successDescription() {
        return "language/number key positions, English and foreign layouts, restore and persistence";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        File local = new File(new File(new File(root, "bootstrap"), "state"), "android-settings.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (String[] layout : new String[][] {{"quanpin", "quanpin", "twenty_six_key"},
                    {"nine_key", "quanpin", "nine_key"}, {"korean", "korean", "twenty_six_key"},
                    {"japanese_nine_key", "japanese", "nine_key"}, {"zhuyin_nine_key", "zhuyin", "nine_key"}}) {
                stage = "prepare " + layout[0];
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", layout[1]).put("touch_keyboard_layout", layout[2])
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put(layout[0])).put("selected", layout[0]));
                publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
                for (boolean swapped : new boolean[] {false, true, false}) {
                    stage = layout[0] + (swapped ? " swapped positions" : " default positions");
                    publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject().put(SETTING, swapped))
                        .toString().getBytes(StandardCharsets.UTF_8));
                    restartIme();
                    openEditor();
                    stage = layout[0] + " ready language key";
                    AccessibilityNodeInfo mode = await(languageKey());
                    if (hasKey(key("q")) && equalsText("英", mode.getText())) {
                        stage = layout[0] + " restore selected scheme";
                        tap(languageKey());
                    }
                    stage = layout[0] + " selected layout";
                    await(layout[0].equals("nine_key") ? key("ABC")
                        : layout[0].equals("japanese_nine_key") ? key("あ")
                        : layout[0].equals("zhuyin_nine_key") ? described("注音 1 ㄅㄆㄇㄈ") : languageKey());
                    if (layout[0].equals("quanpin") && !hasKey(key("123"))) {
                        tap(described("返回原键盘"));
                        await(key("123"));
                    }
                    await(languageKey());
                    stage = layout[0] + (swapped ? " swapped positions" : " default positions");
                    checkPositions(key("123"), swapped);
                    if (layout[0].equals("japanese_nine_key") && swapped) {
                        // 交换后日语九键的语言键在左列最下一格，符 在它上面（#10437）。
                        stage = "japanese_nine_key language key at bottom of left column";
                        checkBelowSymbol();
                    }
                    if (layout[0].equals("quanpin")) {
                        // 英文切换和数字层按键动作仍工作，位置保持交换状态。
                        stage = "quanpin English switch";
                        tap(languageKey());
                        await(key("英"));
                        checkPositions(key("123"), swapped);
                        tap(key("123"));
                        // 英文也借数字九宫格：底行有语言键，返回键（印 ABC）与它按同一个交换设置排，返回后字母层照样保持交换位置。
                        await(described("返回原键盘").and(node -> equalsText("ABC", node.getText())));
                        checkPositions(described("返回原键盘"), swapped);
                        tap(described("返回原键盘"));
                        await(key("123"));
                        checkPositions(key("123"), swapped);
                        stage = "quanpin restore Chinese";
                        tap(languageKey());
                        await(key("中"));
                    }
                }
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else publish(local, originalLocal);
            restartIme();
        }
    }

    // API 28 没有 clearCache；布局重建后刷新每个节点，避免沿用已拆掉的键面和子树。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private boolean hasKey(Predicate<AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), predicate) != null) return true;
        return false;
    }

    private Predicate<AccessibilityNodeInfo> languageKey() {
        // 会话准备期间的语言键可以长按，却不执行点击；等待真实切换目标再测动作。
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isClickable()
            && (equalsText("英", node.getText()) || equalsText("中", node.getText()))
            && node.getContentDescription() != null
            && (node.getContentDescription().toString().startsWith("切换到")
                || node.getContentDescription().toString().startsWith("按键 切换到"));
    }

    private void checkPositions(Predicate<AccessibilityNodeInfo> layerKey, boolean swapped) throws Exception {
        Rect language = new Rect(), layer = new Rect();
        awaitStableBounds(languageKey()).getBoundsInScreen(language);
        awaitStableBounds(layerKey).getBoundsInScreen(layer);
        if (swapped ? language.centerX() >= layer.centerX() : language.centerX() <= layer.centerX())
            throw new AssertionError("Language/number keys did not use selected order");
    }

    private void checkBelowSymbol() throws Exception {
        Rect language = new Rect(), symbol = new Rect();
        awaitStableBounds(languageKey()).getBoundsInScreen(language);
        // 只认左列的 符 键（读屏描述带「按键」前缀），不认顶部工具栏的符号快捷键。
        awaitStableBounds(key("符").and(node -> node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("按键 打开符号面板"))).getBoundsInScreen(symbol);
        if (language.centerX() < symbol.left || language.centerX() > symbol.right || language.top < symbol.bottom)
            throw new AssertionError("Japanese nine-key language key is not below 符 in the left column: language="
                + language.toShortString() + " symbol=" + symbol.toShortString());
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }
}
