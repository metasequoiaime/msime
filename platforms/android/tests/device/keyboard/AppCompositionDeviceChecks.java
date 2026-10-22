package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 通过系统真实包名路由验证搜索兼容，夹具与产品分属不同进程。 */
final class AppCompositionDeviceChecks {
    private static final String BANK = "com.shengjingbank.mobile.cust";

    static void run(DeviceSmoke s) throws Exception {
        File root = s.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Invalid test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (String scheme : new String[] {"quanpin", "nine_key", "xiaohe"}) {
                JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", scheme.equals("xiaohe") ? "shuangpin" : "quanpin")
                    .put("shuangpin_profile", "xiaohe")
                    .put("touch_keyboard_layout", scheme.equals("nine_key") ? "nine_key" : "twenty_six_key")
                    .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
                    .put("learning", false).put("cloud_candidates", false)
                    .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                        .put("emoji", false).put("kaomoji", false))
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put(scheme))
                        .put("selected", scheme));
                s.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
                s.restartIme();
                launch(s, BANK);
                s.stage = scheme + " keyboard-only composition does not write to the search field";
                String[] keys = scheme.equals("nine_key") ? new String[] {"MNO", "GHI"} : new String[] {"n", "i"};
                for (String letter : keys) {
                    s.tap(s.key(letter));
                    s.await(s.describedPrefix("候选 1："));
                    assertUnchanged(s, BANK);
                }
                s.stage = scheme + " spelling remains visible on the keyboard";
                s.await(s.imeTextContains("ni"));
                s.stage = scheme + " deletion changes only the keyboard composition";
                s.tap(s.key("⌫"));
                assertUnchanged(s, BANK);
                s.tap(s.key(keys[1]));
                s.await(s.describedPrefix("候选 1："));
                s.stage = scheme + " candidate confirmation commits once with the existing prefix";
                s.tap(s.describedPrefix("候选 1："));
                s.await(field(BANK, "editor").and(node -> DeviceSmoke.equalsText("前你", node.getText())));
                s.await(field(BANK, "counts").and(node -> DeviceSmoke.equalsText("1:0", node.getText())));
                s.stage = scheme + " fresh composition after commit remains outside the field";
                s.tap(s.key(keys[0]));
                s.await(s.describedPrefix("候选 1："));
                SystemClock.sleep(250);
                s.await(field(BANK, "editor").and(node -> DeviceSmoke.equalsText("前你", node.getText())));
                s.await(field(BANK, "counts").and(node -> DeviceSmoke.equalsText("1:0", node.getText())));
            }
            // 同一控件换成普通测试包，行内组字必须仍然生效，兼容规则不能泄漏到下一应用。
            s.stage = "ordinary package retains inline composition after leaving the excluded app";
            launch(s, "app.msime.android.test");
            s.tap(s.key("n"));
            s.await(field("app.msime.android.test", "editor").and(node -> DeviceSmoke.equalsText("前n", node.getText())));
            s.await(field("app.msime.android.test", "counts").and(node -> DeviceSmoke.equalsText("1:1", node.getText())));
        } finally {
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else s.publish(preferences, original);
            s.shellDiscard("am force-stop " + BANK);
        }
    }

    private static void launch(DeviceSmoke s, String packageName) throws Exception {
        s.shellDiscard("am start -W -f 0x10008000 -n " + packageName
            + "/app.msime.android.test.CompositionSearchEditorActivity");
        s.stage = "search fixture editor focus";
        s.tap(field(packageName, "editor"));
        s.stage = "search fixture language key";
        AccessibilityNodeInfo language = s.await(s.key("中").or(s.key("英")));
        if (DeviceSmoke.equalsText("英", language.getText())) s.tap(s.key("英"));
        s.stage = "search fixture pristine counters";
        s.await(field(packageName, "counts").and(node -> DeviceSmoke.equalsText("0:0", node.getText())));
    }

    private static void assertUnchanged(DeviceSmoke s, String packageName) {
        SystemClock.sleep(250);
        s.await(field(packageName, "editor").and(node -> DeviceSmoke.equalsText("前", node.getText())));
        s.await(field(packageName, "counts").and(node -> DeviceSmoke.equalsText("0:0", node.getText())));
    }

    private static Predicate<AccessibilityNodeInfo> field(String packageName, String id) {
        return node -> DeviceSmoke.equalsText(packageName, node.getPackageName())
            && DeviceSmoke.equalsText("composition-search-" + id, node.getContentDescription());
    }
}
