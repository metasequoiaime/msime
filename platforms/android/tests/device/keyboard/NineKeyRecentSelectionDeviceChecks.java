package app.msime.android.test;

import android.content.Context;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 通过产品的学习记录导出验收九键最近选择，不把模型裁决后的显示座位误当作持久化槽位。 */
final class NineKeyRecentSelectionDeviceChecks {
    private NineKeyRecentSelectionDeviceChecks() {}

    static void assertRecorded(Context target, JSONObject options, String spelling, String word)
            throws Exception {
        File snapshot = File.createTempFile("synthetic-nine-key-recent-", ".ndjson", target.getCacheDir());
        try {
            JSONObject request = new JSONObject().put("options", options)
                .put("action", new JSONObject().put("operation", "export_snapshot")
                    .put("include_learning", true).put("destination", snapshot.getAbsolutePath()));
            Class<?> client = target.getClassLoader().loadClass("app.msime.android.NativeClient");
            String response = (String) client.getMethod("dictionary", String.class)
                .invoke(null, request.toString());
            if (!new JSONObject(response).getBoolean("ok"))
                throw new AssertionError("Synthetic recent-selection export failed");
            for (String line : Files.readAllLines(snapshot.toPath(), StandardCharsets.UTF_8)) {
                JSONObject record = new JSONObject(line);
                if (!"position".equals(record.optString("type"))) continue;
                JSONObject data = record.getJSONObject("data");
                if ("nine-key:64:".equals(data.optString("context"))
                        && spelling.equals(data.optString("code"))
                        && word.equals(data.optString("word"))
                        && data.optInt("position") == 1) return;
            }
            throw new AssertionError("Synthetic unlocked recent selection was not persisted in slot one");
        } finally {
            Files.deleteIfExists(snapshot.toPath());
        }
    }
}
