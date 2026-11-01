package app.msime.android;

import android.text.InputType;
import android.view.inputmethod.EditorInfo;

public final class EditorPolicy {
    private EditorPolicy() {}
    /** 文本框都走引擎，只有密码框不走。`TYPE_TEXT_FLAG_NO_SUGGESTIONS` 只是「别给输入建议」，不是「不能组字」：Chrome 的地址栏（同时是搜索框）就带着它，排除它会让那里只能打英文字母、切不到中文和日语。 */
    public static boolean useEngine(int type) {
        // 自定义聊天控件（例如直播间弹幕）常用 TYPE_NULL，但仍提供可组字的 InputConnection；把它当作非文本框会让九键和 26 键都落入「直接输入」而无法输入中文。
        if (type == 0) return true;
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation != InputType.TYPE_TEXT_VARIATION_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            && variation != InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
    }

    /** 数字和电话输入框不启用 Engine，但仍应显示输入法自己的数字九宫格。 */
    public static boolean numberInput(int type) {
        int typeClass = type & InputType.TYPE_MASK_CLASS;
        return typeClass == InputType.TYPE_CLASS_NUMBER
            || typeClass == InputType.TYPE_CLASS_PHONE;
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

    /** 进框时临时切成英文的输入框：只看邮箱、密码这几种 variation，数字密码也算（{@link #password}）。数字密码框不走引擎、首页是数字九宫格，但「返回原键盘」切到的字母层按中文键面画的话就是上次记住的九键、注音或笔画，密码框里打不出字母；与文字密码一样临时切英文，字母层就是 26 键，离开时恢复原来的中英（鸿蒙的 `PATTERN_PASSWORD_NUMBER` 同样归入密码）。网址框不算（#9725）：Android 没有 iOS 的 `.webSearch`，Firefox、Chrome 这些浏览器的地址栏同时是搜索框，却只能声明成 `TYPE_TEXT_VARIATION_URI`，算进来的话九宫格用户每次点地址栏都被切成 26 键英文，「默认中英文」和按应用记住的模式都被盖掉；网址框按默认模式进框，中文模式下打 `www.`、`http:` 由引擎的网址模式接管。`TYPE_TEXT_FLAG_NO_SUGGESTIONS` 不算：React Native 的 `autoCorrect={false}`、Flutter 的 `enableSuggestions: false` 和不少网页输入框都带着它，算进来的话这些聊天框、搜索框每次进框都是英文，「默认中英文」和按应用记住的模式都被盖掉（#5998）。 */
    public static boolean prefersLatin(int type) {
        if (password(type)) return true;
        if ((type & InputType.TYPE_MASK_CLASS) != InputType.TYPE_CLASS_TEXT) return false;
        int variation = type & InputType.TYPE_MASK_VARIATION;
        return variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS
            || variation == InputType.TYPE_TEXT_VARIATION_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            || variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
    }

    /** 网址输入框（`TYPE_TEXT_VARIATION_URI`），包括浏览器地址栏。进框不切英文（见 {@link #prefersLatin}），但英文模式下照样不给英文建议和纠错、不自动大写：网址不是词典里的单词，也不是句子。 */
    public static boolean uri(int type) {
        return (type & InputType.TYPE_MASK_CLASS) == InputType.TYPE_CLASS_TEXT
            && (type & InputType.TYPE_MASK_VARIATION) == InputType.TYPE_TEXT_VARIATION_URI;
    }

    /** 打到 `xxx@` 时可以给邮箱后缀（{@link EmailSuffixPolicy}，#6147）的输入框：除了密码框（文字和数字密码的各种 variation）都给，不只是系统标记为邮箱的输入框，聊天框、搜索框里写邮箱地址同样能用。密码框里不读上文，与算式结果（`ImeCalculator`）同一个隐私口径。 */
    public static boolean offersEmailSuffixes(int type) {
        return !password(type);
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
