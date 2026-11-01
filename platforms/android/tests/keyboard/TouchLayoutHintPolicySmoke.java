import app.msime.android.KeyboardLayout;
import app.msime.android.TouchLayoutHintPolicy;

public final class TouchLayoutHintPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;

        // 记住九键、副本是出厂的全拼 26 键：会话按九键建，引擎网格和画出的九键一致。
        check("nine_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine, "quanpin",
            "twenty_six_key")), "remembered nine-key seeds the factory quanpin copy");

        // 没记过九键时副本不动，键面和引擎都是 26 键，照常能打字。
        check("twenty_six_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(standard,
            "quanpin", "twenty_six_key")), "standard hint keeps the copy");
        check("twenty_six_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(
            KeyboardLayout.FOURTEEN_KEY_LAYOUT, "quanpin", "twenty_six_key")),
            "other remembered layouts keep the copy");

        // 其他方案的九键要另外的偏好（注音 9 键、日文九键），全拼九键提示不能带过去。
        check("twenty_six_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine,
            "shuangpin", "twenty_six_key")), "shuangpin copy is not seeded");
        check("twenty_six_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine,
            "zhuyin", "twenty_six_key")), "zhuyin copy is not seeded");
        check("twenty_six_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine,
            "japanese", "twenty_six_key")), "japanese copy is not seeded");

        // 副本已经写着别的布局时照原样建会话。
        check("fourteen_key".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine,
            "quanpin", "fourteen_key")), "non-default copy layout is kept");
        check("nine_key_shuangpin".equals(TouchLayoutHintPolicy.startupTouchKeyboardLayout(nine,
            "quanpin", "nine_key_shuangpin")), "nine-key shuangpin copy is kept");
        System.out.println("Android touch layout hint policy passed");
    }
}
