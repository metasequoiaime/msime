package app.msime.android;

/** Host-side gate for sending an uppercase letter to the Engine as composition helpcode. */
public final class ChineseHelpcodePolicy {
    private ChineseHelpcodePolicy() { }

    public static boolean eligible(boolean dedicatedEnglish, String editingText, int scheme,
            String localMode) {
        return !dedicatedEnglish && editingText != null && !editingText.isEmpty()
            && "none".equals(localMode) && (scheme == 0 || scheme == 1);
    }

    public static boolean entersHelpcode(boolean dedicatedEnglish, boolean shifted,
            String editingText, int scheme, String localMode) {
        return shifted && eligible(dedicatedEnglish, editingText, scheme, localMode);
    }
}
