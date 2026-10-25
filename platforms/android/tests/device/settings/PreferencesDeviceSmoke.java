package app.msime.android.test;

import java.io.File;
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
            // 先清除上个编辑器的焦点，避免延迟的隐藏请求收起新键盘。
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            // 等实际选中被测输入法，避免系统的异步回退覆盖选择。
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            stage = "baseline editor focus";
            tap(field("msime-test-plain"));
            stage = "baseline typing";
            typePhrase();
            stage = "baseline five candidates";
            // 横向候选条连续列出全部候选，不再只装引擎当前页（#9472，见 2026-10-23-android-continuous-candidate-strip.md），每页几个只体现在候选条旁的引擎页码上。
            awaitAny(candidateAt(5));
            int fivePerPage = pageCount();
            stage = "active composition defers preferences";
            snapshot.put("revision", revision + 2);
            snapshot.getJSONObject("preferences").put("candidate_page_size", 2).put("chinese_punctuation", false);
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            await(imeTextContains("设置将在组词结束后应用"));
            // 全拼组字在输入框里按音节分词显示（#9448，见 2026-10-23-quanpin-host-preedit-segmentation.md）。
            await(field("msime-test-plain").and(node -> equalsText("ni'hao", node.getText())));
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
            // 同一组字的候选总数 n 不变：每页 5 个时页数是 ⌈n/5⌉，换成每页 2 个后应是 ⌈n/2⌉；页数仍按 5 个或按默认的 6 个计都落不进这个范围。
            int twoPerPage = pageCount();
            if (twoPerPage < (5 * fivePerPage - 3) / 2 || twoPerPage > (5 * fivePerPage + 1) / 2)
                throw new AssertionError("Page count " + twoPerPage + " does not follow page size 2 after " + fivePerPage + " pages of 5");
            stage = "updated second candidate selection";
            tap(candidateAt(2));
            await(field("msime-test-plain").and(node -> node.getText() != null && !node.getText().toString().contains("ni'hao")));
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
            tap(key("#+="));
            await(key("，").and(node -> node.isClickable()));
            tap(key("，"));
            tap(described("切换到字母键盘"));
            await(field("msime-test-plain").and(node -> node.getText() != null && node.getText().toString().endsWith("你好，")));
        } catch (Exception | AssertionError error) {
            shellDiscard("screencap -p /data/local/tmp/msime-preferences-failure.png");
            throw error;
        } finally {
            // Stop editor first; the next session starts with the restored configuration.
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
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

    /** 候选条旁的引擎页码「1/页数」里的页数。 */
    private int pageCount() {
        android.view.accessibility.AccessibilityNodeInfo page = await(node -> equalsText("app.msime.android", node.getPackageName())
            && node.getText() != null && node.getText().toString().matches("1/[0-9]+"));
        return Integer.parseInt(page.getText().toString().substring(2));
    }
}
