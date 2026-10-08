package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
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
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
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

    private void restartIme() throws Exception {
        shell("ime disable app.msime.android/.MSIMEInputService");
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try {
            output.write(contents);
            target.finishWrite(output);
        } catch (Exception error) {
            target.failWrite(output);
            throw error;
        }
    }
}
