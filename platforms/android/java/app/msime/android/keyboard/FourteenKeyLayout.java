package app.msime.android;

import java.util.List;

/**
 * 全拼 14 键的键面：QWERTY 的位置不变，相邻两个字母合成一个键（`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`），L、M 单独一键。
 *
 * <p>点一下输入的是这一组，不是某个字母：宿主经 `msime_client_grid_key` 送这一组的首字母（组码 `q e t u o a d g j l z c b m`），引擎按组码网格解码，组里哪个字母都会被归到这一组。键表与 iOS、HarmonyOS 的 `FourteenKeyLayout` 和引擎 `KeyGrid::FourteenKey` 是同一张。宿主不保存拼音表，读音和拼音选择条都来自引擎的 `View`。
 */
public final class FourteenKeyLayout {
    /**
     * 一个 14 键的键。
     *
     * @param letters 这一组的小写字母，按键面从左到右
     */
    public record Key(String letters) {
        public Key {
            if (letters == null || letters.isEmpty() || letters.length() > 2)
                throw new IllegalArgumentException("A fourteen-key key carries one or two letters");
        }

        /** 点按送给引擎的字母：这一组的首字母，也就是这一组的组码。 */
        public char input() { return letters.charAt(0); }
    }

    private static final List<List<Key>> ROWS = List.of(
        List.of(new Key("qw"), new Key("er"), new Key("ty"), new Key("ui"), new Key("op")),
        List.of(new Key("as"), new Key("df"), new Key("gh"), new Key("jk"), new Key("l")),
        List.of(new Key("zx"), new Key("cv"), new Key("bn"), new Key("m")));

    /** a 到 z 各自落在哪一组（组的首字母），与引擎 `KeyGrid::FourteenKey` 的编码表逐字相同。 */
    public static final String GROUP_CODES = "abcdedggujjlmbooqeatucqztz";

    /** 一个字母键（两个字母）在一行里的宽度份额；一行五个键正好是 26 键第一行十个字母的宽度。 */
    public static final float LETTER_WEIGHT = 2f;

    /** 符号层（123 层和借用的九宫格数字键面）上返回 14 键的键面。 */
    public static final String LAYER_TITLE = "14键";

    private FourteenKeyLayout() {}

    /** 三行字母键：第 1 行五个、第 2 行五个（不缩进）、第 3 行四个，第 3 行两端另有分词键和 ⌫。 */
    public static List<List<Key>> rows() { return ROWS; }

    /** `letter`（a–z，大小写都行）所在那一组的首字母；不是字母时返回 0。 */
    public static char groupCode(char letter) {
        char lower = Character.toLowerCase(letter);
        if (lower < 'a' || lower > 'z') return 0;
        return GROUP_CODES.charAt(lower - 'a');
    }

    /** 键面：这一组的大写字母，没有角标（中文键盘的字母键一律画大写，送进引擎的仍是小写）。 */
    public static String face(Key key) {
        return key.letters().toUpperCase(java.util.Locale.ROOT);
    }

    /** 读屏念的描述（不含「按键 」前缀）：两个字母之间隔一个空格，「Q W」。单字母键的完整描述见 {@link #accessibilityLabel}。 */
    public static String description(Key key) {
        String face = face(key);
        return face.length() == 1 ? face : face.charAt(0) + " " + face.charAt(1);
    }

    /** 完整的无障碍标签：两字母键是「按键 Q W」，单字母键是「字母 L」，与 26 键字母键的念法一致。 */
    public static String accessibilityLabel(Key key) {
        return key.letters().length() == 1 ? "字母 " + face(key) : "按键 " + description(key);
    }

    /** 长按弹出的字母：两字母键弹出这两个小写字母，L、M 只有一个字母，不弹。选中后先结束组字，再上屏这个字母，与九键长按相同。 */
    public static String holdLetters(Key key) {
        return key.letters().length() > 1 ? key.letters() : "";
    }

    /** 按键热力图的键位 id：`FourteenQW`、`FourteenL`，与 client-core `KEY_IDS` 逐字一致。 */
    public static String keyId(Key key) {
        return "Fourteen" + face(key);
    }

    /** 第 3 行左端的键面：组字时是「分词」（送 `'`，在已打的组码末尾定音节分界），空闲时是「符」（打开符号面板）。照搬九键 1 键。 */
    public static String separatorFace(boolean composing) {
        return composing ? "分词" : "符";
    }

    /** 与 {@link #separatorFace} 对应的描述（不含「按键 」前缀），与九键 1 键的念法相同。 */
    public static String separatorDescription(boolean composing) {
        return composing ? "分词，在这里断开音节" : "符号";
    }
}
