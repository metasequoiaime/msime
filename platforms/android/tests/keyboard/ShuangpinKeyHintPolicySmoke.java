import app.msime.android.ShuangpinKeyHintPolicy;
import java.util.Map;

public final class ShuangpinKeyHintPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        Map<String, String> xiaohe = ShuangpinKeyHintPolicy.decode(
            "{\"ok\":true,\"value\":{\"U\":\"sh / u\",\"K\":\"ing uai\"}}");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "u", false, 1, "none")
                .equals("sh / u"), "Engine hint decoder and key normalization");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "K", false, 1, "none")
                .equals("ing uai"), "Engine dual-final hint");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", false, 0, "none").isEmpty(),
            "Full pinyin hides hints");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", true, 1, "none").isEmpty(),
            "English mode hides hints");
        check(ShuangpinKeyHintPolicy.hint(xiaohe, "U", false, 1, "emoji").isEmpty(),
            "Local mode hides hints");
        check(ShuangpinKeyHintPolicy.decode("{\"ok\":false}").isEmpty(),
            "Native failure hides hints");
        check(ShuangpinKeyHintPolicy.decode("{\"ok\":true,\"value\":{\"u\":\"wrong\"}}")
                .isEmpty(), "Invalid key cannot enter the hint map");
        check(ShuangpinKeyHintPolicy.decode("not json").isEmpty(),
            "Malformed native response hides hints");
        System.out.println("Android double-pinyin key hints passed");
    }
}
