package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 五笔 14 键（#9691）的设备验收：原生设置 26→14 的持久化与 86/98 保留、组码出字上屏、左端键始终是符号、展开不是拼音三栏、实体字母、英文、密码和网址回落、重启恢复、切回 26 键。 */
final class FourteenKeyWubiDeviceChecks {
    private final DeviceSmoke t;
    private File preferences;
    private JSONObject configured;
    private long revision;

    private FourteenKeyWubiDeviceChecks(DeviceSmoke test) {
        t = test;
    }

    static void run(DeviceSmoke test) throws Exception {
        new FourteenKeyWubiDeviceChecks(test).run();
    }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        preferences = new File(new File(options.getString("preferences_directory")).getCanonicalFile(), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "wubi").put("wubi_profile", "wubi86")
            .put("touch_keyboard_layout", "twenty_six_key").put("touch_wubi_layout", "twenty_six_key")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("wubi_mixed_pinyin", true).put("wubi_fifth_code_top_commit", true)
            .put("wubi_auto_commit_unique", false)
            .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                .put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                .put("quanpin").put("wubi")).put("selected", "wubi"));
        try {
            t.stage = "fourteen wubi baseline";
            save();
            t.restartIme();
            editor(false);
            t.stage = "26-key wubi fifth-code top commit with mixed pinyin";
            t.shellDiscard("input text wqiy");
            t.await(candidate("你"));
            // 顶字立即上屏「你」，即使安卓开着 `phrase_preedit` 也不进暂存词段；`a` 开始下一组编码，按安卓默认写进输入框组字区，所以输入框是「你a」。读音行不再画暂存的「你」。
            t.shellDiscard("input text a");
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你a", node.getText())));
            absent(t.described("编辑拼写").and(node -> DeviceSmoke.equalsText("你", node.getText())));
            t.tap(candidate("工"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你工", node.getText())));
            layoutSetting("26 键", "14 键");
            assertStored("wubi86", "fourteen_key");
            editor(false);

            t.stage = "fourteen wubi face";
            t.await(t.described("按键 Q W"));
            t.await(t.described("字母 L"));
            t.await(t.described("按键 符号"));
            t.await(t.described("符号栏，可上下滑动"));
            absent(t.key("q"));

            // QW QW 的组码是 qq，可能的五笔码是 qq、qw、wq、ww；读音行显示首选的编码，不显示内部组码，也不写进输入框。
            t.stage = "fourteen wubi code";
            groups("Q W", "Q W");
            t.await(t.described("编辑拼写").and(node -> node.getText() != null
                && java.util.List.of("qq", "qw", "wq", "ww").contains(node.getText().toString())));
            t.await(t.field("msime-test-plain").and(FourteenKeyWubiDeviceChecks::emptyField));
            t.stage = "fourteen wubi left key stays symbols while composing";
            t.await(t.described("按键 符号"));
            absent(t.described("按键 分词，在这里断开音节"));
            absent(t.described("14 键拼音选择"));
            t.await(t.described("符号栏，可上下滑动"));
            t.stage = "fourteen wubi expansion is not the pinyin panel";
            if (present(t.key("展开").and(AccessibilityNodeInfo::isEnabled))) {
                t.tap(t.key("展开").and(AccessibilityNodeInfo::isEnabled));
                t.await(t.described("候选面板"));
                absent(t.described("九键候选面板"));
                t.tap(t.described("收起候选面板"));
                absent(t.described("候选面板"));
            }
            t.stage = "fourteen wubi commit";
            t.tap(candidate("你"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你", node.getText())));

            // 组字中点左端的「符」：按高亮行结束组字并打开符号面板，不把 `'` 交给五笔。
            t.stage = "fourteen wubi symbol key while composing";
            groups("Q W", "Q W");
            t.await(t.described("编辑拼写"));
            t.tap(t.described("按键 符号"));
            t.await(t.described("符号面板"));
            t.await(t.field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().length() > 1 && !node.getText().toString().contains("'")));
            t.tap(t.described("关闭符号面板").or(t.described("按键 关闭符号面板")));
            t.await(t.described("按键 Q W"));
            String afterSymbol = text();

            t.stage = "fourteen wubi hardware letters keep the 26-key path";
            t.shellDiscard("input text wq");
            t.tap(candidate("你"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(afterSymbol + "你", node.getText())));

            t.stage = "fourteen wubi English and symbols";
            t.tap(t.key("中"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            absent(t.described("按键 Q W"));
            t.tap(t.key("英"));
            t.await(t.described("按键 Q W"));
            t.tap(t.key("123"));
            t.await(t.key("1").and(AccessibilityNodeInfo::isClickable));
            t.tap(t.described("返回原键盘"));
            t.await(t.described("按键 Q W"));
            assertStored("wubi86", "fourteen_key");

            t.stage = "fourteen wubi password fallback, URL keeps the layout";
            t.tap(t.field("msime-test-password"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            absent(t.described("按键 Q W"));
            // 网址框（浏览器地址栏同时是搜索框）按默认中英文进框，保留 14 键（#9725）。
            editor(true);
            t.await(t.described("按键 Q W"));
            editor(false);
            t.await(t.described("按键 Q W"));
            t.stage = "fourteen wubi restart preserves the layout";
            t.restartIme();
            editor(false);
            t.await(t.described("按键 Q W"));
            assertStored("wubi86", "fourteen_key");

            // 换到 98 只改 `wubi_profile`，14 键保留；98 下照样组码出字。
            t.stage = "switch Wubi 98 preserves fourteen keys";
            typingSettings();
            t.tap(t.described("五笔，86 五笔"));
            t.stage = "choose Wubi 98";
            t.tap(t.key("五笔 98"));
            assertStored("wubi98", "fourteen_key");
            editor(false);
            t.await(t.described("按键 Q W"));
            String before = text();
            t.stage = "fourteen wubi 98 commit";
            // 换版本后键盘重读偏好、重建会话；等工具栏的方案标成「五98」再打，否则先到的组码会落进旧会话。
            t.await(t.key("五98"));
            t.awaitStableBounds(t.described("按键 Q W"));
            groups("Q W", "Q W");
            t.await(t.described("编辑拼写"));
            t.tap(t.describedPrefix("候选 1："));
            t.await(t.field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().length() > before.length()));

            // 设置布局只改布局字段，98 保留。
            t.stage = "switch back to twenty-six keys keeps Wubi 98";
            layoutSetting("14 键", "26 键");
            assertStored("wubi98", "twenty_six_key");
            editor(false);
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            absent(t.described("按键 Q W"));
        } finally {
            t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, original);
        }
    }

    private void groups(String... pairs) throws Exception {
        for (String pair : pairs) t.tap(t.described((pair.length() == 1 ? "字母 " : "按键 ") + pair));
    }

    private void editor(boolean url) throws Exception {
        // 不用 -W：仪器与编辑器切换时主线程可能互等，改用实际输入框和键面同步。
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_url " + url);
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.dropStaleNodes();
    }

    private String text() throws Exception {
        AccessibilityNodeInfo field = t.await(t.field("msime-test-plain"));
        CharSequence value = field.getText();
        return value == null || field.isShowingHintText() ? "" : value.toString();
    }

    // 输入页「语言与方案」按方案族分行，五笔一行直接列 86 和 98（#9653）。
    private void typingSettings() throws Exception {
        t.stage = "open the typing settings language section";
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TYPING");
        t.await(t.key("语言").and(AccessibilityNodeInfo::isClickable));
        t.await(t.described("五笔，86 五笔"));
    }

    // 五笔的 26／14 键排法在键盘页「布局」的「五笔键盘」，只换排法、保留五笔版本（#9691）。
    private void layoutSetting(String before, String after) throws Exception {
        t.stage = "open the keyboard layout section for Wubi";
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        t.await(t.key("布局").and(AccessibilityNodeInfo::isClickable));
        t.stage = "open Wubi layout choices from " + before;
        t.await(t.key(before));
        t.tap(t.key("五笔键盘"));
        t.await(t.key("保留当前五笔版本"));
        t.stage = "choose Wubi layout " + after;
        t.tap(t.key(after));
    }

    private JSONObject stored() throws Exception {
        return new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8)).getJSONObject("preferences");
    }

    private void save() throws Exception {
        if (preferences.exists()) revision = Math.max(revision,
            new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8)).getLong("revision"));
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
    }

    private void assertStored(String profile, String layout) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            JSONObject value = stored();
            if ("wubi".equals(value.optString("scheme"))
                    && profile.equals(value.optString("wubi_profile", "wubi86"))
                    && layout.equals(value.optString("touch_wubi_layout", "twenty_six_key"))
                    && "twenty_six_key".equals(value.optString("touch_keyboard_layout", "twenty_six_key"))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("五笔版本或布局未持久化");
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            // 五笔候选带编码提示，描述是「候选 3：你，提示：wqiy」或「…，还需输入 iy」，候选字在第一个「，」或「；」之前。
            String value = description.substring(delimiter + 1);
            int end = value.length();
            for (char mark : new char[] {'；', '，'}) {
                int at = value.indexOf(mark);
                if (at >= 0) end = Math.min(end, at);
            }
            return value.substring(0, end).equals(text);
        };
    }

    private static boolean emptyField(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }

    private boolean present(Predicate<AccessibilityNodeInfo> match) throws Exception {
        t.automation.waitForIdle(500, 5000);
        for (AccessibilityWindowInfo window : t.automation.getWindows())
            if (t.find(window.getRoot(), match) != null) return true;
        return false;
    }

    private void absent(Predicate<AccessibilityNodeInfo> match) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (!present(match)) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("五笔 14 键不应显示的界面仍在");
    }
}
