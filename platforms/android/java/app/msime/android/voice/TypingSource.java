package app.msime.android;

/** Apple-compatible source IDs for aggregate committed-character statistics. */
public enum TypingSource {
    QUANPIN("quanpin"), NINE_KEY("nineKey"), SHUANGPIN("shuangpin"),
    ZIRANMA("ziranma"), MICROSOFT("microsoft"), SHOUDAO("shoudao"),
    WUBI("wubi"), JAPANESE("japanese"), KOREAN("korean"), CANTONESE("cantonese"),
    ZHUYIN("zhuyin"), VIETNAMESE("vietnamese"), STROKE("stroke"), HANDWRITING("handwriting"),
    ENGLISH("english"), LOCAL("local"), AI("ai"), REPLY("reply"),
    VOICE("voice"), UNKNOWN("unknown");

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
            case QUANPIN_NINE_KEY -> NINE_KEY;
            case XIAOHE -> SHUANGPIN;
            case ZIRANMA -> ZIRANMA;
            case MICROSOFT -> MICROSOFT;
            case SHOUDAO -> SHOUDAO;
            case WUBI -> WUBI;
            case JAPANESE, JAPANESE_NINE_KEY -> JAPANESE;
            case KOREAN -> KOREAN;
            case CANTONESE -> CANTONESE;
            case ZHUYIN -> ZHUYIN;
            case VIETNAMESE -> VIETNAMESE;
            case STROKE -> STROKE;
            case HANDWRITING -> HANDWRITING;
        };
    }
}
