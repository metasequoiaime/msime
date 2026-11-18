package app.msime.android.policy;

import org.json.JSONException;
import org.json.JSONObject;

/** Android 首次接入快捷短语混输时，旧文档缺字段必须保持原来的关闭行为。 */
public final class QuickPhrasePreferencePolicy {
    private QuickPhrasePreferencePolicy() {}

    /** 缺字段表示旧 Android 没有这项能力，所以按关闭处理。 */
    public static boolean enabled(JSONObject mixed) {
        return mixed != null && enabled(mixed.has("quick_phrase"), mixed.optBoolean("quick_phrase", false));
    }

    /** 供不依赖 Android JSON 运行时的冒烟测试核对缺字段语义。 */
    public static boolean enabled(boolean present, boolean value) {
        return present && value;
    }

    /** 把旧 Android 文档的缺字段补成显式关闭，避免运行时恢复共享层的桌面兼容默认。 */
    public static void normalize(JSONObject preferences) throws JSONException {
        if (preferences == null) return;
        JSONObject mixed = preferences.optJSONObject("mixed_input");
        if (mixed != null && !mixed.has("quick_phrase")) mixed.put("quick_phrase", false);
    }
}
