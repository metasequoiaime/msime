package app.msime.android;

/**
 * 大屏单手侧边数字键盘（#10203）的显示条件和偏好读法：五条全满足才画，任何一条不满足时空出来的那一侧照旧空着；偏好不认识的写法按空着读。
 */
public final class OneHandSidePadPolicySmoke {
    private static final OneHandSidePadPolicy.Panel PAD = OneHandSidePadPolicy.Panel.NUMBER_PAD;
    private static final OneHandSidePadPolicy.Panel OFF = OneHandSidePadPolicy.Panel.OFF;
    private static final KeyboardLayout.Layer LETTERS = KeyboardLayout.Layer.LETTERS;
    private static final KeyboardLayout.Layer SYMBOLS = KeyboardLayout.Layer.SYMBOLS;
    private static final int NINE = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;

    public static void main(String[] args) {
        parsing();
        everyCondition();
        layouts();
        System.out.println("OneHandSidePadPolicySmoke ok");
    }

    private static void parsing() {
        check(OneHandSidePadPolicy.PREFERENCE_KEY.equals("touch_one_handed_side_panel"), "key matches client-core");
        check(OneHandSidePadPolicy.Panel.OFF.wire.equals("off") && PAD.wire.equals("number_pad"),
            "wire names match client-core");
        check(OneHandSidePadPolicy.parse("number_pad") == PAD, "number_pad reads as the number pad");
        check(OneHandSidePadPolicy.parse("off") == OFF, "off reads as off");
        check(OneHandSidePadPolicy.parse(null) == OFF, "a missing or non-string value reads as off");
        check(OneHandSidePadPolicy.parse("text_edit") == OFF, "a later panel this version does not know reads as off");
        check(OneHandSidePadPolicy.parse("NUMBER_PAD") == OFF, "the wire name is exact");
    }

    private static void everyCondition() {
        check(OneHandSidePadPolicy.shown(PAD, "right", 300, NINE, LETTERS, false), "keys on the right draw the pad");
        check(OneHandSidePadPolicy.shown(PAD, "left", 176, NINE, LETTERS, false), "keys on the left draw the pad");
        // 1. 偏好
        check(!OneHandSidePadPolicy.shown(OFF, "right", 300, NINE, LETTERS, false), "the preference off draws nothing");
        // 2. 单手此刻生效（分离式、浮动时 effectiveOneHanded 已是 off）
        check(!OneHandSidePadPolicy.shown(PAD, "off", 300, NINE, LETTERS, false), "one-handed off draws nothing");
        check(!OneHandSidePadPolicy.shown(PAD, "", 300, NINE, LETTERS, false), "an unknown mode draws nothing");
        // 3. 大屏且放得下
        check(!OneHandSidePadPolicy.shown(PAD, "right", 0, NINE, LETTERS, false), "a phone or a too-narrow window draws nothing");
        // 4. 九键一族的字母层
        check(!OneHandSidePadPolicy.shown(PAD, "right", 300, NINE, SYMBOLS, false), "the 123 layer draws nothing");
        check(!OneHandSidePadPolicy.shown(PAD, "right", 300, NINE, LETTERS, true), "a number pad on the main keys draws nothing");
    }

    private static void layouts() {
        check(OneHandSidePadPolicy.shown(PAD, "right", 300, KeyboardLayout.STROKE_LAYOUT, LETTERS, false),
            "strokes are in the nine-key family");
        int[] excluded = {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT,
            KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT,
            KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT};
        for (int layout : excluded) {
            check(!OneHandSidePadPolicy.shown(PAD, "right", 300, layout, LETTERS, false),
                "layout " + layout + " is not in the nine-key family");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
