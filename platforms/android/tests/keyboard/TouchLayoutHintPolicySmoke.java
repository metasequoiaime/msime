import app.msime.android.InputSchemeTraits;
import app.msime.android.KeyboardLayout;
import app.msime.android.TouchLayoutHintPolicy;

public final class TouchLayoutHintPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;

        // 没有视图时按上次确认的键面；从未记过时仍是 26 键。
        check(TouchLayoutHintPolicy.choose(nine, standard, false, false, false, false,
            InputSchemeTraits.QUANPIN) == nine, "missing view keeps nine-key hint");
        check(TouchLayoutHintPolicy.choose(standard, standard, false, false, false, false,
            InputSchemeTraits.QUANPIN) == standard, "missing hint keeps standard layout");

        // 实时偏好未到达时，缓存里的全拼 26 键不能覆盖九键提示。
        check(TouchLayoutHintPolicy.choose(nine, standard, true, false, false, false,
            InputSchemeTraits.QUANPIN) == nine, "stale quanpin view does not overwrite hint");
        check(TouchLayoutHintPolicy.choose(nine, standard, true, false, false, false,
            InputSchemeTraits.SHUANGPIN) == nine, "stale shuangpin view does not overwrite hint");
        check(TouchLayoutHintPolicy.choose(nine, standard, true, false, false, false, -1) == nine,
            "unknown scheme keeps the remembered layout until preferences arrive");

        // 14/18 键、九键视图和实时偏好都应直接采用引擎返回值。
        check(TouchLayoutHintPolicy.choose(nine, KeyboardLayout.FOURTEEN_KEY_LAYOUT, true, false,
            false, false, InputSchemeTraits.QUANPIN) == KeyboardLayout.FOURTEEN_KEY_LAYOUT,
            "fourteen-key runtime view wins");
        check(TouchLayoutHintPolicy.choose(standard, nine, true, false, false, false,
            InputSchemeTraits.QUANPIN) == nine, "nine-key runtime view wins");
        check(TouchLayoutHintPolicy.choose(nine, standard, true, true, false, false,
            InputSchemeTraits.QUANPIN) == standard, "live preferences win");

        // 本地模式、其他方案和专用英文不能误借中文九键提示。
        check(TouchLayoutHintPolicy.choose(nine, standard, true, false, false, true,
            InputSchemeTraits.QUANPIN) == standard, "local mode keeps standard layout");
        check(TouchLayoutHintPolicy.choose(nine, standard, true, false, false, false,
            InputSchemeTraits.WUBI) == standard, "other scheme keeps runtime layout");
        check(TouchLayoutHintPolicy.choose(nine, nine, true, false, true, false,
            InputSchemeTraits.QUANPIN) == standard, "dedicated English keeps standard layout");
        System.out.println("Android touch layout hint policy passed");
    }
}
