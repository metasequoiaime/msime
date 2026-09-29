import app.msime.android.CharacterWidthPolicy;

/**
 * The two vocabularies this host has to read, and the one rule that arbitrates between them.
 *
 * <p>The spellings are not interchangeable: the preferences document writes lowercase
 * `"halfwidth"` / `"fullwidth"` (serde's rename on `CharacterWidthPreference`), and the runtime's
 * view writes capitalised `"Halfwidth"` / `"Fullwidth"` (the `CharacterWidth` enum serialised as
 * it is named). Reading either one with the other's spelling silently yields halfwidth, which
 * looks exactly like a user who never turned it on.
 */
public final class CharacterWidthPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(CharacterWidthPolicy.preferenceIsFullWidth("fullwidth"),
            "the saved preference spells fullwidth in lowercase");
        check(!CharacterWidthPolicy.preferenceIsFullWidth("halfwidth"),
            "halfwidth is the other saved value");
        check(!CharacterWidthPolicy.preferenceIsFullWidth("Fullwidth"),
            "the view's capitalised spelling is not a preference value");
        check(!CharacterWidthPolicy.preferenceIsFullWidth(null)
                && !CharacterWidthPolicy.preferenceIsFullWidth(""),
            "a missing preference is halfwidth, not a crash");

        check(CharacterWidthPolicy.viewIsFullWidth("Fullwidth"),
            "the runtime view spells Fullwidth capitalised");
        check(!CharacterWidthPolicy.viewIsFullWidth("Halfwidth"),
            "Halfwidth is the other view value");
        check(!CharacterWidthPolicy.viewIsFullWidth("fullwidth"),
            "the preference's lowercase spelling is not a view value");
        check(!CharacterWidthPolicy.viewIsFullWidth(null),
            "a view without the field is halfwidth, not a crash");

        // A reload that does not touch 全角输入 leaves a toolbar or chord toggle alone.
        check(!CharacterWidthPolicy.overridesToggle(false, false),
            "saving an unrelated setting must not reset a fullwidth toggle");
        check(!CharacterWidthPolicy.overridesToggle(true, true),
            "an unchanged fullwidth preference must not reset a halfwidth toggle");
        check(CharacterWidthPolicy.overridesToggle(false, true),
            "turning the preference on applies it");
        check(CharacterWidthPolicy.overridesToggle(true, false),
            "turning the preference off applies it");

        check(CharacterWidthPolicy.PREFERENCE_KEY.equals("character_width")
                && CharacterWidthPolicy.VIEW_KEY.equals("character_width"),
            "both documents name the field character_width");
        System.out.println("Android character width policy passed");
    }
}
