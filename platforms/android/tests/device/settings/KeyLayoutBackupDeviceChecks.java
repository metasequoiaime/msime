package app.msime.android.test;

import android.content.Context;
import android.content.ContextWrapper;
import android.content.SharedPreferences;
import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Comparator;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;
import org.json.JSONObject;

/** 在应用 UID 内调用已安装产品的恢复与撤销代码；所有文件和设置均为隔离的合成夹具。 */
final class KeyLayoutBackupDeviceChecks {
    private static final String LOCAL = "app.msime.android.AndroidLocalSettings";
    private static final String BACKUP = "app.msime.android.home.LocalBackup";
    private static final String NATIVE = "app.msime.android.NativeClient";
    private static final String LONG = "platform.android.long_space_layout";
    private static final String NUMBER = "platform.android.number_row";
    private static final String NINE = "platform.android.nine_key_swipe";
    private static final String NINE_SHARED = "touch_nine_key_swipe_digits";

    private KeyLayoutBackupDeviceChecks() {}

    static void run(Context target) throws Exception {
        File root = Files.createTempDirectory(target.getCacheDir().toPath(), "synthetic-layout-backup-").toFile();
        Context isolated = new ContextWrapper(target) {
            @Override public Context getApplicationContext() { return this; }
            @Override public File getFilesDir() { return root; }
            @Override public File getCacheDir() { return root; }
            @Override public File getNoBackupFilesDir() { return root; }
            @Override public SharedPreferences getSharedPreferences(String name, int mode) {
                return super.getSharedPreferences("synthetic-layout-backup-" + name, mode);
            }
        };
        try {
            String directory = new File(root, "preferences").getAbsolutePath();
            JSONObject snapshot = value(call(target, NATIVE, null, "loadPreferences",
                new Class<?>[] {String.class}, directory));
            snapshot.put("format_version", 1);
            value(call(target, NATIVE, null, "savePreferences",
                new Class<?>[] {String.class, long.class, String.class},
                directory, snapshot.getLong("revision"), snapshot.toString()));
            call(target, LOCAL, null, "update", new Class<?>[] {Context.class, Map.class},
                isolated, Map.of(LONG, true, NUMBER, true));

            // 从真实 JSON 经 JNI 迁移：未知样式不能被标准化快照抹掉，数字行仍可独立迁移。
            File document = new File(directory, "preferences.json");
            JSONObject raw = new JSONObject(new String(Files.readAllBytes(document.toPath()), StandardCharsets.UTF_8));
            raw.getJSONObject("preferences").put("touch_twenty_six_key_style", "split_space");
            Files.write(document.toPath(), raw.toString().getBytes(StandardCharsets.UTF_8));
            check(Boolean.TRUE.equals(call(target, "app.msime.android.KeyLayoutStyleMigration", null, "migrate",
                new Class<?>[] {Context.class, String.class}, isolated, directory)), "unknown style migration succeeds");
            JSONObject migrated = new JSONObject(new String(Files.readAllBytes(document.toPath()), StandardCharsets.UTF_8)).getJSONObject("preferences");
            check("split_space".equals(migrated.getString("touch_twenty_six_key_style")), "unknown style stays on disk");
            check(migrated.getBoolean("touch_number_row"), "number row migrates beside an unknown style");
            raw = new JSONObject(new String(Files.readAllBytes(document.toPath()), StandardCharsets.UTF_8));
            raw.getJSONObject("preferences").remove("touch_twenty_six_key_style");
            raw.getJSONObject("preferences").remove("touch_number_row");
            Files.write(document.toPath(), raw.toString().getBytes(StandardCharsets.UTF_8));
            call(target, LOCAL, null, "update", new Class<?>[] {Context.class, Map.class},
                isolated, Map.of(LONG, true, NUMBER, true));

            // 撤销快照先迁移本机旧选择，不能把两个旧键删掉却遗漏共享值。
            Object undo = call(target, BACKUP + "$Undo", null, "capture",
                new Class<?>[] {Context.class, String.class}, isolated, directory);
            assertLayout(target, isolated, directory, "long_space", true);

            // 旧备份的显式 false 必须覆盖当前的 true，缺少 settings.json 也要恢复。
            restore(target, isolated, directory, root, new JSONObject().put(LONG, false).put(NUMBER, false), null);
            assertLayout(target, isolated, directory, "standard", false);
            check(Boolean.TRUE.equals(call(target, BACKUP + "$Undo", undo, "apply",
                new Class<?>[] {Context.class, String.class}, isolated, directory)), "undo succeeds");
            assertLayout(target, isolated, directory, "long_space", true);

            // 新字段优先，不能被同一备份里残留的旧 true 翻回长空格或数字行。
            restore(target, isolated, directory, root, new JSONObject().put(LONG, true).put(NUMBER, true),
                new JSONObject().put("platform.android.twenty_six_key_style", "emoji").put(NUMBER, false));
            assertLayout(target, isolated, directory, "emoji", false);

            // 九键滑动输入数字（#10229）：本机旧方向经 JNI 迁进共享偏好，迁完删掉旧键。
            call(target, LOCAL, null, "update", new Class<?>[] {Context.class, Map.class}, isolated, Map.of(NINE, "down"));
            migrateNineKey(target, isolated, directory);
            check("down".equals(nineKeySwipe(target, directory)), "the local nine-key direction migrates");
            // 别的设备同步来的方向不被本机旧值盖掉，较新版本的写法原样留在文件里。
            for (String kept : new String[] {"up", "sideways"}) {
                raw = new JSONObject(new String(Files.readAllBytes(document.toPath()), StandardCharsets.UTF_8));
                raw.getJSONObject("preferences").put(NINE_SHARED, kept);
                Files.write(document.toPath(), raw.toString().getBytes(StandardCharsets.UTF_8));
                call(target, LOCAL, null, "update", new Class<?>[] {Context.class, Map.class}, isolated, Map.of(NINE, "down"));
                migrateNineKey(target, isolated, directory);
                JSONObject onDisk = new JSONObject(new String(Files.readAllBytes(document.toPath()), StandardCharsets.UTF_8));
                check(kept.equals(onDisk.getJSONObject("preferences").optString(NINE_SHARED)),
                    "a shared nine-key value " + kept + " stays");
            }
            // 旧备份里明确选过的关闭也恢复成关闭；同一备份带着新字段时以新字段为准。
            restore(target, isolated, directory, root, new JSONObject().put(NINE, "off"), null);
            check("off".equals(nineKeySwipe(target, directory)), "an old backup restores nine-key swipe off");
            restore(target, isolated, directory, root, new JSONObject().put(NINE, "down"),
                new JSONObject().put(NINE, "up"));
            check("up".equals(nineKeySwipe(target, directory)), "the shared field wins over the legacy key");
            check(!localHas(target, isolated, NINE), "the legacy nine-key key is removed after restore");
        } finally {
            try (java.util.stream.Stream<java.nio.file.Path> files = Files.walk(root.toPath())) {
                for (java.nio.file.Path file : (Iterable<java.nio.file.Path>) files.sorted(Comparator.reverseOrder())::iterator)
                    Files.delete(file);
            }
        }
    }

    private static void restore(Context target, Context isolated, String directory, File root,
            JSONObject local, JSONObject shared) throws Exception {
        File archive = new File(root, "synthetic.zip");
        try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(archive.toPath()))) {
            entry(zip, "android-local.json", local);
            if (shared != null) entry(zip, "settings.json", new JSONObject().put("settings", shared));
        }
        try (ZipFile zip = new ZipFile(archive)) {
            check(Boolean.TRUE.equals(call(target, BACKUP, null, "restoreSettings",
                new Class<?>[] {Context.class, String.class, ZipFile.class}, isolated, directory, zip)),
                "backup settings are applied");
        }
    }

    private static void entry(ZipOutputStream zip, String name, JSONObject value) throws Exception {
        zip.putNextEntry(new ZipEntry(name));
        zip.write(value.toString().getBytes(StandardCharsets.UTF_8));
        zip.closeEntry();
    }

    private static void assertLayout(Context target, Context isolated, String directory, String style,
            boolean number) throws Exception {
        JSONObject preferences = value(call(target, NATIVE, null, "loadPreferences",
            new Class<?>[] {String.class}, directory)).getJSONObject("preferences");
        check(style.equals(preferences.optString("touch_twenty_six_key_style", "standard")), "shared row style");
        check(number == preferences.optBoolean("touch_number_row", false), "shared number row");
        Object local = call(target, LOCAL, null, "load", new Class<?>[] {Context.class}, isolated);
        Method has = local.getClass().getMethod("has", String.class);
        check(!Boolean.TRUE.equals(has.invoke(local, LONG)) && !Boolean.TRUE.equals(has.invoke(local, NUMBER)),
            "legacy keys are removed after migration or restore");
    }

    private static void migrateNineKey(Context target, Context isolated, String directory) throws Exception {
        check(Boolean.TRUE.equals(call(target, "app.msime.android.KeyLayoutStyleMigration", null, "migrate",
            new Class<?>[] {Context.class, String.class}, isolated, directory)), "nine-key swipe migration succeeds");
        check(!localHas(target, isolated, NINE), "the legacy nine-key key is removed after migration");
    }

    /** 共享偏好里的九键滑动，缺键读成关闭（默认值不写进文档）。 */
    private static String nineKeySwipe(Context target, String directory) throws Exception {
        return value(call(target, NATIVE, null, "loadPreferences", new Class<?>[] {String.class}, directory))
            .getJSONObject("preferences").optString(NINE_SHARED, "off");
    }

    private static boolean localHas(Context target, Context isolated, String key) throws Exception {
        Object local = call(target, LOCAL, null, "load", new Class<?>[] {Context.class}, isolated);
        return Boolean.TRUE.equals(local.getClass().getMethod("has", String.class).invoke(local, key));
    }

    private static Object call(Context target, String type, Object receiver, String name,
            Class<?>[] types, Object... arguments) throws Exception {
        Method method = Class.forName(type, true, target.getClassLoader()).getDeclaredMethod(name, types);
        method.setAccessible(true);
        try {
            return method.invoke(receiver, arguments);
        } catch (InvocationTargetException error) {
            if (error.getCause() instanceof Exception) throw (Exception) error.getCause();
            if (error.getCause() instanceof Error) throw (Error) error.getCause();
            throw error;
        }
    }

    private static JSONObject value(Object response) throws Exception {
        JSONObject result = new JSONObject((String) response);
        check(result.optBoolean("ok"), "native call succeeds: " + result.opt("error"));
        return result.getJSONObject("value");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
