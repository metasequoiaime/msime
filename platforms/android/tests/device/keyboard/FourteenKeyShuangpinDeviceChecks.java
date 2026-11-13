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

/** Android 字母组双拼：真实设置、组码、消歧、分号、回落和重启的验收。 */
final class FourteenKeyShuangpinDeviceChecks {
    private final DeviceSmoke t;
    private final boolean eighteen;
    private final String layout;
    private final String layoutTitle;
    private final String layoutKeyDescription;
    private File preferences;
    private JSONObject configured;
    private long revision;

    private FourteenKeyShuangpinDeviceChecks(DeviceSmoke test, boolean eighteen) {
        t = test;
        this.eighteen = eighteen;
        layout = eighteen ? "eighteen_key" : "fourteen_key";
        layoutTitle = eighteen ? "18 键" : "14 键";
        layoutKeyDescription = eighteen ? "按键 W E" : "按键 Q W";
    }

    static void run(DeviceSmoke test) throws Exception {
        new FourteenKeyShuangpinDeviceChecks(test, false).run();
    }

    static void runEighteen(DeviceSmoke test) throws Exception {
        new FourteenKeyShuangpinDeviceChecks(test, true).run();
    }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        preferences = new File(new File(options.getString("preferences_directory")).getCanonicalFile(), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "shuangpin").put("shuangpin_profile", "xiaohe")
            .put("touch_keyboard_layout", "twenty_six_key").put("touch_shuangpin_layout", "twenty_six_key")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                .put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                .put("quanpin").put("xiaohe").put("ziranma").put("microsoft").put("shoudao"))
                .put("selected", "xiaohe"));
        try {
            t.stage = "fourteen shuangpin baseline";
            save();
            t.restartIme();
            t.stage = "choose fourteen shuangpin in native settings";
            layoutSetting("小鹤", "26 键", layoutTitle);
            assertStored("xiaohe", layout);
            if (eighteen) EighteenKeyGeometryDeviceChecks.run(t);
            editor(false);
            t.stage = "fourteen shuangpin nihao";
            groups("B N", "U I", "G H", "C V");
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你好", node.getText())));

            t.stage = "fourteen shuangpin disambiguation and lock undo";
            groups("B N", "U I");
            t.await(t.described("选择拼音 ni"));
            t.await(t.described("选择拼音 " + (eighteen ? "bi" : "bu")));
            t.tap(t.described("选择拼音 ni"));
            t.await(reading("ni"));
            t.tap(t.key("⌫"));
            t.await(t.described("选择拼音 " + (eighteen ? "bi" : "bu")));
            t.tap(t.key("展开").and(AccessibilityNodeInfo::isEnabled));
            t.await(t.described("九键候选面板"));
            t.tap(t.described("收起候选面板"));
            t.tap(t.described("选择拼音 ni"));
            t.tap(t.described("按键 分词，在这里断开音节"));
            groups("G H", "C V");
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你好你好", node.getText())));

            exactSwipeChecks();

            t.stage = "fourteen shuangpin English and symbols";
            t.tap(t.key("中"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            t.tap(t.key("英"));
            t.await(t.described(layoutKeyDescription));
            t.tap(t.key("123"));
            t.await(t.key("1").and(AccessibilityNodeInfo::isClickable));
            t.tap(t.described("返回原键盘"));
            t.await(t.described(layoutKeyDescription));
            assertStored("xiaohe", layout);

            t.stage = "fourteen shuangpin password fallback, URL keeps the layout";
            t.tap(t.field("msime-test-password"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            absent(t.described(layoutKeyDescription));
            // 网址框（浏览器地址栏同时是搜索框）按默认中英文进框，保留 14 键（#9725）。
            editor(true);
            t.await(t.described(layoutKeyDescription));
            editor(false);
            t.await(t.described(layoutKeyDescription));
            t.restartIme();
            editor(false);
            t.await(t.described(layoutKeyDescription));
            assertStored("xiaohe", layout);

            t.stage = "switch shuangpin profile preserves fourteen keys";
            typingSettings();
            t.stage = "open shuangpin choices to switch profile";
            t.tap(t.described("双拼，小鹤双拼"));
            t.stage = "choose Microsoft shuangpin";
            t.tap(t.key("微软"));
            assertStored("microsoft", layout);
            editor(false);
            t.await(t.described("按键 L 分号"));
            groups("Q W");
            t.stage = "Microsoft semicolon spelling and commit";
            t.tap(t.described("按键 L 分号"));
            commitQing();
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("青", node.getText())));
            t.shellDiscard("screencap -p /data/local/tmp/" + (eighteen ? "msime-9124-eighteen" : "msime-8770-fourteen") + "-shuangpin.png");

            t.stage = "fourteen shuangpin custom semicolon and invalid table fallback";
            configured = stored().put("shuangpin_profile", "custom")
                .put("shuangpin_custom_profile", customMicrosoft());
            configured.getJSONObject("touch_keyboard_schemes").put("selected", "xiaohe");
            save();
            t.restartIme();
            editor(false);
            groups("Q W");
            t.tap(t.described("按键 L 分号"));
            commitQing();
            configured.put("shuangpin_custom_profile", new JSONObject());
            save();
            t.restartIme();
            editor(false);
            groups("B N", "U I", "G H", "C V");
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你好", node.getText())));
            // 设置布局只改布局字段，不能把自定义表回退时显示的小鹤写成用户选择。
            t.stage = "switch back to twenty-six keys keeps the custom profile";
            layoutSetting("小鹤", layoutTitle, "26 键");
            assertStored("custom", "twenty_six_key");
            editor(false);
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            absent(t.described(layoutKeyDescription));
        } finally {
            t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, original);
        }
    }

    /**
     * 真实触摸覆盖左右选择、反向重选、取消、回中心及手势之后的普通点按；14 键和 18 键双拼相同（#8930、#11040）。小鹤「你好」是 N I H C：18 键在 BN、IO、H、XC 上，14 键在 BN、UI、GH、CV 上，i 在 IO 键左边、UI 键右边。
     */
    private void exactSwipeChecks() throws Exception {
        String iKey = eighteen ? "按键 I O" : "按键 U I";
        float iSide = eighteen ? -40 : 40;
        editor(false);
        t.stage = layoutTitle + " exact right swipe";
        GridSwipeTouch.swipe(t, "按键 B N", false, 40);
        t.await(reading("n"));
        t.stage = layoutTitle + " exact swipe cancellation";
        GridSwipeTouch.swipe(t, "按键 B N", true, -40);
        t.await(reading("n"));
        t.stage = layoutTitle + " exact swipe return to center";
        GridSwipeTouch.swipe(t, "按键 B N", false, 40, 0);
        t.await(reading("n"));
        t.stage = layoutTitle + " exact swipe reversal";
        GridSwipeTouch.swipe(t, iKey, false, -iSide, iSide);
        t.await(t.described("选择拼音 ni"));
        absent(t.described("选择拼音 bi"));
        if (eighteen) GridSwipeTouch.swipe(t, "字母 H", false);
        else GridSwipeTouch.swipe(t, "按键 G H", false, 40);
        GridSwipeTouch.swipe(t, eighteen ? "按键 X C" : "按键 C V", false, eighteen ? 40 : -40);
        t.tap(candidate("你好"));
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你好", node.getText())));

        t.stage = layoutTitle + " exact left letter";
        GridSwipeTouch.swipe(t, "按键 B N", false, -40);
        GridSwipeTouch.swipe(t, iKey, false, iSide);
        t.await(t.described("选择拼音 bi"));
        absent(t.described("选择拼音 ni"));
        t.tap(t.key("⌫"));
        t.tap(t.key("⌫"));

        t.stage = layoutTitle + " taps stay ambiguous after swipe";
        GridSwipeTouch.swipe(t, "按键 B N", false);
        GridSwipeTouch.swipe(t, iKey, false);
        t.await(t.described("选择拼音 bi"));
        t.await(t.described("选择拼音 ni"));
        t.tap(t.key("⌫"));
        t.tap(t.key("⌫"));
    }

    private void commitQing() throws Exception {
        t.await(t.described("选择拼音 qing"));
        // 合并键有多种读音；先锁定目标音节，再从展开候选选择，不依赖首行排名。
        t.tap(t.described("选择拼音 qing"));
        t.await(reading("qing"));
        t.tap(t.key("展开").and(AccessibilityNodeInfo::isEnabled));
        t.await(t.described("九键候选面板"));
        t.tap(candidate("青"));
    }

    private void groups(String... pairs) throws Exception {
        for (String pair : pairs) {
            String letters = eighteen ? switch (pair) {
                case "U I" -> "I O";
                case "G H" -> "H";
                case "C V" -> "X C";
                case "Q W" -> "Q";
                default -> pair;
            } : pair;
            t.tap(t.described((letters.length() == 1 ? "字母 " : "按键 ") + letters));
        }
    }

    private void editor(boolean url) throws Exception {
        // 不用 -W：仪器与编辑器切换时主线程可能互等，改用实际输入框和键面同步。
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_url " + url);
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.dropStaleNodes();
    }

    // 输入页「语言与方案」按方案族分行，双拼一行直接列各双拼方案（#9653，见 2026-10-25-mobile-scheme-rows.md）。
    private void typingSettings() throws Exception {
        t.stage = "open the typing settings language section";
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TYPING");
        t.await(t.key("语言").and(AccessibilityNodeInfo::isClickable));
        t.await(t.described("双拼，小鹤双拼"));
    }

    // 双拼的 26／14／18 键排法在键盘页「布局」的「中文键盘」，只换排法、保留双拼方案（#9653）。
    private void layoutSetting(String profile, String before, String after) throws Exception {
        t.stage = "open the keyboard layout section for " + profile;
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        t.await(t.key("布局").and(AccessibilityNodeInfo::isClickable));
        t.stage = "open shuangpin layout choices from " + before;
        t.await(t.key(before));
        t.tap(t.key("中文键盘"));
        t.await(t.key("保留当前双拼方案"));
        t.stage = "choose shuangpin layout " + after;
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
            if (profile.equals(value.optString("shuangpin_profile")) && layout.equals(
                    value.optString("touch_shuangpin_layout", "twenty_six_key"))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("双拼方案或布局未持久化");
    }

    private Predicate<AccessibilityNodeInfo> reading(String text) {
        return t.described("编辑拼写").and(node -> DeviceSmoke.equalsText(text, node.getText()));
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    private void absent(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                if (t.find(window.getRoot(), match) != null) { present = true; break; }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("不应显示字母网格的输入框仍有组码键");
    }

    private JSONObject customMicrosoft() throws Exception {
        return new JSONObject("{\"initials\":{\"sh\":\"u\",\"ch\":\"i\",\"zh\":\"v\"},\"finals\":{\"iu\":\"q\",\"ia\":\"w\",\"ua\":\"w\",\"e\":\"e\",\"uan\":\"r\",\"ue\":\"t\",\"ve\":\"v\",\"uai\":\"y\",\"v\":\"y\",\"u\":\"u\",\"i\":\"i\",\"o\":\"o\",\"uo\":\"o\",\"un\":\"p\",\"a\":\"a\",\"iong\":\"s\",\"ong\":\"s\",\"iang\":\"d\",\"uang\":\"d\",\"en\":\"f\",\"eng\":\"g\",\"ang\":\"h\",\"an\":\"j\",\"ao\":\"k\",\"ai\":\"l\",\"ing\":\";\",\"ei\":\"z\",\"ie\":\"x\",\"iao\":\"c\",\"ui\":\"v\",\"ou\":\"b\",\"in\":\"n\",\"ian\":\"m\"},\"zero_initials\":{\"a\":\"oa\",\"ai\":\"ol\",\"an\":\"oj\",\"ang\":\"oh\",\"ao\":\"ok\",\"e\":\"oe\",\"ei\":\"oz\",\"en\":\"of\",\"eng\":\"og\",\"er\":\"or\",\"o\":\"oo\",\"ou\":\"ob\"}}");
    }
}
