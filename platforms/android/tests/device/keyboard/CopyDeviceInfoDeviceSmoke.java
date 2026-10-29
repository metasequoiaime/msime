package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.SystemClock;
import android.os.Build;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在真实输入法菜单的设备信息子面板中复制和直接插入设备信息（#10045），兼容最低 API 并保留编辑器内容。 */
public final class CopyDeviceInfoDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "keyboard copies device information without hiding its preview, preserves editor contents, falls back when suggestions are disabled and inserts single fields in Chinese and English without touching the clipboard";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("Preferences escaped sandbox");
        File local = new File(root, "bootstrap/state/android-settings.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local), originalHistory = bytes(history);
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        JSONObject values = new JSONObject((original == null ? options : read(preferences))
            .getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("privacy_mode", false).put("clipboard_history", true)
            // 固定工具栏入口和方案，让菜单内容与设备上的用户设置无关。
            .put("touch_voice_shortcut", false)
            .put("touch_toolbar", new JSONObject().put("emoji", true).put("clipboard", true))
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
            .put("settings", new JSONObject()) : read(local);
        settings.getJSONObject("settings").put("platform.android.toolbar_hidden", false)
            .put("platform.android.clipboard_suggestion", true)
            .put("platform.android.toolbar_floating", false).put("platform.android.toolbar_text_edit", false);
        try {
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            copyAndCheck(true);
            settings.getJSONObject("settings").put("platform.android.clipboard_suggestion", false);
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            copyAndCheck(false);
            insertAndCheck();
        } finally {
            restore(preferences, original);
            restore(local, originalLocal);
            restore(history, originalHistory);
            restartIme();
        }
    }

    @Override protected void tap(java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> match)
            throws java.util.concurrent.TimeoutException {
        super.tap(match);
        // 编辑器和输入法分属两个进程；提交后的缓存子树可能仍停在旧文本。
        dropStaleNodes();
    }

    private void copyAndCheck(boolean previewEnabled) throws Exception {
        // 先清除上个编辑器的焦点，等系统确认被测输入法已选中，避免异步回退留下系统键盘。
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        stage = "device information editor focus";
        tap(field("msime-test-plain"));
        ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
        ClipData original = clipboard.getPrimaryClip();
        try {
            // 先放合成哨兵，再输入；输入会按产品规则收起旧预览，避免挡住更多入口。
            clipboard.setPrimaryClip(ClipData.newPlainText("synthetic", "MSIME synthetic clipboard sentinel"));
            stage = "device information copy retains editor";
            for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("ni'hao", node.getText())));
            stage = "device information initial commit (preview " + previewEnabled + ")";
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("你好", node.getText())));
            stage = "device information menu open";
            tap(key("更多"));
            await(toolPanel());
            // 按条目的无障碍描述找「设备信息」，翻页由网格的前后翻页动作完成，菜单增减、重排条目都不影响；API 28 上同样读得到菜单磁贴。
            stage = "device information subpanel";
            showTool("设备信息");
            tap(tool("设备信息"));
            stage = "device information menu copy";
            tap(tool("复制设备信息"));
            stage = "device information clipboard result";
            String copied = awaitDeviceInfo(clipboard);
            if (previewEnabled) {
                stage = "device information preview is not hidden by success notice";
                await(node -> {
                    if (described("提示：已复制设备信息").test(node))
                        throw new AssertionError("Copy success notice hides the clipboard preview");
                    return node.refresh() && describedPrefix("粘贴最近复制的内容：").test(node)
                        && node.getText() != null && node.getText().toString().contains("设备信息");
                });
            } else {
                stage = "device information disabled suggestions retain feedback";
                await(described("提示：已复制设备信息"));
            }
            String[] lines = copied.split("\\n");
            // 白名单（#10045 扩展）：原十一项去掉单行的屏幕分辨率，加上 CPU 名称，屏幕参数自成一组放在最后，没有亮度。
            String[] labels = {"应用版本", "品牌", "型号", "系统版本", "系统构建", "处理器架构", "CPU 名称",
                "存储空间", "运行内存", "Android System WebView", "键盘", "屏幕参数"};
            String[] screenLabels = {"分辨率", "密度", "字体缩放", "尺寸大小", "宽高比", "刷新率", "HDR", "HDR 能力", "广色域"};
            if (lines.length != labels.length + screenLabels.length + 2 || !lines[0].endsWith(" 设备信息")
                || !lines[1].startsWith("用于反馈问题时定位输入法与设备兼容性"))
                throw new AssertionError("Device information whitelist changed");
            for (int index = 0; index < labels.length; index++)
                if (!lines[index + 2].startsWith(labels[index] + "："))
                    throw new AssertionError("Device information field missing: " + labels[index]);
            if (!lines[labels.length + 1].equals("屏幕参数："))
                throw new AssertionError("Device information display group missing");
            for (int index = 0; index < screenLabels.length; index++)
                if (!lines[index + labels.length + 2].startsWith(screenLabels[index] + "："))
                    throw new AssertionError("Device information display field missing: " + screenLabels[index]);
            if (copied.contains("nihao") || copied.contains("ni'hao") || copied.contains("你好") || copied.contains("MSIME synthetic clipboard sentinel")
                || !copied.contains("键盘：已启用 · 当前默认") || copied.contains("亮度")
                || copied.contains("\n分辨率：未知") || copied.contains("应用版本：未知"))
                throw new AssertionError("Device information contains invalid diagnostics");
            stage = "device information copy returns to keyboard";
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("你好", node.getText())));
            if (Build.VERSION.SDK_INT < 30) {
                await(key("n").and(node -> node.refresh() && node.isVisibleToUser()));
            }
            stage = "typing after device information copy";
            for (String letter : new String[] {"s", "h", "i"}) tap(key(letter));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("你好shi", node.getText())));
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("你好是", node.getText())));
        } finally {
            if (original == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(original);
        }
    }

    /** 子面板的单项和「全部」直接写进多行输入框：中文默认，切到 English 后标签换成英文；插入不经过剪贴板，连续插入另起一行。 */
    private void insertAndCheck() throws Exception {
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
        stage = "device information insert editor focus";
        tap(field("msime-test-plain"));
        ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
        ClipData original = clipboard.getPrimaryClip();
        try {
            clipboard.setPrimaryClip(ClipData.newPlainText("synthetic", "MSIME synthetic clipboard sentinel"));
            stage = "device information insert initial commit";
            for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("ni'hao", node.getText())));
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> node.refresh() && equalsText("你好", node.getText())));
            stage = "device information insert subpanel";
            tap(key("更多"));
            await(toolPanel());
            showTool("设备信息");
            tap(tool("设备信息"));
            for (String field : new String[] {"品牌", "型号", "系统版本", "处理器架构", "存储空间", "运行内存",
                    "Android System WebView", "CPU 名称", "屏幕参数"})
                awaitAny(tool("插入" + field).and(AccessibilityNodeInfo::isEnabled));
            awaitAny(tool("中文标签").and(node -> node.refresh() && node.isSelected()));
            stage = "device information insert chinese field";
            tap(tool("插入运行内存"));
            await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().matches("你好\\n运行内存：[0-9.]+ [GM]B")));
            // 连点两项不等第一项读完：插入不互相取消，两行都要按点按顺序进来。
            stage = "device information insert consecutive fields";
            tap(tool("插入品牌"));
            tap(tool("插入型号"));
            await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().matches("你好\\n运行内存：[0-9.]+ [GM]B\\n品牌：[^\\n]+\\n型号：[^\\n]+")));
            stage = "device information switch to english";
            tap(tool("英文标签"));
            awaitAny(tool("英文标签").and(node -> node.refresh() && node.isSelected()));
            stage = "device information insert english display";
            tap(tool("插入屏幕参数"));
            await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().contains("\nDisplay:\nResolution: ")
                && node.getText().toString().contains("\nWide color gamut: ")));
            stage = "device information insert english all";
            tap(tool("插入全部设备信息"));
            String inserted = await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().contains("\nMetasequoia IME device information\n")
                && node.getText().toString().contains("\nCPU name: "))).getText().toString();
            if (!inserted.startsWith("你好\n运行内存：") || inserted.contains("Brightness") || inserted.contains("亮度")
                    || inserted.contains("MSIME synthetic clipboard sentinel"))
                throw new AssertionError("Device information insertion is malformed");
            ClipData after = clipboard.getPrimaryClip();
            CharSequence clip = after == null || after.getItemCount() == 0 ? null : after.getItemAt(0).getText();
            if (clip == null || !"MSIME synthetic clipboard sentinel".contentEquals(clip))
                throw new AssertionError("Device information insertion changed the clipboard");
            stage = "device information insert back to tools";
            tap(described("返回工具"));
            awaitAny(tool("设备信息"));
        } finally {
            if (original == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(original);
        }
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static byte[] bytes(File file) throws Exception {
        return file.isFile() ? Files.readAllBytes(file.toPath()) : null;
    }

    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else publish(file, original);
    }

    /** 测试编辑器保持前台，由当前默认输入法的 UID 读取系统剪贴板，不输出报告正文。 */
    private String awaitDeviceInfo(ClipboardManager clipboard) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            ClipData clip = clipboard.getPrimaryClip();
            CharSequence text = clip == null || clip.getItemCount() == 0 ? null : clip.getItemAt(0).getText();
            if (text != null && text.toString().contains("\n应用版本：")) return text.toString();
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Keyboard device information copy did not finish");
    }

}
