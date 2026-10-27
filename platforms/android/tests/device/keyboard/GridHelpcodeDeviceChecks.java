package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真正的键盘、左栏和应用 composing text 验收，复用十四键专项入口。 */
final class GridHelpcodeDeviceChecks {
    static void run(FourteenKeyDeviceSmoke t) throws Exception {
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File file = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = file.exists() ? Files.readAllBytes(file.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (String scheme : new String[] {"quanpin", "shuangpin"}) for (String layout : new String[] {"nine_key", "fourteen_key", "eighteen_key"}) {
                t.stage = scheme + " " + layout + " preferences";
                boolean nineShuangpin = scheme.equals("shuangpin") && layout.equals("nine_key");
                String selected = scheme.equals("quanpin") ? layout : nineShuangpin ? "nine_key_shuangpin" : "xiaohe";
                JSONObject prefs = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", nineShuangpin ? "quanpin" : scheme).put("shuangpin_profile", "xiaohe")
                    .put("touch_keyboard_layout", nineShuangpin ? "nine_key_shuangpin" : layout)
                    .put("touch_shuangpin_layout", layout.equals("nine_key") ? "twenty_six_key" : layout)
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put((nineShuangpin ? "quanpin" : scheme) + "_helpcode", new JSONObject().put("enabled", true).put("schema", "ziranma")
                        .put("grid_enabled", true).put("strict_filter", true).put("show_in_candidate_window", false))
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin").put("xiaohe").put("nine_key_shuangpin").put(layout)).put("selected", selected));
                t.publish(file, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", prefs).toString().getBytes(StandardCharsets.UTF_8));
                t.restartIme();
                t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                t.tap(t.field("msime-test-plain"));
                t.await(t.key("中"));
                t.stage = scheme + " " + layout + " pinyin and begin";
                if (layout.equals("nine_key")) {
                    for (String face : scheme.equals("quanpin") ? new String[] {"MNO", "GHI", "GHI", "ABC", "MNO"} : new String[] {"6", "4"}) t.tap(t.key(face));
                } else {
                    String[] groups = layout.equals("fourteen_key") ? new String[] {"B N", "U I", "G H", "A S", "O P"}
                        : new String[] {"B N", "I O", "H", "A", "I O"};
                    for (String face : groups) t.tap(t.described((face.length() == 1 ? "字母 " : "按键 ") + face));
                }
                t.tap(t.key("辅"));
                t.await(t.key("辅·小"));
                t.await(t.imeTextContains("|"));
                String first = layout.equals("nine_key") ? "abc" : layout.equals("fourteen_key") ? "qw" : "we";
                if (layout.equals("nine_key")) t.tap(t.key("ABC").or(t.key("abc")));
                else t.tap(t.described("按键 " + (layout.equals("fourteen_key") ? "Q W" : "W E")));
                t.await(t.imeTextContains("|[" + first + "]"));
                t.tap(t.described("选择辅码 " + first.charAt(0)));
                t.await(t.imeTextContains("|" + first.charAt(0)));
                t.tap(t.key("辅·小"));
                t.await(t.key("辅·大"));
                String second = layout.equals("nine_key") ? "DEF" : layout.equals("fourteen_key") ? "ER" : "RT";
                if (layout.equals("nine_key")) t.tap(t.key("DEF"));
                else t.tap(t.described("按键 " + (layout.equals("fourteen_key") ? "E R" : "R T")));
                t.await(t.imeTextContains("|" + first.charAt(0) + "[" + second + "]"));
                // 第三码不得覆盖已有两槽或自动提交。
                if (layout.equals("nine_key")) t.tap(t.key("DEF"));
                else t.tap(t.described("按键 " + (layout.equals("fourteen_key") ? "E R" : "R T")));
                t.await(t.imeTextContains("|" + first.charAt(0) + "[" + second + "]"));
                t.shellDiscard("screencap -p /data/local/tmp/msime-grid-" + scheme + "-" + layout + ".png");
                t.tap(t.described("选择辅码 " + first.charAt(0) + second.charAt(0)));
                t.await(t.imeTextContains("|" + first.charAt(0) + second.charAt(0)));
                t.tap(t.key("⌫"));
                t.await(t.imeTextContains("|" + first.charAt(0)));
                t.stage = scheme + " " + layout + " symbol overlay preserves filter and case";
                swipe(t, "辅·大", -90);
                t.shellDiscard("input keyevent 4");
                t.await(t.key("辅·大"));
                t.await(t.imeTextContains("|" + first.charAt(0)));
                t.stage = scheme + " " + layout + " end retains and reenter defaults lowercase";
                swipe(t, "辅·大", 90);
                t.await(t.key("辅"));
                t.await(t.imeTextContains("|" + first.charAt(0)));
                t.tap(t.key("辅"));
                t.await(t.key("辅·小"));
                t.stage = scheme + " " + layout + " separator clears helper";
                if (!t.await(t.key("辅·小")).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                    throw new AssertionError("helper separator long click unavailable");
                t.await(t.key("辅"));
                t.awaitGone(t.imeTextContains("|"));
                AccessibilityNodeInfo field = t.await(t.field("msime-test-plain"));
                if (field.getText() != null && field.getText().toString().contains("|"))
                    throw new AssertionError("helper text leaked into application");
                t.shellDiscard("input keyevent 4");
            }
        } finally {
            if (original == null) Files.deleteIfExists(file.toPath()); else t.publish(file, original);
        }
    }
    private static void swipe(FourteenKeyDeviceSmoke t, String face, int dy) throws Exception {
        Rect bounds = new Rect(); t.await(t.key(face)).getBoundsInScreen(bounds);
        t.shellDiscard("input swipe " + bounds.centerX() + " " + bounds.centerY() + " " + bounds.centerX() + " " + (bounds.centerY() + dy) + " 180");
    }
}
