package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/**
 * 复制图片后一键发送（#6781）的设备验收：合成编辑器把一张 PNG 放进系统剪贴板，收图片的输入框里工具栏出现「发送复制的图片」，点一下图片经 FileProvider 的缓存副本和 commitContent 到达输入框，字节一致；不收图片的输入框里不出现。
 *
 * <p>「最近复制」这一行的出现和收起都用 {@link #findFresh} 绕过 UiAutomation 的节点缓存来查：安卓 11（API 30）以下输入法窗口的内容变化不会让缓存失效，按缓存查，这一行显示了十几秒也看不到，收起后又一直还在（API 28 上实测）。
 *
 * <p>JVM 冒烟碰不到的是这一整条设备路径：`:ime` 进程读别的应用放进剪贴板的 content URI、在应用缓存里写副本、主进程的 FileProvider 交出 URI、系统把读权限经 commitContent 授给输入框。它以应用本身为目标包运行，才能打开剪贴板历史（「最近复制」跟着它走）并检查键盘进程留下的副本。
 */
public final class ClipboardImageDeviceSmoke extends DeviceSmoke {
    private static final String SEND = "发送最近复制的图片";

    @Override protected String successDescription() {
        return "copied image offered in an image editor, sent through commitContent intact, and not offered in a plain editor";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString());
        File copies = new File(getTargetContext().getCacheDir(), "clipboard-images");
        try {
            stage = "enable clipboard history";
            JSONObject enabled = new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", new JSONObject(base.toString()).put("clipboard_history", true));
            publish(preferences, enabled.toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();

            stage = "image editor focus";
            tap(field("msime-test-image"));
            await(key("n"));
            stage = "copy image in image editor";
            tap(field("msime-test-copy-image"));
            String copied = awaitStatus("copied:");
            stage = "image chip in image editor";
            tapFresh(described(SEND));
            stage = "image received intact";
            // 声明的类型和 FileProvider 按副本扩展名报出的类型都是 image/png。
            String expected = "received:image/png:image/png:" + copied.substring("copied:".length());
            String received = awaitStatus("received:");
            if (!expected.equals(received))
                throw new AssertionError("editor received " + received + ", expected " + expected);
            stage = "image chip retired after sending";
            assertAbsentFresh(described(SEND));
            stage = "one cached copy";
            File[] files = copies.listFiles();
            if (files == null || files.length != 1 || !files[0].getName().endsWith(".png"))
                throw new AssertionError("expected exactly one cached PNG copy, found "
                    + (files == null ? "none" : String.valueOf(files.length)));

            stage = "plain editor focus";
            tap(field("msime-test-plain"));
            await(key("n"));
            stage = "copy image in plain editor";
            tap(field("msime-test-copy-image"));
            awaitStatus("copied:");
            stage = "no image chip in plain editor";
            assertAbsentFresh(described(SEND));

            // 同一张图在一分钟内回到收图片的输入框时出现：是否显示按当前输入框判断，不是复制那一刻判断。
            stage = "image chip back in image editor";
            tap(field("msime-test-image"));
            awaitFresh(described(SEND));
            tapFresh(described("不再显示这条复制的内容"));
            assertAbsentFresh(described(SEND));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
        }
    }

    /** 等合成编辑器的状态行以 `prefix` 开头，返回整行。 */
    private String awaitStatus(String prefix) {
        AccessibilityNodeInfo node = await(field("msime-test-image-status").and(candidate ->
            candidate.getText() != null && candidate.getText().toString().startsWith(prefix)));
        return node.getText().toString();
    }

    /** 等界面空闲，再确认看得见的窗口里没有它。 */
    private void assertAbsentFresh(Predicate<AccessibilityNodeInfo> match) throws java.util.concurrent.TimeoutException {
        automation.waitForIdle(500, 5000);
        SystemClock.sleep(800);
        if (findFresh(match) != null)
            throw new AssertionError("Control appeared where it should not; IME showed " + imeTexts());
    }

    private AccessibilityNodeInfo awaitFresh(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            AccessibilityNodeInfo found = findFresh(match);
            if (found != null) return found;
            SystemClock.sleep(200);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected synthetic UI state was not observed; IME showed " + imeTexts());
    }

    /** 用无障碍点击点它（和 {@link #tap} 对输入法控件的做法一样），节点每次重新取。 */
    private void tapFresh(Predicate<AccessibilityNodeInfo> match) {
        for (int attempt = 1; attempt <= 3; attempt++) {
            if (awaitFresh(match).performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                SystemClock.sleep(150);
                return;
            }
            SystemClock.sleep(300);
        }
        throw new AssertionError("Synthetic control action failed");
    }

    /** 逐个节点 `refresh()` 后再判断，不用缓存里的旧节点。 */
    private AccessibilityNodeInfo findFresh(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = findFresh(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    private AccessibilityNodeInfo findFresh(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node == null || !node.refresh()) return null;
        if (node.isVisibleToUser() && match.test(node)) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = findFresh(node.getChild(index), match);
            if (found != null) return found;
        }
        return null;
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
