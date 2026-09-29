package app.msime.android;

/**
 * Simplified to Traditional output, through the conversion every other host uses.
 *
 * <p>This host used to call {@code android.icu.text.Transliterator("Simplified-Traditional")},
 * which converts one character at a time: 头发 came out 頭發 rather than 頭髮, because whether 发
 * is 發 or 髮 is a property of the word and not of the character. Windows and Linux both go through
 * the shared OpenCC s2t tables, which are phrase-level, and those tables are compiled into the
 * shared library rather than loaded from a resource directory.
 *
 * <p>The transliterator also arrived in API 29 while this host declares minSdk 28, so on an API 28
 * device the 繁体输出 setting did nothing at all and said nothing about it. The shared converter
 * has no such floor.
 */
public final class AndroidChineseTextConversion {
    private AndroidChineseTextConversion() {}

    public static String outputString(String text, boolean traditional, boolean dedicatedEnglish,
                                      int scheme, String localMode) {
        boolean applies = ChineseOutputPolicy.applies(dedicatedEnglish, scheme, localMode);
        return ChineseOutputPolicy.output(text, traditional, applies,
            NativeClient::simplifiedToTraditional);
    }
}
