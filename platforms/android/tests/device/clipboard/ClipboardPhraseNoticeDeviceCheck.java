package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Predicate;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;
import org.json.JSONObject;

/** 只用合成记录操作真实剪贴板菜单，测量提示窗口与列表位置。 */
final class ClipboardPhraseNoticeDeviceCheck {
    private static final String SOURCE = "SyntheticNoticeAlpha";
    private static final String SECOND = "SyntheticNoticeBeta";
    private final DeviceSmoke t;
    private final Map<File, byte[]> backups = new LinkedHashMap<>();

    ClipboardPhraseNoticeDeviceCheck(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        File root = t.getTargetContext().getFilesDir();
        File preferences = new File(read(new File(root, "runtime-options.json"))
            .getString("preferences_directory"), "preferences.json");
        File phrases = new File(preferences.getParentFile(), "CommonPhrases.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        for (File file : new File[] {preferences, phrases, history, local,
                new File(preferences.getParentFile(), "CommonPhrases.seeded")})
            backups.put(file, file.isFile() ? Files.readAllBytes(file.toPath()) : null);
        ClipboardManager clipboard = t.getTargetContext().getSystemService(ClipboardManager.class);
        ClipData previous = clipboard.getPrimaryClip();
        try {
            JSONObject document = read(preferences);
            document.put("revision", document.getLong("revision") + 1);
            document.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            publish(preferences, document.toString());
            publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)
                .put("platform.android.clipboard_suggestion", false)).toString());
            publish(phrases, "{\"phrases\":[],\"packs\":[]}");
            JSONArray records = new JSONArray();
            for (String text : new String[] {SOURCE, SECOND, "SyntheticNotice\tInvalid"})
                records.put(new JSONObject().put("text", text).put("timestampMs", 1700000000000L)
                    .put("pinned", true));
            publish(history, records.toString());
            clipboard.clearPrimaryClip();
            t.restartIme();
            openEditor();
            t.tap(t.described("剪贴板"));
            Rect before = bounds(t.awaitStableBounds(card(SOURCE)));
            t.stage = "successful phrase feedback stays at phone top";
            add(SOURCE);
            notice("已添加到常用语");
            requireSame(before, bounds(t.await(card(SOURCE))));
            t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
            awaitPhrase(phrases, SOURCE);
            capture();
            t.stage = "duplicate feedback replaces success without list movement";
            add(SOURCE);
            notice("已经有这条常用语了。");
            requireSame(before, bounds(t.await(card(SOURCE))));
            t.stage = "invalid phrase is rejected at the phone top";
            add("SyntheticNotice\tInvalid");
            notice("这条记录含有制表符等常用语不能保存的字符，可以先「编辑」去掉再添加。");
            t.stage = "expired notice leaves list and input usable";
            awaitNoNotice(6000);
            requireSame(before, bounds(t.await(card(SOURCE))));
            t.stage = "storage failure keeps corrupt bytes and shows its reason at phone top";
            byte[] saved = Files.readAllBytes(phrases.toPath());
            byte[] corrupt = "[synthetic-invalid-phrases".getBytes(StandardCharsets.UTF_8);
            try {
                t.publish(phrases, corrupt);
                add(SECOND);
                notice("常用语文件已损坏，为避免丢失没有改动它。");
                if (!java.util.Arrays.equals(corrupt, Files.readAllBytes(phrases.toPath())))
                    throw new AssertionError("Failed add replaced the synthetic corrupt file");
            } finally { t.publish(phrases, saved); }
            requireSame(before, bounds(t.await(card(SOURCE))));
            t.stage = "hiding the keyboard cancels a delayed save notice";
            try (Gate gate = new Gate(new File(preferences.getParentFile(), "CommonPhrases.json.lock"))) {
                gate.requireHeld();
                add(SECOND);
                SystemClock.sleep(300);
                t.shellDiscard("input keyevent 4");
                awaitNoNotice(2000);
            }
            awaitPhrase(phrases, SECOND);
            SystemClock.sleep(500);
            awaitNoNotice(1000);
            t.stage = "reopened keyboard has no stale notice and accepts input";
            openEditor();
            awaitNoNotice(1000);
            t.tap(t.described("剪贴板"));
            add(SOURCE);
            notice("已经有这条常用语了。");
            t.tap(t.described("返回键盘"));
            t.tap(t.key("n"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("n", node.getText())));
        } finally {
            if (previous == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(previous);
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else t.publish(entry.getKey(), entry.getValue());
            }
            t.restartIme();
        }
    }

    private void openEditor() throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.await(t.key("n"));
    }

    private void add(String text) throws Exception {
        if (!t.await(card(text)).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Synthetic clipboard menu refused");
        Predicate<AccessibilityNodeInfo> action = node -> DeviceSmoke.equalsText("添加到常用语", node.getText())
            && node.getParent() != null
            && DeviceSmoke.equalsText("剪贴板记录操作", node.getParent().getContentDescription());
        if (!t.await(action).performAction(AccessibilityNodeInfo.ACTION_CLICK))
            throw new AssertionError("Synthetic add action refused");
    }

    private Rect notice(String message) {
        AccessibilityNodeInfo node = t.await(t.described("常用语操作提示：" + message));
        Rect bounds = bounds(node);
        int height = t.getTargetContext().getResources().getDisplayMetrics().heightPixels;
        if (bounds.isEmpty() || bounds.top < 0 || bounds.bottom > height / 4)
            throw new AssertionError("Notice is outside phone top: " + bounds + ", height=" + height);
        return bounds;
    }

    private void awaitNoNotice(long timeout) {
        long deadline = SystemClock.uptimeMillis() + timeout;
        do {
            boolean visible = false;
            for (var window : t.automation.getWindows()) {
                if (t.find(window.getRoot(), node -> node.getContentDescription() != null
                        && node.getContentDescription().toString().startsWith("常用语操作提示：")) != null)
                    visible = true;
            }
            if (!visible) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Transient phrase notice did not disappear");
    }

    private void capture() throws Exception {
        Bitmap bitmap = t.automation.takeScreenshot();
        if (bitmap == null) throw new AssertionError("Synthetic notice screenshot unavailable");
        try (FileOutputStream output = new FileOutputStream(new File(t.getTargetContext().getCacheDir(),
                "clipboard-phrase-notice.png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("Synthetic notice screenshot failed");
        } finally { bitmap.recycle(); }
    }

    private Predicate<AccessibilityNodeInfo> card(String text) {
        return node -> DeviceSmoke.equalsText("app.msime.android", node.getPackageName()) && node.isLongClickable()
            && node.getText() != null && node.getText().toString().startsWith(text);
    }

    private void awaitPhrase(File file, String text) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            JSONArray phrases = read(file).getJSONArray("phrases");
            for (int i = 0; i < phrases.length(); i++)
                if (text.equals(phrases.getJSONObject(i).getString("text"))) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic phrase was not saved");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect value = new Rect();
        node.getBoundsInScreen(value);
        return value;
    }

    private static void requireSame(Rect before, Rect after) {
        if (!before.equals(after)) throw new AssertionError("Clipboard card moved: " + before + " -> " + after);
    }

    private void publish(File file, String contents) throws Exception {
        t.publish(file, contents.getBytes(StandardCharsets.UTF_8));
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    /** 独立进程持有和 Rust 相同的 flock，让保存结果真实跨过收起键盘的边界。 */
    private static final class Gate implements AutoCloseable {
        private final Process process;

        Gate(File lock) throws Exception {
            process = new ProcessBuilder("/system/bin/sh", "-c",
                "exec 3>&1; exec 1>\"$1\"; toybox flock -x 1 || exit 1; echo locked >&3; read release",
                "msime-phrase-notice-gate", lock.getAbsolutePath()).redirectErrorStream(true).start();
            try {
                BufferedReader output = new BufferedReader(new InputStreamReader(
                    process.getInputStream(), StandardCharsets.UTF_8));
                long deadline = SystemClock.uptimeMillis() + 15000;
                while (!output.ready()) {
                    if (!process.isAlive() || SystemClock.uptimeMillis() >= deadline)
                        throw new AssertionError("Synthetic phrase lock did not start");
                    SystemClock.sleep(20);
                }
                if (!"locked".equals(output.readLine()))
                    throw new AssertionError("Synthetic phrase lock failed");
            } catch (Exception | AssertionError error) {
                close();
                throw error;
            }
        }

        void requireHeld() {
            if (!process.isAlive()) throw new AssertionError("Synthetic phrase lock stopped early");
        }

        @Override public void close() {
            try {
                process.getOutputStream().close();
                if (!process.waitFor(5, TimeUnit.SECONDS)) {
                    process.destroyForcibly();
                    throw new AssertionError("Synthetic phrase lock did not stop");
                }
            } catch (java.io.IOException error) {
                process.destroyForcibly();
                throw new IllegalStateException("Synthetic phrase lock pipe failed", error);
            } catch (InterruptedException error) {
                process.destroyForcibly();
                Thread.currentThread().interrupt();
                throw new IllegalStateException("Synthetic phrase lock wait interrupted", error);
            }
        }
    }
}
