package app.msime.android;

/**
 * How this host drives the Vietnamese scheme, whose Engine composes a word from Telex or VNI keys.
 *
 * <p>The tone and vowel rules belong to the Engine. A Vietnamese letter keeps its case all the way into the word, so Shift and Caps Lock are real capitalisation here: the touch Shift is the letter case rather than the language switch, and a hardware key sends the character the keyboard produced, Caps Lock included. The word is written as it composes, the view's `editing_text` is what is marked inline, and there is no candidate list.
 */
public final class VietnameseInputPolicy {
    /** `SchemeType::Vietnamese`: the value `View.scheme` and `commit_context.scheme` carry for this scheme. */
    public static final int VIETNAMESE_SCHEME = InputSchemeTraits.VIETNAMESE;

    private VietnameseInputPolicy() {}

    /** Whether the Engine composes Vietnamese: the Vietnamese scheme outside dedicated English. Vietnamese has no local modes. */
    public static boolean active(int scheme, boolean dedicatedEnglish) {
        return scheme == VIETNAMESE_SCHEME && !dedicatedEnglish;
    }
}
