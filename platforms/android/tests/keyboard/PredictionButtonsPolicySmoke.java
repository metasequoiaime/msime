import app.msime.android.PredictionButtonsPolicy;
import app.msime.android.PredictionButtonsPolicy.Style;

public final class PredictionButtonsPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(PredictionButtonsPolicy.PREFERENCE_KEY.equals("next_word_prediction_buttons"), "shared preference key");
        // 写法与 client-core 的 NextWordPredictionButtons 一一对应。
        check(Style.parse("expand_only") == Style.EXPAND_ONLY, "expand_only");
        check(Style.parse("close_and_expand") == Style.CLOSE_AND_EXPAND, "close_and_expand");
        check(Style.parse("close_only") == Style.CLOSE_ONLY, "close_only");
        for (Style style : Style.values()) check(Style.parse(style.wire()) == style, "wire round trip " + style);
        // 缺键（optString 给空串）、null 和较新版本的写法都按现有样式。
        check(Style.parse("") == Style.EXPAND_ONLY, "missing key reads as the existing style");
        check(Style.parse(null) == Style.EXPAND_ONLY, "null reads as the existing style");
        check(Style.parse("swipe_to_close") == Style.EXPAND_ONLY, "unknown style reads as the existing style");

        // 联想时：方案 2（现有）只有展开键；方案 1 两个都有；方案 3 只有关闭键。
        check(!PredictionButtonsPolicy.showsClose(Style.EXPAND_ONLY, true)
            && PredictionButtonsPolicy.allowsExpand(Style.EXPAND_ONLY, true), "expand only keeps today's strip");
        check(PredictionButtonsPolicy.showsClose(Style.CLOSE_AND_EXPAND, true)
            && PredictionButtonsPolicy.allowsExpand(Style.CLOSE_AND_EXPAND, true), "close and expand shows both");
        check(PredictionButtonsPolicy.showsClose(Style.CLOSE_ONLY, true)
            && !PredictionButtonsPolicy.allowsExpand(Style.CLOSE_ONLY, true), "close only drops the expand key");

        // 普通组字不受样式影响：没有关闭键，展开键照旧。
        for (Style style : Style.values()) {
            check(!PredictionButtonsPolicy.showsClose(style, false), "no close key while composing: " + style);
            check(PredictionButtonsPolicy.allowsExpand(style, false), "expand key unchanged while composing: " + style);
        }
        System.out.println("Prediction buttons policy: three styles and composition fallback hold");
    }
}
