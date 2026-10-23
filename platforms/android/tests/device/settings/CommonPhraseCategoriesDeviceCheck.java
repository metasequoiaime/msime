package app.msime.android.test;

import android.content.Context;
import android.graphics.Rect;
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
import java.util.zip.ZipFile;
import org.json.JSONArray;
import org.json.JSONObject;

/** 合成数据验收原生分类管理、纵向展开、跨进程插入和实际 ZIP 备份。 */
final class CommonPhraseCategoriesDeviceCheck {
    private static final String BODY = "Synthetic category alpha\nSynthetic category beta";
    private static final String OTHER = "Synthetic unclassified phrase";
    private static final String NOTE = "合成分类用途";
    private static final String GROUP = "合成工作";
    private static final String RENAMED = "合成工作已改";
    private static final String EMPTY = "合成空类";
    private final DeviceSmoke test;
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private File phrases;

    CommonPhraseCategoriesDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        File root = test.getTargetContext().getFilesDir();
        File directory = new File(read(new File(root, "runtime-options.json")).getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Synthetic sandbox escaped");
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
            write(phrases, new JSONObject().put("phrases", new JSONArray().put(new JSONObject()
                .put("id", "00000000-0000-4000-8000-000000000001").put("text", BODY).put("note", NOTE).put("pack", JSONObject.NULL))
                .put(new JSONObject().put("id", "00000000-0000-4000-8000-000000000002").put("text", OTHER).put("pack", JSONObject.NULL)))
                .put("packs", new JSONArray()).toString());
            write(seeded, "");
            test.restartIme();
            test.stage = "native common phrase category creation";
            settings();
            test.tap(text("新建"));
            set("常用语分类名称", GROUP);
            test.tap(text("保存").and(AccessibilityNodeInfo::isEnabled));
            awaitCategory(GROUP);
            test.stage = "native category rename keeps its identity";
            String id = category(GROUP).getString("id");
            test.tap(described("管理常用语分类 " + GROUP));
            test.tap(text("修改名称"));
            set("常用语分类名称", RENAMED);
            test.tap(text("保存").and(AccessibilityNodeInfo::isEnabled));
            awaitCategory(RENAMED);
            if (!id.equals(category(RENAMED).getString("id"))) throw new AssertionError("Rename changed category identity");
            test.tap(text("新建"));
            set("常用语分类名称", EMPTY);
            test.tap(text("保存").and(AccessibilityNodeInfo::isEnabled));
            awaitCategory(EMPTY);
            test.stage = "native phrase classification and default selection";
            test.tap(described("设置常用语分类 " + BODY));
            test.tap(described(RENAMED).and(AccessibilityNodeInfo::isClickable));
            awaitAssigned(id);
            setDefaultUi(RENAMED);
            test.stage = "keyboard shows vertically stacked categories and body-only insertion";
            keyboard();
            test.await(described("常用语分类 " + RENAMED + "，已展开"));
            test.await(described("常用语备注：" + NOTE));
            Rect all = bounds(test.await(described("常用语分类 全部，已收起")));
            Rect group = bounds(test.await(described("常用语分类 " + RENAMED + "，已展开")));
            if (group.top < all.bottom || Math.abs(all.left - group.left) > 2)
                throw new AssertionError("Categories are not vertically stacked");
            test.shellDiscard("screencap -p /data/local/tmp/msime-8642-categories.png");
            test.tap(described("常用语分类 " + EMPTY + "，已收起"));
            test.await(text("该分类还没有常用语"));
            assertNoPhrase();
            test.tap(described("常用语分类 全部，已收起"));
            test.await(described("常用语 " + OTHER.substring(0, 20)));
            test.tap(described("常用语分类 全部，已展开"));
            assertNoPhrase();
            keyboard();
            test.await(described("常用语分类 " + RENAMED + "，已展开"));
            test.restartIme();
            keyboard();
            test.await(described("常用语分类 " + RENAMED + "，已展开"));
            test.tap(described("常用语备注：" + NOTE));
            test.await(test.field("msime-test-plain").and(node -> BODY.equals(value(node))));
            test.stage = "collapsed default survives reopening";
            settings();
            setDefaultUi("不展开");
            keyboard();
            test.await(described("常用语分类 全部，已收起"));
            test.await(described("常用语分类 " + RENAMED + "，已收起"));
            assertNoPhrase();
            requireOk(call("setExpandedCategory", new Class<?>[] {String.class}, id));
            test.stage = "category ZIP backup restores names memberships and default";
            backupRoundTrip(directory);
            test.stage = "native category deletion retains body note and restores all default";
            settings();
            test.tap(described("管理常用语分类 " + RENAMED));
            test.tap(text("删除分类"));
            waitUntil(() -> category(RENAMED) == null && phrase(BODY).optString("category", "").isEmpty()
                && read(phrases).optString("expanded_category", "all").equals("all"));
            if (!NOTE.equals(phrase(BODY).optString("note"))) throw new AssertionError("Removing category lost note");
            keyboard();
            test.await(described("常用语分类 全部，已展开"));
            test.tap(described("常用语 " + BODY.substring(0, 20)));
            test.await(test.field("msime-test-plain").and(node -> BODY.equals(value(node))));
        } finally {
            test.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/.EditorActivity");
            test.waitForIdleSync();
            waitUntil(() -> {
                for (StackTraceElement[] trace : Thread.getAllStackTraces().values())
                    for (StackTraceElement frame : trace)
                        if (frame.getClassName().equals("app.msime.android.CommonPhrasesStore")) return false;
                return true;
            });
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else test.publish(entry.getKey(), entry.getValue());
            }
            test.restartIme();
        }
    }

    private void backupRoundTrip(File directory) throws Exception {
        Class<?> backup = test.getTargetContext().getClassLoader().loadClass("app.msime.android.home.LocalBackup");
        Method export = backup.getDeclaredMethod("export", Context.class, android.net.Uri.class);
        export.setAccessible(true);
        File archive = Files.createTempFile(test.getTargetContext().getCacheDir().toPath(), "synthetic-categories-", ".zip").toFile();
        try {
            Object outcome = export.invoke(null, test.getTargetContext(), android.net.Uri.fromFile(archive));
            if (!(outcome instanceof String message) || !message.startsWith("已导出备份：")) throw new AssertionError("Category backup export failed");
            try (ZipFile zip = new ZipFile(archive)) {
                var entry = zip.getEntry("phrases.json");
                if (entry == null || entry.getSize() < 0 || entry.getSize() > 2 * 1024 * 1024) throw new AssertionError("Category backup missing");
                byte[] bytes = new byte[(int) entry.getSize()];
                try (var input = new java.io.DataInputStream(zip.getInputStream(entry))) { input.readFully(bytes); }
                JSONObject organization = new JSONObject(new String(bytes, StandardCharsets.UTF_8)).getJSONObject("organization");
                JSONArray categories = organization.getJSONArray("categories");
                if (categories.length() != 2 || !category(RENAMED).getString("id").equals(organization.getString("expanded_category"))
                    || !BODY.equals(categories.getJSONObject(0).getJSONArray("texts").getString(0)))
                    throw new AssertionError("Category backup content mismatch");
            }
            for (String name : new String[] {RENAMED, EMPTY}) requireOk(call("removeCategory", new Class<?>[] {String.class}, category(name).getString("id")));
            for (String body : new String[] {BODY, OTHER}) requireOk(call("remove", new Class<?>[] {String.class}, phrase(body).getString("id")));
            restore(backup, archive, directory, 2);
            String restored = category(RENAMED).getString("id");
            if (!restored.equals(phrase(BODY).getString("category")) || !restored.equals(read(phrases).getString("expanded_category")))
                throw new AssertionError("Category membership or default did not restore");
            requireOk(call("categorize", new Class<?>[] {String.class, String.class}, phrase(OTHER).getString("id"), category(EMPTY).getString("id")));
            requireOk(call("setExpandedCategory", new Class<?>[] {String.class}, ""));
            restore(backup, archive, directory, 0);
            if (!category(EMPTY).getString("id").equals(phrase(OTHER).getString("category"))
                || !read(phrases).getString("expanded_category").isEmpty()) throw new AssertionError("Backup overwrote local category choice");
            requireOk(call("setExpandedCategory", new Class<?>[] {String.class}, restored));
            test.stage = "full capacity categorized phrase backup passes actual restore verification";
            verifyFullCapacityBackup(backup, export, restored);
        } finally { Files.deleteIfExists(archive.toPath()); }
    }

    private void verifyFullCapacityBackup(Class<?> backup, Method export, String category) throws Exception {
        byte[] original = Files.readAllBytes(phrases.toPath());
        File archive = Files.createTempFile(test.getTargetContext().getCacheDir().toPath(), "synthetic-full-categories-", ".zip").toFile();
        Object prepared = null;
        try {
            JSONObject document = read(phrases);
            JSONArray entries = new JSONArray();
            for (int index = 0; index < 200; index++) {
                String body = String.format(java.util.Locale.ROOT, "%04d", index) + "合".repeat(996);
                entries.put(new JSONObject().put("id", java.util.UUID.randomUUID().toString()).put("text", body)
                    .put("note", "成".repeat(1000)).put("pack", JSONObject.NULL).put("category", category));
            }
            write(phrases, document.put("phrases", entries).toString());
            Object outcome = export.invoke(null, test.getTargetContext(), android.net.Uri.fromFile(archive));
            if (!(outcome instanceof String message) || !message.startsWith("已导出备份："))
                throw new AssertionError("Full capacity category export failed");
            try (ZipFile zip = new ZipFile(archive)) {
                long size = zip.getEntry("phrases.json").getSize();
                if (size <= 2 * 1024 * 1024 || size > 3 * 1024 * 1024)
                    throw new AssertionError("Full capacity backup did not exercise its expanded read limit");
            }
            Method prepare = backup.getDeclaredMethod("prepare", Context.class, android.net.Uri.class);
            prepare.setAccessible(true);
            prepared = prepare.invoke(null, test.getTargetContext(), android.net.Uri.fromFile(archive));
            if (prepared == null) throw new AssertionError("Full capacity backup could not be prepared");
            Method preview = prepared.getClass().getDeclaredMethod("preview");
            preview.setAccessible(true);
            Object details = preview.invoke(prepared);
            Method compatibility = details.getClass().getDeclaredMethod("compatibility");
            compatibility.setAccessible(true);
            if (!"OK".equals(compatibility.invoke(details).toString()))
                throw new AssertionError("Exported full capacity category backup was rejected");
        } finally {
            try {
                if (prepared != null) {
                    Method archivePath = prepared.getClass().getDeclaredMethod("archive");
                    archivePath.setAccessible(true);
                    Files.deleteIfExists((java.nio.file.Path) archivePath.invoke(prepared));
                }
                Files.deleteIfExists(archive.toPath());
            } finally { test.publish(phrases, original); }
        }
    }

    private void restore(Class<?> backup, File archive, File directory, int expected) throws Exception {
        Class<?> undoClass = test.getTargetContext().getClassLoader().loadClass("app.msime.android.home.LocalBackup$Undo");
        Method capture = undoClass.getDeclaredMethod("capture", Context.class, String.class);
        capture.setAccessible(true);
        Object undo = capture.invoke(null, test.getTargetContext(), directory.getAbsolutePath());
        Method restore = backup.getDeclaredMethod("restorePhrases", Context.class, ZipFile.class, undoClass, List.class);
        restore.setAccessible(true);
        List<String> skipped = new ArrayList<>();
        try (ZipFile zip = new ZipFile(archive)) {
            if (!Integer.valueOf(expected).equals(restore.invoke(null, test.getTargetContext(), zip, undo, skipped)) || !skipped.isEmpty())
                throw new AssertionError("Category backup restore failed");
        }
    }

    private Object call(String method, Class<?>[] parameterTypes, String... arguments) throws Exception {
        Class<?> store = test.getTargetContext().getClassLoader().loadClass("app.msime.android.CommonPhrasesStore");
        Class<?>[] types = new Class<?>[parameterTypes.length + 1];
        types[0] = Context.class;
        System.arraycopy(parameterTypes, 0, types, 1, parameterTypes.length);
        Object[] values = new Object[arguments.length + 1];
        values[0] = test.getTargetContext();
        System.arraycopy(arguments, 0, values, 1, arguments.length);
        return store.getMethod(method, types).invoke(null, values);
    }

    private void keyboard() throws Exception {
        test.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/.EditorActivity --ez msime_test_multiline true");
        test.tap(test.field("msime-test-plain"));
        test.await(test.key("n"));
        test.tap(described("常用语"));
        test.await(described("管理常用语"));
    }

    private void assertNoPhrase() {
        test.dropStaleNodes();
        for (var window : test.automation.getWindows())
            if (test.find(window.getRoot(), described("常用语 " + BODY.substring(0, 20))) != null
                || test.find(window.getRoot(), described("常用语 " + OTHER.substring(0, 20))) != null)
                throw new AssertionError("Collapsed category still exposes phrase");
    }

    private void settings() throws Exception {
        test.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page PHRASES");
        test.await(text("新建"));
    }

    private void setDefaultUi(String name) throws Exception {
        test.tap(text("默认展开"));
        test.tap(described(name).and(AccessibilityNodeInfo::isClickable));
        String expected = name.equals("不展开") ? "" : name.equals("全部") ? "all" : category(name).getString("id");
        waitUntil(() -> read(phrases).optString("expanded_category", "all").equals(expected));
    }

    private void awaitCategory(String name) throws Exception { waitUntil(() -> category(name) != null); }
    private void awaitAssigned(String id) throws Exception { waitUntil(() -> id.equals(phrase(BODY).optString("category", ""))); }
    private JSONObject category(String name) throws Exception {
        JSONArray categories = read(phrases).optJSONArray("categories");
        if (categories != null) for (int i = 0; i < categories.length(); i++)
            if (name.equals(categories.getJSONObject(i).getString("name"))) return categories.getJSONObject(i);
        return null;
    }
    private JSONObject phrase(String body) throws Exception {
        JSONArray items = read(phrases).getJSONArray("phrases");
        for (int i = 0; i < items.length(); i++) if (body.equals(items.getJSONObject(i).getString("text"))) return items.getJSONObject(i);
        throw new AssertionError("Synthetic phrase missing");
    }
    private interface Condition { boolean ready() throws Exception; }
    private static void waitUntil(Condition condition) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do { if (condition.ready()) return; SystemClock.sleep(100); } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic category state did not settle");
    }
    private void set(String description, String value) {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value);
        if (!test.await(described(description)).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) throw new AssertionError("Category name input failed");
    }
    private static void requireOk(Object result) throws Exception {
        if (!Boolean.TRUE.equals(result.getClass().getMethod("ok").invoke(result))) throw new AssertionError("Category storage operation failed");
    }
    private static String value(AccessibilityNodeInfo node) { return node.getText() == null ? "" : node.getText().toString(); }
    private static Predicate<AccessibilityNodeInfo> text(String value) { return node -> value.equals(value(node)); }
    private static Predicate<AccessibilityNodeInfo> described(String value) {
        return node -> value.contentEquals(node.getContentDescription() == null ? "" : node.getContentDescription());
    }
    private static Rect bounds(AccessibilityNodeInfo node) { Rect rect = new Rect(); node.getBoundsInScreen(rect); return rect; }
    private void write(File file, String value) throws Exception { test.publish(file, value.getBytes(StandardCharsets.UTF_8)); }
    private static JSONObject read(File file) throws Exception { return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8)); }
}
