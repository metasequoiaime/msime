package app.msime.android;

import android.text.InputType;
import android.view.inputmethod.EditorInfo;

public final class EditorPolicy {
    private EditorPolicy() {}
    /** 文本框都走引擎，只有密码框不走。`TYPE_TEXT_FLAG_NO_SUGGESTIONS` 只是「别给输入建议」，不是「不能组字」：Chrome 的地址栏（同时是搜索框）就带着它，排除它会让那里只能打英文字母、切不到中文和日语。 */
    public static boolean useEngine(int type) {
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation != InputType.TYPE_TEXT_VARIATION_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
    }
    public static boolean allowLearning(int options) {
        return (options & EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING) == 0;
    }

    /** A text or numeric password field. */
    public static boolean password(int type) {
        int typeClass = type & InputType.TYPE_MASK_CLASS;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        if (typeClass == InputType.TYPE_CLASS_NUMBER)
            return variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD;
        return typeClass == InputType.TYPE_CLASS_TEXT
            && (variation == InputType.TYPE_TEXT_VARIATION_PASSWORD
                || variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
                || variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD);
    }

    /**
     * Whether the key heatmap must not count presses in this field: a password field, or one that asked for no personalised learning (an incognito tab, a private field). The same signals that keep the Engine from learning keep the statistics from counting.
     */
    public static boolean excludesKeyStatistics(int type, int options) {
        return password(type) || !allowLearning(options);
    }

    /** 进框时临时切成英文的输入框：只看网址、邮箱、密码这几种 variation。`TYPE_TEXT_FLAG_NO_SUGGESTIONS` 不算：React Native 的 `autoCorrect={false}`、Flutter 的 `enableSuggestions: false` 和不少网页输入框都带着它，算进来的话这些聊天框、搜索框每次进框都是英文，「默认中英文」和按应用记住的模式都被盖掉（#5998）。 */
    public static boolean prefersLatin(int type) {
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation == InputType.TYPE_TEXT_VARIATION_URI
            || variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
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
