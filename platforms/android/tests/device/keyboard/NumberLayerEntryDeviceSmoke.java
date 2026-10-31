package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 通过系统点击验证中文和英文的 123 都进数字九宫格：返回键的写法、符号入口去重与符号输入，以及 #+= 扩展页往返。 */
public final class NumberLayerEntryDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Chinese and English number pads: return titles, symbol entry without duplicates, symbol input and #+= round trip";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File state = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!state.toPath().startsWith(root.toPath())) throw new AssertionError("Preferences escaped sandbox");
        File preferences = new File(state, "preferences.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = Files.readAllBytes(preferences.toPath());
        byte[] originalLocal = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        long revision = read(preferences).getLong("revision");
        try {
            for (int variant = 0; variant < 4; variant++) {
                boolean toolbarEmoji = variant != 1;
                boolean toolbarHidden = variant == 2;
                stage = "configure number entry " + variant;
                JSONObject document = new JSONObject(new String(original, StandardCharsets.UTF_8));
                document.put("revision", ++revision);
                document.getJSONObject("preferences").put("scheme", "quanpin")
                    .put("last_chinese_scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                    .put("touch_toolbar", new JSONObject().put("emoji", toolbarEmoji));
                publish(preferences, bytes(document));
                publish(local, bytes(new JSONObject().put("version", 1).put("settings", new JSONObject()
                    .put("platform.android.toolbar_hidden", toolbarHidden))));
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                // 先完成一次合成中文输入，确认重启后的真实会话已就绪，不能把准备期间的入口当作可用入口。
                AccessibilityNodeInfo language = await(key("中").or(key("英")));
                if (equalsText("英", language.getText())) tap(key("英"));
                for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
                tap(key("空格"));
                await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
                stage = "Chinese number returns to letters " + variant;
                tap(key("123"));
                tap(described("返回原键盘").and(node -> equalsText("返回", node.getText())));
                await(key("中"));
                await(key("n"));
                // 英文的 123 同样进数字九宫格（不再是横排数字页）：返回键印 ABC，底行是 符，不论顶部表情入口开没开都不另放 😀。
                tap(key("中"));
                await(key("英"));
                tap(key("123"));
                Predicate<AccessibilityNodeInfo> back = described("返回原键盘").and(node -> equalsText("ABC", node.getText()));
                Predicate<AccessibilityNodeInfo> panel = described("打开符号面板");
                Predicate<AccessibilityNodeInfo> symbol = described("切换符号键盘");
                Predicate<AccessibilityNodeInfo> emoji = key("😀").and(AccessibilityNodeInfo::isVisibleToUser);
                stage = "English number pad entry geometry " + variant;
                await(key("+"));
                await(key("#+="));
                Rect entry = bounds(await(panel));
                Rect backBounds = bounds(await(back));
                if (entry.width() <= 10 || Math.abs(entry.centerY() - backBounds.centerY()) > 2)
                    throw new AssertionError("Number pad symbol entry has invalid bounds");
                if (findVisible(emoji) != null) throw new AssertionError("Number pad duplicates the emoji entry");
                if (findVisible(key("，")) != null || findVisible(key("。")) != null)
                    throw new AssertionError("English number pad shows Chinese punctuation");
                stage = "number pad symbol action " + variant;
                shellDiscard("input tap " + entry.centerX() + " " + entry.centerY());
                await(described("符号网格；长条目加宽"));
                tap(key("中文"));
                tap(key("，"));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && node.getText().toString().contains("，")));
                tap(described("返回键盘"));
                await(back);
                stage = "extended page keeps symbol entry " + variant;
                tap(key("#+="));
                await(symbol);
                if (findVisible(emoji) != null) throw new AssertionError("Extended page duplicates emoji");
                stage = "extended page returns to the number pad " + variant;
                tap(key("123"));
                await(key("+"));
                stage = "English number pad returns to letters " + variant;
                tap(back);
                await(key("英"));
                await(key("n"));
            }
        } finally {
            publish(preferences, original);
            if (originalLocal != null) publish(local, originalLocal);
            else Files.deleteIfExists(local.toPath());
            restartIme();
        }
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (var window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match.and(AccessibilityNodeInfo::isVisibleToUser));
            if (found != null) return found;
        }
        return null;
    }

    private JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private byte[] bytes(JSONObject document) {
        return document.toString().getBytes(StandardCharsets.UTF_8);
    }
    private Rect bounds(AccessibilityNodeInfo node) {
        Rect bounds = new Rect(); node.getBoundsInScreen(bounds); return bounds;
    }
}
