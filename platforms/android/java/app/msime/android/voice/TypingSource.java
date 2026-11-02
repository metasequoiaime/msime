package app.msime.android;

/** Apple-compatible source IDs for aggregate committed-character statistics. */
public enum TypingSource {
    QUANPIN("quanpin"), NINE_KEY("nineKey"), FOURTEEN_KEY("fourteenKey"), SHUANGPIN("shuangpin"),
    ZIRANMA("ziranma"), MICROSOFT("microsoft"), SHOUDAO("shoudao"),
    WUBI("wubi"), JAPANESE("japanese"), KOREAN("korean"), CANTONESE("cantonese"),
    ZHUYIN("zhuyin"), VIETNAMESE("vietnamese"), TIBETAN("tibetan"), STROKE("stroke"), HANDWRITING("handwriting"),
    ENGLISH("english"), LOCAL("local"), AI("ai"), REPLY("reply"),
    VOICE("voice"), UNKNOWN("unknown"), EIGHTEEN_KEY("eighteenKey");

    private final String id;

    TypingSource(String id) { this.id = id; }

    public String id() { return id; }

    public static TypingSource resolve(KeyboardScheme scheme, boolean dedicatedEnglish,
                                       String localMode) {
        if ("temporary_japanese".equals(localMode)) return JAPANESE;
        if (localMode != null && !localMode.isEmpty() && !"none".equals(localMode)) return LOCAL;
        if (dedicatedEnglish) return ENGLISH;
        if (scheme == null) return UNKNOWN;
        return switch (scheme) {
            case QUANPIN -> QUANPIN;
            // 九键双拼用的是九宫格键盘，记到「9 键」：统计按键盘的形态分，它没有自己的一档。
            case QUANPIN_NINE_KEY, NINE_KEY_SHUANGPIN -> NINE_KEY;
            case QUANPIN_FOURTEEN_KEY -> FOURTEEN_KEY;
            case QUANPIN_EIGHTEEN_KEY -> EIGHTEEN_KEY;
            case XIAOHE -> SHUANGPIN;
            case ZIRANMA -> ZIRANMA;
            case MICROSOFT -> MICROSOFT;
            case SHOUDAO -> SHOUDAO;
            case WUBI -> WUBI;
            case JAPANESE, JAPANESE_NINE_KEY -> JAPANESE;
            case KOREAN -> KOREAN;
            // 粤语双拼与粤拼是同一个 Engine 方案，统计记到粤拼，与 iOS 相同。
            case CANTONESE, CANTONESE_SHUANGPIN -> CANTONESE;
            case ZHUYIN, ZHUYIN_NINE_KEY -> ZHUYIN;
            case VIETNAMESE -> VIETNAMESE;
            case TIBETAN -> TIBETAN;
            case STROKE -> STROKE;
            case HANDWRITING -> HANDWRITING;
        };
    }
}
