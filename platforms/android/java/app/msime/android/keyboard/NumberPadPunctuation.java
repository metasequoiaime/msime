package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 数字九宫格左列符号栏和右列 ！ 的写法跟着当前语言走。每种方案都借这块九宫格之后（{@link KeyboardLayout#drawsNumberPad}），出厂的数字符号表（{@link NineKeySidebarPolicy#DEFAULT_DIGIT_SYMBOLS}）末尾那几个中文全角标点不能原样出现在英文里：
 *
 * <ul>
 * <li>{@link Style#CHINESE}：中文方案打开中文标点（{@link ChineseSymbolFaces#shouldUseChineseFaces}），符号原样显示、原样上屏，与借用之前的中文数字页相同。</li>
 * <li>{@link Style#JAPANESE}：日文方案（罗马字和假名网格）。逗号写成读点「、」，弯引号写成「」『』，其余全角标点日文同样使用，不变。</li>
 * <li>{@link Style#ASCII}：英文模式、本地模式、写半角 ASCII 标点的方案（韩文、越南语、藏文），以及关掉中文标点的中文方案。中文全角标点换成对应的半角 ASCII，没有对应写法的符号（…、—、·、￥ 等）原样保留。</li>
 * </ul>
 *
 * 用户自定义的数字符号表同样按这条规则换写法：表是给所有九宫格数字键盘共用的，在英文里点到「，」只会写出中文标点。四则运算符号本来就是半角，不受影响。
 */
public final class NumberPadPunctuation {
    /** 数字九宫格此刻的标点写法。 */
    public enum Style { CHINESE, JAPANESE, ASCII }

    private static final Map<String, String> ASCII = Map.ofEntries(
        Map.entry("，", ","), Map.entry("。", "."), Map.entry("？", "?"), Map.entry("！", "!"),
        Map.entry("、", ","), Map.entry("；", ";"), Map.entry("：", ":"),
        Map.entry("（", "("), Map.entry("）", ")"), Map.entry("（）", "()"),
        Map.entry("“", "\""), Map.entry("”", "\""), Map.entry("‘", "'"), Map.entry("’", "'"),
        Map.entry("《", "<"), Map.entry("》", ">"), Map.entry("【", "["), Map.entry("】", "]"),
        Map.entry("～", "~"));

    private static final Map<String, String> JAPANESE = Map.ofEntries(
        Map.entry("，", "、"),
        Map.entry("“", "「"), Map.entry("”", "」"), Map.entry("‘", "『"), Map.entry("’", "』"));

    private NumberPadPunctuation() {}

    /**
     * 按当前方案和模式取写法：中文标点生效时是中文（与 123 层、键面提示同一个判断）；日文方案在没有英文模式、没有本地模式时是日文；其余都是半角 ASCII。
     *
     * @param chinesePunctuation 用户的中文标点开关
     * @param dedicatedEnglish 宿主处在英文模式
     * @param scheme View 报告的方案编号，未知时为 -1
     * @param localMode View 的 `local_mode`，没有本地模式时是 `none`
     */
    public static Style style(boolean chinesePunctuation, boolean dedicatedEnglish, int scheme, String localMode) {
        if (ChineseSymbolFaces.shouldUseChineseFaces(dedicatedEnglish, scheme, localMode, chinesePunctuation))
            return Style.CHINESE;
        if (!dedicatedEnglish && scheme == InputSchemeTraits.JAPANESE && "none".equals(localMode))
            return Style.JAPANESE;
        return Style.ASCII;
    }

    /** 一个符号在这种写法下的样子；表里没有的符号原样返回。 */
    public static String face(String symbol, Style style) {
        if (symbol == null) return "";
        return switch (style) {
            case CHINESE -> symbol;
            case JAPANESE -> JAPANESE.getOrDefault(symbol, symbol);
            case ASCII -> ASCII.getOrDefault(symbol, symbol);
        };
    }

    /** 整张符号表换成这种写法；换完撞上前面已有的同一个符号（例如「，」和「、」在 ASCII 下都是 `,`）时只留第一个，免得符号栏里出现两个一样的键。 */
    public static List<String> symbols(List<String> symbols, Style style) {
        if (style == Style.CHINESE) return symbols;
        List<String> result = new ArrayList<>(symbols.size());
        for (String symbol : symbols) {
            String localized = face(symbol, style);
            if (!result.contains(localized)) result.add(localized);
        }
        return List.copyOf(result);
    }

    /** 数字九宫格那一页第五排的括号用不用全角：中文和日文用全角，其余半角（{@link KeyboardLayout#symbolExtraRow}）。 */
    public static boolean fullWidthBrackets(Style style) {
        return style != Style.ASCII;
    }
}
