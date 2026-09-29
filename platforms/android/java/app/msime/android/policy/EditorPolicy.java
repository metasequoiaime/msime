package app.msime.android;

import android.text.InputType;
import android.view.inputmethod.EditorInfo;

public final class EditorPolicy {
    private EditorPolicy() {}
    public static boolean useEngine(int type) {
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation != InputType.TYPE_TEXT_VARIATION_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
            && (type & InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS) == 0;
    }
    public static boolean allowLearning(int options) {
        return (options & EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING) == 0;
    }

    public static boolean prefersLatin(int type) {
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation == InputType.TYPE_TEXT_VARIATION_URI
            || variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
            || (type & InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS) != 0;
    }

    public static EnglishCapitalizationPolicy.Mode capitalizationMode(int type) {
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) {
            return EnglishCapitalizationPolicy.Mode.NONE;
        }
        int variation = type & InputType.TYPE_MASK_VARIATION;
        if (variation == InputType.TYPE_TEXT_VARIATION_URI
                || variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                || variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS) {
            return EnglishCapitalizationPolicy.Mode.NONE;
        }
        if ((type & InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS) != 0) {
            return EnglishCapitalizationPolicy.Mode.ALL_CHARACTERS;
        }
        if ((type & InputType.TYPE_TEXT_FLAG_CAP_WORDS) != 0) {
            return EnglishCapitalizationPolicy.Mode.WORDS;
        }
        if ((type & InputType.TYPE_TEXT_FLAG_CAP_SENTENCES) != 0) {
            return EnglishCapitalizationPolicy.Mode.SENTENCES;
        }
        return EnglishCapitalizationPolicy.Mode.NONE;
    }
}
