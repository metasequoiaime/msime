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
        // 组字中的 ; 是 ing 韵母，交给引擎；空闲时、别的方案里、本地模式里照常是分号。
        check(MicrosoftShuangpinKeyPolicy.routesAsFinal(';', true, false, KeyboardScheme.MICROSOFT, "none"));
        check(!MicrosoftShuangpinKeyPolicy.routesAsFinal(';', false, false, KeyboardScheme.MICROSOFT, "none"));
        check(!MicrosoftShuangpinKeyPolicy.routesAsFinal(';', true, false, KeyboardScheme.XIAOHE, "none"));
        check(!MicrosoftShuangpinKeyPolicy.routesAsFinal(';', true, false, KeyboardScheme.MICROSOFT, "unicode"));
        check(!MicrosoftShuangpinKeyPolicy.routesAsFinal(',', true, false, KeyboardScheme.MICROSOFT, "none"));
        System.out.println("Android Microsoft double-pinyin key policy passed");
    }
}
