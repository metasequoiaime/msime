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
        // 用户在设置里关掉 `touch_shuangpin_key_hints` 后双拼也不画提示；空串会让 KeyHintButton 收回提示的内边距，读屏也不再念「双拼提示」。
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
        // 换应用或换页面时 view 要等会话建好才到（#6997）：那之前按上次记下的双拼方案画提示，提示和下边距不会在会话到达时才出现、让整排字母跳一下。
        check(ShuangpinKeyHintPolicy.rememberedProfile(1, "xiaohe").equals("xiaohe"),
            "A double-pinyin view is remembered by its profile");
        check(ShuangpinKeyHintPolicy.rememberedProfile(0, "xiaohe").isEmpty()
                && ShuangpinKeyHintPolicy.rememberedProfile(1, null).isEmpty(),
            "Other schemes remember no profile");
        check(ShuangpinKeyHintPolicy.profileWithoutView(true, "xiaohe").equals("xiaohe"),
            "While the session starts the remembered profile draws the hints");
        check(ShuangpinKeyHintPolicy.profileWithoutView(false, "xiaohe").isEmpty(),
            "An editor without an Engine session (password, failed start) draws no hints");
        check(ShuangpinKeyHintPolicy.profileWithoutView(true, "").isEmpty()
                && ShuangpinKeyHintPolicy.profileWithoutView(true, null).isEmpty(),
            "Nothing remembered draws no hints");
        // 粤语双拼（#10438）与普通粤拼是同一个引擎方案（5），只有选中的触屏方案区分两者；粤语双拼用引擎的保留名 `cantonese` 取键面，普通粤拼不画。
        check(ShuangpinKeyHintPolicy.rememberedProfile(5, "", "cantonese_shuangpin").equals("cantonese"),
            "Cantonese shuangpin asks the Engine for its reserved face");
        check(ShuangpinKeyHintPolicy.rememberedProfile(5, "", "cantonese").isEmpty(),
            "Plain Jyutping draws no hints");
        check(ShuangpinKeyHintPolicy.rememberedProfile(0, "", "cantonese_shuangpin").isEmpty(),
            "A session not running Cantonese gets no Cantonese face");
        check(ShuangpinKeyHintPolicy.rememberedProfile(1, "xiaohe", "xiaohe").equals("xiaohe"),
            "Mandarin double pinyin is unchanged");
        check(ShuangpinKeyHintPolicy.hintScheme(5, "cantonese") == 1
                && ShuangpinKeyHintPolicy.hintScheme(5, "") == 5,
            "The Cantonese face is drawn like double pinyin");
        Map<String, String> cantonese = ShuangpinKeyHintPolicy.decode(
            "{\"ok\":true,\"value\":{\"I\":\"gw / i\",\";\":\"ik ing\"}}");
        check(ShuangpinKeyHintPolicy.hint(cantonese, "i", false,
                ShuangpinKeyHintPolicy.hintScheme(5, "cantonese"), "none", true).equals("gw / i")
                && ShuangpinKeyHintPolicy.hint(cantonese, ";", false, 1, "none", true).equals("ik ing"),
            "Cantonese shuangpin hints decode and draw");
        check(ShuangpinKeyHintPolicy.profileWithoutView(true, "cantonese", "cantonese_shuangpin").equals("cantonese")
                && ShuangpinKeyHintPolicy.profileWithoutView(true, "cantonese", "cantonese").isEmpty()
                && ShuangpinKeyHintPolicy.profileWithoutView(true, "xiaohe", "quanpin").equals("xiaohe"),
            "The remembered Cantonese face is used only while Cantonese shuangpin stays selected");
        System.out.println("Android double-pinyin key hints passed");
    }
}
