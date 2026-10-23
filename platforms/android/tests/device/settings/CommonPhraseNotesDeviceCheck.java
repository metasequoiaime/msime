package app.msime.android.test;

import android.content.Context;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用合成记录验证原生设置、共享存储、备份和真实键盘的备注边界。 */
final class CommonPhraseNotesDeviceCheck {
    private static final String BODY = "Synthetic phrase alpha\nSynthetic phrase beta\nSynthetic phrase gamma";
    private static final String NOTE = "合成用途第一行\n合成用途第二行\n合成用途第三行";
    private static final String CHANGED = "合成修改后的用途";
    private final DeviceSmoke test;
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private File phrases;

    CommonPhraseNotesDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        File root = test.getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Synthetic phrase directory escaped app sandbox");
        File preferences = new File(directory, "preferences.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        phrases = new File(directory, "CommonPhrases.json");
        File seeded = new File(directory, "CommonPhrases.seeded");
        for (File file : new File[] {preferences, local, phrases, seeded})
            backups.put(file, file.isFile() ? Files.readAllBytes(file.toPath()) : null);
        try {
            JSONObject document = read(preferences);
            document.put("revision", document.getLong("revision") + 1);
            document.getJSONObject("preferences").put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key");
            write(preferences, document.toString());
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false).put("platform.android.toolbar_phrase", true)).toString());
            write(phrases, new JSONObject().put("phrases", new JSONArray())
                .put("packs", new JSONArray()).toString());
            write(seeded, "");
            test.restartIme();
            settings();
            test.stage = "native add dialog exposes separate optional note";
            test.tap(text("添加").and(AccessibilityNodeInfo::isClickable));
            test.await(described("常用语备注"));
            set("常用语正文", BODY);
            set("常用语备注", NOTE);
            test.tap(text("添加").and(AccessibilityNodeInfo::isEnabled).and(AccessibilityNodeInfo::isClickable));
            awaitStored(BODY, NOTE);
            test.await(text("备注：" + NOTE));
            test.stage = "body button inserts full original body without note";
            insert(false);
            test.stage = "note area inserts full original body without note after IME restart";
            test.restartIme();
            insert(true);
            test.stage = "native note survives page recreation and can be modified";
            settings();
            edit(NOTE, CHANGED);
            awaitStored(BODY, CHANGED);
            test.stage = "backup export and restore preserve independent note";
            backupRoundTrip(directory);
            test.stage = "legacy replace request preserves note";
            String id = stored().getString("id");
            Class<?> store = store();
            Object result = store.getMethod("replace", Context.class, String.class, String.class)
                .invoke(null, test.getTargetContext(), id, BODY);
            requireOk(result);
            awaitStored(BODY, CHANGED);
            test.stage = "clearing note retains body and omits empty field";
            settings();
            edit(CHANGED, "");
            awaitStored(BODY, "");
            if (stored().has("note")) throw new AssertionError("Empty note was serialized");
            insert(false);
            test.stage = "removing phrase also removes note";
            settings();
            test.tap(text("删除").and(AccessibilityNodeInfo::isClickable));
            awaitCount(0);
        } finally {
            // 页面退出且异步保存结束后才恢复夹具，避免迟到的写入覆盖原文件。
            test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            test.waitForIdleSync();
            awaitStoreIdle();
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else test.publish(entry.getKey(), entry.getValue());
            }
            test.restartIme();
        }
    }

    private void settings() throws Exception {
        // CLEAR_TASK 确保走原生页面重建；不用 startActivitySync 等待复用实例。
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page PHRASES");
        test.dropStaleNodes();
        // 分类卡片增加了页面高度；先滚动到原有添加区域，再保留原来的可见性断言。
        AccessibilityNodeInfo entry = test.awaitAny(text("添加常用语"));
        if (!entry.isVisibleToUser()) {
            if (!entry.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()))
                throw new AssertionError("Common phrase add section could not be scrolled into view");
            test.dropStaleNodes();
        }
        test.await(text("添加常用语"));
    }

    private void edit(String before, String after) throws Exception {
        test.tap(described(BODY + (before.isEmpty() ? "" : "，备注：" + before) + "，点按修改"));
        test.await(described("常用语备注").and(node -> before.equals(value(node))));
        set("常用语备注", after);
        test.tap(text("保存").and(AccessibilityNodeInfo::isEnabled));
    }

    private void insert(boolean note) throws Exception {
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
        test.tap(test.field("msime-test-plain"));
        test.await(test.key("n"));
        test.tap(described("常用语"));
        test.await(described("常用语备注：" + NOTE).or(described("常用语 " + BODY.substring(0, 20))));
        if (note) {
            test.await(described("常用语备注：" + NOTE));
            android.graphics.Bitmap screenshot = test.automation.takeScreenshot();
            if (screenshot == null) throw new AssertionError("Synthetic note screenshot unavailable");
            try (var output = Files.newOutputStream(new File(test.getTargetContext().getCacheDir(),
                    "common-phrase-notes.png").toPath())) {
                if (!screenshot.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output))
                    throw new AssertionError("Synthetic note screenshot failed");
            } finally { screenshot.recycle(); }
        }
        test.tap(described(note ? "常用语备注：" + NOTE : "常用语 " + BODY.substring(0, 20)));
        test.await(test.field("msime-test-plain").and(node -> BODY.equals(value(node))));
    }

    private void backupRoundTrip(File directory) throws Exception {
        ClassLoader loader = test.getTargetContext().getClassLoader();
        Class<?> backup = loader.loadClass("app.msime.android.home.LocalBackup");
        Method export = backup.getDeclaredMethod("export", Context.class, android.net.Uri.class);
        export.setAccessible(true);
        File archive = Files.createTempFile(test.getTargetContext().getCacheDir().toPath(),
            "synthetic-phrase-notes-", ".zip").toFile();
        try {
            // 用缓存中的空文件代替 SAF 目标，走产品完整的 ZIP 导出与目标写入路径。
            Object outcome = export.invoke(null, test.getTargetContext(), android.net.Uri.fromFile(archive));
            if (!(outcome instanceof String message) || !message.startsWith("已导出备份："))
                throw new AssertionError("Synthetic phrase backup export failed");
            try (ZipFile zip = new ZipFile(archive)) {
                ZipEntry entry = zip.getEntry("phrases.json");
                if (entry == null || entry.getSize() < 0 || entry.getSize() > 2 * 1024 * 1024)
                    throw new AssertionError("Exported common phrase entry is absent or unbounded");
                byte[] bytes = new byte[(int) entry.getSize()];
                try (var input = new java.io.DataInputStream(zip.getInputStream(entry))) { input.readFully(bytes); }
                JSONObject snapshot = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
                JSONArray bodies = snapshot.getJSONArray("phrases");
                if (bodies.length() != 1 || !BODY.equals(bodies.getString(0))
                        || !CHANGED.equals(snapshot.getJSONObject("notes").getString(BODY)))
                    throw new AssertionError("Exported ZIP did not separate body and note");
            }
            Object removed = store().getMethod("remove", Context.class, String.class)
                .invoke(null, test.getTargetContext(), stored().getString("id"));
            requireOk(removed);
            Class<?> undoClass = loader.loadClass("app.msime.android.home.LocalBackup$Undo");
            Method capture = undoClass.getDeclaredMethod("capture", Context.class, String.class);
            capture.setAccessible(true);
            Object undo = capture.invoke(null, test.getTargetContext(), directory.getAbsolutePath());
            Method restore = backup.getDeclaredMethod("restorePhrases", Context.class, ZipFile.class, undoClass, List.class);
            restore.setAccessible(true);
            List<String> skipped = new ArrayList<>();
            try (ZipFile zip = new ZipFile(archive)) {
                if (!Integer.valueOf(1).equals(restore.invoke(null, test.getTargetContext(), zip, undo, skipped)) || !skipped.isEmpty())
                    throw new AssertionError("Backup note restore failed");
            }
            awaitStored(BODY, CHANGED);
        } finally { Files.deleteIfExists(archive.toPath()); }
    }

    private void set(String description, String value) {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value);
        if (!test.await(described(description)).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Synthetic note field refused text");
    }

    private void awaitStored(String body, String note) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            JSONObject phrase = stored();
            if (phrase != null && body.equals(phrase.optString("text")) && note.equals(phrase.optString("note", ""))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic phrase note did not persist");
    }

    private void awaitCount(int count) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (read(phrases).getJSONArray("phrases").length() == count) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic phrase removal did not persist");
    }

    private JSONObject stored() throws Exception {
        JSONArray items = read(phrases).getJSONArray("phrases");
        return items.length() == 1 ? items.getJSONObject(0) : null;
    }

    private static void awaitStoreIdle() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean pending = false;
            for (StackTraceElement[] trace : Thread.getAllStackTraces().values())
                for (StackTraceElement frame : trace)
                    if (frame.getClassName().equals("app.msime.android.CommonPhrasesStore")) pending = true;
            if (!pending) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Common phrase worker did not settle");
    }

    private Class<?> store() throws Exception {
        return test.getTargetContext().getClassLoader().loadClass("app.msime.android.CommonPhrasesStore");
    }

    private static void requireOk(Object result) throws Exception {
        if (!Boolean.TRUE.equals(result.getClass().getMethod("ok").invoke(result)))
            throw new AssertionError("Synthetic phrase storage operation failed");
    }

    private static String value(AccessibilityNodeInfo node) {
        return node.isShowingHintText() || node.getText() == null ? "" : node.getText().toString();
    }

    private static Predicate<AccessibilityNodeInfo> text(String value) {
        return node -> "app.msime.android".contentEquals(node.getPackageName() == null ? "" : node.getPackageName())
            && value.contentEquals(node.getText() == null ? "" : node.getText());
    }

    private static Predicate<AccessibilityNodeInfo> described(String value) {
        return node -> "app.msime.android".contentEquals(node.getPackageName() == null ? "" : node.getPackageName())
            && value.contentEquals(node.getContentDescription() == null ? "" : node.getContentDescription());
    }

    private void write(File file, String text) throws Exception { test.publish(file, text.getBytes(StandardCharsets.UTF_8)); }
    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
}
