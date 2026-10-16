package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 通过系统点击验证数字页去重、符号输入，以及关闭顶部入口后的表情兜底。 */
public final class NumberLayerEntryDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "number layer symbol action, emoji fallback for disabled or hidden toolbar, and restored entry geometry";
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
                boolean fallback = !toolbarEmoji || toolbarHidden;
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
                // 中文数字页使用九宫格；这里验证携带可选表情键的英文横排数字页。
                tap(key("中"));
                await(key("英"));
                tap(key("123"));
                Predicate<AccessibilityNodeInfo> symbol = described("切换符号键盘");
                Predicate<AccessibilityNodeInfo> emoji = key("😀").and(AccessibilityNodeInfo::isVisibleToUser);
                stage = "entry geometry and exclusivity " + variant;
                Rect entry = bounds(await(fallback ? emoji : symbol));
                Rect space = bounds(await(key("空格")));
                if (entry.width() <= 10 || Math.abs(entry.centerY() - space.centerY()) > 2)
                    throw new AssertionError("Number entry has invalid bounds");
                if (findVisible(fallback ? symbol : emoji) != null)
                    throw new AssertionError("Duplicate number-layer entry");
                stage = "number entry action " + variant;
                shellDiscard("input tap " + entry.centerX() + " " + entry.centerY());
                if (fallback) {
                    stage = "emoji panel opens " + variant;
                    await(described("表情面板"));
                    stage = "emoji panel has return action " + variant;
                    await(described("返回键盘"));
                } else {
                    await(described("符号网格；长条目加宽"));
                    tap(key("中文"));
                    tap(key("，"));
                    await(field("msime-test-plain").and(node -> node.getText() != null
                        && node.getText().toString().contains("，")));
                }
                tap(described("返回键盘"));
                await(fallback ? emoji : symbol);
                stage = "extended page keeps symbol entry " + variant;
                tap(key("#+="));
                await(symbol);
                if (findVisible(emoji) != null) throw new AssertionError("Extended page duplicates emoji");
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
