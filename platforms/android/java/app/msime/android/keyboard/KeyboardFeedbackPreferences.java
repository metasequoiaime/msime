package app.msime.android;

/** Persisted Android equivalents of Apple's keyboard sound and haptic settings. */
public final class KeyboardFeedbackPreferences {
    /**
     * 按键振动强度。轻、中、强由 {@link KeyboardHaptics} 换成真实的振动；「跟随系统」不自己振动，交给系统的触感反馈设置。
     *
     * <p>默认是中；缺省和不认识的值都按中读。
     */
    public enum HapticStrength {
        LIGHT("light", "轻", "轻"), MEDIUM("medium", "中", "中"), STRONG("strong", "强", "强"),
        SYSTEM("system", "跟随系统", "系统");

        private final String id;
        private final String title;
        private final String shortTitle;

        HapticStrength(String id, String title, String shortTitle) {
            this.id = id;
            this.title = title;
            this.shortTitle = shortTitle;
        }

        public String id() { return id; }
        /** 设置里显示的名字。 */
        public String title() { return title; }
        /** 功能面板磁贴上「振动强度」后面那一截，磁贴放不下「跟随系统」四个字。 */
        public String shortTitle() { return shortTitle; }
    }

    private KeyboardFeedbackPreferences() {}

    public static HapticStrength strength(String value) {
        for (HapticStrength strength : HapticStrength.values())
            if (strength.id().equals(value)) return strength;
        return HapticStrength.MEDIUM;
    }

    /** 偏好值是不是认识的强度；设置页写入前用它拒绝其他值。 */
    public static boolean known(String value) {
        for (HapticStrength strength : HapticStrength.values())
            if (strength.id().equals(value)) return true;
        return false;
    }
}
