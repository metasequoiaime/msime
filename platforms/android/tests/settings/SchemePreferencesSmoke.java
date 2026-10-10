import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;
import app.msime.android.SchemePreferences;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/**
 * 切换方案时写进偏好的那几个键。JSON 那一层（`withScheme` 拷贝快照、改 `touch_keyboard_schemes`）在这里跑不了，check-host 只有 android.jar 的 `org.json` 桩；它只是把这里测的两个函数的结果写回快照。
 */
public final class SchemePreferencesSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        AppEdition full = AppEdition.FULL;
        Map<String, String> quanpin = SchemePreferences.schemeValues(KeyboardScheme.QUANPIN, "quanpin", "xiaohe", null, full);
        check(List.copyOf(quanpin.keySet()).equals(List.of("scheme", "last_chinese_scheme", "shuangpin_profile", "touch_keyboard_layout")),
            "keys and their order: " + quanpin.keySet());
        check(quanpin.get("touch_keyboard_layout").equals("twenty_six_key"), "quanpin layout");
        // 设置页改选其它输入方式时保留自定义双拼，之后切回双拼还可继续使用原表。
        Map<String, String> leavingCustom = SchemePreferences.schemeValues(
            KeyboardScheme.QUANPIN, "shuangpin", "custom", null, full);
        check(leavingCustom.get("shuangpin_profile").equals("custom"), "custom profile survives scheme switch");

        // 每个方案写的值必须和 KeyboardScheme 自己的映射一模一样，这里只是把它摊成偏好键。
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            KeyboardScheme.PreferenceMapping mapping = scheme.mapping("shuangpin", "ziranma", full);
            Map<String, String> values = SchemePreferences.schemeValues(scheme, "shuangpin", "ziranma", null, full);
            check(values.size() == 4, scheme + " writes four keys without a wubi profile");
            check(values.get("scheme").equals(mapping.scheme()), scheme + " scheme");
            check(values.get("last_chinese_scheme").equals(mapping.lastChineseScheme()), scheme + " last_chinese_scheme");
            check(values.get("shuangpin_profile").equals(mapping.shuangpinProfile()), scheme + " shuangpin_profile");
            check(values.get("touch_keyboard_layout").equals(mapping.touchKeyboardLayout()), scheme + " touch_keyboard_layout");
        }

        Map<String, String> nineKey = SchemePreferences.schemeValues(KeyboardScheme.QUANPIN_NINE_KEY, "quanpin", "xiaohe", null, full);
        check(nineKey.get("touch_keyboard_layout").equals("nine_key"), "nine-key layout");
        Map<String, String> handwriting = SchemePreferences.schemeValues(KeyboardScheme.HANDWRITING, "quanpin", "xiaohe", null, full);
        check(handwriting.get("touch_keyboard_layout").equals("handwriting"), "handwriting layout");

        // 五笔版本只在调用方给出时才写，并且按 KeyboardScheme 的规则规范化。
        Map<String, String> wubi = SchemePreferences.schemeValues(KeyboardScheme.WUBI, "quanpin", "xiaohe", "98", full);
        check(wubi.size() == 5, "wubi profile adds a fifth key");
        check(wubi.get("wubi_profile").equals(KeyboardScheme.normalizedWubiProfile("98")), "wubi profile is normalized");
        Map<String, String> unknownWubi = SchemePreferences.schemeValues(KeyboardScheme.WUBI, "quanpin", "xiaohe", "unknown", full);
        check(unknownWubi.get("wubi_profile").equals(KeyboardScheme.normalizedWubiProfile("unknown")), "unknown wubi profile is normalized");

        // 版本不同，映射也随版本：只有五笔的包里切全拼也要落到它自己的方案上，和 KeyboardScheme 的结果一致。
        AppEdition wubiOnly = AppEdition.of("wubi", "wubi", "wubi", false);
        KeyboardScheme.PreferenceMapping wubiOnlyMapping = KeyboardScheme.QUANPIN.mapping("wubi", "xiaohe", wubiOnly);
        Map<String, String> wubiOnlyValues = SchemePreferences.schemeValues(KeyboardScheme.QUANPIN, "wubi", "xiaohe", null, wubiOnly);
        check(wubiOnlyValues.get("scheme").equals(wubiOnlyMapping.scheme()), "edition-specific scheme");
        check(wubiOnlyValues.get("last_chinese_scheme").equals(wubiOnlyMapping.lastChineseScheme()), "edition-specific last scheme");

        // 方案列表：已列出的不重复，没列出的追加到末尾，null 项原样保留。
        check(SchemePreferences.enabledAfterSwitch(List.of("quanpin", "wubi"), "wubi").equals(List.of("quanpin", "wubi")), "listed scheme kept once");
        check(SchemePreferences.enabledAfterSwitch(List.of("quanpin"), "xiaohe").equals(List.of("quanpin", "xiaohe")), "unlisted scheme appended");
        check(SchemePreferences.enabledAfterSwitch(null, "quanpin").equals(List.of("quanpin")), "missing list becomes one entry");
        check(SchemePreferences.enabledAfterSwitch(Arrays.asList("quanpin", null), "xiaohe").equals(Arrays.asList("quanpin", null, "xiaohe")),
            "null entries survive");
        List<String> original = List.of("quanpin");
        SchemePreferences.enabledAfterSwitch(original, "wubi");
        check(original.equals(List.of("quanpin")), "input list is not modified");

        check(SchemePreferences.withScheme(null, KeyboardScheme.QUANPIN, null) == null, "null snapshot");
    }
}
