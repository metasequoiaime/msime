package app.msime.android.test;

import android.os.SystemClock;
import android.view.KeyEvent;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 辅助码关闭时，完整拼音后按 Shift+字母（#7162）：字母不能丢。 */
public final class ChineseHelpcodeOffDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Shift letter after a complete quanpin syllable with helpcode off";
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
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("quanpin_helpcode", new JSONObject().put("enabled", false).put("schema", "ziranma"));
        try {
            stage = "disable quanpin helpcode";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", configured).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            stage = "complete quanpin composition";
            for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
            // 全拼组字在输入框里按音节分词显示（#9448，见 2026-10-23-quanpin-host-preedit-segmentation.md）。
            await(field("msime-test-plain").and(node -> equalsText("ni'hao", node.getText())));

            stage = "shift letter after the composition";
            // 全拼触屏显示分词，键盘 Shift 修饰仍必须保留原大写字母。
            long time = SystemClock.uptimeMillis();
            KeyEvent letter = new KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_O,
                0, KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON);
            injectKeyboardEvent(letter);
            injectKeyboardEvent(KeyEvent.changeAction(letter, KeyEvent.ACTION_UP));
            // 引擎在拼音组字中接收大写字母（#7146），O 以大写并入组字、不上屏；引擎一旦拒收而宿主不处理，O 就会丢，输入框停在 nihao。
            await(field("msime-test-plain").and(node -> equalsText("nihaoO", node.getText())));
            await(key("o"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
