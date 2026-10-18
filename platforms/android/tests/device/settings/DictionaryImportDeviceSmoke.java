package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.SharedPreferences;
import android.database.sqlite.SQLiteDatabase;
import android.os.Bundle;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Comparator;
import org.json.JSONArray;
import org.json.JSONObject;

/** 从安装产品的导入入口写合成词库，再经过 JNI 验证九键候选与繁忙重试。 */
public final class DictionaryImportDeviceSmoke extends Instrumentation {
    private Class<?> nativeClient;
    private String stage = "prepare synthetic import";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        File root = null;
        long session = 0;
        try {
            ClassLoader loader = getTargetContext().getClassLoader();
            nativeClient = loader.loadClass("app.msime.android.NativeClient");
            Class<?> store = loader.loadClass("app.msime.android.DictionaryCollectionsStore");
            root = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "synthetic-import-").toFile();
            for (String name : new String[] {"resources", "user", "cache", "dictionaries", "preferences"}) {
                check(new File(root, name).mkdir(), "synthetic directory");
            }
            for (String name : new String[] {"resources", "dictionaries"}) {
                try (SQLiteDatabase db = SQLiteDatabase.openOrCreateDatabase(new File(root, name + "/msime-pinyin.db"), null)) {
                    db.execSQL("CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER)");
                    db.execSQL("CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER)");
                }
            }
            JSONObject preferences = value("loadPreferences", new Class<?>[] {String.class},
                new File(root, "preferences").getPath()).getJSONObject("preferences");
            preferences.put("learning", false);
            JSONObject options = new JSONObject().put("api_version", 1).put("preferences", preferences);
            for (String[] entry : new String[][] {{"resources", "resources"}, {"user_data", "user"},
                    {"cache", "cache"}, {"dictionaries", "dictionaries"}, {"preferences_directory", "preferences"}}) {
                options.put(entry[0], new File(root, entry[1]).getPath());
            }
            String optionsText = options.toString();
            Files.write(new File(root, "runtime-options.json").toPath(), optionsText.getBytes(StandardCharsets.UTF_8));
            File files = root;
            Context context = new ContextWrapper(getTargetContext()) {
                @Override public File getFilesDir() { return files; }
                @Override public Context getApplicationContext() { return this; }
                @Override public SharedPreferences getSharedPreferences(String name, int mode) {
                    return super.getSharedPreferences("synthetic-dictionary-import-" + name, mode);
                }
            };
            Method importFile = store.getMethod("importFile", Context.class, String.class, String.class, byte[].class);
            stage = "import seventy-four synthetic words through installed store";
            Object report = importReport(importFile, context, "合成导入", rows(0, 74));
            check((int) report.getClass().getMethod("imported").invoke(report) == 74, "import count");
            String notice = (String) report.getClass().getMethod("applicationNotice").invoke(report);
            if (!notice.isEmpty()) {
                // 产品只同步两秒；慢设备允许明确提示等待，再验证队列最终完整应用，不能把预算当吞词。
                check(notice.contains("等待"), "small import failed instead of queued");
                long deadline = android.os.SystemClock.uptimeMillis() + 30000;
                JSONObject sync;
                do {
                    sync = value("personalDictionarySync", new Class<?>[] {String.class}, optionsText);
                    check(sync.getInt("failed_count") == 0, "small import retry failed");
                    check(android.os.SystemClock.uptimeMillis() < deadline, "small import retry timed out");
                } while (sync.getInt("pending_count") > 0);
            }
            String target = word(5);
            try (SQLiteDatabase db = SQLiteDatabase.openDatabase(new File(root, "dictionaries/msime-pinyin.db").getPath(), null, SQLiteDatabase.OPEN_READONLY);
                    android.database.Cursor cursor = db.rawQuery("SELECT count(*) FROM tbl_2_n", null)) {
                check(cursor.moveToFirst() && cursor.getInt(0) == 74, "import not fully applied");
            }
            stage = "read ninth-key candidates from installed native host";
            session = value("create", new Class<?>[] {String.class}, optionsText).getLong("session");
            value("focus", new Class<?>[] {long.class, boolean.class}, session, true);
            value("setNineKeyMode", new Class<?>[] {long.class, boolean.class}, session, true);
            JSONObject typed = null;
            for (char key : "64426".toCharArray()) {
                typed = value("character", new Class<?>[] {long.class, int.class, boolean.class}, session, (int) key, false);
            }
            JSONArray candidates = typed.getJSONObject("view").getJSONArray("candidates");
            boolean found = false;
            for (int index = 0; index < candidates.length(); index++) {
                found |= target.equals(candidates.getJSONObject(index).getString("text"));
            }
            check(found, "sixth imported word absent from nine-key candidates");
            stage = "busy import remains queued instead of failed";
            report = importReport(importFile, context, "繁忙合成导入", rows(100, 9));
            check(((String) report.getClass().getMethod("applicationNotice").invoke(report)).contains("等待"), "busy notice missing");
            stage = "read synthetic pending queue";
            JSONArray queued = new JSONObject(new String(Files.readAllBytes(new File(root, "preferences/PersonalDictionary/sync.json").toPath()), StandardCharsets.UTF_8)).getJSONArray("requests");
            int pending = 0;
            for (int index = 0; index < queued.length(); index++) {
                String status = queued.getJSONObject(index).getString("status");
                check(!"failed".equals(status), "busy import permanently failed");
                if ("pending".equals(status)) pending++;
            }
            check(pending == 9, "busy import queue changed");
            destroy(session);
            session = 0;
            stage = "retry busy import after session closes";
            JSONObject synchronizedWords;
            do {
                synchronizedWords = value("personalDictionarySync", new Class<?>[] {String.class}, optionsText);
                check(synchronizedWords.getInt("failed_count") == 0, "retry failed");
            } while (synchronizedWords.getInt("pending_count") > 0);
            result.putString("result", "MSIME_DICTIONARY_IMPORT_PASSED: 74 words applied, sixth word in nine-key candidates, busy queue preserved and retried");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | LinkageError | AssertionError error) {
            Throwable cause = error instanceof java.lang.reflect.InvocationTargetException
                ? ((java.lang.reflect.InvocationTargetException) error).getTargetException() : error;
            String detail = cause instanceof AssertionError || cause instanceof org.json.JSONException
                ? ": " + cause.getMessage() : "";
            result.putString("result", "MSIME_DICTIONARY_IMPORT_FAILED at " + stage + ": "
                + cause.getClass().getSimpleName() + detail);
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            if (session != 0) {
                try { destroy(session); }
                catch (Exception ignored) { }
            }
            if (root != null) {
                try (var paths = Files.walk(root.toPath())) {
                    for (var path : paths.sorted(Comparator.reverseOrder()).toArray(java.nio.file.Path[]::new)) Files.deleteIfExists(path);
                } catch (Exception ignored) { }
            }
        }
    }

    private JSONObject value(String name, Class<?>[] types, Object... args) throws Exception {
        JSONObject response = new JSONObject((String) nativeClient.getMethod(name, types).invoke(null, args));
        check(response.optBoolean("ok"), "native operation rejected: " + name);
        return response.getJSONObject("value");
    }

    private void destroy(long session) throws Exception {
        JSONObject response = new JSONObject((String) nativeClient.getMethod("destroy", long.class).invoke(null, session));
        check(response.optBoolean("ok"), "native operation rejected: destroy");
    }

    private static Object importReport(Method method, Context context, String name, String text) throws Exception {
        Object imported = method.invoke(null, context, "standard", name, text.getBytes(StandardCharsets.UTF_8));
        check((boolean) imported.getClass().getMethod("ok").invoke(imported), "installed import rejected");
        Object view = imported.getClass().getMethod("value").invoke(imported);
        return view.getClass().getMethod("importReport").invoke(view);
    }

    private static String word(int index) { return "" + (char) (0x4e00 + index) + "蒿"; }

    private static String rows(int start, int count) {
        StringBuilder text = new StringBuilder();
        for (int index = start; index < start + count; index++) {
            text.append(word(index)).append("\tni'hao\t").append(index == 5 ? 20_000 : 10_000).append('\n');
        }
        return text.toString();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
