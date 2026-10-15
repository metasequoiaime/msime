package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import org.json.JSONObject;

/** 在应用沙箱里经真实 JNI 导入十万条码表，并用末行的辅助码过滤候选，不改产品偏好。 */
public final class HelpcodeImportDeviceSmoke extends Instrumentation {
    private String stage = "prepare synthetic table";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Path root = null;
        Class<?> nativeType = null;
        long session = 0;
        int code = Activity.RESULT_CANCELED;
        try {
            root = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "helpcode-import-");
            nativeType = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
            Method plugins = nativeType.getMethod("plugins", String.class);
            StringBuilder table = new StringBuilder(800_000);
            for (int index = 0; index < 99_999; index++) table.appendCodePoint(0x20000 + index).append("=ab\n");
            table.append("你=zz\n");
            JSONObject action = new JSONObject().put("operation", "import_helpcode_table")
                .put("file_name", "synthetic-large.txt").put("text", table.toString());
            JSONObject request = new JSONObject().put("state_root", root.resolve("state").toString()).put("action", action);
            stage = "import one hundred thousand entries";
            JSONObject imported = value(plugins.invoke(null, request.toString()));
            check(imported.getInt("entries") == 100_000, "all entries imported");
            String id = imported.getString("id");
            JSONObject loaded = value(plugins.invoke(null, request.put("action", new JSONObject()
                .put("operation", "catalog")).toString()));
            check(loaded.getJSONArray("packages").length() == 1, "one installed table");
            JSONObject pack = loaded.getJSONArray("packages").getJSONObject(0);
            check(pack.getString("id").equals(id) && pack.getInt("entries") == 100_000,
                "installed table reloads without truncation");

            // 只复用产品已校验的只读词库；偏好、用户词库和缓存全部指向本次合成目录。
            JSONObject installed = new JSONObject(new String(Files.readAllBytes(new File(
                getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
            JSONObject preferences = value(nativeType.getMethod("defaultPreferences").invoke(null));
            preferences.put("scheme", "quanpin").put("learning", false).put("cloud_candidates", false)
                .put("quanpin_helpcode", new JSONObject().put("enabled", true).put("schema", "ziranma"))
                .put("plugins", new JSONObject().put("helpcode_pack_quanpin", id));
            JSONObject options = new JSONObject().put("api_version", 1).put("resources", installed.getString("resources"))
                .put("dictionaries", installed.getString("dictionaries"))
                .put("user_data", Files.createDirectories(root.resolve("user")).toString())
                .put("cache", Files.createDirectories(root.resolve("cache")).toString())
                .put("preferences_directory", root.resolve("state").toString()).put("preferences", preferences);
            stage = "load table into Engine";
            session = value(nativeType.getMethod("create", String.class).invoke(null, options.toString())).getLong("session");
            value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
            Method character = nativeType.getMethod("character", long.class, int.class, boolean.class);
            value(character.invoke(null, session, (int) 'n', false));
            value(character.invoke(null, session, (int) 'i', false));
            JSONObject view = value(character.invoke(null, session, (int) 'Z', true)).getJSONObject("view");
            check(view.getJSONArray("candidates").length() > 0, "tail entry narrows candidates");
            check(view.getJSONArray("candidates").getJSONObject(0).getString("text").equals("你"), "tail entry reaches Engine");
            value(nativeType.getMethod("destroy", long.class).invoke(null, session));
            session = 0;
            stage = "remove imported table";
            value(plugins.invoke(null, request.put("action", new JSONObject().put("operation", "remove")
                .put("kind", "helpcode").put("id", id)).toString()));
            JSONObject catalog = value(plugins.invoke(null, request.put("action", new JSONObject()
                .put("operation", "catalog")).toString()));
            check(catalog.getJSONArray("packages").length() == 0, "import removed");
            code = Activity.RESULT_OK;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: 100000 helpcodes imported, reloaded, narrowed by the tail entry and removed through installed JNI\n");
        } catch (Exception | AssertionError | LinkageError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + error + ")\n");
        } finally {
            try {
                if (session != 0 && nativeType != null) nativeType.getMethod("destroy", long.class).invoke(null, session);
                if (root != null) try (var paths = Files.walk(root)) {
                    var iterator = paths.sorted(Comparator.reverseOrder()).iterator();
                    while (iterator.hasNext()) Files.delete(iterator.next());
                }
            } catch (Exception error) {
                code = Activity.RESULT_CANCELED;
                result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: synthetic cleanup\n");
            }
        }
        finish(code, result);
    }

    private static JSONObject value(Object raw) throws Exception {
        JSONObject response = new JSONObject((String) raw);
        check(response.getBoolean("ok"), response.optString("error") + ": " + response.optString("detail"));
        Object value = response.opt("value");
        return value instanceof JSONObject ? (JSONObject) value : new JSONObject();
    }

    private static void check(boolean accepted, String message) {
        if (!accepted) throw new AssertionError(message);
    }
}
