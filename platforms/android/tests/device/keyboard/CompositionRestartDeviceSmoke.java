package app.msime.android.test;

import android.os.SystemClock;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 组字写入后立即 restartInput 的合成编辑器，必须继续使用同一串候选。 */
public final class CompositionRestartDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() { return "composition survives same-editor restarts"; }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Invalid test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
            .put("learning", false).put("cloud_candidates", false)
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        try {
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.RestartingEditorActivity");
            tap(field("msime-restart-editor"));
            if (await(key("中").or(key("英"))).getText().toString().equals("英")) tap(key("英"));
            int previous = count();
            tap(key("n"));
            stage = "first composition write restarts connection";
            await(field("msime-restart-count").and(node -> Integer.parseInt(node.getText().toString()) > previous));
            SystemClock.sleep(500);
            stage = "restart preserves the first letter candidate list";
            await(describedPrefix("候选 1："));
            tap(key("i"));
            stage = "second letter continues the composition";
            await(field("msime-restart-editor").and(node -> equalsText("ni", node.getText())));
            await(describedPrefix("候选 1："));
            tap(key("空格"));
            stage = "candidate commit replaces the complete raw spelling";
            await(field("msime-restart-editor").and(node -> equalsText("你", node.getText())));
            for (String action : new String[] {"clear", "rewrite", "move"}) {
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.RestartingEditorActivity");
                tap(field("msime-restart-editor"));
                await(key("n"));
                tap(key("n"));
                await(describedPrefix("候选 1："));
                tap(field("msime-restart-" + action));
                stage = "external " + action + " does not revive old composition";
                SystemClock.sleep(500);
                tap(key("i"));
                String expected = action.equals("clear") ? "i" : action.equals("rewrite") ? "xi" : "in";
                await(field("msime-restart-editor").and(node -> equalsText(expected, node.getText())));
            }
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.RestartingEditorActivity");
            tap(field("msime-restart-editor"));
            tap(key("n"));
            await(describedPrefix("候选 1："));
            SystemClock.sleep(500);
            int beforeFinish = count();
            tap(field("msime-restart-finish"));
            stage = "marker loss without restart expires and does not replace old text";
            SystemClock.sleep(500);
            if (count() != beforeFinish) throw new AssertionError("marker loss restarted the fixture");
            stage = "next letter after marker expiry appends without rewriting old text";
            tap(key("a"));
            await(field("msime-restart-editor").and(node -> equalsText("na", node.getText())));
            stage = "candidate after marker expiry replaces only the new composition";
            tap(key("空格"));
            await(field("msime-restart-editor").and(node -> node.getText() != null
                && node.getText().length() == 2 && node.getText().charAt(0) == 'n'
                && Character.UnicodeScript.of(node.getText().charAt(1)) == Character.UnicodeScript.HAN));
            tap(field("msime-restart-other"));
            stage = "another field does not inherit the previous composition";
            tap(key("i"));
            await(field("msime-restart-other").and(node -> equalsText("i", node.getText())));
            base.put("scheme", "shuangpin").put("shuangpin_profile", "shoudao")
                .put("last_chinese_scheme", "shuangpin")
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new JSONArray().put("shoudao")).put("selected", "shoudao"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 2)
                .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.RestartingEditorActivity");
            tap(field("msime-restart-editor"));
            stage = "shoudao first letter survives restart";
            tap(key("n"));
            SystemClock.sleep(500);
            await(describedPrefix("候选 1："));
            tap(key("i"));
            stage = "shoudao second letter and candidate commit";
            await(field("msime-restart-editor").and(node -> equalsText("ni", node.getText())));
            tap(key("空格"));
            await(field("msime-restart-editor").and(node -> equalsText("你", node.getText())));
        } finally {
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private int count() {
        return Integer.parseInt(await(field("msime-restart-count")).getText().toString());
    }
}
