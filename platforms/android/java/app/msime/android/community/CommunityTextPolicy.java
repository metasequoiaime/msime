package app.msime.android;

/** Shared validation rules for text fields exchanged with the community service. */
public final class CommunityTextPolicy {
    private CommunityTextPolicy() {}

    /** Return whether text contains a disallowed Unicode control character. */
    public static boolean hasDisallowedControl(String text, boolean multiline) {
        for (int index = 0; index < text.length();) {
            int codePoint = text.codePointAt(index);
            if (Character.isISOControl(codePoint)
                    && !(multiline && (codePoint == '\n' || codePoint == '\t'))) return true;
            index += Character.charCount(codePoint);
        }
        return false;
    }
}
