package app.msime.android;

public final class CustomKeyboardSkinFeedbackSmoke {
    public static void main(String[] args) {
        CustomKeyboardSkin defaults = CustomKeyboardSkin.defaults();
        check("default".equals(defaults.soundPack()) && "none".equals(defaults.pressAnimation()),
            "a design without feedback fields keeps the system key sound and no animation");
        check("msime-woodblock".equals(CustomKeyboardSkin.soundPackValue("msime-woodblock")),
            "built-in pack ids pass");
        check("default".equals(CustomKeyboardSkin.soundPackValue("../evil"))
            && "default".equals(CustomKeyboardSkin.soundPackValue(""))
            && "default".equals(CustomKeyboardSkin.soundPackValue(Integer.valueOf(3)))
            && "default".equals(CustomKeyboardSkin.soundPackValue("x".repeat(65))),
            "unsafe or non-string pack ids fall back");
        for (String animation : new String[] {"none", "bounce", "ripple", "glow", "lift"})
            check(animation.equals(CustomKeyboardSkin.pressAnimationValue(animation)), animation);
        check("none".equals(CustomKeyboardSkin.pressAnimationValue("spin"))
            && "none".equals(CustomKeyboardSkin.pressAnimationValue(Boolean.TRUE)),
            "unknown animations fall back to none");
        CustomKeyboardSkin tuned = defaults.withFeedback("msime-bubble", "ripple");
        check("msime-bubble".equals(tuned.soundPack()) && "ripple".equals(tuned.pressAnimation()),
            "withFeedback sets both fields");
        check("default".equals(defaults.soundPack()), "withFeedback leaves the original alone");
        check(!tuned.key().equals(defaults.key()), "the feedback fields are part of the design key");
        check(tuned.background().equals(defaults.background())
            && tuned.cornerRadius() == defaults.cornerRadius(), "the rest of the design is copied");
        check("default".equals(defaults.withFeedback(null, "glow").soundPack()),
            "a null pack reads as default");
        System.out.println("Android custom skin feedback: sound pack and press animation fields passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
