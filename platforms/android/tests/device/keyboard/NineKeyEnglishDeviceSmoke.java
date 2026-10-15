package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** Device acceptance for Engine-owned English candidates entered through the real nine-key grid. */
public final class NineKeyEnglishDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key English candidate discovery and exact InputConnection commit";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", true)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable shared nine-key English preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            stage = "real nine-key digit entry";
            tap(key("MNO"));
            tap(key("JKL"));
            // 九键的组字（按下的数字）不写进输入框，只显示在键盘自己的预编辑栏上（MSIMEInputService 里 hidesNineKeyComposing 的说明）：输入框保持空，引擎收到两个数字后有了候选，「展开」可用。
            await(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(field("msime-test-plain").and(node -> node.getText() == null
                || node.getText().length() == 0 || equalsText("Plain editor", node.getText())));

            // dict-v1.0.0 has zero English weights, so production acceptance deliberately
            // checks the complete Engine candidate set instead of assuming first-page rank.
            stage = "complete nine-key English candidates";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            tap(candidate("ok"));

            stage = "nine-key English commit identity";
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok")));

            // 第一格长按的 1 经真实 InputConnection 上屏；短按的常用标点见下面的 quick punctuation 一段。
            stage = "nine-key long-press one";
            AccessibilityNodeInfo one = await(key("@#"));
            if (!one.isLongClickable() || !one.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Key 1 must offer its literal digit");
            tap(described("输入 1"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok1")));

            stage = "nine-key numeric one and layer restoration";
            tap(key("123"));
            tap(key("1"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok11")));
            tap(key("九键"));
            await(key("@#").and(AccessibilityNodeInfo::isLongClickable));

            // 空闲时第一格短按不打开符号面板，而是把常用标点放进候选条；点一个经 Engine 上屏（中文标点下 \ 写成 、），候选条随后回到空闲。
            stage = "nine-key 1 key quick punctuation";
            tap(key("@#"));
            tap(described("常用标点 5：、"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok11、")));
            awaitGone(describedPrefix("常用标点 "));

            // 符号面板改从九键底行最左边的 符 打开。
            stage = "nine-key bottom row symbol panel key";
            tap(described("打开符号面板"));
            await(described("符号面板"));
            tap(described("关闭符号面板"));
            await(key("@#"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            // 英文候选按英文词典的写法显示（ok 显示为 OK），不区分大小写比较。
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equalsIgnoreCase(text);
        };
    }


    /** 等匹配的节点从所有窗口里消失。 */
    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            boolean shown = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                shown |= find(window.getRoot(), match) != null;
            if (!shown) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("The quick punctuation stayed in the candidate strip after one was used; IME showed " + imeTexts());
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
