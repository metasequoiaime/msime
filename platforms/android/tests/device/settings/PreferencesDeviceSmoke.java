package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** Signed fixture targets the preview UID only; no exported preference-writing API. */
public final class PreferencesDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() { return "live preferences deferral, page size, punctuation, malformed-file preservation and recovery"; }
    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject snapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", new JSONObject(options.getJSONObject("preferences").toString())
                .put("candidate_page_size", 5).put("chinese_punctuation", true).put("learning", false));
        try {
            stage = "baseline preferences";
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            // Clear the previous editor's focus before the instrumentation-driven
            // rebind; otherwise its delayed hide request can hide the new keyboard.
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            // Instrumenting the IME package restarts its process. Rebind the system
            // service before opening the editor; this fixture runs only on the guarded AVD.
            shell("ime disable app.msime.android/.MSIMEInputService");
            shell("ime enable app.msime.android/.MSIMEInputService");
            shell("ime set app.msime.android/.MSIMEInputService");
            SystemClock.sleep(1000);
            shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            stage = "baseline editor focus";
            tap(field("msime-test-plain"));
            stage = "baseline typing";
            typePhrase();
            stage = "baseline five candidates";
            // Page size is about how many candidates the page renders, not how many happen to fit
            // on screen: the strip scrolls horizontally, so a wide candidate pushes later slots out
            // of view without changing the page.
            awaitAny(candidateAt(5));
            assertNoRendered(candidateAt(6));
            stage = "active composition defers preferences";
            snapshot.put("revision", revision + 2);
            snapshot.getJSONObject("preferences").put("candidate_page_size", 2).put("chinese_punctuation", false);
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            await(imeTextContains("设置将在组词结束后应用"));
            await(field("msime-test-plain").and(node -> equalsText("nihao", node.getText())));
            stage = "commit preserves composition";
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
            stage = "updated punctuation";
            tapSymbol(",");
            try {
                await(field("msime-test-plain").and(node -> equalsText("你好,", node.getText())));
            } catch (AssertionError error) {
                android.view.accessibility.AccessibilityNodeInfo editor = await(field("msime-test-plain"));
                throw new AssertionError("Editor held " + editor.getText());
            }
            stage = "updated page size";
            typePhrase();
            awaitAny(candidateAt(2));
            assertNoRendered(candidateAt(3));
            stage = "updated second candidate selection";
            tap(candidateAt(2));
            await(field("msime-test-plain").and(node -> node.getText() != null && !node.getText().toString().contains("nihao")));
            stage = "malformed preferences preserve working input";
            byte[] broken = "broken".getBytes(StandardCharsets.UTF_8);
            publish(preferences, broken);
            // 失败提示显示在键盘的诊断行（MSIMEInputService.notice），不用 Toast：安卓 13 起没有通知权限时 Toast 会被系统吞掉。
            await(key("设置读取或应用失败，保留当前设置"));
            typePhrase();
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> node.getText() != null && node.getText().toString().endsWith("你好")));
            if (!java.util.Arrays.equals(broken, Files.readAllBytes(preferences.toPath()))) throw new AssertionError("Malformed file overwritten");
            stage = "valid preferences recover after read failure";
            snapshot.put("revision", revision + 3);
            snapshot.getJSONObject("preferences").put("chinese_punctuation", true);
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            // 键盘每秒轮询一次设置文件，读到新设置后会按「中文标点」重建 123 层，第三排的逗号键面随之从 `,` 换成 `，`。写入后立刻去点，重建可能落在点逗号之后（插进去的还是旧设置的半角逗号），也可能落在点 123 和点逗号之间（拿到的逗号键已被替换，点击返回 false）；API 35 模拟器慢，两种都出现过。所以先等到全角键面：它出现说明新设置已生效、这一层已重建，之后不会再换。
            tap(key("123"));
            await(key("，").and(node -> node.isClickable()));
            tap(key("，"));
            tap(described("切换到字母键盘"));
            await(field("msime-test-plain").and(node -> node.getText() != null && node.getText().toString().endsWith("你好，")));
        } catch (Exception | AssertionError error) {
            shell("screencap -p /data/local/tmp/msime-preferences-failure.png");
            throw error;
        } finally {
            // Stop editor first; the next session starts with the restored configuration.
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
        }
    }
    private void typePhrase() throws Exception {
        String prefix = stage;
        for (String key : new String[] {"n", "i", "h", "a", "o"}) { stage = prefix + ": " + key; tap(key(key)); }
    }

    private java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> candidateAt(
            int index) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 " + index + "：")
            && node.isClickable();
    }

    private void assertNoRendered(
            java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> match) {
        for (var window : automation.getWindows()) {
            if (findAny(window.getRoot(), match) != null)
                throw new AssertionError("Unexpected rendered candidate slot");
        }
    }
    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { /* Discard synthetic command output. */ }
        }
    }
    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try { output.write(contents); target.finishWrite(output); }
        catch (Exception error) { target.failWrite(output); throw error; }
    }
}
