package app.msime.android.test;

import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** Device acceptance for the Microsoft double-pinyin ing key and Engine routing. */
public final class MicrosoftShuangpinDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Microsoft double-pinyin ing key visibility and Engine routing";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "shuangpin")
            .put("shuangpin_profile", "microsoft")
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("touch_shuangpin_key_hints", true)
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("microsoft"))
                .put("selected", "microsoft"));
        try {
            stage = "enable Microsoft double-pinyin preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", configured).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            stage = "Microsoft ing key visibility";
            await(microsoftKey());
            stage = "Microsoft double-pinyin hints visible";
            await(node -> node.getContentDescription() != null
                && node.getContentDescription().toString().contains("；双拼提示"));
            stage = "Microsoft ing key Engine route";
            tap(key("b"));
            tap(microsoftKey());
            // ; 是 ing 韵母，留在组字里（组字写进输入框，候选栏不再画它）；被当成分号时会上屏「把；」。
            await(field("msime-test-plain").and(node -> equalsText("b;", node.getText())));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private Predicate<AccessibilityNodeInfo> microsoftKey() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("微软双拼 ing", node.getContentDescription())
            && node.isClickable() && node.isVisibleToUser();
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
