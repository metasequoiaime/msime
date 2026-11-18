package app.msime.android;

import app.msime.android.policy.QuickPhrasePreferencePolicy;

/** Android 新接入快捷短语混输时，旧文档缺字段仍保持关闭。 */
public final class QuickPhrasePreferencePolicySmoke {
    private QuickPhrasePreferencePolicySmoke() {}

    public static void main(String[] args) {
        check(!QuickPhrasePreferencePolicy.enabled(false, false), "缺少 quick_phrase 时保持关闭");
        check(!QuickPhrasePreferencePolicy.enabled(false, true), "缺字段不能被默认值开启");
        check(QuickPhrasePreferencePolicy.enabled(true, true), "显式开启仍然生效");
        check(!QuickPhrasePreferencePolicy.enabled(true, false), "显式关闭仍然生效");
        System.out.println("quick phrase preference policy: passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

}
