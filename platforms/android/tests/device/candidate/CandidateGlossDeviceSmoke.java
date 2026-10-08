package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** Device acceptance for default-on offline candidate gloss presentation and selection identity. */
public final class CandidateGlossDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "offline candidate gloss strip, expanded long press, selection and opt-out";
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
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false);
        try {
            stage = "default offline gloss preference";
            publish(preferences, snapshot(revision + 1, new JSONObject(base.toString())));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            typeGreeting();

            stage = "candidate strip offline gloss";
            AccessibilityNodeInfo strip = await(glossCandidate());
            if (strip.getText() == null || !strip.getText().toString().contains("hello"))
                throw new AssertionError("Candidate strip did not expose the offline annotation");

            stage = "expanded candidate offline gloss";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            AccessibilityNodeInfo expanded = await(expandedGlossCandidate());
            stage = "expanded candidate long press menu";
            if (!expanded.isLongClickable()
                    || !expanded.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Expanded candidate did not expose long press");
            await(glossMenuItem());
            if (!automation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK))
                throw new AssertionError("Candidate gloss menu did not close");
            tap(candidatePanelClose());

            stage = "glossed candidate selection identity";
            tap(glossCandidate());
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            stage = "disable offline gloss preference";
            publish(preferences, snapshot(revision + 2,
                new JSONObject(base.toString()).put("candidate_english_gloss", false)));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            typeGreeting();
            await(plainCandidate());
            SystemClock.sleep(1200);
            if (findVisible(glossCandidate()) != null)
                throw new AssertionError("Disabled candidate gloss became visible");
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private byte[] snapshot(long revision, JSONObject preferences) throws Exception {
        return new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8);
    }

    private void typeGreeting() throws Exception {
        for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
    }

    private Predicate<AccessibilityNodeInfo> glossCandidate() {
        return node -> preview(node) && node.getContentDescription() != null
            && node.getText() != null
            && !node.getText().toString().startsWith("1 ")
            && node.getContentDescription().toString().startsWith("候选 1：你好")
            && node.getContentDescription().toString().contains("英文释义：hello");
    }

    private Predicate<AccessibilityNodeInfo> expandedGlossCandidate() {
        return node -> preview(node) && node.getText() != null
            && node.getText().toString().startsWith("你好  ")
            && node.getText().toString().contains("hello")
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 1：你好");
    }

    private Predicate<AccessibilityNodeInfo> glossMenuItem() {
        return node -> preview(node) && equalsText("hello", node.getText()) && node.isClickable();
    }

    private Predicate<AccessibilityNodeInfo> plainCandidate() {
        return node -> preview(node) && node.getContentDescription() != null
            && node.getText() != null
            && !node.getText().toString().startsWith("1 ")
            && node.getContentDescription().toString().startsWith("候选 1：你好")
            && !node.getContentDescription().toString().contains("英文释义：");
    }

    private Predicate<AccessibilityNodeInfo> candidatePanelClose() {
        return node -> preview(node) && node.getContentDescription() != null
            && equalsText("收起候选面板", node.getContentDescription());
    }

    private boolean preview(AccessibilityNodeInfo node) {
        return equalsText("app.msime.android", node.getPackageName());
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> predicate) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), predicate);
            if (found != null) return found;
        }
        return null;
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
