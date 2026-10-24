package app.msime.android;

import java.util.List;

/** Android 十八键全拼的键面；字母组交给共享引擎消歧。 */
public final class EighteenKeyLayout {
    private static final List<List<FourteenKeyLayout.Key>> ROWS = List.of(
        List.of(key("q"), key("we"), key("rt"), key("y"), key("u"), key("io"), key("p")),
        List.of(key("a"), key("sd"), key("fg"), key("h"), key("jk"), key("l")),
        List.of(key("z"), key("xc"), key("v"), key("bn"), key("m")));

    public static final String LAYER_TITLE = "18键";

    private EighteenKeyLayout() {}

    private static FourteenKeyLayout.Key key(String letters) {
        return new FourteenKeyLayout.Key(letters);
    }

    /** 三排依次为七、六、五个字母键；第二排居中，第三排两端留给分词和删除。 */
    public static List<List<FourteenKeyLayout.Key>> rows() { return ROWS; }

    /** 独立的十八键热力图键位，不混入十四键或二十六键。 */
    public static String keyId(FourteenKeyLayout.Key key) {
        return "Eighteen" + FourteenKeyLayout.face(key);
    }
}
