package app.msime.android;

/**
 * Where this host's fullwidth state comes from, and when a saved setting is allowed to replace it.
 *
 * <p>「全角输入」 in the shared settings is the width a session starts at, not a latch the keyboard
 * owns: the toolbar card and the hardware chord switch it for the session, and the runtime holds
 * the result so Engine commits carry it too. The three rules here are the ones that decide which
 * of those two sources wins, kept out of the service so they can be read and tested on their own.
 */
public final class CharacterWidthPolicy {
    /** The name the shared preferences document uses for the fullwidth state. */
    public static final String PREFERENCE_KEY = "character_width";
    /** The name the runtime's view uses for the same state. */
    public static final String VIEW_KEY = "character_width";

    private CharacterWidthPolicy() {}

    /** Whether the saved preference asks for fullwidth; anything unrecognised means halfwidth. */
    public static boolean preferenceIsFullWidth(String value) {
        return "fullwidth".equals(value);
    }

    /** Whether the runtime reports fullwidth; anything unrecognised means halfwidth. */
    public static boolean viewIsFullWidth(String value) {
        return "Fullwidth".equals(value);
    }

    /**
     * Whether a reloaded preferences document should replace the current width.
     *
     * <p>Only a change to 「全角输入」 itself does. Saving any other setting reloads the whole
     * document, and re-applying it unconditionally would drop a toggle the user had just made.
     */
    public static boolean overridesToggle(boolean previousPreference, boolean nextPreference) {
        return previousPreference != nextPreference;
    }
}
