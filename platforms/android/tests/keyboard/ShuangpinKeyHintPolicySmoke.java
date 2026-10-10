import app.msime.android.ShuangpinKeyHintPolicy;
import java.util.Map;

public final class ShuangpinKeyHintPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        Map<String, String> xiaohe = ShuangpinKeyHintPolicy.decode(
            "{\"ok\":true,\"value\":{\"U\":\"sh / u\",\"K\":\"ing uai\"}}");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "u", false, 1, "none", true)
                .equals("sh / u"), "Engine hint decoder and key normalization");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "K", false, 1, "none", true)
                .equals("ing uai"), "Engine dual-final hint");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", false, 0, "none", true).isEmpty(),
            "Full pinyin hides hints");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", true, 1, "none", true).isEmpty(),
            "English mode hides hints");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", false, 1, "emoji", true).isEmpty(),
            "Local mode hides hints");
        // 用户在设置里关掉 `touch_shuangpin_key_hints` 后双拼也不画提示；空串会让 KeyHintButton 收回 11dp 的下边距，读屏也不再念「双拼提示」。
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", false, 1, "none", false).isEmpty(),
            "The user's switch hides hints");
        check(!ShuangpinKeyHintPolicy.visible(false, 1, "none", false), "The switch overrides shuangpin");
        check(ShuangpinKeyHintPolicy.PREFERENCE_KEY.equals("touch_shuangpin_key_hints"),
            "Shared preference key");
        check(ShuangpinKeyHintPolicy.decode("{\"ok\":false}").isEmpty(),
            "Native failure hides hints");
        check(ShuangpinKeyHintPolicy.decode("{\"ok\":true,\"value\":{\"u\":\"wrong\"}}")
                .isEmpty(), "Invalid key cannot enter the hint map");
        check(ShuangpinKeyHintPolicy.decode("not json").isEmpty(),
            "Malformed native response hides hints");
        System.out.println("Android double-pinyin key hints passed");
    }
}
