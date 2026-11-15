package app.msime.android;

/** 拼音九键组字时 1 键的动作。 */
public final class NineKeyCompositionActionPolicy {
    public static final String SEPARATE = "separate";
    public static final String REWRITE = "rewrite";

    private NineKeyCompositionActionPolicy() {}

    /** 只接受设置页写入的两种值，缺失或非法值回到原来的「分词」。 */
    public static String normalize(String value) {
        return REWRITE.equals(value) ? REWRITE : SEPARATE;
    }

    public static String label(String value) {
        return REWRITE.equals(normalize(value)) ? "重输" : "分词";
    }
}
