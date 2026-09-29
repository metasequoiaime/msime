import app.msime.android.JapaneseVariantPolicy;

public final class JapaneseVariantPolicySmoke {
    public static void main(String[] args) {
        check(JapaneseVariantPolicy.enabled(true, false, true),
            "Japanese variants are enabled during composition");
        check(!JapaneseVariantPolicy.enabled(true, false, false),
            "Japanese variants require a kana composition");
        check(!JapaneseVariantPolicy.enabled(true, true, true),
            "Japanese digit layer does not expose kana variants");
        check(!JapaneseVariantPolicy.enabled(false, false, true),
            "Other layouts do not expose Japanese variants");
        check(JapaneseVariantPolicy.accessibilityLabel(false)
            .contains("请先输入假名"), "Disabled state explains the prerequisite");
        check(JapaneseVariantPolicy.accessibilityLabel(true)
            .equals("小假名、浊音、半浊音"), "Enabled state stays concise");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
