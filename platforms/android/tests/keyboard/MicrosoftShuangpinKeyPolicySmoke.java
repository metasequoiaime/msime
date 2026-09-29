import app.msime.android.KeyboardScheme;
import app.msime.android.MicrosoftShuangpinKeyPolicy;

public final class MicrosoftShuangpinKeyPolicySmoke {
    private static void check(boolean value) {
        if (!value) throw new AssertionError("Microsoft double-pinyin key policy assertion failed");
    }

    public static void main(String[] args) {
        check(MicrosoftShuangpinKeyPolicy.visible(false, KeyboardScheme.MICROSOFT, "none"));
        check(!MicrosoftShuangpinKeyPolicy.visible(true, KeyboardScheme.MICROSOFT, "none"));
        check(!MicrosoftShuangpinKeyPolicy.visible(false, KeyboardScheme.QUANPIN, "none"));
        check(!MicrosoftShuangpinKeyPolicy.visible(false, KeyboardScheme.XIAOHE, "none"));
        check(!MicrosoftShuangpinKeyPolicy.visible(false, KeyboardScheme.MICROSOFT, "unicode"));
        System.out.println("Android Microsoft double-pinyin key policy passed");
    }
}
